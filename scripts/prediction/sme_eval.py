"""Inputs and baselines for measuring North Sámi unit prediction.

Reads the analysed text sme_corpus.py writes and splits it into k folds
exactly as hfst predict-eval does: contiguous parts of the token list, part
p running from p*n//k to (p+1)*n//k. Subcommands:

  targets     Find each token's target as predict-eval does: its first
              reading the generator accepts and the unit grammar cuts into
              units. Writes the number of units and the lemma of each.
  mask        Write predict-eval inputs that score only some tokens: the
              others keep their readings, so models and contexts are the
              same, but their surface form is replaced by a middle dot,
              which the measure does not count, so they are not typed.
              Writes one input for each group of a split of the tokens, or
              for each token named with --split token.
  recordings  Write, for each fold, a predict-eval input and a recording of
              ranked word forms, to check this script's scoring against the
              harness. See write_recordings.
  baselines   Score word-form completion by the measure's rule for
              recorded suggestions: the unigram and bigram baselines,
              overall, by units in the word and by group.

The splits group tokens by what unit prediction is meant to help with:
  gradation   whether the lemma is a prefix of the word as written
  novelty     whether the word form, and else its lemma, is in the fold's
              training text

Run with python3 -I. Standard library only.
"""

import argparse
import bisect
import heapq
import random
import re
import subprocess
import sys
import unicodedata
from collections import Counter, defaultdict
from fnmatch import fnmatchcase
from pathlib import Path

MASK = "·"

# These must match the boundary and enclitic patterns of sme.toml.
BOUNDARY = ["+Der/*", "#"]
ENCLITIC = ["+Qst", "+Foc/*"]
SYMBOL = re.compile(r"\+[^+#]+|.")


def read_analysed(path):
    """Tokens as predict-eval reads them: (surface, readings, lines) for each
    block of hfst-lookup output, readings that are the surface with +? left
    out."""
    tokens = []
    text = Path(path).read_text(encoding="utf-8")
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
        tokens.append((surface, readings, lines))
    return tokens


def is_character(c):
    return not c.isspace() and not unicodedata.category(c).startswith("P")


def characters(word):
    return sum(1 for c in word if is_character(c))


def folds(n, k):
    """(scored, built_from) for each fold, as predict-eval's Corpus::folds."""
    bound = [p * n // k for p in range(k + 1)]
    return [
        (
            range(bound[p], bound[p + 1]),
            [r for r in (range(0, bound[p]), range(bound[p + 1], n)) if len(r)],
        )
        for p in range(k)
    ]


def sampled(tokens, rate, seed):
    """The tokens scored: each token the measure types, kept with
    probability rate by a seeded draw."""
    rng = random.Random(seed)
    keep = set()
    for i, (surface, _, _) in enumerate(tokens):
        draw = rng.random()
        if characters(surface) > 0 and draw < rate:
            keep.add(i)
    return keep


def segment(reading):
    """The units sme.toml cuts a reading into, as (kind, text) pairs, or None
    where the grammar refuses it. sme.toml has no prefixes or derivation
    tags."""
    symbols = SYMBOL.findall(reading)
    tag = [len(s) > 1 or s == "#" for s in symbols]
    n = len(symbols)

    def next_tag(at):
        return next((i for i in range(at, n) if tag[i]), n)

    def next_letter(at):
        return next((i for i in range(at, n) if not tag[i]), n)

    def matches(patterns, s):
        return any(fnmatchcase(s, p) for p in patterns)

    def next_enclitic(at):
        return next((i for i in range(at, n) if matches(ENCLITIC, symbols[i])), n)

    if n == 0 or tag[0]:
        return None
    units = []
    at = next_tag(0)
    units.append(("stem", "".join(symbols[:at])))
    while True:
        letter = next_letter(at)
        boundary = next(
            (i for i in range(at, letter) if matches(BOUNDARY, symbols[i])), None
        )
        if boundary is None:
            break
        units.append(("boundary", "".join(symbols[at : boundary + 1])))
        at = boundary + 1
        if at < n and not tag[at]:
            end = next_tag(at)
            units.append(("stem", "".join(symbols[at:end])))
            at = end
    if next_letter(at) < n:
        return None
    first = next_enclitic(at)
    if first > at:
        units.append(("ending", "".join(symbols[at:first])))
        at = first
    while at < n:
        end = next_enclitic(at + 1)
        units.append(("enclitic", "".join(symbols[at:end])))
        at = end
    return units


def generator_accepts(hfst, generator, readings):
    """The readings the generator writes a word for."""
    readings = sorted(readings)
    run = subprocess.run(
        [hfst, "lookup", "-q", "--pipe-mode=input", generator],
        input="".join(r + "\n" for r in readings),
        capture_output=True,
        text=True,
        check=True,
    )
    accepted = set()
    for block in run.stdout.split("\n\n"):
        for line in block.split("\n"):
            fields = line.split("\t")
            if len(fields) == 3 and fields[1] != fields[0] + "+?":
                accepted.add(fields[0])
    return accepted


def targets(args):
    tokens = read_analysed(args.analysed)
    readings = {r for _, rs, _ in tokens for r in rs}
    accepted = generator_accepts(args.hfst, args.generator, readings)
    with open(args.out, "w", encoding="utf-8") as out:
        out.write("token\tsurface\tunits\tlemma\treading\n")
        for i, (surface, rs, _) in enumerate(tokens):
            found = next(
                ((r, u) for r in rs if r in accepted for u in [segment(r)] if u),
                None,
            )
            if found is None:
                out.write(f"{i}\t{surface}\t0\t\t\n")
                continue
            reading, units = found
            out.write(f"{i}\t{surface}\t{len(units)}\t{units[0][1]}\t{reading}\n")
    print(
        f"{len(readings)} readings, {len(accepted)} accepted by the generator",
        file=sys.stderr,
    )


def read_targets(path):
    """(units, lemma) for each token."""
    rows = Path(path).read_text(encoding="utf-8").split("\n")[1:]
    found = []
    for row in rows:
        if row:
            fields = row.split("\t")
            found.append((int(fields[2]), fields[3]))
    return found


def same_letter(a, b):
    return a == b or a.lower() == b.lower()


def fits(start, word):
    """True when word starts with start, capitals aside."""
    return len(start) <= len(word) and all(
        same_letter(a, b) for a, b in zip(start, word)
    )


def training_counts(tokens, targets, ranges):
    """Word forms, in small letters, and lemmas counted in a fold's
    training text."""
    forms = Counter()
    lemmas = Counter()
    for r in ranges:
        for i in r:
            surface = tokens[i][0]
            if characters(surface) > 0:
                forms[surface.lower()] += 1
                if targets[i][0]:
                    lemmas[targets[i][1].lower()] += 1
    return forms, lemmas


def groups(tokens, targets, k, split):
    """The group of each token under a split."""
    found = [None] * len(tokens)
    for scored, built_from in folds(len(tokens), k):
        if split == "novelty":
            forms, lemmas = training_counts(tokens, targets, built_from)
        for i in scored:
            surface = tokens[i][0]
            units, lemma = targets[i]
            if split == "none":
                found[i] = "all"
            elif units == 0:
                found[i] = "no-target"
            elif split == "gradation":
                found[i] = "lemma-written" if fits(lemma, surface) else "lemma-changed"
            elif forms[surface.lower()]:
                found[i] = "form-seen"
            elif lemmas[lemma.lower()]:
                found[i] = "lemma-seen"
            else:
                found[i] = "both-unseen"
    return found


def masked_block(token):
    """A token's lines with its surface form masked. A token with no reading
    becomes the masked token with none, as hfst lookup writes one."""
    _, readings, lines = token
    if not readings:
        return f"{MASK}\t{MASK}+?\tinf\n"
    return "".join(MASK + line[line.index("\t") :] + "\n" for line in lines)


def mask(args):
    tokens = read_analysed(args.analysed)
    targets = read_targets(args.targets)
    if args.split == "token":
        keep = set(args.tokens)
        found = [f"{i}-{surface}" for i, (surface, _, _) in enumerate(tokens)]
    else:
        keep = sampled(tokens, args.sample, args.seed)
        found = groups(tokens, targets, args.folds, args.split)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    for group in sorted({found[i] for i in keep}):
        scored = 0
        with open(out / f"{args.split}-{group}.txt", "w", encoding="utf-8") as f:
            for i, token in enumerate(tokens):
                if i in keep and found[i] == group:
                    scored += 1
                    f.write("".join(line + "\n" for line in token[2]) + "\n")
                else:
                    f.write(masked_block(token) + "\n")
        print(f"{args.split}-{group}.txt\t{scored} tokens scored", file=sys.stderr)


class Forms:
    """Word forms of a training text, ranked by count: the forms that start
    with a prefix, capitals aside, most frequent first, ties in the order
    of their letters."""

    def __init__(self, counts):
        self.forms = sorted(counts)
        self.counts = counts
        self.cache = {}

    def top(self, prefix, n):
        key = (prefix, n)
        if key not in self.cache:
            lo = bisect.bisect_left(self.forms, prefix)
            hi = bisect.bisect_left(self.forms, prefix + "\U0010ffff")
            self.cache[key] = heapq.nsmallest(
                n, self.forms[lo:hi], key=lambda f: (-self.counts[f], f)
            )
        return self.cache[key]


def shown_as_typed(suggestions, word):
    """A suggestion the word starts with, capitals aside, is shown with the
    word's own capitals: the keyboard's shift state carries over."""
    return [word[: len(s)] if fits(s, word) else s for s in suggestions]


def bigram(forms, after, previous, word, prefix, n):
    """The forms seen after the previous token that start with the prefix,
    most frequent first, then the unigram's to make up n."""
    shown = []
    if previous is not None and previous in after:
        shown = after[previous].top(prefix.lower(), n)
    for form in forms.top(prefix.lower(), n):
        if len(shown) == n:
            break
        if form not in shown:
            shown.append(form)
    return shown_as_typed(shown, word)


def type_word(word, suggest):
    """Taps for a word, as predict-eval's Recording::type_word: pick the
    longest suggestion the word starts with that is longer than what is
    typed, or type the next letter."""
    typed = 0
    taps = 0
    while typed < len(word):
        prefix = word[:typed]
        longer = [
            len(s) for s in suggest(prefix) if word.startswith(s) and len(s) > typed
        ]
        if longer:
            taps += 1
            typed = max(longer)
        else:
            taps += is_character(word[typed])
            typed += 1
    return taps


def bigram_tables(tokens, ranges):
    """For each previous token, in small letters, the forms that follow it in
    the training text. No pair spans two ranges."""
    pairs = defaultdict(Counter)
    for r in ranges:
        for i in r:
            if i == r.start:
                continue
            surface = tokens[i][0]
            if characters(surface) > 0:
                pairs[tokens[i - 1][0].lower()][surface.lower()] += 1
    return {previous: Forms(c) for previous, c in pairs.items()}


class Tally:
    """Counts over words scored one way. A word is first, or shown, at a
    look when the suggestions then hold the whole word first, or among
    those shown. It is looked at before its first letter and after it."""

    def __init__(self):
        self.words = 0
        self.chars = 0
        self.taps = 0
        self.first = [0, 0]
        self.shown = [0, 0]

    def add(self, chars, taps, looks):
        self.words += 1
        self.chars += chars
        self.taps += taps
        for at, (first, shown) in enumerate(looks):
            self.first[at] += first
            self.shown[at] += shown

    def cells(self):
        def percent(count, of):
            return f"{100 * count / of:.1f}%" if of else "-"

        saved = percent(self.chars - self.taps, self.chars)
        return [saved] + [
            percent(counts[at], self.words)
            for at in (0, 1)
            for counts in (self.first, self.shown)
        ]


def suggester(kind, forms, after, previous, word, n):
    if kind == "unigram":
        return lambda prefix: shown_as_typed(forms.top(prefix.lower(), n), word)
    return lambda prefix: bigram(forms, after, previous, word, prefix, n)


def looks(word, suggest):
    """Whether the word is first, and whether it is shown, before its first
    letter and after it."""
    found = []
    for typed in (0, 1):
        shown = suggest(word[:typed])
        found.append((shown[:1] == [word], word in shown))
    return found


def baselines(args):
    tokens = read_analysed(args.analysed)
    targets = read_targets(args.targets)
    keep = sampled(tokens, args.sample, args.seed)
    found = groups(tokens, targets, args.folds, args.split)
    systems = [
        (f"{kind}-n{n}", kind, n) for kind in ("unigram", "bigram") for n in args.shown
    ]
    # tallies[system][(fold or "all", group, units)]
    tallies = {name: defaultdict(Tally) for name, _, _ in systems}
    for fold, (scored, built_from) in enumerate(folds(len(tokens), args.folds)):
        counts, _ = training_counts(tokens, targets, built_from)
        forms = Forms(counts)
        after = bigram_tables(tokens, built_from)
        for i in scored:
            if i not in keep:
                continue
            word = tokens[i][0]
            previous = tokens[i - 1][0].lower() if i > scored.start else None
            chars = characters(word)
            units = targets[i][0]
            keys = [(fold, "*", "*"), ("all", "*", "*"), ("all", "*", units)]
            if args.split != "none":
                keys += [("all", found[i], "*"), ("all", found[i], units)]
            for name, kind, n in systems:
                suggest = suggester(kind, forms, after, previous, word, n)
                taps = type_word(word, suggest)
                seen = looks(word, suggest)
                for key in keys:
                    tallies[name][key].add(chars, taps, seen)
    print_tables(systems, tallies)


def print_tables(systems, tallies):
    """One line for each fold, group and number of units in the word, with
    each system's keystrokes saved and how often the word was first and
    shown at the start (0) and after one letter (1)."""
    keys = sorted(
        {key for t in tallies.values() for key in t},
        key=lambda key: (str(key[0]) != "all", str(key[0]), key[1], str(key[2])),
    )
    names = [name for name, _, _ in systems]
    measures = ["saved", "first0", "shown0", "first1", "shown1"]
    print(
        "fold\tgroup\tunits\twords\tchars\t"
        + "\t".join(f"{name}:{m}" for name in names for m in measures)
    )
    for key in keys:
        first = tallies[names[0]][key]
        print(
            "\t".join(str(part) for part in key)
            + f"\t{first.words}\t{first.chars}\t"
            + "\t".join(c for name in names for c in tallies[name][key].cells())
        )


def write_recordings(args):
    """For each fold, an input of two halves for predict-eval --folds 2: the
    fold's training text, masked so nothing in it is typed, then the fold's
    scored tokens with their readings taken away, so unit prediction types
    them in full without a walk, padded with masked tokens to the length of
    the first half. The second half is scored with the recording, built
    from the first half's words."""
    tokens = read_analysed(args.analysed)
    keep = sampled(tokens, args.sample, args.seed)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    filler = f"{MASK}\t{MASK}+?\tinf\n\n"
    for fold, (scored, built_from) in enumerate(folds(len(tokens), args.folds)):
        counts = Counter()
        first = []
        for r in built_from:
            for i in r:
                surface = tokens[i][0]
                if characters(surface) > 0:
                    counts[surface.lower()] += 1
                first.append(masked_block(tokens[i]) + "\n")
            first.append(filler)
        second = []
        lines = set()
        forms = Forms(counts)
        for i in scored:
            word = tokens[i][0]
            if i not in keep:
                second.append(filler)
                continue
            second.append(f"{word}\t{word}+?\tinf\n\n")
            for typed in range(len(word)):
                prefix = word[:typed]
                shown = shown_as_typed(forms.top(prefix.lower(), args.shown[0]), word)
                lines.add("\t".join([word, prefix] + shown) + "\n")
        second.extend([filler] * (len(first) - len(second)))
        with open(out / f"fold{fold}.txt", "w", encoding="utf-8") as f:
            f.write("".join(first) + "".join(second))
        with open(out / f"fold{fold}.tsv", "w", encoding="utf-8") as f:
            f.write("".join(sorted(lines)))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)

    def common(p, with_targets=True):
        p.add_argument("--analysed", required=True)
        if with_targets:
            p.add_argument("--targets", required=True)
        p.add_argument("--folds", type=int, default=5)
        p.add_argument("--sample", type=float, default=1.0)
        p.add_argument("--seed", type=int, default=1)

    t = sub.add_parser("targets")
    t.add_argument("--analysed", required=True)
    t.add_argument("--hfst", required=True)
    t.add_argument("--generator", required=True)
    t.add_argument("--out", required=True)
    m = sub.add_parser("mask")
    common(m)
    m.add_argument(
        "--split", choices=["none", "gradation", "novelty", "token"], default="none"
    )
    m.add_argument(
        "--tokens",
        type=lambda s: [int(x) for x in s.split(",")],
        default=[],
        help="with --split token, the indices of the tokens to score",
    )
    m.add_argument("--out-dir", required=True)
    r = sub.add_parser("recordings")
    common(r, with_targets=False)
    r.add_argument("--shown", type=lambda s: [int(x) for x in s.split(",")], default=[3])
    r.add_argument("--out-dir", required=True)
    b = sub.add_parser("baselines")
    common(b)
    b.add_argument("--split", choices=["none", "gradation", "novelty"], default="none")
    b.add_argument("--shown", type=lambda s: [int(x) for x in s.split(",")], default=[3, 4])
    args = parser.parse_args()
    {
        "targets": targets,
        "mask": mask,
        "recordings": write_recordings,
        "baselines": baselines,
    }[args.command](args)


if __name__ == "__main__":
    main()
