"""Sum hfst predict-eval reports of the same rows over parts of a text.

A recording made with weights that left out one fold is scored on that
fold's text alone, so each fold has a report of its own. This script reads
them all and writes, for each row and each line (all words, and each
number of units), the keystrokes saved over all the folds: one minus the
taps over the characters, the taps being read back from each report as its
characters times one minus its keystrokes saved. A report rounds keystrokes
saved to a tenth of a percent, so a sum is off by at most half a tenth of a
percent.

Usage: python3 -I crk_sum_folds.py REPORT...
"""

import collections
import re
import sys


def read(path):
    """{(row, line): (characters, taps)} for one report."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    names = re.split(r" {2,}", lines[0].strip())
    found = {}
    for line in lines[2:]:
        fields = line.split()
        if fields[:2] == ["no", "reading"]:
            fields = ["no reading"] + fields[2:]
        group, characters = fields[0], int(fields[2])
        cells = fields[3:]
        for at, name in enumerate(names):
            saved = cells[4 * at]
            if saved == "-":
                continue
            rate = float(saved.rstrip("%")) / 100
            found[(name, group)] = (characters, characters * (1 - rate))
    return names, found


def total(paths):
    """The rows' names, the lines in the order first met, and for each row
    and line the characters and taps over all the reports."""
    totals = collections.defaultdict(lambda: [0, 0.0])
    names = []
    groups = []
    for path in paths:
        names, found = read(path)
        for (name, group), (characters, taps) in found.items():
            totals[(name, group)][0] += characters
            totals[(name, group)][1] += taps
            if group not in groups:
                groups.append(group)
    return names, groups, totals


def main():
    names, groups, totals = total(sys.argv[1:])
    print("\t".join(["line", "chars"] + names))
    for group in groups:
        characters = max(totals[(name, group)][0] for name in names)
        cells = []
        for name in names:
            chars, taps = totals[(name, group)]
            cells.append(f"{100 * (1 - taps / chars):.1f}%" if chars else "-")
        print("\t".join([group, str(characters)] + cells))


if __name__ == "__main__":
    main()
