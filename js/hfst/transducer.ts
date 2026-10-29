import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { TransducerGlue } from "./glue.ts";
import { Handle } from "./handle.ts";
import { type LookupOptions, type LookupResult, lookupArgs } from "./lookup.ts";

// [spec:hfst:req:javascript.load-bytes+2]
/**
 * The transducer types the package reads, under the names
 * `hfst fst2fst --format` gives them. They do not change between releases.
 *
 * - `optimized-lookup-weighted`: HFST's weighted optimized-lookup format
 *   (`HFST_OLW`), which analysers, spellers and pmatch containers use.
 * - `optimized-lookup-unweighted`: the unweighted optimized-lookup format
 *   (`HFST_OL`).
 */
const TRANSDUCER_TYPES = ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const;

export type TransducerType = (typeof TRANSDUCER_TYPES)[number];

/** The HFST3 header properties a transducer carries. */
export type TransducerProperties = Readonly<Record<string, string>>;

function isTransducerType(type: string): type is TransducerType {
  return (TRANSDUCER_TYPES as readonly string[]).includes(type);
}

function properties(pairs: readonly string[]): TransducerProperties {
  const object: Record<string, string> = {};
  for (let i = 0; i + 1 < pairs.length; i += 2) {
    const key = pairs[i];
    const value = pairs[i + 1];
    if (key !== undefined && value !== undefined) object[key] = value;
  }
  return Object.freeze(object);
}

let wrap: (core: Core, glue: TransducerGlue) => Transducer;

/**
 * An optimized-lookup transducer loaded from bytes. Its type, name and
 * properties are read once, when it loads; reading them after `free()` throws
 * `freed`, as any other use does. It owns module memory: call `free()` when
 * done with it, or declare it with `using`.
 */
export class Transducer extends Handle<TransducerGlue> {
  readonly #type: TransducerType;
  readonly #name: string;
  readonly #properties: TransducerProperties;

  private constructor(core: Core, glue: TransducerGlue) {
    super(core, glue);
    const type = this.use((g) => g.format());
    if (!isTransducerType(type)) {
      throw new HfstError("internal", `the module sent a transducer of the unknown type ${type}`);
    }
    this.#type = type;
    this.#name = this.use((g) => g.name());
    this.#properties = properties(this.use((g) => g.propertyPairs()));
  }

  static {
    wrap = (core, glue) => new Transducer(core, glue);
  }

  /** Which optimized-lookup format the transducer is in, weighted or unweighted. */
  get type(): TransducerType {
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

  // [spec:hfst:req:javascript.lookup]
  /**
   * Every result the hfst library's optimized lookup finds for `input` within
   * the limits, ordered by weight, lightest first, then by output in code
   * point order, then by symbols. Flag diacritics are obeyed, so a path they
   * block is not a result. Input with no analyses, such as input in symbols
   * the transducer cannot read, gives an empty array; a time limit that runs
   * out gives the results found so far.
   */
  lookup(input: string, options?: LookupOptions): LookupResult[] {
    const [text, limit, timeLimitMs] = lookupArgs(input, options);
    return this.use((g) => g.lookup(text, limit, timeLimitMs));
  }
}

export interface TransducerReaders {
  /**
   * Every transducer in an HFST transducer stream, in stream order. A pmatch
   * container loads as TOP followed by its definitions, each under its name.
   * Optimized-lookup transducers load with or without an HFST3 header.
   * Bytes are copied into the module once and not kept. Bytes in any other
   * format throw `unsupported-format`, naming it; bytes that are not a
   * transducer stream, or end partway through one, throw `invalid-format`.
   */
  readTransducers(bytes: Uint8Array): Transducer[];
  /**
   * The one transducer in a stream. A stream holding several throws
   * `transducer-count`, and frees what it loaded.
   */
  readTransducer(bytes: Uint8Array): Transducer;
}

/** {@link TransducerReaders} that load into the module instance `core`. */
export function transducerReaders(core: Core): TransducerReaders {
  const readTransducers = (bytes: Uint8Array): Transducer[] => {
    if (!(bytes instanceof Uint8Array)) {
      throw new HfstError("invalid-argument", "transducers load from the stream's bytes, as a Uint8Array");
    }
    return core.call((glue) => glue.readTransducers(bytes).map((transducer) => wrap(core, transducer)));
  };
  return {
    readTransducers,
    readTransducer: (bytes) => single(readTransducers(bytes)),
  };
}

/** The one transducer of `all`. When there are more, they are freed and `transducer-count` is thrown. */
function single(all: Transducer[]): Transducer {
  const [first] = all;
  if (first !== undefined && all.length === 1) return first;
  for (const transducer of all) transducer.free();
  throw new HfstError(
    "transducer-count",
    `readTransducer takes exactly one transducer, and the stream holds ${all.length}`,
  );
}
