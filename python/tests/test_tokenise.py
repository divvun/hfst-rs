"""Tokenising: tokens with code point offsets, the hfst tokenise formats, and
the options of each."""

from __future__ import annotations

import dataclasses
import math
import typing
from typing import Any

import pytest

import divvun_hfst
from conftest import Fixtures

TEXT = "The cat saw a dog in New York, blorf! Øl 🐱 blåbær 🐱🐱."


@pytest.fixture(scope="module")
def tokeniser(fixtures: Fixtures) -> divvun_hfst.Tokeniser:
    return divvun_hfst.load_tokeniser(fixtures.path("tokenise.pmhfst"))


def format_options(given: dict[str, Any]) -> divvun_hfst.FormatOptions:
    """The FormatOptions an npm package FormatOptions object stands for."""
    kwargs: dict[str, Any] = {}
    for key, value in given.items():
        name = "".join(f"_{c.lower()}" if c.isupper() else c for c in key)
        kwargs[name] = value / 1000 if name == "time_limit" else value
    return divvun_hfst.FormatOptions(**kwargs)


# [spec:hfst:req:python.tokenise/test]
def test_offsets_index_the_text_by_code_point(tokeniser: divvun_hfst.Tokeniser) -> None:
    for options in (divvun_hfst.TokeniseOptions(), divvun_hfst.TokeniseOptions(print_all=True)):
        tokens = tokeniser.tokenise(TEXT, options)
        for token in tokens:
            assert TEXT[token.start : token.end] == token.surface
        starts = [t.start for t in tokens]
        assert starts == sorted(starts)
    cats = [t for t in tokeniser.tokenise(TEXT) if t.surface == "🐱"]
    assert cats and all(t.end - t.start == 1 for t in cats)


# [spec:hfst:req:python.tokenise/test]
def test_tokens_are_the_cg_cohorts(tokeniser: divvun_hfst.Tokeniser) -> None:
    line = TEXT
    tokens = tokeniser.tokenise(line)
    printed = tokeniser.format(line, "cg", divvun_hfst.FormatOptions(print_weights=True))
    cohorts = [block for block in printed.split("\n\n") if block.strip()]
    assert [c.splitlines()[0] for c in cohorts] == [f'"<{t.surface}>"' for t in tokens]
    assert [len(c.splitlines()) - 1 for c in cohorts] == [len(t.readings) for t in tokens]
    saw = next(t for t in tokens if t.surface == "saw")
    assert saw.analysed
    assert [r.analysis for r in saw.readings][:2] == ["see+V+Past", "saw+N"]
    blorf = next(t for t in tokens if t.surface == "blorf")
    assert not blorf.analysed


# [spec:hfst:req:python.tokenise/test]
def test_format_prints_what_hfst_tokenise_prints(
    tokeniser: divvun_hfst.Tokeniser, fixtures: Fixtures
) -> None:
    cases = fixtures.json("tokenise-cases.json")
    for case in cases["formats"]:
        text = cases["texts"][case["text"]]
        got = tokeniser.format(text, case["format"], format_options(case["options"]))
        assert got == fixtures.text(f"tokenise.{case['name']}.out"), case["name"]


# [spec:hfst:req:python.tokenise/test]
def test_options_are_checked_when_made() -> None:
    wrong_type: list[dict[str, Any]] = [
        {"dedupe": 1},
        {"max_weight_classes": 1.5},
        {"max_weight_classes": True},
        {"beam": "0.5"},
        {"beam": True},
        {"time_limit": "1"},
        {"print_all": 0},
    ]
    for kwargs in wrong_type:
        with pytest.raises(TypeError, match=next(iter(kwargs))):
            divvun_hfst.TokeniseOptions(**kwargs)
    wrong_value: list[dict[str, Any]] = [
        {"max_weight_classes": 0},
        {"max_weight_classes": -3},
        {"beam": -0.5},
        {"beam": math.nan},
        {"time_limit": -1},
        {"weight_cutoff": math.nan},
    ]
    for kwargs in wrong_value:
        with pytest.raises(ValueError, match=next(iter(kwargs))):
            divvun_hfst.TokeniseOptions(**kwargs)
    with pytest.raises(TypeError):
        divvun_hfst.TokeniseOptions(weightCutoff=1)  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        divvun_hfst.FormatOptions(weight_cutoff=1)  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        divvun_hfst.TokeniseOptions(True)  # type: ignore[call-arg]
    divvun_hfst.TokeniseOptions(max_weight_classes=10**30, weight_cutoff=math.inf)


# [spec:hfst:req:python.tokenise/test]
def test_options_are_frozen_values() -> None:
    options = divvun_hfst.TokeniseOptions(beam=0.5, print_all=True)
    assert repr(options) == "TokeniseOptions(beam=0.5, print_all=True)"
    assert repr(divvun_hfst.FormatOptions()) == "FormatOptions()"
    with pytest.raises(dataclasses.FrozenInstanceError):
        options.beam = 1.0  # type: ignore[misc]
    assert dataclasses.replace(options, beam=None) == divvun_hfst.TokeniseOptions(print_all=True)
    with pytest.raises(ValueError):
        dataclasses.replace(options, beam=-1.0)


# [spec:hfst:req:python.tokenise/test]
def test_options_of_the_wrong_class_are_refused(tokeniser: divvun_hfst.Tokeniser) -> None:
    with pytest.raises(TypeError, match="TokeniseOptions"):
        tokeniser.tokenise("cat", divvun_hfst.FormatOptions())  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="FormatOptions"):
        tokeniser.format("cat", "cg", divvun_hfst.TokeniseOptions())  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        tokeniser.tokenise("cat", {"print_all": True})  # type: ignore[arg-type]
    assert tokeniser.tokenise("cat", None) == tokeniser.tokenise("cat")


# [spec:hfst:req:python.tokenise/test]
def test_format_names_are_hfst_tokenises(tokeniser: divvun_hfst.Tokeniser) -> None:
    assert typing.get_args(divvun_hfst.TokeniseFormat) == (
        "segment",
        "space-separated",
        "xerox",
        "cg",
        "finnpos",
        "giella-cg",
        "conllu",
        "visl",
    )
    with pytest.raises(ValueError, match="'giellacg'"):
        tokeniser.format("cat", "giellacg")  # type: ignore[arg-type]


# [spec:hfst:req:python.tokenise/test]
def test_tokenise_options_change_the_tokens(tokeniser: divvun_hfst.Tokeniser) -> None:
    plain = tokeniser.tokenise("The cat blorf")
    assert [t.surface for t in plain] == ["The", "cat", "blorf"]
    every = tokeniser.tokenise("The cat blorf", divvun_hfst.TokeniseOptions(print_all=True))
    assert [t.surface for t in every] == ["The", " ", "cat", " ", "blorf"]
    assert every[1].readings == () and not every[1].analysed
    cut = tokeniser.tokenise("The cat blorf", divvun_hfst.TokeniseOptions(weight_cutoff=50))
    assert [t.surface for t in cut] == ["cat"]
    one = tokeniser.tokenise("saw", divvun_hfst.TokeniseOptions(max_weight_classes=1))
    assert [r.analysis for r in one[0].readings] == ["see+V+Past"]


def test_tokens_compare_by_value(tokeniser: divvun_hfst.Tokeniser) -> None:
    [cat] = tokeniser.tokenise("cat")
    assert cat == tokeniser.tokenise("cat")[0]
    assert repr(cat) == (
        "Token(surface='cat', start=0, end=3, analysed=True, readings=("
        "Reading(analysis='cat+N', weight=0.0), Reading(analysis='cat ??', weight=100.0)))"
    )
    assert isinstance(cat.readings[0], divvun_hfst.Reading)
