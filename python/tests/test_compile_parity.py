"""The package against the hfst tools: each compiler, operation and writer
gives, through the built package, the bytes or text the tool writes of the
inputs the repository's tests give it. compile_parity.py says how the cases
are read."""

from __future__ import annotations

import warnings
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import divvun_hfst
from compile_parity import Case, Runner, att_pieces, check, stream
from conftest import Fixtures


@pytest.fixture(scope="module")
def manifest(fixtures: Fixtures) -> dict[str, Any]:
    loaded: dict[str, Any] = fixtures.json("compile-parity.json")
    return loaded


@pytest.fixture(scope="module")
def runner(fixtures: Fixtures, manifest: dict[str, Any]) -> Runner:
    return Runner(fixtures, manifest)


# [spec:hfst:req:python.compile-parity/test]
def test_each_type_is_written_as_fst2fst_writes(
    manifest: dict[str, Any], runner: Runner, fixtures: Fixtures
) -> None:
    for name in ("words", "words.foma", "words.olw", "words.olu"):
        assert runner.one(name).to_bytes() == fixtures.bytes(runner.inputs[name]), name

    def run(case: Case) -> bytes:
        t = runner.one(case["input"])
        return t.to_bytes(case["type"], use_backend_format=case["use_backend_format"])

    check(manifest, {"to_bytes"}, run, runner.expected)


# [spec:hfst:req:python.compile-parity/test]
def test_operations_write_what_the_tools_write(manifest: dict[str, Any], runner: Runner) -> None:
    def unary(case: Case) -> bytes:
        ts = runner.all(case["input"])
        for t in ts:
            assert runner.call(t, case["method"], case["kwargs"]) is None
        return stream(ts)

    def binary(case: Case) -> bytes:
        t = runner.one(case["first"])
        other = t if case["second"] is None else runner.one(case["second"])
        runner.call(t, case["method"], {"$args": [other], **case["kwargs"]})
        return t.to_bytes()

    def compose_intersect(case: Case) -> bytes:
        t = runner.one(case["input"])
        rules = [runner.one(name) for name in case["rules"]]
        runner.call(t, "compose_intersect", {"$args": [rules], **case["kwargs"]})
        return t.to_bytes()

    def guessify(case: Case) -> bytes:
        t = runner.one(case["input"])
        generator = runner.call(t, "guessify", case["kwargs"])
        return t.to_bytes() + (b"" if generator is None else generator.to_bytes())

    runs: dict[str, Callable[[Case], bytes]] = {
        "unary": unary,
        "binary": binary,
        "compose_intersect": compose_intersect,
        "guessify": guessify,
    }
    check(manifest, set(runs), lambda case: runs[case["kind"]](case), runner.expected)


# [spec:hfst:req:python.compile-parity/test]
def test_writers_write_what_the_tools_write(
    manifest: dict[str, Any], runner: Runner, tmp_path: Path
) -> None:
    def text(case: Case) -> str:
        written: str = runner.call(runner.one(case["input"]), case["method"], case["kwargs"])
        return written

    check(manifest, {"text"}, text, runner.printed)

    def dhfst(case: Case) -> bytes:
        t = divvun_hfst.read_transducer(runner.fixtures.bytes(case["source"]))
        written: bytes = runner.call(t, case["method"], case["kwargs"])
        return written

    check(manifest, {"dhfst"}, dhfst, runner.expected)

    def bhfst(case: Case) -> bytes:
        return divvun_hfst.pack_bhfst(
            runner.fixtures.bytes(case["acceptor"]),
            runner.fixtures.bytes(case["error_model"]),
            runner.fixtures.text(case["metadata"]),
            **{key: runner.value(v) for key, v in case["kwargs"].items()},
        )

    check(manifest, {"bhfst"}, bhfst, runner.expected)

    def thfst(case: Case) -> None:
        directory = tmp_path / Path(case["expected"]).name
        runner.one(case["input"]).write_thfst(directory)
        for member in ("alphabet", "index", "transition"):
            expected = runner.fixtures.bytes(f"{case['expected']}/{member}")
            assert (directory / member).read_bytes() == expected, (case["tool"], member)

    check(manifest, {"thfst"}, thfst, runner.expected)


# [spec:hfst:req:python.compile-parity/test]
def test_compilers_write_what_the_tools_write(manifest: dict[str, Any], runner: Runner) -> None:
    def sources(case: Case) -> list[tuple[str, str]]:
        return [(name, runner.fixtures.text(name)) for name in case["sources"]]

    def regex(case: Case) -> bytes:
        [(name, text)] = sources(case)
        options = divvun_hfst.RegexOptions(**case["options"])
        return divvun_hfst.regex(text, options, name=name).to_bytes()

    def lexc(case: Case) -> bytes:
        named = sources(case)
        options = divvun_hfst.LexcOptions(**case["options"])
        t = divvun_hfst.compile_lexc([t for _, t in named], options, names=[n for n, _ in named])
        return t.to_bytes()

    def twolc(case: Case) -> bytes:
        options = divvun_hfst.TwolcOptions(**case["options"])
        return stream(divvun_hfst.compile_twolc(case["text"], options))

    def pmatch(case: Case) -> bytes:
        [(name, text)] = sources(case)
        options = divvun_hfst.PmatchOptions(**case["options"])
        return divvun_hfst.compile_pmatch(text, options, name=name)

    def strings(case: Case) -> bytes:
        [(name, text)] = sources(case)
        options = divvun_hfst.StringsOptions(**case["options"])
        if options.disjunct_strings:
            return divvun_hfst.compile_strings(text, options, name=name).to_bytes()
        lines = [line + "\n" for line in text.splitlines() if line]
        return stream([divvun_hfst.compile_strings(line, options, name=name) for line in lines])

    def att(case: Case) -> bytes:
        [(name, text)] = sources(case)
        options = divvun_hfst.AttOptions(**case["options"])
        if options.disjunct:
            return divvun_hfst.parse_att(text, options, name=name).to_bytes()
        return stream([divvun_hfst.parse_att(p, options, name=name) for p in att_pieces(text)])

    def prolog(case: Case) -> bytes:
        [(name, text)] = sources(case)
        options = divvun_hfst.PrologOptions(**case["options"])
        return divvun_hfst.parse_prolog(text, options, name=name).to_bytes()

    runs: dict[str, Callable[[Case], bytes]] = {
        "regex": regex,
        "lexc": lexc,
        "twolc": twolc,
        "pmatch2fst": pmatch,
        "strings": strings,
        "att": att,
        "prolog": prolog,
    }
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", divvun_hfst.CompileWarning)
        check(manifest, set(runs), lambda case: runs[case["kind"]](case), runner.expected)


# [spec:hfst:req:python.compile-parity/test]
def test_xfst_scripts_run_as_the_tool_does(manifest: dict[str, Any], runner: Runner) -> None:
    def xfst(case: Case) -> None:
        options = divvun_hfst.XfstOptions(**case["options"])
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            result = divvun_hfst.compile_xfst(case["script"], options, name="s.xfst", files={})
        assert result.output == runner.fixtures.text(f"{case['expected']}/stdout"), case["tool"]
        assert sorted(result.written) == case["written"]
        for name in case["written"]:
            expected = runner.fixtures.bytes(f"{case['expected']}/{name}")
            assert result.written[name] == expected, (case["tool"], name)
        assert any("empty stack" in str(w.message) for w in caught)

    check(manifest, {"xfst"}, xfst, runner.expected)
