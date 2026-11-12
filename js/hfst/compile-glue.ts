// The generated wasm-bindgen glue of the compile module, as the wrapper uses
// it. The generated module hands most results back untyped; these types give
// them the shapes crates/hfst-wasm-compile builds, and tsc checks at build
// time that the generated module satisfies them when compile-binding.ts hands
// it to Core.

import type {
  AlphabetComparison,
  Alphabets,
  ExtractedPath,
  GuessReport,
  Summary,
} from "./compile-transducer-base.ts";
import type { PmatchResult } from "./compile-tokeniser.ts";
import type { Diagnostic } from "./errors.ts";
import type { GlueObject, ModuleGlue, TokeniserGlue } from "./glue.ts";
import type {
  GrepReport,
  LookupReport,
  OptimizedLookupReport,
  PairTestResult,
} from "./reports.ts";

/** The options object of a call, which the module checks field by field. */
export type OptionsGlue = object | undefined;

/**
 * The files of a compile as the module takes them: none, name and content
 * pairs, or the caller's function.
 */
export type FilesGlue =
  | undefined
  | [string, string | Uint8Array][]
  | ((name: string) => string | Uint8Array | undefined);

/** A transducer of any type, from `crates/hfst-wasm-compile/src/transducer.rs`. */
export interface CompileTransducerGlue extends GlueObject {
  /** The name of its type, one the wrapper lists. */
  format(): string;
  name(): string;
  /** Its other properties, flattened to key, value, key, value. */
  propertyPairs(): string[];
  copy(): CompileTransducerGlue;
  convert(format: string, quick: boolean): void;
  /** The results for `input` within the limits, as a JSON array of `LookupResult` objects. */
  lookup(input: string, limit: number | undefined, timeLimitMs: number | undefined): string;

  determinize(options: OptionsGlue): void;
  minimize(options: OptionsGlue): void;
  invert(): void;
  reverse(): void;
  removeEpsilons(): void;
  pruneAlphabet(options: OptionsGlue): void;
  project(options: OptionsGlue): void;
  pushWeights(options: OptionsGlue): void;
  pushLabels(options: OptionsGlue): void;
  realign(): void;
  repeat(options: OptionsGlue): void;
  insertFreely(options: OptionsGlue): void;
  eliminateFlags(options: OptionsGlue): void;
  preprocessForOptimizedLookupFormat(): void;
  affixGuessify(options: OptionsGlue): void;
  expandEquivalences(options: OptionsGlue): void;
  killPaths(options: OptionsGlue): void;
  reweight(options: OptionsGlue): void;
  setName(name: string, options: OptionsGlue): void;
  editMetadata(options: OptionsGlue): void;
  /** The model form generator, or undefined with `doNotCompileGenerator`. */
  guessify(options: OptionsGlue): CompileTransducerGlue | undefined;
  substitute(options: OptionsGlue): void;
  substituteTransducer(to: CompileTransducerGlue, options: OptionsGlue): void;
  compose(other: CompileTransducerGlue, options: OptionsGlue): void;
  concatenate(other: CompileTransducerGlue, options: OptionsGlue): void;
  conjunct(other: CompileTransducerGlue, options: OptionsGlue): void;
  subtract(other: CompileTransducerGlue, options: OptionsGlue): void;
  disjunct(other: CompileTransducerGlue, options: OptionsGlue): void;
  priorityDisjunct(other: CompileTransducerGlue, options: OptionsGlue): void;
  shuffle(other: CompileTransducerGlue, options: OptionsGlue): void;
  composeIntersect(rules: TransducerListGlue, options: OptionsGlue): void;

  summarize(options: OptionsGlue): Summary;
  checkAlpha(other: CompileTransducerGlue): AlphabetComparison;
  compare(other: CompileTransducerGlue, options: OptionsGlue): boolean;
  dumpAlphabets(): Alphabets;
  extractPaths(options: OptionsGlue): ExtractedPath[];
  guess(words: string[], options: OptionsGlue): GuessReport[];
  guessWith(words: string[], generator: CompileTransducerGlue, options: OptionsGlue): GuessReport[];
  naiveTokeniser(): CompileTokeniserGlue;

  toBytes(format: string | undefined, options: OptionsGlue): Uint8Array;
  toAtt(options: OptionsGlue): string;
  toProlog(options: OptionsGlue): string;
  toDot(options: OptionsGlue): string;
  toPckimmo(options: OptionsGlue): string;
  toDhfstAcceptor(options: OptionsGlue): Uint8Array;
  toDhfstErrorModel(options: OptionsGlue): Uint8Array;
}

/** A tokeniser of the compile module, which also runs `hfst pmatch`. */
export interface CompileTokeniserGlue extends TokeniserGlue {
  pmatch(text: string, options: OptionsGlue): PmatchResult[];
}

/** The transducers of a call that takes several, from `crates/hfst-wasm-compile/src/list.rs`. */
export interface TransducerListGlue extends GlueObject {
  push(transducer: CompileTransducerGlue): void;
}

/** What a compiler that makes one transducer hands back. */
export interface CompiledGlue {
  transducer: CompileTransducerGlue;
  warnings: Diagnostic[];
}

/** The exports of `crates/hfst-wasm-compile`. */
export interface CompileGlue extends ModuleGlue {
  TransducerList: new () => TransducerListGlue;
  /** Every transducer in a stream of any type, in stream order. */
  readTransducers(bytes: Uint8Array): CompileTransducerGlue[];
  readTokeniser(bytes: Uint8Array): CompileTokeniserGlue;

  regex(source: string, name: string, files: FilesGlue, options: OptionsGlue): CompiledGlue;
  compileLexc(sources: string[], names: string[], files: FilesGlue, options: OptionsGlue): CompiledGlue;
  compileTwolc(
    source: string,
    name: string,
    files: FilesGlue,
    options: OptionsGlue,
  ): { transducers: CompileTransducerGlue[]; warnings: Diagnostic[] };
  compileXfst(
    source: string,
    input: string,
    name: string,
    files: FilesGlue,
    options: OptionsGlue,
  ): {
    output: string;
    stack: CompileTransducerGlue[];
    definitions: Record<string, CompileTransducerGlue>;
    written: Record<string, Uint8Array>;
    warnings: Diagnostic[];
  };
  compilePmatch(
    source: string,
    name: string,
    files: FilesGlue,
    options: OptionsGlue,
  ): { archive: Uint8Array; warnings: Diagnostic[] };
  /** The transducer of text that names no file: `strings` as `compileStrings` takes them, or `att` or `prolog` text. */
  compileText(
    kind: "strings" | "att" | "prolog",
    source: string,
    name: string,
    files: FilesGlue,
    options: OptionsGlue,
  ): CompiledGlue;

  lookupReport(transducers: TransducerListGlue, inputs: string[], options: OptionsGlue): LookupReport[];
  flookupReport(transducers: TransducerListGlue, inputs: string[], options: OptionsGlue): LookupReport[];
  optimizedLookupReport(
    transducers: TransducerListGlue,
    words: string[],
    options: OptionsGlue,
  ): OptimizedLookupReport[];
  pairTest(rules: TransducerListGlue, tests: string, options: OptionsGlue): PairTestResult[];
  grep(pattern: string, text: string, options: OptionsGlue): GrepReport;
  packBhfst(
    acceptor: Uint8Array,
    errorModel: Uint8Array,
    metadata: string | Uint8Array | undefined,
    options: OptionsGlue,
  ): Uint8Array;
}
