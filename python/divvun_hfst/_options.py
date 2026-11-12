"""What changes the tokens and text a tokeniser returns, and what a compiler
makes of its source, as immutable values."""

from __future__ import annotations

from dataclasses import dataclass, fields
from typing import TYPE_CHECKING, Literal

from divvun_hfst._divvun_hfst import (
    TransducerType,
    _check_compile_options,
    _check_format_options,
    _check_tokenise_options,
)

if TYPE_CHECKING:
    from _typeshed import DataclassInstance


def _given(options: DataclassInstance) -> str:
    """The fields that differ from their defaults, as a call that makes them
    would name them."""
    return ", ".join(
        f"{f.name}={getattr(options, f.name)!r}"
        for f in fields(options)
        if getattr(options, f.name) != f.default
    )


# [spec:hfst:req:python.tokenise]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class TokeniseOptions:
    """What changes the tokens `Tokeniser.tokenise` returns. Each option has
    the value `hfst tokenise` gives it unless set.

    - `dedupe`: drop each reading that repeats another, in analysis, pattern
      and weight, as `hfst tokenise` does unless given `--duplicates`.
    - `max_weight_classes`: keep the readings of only this many of a token's
      best weights, as `--weight-classes`. Readings of equal weight make one
      class. An unknown reading counts toward none, and is kept unless it
      comes after the first reading the limit drops. A whole number of at
      least 1; None keeps every class.
    - `beam`: keep only the readings whose weight is within this much of the
      token's best, as `--beam`. At least 0; None keeps every reading.
    - `weight_cutoff`: abandon any match heavier than this, so it gives
      neither a token nor a reading. `hfst tokenise` has no such setting.
    - `time_limit`: stop looking for further matches after this many seconds
      and keep what was found, as `--time-cutoff`. None or 0 sets no limit.
    - `tokenize_multichar`: read multicharacter symbols in the text, as
      `--tokenize-multichar`. Otherwise the text is read one grapheme at a
      time, unless the tokeniser's patterns read multicharacter symbols.
    - `print_all`: include each stretch of text where no pattern matched, as
      `--print-all`.

    The fields are checked when the options are made, so one TokeniseOptions
    serves any number of calls, and `dataclasses.replace` makes a variant.
    """

    dedupe: bool = True
    max_weight_classes: int | None = None
    beam: float | None = None
    weight_cutoff: float | None = None
    time_limit: float | None = None
    tokenize_multichar: bool = False
    print_all: bool = False

    def __post_init__(self) -> None:
        _check_tokenise_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"TokeniseOptions({_given(self)})"


# [spec:hfst:req:python.tokenise]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class FormatOptions:
    """What changes the text `Tokeniser.format` returns: every option
    TokeniseOptions has but `weight_cutoff`, which `hfst tokenise` has no
    setting for, and the options below. An option left as None has the value
    `hfst tokenise` gives it with the format's flag; an option set overrides
    what that flag implies, as a flag after it would. `giella-cg` implies
    `print_weights`, `print_all`, `hack_uncompose` and a `max_weight_classes`
    of 2; `visl` implies `print_all` and no weights.

    - `print_weights`: whether the output carries weights. True stands for
      `--print-weights` and False for `--no-weights`.
    - `hack_uncompose`: print a reading's intermediate form in `giella-cg` and
      `visl` output, when the tokeniser was compiled to carry one.
    - `newline`: take each line, not each paragraph, as one input, as
      `--newline`.
    - `keep_newline`: keep the newline that ends each input, as
      `--keep-newline`, which also takes each line as one input.
    - `superblanks`: pass text inside unescaped square brackets through
      unmatched, as `--superblanks`.

    `hfst tokenise` takes each paragraph of its input (each line, under
    `newline`) as one input, while `giella-cg`, `visl` and `superblanks` read
    the text line by line, and `format` does the same.
    """

    dedupe: bool | None = None
    max_weight_classes: int | None = None
    beam: float | None = None
    time_limit: float | None = None
    tokenize_multichar: bool | None = None
    print_all: bool | None = None
    print_weights: bool | None = None
    hack_uncompose: bool | None = None
    newline: bool | None = None
    keep_newline: bool | None = None
    superblanks: bool | None = None

    def __post_init__(self) -> None:
        _check_format_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"FormatOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class RegexOptions:
    """What `regex` makes of its source, as the `hfst regexp2fst` option of
    the same name makes it.

    - `format`: the type to compile to.
    - `semicolon`: each expression ends with `;`, rather than a line.
    - `disjunct`: one transducer, the union of the expressions.
    - `do_not_harmonize`: do not expand `?` by the other operand's symbols.
    - `harmonize_flags`: harmonize flag diacritics.
    - `do_not_minimize`: determinize each result rather than minimizing it.
    - `encode_weights`: minimize with the weights as part of the labels.
    - `xerox_composition`: flag diacritics are ordinary symbols in
      composition.
    - `flag_is_epsilon`: a one-sided flag diacritic is an epsilon in
      composition, as `--xfst flag-is-epsilon`.
    - `replace_pass`: compile each expression, one parallel optional replace
      rule, as one rewrite pass.
    - `verbose`: say what the compile does, at the info level of the `hfst`
      logger.
    """

    format: TransducerType = "openfst-tropical"
    semicolon: bool = False
    disjunct: bool = False
    do_not_harmonize: bool = False
    harmonize_flags: bool = False
    do_not_minimize: bool = False
    encode_weights: bool = False
    xerox_composition: bool = False
    flag_is_epsilon: bool = False
    replace_pass: bool = False
    verbose: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"RegexOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class LexcOptions:
    """What `compile_lexc` makes of its sources, as the `hfst lexc` option of
    the same name makes it, `alignStrings`, `withFlags`, `minimizeFlags` and
    `renameFlags` spelt in snake_case.

    - `format`: the type to compile to.
    - `align_strings`: align the characters of a string pair one to one.
    - `with_flags`: join the lexicons with flag diacritics rather than
      composing them; `minimize_flags` then uses as few flags as it can, and
      `rename_flags` renames them.
    - `xerox_composition`: compose as Xerox tools do, as the tool does unless
      told `--xerox-composition=no`.
    - `flag_is_epsilon`: a one-sided flag diacritic is an epsilon in
      composition.
    - `encode_weights`: minimize with the weights as part of the labels.
    - `split_characters`: do not read the source's text as Unicode
      characters.
    - `werror`: a warning fails the compile, as `-Werror`.
    - `warn_one_sided_flags`, `warn_unused_lexicons`,
      `warn_repeated_lexicons`, `warn_missing_lexicons`,
      `warn_missing_alphabets` and `warn_unnecessary_escapes`: the `-W`
      switches of those names.
    - `quiet`: no warnings or notes; `verbose`: progress as well.
    """

    format: TransducerType = "openfst-tropical"
    align_strings: bool = False
    with_flags: bool = False
    minimize_flags: bool = False
    rename_flags: bool = False
    xerox_composition: bool = True
    flag_is_epsilon: bool = False
    encode_weights: bool = False
    split_characters: bool = False
    werror: bool = False
    warn_one_sided_flags: bool = False
    warn_unused_lexicons: bool = False
    warn_repeated_lexicons: bool = False
    warn_missing_lexicons: bool = False
    warn_missing_alphabets: bool = False
    warn_unnecessary_escapes: bool = False
    quiet: bool = False
    verbose: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"LexcOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class TwolcOptions:
    """What `compile_twolc` makes of its grammar, as the `hfst twolc` option
    of the same name makes it.

    - `format`: the type to compile to, tropical OpenFst or foma.
    - `resolve`: resolve left-arrow conflicts.
    - `dont_resolve_right`: do not resolve right-arrow conflicts.
    - `quiet`: no warnings; `verbose`: say what the compile does.
    """

    format: Literal["openfst-tropical", "foma"] = "openfst-tropical"
    resolve: bool = False
    dont_resolve_right: bool = False
    quiet: bool = False
    verbose: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"TwolcOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class XfstOptions:
    """How `compile_xfst` runs its script, as the `hfst xfst` option of the
    same name runs it.

    - `format`: the type the script works in, tropical OpenFst or foma.
    - `print_weight`: print weights where words are printed.
    - `restricted_mode`: read and write only in the working directory, and
      run no shell command.
    - `silent`: print no command output.
    """

    format: Literal["openfst-tropical", "foma"] = "openfst-tropical"
    print_weight: bool = False
    restricted_mode: bool = False
    silent: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"XfstOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class PmatchOptions:
    """What `compile_pmatch` makes of its script, as the `hfst pmatch2fst`
    option of the same name makes it.

    - `flatten`: compile every RTN into the network that uses it.
    - `cosine_distances`: include the cosine distances `Like()` reads.
    - `verbose`: say what the compile does.
    """

    flatten: bool = False
    cosine_distances: bool = False
    verbose: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"PmatchOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class StringsOptions:
    """What `compile_strings` makes of its lines, as the `hfst strings2fst`
    option of the same name makes it.

    - `format`: the type to compile to.
    - `disjunct_strings`: one transducer, the union of the lines.
    - `norm`: divide each weight by the sum of all of them, with
      `disjunct_strings`.
    - `log`, `log10`: take the negative natural or base 10 logarithm of each
      weight.
    - `pairstrings`: the lines are pair strings, `a:b c`, rather than string
      pairs, `ac:bc`.
    - `has_spaces`: symbols are separated by spaces.
    - `epsilon`: what the lines spell the epsilon as.
    - `multichar_symbols`: the strings read as one symbol each, the lines of
      the tool's `--multichar-symbols` file.
    - `warn_negative_weights`: warn of each negative weight, as
      `-Wnegative-weights`; `werror`: a warning fails the compile.
    """

    format: TransducerType = "openfst-tropical"
    disjunct_strings: bool = False
    norm: bool = False
    log: bool = False
    log10: bool = False
    pairstrings: bool = False
    has_spaces: bool = False
    epsilon: str = "@0@"
    multichar_symbols: tuple[str, ...] = ()
    warn_negative_weights: bool = True
    werror: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"StringsOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class AttOptions:
    """What `parse_att` makes of its text, as the `hfst txt2fst` option of
    the same name makes it.

    - `format`: the type to read into.
    - `epsilon`: what the text spells the epsilon as.
    - `disjunct`: one transducer, the union of the text's.
    - `check_negative_epsilon_cycles`: warn of epsilon cycles with a negative
      weight.
    - `warn_negative_weights`: warn of each negative weight, as
      `-Wnegative-weights`.
    """

    format: TransducerType = "openfst-tropical"
    epsilon: str = "@0@"
    disjunct: bool = False
    check_negative_epsilon_cycles: bool = False
    warn_negative_weights: bool = True

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"AttOptions({_given(self)})"


# [spec:hfst:req:python.compile]
@dataclass(frozen=True, kw_only=True, slots=True, repr=False)
class PrologOptions:
    """What `parse_prolog` makes of its text, as `hfst txt2fst --prolog`
    makes it with the option of the same name.

    - `format`: the type to read into.
    - `check_negative_epsilon_cycles`: warn of epsilon cycles with a negative
      weight.
    """

    format: TransducerType = "openfst-tropical"
    check_negative_epsilon_cycles: bool = False

    def __post_init__(self) -> None:
        _check_compile_options(self)

    def __repr__(self) -> str:
        """Names only the options set, as a call that makes them would."""
        return f"PrologOptions({_given(self)})"

