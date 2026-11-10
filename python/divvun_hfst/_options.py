"""What changes the tokens and text a tokeniser returns, as immutable values."""

from __future__ import annotations

from dataclasses import dataclass, fields

from divvun_hfst._divvun_hfst import _check_format_options, _check_tokenise_options


def _given(options: TokeniseOptions | FormatOptions) -> str:
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
