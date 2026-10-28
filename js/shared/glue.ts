// The generated wasm-bindgen glue, as the shared wrapper uses it. Each
// package's binding module hands its own generated module to the wrapper, and
// tsc checks at build time that the generated module satisfies these types.
// Declaring them here lets deno check the wrapper on a checkout that has no
// build output.

/** What the generated init function is given. The wrapper resolves URLs itself. */
export type GlueInitInput = BufferSource | Response | WebAssembly.Module;

/** The exports of `crates/hfst-wasm-runtime`, which both packages carry. */
export interface RuntimeGlue {
  /** Instantiates the module. The generated glue keeps the instance it makes. */
  default(options: { module_or_path: GlueInitInput }): Promise<unknown>;
  version(): string;
}
