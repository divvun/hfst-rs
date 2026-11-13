"""Tests of the Greenlandic evaluation script's own logic, without hfst or
lang-kal.

Run with python3 -I scripts/prediction/test_kal_scripts.py.
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import kal_corpus  # noqa: E402


class Corpus(unittest.TestCase):
    def test_punctuation_at_the_edges_is_split_off(self):
        self.assertEqual(
            kal_corpus.tokens('"Qarsaaq," taanna (aamma).', 10),
            ['"', "Qarsaaq", ",", '"', "taanna", "(", "aamma", ")", "."],
        )
        self.assertEqual(kal_corpus.tokens("a-b c", 10), ["a-b", "c"])

    def test_only_the_first_tokens_are_kept(self):
        self.assertEqual(kal_corpus.tokens("a b, c d", 3), ["a", "b", ","])

    def test_readings_are_lightest_first_and_once(self):
        output = (
            "nuna\tnuna+N+Abs+Sg\t1.5\n"
            "nuna\tnuna+N+Abs+Pl\t0.5\n"
            "nuna\tnuna+N+Abs+Sg\t2.0\n\n"
            "xyz\txyz+?\tinf\n\n"
        )
        found = kal_corpus.readings(output)
        self.assertEqual(found["nuna"], [(0.5, "nuna+N+Abs+Pl"), (1.5, "nuna+N+Abs+Sg")])
        self.assertEqual(found["xyz"], [])

    def test_a_token_becomes_lookup_lines(self):
        self.assertEqual(
            kal_corpus.block("nuna", [(0.0, "nuna+N+Abs+Sg")]),
            "nuna\tnuna+N+Abs+Sg\t0.000000\n\n",
        )
        self.assertEqual(kal_corpus.block("xyz", []), "xyz\txyz+?\tinf\n\n")


if __name__ == "__main__":
    unittest.main()
