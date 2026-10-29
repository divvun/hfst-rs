// [spec:hfst:def:javascript.packages+2]
/**
 * `@divvun/hfst`: Divvun HFST for JavaScript. It loads optimized-lookup
 * transducers, pmatch containers and DHFST spellers and runs lookup,
 * tokenisation and DHFST acceptor lookup, in browsers, Web Workers, Deno, Node
 * and Bun. It replaces `hfst-js` for reading and looking up transducers, and
 * does not compile, write or transform them.
 *
 * @module
 */
import { core } from "./binding.ts";
import type { InitSource } from "./core.ts";
import { dhfstReader } from "./dhfst.ts";
import type { Logger, LogLevel } from "./logging.ts";
import { tokeniserReader } from "./tokenise.ts";
import { transducerReaders } from "./transducer.ts";

// [spec:hfst:req:javascript.typed-surface+1]
// This module declares the whole public API. Results are plain values, and
// js/build.mjs fails a build whose declarations use any.
/**
 * Loads and starts the package's WebAssembly module. Call it once and wait
 * for it before anything else. With no argument it loads the `.wasm` file
 * that ships beside the package. Calling it again returns the same promise.
 */
export function init(source?: InitSource): Promise<void> {
  return core.init(source);
}

/** The Divvun HFST version the package was built from. */
export function version(): string {
  return core.call((glue) => glue.version());
}

/**
 * Sends the library's log events to `logger`. `null` puts back the default,
 * which writes each event to the console method for its level.
 */
export function setLogger(logger: Logger | null): void {
  core.setLogger(logger);
}

/**
 * Logs events at `level` and the more severe levels; `off` logs nothing.
 * The default is `warn`. The module never sends the levels below it.
 */
export function setLogLevel(level: LogLevel): void {
  core.setLogLevel(level);
}

export const { readTransducers, readTransducer } = transducerReaders(core);
export const { readTokeniser } = tokeniserReader(core);
export const { readDhfst } = dhfstReader(core);
export { Transducer } from "./transducer.ts";
export type { TransducerProperties, TransducerType } from "./transducer.ts";
export { Tokeniser } from "./tokenise.ts";
export type {
  FormatOptions,
  Reading,
  Token,
  TokeniseFormat,
  TokeniseOptions,
} from "./tokenise.ts";
export { DhfstAcceptor, DhfstErrorModel } from "./dhfst.ts";
export type { AcceptorInfo, DhfstType } from "./dhfst.ts";
export { HfstError } from "./errors.ts";
export type { HfstErrorCode } from "./errors.ts";
export type { InitSource } from "./core.ts";
export type { LogEvent, Logger, LogLevel } from "./logging.ts";
export type { LookupOptions, LookupResult } from "./lookup.ts";
