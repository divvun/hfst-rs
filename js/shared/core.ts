import { HfstError, messageOf } from "./errors.ts";
import type { GlueInitInput, RuntimeGlue } from "./glue.ts";

/**
 * Where `init()` loads the module from: a URL (a string is read as one), a
 * `Response`, the module's bytes, or a compiled `WebAssembly.Module`.
 */
export type InitSource = string | URL | Response | BufferSource | WebAssembly.Module;

/**
 * One package's WebAssembly module instance and the state the wrapper keeps
 * about it. Each package makes exactly one, and every call into the module
 * goes through it.
 */
export class Core {
  readonly #glue: RuntimeGlue;
  readonly #defaultSource: URL;
  #starting: Promise<void> | null = null;
  #ready = false;

  /** `defaultSource` is the package's own `.wasm` file. Nothing is loaded yet. */
  constructor(glue: RuntimeGlue, defaultSource: URL) {
    this.#glue = glue;
    this.#defaultSource = defaultSource;
  }

  // [spec:hfst:req:javascript.init]
  /**
   * Loads and starts the module. Every call after the first returns the first
   * call's promise, whatever its argument, so there is only ever one instance.
   * A start that fails leaves no instance, and the next call tries again.
   */
  init(source?: InitSource): Promise<void> {
    if (this.#starting === null) {
      const starting = this.#start(source ?? this.#defaultSource);
      this.#starting = starting;
      starting.catch(() => {
        if (this.#starting === starting) this.#starting = null;
      });
    }
    return this.#starting;
  }

  async #start(source: InitSource): Promise<void> {
    try {
      await this.#glue.default({ module_or_path: await load(source) });
    } catch (error) {
      throw new HfstError("init-failed", `cannot start Divvun HFST: ${messageOf(error)}`, {
        cause: error,
      });
    }
    this.#ready = true;
  }

  /** Runs `f` against the glue, once `init()` has resolved. */
  call<T>(f: (glue: RuntimeGlue) => T): T {
    if (!this.#ready) {
      throw new HfstError(
        "not-initialised",
        "Divvun HFST is not initialised: call init() and wait for it to resolve first",
      );
    }
    return f(this.#glue);
  }
}

async function load(source: InitSource): Promise<GlueInitInput> {
  if (typeof source === "string" || source instanceof URL) {
    const url = typeof source === "string" && URL.canParse(source) ? new URL(source) : source;
    if (url instanceof URL && url.protocol === "file:") return await readFile(url);
    return checked(await fetch(url));
  }
  if (source instanceof Response) return checked(source);
  return source;
}

function checked(response: Response): Response {
  if (!response.ok) {
    throw new Error(`${response.url || "the response"} answered HTTP ${response.status}`);
  }
  return response;
}

/** The part of `node:fs/promises` that {@link readFile} uses. */
interface NodeFs {
  readFile(path: URL): Promise<Uint8Array<ArrayBuffer>>;
}

// Node's fetch cannot read file: URLs, so they are read with node:fs. The
// specifier is a variable so that bundlers and type checkers do not follow it:
// code that never passes a file URL never asks for node:fs.
async function readFile(url: URL): Promise<Uint8Array<ArrayBuffer>> {
  const specifier = "node:fs/promises";
  const fs: NodeFs = await import(/* @vite-ignore */ /* webpackIgnore: true */ specifier);
  return await fs.readFile(url);
}
