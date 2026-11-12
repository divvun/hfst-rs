"""Turn lang-crk's tagged frequency lists into analysed text for
hfst predict-eval.

The lists hold word types, not running text: each line is a count, a space,
a word, and the word's analyses, each after a tab. Their analyses use an
older tag set than today's lang-crk (a +Prs tag, y for ý in lemmas, other
pronoun and particle tags), which the generator refuses. So each word is
analysed again with today's normative analyser, and the generator's own
check keeps only readings it accepts. A reading that agrees with one of the
list's analyses, once +Prs is dropped and ý read as y, comes first, so that
the first reading is the list's where today's analyser has it.

A word is left out of the text when it is punctuation, when the list marks
every analysis of it as English (+Eng) or misspelt (+Err/), or when today's
analyser does not know it. Each is counted by why.

Each word type is expanded into its count of tokens and the tokens are
shuffled with a fixed seed. The text is hfst-lookup output: one block of
lines per token, each line the word, a tab, one reading, a tab and a weight,
and a blank line after the block. After each word comes the token "." with
no reading, which the model builder and the harness both read as a break: no
run of units spans it, and every word is typed with no units before it.
Real word order is not known, so no order is invented.

Typing a word with unit prediction is slow, so --typed N types only N
tokens, drawn at random with the seed. The others are written with the
surface "_", which has no character the harness counts: the model builder
counts their readings, the harness does not type them, and another system's
recording is not asked for them. Each fold's model is then built from all
the text outside the fold, and a random sample of the fold is scored.

The harness splits a text into k contiguous parts of nearly equal numbers
of tokens, the part i being tokens i*n//k to (i+1)*n//k. The script writes
that same split: fold-I.txt holds the tokens part I scores, so that another
system's suggestions can be recorded and scored part by part.

Run with python3 -I. Standard library only.
"""

import argparse
import collections
import os
import random
import re
import subprocess
import sys
import unicodedata

SEPARATOR = ".\t.+?\tinf\n\n"
UNTYPED = "_"
LINE = re.compile(r"^ *(\d+) (.*)$")


def read_lists(paths):
    """Each list's entries: (count, word, analyses)."""
    entries = []
    for path in paths:
        with open(path, encoding="utf-8") as lines:
            for number, line in enumerate(lines, 1):
                line = line.rstrip("\n")
                if not line:
                    continue
                match = LINE.match(line)
                if not match:
                    sys.exit(f"{path}:{number}: not a count, a space and a word")
                fields = match.group(2).split("\t")
                analyses = [field for field in fields[1:] if field]
                entries.append((int(match.group(1)), fields[0], analyses))
    return entries


def lookup(hfst, transducer, inputs):
    """What hfst lookup writes for each input: a list of outputs, empty when
    the transducer has none."""
    text = "".join(item + "\n" for item in inputs)
    done = subprocess.run(
        [hfst, "lookup", "-q", transducer],
        input=text,
        capture_output=True,
        text=True,
        check=True,
    )
    outputs = {item: [] for item in inputs}
    for line in done.stdout.splitlines():
        fields = line.split("\t")
        if len(fields) != 3:
            continue
        item, output, _ = fields
        if item in outputs and output != item + "+?":
            outputs[item].append(output)
    return outputs


def agreeing(analysis):
    """An analysis as the old tag set and the new one can both be read."""
    return analysis.replace("+Prs", "").replace("ý", "y")


def ordered(readings, listed):
    """Today's readings, those that agree with a listed analysis first, in
    the list's order, then the rest in the analyser's order."""
    rank = {}
    for at, analysis in enumerate(listed):
        rank.setdefault(agreeing(analysis), at)
    first = sorted(
        (r for r in readings if agreeing(r) in rank), key=lambda r: rank[agreeing(r)]
    )
    return first + [r for r in readings if agreeing(r) not in rank]


def characters(word):
    """The characters the harness counts: not spaces, not punctuation."""
    return sum(
        1
        for c in word
        if not c.isspace() and not unicodedata.category(c).startswith("P")
    )


def marked(listed):
    """True when the list marks every analysis as English or misspelt."""
    return bool(listed) and all("+Eng" in a or "+Err/" in a for a in listed)


def punctuation(word, listed):
    return characters(word) == 0 or (
        bool(listed) and all("+CLB" in a or "+PUNCT" in a for a in listed)
    )


def block(word, readings):
    return "".join(f"{word}\t{r}\t0.000000\n" for r in readings) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("lists", nargs="+", help="tagged frequency lists")
    parser.add_argument("--hfst", required=True, help="the hfst binary")
    parser.add_argument("--analyser", required=True, help="normative analyser .hfstol")
    parser.add_argument("--generator", required=True, help="generator .hfstol")
    parser.add_argument("--out", required=True, help="directory to write to")
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--folds", type=int, default=10)
    parser.add_argument("--typed", type=int, help="tokens to type; all when not given")
    args = parser.parse_args()

    entries = read_lists(args.lists)
    words = sorted({word for _, word, _ in entries})
    analysed = lookup(args.hfst, args.analyser, words)
    readings = sorted({r for found in analysed.values() for r in found})
    forms = lookup(args.hfst, args.generator, readings)

    tokens = []
    left_out = collections.Counter()
    agreed = 0
    unwritten = 0
    for count, word, listed in entries:
        if punctuation(word, listed):
            left_out["punctuation"] += count
            continue
        if marked(listed):
            left_out["marked by the list as English or misspelt"] += count
            continue
        accepted = [r for r in analysed[word] if forms[r]]
        if not accepted:
            left_out["not analysed by today's normative analyser"] += count
            continue
        accepted = ordered(accepted, listed)
        if agreeing(accepted[0]) in {agreeing(a) for a in listed}:
            agreed += count
        if forms[accepted[0]][0].lower() != word.lower():
            unwritten += count
        tokens.extend([(word, tuple(accepted))] * count)

    shuffle = random.Random(args.seed)
    shuffle.shuffle(tokens)
    typed = range(len(tokens))
    if args.typed is not None:
        typed = set(shuffle.sample(typed, args.typed))
    blocks = []
    for at, (word, readings) in enumerate(tokens):
        shown = word if at in typed else UNTYPED
        blocks += [block(shown, readings), SEPARATOR]
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "text.txt"), "w", encoding="utf-8") as out:
        out.write("".join(blocks))
    n = len(blocks)
    bound = [part * n // args.folds for part in range(args.folds + 1)]
    for part in range(args.folds):
        path = os.path.join(args.out, f"fold-{part + 1}.txt")
        with open(path, "w", encoding="utf-8") as out:
            out.write("".join(blocks[bound[part] : bound[part + 1]]))

    types = len({word for word, _ in tokens})
    print(f"kept: {len(tokens)} tokens, {types} distinct words", file=sys.stderr)
    print(f"typed: {len(typed)} tokens", file=sys.stderr)
    print(f"kept, first reading agrees with the list: {agreed} tokens", file=sys.stderr)
    print(
        f"kept, the generator's first form of the first reading is another word: {unwritten} tokens",
        file=sys.stderr,
    )
    for reason, count in left_out.most_common():
        print(f"left out, {reason}: {count} tokens", file=sys.stderr)
    print(f"harness tokens: {n}, fold bounds: {bound}", file=sys.stderr)


if __name__ == "__main__":
    main()
