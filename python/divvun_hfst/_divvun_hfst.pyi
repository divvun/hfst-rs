# [spec:hfst:req:python.typed]
"""The types of the extension module crates/hfst-python builds, which the
divvun_hfst package re-exports. Every public name is declared here, and
nothing uses Any.
"""

from collections.abc import Mapping, Sequence
from os import PathLike
from typing import ClassVar, Final, Literal, TypeAlias, final

from divvun_hfst._options import (
    AttOptions,
    FormatOptions,
    LexcOptions,
    PmatchOptions,
    PrologOptions,
    RegexOptions,
    StringsOptions,
    TokeniseOptions,
    TwolcOptions,
    XfstOptions,
)
from divvun_hfst._results import (
    AlphabetComparison,
    Alphabets,
    Diagnostic,
    ExtractedPath,
    GrepReport,
    GuessReport,
    LookupReport,
    OptimizedLookupReport,
    PairTestResult,
    PmatchResult,
    Summary,
    XfstResult,
)

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

TransducerType: TypeAlias = Literal[
    "optimized-lookup-weighted", "optimized-lookup-unweighted", "openfst-tropical", "foma"
]

ReweightFunction: TypeAlias = Literal[
    "cos",
    "sin",
    "tan",
    "acos",
    "asin",
    "atan",
    "cosh",
    "sinh",
    "tanh",
    "exp",
    "log",
    "log10",
    "sqrt",
    "floor",
    "ceil",
]
"""The functions `Transducer.reweight` applies to a weight, as `hfst reweight
--function` names them."""

LookupInputFormat: TypeAlias = Literal["text", "spaced", "apertium"]
"""How a lookup reads each line: as text, as symbols separated by spaces, or
in Apertium stream format."""

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

class CompileError(HfstError):
    """A compile that failed. `diagnostics` holds every diagnostic it made,
    as Diagnostic objects, and `code` names the failure as HfstError's
    does."""

    diagnostics: tuple[Diagnostic, ...]

class CompileWarning(UserWarning):
    """A warning of a compile that succeeded, issued through warnings.warn.
    `diagnostic` is the Diagnostic it stands for."""

    diagnostic: Diagnostic

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
    """A transducer of any type the hfst library reads: tropical OpenFst,
    foma, and optimized lookup, weighted and unweighted. Its operations change
    it in place and return None; copy() gives one of its own to change.
    Threads may share one: an operation holds it for its length, and a call on
    a transducer another thread is changing waits until that change is done.
    Every call releases the GIL while the library works. Make one with
    read_transducers, read_transducer, their load_ forms or a compiler."""

    @property
    def type(self) -> TransducerType:
        """The transducer's type."""
    @property
    def name(self) -> str:
        """The `name` property of its HFST3 header, or `""` when there is
        none."""
    @property
    def properties(self) -> dict[str, str]:
        """The properties of its HFST3 header, such as a pmatch container's
        settings, as a new dict. `name` and `type` have their own attributes,
        and the header's `version` belongs to the stream rather than the
        transducer, so none of the three is here."""
    def lookup(
        self, input: str, /, *, limit: int | None = None, time_limit: float | None = None
    ) -> list[LookupResult]:
        """Every result the hfst library's lookup finds for `input`, lightest
        first, then by output in code point order, then by symbols. In
        optimized lookup, flag diacritics are obeyed, so a path they block is
        not a result; `limit` stops the search once it has found that many
        paths, so they are the first it found, not the lightest; `time_limit`
        stops it after that many seconds, and the lookup returns what it
        found, 0 setting no limit. In the other types the results are what
        `hfst lookup` finds, in its order, at most `limit` of them.
        OptimisedTransducer.lookup gives divvunspell's results instead."""
    def copy(self, /) -> Transducer:
        """A transducer of its own with the same contents, which changes
        independently of this one."""
    def convert(self, type: TransducerType, /, *, quick: bool = False) -> None:
        """Converts the transducer to `type`, in place, as `hfst fst2fst`
        converts it. `quick` packs optimized-lookup tables less tightly, as
        `--quick`."""

    # Operations: each changes the transducer, as the hfst tool it is named
    # after changes the transducer it reads, and returns None.

    def determinize(self, /, *, encode_weights: bool = False) -> None:
        """Determinizes the transducer, as `hfst determinize`.
        `encode_weights` treats each weight as part of its arc's label, as
        `--encode-weights`."""
    def minimize(self, /, *, encode_weights: bool = False) -> None:
        """Minimizes the transducer, as `hfst minimize`."""
    def invert(self, /) -> None:
        """Swaps the input and output sides, as `hfst invert`."""
    def reverse(self, /) -> None:
        """Reverses every path, as `hfst reverse`."""
    def remove_epsilons(self, /) -> None:
        """Removes the epsilon arcs, as `hfst remove-epsilons`."""
    def prune_alphabet(self, /, *, force: bool = False) -> None:
        """Drops the symbols no arc uses from the alphabet, as
        `hfst prune-alphabet`. `force` prunes even when an arc uses the
        unknown or identity symbol, as `--force`."""
    def project(self, /, *, project: Literal["input", "output"] = "output") -> None:
        """Makes the transducer the identity on one side's language, as
        `hfst project`."""
    def push_weights(self, /, *, push: Literal["initial", "final"] = "final") -> None:
        """Pushes the weights towards the initial state or the final states,
        as `hfst push-weights`."""
    def push_labels(self, /, *, push: Literal["initial", "final"] = "final") -> None:
        """Pushes the labels towards the initial state or the final states,
        as `hfst push-labels`."""
    def realign(self, /) -> None:
        """Realigns the symbols of each path, as `hfst realign`."""
    def repeat(self, /, *, from_: float = 0.0, to: float | None = None) -> None:
        """Makes the transducer repeat at least `from_` and at most `to`
        times, as `hfst repeat --from --to`. Each is a whole number or
        `math.inf`, and `to` None is `math.inf`."""
    def insert_freely(self, /, *, symbol_pair: str, harmonise: bool = False) -> None:
        """Inserts `symbol_pair`, `IN:OUT` with `@0@` for the epsilon, freely
        anywhere along each path, as `hfst insert-freely`. `harmonise`
        expands the identity and unknown symbols by the pair's symbols."""
    def eliminate_flags(self, /, *, flag: str | None = None) -> None:
        """Compiles the flag diacritics of the feature `flag`, or of every
        feature, into the transducer's paths, as `hfst eliminate-flags`."""
    def preprocess_for_optimized_lookup_format(self, /) -> None:
        """Prepares the transducer for conversion to optimized lookup, as
        `hfst preprocess-for-optimized-lookup-format`."""
    def affix_guessify(
        self, /, *, direction: Literal["prefix", "suffix"] = "suffix", weight: float = 1.0
    ) -> None:
        """Makes the transducer a guesser of the analyses of unknown words by
        their affixes, as `hfst affix-guessify`: by their suffixes or their
        prefixes, each guessed character costing `weight`."""
    def expand_equivalences(
        self,
        /,
        *,
        from_: str | None = None,
        to: str | None = None,
        tsv: str | None = None,
        level: Literal["first", "second", "both"] = "first",
    ) -> None:
        """Lets each symbol stand for the symbols it is equivalent to, as
        `hfst expand-equivalences`: `from_` for `to`, or each line of `tsv`,
        `FROM<TAB>TO<TAB>TO...`, on the `level` side."""
    def kill_paths(self, /, *, symbol: str | None = None, tsv: str | None = None) -> None:
        """Removes every arc carrying `symbol`, or any symbol `tsv` lists one
        a line, as `hfst kill-paths`."""
    def reweight(
        self,
        /,
        *,
        addition: float = 0.0,
        multiplier: float = 1.0,
        function: ReweightFunction | None = None,
        lower_bound: float = 0.0,
        upper_bound: float | None = None,
        input_symbol: str | None = None,
        output_symbol: str | None = None,
        symbol: str | None = None,
        end_states_only: bool = False,
        arcs_only: bool = False,
        tsv: str | None = None,
    ) -> None:
        """Changes the weights, as `hfst reweight`: each weight in bounds
        becomes `function(weight * multiplier + addition)`, on the arcs and
        final states `symbol`, `input_symbol` and `output_symbol` select, or
        by the rules of `tsv`, `SYMBOL<TAB>WEIGHT` a line. `upper_bound` None
        sets no upper bound."""
    def set_name(self, name: str, /, *, truncate_length: int = 0) -> None:
        """Sets the transducer's name, as `hfst name --name`, cut to
        `truncate_length` bytes unless that is 0. The tool is `name`, which
        is the attribute that reads the name."""
    def edit_metadata(self, /, *, add: Mapping[str, str], truncate_length: int = 0) -> None:
        """Adds or replaces each property of `add`, as `hfst edit-metadata
        --add NAME=VALUE`, each value cut to `truncate_length` bytes unless
        that is 0."""
    def guessify(
        self, /, *, default_penalty: float = 1.0, do_not_compile_generator: bool = False
    ) -> Transducer | None:
        """Makes the transducer, a morphological analyser, a guesser of the
        analyses of unknown words, as `hfst guessify`: weighted optimized
        lookup that `guess` takes. Returns the model form generator the tool
        writes after the guesser, or None with `do_not_compile_generator`."""
    def substitute(
        self,
        /,
        *,
        from_label: str | None = None,
        to_label: str | None = None,
        to_transducer: Transducer | None = None,
        from_file: str | None = None,
        file_name: str = "",
        in_order: bool = False,
        do_not_convert: bool = False,
        compose: bool = False,
    ) -> None:
        """Replaces labels, as `hfst substitute`: `from_label` by `to_label`
        or by `to_transducer`, or each `FROM<TAB>TO` line of `from_file`, the
        text of a relabel file, in turn with `in_order`. A label is a symbol
        or a colon pair, `@0@` for the epsilon. `file_name` is the name of
        the file the tool would read `to_transducer` or `from_file` from,
        which names the result. `compose` composes symbol-for-symbol
        substitutions in rather than rewriting arcs."""
    def compose(
        self,
        other: Transducer,
        /,
        *,
        do_not_harmonize: bool = False,
        harmonize_flags: bool = False,
        xerox_composition: bool = False,
        flag_is_epsilon: bool = False,
        do_not_convert: bool = False,
    ) -> None:
        """Composes `other` after the transducer, as `hfst compose`. Flag
        diacritics are harmonized unless `do_not_harmonize`; with
        `xerox_composition` they are ordinary symbols, and with
        `flag_is_epsilon` a one-sided one is an epsilon. Transducers of two
        algebra types are converted to one as the tool converts them, unless
        `do_not_convert`. `t.compose(t)` composes the transducer with itself
        as it was."""
    def concatenate(
        self,
        other: Transducer,
        /,
        *,
        do_not_harmonize: bool = False,
        harmonize_flags: bool = False,
        do_not_convert: bool = False,
    ) -> None:
        """Concatenates `other` after the transducer, as `hfst concatenate`."""
    def conjunct(
        self,
        other: Transducer,
        /,
        *,
        do_not_harmonize: bool = False,
        harmonize_flags: bool = False,
        do_not_convert: bool = False,
    ) -> None:
        """Intersects the transducer with `other`, as `hfst conjunct`."""
    def subtract(
        self,
        other: Transducer,
        /,
        *,
        do_not_harmonize: bool = False,
        harmonize_flags: bool = False,
        do_not_convert: bool = False,
    ) -> None:
        """Subtracts `other` from the transducer, as `hfst subtract`."""
    def disjunct(
        self,
        other: Transducer,
        /,
        *,
        do_not_harmonize: bool = False,
        do_not_convert: bool = False,
    ) -> None:
        """Unites the transducer with `other`, as `hfst disjunct`."""
    def priority_disjunct(self, other: Transducer, /, *, do_not_convert: bool = False) -> None:
        """Unites the transducer with the paths of `other` whose input it does
        not read, as `hfst priority-disjunct`."""
    def shuffle(self, other: Transducer, /, *, do_not_convert: bool = False) -> None:
        """Shuffles the transducer with `other`, as `hfst shuffle`."""
    def compose_intersect(
        self,
        rules: Sequence[Transducer],
        /,
        *,
        invert: bool = False,
        fast: bool = False,
        encode_weights: bool = False,
        harmonize: bool = False,
        lexicon_name: str = "",
        rules_name: str = "",
        do_not_convert: bool = False,
    ) -> None:
        """Composes the transducer, a lexicon, with the intersection of
        `rules`, as `hfst compose-intersect`. The result is named after the
        lexicon, or `lexicon_name` when it has no name, and `rules_name`, as
        the tool names it after its files."""

    # Queries: each reports what the hfst tool it is named after reports.

    def summarize(self, /, *, verbose: bool = False) -> Summary:
        """What `hfst summarize` reports on the transducer, and with
        `verbose` what `--verbose` measures besides."""
    def check_alpha(self, other: Transducer, /) -> AlphabetComparison:
        """What `hfst check-alpha` reports on the transducer and `other`."""
    def compare(
        self,
        other: Transducer,
        /,
        *,
        do_not_harmonize: bool = False,
        eliminate_flags: bool = False,
    ) -> bool:
        """Whether the transducer and `other` are equivalent, as `hfst
        compare` decides. Like the tool, it compares two transducers of one
        algebra type; flag diacritics are harmonized unless
        `do_not_harmonize`, and eliminated first with `eliminate_flags`."""
    def dump_alphabets(self, /) -> Alphabets:
        """What `hfst dump-alphabets` reports on the transducer."""
    def extract_paths(
        self,
        /,
        *,
        max_strings: int | None = None,
        nbest: int | None = None,
        random: int | None = None,
        cycles: int | None = None,
        beam: float | None = None,
        obey_flags: bool = False,
        print_flags: bool = False,
        max_in_length: int | None = None,
        max_out_length: int | None = None,
        in_prefix: str = "",
        out_prefix: str = "",
        in_exclude: str = "",
        out_exclude: str = "",
    ) -> list[ExtractedPath]:
        """The paths `hfst fst2strings` prints, in its order. Each limit left
        as None sets none: at most `max_strings` paths, the `nbest` lightest,
        `random` paths chosen at random, cycles followed at most `cycles`
        times, and only paths within `beam` of the lightest, whose input is
        at most `max_in_length` bytes and output `max_out_length`. Each path
        must start with `in_prefix` and `out_prefix` and must not contain
        `in_exclude` or `out_exclude`, unless they are empty. `obey_flags`
        drops the paths flag diacritics forbid, and `print_flags` keeps the
        flags in the paths."""
    def guess(
        self,
        words: Sequence[str],
        /,
        *,
        generator: Transducer | None = None,
        model_forms: str | None = None,
        max_number_of_guesses: int = 5,
        max_number_of_forms: int = 2,
        generate_threshold: float = 50.0,
    ) -> list[GuessReport]:
        """What `hfst guess` prints for each of `words` with the transducer,
        a guesser `guessify` made. With `model_forms`, the text of a model
        form file, it generates model forms for each guess with `generator`,
        or with one compiled from the guesser."""
    def naive_tokeniser(self, /) -> Tokeniser:
        """The tokeniser `hfst tokenize` makes of a transducer that is not a
        pmatch container: each word of the transducer's input side a token,
        analysed as its outputs. Multicharacter symbols are matched longest
        first when the transducer has any, or when the options of a call set
        `tokenize_multichar`."""

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
    def pmatch(
        self,
        text: str,
        /,
        *,
        newline: bool = False,
        locate: bool = False,
        extract_patterns: bool | None = None,
        delete_patterns: bool | None = None,
        mark_patterns: bool | None = None,
        max_context: int | None = None,
        max_recursion: int | None = None,
        time_cutoff: float = 0.0,
        weight_cutoff: float | None = None,
    ) -> list[PmatchResult]:
        """What `hfst pmatch` reports for `text` with the tokeniser's
        container: for each piece of it, each run of lines up to an empty one
        or with `newline` each line, the piece with its matches marked or
        with `locate` its matches. The options left as None keep the
        container's own settings."""

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
    order, of any type the hfst library reads: tropical OpenFst, foma, and
    optimized lookup, weighted and unweighted, with an HFST3 header or
    without one. A pmatch container reads as TOP followed by its definitions,
    each under its name. Bytes that are not a transducer stream raise
    FormatError, its code saying why."""

def read_transducer(data: bytes | bytearray) -> Transducer:
    """The one transducer in the bytes of a stream, as read_transducers reads
    it. A stream holding more raises FormatError with code
    `transducer-count`."""

def load_transducers(path: str | PathLike[str]) -> list[Transducer]:
    """Every transducer at `path`, as read_transducers reads a file's bytes; a
    THFST directory, as `hfst fst2fst -f thfst` writes one, reads as its one
    transducer, in weighted optimized lookup. A file that cannot be read
    raises the OSError open() raises."""

def load_transducer(path: str | PathLike[str]) -> Transducer:
    """The one transducer at `path`, as load_transducers reads it. A stream
    holding more raises FormatError with code `transducer-count`."""

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

def _check_compile_options(
    options: RegexOptions
    | LexcOptions
    | TwolcOptions
    | XfstOptions
    | PmatchOptions
    | StringsOptions
    | AttOptions
    | PrologOptions,
) -> None:
    """Checks one of the compile option dataclasses as it is made, raising
    what a compiler reading its fields would raise."""

def _check_tokenise_options(options: TokeniseOptions) -> None:
    """Checks a TokeniseOptions as it is made, raising what a call reading its
    fields would raise."""

def _check_format_options(options: FormatOptions) -> None:
    """Checks a FormatOptions as it is made, raising what a call reading its
    fields would raise."""

def lookup_report(
    transducers: Sequence[Transducer],
    inputs: Sequence[str],
    /,
    *,
    input_format: LookupInputFormat = "text",
    epsilon_format: str = "",
    max_number: int | None = None,
    cycles: int = 5,
    beam: float | None = None,
    time_cutoff: float = 0.0,
    cascade: Literal["union", "priority-union", "composition"] = "union",
    obey_flags: bool = True,
    show_flags: bool = False,
) -> list[LookupReport]:
    """What `hfst lookup` reports for each of `inputs` with the stream
    `transducers`. `input_format` reads each line as `"text"`, as symbols
    `"spaced"` apart, or as `"apertium"` stream format. `cascade` combines the
    results of several transducers: `"union"`, `"priority-union"` or
    `"composition"`. Each other option is the tool's: `max_number` results a
    word in optimized lookup, epsilon cycles followed `cycles` times, results
    within `beam` of the lightest, `time_cutoff` seconds a word, flags obeyed
    unless not `obey_flags` and shown with `show_flags`."""

def flookup_report(
    transducers: Sequence[Transducer],
    inputs: Sequence[str],
    /,
    *,
    input_format: LookupInputFormat = "text",
    epsilon_format: str = "",
    cycles: int = 5,
    beam: float | None = None,
    time_cutoff: float = 0.0,
    invert: bool = False,
    force_ol: bool = False,
    obey_flags: bool = True,
    show_flags: bool = False,
) -> list[LookupReport]:
    """What `hfst flookup` reports for each of `inputs` with the stream
    `transducers`. Like the tool, it looks up in each transducer inverted
    unless `invert`, and then refuses optimized lookup unless `force_ol`.
    The other options are lookup_report's."""

def optimized_lookup_report(
    transducers: Sequence[Transducer],
    words: Sequence[str],
    /,
    *,
    unique: bool = False,
    analyses: int | None = None,
    time_cutoff: float = 0.0,
    beam: float | None = None,
    fast: bool = False,
) -> list[OptimizedLookupReport]:
    """What `hfst optimized-lookup` reports for each of `words` with the union
    of the stream `transducers`: each analysis once with `unique`, at most
    `analyses` of them, `time_cutoff` seconds a word, only those within `beam`
    of the lightest, and with `fast` every analysis as found."""

def pair_test(
    rules: Sequence[Transducer],
    tests: str,
    /,
    *,
    negative_test: bool = False,
    xerox_mode: bool = False,
) -> list[PairTestResult]:
    """What `hfst pair-test` reports with the twolc rule transducers `rules`
    on `tests`: each line a pair string, a negative test with
    `negative_test`, or with `xerox_mode` the text of a twolc source holding
    test cases. Blank and `!` comment lines are not tests."""

def grep(
    pattern: str,
    text: str,
    /,
    *,
    fixed_strings: bool = False,
    color: bool = False,
    word_regexp: bool = False,
    line_regexp: bool = False,
    invert_match: bool = False,
    max_count: int = 0,
) -> GrepReport:
    """What `hfst grep` reports for `pattern`, a Xerox regular expression or
    with `fixed_strings` a string, on `text`: the lines that match, or with
    `invert_match` those that do not, their matches marked with `{{{ }}}` or
    with `color` colour codes, at most `max_count` of them unless it is 0.
    `word_regexp` matches only between spaces, `line_regexp` whole lines."""

def regex(
    source: str,
    /,
    options: RegexOptions | None = None,
    *,
    name: str = "<stdin>",
    files: Mapping[str, str | bytes] | None = None,
) -> Transducer:
    """The transducer `hfst regexp2fst` writes of the regular expression
    `source`. With RegexOptions(disjunct=True) a source of several
    expressions, one a line or with `semicolon` each ending in `;`, compiles
    to their union; without it, a source of more than one raises CompileError
    with code `transducer-count`. `name` is the file name the tool would read
    the source from, which diagnostics give it. Each file the expression
    reads comes from `files` when it is given, and from the filesystem
    otherwise. A source that does not compile raises CompileError, and each
    warning of one that does is issued as a CompileWarning."""

def compile_lexc(
    sources: Sequence[str],
    /,
    options: LexcOptions | None = None,
    *,
    names: Sequence[str] | None = None,
    files: Mapping[str, str | bytes] | None = None,
) -> Transducer:
    """The transducer `hfst lexc` writes of the lexc `sources`, compiled
    together and named after the first. `names` are the file names the tool
    would read them from, which diagnostics and the result's name give them;
    each is `<stdin>` unless given. Each file their regular expressions read
    comes from `files` when it is given, and from the filesystem otherwise."""

def compile_twolc(
    source: str,
    /,
    options: TwolcOptions | None = None,
    *,
    name: str = "<twolc>",
    files: Mapping[str, str | bytes] | None = None,
) -> list[Transducer]:
    """The rule transducers `hfst twolc` writes of the twolc grammar
    `source`, in tropical OpenFst or foma. `name` is the file name the tool
    would read it from, `<twolc>` for its standard input. A twolc grammar
    names no file, so `files` has none to supply."""

def compile_xfst(
    source: str,
    /,
    options: XfstOptions | None = None,
    *,
    input: str = "",
    name: str = "<stdin>",
    files: Mapping[str, str | bytes] | None = None,
) -> XfstResult:
    """Runs the xfst script `source` as `hfst xfst -F` runs it, in tropical
    OpenFst or foma, with `input` as the standard input its commands read:
    what it printed, its stack, its defined networks and the files it wrote.
    Each file it reads comes from `files` when it is given, and each it
    writes lands there; without `files`, it reads and writes the filesystem.
    Either way `XfstResult.written` holds each file it wrote. A command that
    fails is a warning, and the script raises CompileError only when it
    stops on one."""

def compile_pmatch(
    source: str,
    /,
    options: PmatchOptions | None = None,
    *,
    name: str = "<stdin>",
    files: Mapping[str, str | bytes] | None = None,
) -> bytes:
    """The archive `hfst pmatch2fst` writes of the pmatch script `source`,
    which read_tokeniser reads. `name` is the file name the tool would read
    it from: a relative `@bin"FILE"` or `@txt"FILE"` is found beside it, as
    the tool finds one beside its input. Each file the script reads comes
    from `files` when it is given, and from the filesystem otherwise."""

def compile_strings(
    source: str,
    /,
    options: StringsOptions | None = None,
    *,
    name: str = "<stdin>",
    files: Mapping[str, str | bytes] | None = None,
) -> Transducer:
    """The transducer `hfst strings2fst` writes of the strings `source`, one
    a line, each with an optional tab and weight. With
    StringsOptions(disjunct_strings=True) the lines compile to their union;
    without it, a source of more than one line raises CompileError with code
    `transducer-count`. Strings name no file, so `files` has none to
    supply."""

def parse_att(
    source: str,
    /,
    options: AttOptions | None = None,
    *,
    name: str = "<stdin>",
    files: Mapping[str, str | bytes] | None = None,
) -> Transducer:
    """The transducer `hfst txt2fst` writes of the AT&T text `source`, named
    after `name`. With AttOptions(disjunct=True) a text of several
    transducers, separated by `--` lines, reads as their union; without it, a
    text of more than one raises CompileError with code `transducer-count`.
    AT&T text names no file, so `files` has none to supply."""

def parse_prolog(
    source: str,
    /,
    options: PrologOptions | None = None,
    *,
    name: str = "<stdin>",
    files: Mapping[str, str | bytes] | None = None,
) -> Transducer:
    """The transducer `hfst txt2fst --prolog` writes of the Prolog text
    `source`, named after `name`. A text of more than one network raises
    CompileError with code `transducer-count`. Prolog text names no file, so
    `files` has none to supply."""

