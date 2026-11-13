"""Score word-form completion alone on the Plains Cree comparison text, as
hfst predict-eval's harness scores it with units off, on exactly the tokens
a masked text types.

crk_corpus.py --typed writes the surface "_" for every token it does not
type. The model builder counts each counted token's surface as a word form,
so a masked text would teach the forms model "_" in place of nearly every
training word. This script reads the unmasked text, which crk_corpus.py
writes in the same order when --typed is left out, for the forms each part
is scored with, and the masked text for which tokens to type.

Every word in the text is followed by a token with no reading, which breaks
the context, so each part's form model is a unigram: the forms of the
tokens outside the part that have a reading, capitals folded, by count. The
user looks before each letter, starting with none typed. It picks the word,
for one tap, when it is among the N forms shown, best first by count then
by the bytes of the form, and otherwise types the next letter; a form with
no counted character is never shown. A word never shown is typed in full.
This is the harness's user, and on the whole unmasked text this script and
the harness spend the same taps.

Run with python3 -I. Standard library only.
"""

import argparse
import bisect
import unicodedata
from collections import Counter


def read_tokens(path):
    """(surface, has a reading) for each block of hfst-lookup output."""
    tokens = []
    with open(path, encoding="utf-8") as f:
        text = f.read()
    for block in text.split("\n\n"):
        lines = [line for line in block.split("\n") if line]
        if not lines:
            continue
        surface = lines[0].split("\t")[0]
        readings = [
            line.split("\t")[1]
            for line in lines
            if line.split("\t")[1] != surface + "+?"
        ]
        tokens.append((surface, bool(readings)))
    return tokens


def is_character(c):
    return not c.isspace() and not unicodedata.category(c).startswith("P")


def fold(text):
    return "".join(c.lower() for c in text)


class Forms:
    """The forms of a part's training text, the best N for a start."""

    def __init__(self, counts):
        self.counts = counts
        self.forms = sorted(f for f in counts if any(is_character(c) for c in f))
        self.cache = {}

    def top(self, start, n):
        key = (start, n)
        if key not in self.cache:
            lo = bisect.bisect_left(self.forms, start)
            hi = bisect.bisect_left(self.forms, start + "\U0010ffff")
            ranked = sorted(
                self.forms[lo:hi], key=lambda f: (-self.counts[f], f.encode())
            )
            self.cache[key] = ranked[:n]
        return self.cache[key]


def type_word(word, forms, n):
    """Taps for a word: a look before each letter, a pick of the whole word
    when shown, or every letter typed."""
    letters = list(word)
    taps = 0
    for at, letter in enumerate(letters):
        if fold(word) in forms.top(fold("".join(letters[:at])), n):
            return taps + 1
        taps += is_character(letter)
    return taps


def score(full, typed, k, shown):
    """(words, characters, taps) for each N, over the tokens `typed` holds,
    each part scored with the forms of the others."""
    n = len(full)
    bound = [p * n // k for p in range(k + 1)]
    totals = {each: [0, 0, 0] for each in shown}
    for p in range(k):
        counts = Counter()
        for part in (range(0, bound[p]), range(bound[p + 1], n)):
            for i in part:
                surface, counted = full[i]
                if counted:
                    counts[fold(surface)] += 1
        forms = Forms(counts)
        for i in range(bound[p], bound[p + 1]):
            word = full[i][0]
            characters = sum(1 for c in word if is_character(c))
            if i not in typed or characters == 0:
                continue
            for each in shown:
                totals[each][0] += 1
                totals[each][1] += characters
                totals[each][2] += type_word(word, forms, each)
    return totals


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("full", help="the unmasked text, crk_corpus.py without --typed")
    parser.add_argument("masked", help="the masked text, crk_corpus.py --typed")
    parser.add_argument("--folds", type=int, default=10)
    parser.add_argument(
        "--shown", type=lambda s: [int(x) for x in s.split(",")], default=[3, 10]
    )
    args = parser.parse_args()
    full = read_tokens(args.full)
    masked = read_tokens(args.masked)
    if len(full) != len(masked):
        raise SystemExit("the two texts do not hold the same tokens")
    typed = {i for i, (surface, _) in enumerate(masked) if surface != "_"}
    for each, (words, characters, taps) in score(full, typed, args.folds, args.shown).items():
        saved = 100 * (characters - taps) / characters
        print(f"n{each}\t{words} words\t{characters} chars\t{taps} taps\t{saved:.2f}% saved")


if __name__ == "__main__":
    main()
