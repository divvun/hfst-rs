"""The parity check: the package against the native library.

js/test/parity-fixtures.mjs writes parity.json beside the fixtures, holding
what the native hfst library gives for each of them, what native divvun-fst's
lookup gives, and what this checkout's hfst tokenise prints. The npm package's
parity suite (js/test/parity.ts) runs the same file. Here every entry runs
through the built package and its result is compared with the native one,
value for value: both sides hold each weight as the float that holds its f32
exactly, so no tolerance is needed, and any would hide a weight computed
differently.

The options in parity.json are the npm package's, in camelCase and with time
limits in milliseconds; they are read as the dataclasses that stand for them.
"""

from __future__ import annotations

import functools
from collections.abc import Callable
from typing import Any, TypeVar

import pytest

import divvun_hfst
from conftest import Fixtures

T = TypeVar("T")


@pytest.fixture(scope="module")
def expected(fixtures: Fixtures) -> dict[str, list[dict[str, Any]]]:
    native: dict[str, list[dict[str, Any]]] = fixtures.json("parity.json")
    return native


def loaded(fixtures: Fixtures, read: Callable[[bytes], T]) -> Callable[[str], T]:
    """`read` of each fixture, read once."""
    return functools.cache(lambda name: read(fixtures.bytes(name)))


def snake(options: dict[str, Any]) -> dict[str, Any]:
    """The npm package's options under the dataclasses' names and units."""
    kwargs: dict[str, Any] = {}
    for key, value in options.items():
        name = "".join(f"_{c.lower()}" if c.isupper() else c for c in key)
        kwargs[name] = value / 1000 if name == "time_limit" else value
    return kwargs


def camel(name: str) -> str:
    """A snake_case name as the npm package spells it."""
    first, *rest = name.split("_")
    return first + "".join(word.title() for word in rest)


def results(found: list[divvun_hfst.LookupResult]) -> list[dict[str, Any]]:
    return [{"output": r.output, "symbols": list(r.symbols), "weight": r.weight} for r in found]


def differences(entries: list[dict[str, Any]], run: Callable[[dict[str, Any]], tuple[Any, Any]]) -> list[str]:
    """Each entry whose result differs from the native one. A section with no
    entries fails: the native expectations are missing, not met."""
    assert entries, "parity.json holds no entries of this kind"
    found = []
    for entry in entries:
        ours, native = run(entry)
        if ours != native:
            what = entry.get("name", entry.get("input", entry.get("word")))
            found.append(f"{entry.get('file')} {what!r}: {ours!r}, native {native!r}")
    return found


# [spec:hfst:req:python.parity/test]
def test_lookups_are_the_native_librarys(fixtures: Fixtures, expected: dict[str, Any]) -> None:
    streams = loaded(fixtures, divvun_hfst.read_transducers)
    members: dict[str, int] = {}
    for entry in expected["lookup"]:
        members[entry["file"]] = max(members.get(entry["file"], 0), entry["member"] + 1)
    assert {f: len(streams(f)) for f in members} == members

    def run(entry: dict[str, Any]) -> tuple[Any, Any]:
        transducer = streams(entry["file"])[entry["member"]]
        return results(transducer.lookup(entry["input"], limit=entry["limit"])), entry["results"]

    assert differences(expected["lookup"], run) == []


# [spec:hfst:req:python.parity/test]
def test_optimised_lookups_are_native_divvun_fsts(fixtures: Fixtures, expected: dict[str, Any]) -> None:
    transducers = loaded(fixtures, divvun_hfst.read_optimised_transducer)

    def run(entry: dict[str, Any]) -> tuple[Any, Any]:
        transducer = transducers(entry["file"])
        return results(transducer.lookup(entry["input"], limit=entry["limit"])), entry["results"]

    assert differences(expected["optimised"], run) == []


# [spec:hfst:req:python.parity/test]
def test_tokens_are_the_native_librarys(fixtures: Fixtures, expected: dict[str, Any]) -> None:
    tokenisers = loaded(fixtures, divvun_hfst.read_tokeniser)

    def run(entry: dict[str, Any]) -> tuple[Any, Any]:
        text = entry["text"]
        options = divvun_hfst.TokeniseOptions(**snake(entry["options"]))
        tokens = tokenisers(entry["file"]).tokenise(text, options)
        for token in tokens:
            assert text[token.start : token.end] == token.surface, (entry["name"], token)
        ours = [
            {
                "surface": t.surface,
                "analysed": t.analysed,
                "readings": [{"analysis": r.analysis, "weight": r.weight} for r in t.readings],
            }
            for t in tokens
        ]
        return ours, entry["tokens"]

    assert differences(expected["tokens"], run) == []


# [spec:hfst:req:python.parity/test]
def test_formats_are_what_hfst_tokenise_prints(fixtures: Fixtures, expected: dict[str, Any]) -> None:
    tokenisers = loaded(fixtures, divvun_hfst.read_tokeniser)

    def run(entry: dict[str, Any]) -> tuple[Any, Any]:
        options = divvun_hfst.FormatOptions(**snake(entry["options"]))
        printed = tokenisers(entry["file"]).format(entry["text"], entry["format"], options)
        return printed, entry["printed"]

    assert differences(expected["formats"], run) == []


# [spec:hfst:req:python.parity/test]
def test_acceptor_lookups_are_the_native_librarys(fixtures: Fixtures, expected: dict[str, Any]) -> None:
    models = loaded(fixtures, divvun_hfst.read_dhfst)

    def run(entry: dict[str, Any]) -> tuple[Any, Any]:
        model = models(entry["file"])
        assert isinstance(model, divvun_hfst.DhfstAcceptor)
        return model.lookup(entry["word"]), entry["weight"]

    assert differences(expected["acceptors"], run) == []


# [spec:hfst:req:python.parity/test]
def test_dhfst_descriptions_are_the_native_librarys(fixtures: Fixtures, expected: dict[str, Any]) -> None:
    def run(entry: dict[str, Any]) -> tuple[Any, Any]:
        model = divvun_hfst.read_dhfst(fixtures.bytes(entry["file"]))
        described: dict[str, Any] = {
            "file": entry["file"],
            "type": model.type,
            "version": model.version,
            "symbols": list(model.symbols),
            "meta": model.meta,
        }
        if isinstance(model, divvun_hfst.DhfstAcceptor):
            info = model.info
            fields = [name for name in dir(info) if not name.startswith("_")]
            described["info"] = {camel(name): getattr(info, name) for name in fields}
        else:
            described["stateCount"] = model.state_count
        return described, entry

    assert differences(expected["models"], run) == []
