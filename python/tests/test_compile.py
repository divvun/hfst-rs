"""The compilers: what they make, the files they read and write, the options
they take, and the diagnostics they give."""

from __future__ import annotations

import dataclasses
import logging
import warnings
from pathlib import Path

import pytest

import divvun_hfst
from test_threads import BUSY, python_runs_during


def outputs(t: divvun_hfst.Transducer, word: str) -> list[str]:
    return sorted(r.output for r in t.lookup(word))


# [spec:hfst:req:python.compile/test]
def test_regex_compiles_an_expression() -> None:
    t = divvun_hfst.regex("{cat}:{dog} | {mouse}")
    assert t.type == "openfst-tropical"
    assert t.name == "xre(?)"
    assert outputs(t, "cat") == ["dog"]
    for kind in ("foma", "optimized-lookup-weighted", "optimized-lookup-unweighted"):
        options = divvun_hfst.RegexOptions(format=kind)
        compiled = divvun_hfst.regex("{cat}:{dog}", options)
        assert compiled.type == kind
        assert outputs(compiled, "cat") == ["dog"]


# [spec:hfst:req:python.compile/test]
def test_several_expressions_make_one_transducer_only_disjunct() -> None:
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.regex("a\nb\n")
    assert raised.value.code == "transducer-count"
    assert "disjunct" in str(raised.value)
    union = divvun_hfst.regex("a\nb\n", divvun_hfst.RegexOptions(disjunct=True))
    assert outputs(union, "a") == ["a"] and outputs(union, "b") == ["b"]
    semicolons = divvun_hfst.RegexOptions(semicolon=True, disjunct=True)
    assert outputs(divvun_hfst.regex("a ; b ;", semicolons), "b") == ["b"]


# [spec:hfst:req:python.compile/test]
def test_lexc_is_named_after_its_first_source() -> None:
    root = "Multichar_Symbols +N\nLEXICON Root\nNouns ;\n"
    nouns = "LEXICON Nouns\ncat+N:cat # ;\n"
    t = divvun_hfst.compile_lexc([root, nouns], names=["root.lexc", "nouns.lexc"])
    assert t.name == "lexc(root.lexc)"
    assert outputs(t, "cat+N") == ["cat"]
    foma = divvun_hfst.compile_lexc(
        ("LEXICON Root\ndog # ;\n",), divvun_hfst.LexcOptions(format="foma")
    )
    assert foma.type == "foma" and foma.name == "lexc(<stdin>)"
    with pytest.raises(TypeError):
        divvun_hfst.compile_lexc("LEXICON Root\ndog # ;\n")
    with pytest.raises(ValueError):
        divvun_hfst.compile_lexc([root, nouns], names=["root.lexc"])


# [spec:hfst:req:python.compile/test]
def test_twolc_returns_its_rule_transducers() -> None:
    grammar = (
        'Alphabet a b c a:b ;\nSets\nCns = a b ;\nRules\n'
        '"R1"\nCns:0 <=> _ c ;\n"R2"\na:b => c _ ;\n'
    )
    with pytest.warns(divvun_hfst.CompileWarning, match="Alphabet"):
        rules = divvun_hfst.compile_twolc(grammar)
    assert {r.name.split()[0] for r in rules} == {'"R1"', '"R2"'}
    foma = divvun_hfst.compile_twolc(
        grammar, divvun_hfst.TwolcOptions(format="foma", quiet=True)
    )
    assert all(r.type == "foma" for r in foma)
    results = divvun_hfst.pair_test(rules, "c:c a:b\n")
    assert [r.test for r in results] == ["c:c a:b"]


# [spec:hfst:req:python.compile/test]
def test_xfst_returns_what_the_script_left() -> None:
    script = "define X c ;\nregex a:b ;\nsave stack out.hfst\nprint words\n"
    result = divvun_hfst.compile_xfst(script, files={})
    assert isinstance(result, divvun_hfst.XfstResult)
    assert "a:b" in result.output
    assert [t.type for t in result.stack] == ["openfst-tropical"]
    assert outputs(result.definitions["X"], "c") == ["c"]
    assert list(result.written) == ["out.hfst"]
    [saved] = divvun_hfst.read_transducers(result.written["out.hfst"])
    assert outputs(saved, "a") == ["b"]
    loaded = divvun_hfst.compile_xfst(
        "load stack in.hfst\nprint words\n",
        divvun_hfst.XfstOptions(format="foma"),
        files={"in.hfst": result.written["out.hfst"]},
    )
    assert "a:b" in loaded.output
    assert [t.type for t in loaded.stack] == ["foma"]


# [spec:hfst:req:python.compile/test]
def test_xfst_reads_its_input_and_the_filesystem(tmp_path: Path) -> None:
    out = tmp_path / "out.hfst"
    result = divvun_hfst.compile_xfst(
        f"regex {{cat}}:{{dog}} ;\nsave stack {out}\napply down cat\n", input="cat\n"
    )
    assert "dog" in result.output
    assert result.written == {str(out): out.read_bytes()}


# [spec:hfst:req:python.compile/test]
def test_files_come_from_the_mapping_or_disk(tmp_path: Path) -> None:
    files: dict[str, str | bytes] = {"words.txt": "cat\ndog\n", "bytes.txt": b"mouse\n"}
    t = divvun_hfst.regex('@txt"words.txt" | @txt"bytes.txt"', files=files)
    assert outputs(t, "dog") == ["dog"] and outputs(t, "mouse") == ["mouse"]
    with pytest.raises(divvun_hfst.CompileError):
        divvun_hfst.regex('@txt"words.txt"', files={})
    (tmp_path / "words.txt").write_text("cow\n", encoding="utf-8")
    t = divvun_hfst.regex(f'@txt"{tmp_path / "words.txt"}"')
    assert outputs(t, "cow") == ["cow"]
    with pytest.raises(TypeError):
        divvun_hfst.regex("a", files={"a.txt": 1})  # type: ignore[dict-item]
    with pytest.raises(TypeError):
        divvun_hfst.regex("a", files=["a.txt"])  # type: ignore[arg-type]


# [spec:hfst:req:python.compile/test]
def test_pmatch_makes_an_archive_read_tokeniser_reads() -> None:
    script = 'Define TOP [ {cat} | {dog} ] EndTag(Animal) ;\n'
    archive = divvun_hfst.compile_pmatch(script)
    assert isinstance(archive, bytes)
    tokeniser = divvun_hfst.read_tokeniser(archive)
    assert [t.surface for t in tokeniser.tokenise("a cat")] == ["cat"]
    flattened = divvun_hfst.compile_pmatch(script, divvun_hfst.PmatchOptions(flatten=True))
    assert divvun_hfst.read_transducers(flattened)[0].name == "TOP"


# [spec:hfst:req:python.compile/test]
def test_pmatch_finds_files_beside_its_source() -> None:
    words = "cow\nhorse\n"
    script = 'Define TOP @txt"animals.txt" EndTag(Animal) ;\n'
    beside = divvun_hfst.compile_pmatch(
        script, name="lang/x.pmatch", files={"lang/animals.txt": words}
    )
    tokeniser = divvun_hfst.read_tokeniser(beside)
    assert [t.surface for t in tokeniser.tokenise("a horse")] == ["horse"]
    with pytest.raises(divvun_hfst.CompileError):
        divvun_hfst.compile_pmatch(script, name="lang/x.pmatch", files={"animals.txt": words})


# [spec:hfst:req:python.compile/test]
def test_strings_and_text_compile_to_one_transducer() -> None:
    t = divvun_hfst.compile_strings("cat:dog\t1.5\n")
    assert [r.weight for r in t.lookup("cat")] == [1.5]
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.compile_strings("cat\ndog\n")
    assert raised.value.code == "transducer-count"
    union = divvun_hfst.compile_strings(
        "cat\ndog\n", divvun_hfst.StringsOptions(disjunct_strings=True, format="foma")
    )
    assert outputs(union, "dog") == ["dog"] and union.type == "foma"
    spaced = divvun_hfst.compile_strings(
        "c a t\n", divvun_hfst.StringsOptions(has_spaces=True, multichar_symbols=("ca",))
    )
    assert outputs(spaced, "cat") == ["cat"]
    att = divvun_hfst.parse_att("0\t1\tc\td\n1\n", name="t.att")
    assert outputs(att, "c") == ["d"]
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.parse_att("0\t1\ta\ta\n1\n--\n0\t1\tb\tb\n1\n")
    assert raised.value.code == "transducer-count"
    joined = divvun_hfst.parse_att(
        "0\t1\ta\ta\n1\n--\n0\t1\tb\tb\n1\n", divvun_hfst.AttOptions(disjunct=True)
    )
    assert outputs(joined, "b") == ["b"]
    prolog = divvun_hfst.parse_prolog(
        'network(n).\narc(n, 0, 1, "c":"d").\nfinal(n, 1).\n',
        divvun_hfst.PrologOptions(format="foma"),
    )
    assert prolog.type == "foma" and outputs(prolog, "c") == ["d"]


# [spec:hfst:req:python.compile-errors/test]
def test_a_failed_compile_carries_its_diagnostics() -> None:
    assert issubclass(divvun_hfst.CompileError, divvun_hfst.HfstError)
    assert divvun_hfst.CompileError.__module__ == "divvun_hfst"
    source = "čáhci ]"
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.regex(source, name="nouns.xre")
    error = raised.value
    assert error.code == "failed"
    assert "nouns.xre" in str(error)
    [diagnostic] = [d for d in error.diagnostics if d.severity == "error"]
    assert isinstance(diagnostic, divvun_hfst.Diagnostic)
    assert diagnostic.source_name == "nouns.xre"
    assert diagnostic.span == (6, 7)
    assert source[slice(*diagnostic.span)] == "]"
    assert diagnostic.labels[0].span == diagnostic.span
    assert isinstance(diagnostic.notes, tuple)
    assert "\x1b[" not in diagnostic.rendered
    assert "nouns.xre" in diagnostic.rendered


# [spec:hfst:req:python.compile-errors/test]
def test_spans_count_code_points_on_later_lines() -> None:
    source = "Æ ø\n[ŋ |\n"
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.regex(source)
    spans = [d.span for d in raised.value.diagnostics if d.span is not None]
    assert spans
    for start, end in spans:
        assert 0 <= start <= end <= len(source)
        assert start >= source.index("\n")


# [spec:hfst:req:python.compile-errors/test]
def test_lexc_and_twolc_failures_carry_their_diagnostics() -> None:
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.compile_lexc(
            ["LEXICON Root\ncat # ;\ndog Nowhere ;\n"],
            divvun_hfst.LexcOptions(werror=True, warn_missing_lexicons=True),
        )
    assert raised.value.diagnostics
    with pytest.raises(divvun_hfst.CompileError) as raised:
        divvun_hfst.compile_twolc('Alphabet a b ;\nRules\n"R1"\na:b <=> _ "c ;\n')
    assert any(d.severity == "error" for d in raised.value.diagnostics)


# [spec:hfst:req:python.compile-errors/test]
def test_warnings_of_a_compile_are_compile_warnings() -> None:
    assert issubclass(divvun_hfst.CompileWarning, UserWarning)
    source = "cát\t-1\n"
    with pytest.warns(divvun_hfst.CompileWarning) as caught:
        t = divvun_hfst.compile_strings(source, name="w.txt")
    assert outputs(t, "cát") == ["cát"]
    [warning] = [w.message for w in caught]
    assert isinstance(warning, divvun_hfst.CompileWarning)
    diagnostic = warning.diagnostic
    assert diagnostic.severity == "warning"
    assert diagnostic.source_name == "w.txt"
    assert diagnostic.span == (0, 6)
    assert str(warning) == diagnostic.message
    with pytest.warns(divvun_hfst.CompileWarning) as caught:
        divvun_hfst.compile_lexc(
            ["LEXICON Root\ncat # ;\ndog Nowhere ;\n"],
            divvun_hfst.LexcOptions(warn_missing_lexicons=True),
        )
    assert any("Nowhere" in str(w.message) for w in caught)
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        divvun_hfst.compile_strings(
            source, divvun_hfst.StringsOptions(warn_negative_weights=False)
        )


# [spec:hfst:req:python.compile-errors/test]
def test_verbose_compiles_log_what_they_say(caplog: pytest.LogCaptureFixture) -> None:
    with caplog.at_level(logging.INFO, logger="hfst"):
        divvun_hfst.regex("a", divvun_hfst.RegexOptions(verbose=True))
    assert all(record.name == "hfst" for record in caplog.records)


# [spec:hfst:req:python.compile/test]
def test_options_are_frozen_and_checked_when_made() -> None:
    options = divvun_hfst.RegexOptions(disjunct=True)
    assert repr(options) == "RegexOptions(disjunct=True)"
    with pytest.raises(dataclasses.FrozenInstanceError):
        options.disjunct = False  # type: ignore[misc]
    assert dataclasses.replace(options, semicolon=True).semicolon
    with pytest.raises(TypeError):
        divvun_hfst.RegexOptions(True)  # type: ignore[call-arg, arg-type]
    with pytest.raises(TypeError):
        divvun_hfst.RegexOptions(disjunct=1)  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        divvun_hfst.RegexOptions(format="sfst")  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        divvun_hfst.TwolcOptions(format="optimized-lookup-weighted")  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        divvun_hfst.XfstOptions(format="optimized-lookup-unweighted")  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        divvun_hfst.LexcOptions(quiet=True, verbose=True)
    with pytest.raises(ValueError):
        divvun_hfst.StringsOptions(log=True, log10=True)
    with pytest.raises(TypeError):
        divvun_hfst.StringsOptions(multichar_symbols="ab")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        divvun_hfst.AttOptions(epsilon=0)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        divvun_hfst.regex("a", divvun_hfst.LexcOptions())  # type: ignore[arg-type]
    for cls in (
        divvun_hfst.RegexOptions,
        divvun_hfst.LexcOptions,
        divvun_hfst.TwolcOptions,
        divvun_hfst.XfstOptions,
        divvun_hfst.PmatchOptions,
        divvun_hfst.StringsOptions,
        divvun_hfst.AttOptions,
        divvun_hfst.PrologOptions,
    ):
        made = cls()
        assert repr(made) == f"{cls.__name__}()"
        assert all(f.kw_only for f in dataclasses.fields(made))


# [spec:hfst:req:python.compile/test]
def test_compiling_releases_the_gil() -> None:
    source = "[a:b | a:c | a:d | a:e | b:a | c:a]^6"
    assert python_runs_during(lambda: [divvun_hfst.regex(source) for _ in range(5)]) > BUSY
