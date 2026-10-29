import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { TokeniserGlue } from "./glue.ts";
import { Handle } from "./handle.ts";

// [spec:hfst:req:javascript.tokenise]
/** One reading of a token. */
export interface Reading {
  /** The tokeniser's output for the token, such as `cat+N`. */
  analysis: string;
  /** The reading's weight. Lower is better. */
  weight: number;
}

// [spec:hfst:req:javascript.tokenise]
/**
 * A token the tokeniser found or, under `printAll`, a stretch of text
 * between tokens where no pattern matched.
 */
export interface Token {
  /** The token's text, as the tokeniser read it. */
  surface: string;
  /**
   * Where the token starts in the text, in UTF-16 code units, as string
   * indices count. `text.slice(start, end)` is the surface, unless the text
   * holds something the tokeniser reads without printing, such as the
   * spelling of a flag diacritic.
   */
  start: number;
  /** Where the token ends in the text, in UTF-16 code units, exclusive. */
  end: number;
  /**
   * Whether any reading is an analysis. A reading the tokeniser marks as
   * unknown, by an empty output or one containing ` ??`, is not one.
   */
  analysed: boolean;
  /**
   * The readings `hfst tokenise --cg --print-weights` prints for the token,
   * in that order. A stretch where no pattern matched has none.
   */
  readings: Reading[];
}

// [spec:hfst:req:javascript.tokenise]
/**
 * What changes the tokens and readings `tokenise` returns. Each option
 * left out has the value `hfst tokenise` gives it.
 */
export interface TokeniseOptions {
  /**
   * Drops each reading that repeats another, as `hfst tokenise` does unless
   * given `--duplicates`. Readings repeat when their analysis and pattern are
   * the same, and their weight too, except in `format` output that prints no
   * weights. Default `true`.
   */
  dedupe?: boolean;
  /**
   * Keeps the readings of only this many of a token's best weights, as
   * `--weight-classes`. Readings of equal weight make one class. An unknown
   * reading counts toward none, and is kept unless it comes after the first
   * reading the limit drops. A whole number of at least 1. Default: every
   * class.
   */
  maxWeightClasses?: number;
  /**
   * Keeps only the readings whose weight is within this much of the token's
   * best, as `--beam`. At least 0. Default: every reading.
   */
  beam?: number;
  /**
   * Abandons any match heavier than this, so it gives neither a token nor a
   * reading. `hfst tokenise` has no such setting. Default: none.
   */
  weightCutoff?: number;
  /**
   * Stops looking for further matches once this many milliseconds have
   * passed and keeps what was found, as `--time-cutoff` does in seconds.
   * Default 0, which sets no limit.
   */
  timeLimit?: number;
  /**
   * Reads multicharacter symbols in the text, as `--tokenize-multichar`.
   * Without it the text is read one grapheme at a time, unless the
   * tokeniser's patterns read multicharacter symbols. Default `false`.
   */
  tokenizeMultichar?: boolean;
  /**
   * Includes each stretch of text where no pattern matched, as
   * `--print-all`. Default `false`.
   */
  printAll?: boolean;
}

// [spec:hfst:req:javascript.tokenise]
/**
 * What changes the text `format` returns: every option {@link TokeniseOptions}
 * has but `weightCutoff`, which `hfst tokenise` has no setting for, and the
 * options below. An option left out has the value `hfst tokenise` gives it
 * with the format's flag; an option given overrides what that flag implies,
 * as a flag after it would. `giella-cg` implies `printWeights`, `printAll`,
 * `hackUncompose` and a `maxWeightClasses` of 2; `visl` implies `printAll`
 * and no weights.
 *
 * `hfst tokenise` takes each paragraph of its input (each line, under
 * `newline`) as one input, while `giella-cg`, `visl` and `superblanks` read
 * the text line by line, and `format` does the same.
 */
export interface FormatOptions extends Omit<TokeniseOptions, "weightCutoff"> {
  /**
   * Whether the output carries weights. `true` stands for `--print-weights`
   * and `false` for `--no-weights`, which turns off the weights `giella-cg`
   * implies.
   */
  printWeights?: boolean;
  /**
   * Prints a reading's intermediate form in `giella-cg` and `visl` output,
   * when the tokeniser was compiled to carry one.
   */
  hackUncompose?: boolean;
  /** Takes each line, not each paragraph, as one input, as `--newline`. */
  newline?: boolean;
  /**
   * Keeps the newline that ends each input, as `--keep-newline`, which also
   * takes each line as one input.
   */
  keepNewline?: boolean;
  /**
   * Passes text inside unescaped square brackets through unmatched, as
   * `--superblanks`.
   */
  superblanks?: boolean;
}

// [spec:hfst:req:javascript.tokenise]
/**
 * The formats `format` prints, under the names of the `hfst tokenise`
 * options that choose them. They do not change between releases.
 *
 * - `segment`: one token per line (`--segment`, the default).
 * - `space-separated`: tokens separated by spaces (`--space-separated`).
 * - `xerox`: each reading with its surface and weight (`--xerox`).
 * - `cg`: Constraint Grammar cohorts (`--cg`).
 * - `finnpos`: FinnPos input (`--finnpos`).
 * - `giella-cg`: the Constraint Grammar form Giella grammars read (`--giella-cg`).
 * - `conllu`: CoNLL-U (`--conllu`).
 * - `visl`: VISL CG-3 (`--visl`).
 */
export const TOKENISE_FORMATS = [
  "segment",
  "space-separated",
  "xerox",
  "cg",
  "finnpos",
  "giella-cg",
  "conllu",
  "visl",
] as const;

export type TokeniseFormat = (typeof TOKENISE_FORMATS)[number];

function checkText(text: unknown, what: string): asserts text is string {
  if (typeof text !== "string") {
    throw new HfstError("invalid-argument", `${what} takes the text as a string`);
  }
}

// [spec:hfst:req:javascript.tokenise]
/**
 * A tokeniser loaded from a pmatch container, as `hfst tokenise` loads one.
 * It has the same members, with the same behaviour, in both packages. It
 * owns module memory: call `free()` when done with it, or declare it with
 * `using`.
 */
export abstract class TokeniserBase extends Handle<TokeniserGlue> {
  protected constructor(core: Core, glue: TokeniserGlue) {
    super(core, glue);
  }

  /**
   * The tokens the tokeniser finds in `text`, in text order, matching the
   * whole text as one input. `hfst tokenise` takes each paragraph as one
   * input, so for text with no blank line these are the cohorts
   * `hfst tokenise --cg --print-weights` prints, given the same options.
   * Option values the command line would turn down throw `invalid-argument`.
   */
  tokenise(text: string, options: TokeniseOptions = {}): Token[] {
    checkText(text, "tokenise");
    return this.use((glue) => glue.tokenise(text, options));
  }

  /**
   * Exactly what `hfst tokenise` prints in `format` when `text` is its
   * standard input, given the flags `options` stand for.
   */
  format(text: string, format: TokeniseFormat, options: FormatOptions = {}): string {
    checkText(text, "format");
    if (!(TOKENISE_FORMATS as readonly unknown[]).includes(format)) {
      throw new HfstError(
        "invalid-argument",
        `there is no tokenise format called ${String(format)}; the formats are ${TOKENISE_FORMATS.join(", ")}`,
      );
    }
    return this.use((glue) => glue.format(text, format, options));
  }
}

/** The function that loads a package's tokeniser. */
export interface TokeniserReader<T> {
  /**
   * The tokeniser in a pmatch container's bytes, as `hfst pmatch2fst`
   * writes them. Bytes are copied into the module once and not kept. Bytes
   * that are not a pmatch container throw `invalid-format`, among them a
   * transducer that is not one: `hfst tokenise` wraps such a transducer in a
   * tokeniser of its own, which this API does not make.
   */
  readTokeniser(bytes: Uint8Array): T;
}

/** {@link TokeniserReader} over a package's reader, which `read` calls. */
export function tokeniserReader<T>(read: (bytes: Uint8Array) => T): TokeniserReader<T> {
  return {
    readTokeniser: (bytes) => {
      if (!(bytes instanceof Uint8Array)) {
        throw new HfstError("invalid-argument", "a tokeniser loads from the container's bytes, as a Uint8Array");
      }
      return read(bytes);
    },
  };
}
