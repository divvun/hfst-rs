// The generated wasm-bindgen glue, as the shared wrapper uses it. Each
// package's binding module hands its own generated module to the wrapper, and
// tsc checks at build time that the generated module satisfies these types.
// Declaring them here lets deno check the wrapper on a checkout that has no
// build output.

/** What the generated init function is given. The wrapper resolves URLs itself. */
export type GlueInitInput = BufferSource | Response | WebAssembly.Module;

/**
 * What the module calls back into, from `crates/hfst-wasm-runtime/src/sink.rs`.
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

/** The exports of `crates/hfst-wasm-runtime`, which both packages carry. */
export interface RuntimeGlue {
  /** Instantiates the module. The generated glue keeps the instance it makes. */
  default(options: { module_or_path: GlueInitInput }): Promise<unknown>;
  /** Starts the instance with the wrapper's sink and a log level. Called once, by `init()`. */
  install(sink: GlueSink, level: number): void;
  /** Sends events at `level` and above from here on, and nothing below it. */
  setLogLevel(level: number): void;
  version(): string;
}
