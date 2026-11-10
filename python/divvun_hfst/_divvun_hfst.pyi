# [spec:hfst:req:python.typed]
"""The types of the extension module crates/hfst-python builds, which the
divvun_hfst package re-exports. Every public name is declared here, and
nothing uses Any.
"""

from os import PathLike
from typing import ClassVar, Final, Literal, TypeAlias, final

from divvun_hfst._options import FormatOptions, TokeniseOptions

__version__: Final[str]

HfstErrorCode: TypeAlias = Literal[
    "type-mismatch",
    "unsupported-format",
    "invalid-format",
    "io",
    "not-automaton",
    "cyclic",
    "invalid-argument",
    "internal",
    "failed",
    "transducer-count",
]

TransducerType: TypeAlias = Literal["optimized-lookup-weighted", "optimized-lookup-unweighted"]

TokeniseFormat: TypeAlias = Literal[
    "segment", "space-separated", "xerox", "cg", "finnpos", "giella-cg", "conllu", "visl"
]

class HfstError(Exception):
    """A failure the hfst library reports. `code` names it as @divvun/hfst
    does, in a form code can test."""

    code: HfstErrorCode

class FormatError(HfstError):
    """Bytes a reader does not load: a format the package does not read, bytes
    that are not well formed in the format they claim, a stream mixing types,
    or more transducers than the reader takes. `code` says which."""

@final
class LookupResult:
    """One path a lookup found: its output symbols, joined and one by one, and
    its weight. Flag diacritics and epsilons are left out of both, as `hfst
    lookup` leaves them out of what it prints."""

    __hash__: ClassVar[None]  # type: ignore[assignment]
    output: Final[str]
    symbols: Final[tuple[str, ...]]
    weight: Final[float]
    """The sum of the weights along the path. Lower is better."""

@final
class Transducer:
    """An optimized-lookup transducer, for lookup with the hfst library's
    engine, whose results are what `hfst lookup` prints. It is immutable, so
    threads may look up in it at the same time, each lookup a run of its own
    without the GIL. Make one with read_transducers, read_transducer or their
    load_ forms."""

    @property
    def type(self) -> TransducerType:
        """Which optimized-lookup format the transducer is in, weighted or
        unweighted."""
    @property
    def name(self) -> str:
        """The `name` property of its HFST3 header, or `""` when there is
        none."""
    @property
    def properties(self) -> dict[str, str]:
        """The properties of its HFST3 header, such as a pmatch container's
        settings, as a new dict. `name` and `type` have their own attributes,
        and the header's `version` belongs to the stream rather than the
        transducer, so none of the three is here. A transducer with no header
        has none."""
    def lookup(
        self, input: str, /, *, limit: int | None = None, time_limit: float | None = None
    ) -> list[LookupResult]:
        """Every result the hfst library's lookup finds for `input`, lightest
        first, then by output in code point order, then by symbols. Flag
        diacritics are obeyed, so a path they block is not a result. `limit`
        stops the search once it has found that many paths, so they are the
        first it found, not the lightest. `time_limit` stops it after that
        many seconds, and the lookup returns what it found; 0 sets no limit.
        OptimisedTransducer.lookup gives divvunspell's results instead."""

@final
class OptimisedTransducer:
    """An optimized-lookup transducer read for the optimised lookup, which is
    divvunspell's: its results are what divvunspell gives for the same
    transducer and word. Transducer.lookup gives hfst's. The two agree for
    most words; they differ on Unicode normalisation, text outside the
    alphabet, cycles of epsilon and flag arcs, duplicate paths and what
    `limit` means, as @divvun/hfst's MIGRATING.md describes. It is immutable,
    so threads may look up in it at the same time, without the GIL. Make one
    with read_optimised_transducer or load_optimised_transducer."""

    @property
    def type(self) -> TransducerType:
        """Which optimized-lookup format the transducer is in, weighted or
        unweighted."""
    @property
    def name(self) -> str:
        """The `name` property of its HFST3 header, or `""` when there is
        none."""
    @property
    def properties(self) -> dict[str, str]:
        """The properties of its HFST3 header, as Transducer.properties has
        them: without `name`, `type` and `version`. A transducer with no
        header has none."""
    def lookup(self, input: str, /, *, limit: int | None = None) -> list[LookupResult]:
        """What divvunspell's lookup returns for `input`: every output of a
        path that reads it, with the path's weight, final weight included,
        lightest first, then by output in code point order. Each output comes
        once at each weight it has. `limit` keeps the lightest results; every
        result is found first, so there is no time limit."""

@final
class Reading:
    """One reading of a token."""

    __hash__: ClassVar[None]  # type: ignore[assignment]
    analysis: Final[str]
    """The tokeniser's output for the token, such as `cat+N`."""
    weight: Final[float]
    """The reading's weight. Lower is better."""

@final
class Token:
    """A token the tokeniser found or, under `print_all`, a stretch of text
    between tokens where no pattern matched."""

    __hash__: ClassVar[None]  # type: ignore[assignment]
    surface: Final[str]
    """The token's text, as the tokeniser read it."""
    start: Final[int]
    """Where the token starts in the text, in code points, as `str`
    indices count. `text[start:end]` is the surface, unless the text
    holds something the tokeniser reads without printing, such as the
    spelling of a flag diacritic."""
    end: Final[int]
    """Where the token ends in the text, in code points, exclusive."""
    analysed: Final[bool]
    """Whether any reading is an analysis. A reading the tokeniser marks
    as unknown, by an empty output or one containing ` ??`, is not one."""
    readings: Final[tuple[Reading, ...]]
    """The readings `hfst tokenise --cg --print-weights` prints for the
    token, in that order. A stretch where no pattern matched has none."""

@final
class Tokeniser:
    """A tokeniser read from a pmatch container, as `hfst tokenise` reads one.
    It holds the container's tables alone and is immutable, so threads may
    tokenise with it at the same time: each call is a run of its own, without
    the GIL. Make one with read_tokeniser or load_tokeniser."""

    def tokenise(self, text: str, /, options: TokeniseOptions | None = None) -> list[Token]:
        """The tokens the tokeniser finds in `text`, in text order, matching
        the whole text as one input. `hfst tokenise` takes each paragraph as
        one input, so for text with no blank line these are the cohorts
        `hfst tokenise --cg --print-weights` prints, given the same
        options."""
    def format(
        self, text: str, format: TokeniseFormat, /, options: FormatOptions | None = None
    ) -> str:
        """Exactly what `hfst tokenise` prints in `format` when `text` is its
        standard input, given the flags `options` stand for."""

@final
class AcceptorInfo:
    """How a DHFST acceptor stores its arcs, as `dhfst-tools` reports it."""

    __hash__: ClassVar[None]  # type: ignore[assignment]
    ids: Final[int]
    """State numbers: every state is numbered below this."""
    slots: Final[int]
    """Slots in the table, at least `ids` plus `label_max`."""
    used_slots: Final[int]
    """Slots that hold arcs, free or regular."""
    check_bytes: Final[int]
    """Bytes per slot check: 1 when every regular label is below 256,
    else 2."""
    label_max: Final[int]
    """The greatest regular label."""
    record_bytes: Final[int]
    """Bytes per record, each a target and the index of a weight."""
    list_records: Final[int]
    """Records in the list of arcs that do not fit their slot."""
    arcs: Final[int]
    """Arcs, free and regular."""
    free_arcs: Final[int]
    """Free arcs: epsilon and flag diacritic arcs."""
    free_pairs: Final[int]
    """Distinct symbol and weight pairs of the free arcs."""
    weights: Final[int]
    """Distinct regular arc weights."""
    finals: Final[int]
    """Final states."""
    distances: Final[bool]
    """Whether the file stores each state's distance to a final state."""

@final
class DhfstAcceptor:
    """A DHFST acceptor (type 2): a speller's lexicon, read and validated as
    divvunspell reads it. It is immutable, so threads may look words up in
    it at the same time. Make one with read_dhfst or load_dhfst."""

    @property
    def type(self) -> Literal["acceptor"]:
        """Always `"acceptor"`, which tells this from a DhfstErrorModel."""
    @property
    def version(self) -> int:
        """The format version the file declares."""
    @property
    def symbols(self) -> tuple[str, ...]:
        """The symbol names, `@_EPSILON_SYMBOL_@` first."""
    @property
    def meta(self) -> str | None:
        """The JSON the writer left in the file's `meta` section, or None
        when it has none."""
    @property
    def info(self) -> AcceptorInfo:
        """How the file stores its arcs."""
    def lookup(self, word: str, /) -> float | None:
        """The least weight of a path that accepts `word`, final weight
        included, or None when no path does: what divvunspell's speller finds
        checking the word as typed against this lexicon alone. The word is
        read one extended grapheme cluster at a time, each the symbol of the
        same text or else the lexicon's unknown symbol. Epsilon and flag
        diacritic arcs are walked, and the flags obeyed, as divvunspell does.
        There is no case folding or suggestion search: those are
        divvunspell's."""

@final
class DhfstErrorModel:
    """A DHFST error model (type 1), read and validated as divvunspell reads
    one. The package offers no suggestion search, so an error model only
    describes itself, and keeps none of its file's bytes."""

    @property
    def type(self) -> Literal["error-model"]:
        """Always `"error-model"`, which tells this from a DhfstAcceptor."""
    @property
    def version(self) -> int:
        """The format version the file declares."""
    @property
    def state_count(self) -> int:
        """The number of states, one for each state of the source transducer
        that its start state reaches."""
    @property
    def symbols(self) -> tuple[str, ...]:
        """The symbol names, `@_EPSILON_SYMBOL_@` first."""
    @property
    def meta(self) -> str | None:
        """The JSON the writer left in the file's `meta` section, or None
        when it has none."""

def read_transducers(data: bytes | bytearray) -> list[Transducer]:
    """Every transducer in the bytes of an HFST transducer stream, in stream
    order: weighted and unweighted optimized-lookup transducers, with or
    without an HFST3 header. A pmatch container reads as TOP followed by its
    definitions, each under its name. Bytes in any other format raise
    FormatError with code `unsupported-format`, naming the format; bytes that
    are not a transducer stream, or end partway through one, raise
    FormatError with code `invalid-format`."""

def read_transducer(data: bytes | bytearray) -> Transducer:
    """The one transducer in the bytes of a stream, as read_transducers reads
    it. A stream holding more raises FormatError with code
    `transducer-count`."""

def load_transducers(path: str | PathLike[str]) -> list[Transducer]:
    """Every transducer in the file at `path`, as read_transducers reads its
    bytes. A file that cannot be read raises the OSError open() raises."""

def load_transducer(path: str | PathLike[str]) -> Transducer:
    """The one transducer in the file at `path`, as read_transducer reads its
    bytes. A file that cannot be read raises the OSError open() raises."""

def read_optimised_transducer(data: bytes | bytearray) -> OptimisedTransducer:
    """The one optimized-lookup transducer in `data`, weighted or unweighted,
    with or without an HFST3 header, read for the optimised lookup. Bytes in
    any other format raise FormatError with code `unsupported-format`, naming
    it, and bytes that are not a transducer stream, or end partway through
    one, with code `invalid-format`. A stream of several transducers, such as
    a pmatch container, raises FormatError with code `transducer-count`."""

def load_optimised_transducer(path: str | PathLike[str]) -> OptimisedTransducer:
    """The optimized-lookup transducer in the file at `path`, as
    read_optimised_transducer reads its bytes. A file that cannot be read
    raises the OSError open() raises."""

def read_tokeniser(data: bytes | bytearray) -> Tokeniser:
    """The tokeniser in a pmatch container's bytes, as `hfst pmatch2fst`
    writes them. Bytes that are not a pmatch container raise FormatError with
    code `invalid-format`, among them a transducer that is not one: `hfst
    tokenise` wraps such a transducer in a tokeniser of its own, which this
    package does not make."""

def load_tokeniser(path: str | PathLike[str]) -> Tokeniser:
    """The tokeniser in the pmatch container at `path`, as read_tokeniser
    reads its bytes. A file that cannot be read raises the OSError open()
    raises."""

def read_dhfst(data: bytes | bytearray) -> DhfstAcceptor | DhfstErrorModel:
    """The DHFST file in `data`, a DhfstAcceptor or a DhfstErrorModel, as its
    `type` says. A file that fails the validation divvunspell runs on load, or
    is truncated, raises FormatError with code `invalid-format` and the
    reason. Bytes that are not DHFST raise FormatError with code
    `invalid-format`, or `unsupported-format` naming a format the package
    does not read."""

def load_dhfst(path: str | PathLike[str]) -> DhfstAcceptor | DhfstErrorModel:
    """The DHFST file at `path`, as read_dhfst reads its bytes. A file that
    cannot be read raises the OSError open() raises."""

def _check_tokenise_options(options: TokeniseOptions) -> None:
    """Checks a TokeniseOptions as it is made, raising what a call reading its
    fields would raise."""

def _check_format_options(options: FormatOptions) -> None:
    """Checks a FormatOptions as it is made, raising what a call reading its
    fields would raise."""
