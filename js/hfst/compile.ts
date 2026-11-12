// [spec:hfst:def:javascript.packages+3]
// [spec:hfst:req:javascript.compile-module]
/**
 * `@divvun/hfst/compile`: what hfst-embed's `compile` feature does, in
 * browsers, Web Workers, Deno, Node and Bun. It compiles regular
 * expressions, lexc, twolc, xfst scripts, pmatch scripts, strings and AT&T
 * and Prolog text; it reads transducers of every type the hfst library reads
 * and transforms, inspects and writes them as the hfst tools do, giving the
 * bytes and reports the tools give. It is a WebAssembly module of its own,
 * apart from `@divvun/hfst`'s, which only a caller who compiles loads: start
 * it with this entry point's `init`. A transducer crosses between the two as
 * the bytes `toBytes` writes, which the other's readers read.
 *
 * @module
 */
import { core } from "./compile-binding.ts";
import { tokeniserReader } from "./compile-tokeniser.ts";
import { transducerReaders } from "./compile-transducer.ts";
import { compilers } from "./compilers.ts";
import type { InitSource } from "./core.ts";
import type { Logger, LogLevel } from "./logging.ts";
import { reports } from "./reports.ts";

// [spec:hfst:req:javascript.typed-surface+1]
// This module declares the whole public API of the compile entry point.
// Results are plain values, and js/build.mjs fails a build whose declarations
// use any.

// [spec:hfst:req:javascript.compile-module]
/**
 * Loads and starts the compile entry point's WebAssembly module. Call it once
 * and wait for it before anything else here. With no argument it loads the
 * `.wasm` file that ships beside it, not `@divvun/hfst`'s, which has an
 * `init` of its own. Calling it again returns the same promise.
 */
export function init(source?: InitSource): Promise<void> {
  return core.init(source);
}

/** The package's version, which both entry points report. */
export function version(): string {
  return core.call((glue) => glue.version());
}

/**
 * Sends the log events of compiling to `logger`. `null` puts back the
 * default, which writes each event to the console method for its level. This
 * entry point logs on its own: `@divvun/hfst`'s `setLogger` does not reach it.
 */
export function setLogger(logger: Logger | null): void {
  core.setLogger(logger);
}

/** Logs events at `level` and the more severe levels; `off` logs nothing. The default is `warn`. */
export function setLogLevel(level: LogLevel): void {
  core.setLogLevel(level);
}

export const { readTransducers, readTransducer } = transducerReaders(core);
export const { readTokeniser } = tokeniserReader(core);
export const { regex, compileLexc, compileTwolc, compileXfst, compilePmatch, compileStrings, parseAtt, parseProlog } =
  compilers(core);
export const { lookupReport, flookupReport, optimizedLookupReport, pairTest, grep, packBhfst } = reports(core);
export { Transducer } from "./compile-transducer.ts";
export type {
  AffixGuessifyOptions,
  BinaryOptions,
  ComposeIntersectOptions,
  ComposeOptions,
  ConversionOptions,
  ConvertOptions,
  DeterminizeOptions,
  DisjunctOptions,
  EditMetadataOptions,
  EliminateFlagsOptions,
  ExpandEquivalencesOptions,
  GuessifyOptions,
  InsertFreelyOptions,
  KillPathsOptions,
  ProjectOptions,
  PruneAlphabetOptions,
  PushOptions,
  RepeatOptions,
  ReweightFunction,
  ReweightOptions,
  SetNameOptions,
  SubstituteOptions,
} from "./compile-transducer.ts";
export type {
  AlphabetComparison,
  Alphabets,
  AttTextOptions,
  CompareOptions,
  DhfstAcceptorOptions,
  DhfstErrorModelOptions,
  ExtractedPath,
  ExtractPathsOptions,
  GuessOptions,
  GuessReport,
  SummarizeOptions,
  Summary,
  SummaryDetails,
  TextOptions,
  ToBytesOptions,
  TransducerBase,
  TransducerType,
} from "./compile-transducer-base.ts";
export { Tokeniser } from "./compile-tokeniser.ts";
export type { PmatchLocation, PmatchOptions, PmatchResult } from "./compile-tokeniser.ts";
export type {
  AttOptions,
  CompileFiles,
  Compiled,
  CompiledArchive,
  CompiledRules,
  LexcOptions,
  PmatchCompileOptions,
  PrologOptions,
  RegexOptions,
  SourceOptions,
  StringsOptions,
  TwolcOptions,
  XfstOptions,
  XfstResult,
} from "./compilers.ts";
export type {
  FlookupReportOptions,
  GrepLine,
  GrepOptions,
  GrepReport,
  LookupInputFormat,
  LookupReport,
  LookupReportOptions,
  OptimizedLookupReport,
  OptimizedLookupReportOptions,
  PackBhfstOptions,
  PairTestOptions,
  PairTestResult,
  ReportOptions,
  RuleFailure,
} from "./reports.ts";
export type { FormatOptions, Reading, Token, TokeniseFormat, TokeniseOptions } from "./tokenise.ts";
export type { TransducerProperties } from "./transducer.ts";
export { HfstError } from "./errors.ts";
export type { Diagnostic, DiagnosticLabel, DiagnosticSpan, HfstErrorCode } from "./errors.ts";
export type { InitSource } from "./core.ts";
export type { LogEvent, Logger, LogLevel } from "./logging.ts";
export type { LookupOptions, LookupResult } from "./lookup.ts";
