"""The package against the hfst tools: each query reports, through the built
package, data that printed as the tool prints it is what the tool printed of
the inputs the repository's tests give it. compile_parity.py says how the
cases are read."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import pytest

import divvun_hfst
from compile_parity import Case, Runner, check, lookup_text, shown
from conftest import Fixtures


@pytest.fixture(scope="module")
def manifest(fixtures: Fixtures) -> dict[str, Any]:
    loaded: dict[str, Any] = fixtures.json("compile-parity.json")
    return loaded


@pytest.fixture(scope="module")
def runner(fixtures: Fixtures, manifest: dict[str, Any]) -> Runner:
    return Runner(fixtures, manifest)


# [spec:hfst:req:python.compile-parity/test]
def test_extract_paths_finds_what_fst2strings_prints(
    manifest: dict[str, Any], runner: Runner
) -> None:
    def run(case: Case) -> str:
        paths = runner.call(runner.one(case["input"]), "extract_paths", case["kwargs"])
        lines = []
        for p in paths:
            identity = all(i == o for i, o in p.pairs)
            side = p.input if identity else f"{p.input}:{p.output}"
            lines.append(f"{side}\t{shown(p.weight)}\n")
        return "".join(lines)

    check(manifest, {"extract_paths"}, run, runner.printed)


# [spec:hfst:req:python.compile-parity/test]
def test_lookups_report_what_the_tools_print(manifest: dict[str, Any], runner: Runner) -> None:
    def lookup(case: Case) -> str:
        ts = [runner.one(name) for name in case["inputs"]]
        return lookup_text(divvun_hfst.lookup_report(ts, case["lines"], **case["kwargs"]))

    def flookup(case: Case) -> str:
        ts = [runner.one(name) for name in case["inputs"]]
        reports = divvun_hfst.flookup_report(ts, case["lines"], **case["kwargs"])
        text = ""
        for report in reports:
            if report.results:
                for r in report.results:
                    text += f"{report.input}\t{r.output}\t{r.weight:.6f}\n"
                text += "\n"
            else:
                text += f"{report.input}\t{report.input}+?\tinf\n\n"
        return text

    def optimized(case: Case) -> str:
        ts = [runner.one(name) for name in case["inputs"]]
        weighted = ts[0].type == "optimized-lookup-weighted"
        reports = divvun_hfst.optimized_lookup_report(ts, case["lines"], **case["kwargs"])
        text = ""
        for report in reports:
            word = report.input
            if report.analyses is None:
                text += f"{word}\t{word}\t+?\n\n\n\n"
            elif not report.analyses:
                text += f"{word}\t{word}\t+?" + ("\n\n" if weighted else "\n\n\n\n")
            else:
                for output, weight in report.analyses:
                    text += f"{word}\t{output}" + (f"\t{shown(weight)}" if weighted else "") + "\n"
                text += "\n"
        return text

    runs: dict[str, Callable[[Case], str]] = {
        "lookup_report": lookup,
        "flookup_report": flookup,
        "optimized_lookup_report": optimized,
    }
    check(manifest, set(runs), lambda case: runs[case["kind"]](case), runner.printed)


# [spec:hfst:req:python.compile-parity/test]
def test_reports_are_what_the_tools_print(manifest: dict[str, Any], runner: Runner) -> None:
    def compare(case: Case) -> str:
        first, second = runner.one(case["first"]), runner.one(case["second"])
        equal = first.compare(second)
        names = [
            t.name or runner.inputs[name]
            for t, name in ((first, case["first"]), (second, case["second"]))
        ]
        return f"{names[0]} {'==' if equal else '!='} {names[1]}\n"

    # compare exits 1 for transducers that differ, so a case it printed for
    # is a comparison made.
    cases = [c for c in manifest["cases"] if c["kind"] == "compare"]
    for case in cases:
        if runner.printed(case):
            assert compare(case) == runner.printed(case), case["tool"]
        else:
            with pytest.raises(divvun_hfst.HfstError):
                compare(case)

    def dump(case: Case) -> str:
        a = runner.one(case["input"]).dump_alphabets()
        return "".join(f"{s}\n" for s in (*a.declared, *a.seen))

    check(manifest, {"dump_alphabets"}, dump, runner.printed)

    def check_alpha(case: Case) -> None:
        first, second = runner.one(case["first"]), runner.one(case["second"])
        c = first.check_alpha(second)
        printed = runner.printed(case)
        first_only = [s for s in c.first_used if s not in c.second_used]
        second_only = [s for s in c.second_declared if s not in c.first_declared]
        assert c.mismatch
        assert (
            f"In first {first.name} but not in second {second.name}:{', '.join(first_only)}\n"
            in printed
        )
        assert (
            f"Second {second.name} has but first {first.name} does not: "
            f"{', '.join(second_only)}\n" in printed
        )

    check(manifest, {"check_alpha"}, check_alpha, runner.printed)

    def summarize(case: Case) -> None:
        summary = runner.one(case["input"]).summarize(verbose=True)
        printed = runner.printed(case)
        assert f"# of states: {summary.states}\n" in printed
        assert f"# of arcs: {summary.arcs}\n" in printed

    check(manifest, {"summarize"}, summarize, runner.printed)


# [spec:hfst:req:python.compile-parity/test]
def test_text_queries_are_what_the_tools_print(manifest: dict[str, Any], runner: Runner) -> None:
    def grep(case: Case) -> str:
        report = divvun_hfst.grep(case["pattern"], case["text"], **case["kwargs"])
        return "".join(f"{line.text}\n" for line in report.lines)

    def pmatch(case: Case) -> str:
        archive = runner.fixtures.bytes(runner.inputs[case["archive"]])
        tokeniser = divvun_hfst.read_tokeniser(archive)
        text = ""
        for piece in tokeniser.pmatch(case["text"], **case["kwargs"]):
            if piece.marked is not None:
                text += piece.marked + "\n" + ("" if case["kwargs"].get("newline") else "\n")
            elif piece.located:
                for loc in piece.located:
                    text += f"{loc.start}|{loc.length}|{loc.output}|{loc.tag}|{shown(loc.weight)}\n"
                text += "\n"
        return text

    def guess(case: Case) -> str:
        guesser, generator = runner.all(case["guesser"])
        text = ""
        for report in guesser.guess(case["words"], generator=generator):
            text += "".join(f"{report.input}\t{g}\n" for g in report.guesses) + "\n"
        return text

    def tokenise(case: Case) -> str:
        return runner.one(case["input"]).naive_tokeniser().format(case["text"], "segment")

    runs: dict[str, Callable[[Case], str]] = {
        "grep": grep,
        "pmatch": pmatch,
        "guess": guess,
        "naive_tokeniser": tokenise,
    }
    check(manifest, set(runs), lambda case: runs[case["kind"]](case), runner.printed)

    def pair_test(case: Case) -> None:
        rules = runner.all(case["rules"])
        results = divvun_hfst.pair_test(rules, case["tests"], **case["kwargs"])
        assert any(r.failed for r in results) != case["ok"], case["tool"]
        for result in results:
            for failure in result.failures:
                assert f"Rule {failure.rule} fails:" in runner.printed(case)

    for case in [c for c in manifest["cases"] if c["kind"] == "pair_test"]:
        pair_test(case)
