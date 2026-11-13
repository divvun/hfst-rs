"""Tests of the North Sámi evaluation scripts' own logic, without hfst,
lang-sme or corpus-sme.

Run with python3 -I scripts/prediction/test_sme_scripts.py.
"""

import os
import random
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import sme_corpus  # noqa: E402
import sme_eval  # noqa: E402

DOCUMENT = """<document xml:lang="sme"><header><title>T</title></header><body>
<p type="title">Bajilčála</p>
<p>Mun  lean <span xml:lang="eng">really</span> girjjiid lohkan.</p>
<p xml:lang="nob">Dette er norsk.</p>
<list><p type="listitem">Listu</p></list>
<p type="text">Álbmot- ja <em>servodat</em>.</p>
</body></document>
"""


def analysed(tokens):
    """hfst-lookup output for (surface, readings) pairs."""
    blocks = []
    for surface, readings in tokens:
        lines = [f"{surface}\t{r}\t0.000000" for r in readings]
        blocks.append("\n".join(lines or [f"{surface}\t{surface}+?\tinf"]))
    return "\n\n".join(blocks) + "\n\n"


class Corpus(unittest.TestCase):
    def test_tokens_keep_hyphens_and_colons(self):
        text = "Sámi- ja 2010-logu NRK:s, “girjjiid”."
        self.assertEqual(
            sme_corpus.TOKEN.findall(text),
            ["Sámi-", "ja", "2010-logu", "NRK:s", ",", "“", "girjjiid", "”", "."],
        )
        self.assertTrue(sme_corpus.is_word("2010"))
        self.assertFalse(sme_corpus.is_word("–"))

    def test_only_north_sami_running_text_is_kept(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "doc.xml"
            path.write_text(DOCUMENT, encoding="utf-8")
            self.assertEqual(
                sme_corpus.paragraphs(path),
                ["Mun lean girjjiid lohkan.", "Álbmot- ja servodat."],
            )
            path.write_text(
                DOCUMENT.replace('lang="sme"', 'lang="nob"', 1), encoding="utf-8"
            )
            self.assertEqual(sme_corpus.paragraphs(path), [])

    def test_a_genre_stops_at_its_words(self):
        with tempfile.TemporaryDirectory() as directory:
            genre = Path(directory) / "news"
            genre.mkdir()
            for name in "abc":
                (genre / f"{name}.xml").write_text(DOCUMENT, encoding="utf-8")
            chosen = sme_corpus.choose_documents(
                Path(directory), "news", 5, 1000, random.Random(1)
            )
            self.assertEqual(len(chosen), 1)
            self.assertEqual(chosen[0][1][:2], ["Mun", "lean"])


class Folds(unittest.TestCase):
    def test_parts_match_the_harness(self):
        self.assertEqual(
            sme_eval.folds(10, 3),
            [
                (range(0, 3), [range(3, 10)]),
                (range(3, 6), [range(0, 3), range(6, 10)]),
                (range(6, 10), [range(0, 6)]),
            ],
        )

    def test_characters_leave_out_punctuation(self):
        self.assertEqual(sme_eval.characters("álbmot-"), 6)
        self.assertEqual(sme_eval.characters(sme_eval.MASK), 0)


class Segment(unittest.TestCase):
    def test_units_follow_the_grammar(self):
        cut = sme_eval.segment
        self.assertEqual(cut("girji+N+Pl+Acc"), [("stem", "girji"), ("ending", "+N+Pl+Acc")])
        self.assertEqual(
            [kind for kind, _ in cut("2010+Num+Arab+Cmp/Hyph+Cmp#lohku+N+Sg+Acc")],
            ["stem", "boundary", "stem", "ending"],
        )
        self.assertEqual(
            cut("oahppat+V+TV+Der/NomAg+N+Sg+Nom"),
            [("stem", "oahppat"), ("boundary", "+V+TV+Der/NomAg"), ("ending", "+N+Sg+Nom")],
        )
        self.assertEqual(
            cut("leat+V+IV+Ind+Prs+Sg3+Qst"),
            [("stem", "leat"), ("ending", "+V+IV+Ind+Prs+Sg3"), ("enclitic", "+Qst")],
        )
        self.assertIsNone(cut("+N+Sg+Nom"))


class Typing(unittest.TestCase):
    def test_the_longest_fitting_suggestion_is_picked(self):
        shown = {"": ["gi", "girj", "ja"], "girjj": ["girjjiid"]}
        self.assertEqual(sme_eval.type_word("girjjiid", lambda p: shown.get(p, [])), 3)
        self.assertEqual(sme_eval.type_word("álbmot-", lambda p: []), 6)

    def test_the_whole_user_picks_only_the_word(self):
        shown = {"": ["gi", "girj", "ja"], "girjj": ["girjjiid"]}
        self.assertEqual(
            sme_eval.type_word("girjjiid", lambda p: shown.get(p, []), whole=True), 6
        )
        self.assertEqual(sme_eval.type_word("ja", lambda p: ["ja"], whole=True), 1)
        self.assertEqual(sme_eval.type_word("jagi", lambda p: ["ja"], whole=True), 4)

    def test_forms_rank_by_count_then_letters(self):
        forms = sme_eval.Forms(Counter({"ja": 5, "jahki": 2, "jagi": 2, "lea": 3}))
        self.assertEqual(forms.top("ja", 2), ["ja", "jagi"])
        self.assertEqual(forms.top("", 2), ["ja", "lea"])
        self.assertEqual(forms.top("x", 2), [])

    def test_suggestions_take_the_capitals_typed(self):
        self.assertEqual(
            sme_eval.shown_as_typed(["sámi", "sápmi"], "Sámi"), ["Sámi", "sápmi"]
        )

    def test_the_bigram_fills_up_from_the_unigram(self):
        forms = sme_eval.Forms(Counter({"ja": 5, "lea": 3, "leat": 1}))
        after = {"ja": sme_eval.Forms(Counter({"leat": 2}))}
        self.assertEqual(
            sme_eval.bigram(forms, after, "ja", "leat", "", 3), ["leat", "ja", "lea"]
        )
        self.assertEqual(sme_eval.bigram(forms, after, None, "lea", "l", 1), ["lea"])

    def test_words_are_looked_for_twice(self):
        shown = {"": ["lea", "ja"], "j": ["ja"]}
        self.assertEqual(
            sme_eval.looks("ja", lambda p: shown[p]), [(False, True), (True, True)]
        )
        tally = sme_eval.Tally()
        tally.add(2, 1, [(False, True), (True, True)])
        self.assertEqual(tally.cells(), ["50.0%", "0.0%", "100.0%", "100.0%", "100.0%"])


class Groups(unittest.TestCase):
    TOKENS = [
        ("girji", ["girji+N+Sg+Nom"]),
        ("ja", ["ja+CC"]),
        ("Oslo", []),
        ("girjjiid", ["girji+N+Pl+Acc", "girji+N+Pl+Gen"]),
        ("ja", ["ja+CC"]),
        ("mánáid", ["mánná+N+Pl+Acc"]),
    ]

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "analysed.txt"
        path.write_text(analysed(self.TOKENS), encoding="utf-8")
        self.tokens = sme_eval.read_analysed(path)
        self.targets = [
            (len(units), units[0][1]) if units else (0, "")
            for units in (sme_eval.segment(r[0]) if r else None for _, r, _ in self.tokens)
        ]

    def test_unknown_tokens_have_no_reading(self):
        self.assertEqual(self.tokens[2][:2], ("Oslo", []))
        self.assertEqual(len(self.tokens[3][1]), 2)

    def test_gradation_compares_the_lemma_with_the_word(self):
        self.assertEqual(
            sme_eval.groups(self.tokens, self.targets, 2, "gradation"),
            ["lemma-written", "lemma-written", "no-target"]
            + ["lemma-changed", "lemma-written", "lemma-changed"],
        )

    def test_novelty_looks_in_the_other_parts_only(self):
        self.assertEqual(
            sme_eval.groups(self.tokens, self.targets, 2, "novelty"),
            ["lemma-seen", "form-seen", "no-target"]
            + ["lemma-seen", "form-seen", "both-unseen"],
        )

    def test_a_masked_token_keeps_its_readings(self):
        self.assertEqual(
            sme_eval.masked_block(self.tokens[1]), f"{sme_eval.MASK}\tja+CC\t0.000000\n"
        )
        self.assertEqual(
            sme_eval.masked_block(self.tokens[2]), f"{sme_eval.MASK}\t{sme_eval.MASK}+?\tinf\n"
        )


if __name__ == "__main__":
    unittest.main()
