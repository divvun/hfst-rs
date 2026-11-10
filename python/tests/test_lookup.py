"""Both lookups: hfst's on a Transducer, divvunspell's on an OptimisedTransducer."""

from __future__ import annotations

import math

import pytest

import divvun_hfst
from conftest import READ, Fixtures

SOURCES = ("words", "flags", "ties")


def printed(text: str) -> list[list[tuple[str, tuple[str, ...], str]]]:
    """What `hfst lookup -X print-space` printed, one list of results per
    input: each result's output, its symbols and its weight as printed. An
    input with no results prints one line of weight `inf`."""
    blocks: list[list[tuple[str, tuple[str, ...], str]]] = []
    block: list[tuple[str, tuple[str, ...], str]] = []
    for line in text.splitlines():
        if line == "":
            blocks.append(block)
            block = []
            continue
        _, spaced, weight = line.split("\t")
        if weight != "inf":
            symbols = tuple(s for s in spaced.split(" ") if s)
            block.append(("".join(symbols), symbols, weight))
    assert block == []
    return blocks


def summary(results: list[divvun_hfst.LookupResult]) -> list[tuple[str, tuple[str, ...], str]]:
    return [(r.output, r.symbols, f"{r.weight:.6f}") for r in results]


# [spec:hfst:req:python.lookup/test]
@pytest.mark.parametrize("kind", READ)
@pytest.mark.parametrize("source", SOURCES)
def test_lookup_finds_what_hfst_lookup_prints(fixtures: Fixtures, source: str, kind: str) -> None:
    transducer = divvun_hfst.load_transducer(fixtures.path(f"{source}.{kind}.hfst"))
    inputs = fixtures.text("lookup-inputs.txt").splitlines()
    for limit, name in ((None, f"lookup.{source}.{kind}.txt"), (1, f"lookup.{source}.{kind}.limit1.txt")):
        expected = printed(fixtures.text(name))
        assert len(expected) == len(inputs)
        for given, wanted in zip(inputs, expected):
            results = transducer.lookup(given, limit=limit)
            assert sorted(summary(results)) == sorted(wanted), (given, limit)
            keys = [(r.weight, r.output, r.symbols) for r in results]
            assert keys == sorted(keys), given


# [spec:hfst:req:python.lookup/test]
def test_results_are_typed_and_compare_by_value(fixtures: Fixtures) -> None:
    transducer = divvun_hfst.load_transducer(fixtures.path("flags.optimized-lookup-weighted.hfst"))
    [nominative, genitive] = transducer.lookup("talo")
    assert nominative.output == "talo+N+Nom"
    assert nominative.symbols == ("t", "a", "l", "o", "+N", "+Nom")
    assert nominative.weight == 0.5
    assert genitive.weight == 1.5
    assert transducer.lookup("talo")[0] == nominative
    assert nominative != genitive
    assert repr(nominative) == (
        "LookupResult(output='talo+N+Nom', symbols=('t', 'a', 'l', 'o', '+N', '+Nom'), weight=0.5)"
    )
    with pytest.raises(TypeError):
        hash(nominative)


# [spec:hfst:req:python.lookup/test]
def test_a_limit_stops_after_that_many_paths(fixtures: Fixtures) -> None:
    many = divvun_hfst.load_transducer(fixtures.path("many.optimized-lookup-weighted.hfst"))
    assert len(many.lookup("aaaaaaa")) == 4**7
    assert len(many.lookup("aaaaaaa", limit=5)) == 5
    assert many.lookup("aaaaaaa", limit=0) == []
    assert len(many.lookup("aaaaaaa", limit=10**30)) == 4**7


# [spec:hfst:req:python.lookup/test]
def test_a_time_limit_keeps_what_it_found(fixtures: Fixtures) -> None:
    many = divvun_hfst.load_transducer(fixtures.path("many.optimized-lookup-weighted.hfst"))
    every = {r.output for r in many.lookup("aaaaaaa")}
    timed = many.lookup("aaaaaaa", time_limit=0.001)
    assert len(timed) < len(every)
    assert all(r.output in every and len(r.symbols) == 7 for r in timed)
    words = divvun_hfst.load_transducer(fixtures.path("words.optimized-lookup-weighted.hfst"))
    assert words.lookup("mouse", time_limit=0) == words.lookup("mouse")


# [spec:hfst:req:python.lookup/test]
def test_limits_are_checked_and_keyword_only(fixtures: Fixtures) -> None:
    transducer = divvun_hfst.load_transducer(fixtures.path("words.optimized-lookup-weighted.hfst"))
    for bad in (-1, -(10**30)):
        with pytest.raises(ValueError, match="'limit'"):
            transducer.lookup("cat", limit=bad)
    for wrong in (True, 1.0, "1"):
        with pytest.raises(TypeError, match="'limit'"):
            transducer.lookup("cat", limit=wrong)  # type: ignore[arg-type]
    for bad_time in (-1.0, math.nan, math.inf):
        with pytest.raises(ValueError, match="'time_limit'"):
            transducer.lookup("cat", time_limit=bad_time)
    for wrong_time in (False, "0.5"):
        with pytest.raises(TypeError, match="'time_limit'"):
            transducer.lookup("cat", time_limit=wrong_time)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        transducer.lookup("cat", 1)  # type: ignore[call-arg]
    assert [r.output for r in transducer.lookup("cat", time_limit=1)] == ["dog"]


# [spec:hfst:req:python.lookup/test]
def test_input_with_no_analyses_gives_none(fixtures: Fixtures) -> None:
    for kind in READ:
        transducer = divvun_hfst.load_transducer(fixtures.path(f"words.{kind}.hfst"))
        optimised = divvun_hfst.load_optimised_transducer(fixtures.path(f"words.{kind}.hfst"))
        for given in ("", "mice", "zzz", "ølø"):
            assert transducer.lookup(given) == []
            assert optimised.lookup(given) == []


# [spec:hfst:req:python.lookup/test]
def test_the_optimised_lookup_keeps_the_lightest(fixtures: Fixtures) -> None:
    optimised = divvun_hfst.load_optimised_transducer(
        fixtures.path("words.optimized-lookup-weighted.hfst")
    )
    every = optimised.lookup("mouse")
    assert [(r.output, r.weight) for r in every] == [("mouse", 1.0), ("mice", 2.0)]
    assert optimised.lookup("mouse", limit=1) == every[:1]
    assert optimised.lookup("mouse", limit=0) == []
    assert optimised.lookup("mouse", limit=10**30) == every
    with pytest.raises(TypeError):
        optimised.lookup("mouse", time_limit=1)  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="'limit'"):
        optimised.lookup("mouse", limit=-1)


# [spec:hfst:req:python.lookup/test]
def test_the_lookups_differ_where_divvunspell_does(fixtures: Fixtures) -> None:
    path = fixtures.path("ties.optimized-lookup-weighted.hfst")
    hfst_results = divvun_hfst.load_transducer(path).lookup("a")
    optimised = divvun_hfst.load_optimised_transducer(path).lookup("a")
    outputs = [r.output for r in hfst_results]
    assert outputs.count("xy") == 2, "hfst gives a result for each path"
    assert [r.output for r in optimised].count("xy") == 1, "divvunspell gives each output once"
