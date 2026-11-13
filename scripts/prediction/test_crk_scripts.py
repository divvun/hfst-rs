"""Tests of the Plains Cree comparison scripts' own logic, without hfst,
divvunspell or lang-crk.

Run with python3 -I scripts/prediction/test_crk_scripts.py.
"""

import math
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import crk_corpus  # noqa: E402
import crk_forms  # noqa: E402
import crk_record  # noqa: E402
import crk_speller  # noqa: E402
import crk_sum_folds  # noqa: E402


def written_file(directory, name, text):
    path = os.path.join(directory, name)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return path


class Corpus(unittest.TestCase):
    def test_lines_hold_count_word_and_analyses(self):
        with tempfile.TemporaryDirectory() as directory:
            path = written_file(directory, "list.txt", "   3 nipâw\tA\tB\t\n  12 ,\t,+CLB\n")
            self.assertEqual(
                crk_corpus.read_lists([path]),
                [(3, "nipâw", ["A", "B"]), (12, ",", [",+CLB"])],
            )

    def test_readings_agreeing_with_the_list_come_first(self):
        readings = ["aýa+N+A+Sg", "aya+Pron", "aya+Ipc"]
        listed = ["aya+Ipc", "aya+N+A+Prs+Sg"]
        self.assertEqual(
            crk_corpus.ordered(readings, listed),
            ["aya+Ipc", "aýa+N+A+Sg", "aya+Pron"],
        )

    def test_words_the_list_marks_are_left_out(self):
        self.assertTrue(crk_corpus.marked(["the+Eng", "isi+Ipc+Err/Orth"]))
        self.assertFalse(crk_corpus.marked(["âw+Ipc", "awa+Pron+Err/Orth"]))
        self.assertFalse(crk_corpus.marked([]))
        self.assertTrue(crk_corpus.punctuation(".", []))
        self.assertTrue(crk_corpus.punctuation("̵", ["̵+PUNCT"]))
        self.assertFalse(crk_corpus.punctuation("ê-kîwêt", ["PV/e+kîwêw+V+AI+Cnj+3Sg"]))

    def test_a_block_ends_with_a_blank_line(self):
        self.assertEqual(
            crk_corpus.block("êkwa", ("êkwa+Ipc", "êkwa+Interj")),
            "êkwa\têkwa+Ipc\t0.000000\nêkwa\têkwa+Interj\t0.000000\n\n",
        )


class Record(unittest.TestCase):
    def test_starts_run_up_to_the_last_letter(self):
        self.assertEqual(crk_record.starts("êsa"), ["", "ê", "ês"])

    def test_typed_words_skip_uncounted_tokens(self):
        text = "êkwa\têkwa+Ipc\t0\n\n.\t.+?\tinf\n\n_\tawa+Pron\t0\n\nawa\tawa+Pron\t0\n"
        with tempfile.TemporaryDirectory() as directory:
            path = written_file(directory, "text.txt", text)
            self.assertEqual(crk_record.typed_words(path), ["êkwa", "awa"])

    def test_complete_word_helps_only_as_whole_word(self):
        self.assertEqual(crk_record.written("ê-~", "ê-nipât", False), "ê-")
        self.assertEqual(crk_record.written("ê-nipât", "ê-nipât", False), "ê-nipât")
        self.assertEqual(crk_record.written("nipâ", "nipâw", False), "nipâ ")
        self.assertEqual(crk_record.written("nipâ", "nipâw", True), "nipâ")

    def test_useful_suggestion_takes_word_further(self):
        found = {"n": ["nipâ", "ni~"], "ni": ["nipâw", "nipi~"], "nip": ["nipâ~"], "nipâ": []}
        self.assertEqual(crk_record.useful_share(["nipâw"], found, 1), (2, 4))
        self.assertEqual(crk_record.useful_share(["nipâw"], found, 2), (3, 4))


class Speller(unittest.TestCase):
    # Each form and what the gawk program in
    # src/derivation/morpheme-completion-weighting.sh prints for it.
    MERGED = {
        "ni<nipâ>n": "ni<nipân$",
        "nipâ>w": "nipâw$",
        "ê-<nipâ>t": "ê-<nipât$",
        "ni<kiskinwahamâkosi>nân": "ni<kiskinwahamâkosi>nân$",
        "a<bcd>efg>hi": "a<bcd>efghi$",
        "ê-/<pê>kîwê>yâ>n": "ê-/pê>kîwêyân$",
    }

    def test_forms_merge_as_the_shell_script_does(self):
        for form, merged in self.MERGED.items():
            self.assertEqual(crk_speller.merged(form), merged, form)

    def test_starts_weighted_by_their_share(self):
        weighted = crk_speller.weights(["ni<nipân$", "nipâw$", "ni<pê>yâ$"])
        self.assertAlmostEqual(weighted["ni"], -math.log(2 / 3))
        self.assertAlmostEqual(weighted["ninipân$"], -math.log(1 / 3))
        self.assertAlmostEqual(weighted["nipê"], -math.log(1 / 3))
        self.assertEqual(set(weighted), {"ni", "ninipân$", "nipâw$", "nipê", "nipêyâ$"})
        self.assertTrue(crk_speller.lexc(weighted).startswith("LEXICON Root\nni # "))

    def test_training_leaves_out_the_part_scored(self):
        text = "".join(
            f"{word}\t{reading}\t0\n\n.\t.+?\tinf\n\n"
            for word, reading in [("a", "a+Ipc"), ("b", "b+N+Prop"), ("c", "c+Ipc"), ("d", "d+Ipc")]
        )
        with tempfile.TemporaryDirectory() as directory:
            path = written_file(directory, "text.txt", text)
            self.assertEqual(crk_speller.training_readings(path, 2, 1), ["c+Ipc", "d+Ipc"])
            self.assertEqual(crk_speller.training_readings(path, 2, None), ["a+Ipc", "c+Ipc", "d+Ipc"])


REPORT = """\
                              o1 stupid-backoff n3              held-out n3
units        words    chars     saved   first   shown    full     saved   first   shown    full
all             10      100     10.0%   40.0%   60.0%       1     20.0%       -       -       0
2               10      100     10.0%   40.0%   60.0%       1     20.0%       -       -       0
"""


class Forms(unittest.TestCase):
    def test_forms_are_picked_only_whole(self):
        forms = crk_forms.Forms({"nipâw": 3, "nipâwak": 1, "ni": 5, ".": 9})
        self.assertEqual(forms.top("", 2), ["ni", "nipâw"])
        self.assertEqual(crk_forms.type_word("nipâw", forms, 2), 1)
        self.assertEqual(crk_forms.type_word("Nipâwak", forms, 1), 7)
        self.assertEqual(crk_forms.type_word("Nipâwak", forms, 3), 1)
        self.assertEqual(crk_forms.type_word("nipi", forms, 3), 4)

    def test_parts_count_the_other_parts_only(self):
        full = [("nipâw", True), (".", False), ("nipâw", True), ("_x", False)]
        totals = crk_forms.score(full, {0, 2}, 2, [1])
        self.assertEqual(totals[1], [2, 10, 2])
        alone = crk_forms.score(full, {0}, 2, [1])
        self.assertEqual(alone[1], [1, 5, 1])

    def test_lookup_blocks_become_tokens(self):
        with tempfile.TemporaryDirectory() as directory:
            text = written_file(directory, "t.txt", "a\ta+Ipc\t0\n\n.\t.+?\tinf\n\n")
            self.assertEqual(crk_forms.read_tokens(text), [("a", True), (".", False)])


class SumFolds(unittest.TestCase):
    def test_parts_are_summed_by_characters(self):
        other = REPORT.replace("      100", "      300").replace("20.0%", "40.0%")
        with tempfile.TemporaryDirectory() as directory:
            paths = [
                written_file(directory, "1.txt", REPORT),
                written_file(directory, "2.txt", other),
            ]
            names, groups, totals = crk_sum_folds.total(paths)
        self.assertEqual(names, ["o1 stupid-backoff n3", "held-out n3"])
        self.assertEqual(groups, ["all", "2"])
        characters, taps = totals[("held-out n3", "all")]
        self.assertEqual(characters, 400)
        self.assertAlmostEqual(taps, 80 + 180)


if __name__ == "__main__":
    unittest.main()
