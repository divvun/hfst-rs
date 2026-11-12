"""Record what lang-crk's morph-completion speller suggests, for scoring
with hfst predict-eval --recording.

The words are those of an analysed text that the harness types: tokens
with a character the harness counts. For each word and each start of it
a recording has one line: the word, a tab, the start, then each suggestion
after a tab, in divvunspell's order. The empty start is not asked for:
divvunspell skips an empty line, so the speller offers nothing before the
first letter.

divvunspell is run with --continuation-marker "~" and --json: its
plain-text output adds the marker to complete words too. A suggestion
ending in the marker is an incomplete word, and picking it writes its text
without the marker. Any other suggestion is a complete word, and picking it
writes the word and a space. So a complete word helps only when it is the
whole word being typed: when it is not, it is recorded with the space after
it, which no word starts with. The lenient recordings write a complete word
bare, so the harness may also pick it as the start of a longer word.

Each distinct start is asked for once, with -n set to the largest number
shown; the first N of those are taken as what N shown would be. The answers
are kept in a cache file, one JSON object per line, so a run cut short by
--seconds continues where it stopped when run again. divvunspell reads a
batch of starts from its standard input; --jobs processes run at once.

Written: OUT-nN.tsv and OUT-nN-lenient.tsv for each N in --shown. Printed
to standard error, for the first suggestion and for each N: over every
typed token and every start of it from one letter to all but the last, the
share of starts for which a suggestion shown is useful, one the word starts
with that is longer than the start (a complete word only when it is the
word).

Exits with status 3, writing no recording, when --seconds ran out first.

Run with python3 -I. Standard library only.
"""

import argparse
import concurrent.futures
import json
import os
import subprocess
import sys
import time
import unicodedata

MARKER = "~"
BATCH = 50


def characters(word):
    """The characters the harness counts: not spaces, not punctuation."""
    return sum(
        1
        for c in word
        if not c.isspace() and not unicodedata.category(c).startswith("P")
    )


def typed_words(path):
    """Each typed token's word, in text order."""
    words = []
    block = None
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            line = line.rstrip("\n")
            if not line:
                if block is not None and characters(block) > 0:
                    words.append(block)
                block = None
                continue
            block = line.split("\t", 1)[0]
    if block is not None and characters(block) > 0:
        words.append(block)
    return words


def starts(word):
    return [word[:length] for length in range(len(word))]


def suggest(command, batch):
    """divvunspell's suggestions for each start in the batch."""
    done = subprocess.run(
        command,
        input="".join(start + "\n" for start in batch),
        capture_output=True,
        text=True,
        check=True,
    )
    answers = json.loads(done.stdout)["suggest"]
    if [answer["word"] for answer in answers] != batch:
        sys.exit("divvunspell answered other words than it was asked")
    found = {}
    for answer in answers:
        values = [s["value"] for s in answer["suggestions"]]
        for s in answer["suggestions"]:
            if s["value"].endswith(MARKER) == s["completed"]:
                sys.exit(f"marker and completed flag disagree for {answer['word']!r}")
        found[answer["word"]] = values
    return found


def written(value, word, lenient):
    """What picking a suggestion writes, as far as the word goes."""
    if value.endswith(MARKER):
        return value[: -len(MARKER)]
    if lenient or value == word:
        return value
    return value + " "


def read_cache(path):
    found = {}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as lines:
            for line in lines:
                entry = json.loads(line)
                found[entry["start"]] = entry["suggestions"]
    return found


def ask(command, asked, cache, jobs, seconds):
    """Ask for every start not in the cache, adding each answer to it.
    False when the time ran out first."""
    batches = [asked[at : at + BATCH] for at in range(0, len(asked), BATCH)]
    deadline = time.monotonic() + seconds
    complete = True
    with open(cache, "a", encoding="utf-8") as out:
        with concurrent.futures.ThreadPoolExecutor(jobs) as pool:
            def run(batch):
                if time.monotonic() > deadline:
                    return None
                return suggest(command, batch)

            for answered in pool.map(run, batches):
                if answered is None:
                    complete = False
                    continue
                for start, values in answered.items():
                    entry = {"start": start, "suggestions": values}
                    out.write(json.dumps(entry, ensure_ascii=False) + "\n")
                out.flush()
    return complete


def write_recordings(out, words, found, shown):
    for n in shown:
        for lenient in (False, True):
            path = f"{out}-n{n}{'-lenient' if lenient else ''}.tsv"
            with open(path, "w", encoding="utf-8") as recording:
                for word in sorted(set(words)):
                    for start in starts(word):
                        values = found.get(start, [])[:n]
                        line = [word, start] + [written(v, word, lenient) for v in values]
                        recording.write("\t".join(line) + "\n")


def useful_share(words, found, n):
    queries = useful = 0
    for word in words:
        for start in starts(word)[1:]:
            queries += 1
            shown = [written(v, word, False) for v in found[start][:n]]
            useful += any(len(s) > len(start) and word.startswith(s) for s in shown)
    return useful, queries


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("text", help="analysed text, as for hfst predict-eval")
    parser.add_argument("--divvunspell", required=True)
    parser.add_argument("--shown", default="3,10", help="numbers shown, with commas")
    parser.add_argument("--out", required=True, help="start of the recordings' paths")
    parser.add_argument("--cache", required=True, help="the answers file")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--seconds", type=float, default=float("inf"))
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--archive", help="the speller's .zhfst")
    source.add_argument(
        "--lexicon",
        nargs=2,
        metavar=("ACCEPTOR", "ERRMODEL"),
        help="the speller's acceptor and error model, unzipped",
    )
    args = parser.parse_args()
    shown = [int(n) for n in args.shown.split(",")]

    command = [args.divvunspell, "suggest", "-S", "--json", "-n", str(max(shown))]
    command += ["--continuation-marker", MARKER]
    if args.archive:
        command += ["-a", args.archive]
    else:
        command += ["--lexicon-path", args.lexicon[0], "--mutator-path", args.lexicon[1]]

    words = typed_words(args.text)
    found = read_cache(args.cache)
    asked = sorted({s for word in set(words) for s in starts(word) if s} - found.keys())
    print(f"starts to ask: {len(asked)}", file=sys.stderr)
    if not ask(command, asked, args.cache, args.jobs, args.seconds):
        print("time ran out; run again to go on", file=sys.stderr)
        sys.exit(3)
    found = read_cache(args.cache)

    write_recordings(args.out, words, found, shown)
    for n in sorted({1, *shown}):
        useful, queries = useful_share(words, found, n)
        print(
            f"n{n}: token starts with a useful suggestion: {useful} of {queries} "
            f"({100 * useful / max(queries, 1):.1f}%)",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
