import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { TransducerGlue } from "./glue.ts";
import { Handle } from "./handle.ts";

// [spec:hfst:req:javascript.load-bytes+1]
/**
 * The transducer types `@divvun/hfst-runtime` reads, under the names
 * `hfst fst2fst --format` gives them. They do not change between releases.
 *
 * - `optimized-lookup-weighted`: HFST's weighted optimized-lookup format
 *   (`HFST_OLW`), which analysers, spellers and pmatch containers use.
 * - `optimized-lookup-unweighted`: the unweighted optimized-lookup format
 *   (`HFST_OL`).
 */
export const RUNTIME_TRANSDUCER_TYPES = [
  "optimized-lookup-weighted",
  "optimized-lookup-unweighted",
] as const;

export type RuntimeTransducerType = (typeof RUNTIME_TRANSDUCER_TYPES)[number];

/** The HFST3 header properties a transducer carries. */
export type TransducerProperties = Readonly<Record<string, string>>;

function properties(pairs: readonly string[]): TransducerProperties {
  const object: Record<string, string> = {};
  for (let i = 0; i + 1 < pairs.length; i += 2) {
    const key = pairs[i];
    const value = pairs[i + 1];
    if (key !== undefined && value !== undefined) object[key] = value;
  }
  return Object.freeze(object);
}

/**
 * What every transducer has in both packages. Its type, name and properties
 * are read once, when it loads; reading them after `free()` throws `freed`,
 * as any other use does.
 */
export abstract class TransducerBase<T extends string, G extends TransducerGlue> extends Handle<G> {
  readonly #type: T;
  readonly #name: string;
  readonly #properties: TransducerProperties;

  protected constructor(core: Core, glue: G, types: readonly T[]) {
    super(core, glue);
    const type = this.use((g) => g.format());
    if (!(types as readonly string[]).includes(type)) {
      throw new HfstError("internal", `the module sent a transducer of the unknown type ${type}`);
    }
    this.#type = type as T;
    this.#name = this.use((g) => g.name());
    this.#properties = properties(this.use((g) => g.propertyPairs()));
  }

  /** The transducer's format. */
  get type(): T {
    return this.use(() => this.#type);
  }

  /** The `name` property of its HFST3 header, or `""` when there is none. */
  get name(): string {
    return this.use(() => this.#name);
  }

  /**
   * The properties of its HFST3 header, such as a pmatch container's
   * settings. `name` and `type` have their own accessors, and the header's
   * `version` belongs to the stream rather than the transducer, so none of
   * the three is here. A transducer with no header has none.
   */
  get properties(): TransducerProperties {
    return this.use(() => this.#properties);
  }
}

/** The two functions that load a package's transducers from bytes. */
export interface TransducerReaders<T> {
  /**
   * Every transducer in an HFST transducer stream, in stream order. A pmatch
   * container loads as TOP followed by its definitions, each under its name.
   * Optimized-lookup transducers load with or without an HFST3 header.
   * Bytes are copied into the module once and not kept. Bytes in a type the
   * package does not read throw `unsupported-format`; bytes that are not a
   * transducer stream, or end partway through one, throw `invalid-format`.
   */
  readTransducers(bytes: Uint8Array): T[];
  /**
   * The one transducer in a stream. A stream holding several throws
   * `transducer-count`, and frees what it loaded.
   */
  readTransducer(bytes: Uint8Array): T;
}

/** {@link TransducerReaders} over a package's reader, which `read` calls. */
export function transducerReaders<T extends Disposable>(
  read: (bytes: Uint8Array) => T[],
): TransducerReaders<T> {
  const readTransducers = (bytes: Uint8Array): T[] => {
    if (!(bytes instanceof Uint8Array)) {
      throw new HfstError("invalid-argument", "transducers load from the stream's bytes, as a Uint8Array");
    }
    return read(bytes);
  };
  return {
    readTransducers,
    readTransducer: (bytes) => single(readTransducers(bytes), "readTransducer", "the stream"),
  };
}

/**
 * The one transducer of `all`. When there are more, or none, they are freed
 * and `transducer-count` is thrown: `what` names the call and `source` what
 * it read.
 */
export function single<T extends Disposable>(all: T[], what: string, source: string): T {
  const [first] = all;
  if (first !== undefined && all.length === 1) return first;
  for (const transducer of all) transducer[Symbol.dispose]();
  throw new HfstError(
    "transducer-count",
    `${what} takes exactly one transducer, and ${source} holds ${all.length}`,
  );
}
