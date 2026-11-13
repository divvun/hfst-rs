"""Turn the start of lang-kal's test corpus into analysed text for
hfst predict-eval.

The corpus is split at spaces. Characters that are neither letters nor
digits at the start or end of a piece are tokens of their own, one each, in
their order; what lies between them is a word. The first --tokens tokens
are kept. Each distinct token is looked up once with hfst lookup in the
analyser; a token's readings are written lightest first, then in the order
of their text, each once, as hfst-lookup output: the token, a tab, the
reading, a tab and the weight, with a blank line after each token. A token
the analyser does not know is written as hfst lookup writes it, with the
reading token+? and an infinite weight.

Run with python3 -I. Standard library only.
"""

import argparse
import subprocess


def tokens(text, limit):
    """The first `limit` tokens of the text."""
    found = []
    for piece in text.split():
        start, end = 0, len(piece)
        while start < end and not piece[start].isalnum():
            start += 1
        while end > start and not piece[end - 1].isalnum():
            end -= 1
        found.extend(piece[:start])
        if start < end:
            found.append(piece[start:end])
        found.extend(piece[end:])
        if len(found) >= limit:
            break
    return found[:limit]


def readings(output):
    """Each looked-up token's readings from hfst lookup's output, lightest
    first, then by their text, each once."""
    found = {}
    for block in output.split("\n\n"):
        for line in block.split("\n"):
            fields = line.split("\t")
            if len(fields) != 3:
                continue
            word, reading, weight = fields
            listed = found.setdefault(word, [])
            if reading != word + "+?":
                listed.append((float(weight), reading))
    for word, listed in found.items():
        kept = []
        for weight, reading in sorted(listed):
            if reading not in [known for _, known in kept]:
                kept.append((weight, reading))
        found[word] = kept
    return found


def block(word, analyses):
    if not analyses:
        return f"{word}\t{word}+?\tinf\n\n"
    return "".join(f"{word}\t{r}\t{w:.6f}\n" for w, r in analyses) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("corpus")
    parser.add_argument("--hfst", required=True)
    parser.add_argument("--analyser", required=True)
    parser.add_argument("--tokens", type=int, default=3000)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    with open(args.corpus, encoding="utf-8") as f:
        kept = tokens(f.read(), args.tokens)
    distinct = sorted(set(kept))
    done = subprocess.run(
        [args.hfst, "lookup", "-q", args.analyser],
        input="".join(word + "\n" for word in distinct),
        capture_output=True,
        text=True,
        check=True,
    )
    found = readings(done.stdout)
    with open(args.out, "w", encoding="utf-8") as out:
        for word in kept:
            out.write(block(word, found.get(word, [])))


if __name__ == "__main__":
    main()
