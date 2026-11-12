import type { CompileGlue } from "./compile-glue.ts";
import type { Transducer } from "./compile-transducer.ts";
import { strings, withList } from "./compile-transducer-base.ts";
import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { LookupResult } from "./lookup.ts";

/**
 * How a lookup reads each line: as text, as symbols separated by spaces, or
 * in Apertium stream format.
 */
export type LookupInputFormat = "text" | "spaced" | "apertium";

/** What `lookupReport` and `flookupReport` share, each the tool's option of that name. */
export interface ReportOptions {
  /** Default `text`. */
  inputFormat?: LookupInputFormat;
  /** What the input spells the epsilon as. */
  epsilonFormat?: string;
  /** Epsilon cycles are followed this many times. Default 5. */
  cycles?: number;
  /** Only results within this much of the lightest. */
  beam?: number;
  /** Milliseconds of search a word, as `--time-cutoff` does in seconds; 0, the default, sets no limit. */
  timeCutoff?: number;
  /** Default `true`. */
  obeyFlags?: boolean;
  showFlags?: boolean;
}

export interface LookupReportOptions extends ReportOptions {
  /** At most this many results a word, in optimized lookup. */
  maxNumber?: number;
  /** How the results of several transducers combine. Default `union`. */
  cascade?: "union" | "priority-union" | "composition";
}

export interface FlookupReportOptions extends ReportOptions {
  /** Looks up in each transducer as it is, not inverted, as `--invert`. */
  invert?: boolean;
  /** Looks up in optimized lookup, which the tool refuses unless told to. */
  forceOl?: boolean;
}

export interface OptimizedLookupReportOptions {
  /** Each analysis once. */
  unique?: boolean;
  /** At most this many analyses a word. */
  analyses?: number;
  /** Milliseconds of search a word; 0, the default, sets no limit. */
  timeCutoff?: number;
  /** Only analyses within this much of the lightest. */
  beam?: number;
  /** Every analysis as found, without limit or ordering. */
  fast?: boolean;
}

export interface PairTestOptions {
  negativeTest?: boolean;
  /** The tests are the text of a twolc source holding test cases. */
  xeroxMode?: boolean;
}

/** `hfst grep`'s options, which `grep` takes. */
export interface GrepOptions {
  /** The pattern is a string, not a Xerox regular expression. */
  fixedStrings?: boolean;
  /** Marks matches with colour codes rather than `{{{ }}}`. */
  color?: boolean;
  /** Matches only between spaces. */
  wordRegexp?: boolean;
  /** Matches only whole lines. */
  lineRegexp?: boolean;
  /** Reports the lines that do not match. */
  invertMatch?: boolean;
  /** At most this many lines; 0, the default, sets no limit. */
  maxCount?: number;
}

export interface PackBhfstOptions {
  /** The metadata is a zhfst index.xml, to convert, as `--index-xml`. */
  indexXml?: boolean;
  /** The text of the speller runtime configuration `--speller-config` adds to meta.json. */
  spellerConfig?: string | Uint8Array;
}

/** What `hfst lookup` or `hfst flookup` reports for one line. */
export interface LookupReport {
  /** The line as it was looked up; an Apertium line loses its escapes and markup. */
  input: string;
  /** An Apertium line's bracketed markup, or `""`. */
  markup: string;
  /**
   * The results the tool prints, in its order: none for a word with no
   * analysis, and `null` for a line with a symbol no transducer knows, which
   * was not looked up.
   */
  results: LookupResult[] | null;
  /** Whether the lookup was infinitely ambiguous, and so cut short. */
  infinite: boolean;
}

/** What `hfst optimized-lookup` reports for a word. */
export interface OptimizedLookupReport {
  input: string;
  /**
   * The analyses it prints, each its output and weight, in its order;
   * `null` for a word with a symbol no transducer knows.
   */
  analyses: [string, number][] | null;
}

/** Where a rule stopped accepting a pair test. */
export interface RuleFailure {
  rule: string;
  /** How many of the test's pairs the rule accepted before it stopped. */
  recognizedPrefix: number;
}

/** `hfst pair-test`'s report on one test. */
export interface PairTestResult {
  /** The test as the tool shows it: the pair string, or `INPUT : OUTPUT`. */
  test: string;
  positive: boolean;
  /** The test's pairs, input then output, word boundaries included. */
  pairs: [string, string][];
  /** A positive test some rule rejected, or a negative test none did. */
  failed: boolean;
  /** For a positive test, each rule that rejected it. */
  failures: RuleFailure[];
}

export interface GrepLine {
  number: number;
  /** The line, its matches marked. */
  text: string;
}

/** What `hfst grep` reports on a text. */
export interface GrepReport {
  lines: GrepLine[];
  /** Whether the text counts as matching: some line matched, or with `invertMatch`, none did. */
  matched: boolean;
}

export interface Reports {
  /**
   * What `hfst lookup` reports for each of `inputs` with the stream
   * `transducers`.
   */
  lookupReport(
    transducers: readonly Transducer[],
    inputs: readonly string[],
    options?: LookupReportOptions,
  ): LookupReport[];
  /**
   * What `hfst flookup` reports for each of `inputs` with the stream
   * `transducers`. Like the tool, it looks up in each transducer inverted
   * unless `invert`, and then refuses optimized lookup unless `forceOl`.
   */
  flookupReport(
    transducers: readonly Transducer[],
    inputs: readonly string[],
    options?: FlookupReportOptions,
  ): LookupReport[];
  /**
   * What `hfst optimized-lookup` reports for each of `words` with the union
   * of the stream `transducers`.
   */
  optimizedLookupReport(
    transducers: readonly Transducer[],
    words: readonly string[],
    options?: OptimizedLookupReportOptions,
  ): OptimizedLookupReport[];
  /**
   * What `hfst pair-test` reports with the twolc rule transducers `rules` on
   * `tests`: each line a pair string. Blank and `!` comment lines are not
   * tests.
   */
  pairTest(rules: readonly Transducer[], tests: string, options?: PairTestOptions): PairTestResult[];
  /**
   * What `hfst grep` reports for `pattern`, a Xerox regular expression or
   * with `fixedStrings` a string, on `text`.
   */
  grep(pattern: string, text: string, options?: GrepOptions): GrepReport;
  /**
   * The BHFST archive `hfst bhfst` packs of a speller: `acceptor` and
   * `errorModel` each the bytes of a DHFST file, packed as they are, or of a
   * transducer stream, converted to THFST; and `metadata`, the text of the
   * archive's meta.json, or with `indexXml` of a zhfst index.xml to convert.
   */
  packBhfst(
    acceptor: Uint8Array,
    errorModel: Uint8Array,
    metadata?: string | Uint8Array,
    options?: PackBhfstOptions,
  ): Uint8Array;
}

function invalid(message: string): HfstError {
  return new HfstError("invalid-argument", message);
}

function text(call: string, what: string, value: unknown): string {
  if (typeof value !== "string") throw invalid(`${call} takes ${what} as a string`);
  return value;
}

function bytes(call: string, what: string, value: unknown): Uint8Array {
  if (!(value instanceof Uint8Array)) throw invalid(`${call} takes ${what} as a Uint8Array`);
  return value;
}

function given<T extends object>(call: string, options: T | undefined): T | undefined {
  if (options !== undefined && (typeof options !== "object" || options === null || Array.isArray(options))) {
    throw invalid(`${call} takes its options as an object`);
  }
  return options;
}

// [spec:hfst:req:javascript.compile-module]
/** The {@link Reports} over the compile module instance `core`. */
export function reports(core: Core<CompileGlue>): Reports {
  return {
    lookupReport: (transducers, inputs, options) => {
      const lines = strings("lookupReport", inputs);
      const checked = given("lookupReport", options);
      return withList(core, "lookupReport", transducers, (list) =>
        core.call((glue) => glue.lookupReport(list, lines, checked)),
      );
    },
    flookupReport: (transducers, inputs, options) => {
      const lines = strings("flookupReport", inputs);
      const checked = given("flookupReport", options);
      return withList(core, "flookupReport", transducers, (list) =>
        core.call((glue) => glue.flookupReport(list, lines, checked)),
      );
    },
    optimizedLookupReport: (transducers, words, options) => {
      const read = strings("optimizedLookupReport", words);
      const checked = given("optimizedLookupReport", options);
      return withList(core, "optimizedLookupReport", transducers, (list) =>
        core.call((glue) => glue.optimizedLookupReport(list, read, checked)),
      );
    },
    pairTest: (rules, tests, options) => {
      const read = text("pairTest", "the tests", tests);
      const checked = given("pairTest", options);
      return withList(core, "pairTest", rules, (list) => core.call((glue) => glue.pairTest(list, read, checked)));
    },
    grep: (pattern, input, options) =>
      core.call((glue) =>
        glue.grep(text("grep", "the pattern", pattern), text("grep", "the text", input), given("grep", options)),
      ),
    packBhfst: (acceptor, errorModel, metadata, options) =>
      core.call((glue) => {
        if (metadata !== undefined && typeof metadata !== "string" && !(metadata instanceof Uint8Array)) {
          throw invalid("packBhfst takes the metadata as a string or a Uint8Array");
        }
        return glue.packBhfst(
          bytes("packBhfst", "the acceptor", acceptor),
          bytes("packBhfst", "the error model", errorModel),
          metadata,
          given("packBhfst", options),
        );
      }),
  };
}
