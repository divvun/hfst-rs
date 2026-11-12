import type { CompileGlue, CompileTokeniserGlue } from "./compile-glue.ts";
import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import { checkText, Tokeniser as ReadTokeniser } from "./tokenise.ts";

// [spec:hfst:req:javascript.compile-module]
/**
 * What `pmatch` reports with: each option left out has the value
 * `hfst pmatch` gives it, and those of the container's own settings keep
 * the container's.
 */
export interface PmatchOptions {
  /** Takes each line, not each run of lines up to an empty one, as one piece, as `--newline`. */
  newline?: boolean;
  /** Reports each piece's matches rather than the piece marked up, as `--locate`. */
  locate?: boolean;
  extractPatterns?: boolean;
  deletePatterns?: boolean;
  markPatterns?: boolean;
  maxContext?: number;
  maxRecursion?: number;
  /** Milliseconds a piece may take, as `--time-cutoff` does in seconds; 0, the default, sets no limit. */
  timeCutoff?: number;
  /** Abandons any match heavier than this. */
  weightCutoff?: number;
}

/** A match `hfst pmatch --locate` reports. */
export interface PmatchLocation {
  /** Where it starts in the piece, as the library counts. */
  start: number;
  length: number;
  output: string;
  tag: string;
  weight: number;
}

/** What `hfst pmatch` reports for a piece of its input. */
export interface PmatchResult {
  /** The piece: a line, or a run of lines up to an empty one. */
  input: string;
  /** The piece with its matches marked up, unless `locate`. */
  marked: string | null;
  /** Its matches, with `locate`. */
  located: PmatchLocation[] | null;
}

/** Wraps a tokeniser the glue hands back, as `naiveTokeniser` does. */
export let wrapTokeniser: (core: Core<CompileGlue>, glue: CompileTokeniserGlue) => Tokeniser;

// [spec:hfst:req:javascript.compile-module]
/**
 * A tokeniser read from a pmatch container, or made of a transducer by
 * `naiveTokeniser`. It tokenises as `@divvun/hfst`'s does, and reports what
 * `hfst pmatch` reports. It owns module memory: call `free()` when done with
 * it, or declare it with `using`.
 */
export class Tokeniser extends ReadTokeniser<CompileTokeniserGlue> {
  private constructor(core: Core<CompileGlue>, glue: CompileTokeniserGlue) {
    super(core, glue);
  }

  static {
    wrapTokeniser = (core, glue) => new Tokeniser(core, glue);
  }

  /**
   * What `hfst pmatch` reports for `text` with the tokeniser's container:
   * for each piece of it, each run of lines up to an empty one or with
   * `newline` each line, the piece with its matches marked or with `locate`
   * its matches. Each call starts from the container's own settings.
   */
  pmatch(text: string, options?: PmatchOptions): PmatchResult[] {
    checkText(text, "pmatch");
    if (options !== undefined && (typeof options !== "object" || options === null)) {
      throw new HfstError("invalid-argument", "pmatch takes its options as an object");
    }
    return this.use((glue) => glue.pmatch(text, options));
  }
}

export interface TokeniserReader {
  /**
   * The tokeniser in a pmatch container's bytes, as `hfst pmatch2fst`
   * writes them and `compilePmatch` returns them. Bytes that are not a
   * pmatch container throw `invalid-format`.
   */
  readTokeniser(bytes: Uint8Array): Tokeniser;
}

/** The {@link TokeniserReader} that reads into the module instance `core`. */
export function tokeniserReader(core: Core<CompileGlue>): TokeniserReader {
  return {
    readTokeniser: (bytes) => {
      if (!(bytes instanceof Uint8Array)) {
        throw new HfstError("invalid-argument", "a tokeniser reads from the container's bytes, as a Uint8Array");
      }
      return core.call((glue) => wrapTokeniser(core, glue.readTokeniser(bytes)));
    },
  };
}
