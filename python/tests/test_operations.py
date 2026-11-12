"""Transducers of every type: their operations, which change them in place,
and their queries, which report on them as the hfst tools do."""

from __future__ import annotations

import math
from collections.abc import Callable

import pytest

import divvun_hfst
from conftest import Fixtures

Transducer = divvun_hfst.Transducer


def words(fixtures: Fixtures, kind: str = "openfst-tropical") -> Transducer:
    return divvun_hfst.load_transducer(fixtures.path(f"words.{kind}.hfst"))


def outputs(t: Transducer, word: str) -> list[str]:
    return sorted(r.output for r in t.lookup(word))


def paths(t: Transducer) -> list[tuple[str, str, float]]:
    return sorted((p.input, p.output, p.weight) for p in t.extract_paths())


# Each operation that takes the transducer alone, with options, as the tool
# runs it.
UNARY: dict[str, Callable[[Transducer], object]] = {
    "determinize": lambda t: t.determinize(),
    "determinize encode_weights": lambda t: t.determinize(encode_weights=True),
    "minimize": lambda t: t.minimize(),
    "minimize encode_weights": lambda t: t.minimize(encode_weights=True),
    "invert": lambda t: t.invert(),
    "reverse": lambda t: t.reverse(),
    "remove_epsilons": lambda t: t.remove_epsilons(),
    "prune_alphabet": lambda t: t.prune_alphabet(),
    "prune_alphabet force": lambda t: t.prune_alphabet(force=True),
    "project input": lambda t: t.project(project="input"),
    "project": lambda t: t.project(),
    "push_weights initial": lambda t: t.push_weights(push="initial"),
    "push_weights": lambda t: t.push_weights(),
    "push_labels initial": lambda t: t.push_labels(push="initial"),
    "push_labels": lambda t: t.push_labels(),
    "realign": lambda t: t.realign(),
    "repeat 1 to 3": lambda t: t.repeat(from_=1, to=3),
    "repeat 2": lambda t: t.repeat(from_=2),
    "repeat inf": lambda t: t.repeat(from_=math.inf, to=math.inf),
    "insert_freely": lambda t: t.insert_freely(symbol_pair="x:y"),
    "eliminate_flags": lambda t: t.eliminate_flags(),
    "preprocess_for_optimized_lookup_format": lambda t: t.preprocess_for_optimized_lookup_format(),
    "affix_guessify": lambda t: t.affix_guessify(),
    "affix_guessify prefix": lambda t: t.affix_guessify(direction="prefix", weight=2),
    "kill_paths": lambda t: t.kill_paths(symbol="c"),
    "reweight": lambda t: t.reweight(addition=1, multiplier=2),
    "reweight sqrt": lambda t: t.reweight(symbol="a", function="sqrt"),
    "set_name": lambda t: t.set_name("renamed"),
    "edit_metadata": lambda t: t.edit_metadata(add={"source": "test"}),
    "expand_equivalences": lambda t: t.expand_equivalences(from_="a", to="A"),
    "substitute": lambda t: t.substitute(from_label="a", to_label="A"),
    "substitute compose": lambda t: t.substitute(from_label="a", to_label="A", compose=True),
}


# [spec:hfst:req:python.operations/test]
def test_each_operation_changes_in_place(fixtures: Fixtures) -> None:
    for kind in ("openfst-tropical", "foma"):
        original = words(fixtures, kind)
        for name, operation in UNARY.items():
            t = original.copy()
            assert operation(t) is None, name
            assert t.summarize().states > 0, name
        assert paths(original) == paths(words(fixtures, kind)), kind


# [spec:hfst:req:python.operations/test]
def test_operations_do_what_the_tools_do(fixtures: Fixtures) -> None:
    t = words(fixtures)
    t.invert()
    assert outputs(t, "dog") == ["cat"]
    t.invert()
    t.reverse()
    assert outputs(t, "tac") == ["god"]
    t.reverse()
    t.project(project="input")
    assert outputs(t, "cat") == ["cat"]
    t = words(fixtures)
    t.repeat(from_=2, to=2)
    assert outputs(t, "catcat") == ["dogdog"]
    assert outputs(t, "cat") == []
    t = words(fixtures)
    t.kill_paths(symbol="d")
    assert outputs(t, "cat") == []
    assert outputs(t, "mouse") == ["mice", "mouse"]
    t = words(fixtures)
    t.reweight(addition=1, end_states_only=True)
    assert [r.weight for r in t.lookup("cat")] == [1.5]
    t.set_name("renamed")
    assert t.name == "renamed"
    t.edit_metadata(add={"source": "test"})
    assert t.properties["source"] == "test"


# [spec:hfst:req:python.operations/test]
def test_binary_operations_take_another_transducer(fixtures: Fixtures) -> None:
    for name in ("compose", "concatenate", "conjunct", "subtract", "disjunct",
                 "priority_disjunct"):
        t = words(fixtures)
        assert getattr(t, name)(words(fixtures)) is None, name
        assert t.summarize().states > 0, name
    acceptor = words(fixtures)
    acceptor.project(project="input")
    other = acceptor.copy()
    acceptor.shuffle(other)
    assert "ccaatt" in outputs(acceptor, "ccaatt")
    with pytest.raises(divvun_hfst.HfstError) as raised:
        words(fixtures).shuffle(words(fixtures))
    assert raised.value.code == "not-automaton"
    t = words(fixtures)
    other = words(fixtures)
    other.invert()
    t.compose(other)
    assert outputs(t, "cat") == ["cat"]
    t = words(fixtures)
    t.disjunct(words(fixtures, "foma"))
    assert t.type == "openfst-tropical"
    with pytest.raises(divvun_hfst.HfstError) as raised:
        words(fixtures).disjunct(words(fixtures, "foma"), do_not_convert=True)
    assert raised.value.code == "type-mismatch"
    lexicon = words(fixtures)
    rule = words(fixtures)
    rule.project(project="output")
    lexicon.compose_intersect([rule], lexicon_name="words", rules_name="rules")
    assert lexicon.summarize().states > 0


# [spec:hfst:req:python.operations/test]
def test_guessify_returns_the_generator(fixtures: Fixtures) -> None:
    t = words(fixtures)
    generator = t.guessify()
    assert t.type == "optimized-lookup-weighted"
    assert generator is not None and generator.type == "optimized-lookup-weighted"
    assert words(fixtures).guessify(do_not_compile_generator=True) is None
    [report] = t.guess(["cats"])
    assert report.input == "cats"
    assert not report.paradigms
    assert all(isinstance(g, str) for g in report.guesses)


# [spec:hfst:req:python.operations/test]
def test_substitute_by_a_transducer_or_a_file(fixtures: Fixtures) -> None:
    t = words(fixtures)
    replacement = words(fixtures)
    t.substitute(from_label="c", to_transducer=replacement, file_name="net.hfst")
    assert "net.hfst" in t.name
    t = words(fixtures)
    t.substitute(from_file="c\tk\n", file_name="relabel.tsv")
    assert outputs(t, "kat") == ["dog"]
    t = words(fixtures)
    t.substitute(from_label="c", to_transducer=t)
    assert t.summarize().states > 0


# [spec:hfst:req:python.operations/test]
def test_algebra_refuses_optimized_lookup_and_stays_usable(fixtures: Fixtures) -> None:
    ol = words(fixtures, "optimized-lookup-weighted")
    for operation in (ol.invert, ol.minimize, lambda: ol.compose(words(fixtures))):
        with pytest.raises(divvun_hfst.HfstError) as raised:
            operation()
        assert raised.value.code == "type-mismatch"
    assert outputs(ol, "cat") == ["dog"]
    ol.convert("openfst-tropical")
    ol.invert()
    assert outputs(ol, "dog") == ["cat"]


# [spec:hfst:req:python.operations/test]
def test_options_are_checked(fixtures: Fixtures) -> None:
    t = words(fixtures)
    refused: list[Callable[[], object]] = [
        lambda: t.project(project="sideways"),  # type: ignore[arg-type]
        lambda: t.push_weights(push="middle"),  # type: ignore[arg-type]
        lambda: t.reweight(function="square"),  # type: ignore[arg-type]
        lambda: t.affix_guessify(direction="infix"),  # type: ignore[arg-type]
        lambda: t.expand_equivalences(from_="a", to="b", level="third"),  # type: ignore[arg-type]
        lambda: t.repeat(from_=-1),
        lambda: t.repeat(to=1.5),
        lambda: t.convert("sfst"),  # type: ignore[arg-type]
    ]
    for call in refused:
        with pytest.raises(ValueError):
            call()
    with pytest.raises(TypeError):
        t.determinize(True)  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        t.compose("words")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        t.edit_metadata(add=["source"])  # type: ignore[arg-type]
    with pytest.raises(divvun_hfst.HfstError) as raised:
        t.repeat(from_=3, to=2)
    assert raised.value.code == "invalid-argument"
    assert outputs(t, "cat") == ["dog"]


# [spec:hfst:req:python.transducer/test]
def test_copy_is_a_transducer_of_its_own(fixtures: Fixtures) -> None:
    for kind in ("openfst-tropical", "foma", "optimized-lookup-weighted"):
        t = words(fixtures, kind)
        copied = t.copy()
        assert copied is not t
        assert (copied.type, copied.name, copied.properties) == (t.type, t.name, t.properties)
        assert copied.lookup("mouse") == t.lookup("mouse")
        copied.convert("foma")
        copied.invert()
        assert t.type == kind
        assert outputs(t, "cat") == ["dog"]
        assert outputs(copied, "dog") == ["cat"]


# [spec:hfst:req:python.transducer/test]
def test_lookup_in_every_type_is_hfst_lookups(fixtures: Fixtures) -> None:
    ol = words(fixtures, "optimized-lookup-weighted")
    expected = [(r.output, r.symbols, r.weight) for r in ol.lookup("mouse")]
    for kind in ("openfst-tropical", "foma"):
        t = words(fixtures, kind)
        [report] = divvun_hfst.lookup_report([t], ["mouse"])
        assert report.results is not None
        assert t.lookup("mouse") == list(report.results)
        assert len(t.lookup("mouse", limit=1)) == 1
    tropical = [(r.output, r.symbols, r.weight) for r in words(fixtures).lookup("mouse")]
    assert tropical == expected
    assert isinstance(ol.lookup("cat")[0], divvun_hfst.LookupResult)


# [spec:hfst:req:python.transducer/test]
def test_convert_changes_the_type_in_place(fixtures: Fixtures) -> None:
    t = words(fixtures)
    kinds: list[divvun_hfst.TransducerType] = [
        "foma",
        "optimized-lookup-unweighted",
        "optimized-lookup-weighted",
        "openfst-tropical",
    ]
    for kind in kinds:
        t.convert(kind, quick=True)
        assert t.type == kind
        assert outputs(t, "cat") == ["dog"]


# [spec:hfst:req:python.operations/test]
def test_queries_report_what_the_tools_report(fixtures: Fixtures) -> None:
    t = words(fixtures)
    summary = t.summarize()
    assert isinstance(summary, divvun_hfst.Summary)
    assert summary.type == "openfst-tropical"
    assert summary.details is None
    assert ("c", "d", 1) in summary.symbol_pairs
    detailed = t.summarize(verbose=True)
    assert isinstance(detailed.details, divvun_hfst.SummaryDetails)
    alphabets = t.dump_alphabets()
    assert "c" in alphabets.seen and "c" in alphabets.declared
    many = divvun_hfst.load_transducer(fixtures.path("many.openfst-tropical.hfst"))
    comparison = t.check_alpha(many)
    assert comparison.mismatch
    assert "e" in comparison.second_used
    assert t.compare(words(fixtures))
    assert t.compare(t)
    assert not t.compare(many)
    found = t.extract_paths()
    assert all(isinstance(p, divvun_hfst.ExtractedPath) for p in found)
    assert ("cat", "dog", 0.5) in paths(t)
    assert ("c", "d") in next(p.pairs for p in found if p.input == "cat")
    assert len(t.extract_paths(max_strings=1)) == 1
    assert len(t.extract_paths(nbest=2)) == 2


# [spec:hfst:req:python.operations/test]
def test_lookup_tools_report_each_line(fixtures: Fixtures) -> None:
    t = words(fixtures)
    found, unknown = divvun_hfst.lookup_report([t], ["cat", "dog"])
    assert found.input == "cat" and found.results is not None
    assert [r.output for r in found.results] == ["dog"]
    assert unknown.results == ()
    [inverted] = divvun_hfst.flookup_report([t], ["dog"])
    assert inverted.results is not None
    assert [r.output for r in inverted.results] == ["cat"]
    ol = words(fixtures, "optimized-lookup-weighted")
    [report] = divvun_hfst.optimized_lookup_report([ol], ["cat"])
    assert report.analyses == (("dog", 0.5),)
    with pytest.raises(ValueError):
        divvun_hfst.lookup_report([t], ["cat"], cascade="intersection")  # type: ignore[arg-type]


# [spec:hfst:req:python.operations/test]
def test_grep_pmatch_and_naive_tokenising(fixtures: Fixtures) -> None:
    report = divvun_hfst.grep("{cat}", "the cat\nno dog\n")
    assert report.matched
    assert [line.number for line in report.lines] == [1]
    assert "{{{cat}}}" in report.lines[0].text
    animals = divvun_hfst.load_tokeniser(fixtures.path("animals.pmhfst"))
    [piece] = animals.pmatch("a cat")
    assert piece.input == "a cat" and piece.marked is not None and piece.located is None
    [located] = animals.pmatch("a cat", locate=True)
    assert located.located is not None
    assert all(isinstance(loc, divvun_hfst.PmatchLocation) for loc in located.located)
    tokeniser = words(fixtures).naive_tokeniser()
    [token] = tokeniser.tokenise("cat")
    assert [r.analysis for r in token.readings] == ["dog"]


# [spec:hfst:req:python.operations/test]
def test_pair_test_reports_each_test(fixtures: Fixtures) -> None:
    rule = words(fixtures)
    results = divvun_hfst.pair_test([rule], "c:d a:o t:g\n! a comment\n")
    assert [r.test for r in results] == ["c:d a:o t:g"]
    assert all(isinstance(r, divvun_hfst.PairTestResult) for r in results)
