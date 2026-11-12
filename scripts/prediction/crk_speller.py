"""Rebuild the acceptor of lang-crk's morph-completion speller with prefix
weights counted from part of an analysed text only.

The speller's acceptor (tools/spellcheckers/acceptor.default.predict.hfscript)
weights each start of a word by how often words in a corpus begin with it.
Its weights, tools/spellcheckers/prefix_weighting.lexc, were counted by
src/derivation/morpheme-completion-weighting.sh from the whole
Ahenakew-Wolfart-Bloomfield corpus. That script reads a file which is not in
lang-crk; this one counts the same way from the text crk_corpus.py writes,
leaving out the fold the harness scores, so that the speller has not seen
the words it is scored on.

The count, as the shell script does it with its prefix type set to
morphemes: each token's first reading is generated with
src/fst/crk-strict-generator-with-morpheme-boundaries-giellaltbuild.hfstol,
each form it writes counting once. A form keeps only its first ">"
boundary; a morpheme of at most two letters after a boundary is joined to
what comes before it, and "$" ends the form. The form is cut at "<", ">"
and "/", and each run of pieces from its start counts once for that start.
A start's weight is minus the natural log of its count over the number of
forms counted. Tokens are skipped as the shell script skips them: those
with "+?", "+PUNCT", "+CLB", "+Quant", "+Prop", "?" or "Iph".

With --build, the weights are then compiled into a speller acceptor as
lang-crk's Makefile.mod-mobile-hfst.am does it, with the same hfscript, in
the directory given: about a minute and 1.5 GB. The result is
acceptor.hfstol, which divvunspell reads with --lexicon-path, beside the
shipped error model given to --mutator-path. Nothing in lang-crk is written.

Run with python3 -I. Standard library only.
"""

import argparse
import collections
import math
import os
import re
import subprocess
import sys

SKIPPED = ["+?", "+PUNCT", "+CLB", "+Quant", "+Prop", "?", "Iph"]
BOUNDARY = re.compile(r"[<>/]")
BOUNDARY_GENERATOR = "src/fst/crk-strict-generator-with-morpheme-boundaries-giellaltbuild"
HFSCRIPT_LOAD = "load ../../src/crk-strict-generator-with-morpheme-boundaries-giellaltbuild.hfst"


def blocks(path):
    """Each harness token's (word, first reading); None for a token with no
    reading."""
    with open(path, encoding="utf-8") as text:
        for block in text.read().split("\n\n"):
            lines = [line for line in block.split("\n") if line]
            if not lines:
                continue
            word, reading, _ = lines[0].split("\t")
            yield None if reading.endswith("+?") else (word, reading)


def training_readings(path, folds, leave_out):
    """The first reading of each token outside the part left out, in the
    harness's split of the text into parts."""
    tokens = list(blocks(path))
    n = len(tokens)
    start, end = 0, 0
    if leave_out is not None:
        start, end = (leave_out - 1) * n // folds, leave_out * n // folds
    return [
        token[1]
        for at, token in enumerate(tokens)
        if token is not None and not start <= at < end
        and not any(s in token[0] or s in token[1] for s in SKIPPED)
    ]


def lookup(hfst, transducer, inputs):
    done = subprocess.run(
        [hfst, "lookup", "-q", transducer],
        input="".join(item + "\n" for item in inputs),
        capture_output=True,
        text=True,
        check=True,
    )
    outputs = collections.defaultdict(list)
    for line in done.stdout.splitlines():
        fields = line.split("\t")
        if len(fields) == 3 and "+?" not in line:
            outputs[fields[0]].append(fields[1])
    return outputs


def merged(form):
    """A generated form with its short suffixes joined, as the shell
    script's second gawk program writes it."""
    form = form.replace(">", "|", 1).replace(">", "").replace("|", ">", 1)
    form = re.sub(r"[</]", lambda m: m.group(0) + "|", form).replace(">", "|>")
    pieces = form.split("|")
    out = pieces[0]
    for piece in pieces[1:]:
        letters = BOUNDARY.sub("", piece)
        out += letters if len(letters) <= 2 else piece
    return out + "$"


def weights(forms):
    counts = collections.Counter()
    for form in forms:
        pieces = BOUNDARY.split(form)
        for length in range(1, len(pieces) + 1):
            counts["".join(pieces[:length])] += 1
    total = len(forms)
    return {start: -math.log(count / total) for start, count in counts.items()}


def lexc(weighted):
    lines = ["LEXICON Root"]
    for start, weight in sorted(weighted.items(), key=lambda item: item[1]):
        lines.append(f'{start} # "weight: {weight:f}" ;')
    return "\n".join(lines) + "\n"


def compare(ours, path):
    """How the weights differ from another prefix_weighting.lexc."""
    theirs = {}
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            match = re.match(r'^(\S+) # "weight: ([0-9.]+)" ;$', line.strip())
            if match:
                theirs[match.group(1)] = float(match.group(2))
    shared = ours.keys() & theirs.keys()
    gap = sum(abs(ours[s] - theirs[s]) for s in shared) / max(len(shared), 1)
    print(
        f"starts: ours {len(ours)}, theirs {len(theirs)}, shared {len(shared)}; "
        f"mean weight gap on shared starts {gap:.3f}",
        file=sys.stderr,
    )
    for start in sorted(theirs, key=theirs.get)[:15]:
        print(f"  {start}\ttheirs {theirs[start]:.3f}\tours {ours.get(start, float('nan')):.3f}", file=sys.stderr)


def build(hfst, lang, out):
    """Compile prefix_weighting.lexc in OUT into OUT/acceptor.hfstol."""
    spellers = os.path.join(lang, "tools/spellcheckers")
    with open(os.path.join(spellers, "acceptor.default.predict.hfscript"), encoding="utf-8") as f:
        script = f.read()
    if HFSCRIPT_LOAD not in script:
        sys.exit("the hfscript no longer loads the generator this script expects")
    script = script.replace(HFSCRIPT_LOAD, f"load {os.path.join(lang, BOUNDARY_GENERATOR)}.hfst")
    with open(os.path.join(out, "acceptor.hfscript"), "w", encoding="utf-8") as f:
        f.write(script)

    def run(*command, **options):
        subprocess.run([hfst, *command], check=True, cwd=out, **options)

    run("lexc", "-q", "-o", "PrefixWeighting.hfst", "prefix_weighting.lexc")
    run("xfst", "-q", "-F", "acceptor.hfscript", stdout=subprocess.DEVNULL)
    joined = subprocess.run(
        [hfst, "disjunct", "-q", "Acceptor.hfst",
         os.path.join(spellers, "easteregg.predict.mobile.hfst")],
        check=True, cwd=out, capture_output=True,
    ).stdout
    pushed = subprocess.run(
        [hfst, "push-weights", "-q", "--push=initial"],
        input=joined, check=True, cwd=out, capture_output=True,
    ).stdout
    run("fst2fst", "-q", "-f", "olw", "-o", "acceptor.hfstol", input=pushed)
    os.remove(os.path.join(out, "Acceptor.hfst"))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("text", help="the text crk_corpus.py wrote")
    parser.add_argument("--hfst", required=True)
    parser.add_argument("--lang-crk", required=True, help="a built lang-crk checkout")
    parser.add_argument("--out", required=True, help="directory to write to")
    parser.add_argument("--folds", type=int, default=10)
    parser.add_argument("--leave-out", type=int, help="the part left out, from 1")
    parser.add_argument("--compare", help="a prefix_weighting.lexc to compare with")
    parser.add_argument("--build", action="store_true", help="build the acceptor")
    args = parser.parse_args()
    args.hfst = os.path.abspath(args.hfst)

    readings = training_readings(args.text, args.folds, args.leave_out)
    generated = lookup(
        args.hfst, os.path.join(args.lang_crk, BOUNDARY_GENERATOR + ".hfstol"), sorted(set(readings))
    )
    forms = [merged(form) for reading in readings for form in generated.get(reading, [])]
    weighted = weights(forms)
    print(f"tokens counted: {len(readings)}, forms: {len(forms)}", file=sys.stderr)

    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "prefix_weighting.lexc"), "w", encoding="utf-8") as f:
        f.write(lexc(weighted))
    if args.compare:
        compare(weighted, args.compare)
    if args.build:
        build(args.hfst, args.lang_crk, args.out)


if __name__ == "__main__":
    main()
