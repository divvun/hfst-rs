// The generated wasm-bindgen glue, as the wrapper uses it. The generated
// module hands most results back untyped; these types give them the shapes
// the module builds, and tsc checks at build time that the generated module
// satisfies them when binding.ts hands it to Core. Declaring them here also
// lets the unit tests run the wrapper on a fake glue, and lets deno check the
// wrapper on a checkout that has no build output.

import type { LookupResult } from "./lookup.ts";
import type { FormatOptions, Token, TokeniseOptions } from "./tokenise.ts";

/** What the generated init function is given. The wrapper resolves URLs itself. */
export type GlueInitInput = BufferSource | Response | WebAssembly.Module;

/**
 * What the module calls back into, from `crates/hfst-wasm/src/sink.rs`.
 * Levels are numbers: 0 is off, then 1 error, 2 warn, 3 info, 4 debug, 5 trace.
 * These calls arrive while the module is running, so none of them may throw:
 * an exception would unwind through the module and leave it half run.
 */
export interface GlueSink {
  /** One tracing event, which the module has already checked against the level. */
  log(level: number, target: string, message: string): void;
  /** A panic's message. The trap that ends the call follows at once. */
  panic(message: string): void;
}

/**
 * An object the glue hands out, which owns module memory until `free()`.
 * wasm-bindgen also frees one from its own `FinalizationRegistry` when the
 * object becomes unreachable, which the wrapper must never let happen after a
 * trap; see `Core`.
 */
export interface GlueObject {
  free(): void;
}

/** A transducer the glue holds, from `crates/hfst-wasm/src/transducer.rs`. */
export interface TransducerGlue extends GlueObject {
  /** The name of its format, one the wrapper lists as a transducer type. */
  format(): string;
  name(): string;
  /** Its other properties, flattened to key, value, key, value. */
  propertyPairs(): string[];
  /** The results for `input` within the limits, in the order `lookup` returns them. */
  lookup(input: string, limit: number | undefined, timeLimitMs: number | undefined): LookupResult[];
}

/** A tokeniser the glue holds, from `crates/hfst-wasm/src/tokenise.rs`. */
export interface TokeniserGlue extends GlueObject {
  /** The tokens of `text`, which the module builds as plain objects. */
  tokenise(text: string, options: TokeniseOptions): Token[];
  /** What `hfst tokenise` prints for `text` in the format named `format`. */
  format(text: string, format: string, options: FormatOptions): string;
}

/** The exports of `crates/hfst-wasm`. */
export interface Glue {
  /** Instantiates the module. The generated glue keeps the instance it makes. */
  default(options: { module_or_path: GlueInitInput }): Promise<unknown>;
  /** Starts the instance with the wrapper's sink and a log level. Called once, by `init()`. */
  install(sink: GlueSink, level: number): void;
  /** Sends events at `level` and above from here on, and nothing below it. */
  setLogLevel(level: number): void;
  version(): string;
  /** Every optimized-lookup transducer in a stream, in stream order. */
  readTransducers(bytes: Uint8Array): TransducerGlue[];
  /** The tokeniser in a pmatch container's bytes. */
  readTokeniser(bytes: Uint8Array): TokeniserGlue;
}
