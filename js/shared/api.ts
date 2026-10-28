import type { Core, InitSource } from "./core.ts";

// [spec:hfst:req:javascript.typed-surface]
/**
 * The functions both packages export, under these names. Results are plain
 * values; `js/build.mjs` fails a build whose declarations use `any`.
 */
export interface RuntimeApi {
  /**
   * Loads and starts the package's WebAssembly module. Call it once and wait
   * for it before anything else. With no argument it loads the `.wasm` file
   * that ships beside the package. Calling it again returns the same promise.
   */
  init(source?: InitSource): Promise<void>;
  /** The Divvun HFST version the package was built from. */
  version(): string;
}

/** The {@link RuntimeApi} functions, bound to one package's module instance. */
export function runtimeApi(core: Core): RuntimeApi {
  return {
    init: (source) => core.init(source),
    version: () => core.call((glue) => glue.version()),
  };
}
