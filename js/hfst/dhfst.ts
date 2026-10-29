import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { DhfstAcceptorGlue, DhfstErrorModelGlue, DhfstGlue, GlueObject } from "./glue.ts";
import { Handle } from "./handle.ts";

// [spec:hfst:req:javascript.dhfst+2]
/** Which of divvunspell's DHFST formats a file holds: type 2 or type 1. */
export type DhfstType = "acceptor" | "error-model";

// [spec:hfst:req:javascript.dhfst+2]
/** How a DHFST acceptor stores its arcs, as `dhfst-tools` reports it. */
export interface AcceptorInfo {
  /** State numbers: every state is numbered below this. */
  ids: number;
  /** Slots in the table, at least `ids` plus `labelMax`. */
  slots: number;
  /** Slots that hold arcs, free or regular. */
  usedSlots: number;
  /** Bytes per slot check: 1 when every regular label is below 256, else 2. */
  checkBytes: number;
  /** The greatest regular label. */
  labelMax: number;
  /** Bytes per record, each a target and the index of a weight. */
  recordBytes: number;
  /** Records in the list of arcs that do not fit their slot. */
  listRecords: number;
  /** Arcs, free and regular. */
  arcs: number;
  /** Free arcs: epsilon and flag diacritic arcs. */
  freeArcs: number;
  /** Distinct symbol and weight pairs of the free arcs. */
  freePairs: number;
  /** Distinct regular arc weights. */
  weights: number;
  /** Final states. */
  finals: number;
  /** Whether the file stores each state's distance to a final state. */
  distances: boolean;
}

/** What both DHFST handles read once, when they load. */
interface Metadata {
  version: number;
  symbols: readonly string[];
  meta: string | null;
}

function metadata(glue: DhfstAcceptorGlue | DhfstErrorModelGlue): Metadata {
  return {
    version: glue.version(),
    symbols: Object.freeze(glue.symbols()),
    meta: glue.meta() ?? null,
  };
}

let wrapAcceptor: (core: Core, glue: DhfstAcceptorGlue) => DhfstAcceptor;
let wrapErrorModel: (core: Core, glue: DhfstErrorModelGlue) => DhfstErrorModel;

/**
 * A DHFST acceptor (type 2): a speller's lexicon, loaded and validated as
 * divvunspell loads it. Its metadata is read once, when it loads; reading it
 * after `free()` throws `freed`, as any other use does. It owns module memory:
 * call `free()` when done with it, or declare it with `using`.
 */
export class DhfstAcceptor extends Handle<DhfstAcceptorGlue> {
  readonly #metadata: Metadata;
  readonly #info: Readonly<AcceptorInfo>;

  private constructor(core: Core, glue: DhfstAcceptorGlue) {
    super(core, glue);
    this.#metadata = this.use(metadata);
    this.#info = Object.freeze(this.use((g) => g.info()));
  }

  static {
    wrapAcceptor = (core, glue) => new DhfstAcceptor(core, glue);
  }

  /** Always `acceptor`, which tells this from a {@link DhfstErrorModel}. */
  get type(): "acceptor" {
    return this.use(() => "acceptor");
  }

  /** The format version the file declares. */
  get version(): number {
    return this.use(() => this.#metadata.version);
  }

  /** The symbol names, `@_EPSILON_SYMBOL_@` first. */
  get symbols(): readonly string[] {
    return this.use(() => this.#metadata.symbols);
  }

  /** The JSON the writer left in the file's `meta` section, or `null` when it has none. */
  get meta(): string | null {
    return this.use(() => this.#metadata.meta);
  }

  /** How the file stores its arcs. */
  get info(): Readonly<AcceptorInfo> {
    return this.use(() => this.#info);
  }

  // [spec:hfst:req:javascript.dhfst+2]
  /**
   * The least weight of a path that accepts `word`, final weight included,
   * or `null` when no path does: what divvunspell's speller finds checking
   * the word as typed against this lexicon alone. The word is read one
   * extended grapheme cluster at a time, each the symbol of the same text or
   * else the lexicon's unknown symbol, so a multicharacter symbol is never
   * read out of several clusters and a symbol only matches text in its own
   * Unicode normalisation form. Epsilon and flag diacritic arcs are walked,
   * and the flags obeyed, as divvunspell does. There is no case folding or
   * suggestion search: those are divvunspell's.
   */
  lookup(word: string): number | null {
    if (typeof word !== "string") {
      throw new HfstError("invalid-argument", `lookup takes the word as a string, not ${typeof word}`);
    }
    return this.use((g) => g.lookup(word) ?? null);
  }
}

/**
 * A DHFST error model (type 1), loaded and validated as divvunspell loads
 * one. The package offers no suggestion search, so an error model only
 * describes itself. It owns module memory: call `free()` when done with it,
 * or declare it with `using`.
 */
export class DhfstErrorModel extends Handle<DhfstErrorModelGlue> {
  readonly #metadata: Metadata;
  readonly #stateCount: number;

  private constructor(core: Core, glue: DhfstErrorModelGlue) {
    super(core, glue);
    this.#metadata = this.use(metadata);
    this.#stateCount = this.use((g) => g.stateCount());
  }

  static {
    wrapErrorModel = (core, glue) => new DhfstErrorModel(core, glue);
  }

  /** Always `error-model`, which tells this from a {@link DhfstAcceptor}. */
  get type(): "error-model" {
    return this.use(() => "error-model");
  }

  /** The format version the file declares. */
  get version(): number {
    return this.use(() => this.#metadata.version);
  }

  /** The number of states, one for each state of the source transducer that its start state reaches. */
  get stateCount(): number {
    return this.use(() => this.#stateCount);
  }

  /** The symbol names, `@_EPSILON_SYMBOL_@` first. */
  get symbols(): readonly string[] {
    return this.use(() => this.#metadata.symbols);
  }

  /** The JSON the writer left in the file's `meta` section, or `null` when it has none. */
  get meta(): string | null {
    return this.use(() => this.#metadata.meta);
  }
}

export interface DhfstReader {
  // [spec:hfst:req:javascript.dhfst+2]
  /**
   * The DHFST file in `bytes`, an acceptor or an error model, as its `type`
   * says. Bytes are copied into the module once; an error model keeps none of
   * them. A file that fails the validation divvunspell runs on load, or is
   * truncated, throws `invalid-format` with the reason. Bytes that are not
   * DHFST throw `invalid-format`, or `unsupported-format` naming a format the
   * package does not read.
   */
  readDhfst(bytes: Uint8Array): DhfstAcceptor | DhfstErrorModel;
}

/** The {@link DhfstReader} that loads into the module instance `core`. */
export function dhfstReader(core: Core): DhfstReader {
  return {
    readDhfst: (bytes) => {
      if (!(bytes instanceof Uint8Array)) {
        throw new HfstError("invalid-argument", "a DHFST file loads from its bytes, as a Uint8Array");
      }
      return core.call((glue) => wrap(core, glue.readDhfst(bytes)));
    },
  };
}

function wrap(core: Core, read: DhfstGlue): DhfstAcceptor | DhfstErrorModel {
  switch (read.type) {
    case "acceptor":
      return wrapAcceptor(core, read.model);
    case "error-model":
      return wrapErrorModel(core, read.model);
    default: {
      const unknown: { type: unknown; model: GlueObject } = read;
      unknown.model.free();
      throw new HfstError("internal", `the module sent a DHFST file of the unknown type ${String(unknown.type)}`);
    }
  }
}
