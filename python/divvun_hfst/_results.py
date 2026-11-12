"""What the queries on transducers report, as immutable values.

The extension module makes each of these from what the hfst library reports,
so they hold plain values: none is a handle that needs another call to read.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from divvun_hfst._divvun_hfst import LookupResult, Transducer, TransducerType


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class SummaryDetails:
    """What `hfst summarize --verbose` measures besides the summary."""

    infinitely_ambiguous: bool
    first_input_symbols: tuple[str, ...] | None
    """The symbols a path can start with; None for optimized lookup, which has
    no graph to walk."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class Summary:
    """What `hfst summarize` reports on a transducer."""

    name: str
    type: TransducerType
    states: int
    final_states: int
    arcs: int
    io_epsilons: int
    """Arcs with an epsilon on both sides."""
    input_epsilons: int
    output_epsilons: int
    densest_arcs: int
    """The most arcs a state has."""
    sparsest_arcs: int
    """The fewest arcs a state has."""
    unique_input_arcs: int
    unique_output_arcs: int
    most_ambiguous_input: tuple[str, int]
    """The input symbol with the most arcs out of one state, and how many."""
    most_ambiguous_output: tuple[str, int]
    found_alphabet: tuple[str, ...]
    """The symbols the arcs use."""
    symbol_pairs: tuple[tuple[str, str, int], ...]
    """Each pair of symbols an arc carries, input then output, and how many
    arcs carry it."""
    acceptor: bool
    input_deterministic: bool
    output_deterministic: bool
    cyclic: bool
    cyclic_at_initial_state: bool
    alphabet: tuple[str, ...]
    """The alphabet the transducer declares."""
    details: SummaryDetails | None
    """What `verbose` measures besides; None without it."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class AlphabetComparison:
    """`hfst check-alpha`'s report on two transducers."""

    first_used: tuple[str, ...]
    """The symbols the first transducer's arcs use."""
    second_used: tuple[str, ...]
    first_declared: tuple[str, ...]
    """The alphabet the first transducer declares."""
    second_declared: tuple[str, ...]
    mismatch: bool
    """Whether either has a symbol, used or declared, the other lacks."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class Alphabets:
    """`hfst dump-alphabets`' report on a transducer."""

    declared: tuple[str, ...]
    seen: tuple[str, ...]
    """The symbols its arcs use."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class ExtractedPath:
    """One path `hfst fst2strings` prints."""

    input: str
    """The input side as the tool prints it: no epsilons, and flag diacritics
    only with `print_flags`."""
    output: str
    pairs: tuple[tuple[str, str], ...]
    """The path's symbol pairs, input then output, an epsilon as `""`; a pair
    of flag diacritics only with `print_flags`."""
    weight: float


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class LookupReport:
    """What `hfst lookup` or `hfst flookup` reports for one line."""

    input: str
    """The line as it was looked up; an Apertium line loses its escapes and
    markup."""
    markup: str
    """An Apertium line's bracketed markup, or `""`."""
    results: tuple[LookupResult, ...] | None
    """The results the tool prints, in its order: none for a word with no
    analysis, and None for a line with a symbol no transducer knows, which
    was not looked up."""
    infinite: bool
    """Whether the lookup was infinitely ambiguous, and so cut short."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class OptimizedLookupReport:
    """What `hfst optimized-lookup` reports for a word."""

    input: str
    analyses: tuple[tuple[str, float], ...] | None
    """The analyses it prints, each its output and weight, in its order; None
    for a word with a symbol no transducer knows."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class GuessReport:
    """What `hfst guess` prints for a word."""

    input: str
    paradigms: bool
    """Whether `guesses` are paradigms, as with model forms, rather than
    analyses."""
    guesses: tuple[str, ...]
    """Each guessed analysis or, with model forms, for each guess the word,
    the analysis and the model forms generated for it, separated by tabs."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class RuleFailure:
    """Where a rule stopped accepting a pair test."""

    rule: str
    recognized_prefix: int
    """How many of the test's pairs the rule accepted before it stopped."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class PairTestResult:
    """`hfst pair-test`'s report on one test."""

    test: str
    """The test as the tool shows it: the pair string, or `INPUT : OUTPUT`."""
    positive: bool
    pairs: tuple[tuple[str, str], ...]
    """The test's pairs, input then output, word boundaries included."""
    failed: bool
    """A positive test some rule rejected, or a negative test none did."""
    failures: tuple[RuleFailure, ...]
    """For a positive test, each rule that rejected it."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class GrepLine:
    """A line `hfst grep` prints."""

    number: int
    """Its number in the text, from 1."""
    text: str
    """The line, its matches marked."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class GrepReport:
    """What `hfst grep` reports on a text."""

    lines: tuple[GrepLine, ...]
    matched: bool
    """Whether the text counts as matching: some line matched, or with
    `invert_match`, none did."""


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class PmatchLocation:
    """A match `hfst pmatch --locate` reports."""

    start: int
    """Where it starts in the piece, as the library counts."""
    length: int
    output: str
    tag: str
    weight: float


# [spec:hfst:req:python.operations]
@dataclass(frozen=True, slots=True)
class PmatchResult:
    """What `hfst pmatch` reports for a piece of its input."""

    input: str
    """The piece: a line, or a run of lines up to an empty one."""
    marked: str | None
    """The piece with its matches marked up, unless `locate`."""
    located: tuple[PmatchLocation, ...] | None
    """Its matches, with `locate`."""


# [spec:hfst:req:python.compile-errors]
@dataclass(frozen=True, slots=True)
class DiagnosticLabel:
    """A piece of a compile's source a diagnostic points at."""

    span: tuple[int, int]
    """Where the piece starts and ends in the source, in code points, as `str`
    indices count."""
    message: str


# [spec:hfst:req:python.compile-errors]
@dataclass(frozen=True, slots=True)
class Diagnostic:
    """Something a compile said about its source: an error, a warning or a
    note."""

    severity: Literal["error", "warning", "info"]
    message: str
    source_name: str
    """The name of the source it is about, as the compile was given it."""
    span: tuple[int, int] | None
    """Where its primary label is in the source, in code points, as `str`
    indices count; None when it points at no piece of the source."""
    labels: tuple[DiagnosticLabel, ...]
    """Every label, the primary one first."""
    notes: tuple[str, ...]
    help: str | None
    rendered: str
    """The report the hfst tools print for it, without colour."""


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, slots=True)
class XfstResult:
    """What an xfst script left behind."""

    output: str
    """What it printed: the tool's standard output."""
    stack: tuple[Transducer, ...]
    """Its stack, the bottom first."""
    definitions: dict[str, Transducer]
    """The networks it defined, by name."""
    written: dict[str, bytes]
    """Each file it wrote, by name, in the order it first wrote each, holding
    what the script left in it."""

