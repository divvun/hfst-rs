import type { CompileGlue, CompileTransducerGlue, TransducerListGlue } from "./compile-glue.ts";
import { type Tokeniser, wrapTokeniser } from "./compile-tokeniser.ts";
import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import { Handle } from "./handle.ts";
import { decodeResults, type LookupOptions, type LookupResult, lookupArgs } from "./lookup.ts";
import { type TransducerProperties, transducerProperties } from "./transducer.ts";

// [spec:hfst:req:javascript.compile-module]
/**
 * The transducer types `@divvun/hfst/compile` holds, under the names
 * `hfst fst2fst --format` gives them. They do not change between releases.
 *
 * - `openfst-tropical`: OpenFst with tropical weights, what the compilers
 *   build in by default.
 * - `foma`: foma's unweighted transducers.
 * - `optimized-lookup-weighted`: HFST's weighted optimized-lookup format
 *   (`HFST_OLW`), which analysers, spellers and pmatch containers use.
 * - `optimized-lookup-unweighted`: the unweighted optimized-lookup format
 *   (`HFST_OL`).
 *
 * The operations work on tropical OpenFst and foma; optimized lookup is for
 * looking up and writing, as it is for the tools.
 */
const TRANSDUCER_TYPES = [
  "openfst-tropical",
  "foma",
  "optimized-lookup-weighted",
  "optimized-lookup-unweighted",
] as const;

export type TransducerType = (typeof TRANSDUCER_TYPES)[number];

export interface SummarizeOptions {
  /** Measures what `--verbose` measures besides. */
  verbose?: boolean;
}

export interface CompareOptions {
  doNotHarmonize?: boolean;
  /** Eliminates the flag diacritics of both first. */
  eliminateFlags?: boolean;
}

/**
 * Options of `extractPaths`. Each limit left out sets none: at most
 * `maxStrings` paths, the `nbest` lightest, `random` paths chosen at random,
 * cycles followed at most `cycles` times, and only paths within `beam` of
 * the lightest, whose input is at most `maxInLength` bytes and output
 * `maxOutLength`. Each path must start with `inPrefix` and `outPrefix` and
 * must not contain `inExclude` or `outExclude`.
 */
export interface ExtractPathsOptions {
  maxStrings?: number;
  nbest?: number;
  random?: number;
  cycles?: number;
  beam?: number;
  /** Drops the paths flag diacritics forbid. */
  obeyFlags?: boolean;
  /** Keeps the flag diacritics in the paths. */
  printFlags?: boolean;
  maxInLength?: number;
  maxOutLength?: number;
  inPrefix?: string;
  outPrefix?: string;
  inExclude?: string;
  outExclude?: string;
}

export interface GuessOptions {
  /** The model form generator `guessify` returned. */
  generator?: TransducerBase;
  /** The text of a model form file: generate model forms for each guess. */
  modelForms?: string;
  /** Default 5. */
  maxNumberOfGuesses?: number;
  /** Default 2. */
  maxNumberOfForms?: number;
  /** Default 50. */
  generateThreshold?: number;
}

export interface ToBytesOptions {
  /** Leaves out the HFST3 header, as `--use-backend-format`. */
  useBackendFormat?: boolean;
  /** Packs optimized-lookup tables less tightly, as `--quick`. */
  quick?: boolean;
}

/**
 * Options of the text writers. Weights are written for every type but foma,
 * which has none, unless `doNotPrintWeights`; `printWeights` writes them for
 * foma too.
 */
export interface TextOptions {
  printWeights?: boolean;
  doNotPrintWeights?: boolean;
}

export interface AttTextOptions extends TextOptions {
  /** Writes symbol numbers rather than names. */
  useNumbers?: boolean;
}

export interface DhfstAcceptorOptions {
  /** Packs the conversion's tables less tightly. */
  quick?: boolean;
  /**
   * The file name, without its directory, the tool would have read the
   * transducer from, which the file's meta section names.
   */
  sourceName?: string;
}

export interface DhfstErrorModelOptions extends DhfstAcceptorOptions {
  /**
   * The longest fallback chain, as `--max-fallback-depth`; `null` for no
   * bound, as `--unbounded-fallback`. Default 4.
   */
  maxFallbackDepth?: number | null;
}

/** What `hfst summarize --verbose` measures besides the summary. */
export interface SummaryDetails {
  infinitelyAmbiguous: boolean;
  /** The symbols a path can start with; `null` for optimized lookup, which has no graph to walk. */
  firstInputSymbols: string[] | null;
}

/** What `hfst summarize` reports on a transducer. */
export interface Summary {
  name: string;
  type: TransducerType;
  states: number;
  finalStates: number;
  arcs: number;
  /** Arcs with an epsilon on both sides. */
  ioEpsilons: number;
  inputEpsilons: number;
  outputEpsilons: number;
  /** The most arcs a state has. */
  densestArcs: number;
  /** The fewest arcs a state has. */
  sparsestArcs: number;
  uniqueInputArcs: number;
  uniqueOutputArcs: number;
  /** The input symbol with the most arcs out of one state, and how many. */
  mostAmbiguousInput: [string, number];
  mostAmbiguousOutput: [string, number];
  /** The symbols the arcs use. */
  foundAlphabet: string[];
  /** Each pair of symbols an arc carries, input then output, and how many arcs carry it. */
  symbolPairs: [string, string, number][];
  acceptor: boolean;
  inputDeterministic: boolean;
  outputDeterministic: boolean;
  cyclic: boolean;
  cyclicAtInitialState: boolean;
  /** The alphabet the transducer declares. */
  alphabet: string[];
  /** What `verbose` measures besides; `null` without it. */
  details: SummaryDetails | null;
}

/** `hfst check-alpha`'s report on two transducers. */
export interface AlphabetComparison {
  /** The symbols the first transducer's arcs use. */
  firstUsed: string[];
  secondUsed: string[];
  /** The alphabet the first transducer declares. */
  firstDeclared: string[];
  secondDeclared: string[];
  /** Whether either has a symbol, used or declared, the other lacks. */
  mismatch: boolean;
}

/** `hfst dump-alphabets`' report on a transducer. */
export interface Alphabets {
  declared: string[];
  /** The symbols its arcs use. */
  seen: string[];
}

/** One path `hfst fst2strings` prints. */
export interface ExtractedPath {
  /** The input side as the tool prints it: no epsilons, and flag diacritics only with `printFlags`. */
  input: string;
  output: string;
  /** The path's symbol pairs, input then output, an epsilon as `""`. */
  pairs: [string, string][];
  weight: number;
}

/** What `hfst guess` prints for a word. */
export interface GuessReport {
  input: string;
  /** Whether `guesses` are paradigms, as with model forms, rather than analyses. */
  paradigms: boolean;
  /**
   * Each guessed analysis or, with model forms, for each guess the word, the
   * analysis and the model forms generated for it, separated by tabs.
   */
  guesses: string[];
}

export function isTransducerType(type: string): type is TransducerType {
  return (TRANSDUCER_TYPES as readonly string[]).includes(type);
}

export function invalid(message: string): HfstError {
  return new HfstError("invalid-argument", message);
}

/** The options a method was given, refused unless an object; the module checks each field. */
export function given<T extends object>(call: string, options: T | undefined): T | undefined {
  if (options === undefined) return undefined;
  if (typeof options !== "object" || options === null || Array.isArray(options)) {
    throw invalid(`${call} takes its options as an object`);
  }
  return options;
}

let glueOf: (call: string, transducer: unknown) => CompileTransducerGlue;

// [spec:hfst:req:javascript.compile-module]
/**
 * What reads a transducer of `@divvun/hfst/compile`: its type, name and
 * properties, lookup, the queries and the writers. {@link Transducer} adds
 * the operations, and is the only kind there is.
 */
export abstract class TransducerBase extends Handle<CompileTransducerGlue> {
  /** The module instance the transducer lives in, where what it makes is wrapped. */
  protected readonly compileCore: Core<CompileGlue>;

  protected constructor(core: Core<CompileGlue>, glue: CompileTransducerGlue) {
    super(core, glue);
    this.compileCore = core;
  }

  static {
    glueOf = (call, transducer) => {
      if (!(transducer instanceof TransducerBase)) {
        throw invalid(
          `${call} takes a Transducer of @divvun/hfst/compile; one of @divvun/hfst crosses over as the bytes its toBytes writes`,
        );
      }
      return transducer.use((glue) => glue);
    };
  }

  /** `f` on this transducer's glue and the glue of `other`, which `call` takes. */
  protected withOther<T>(
    call: string,
    other: unknown,
    f: (glue: CompileTransducerGlue, other: CompileTransducerGlue) => T,
  ): T {
    const theirs = glueOf(call, other);
    return this.use((glue) => f(glue, theirs));
  }

  /** Its type, which `convert` changes. */
  get type(): TransducerType {
    const type = this.use((glue) => glue.format());
    if (!isTransducerType(type)) {
      throw new HfstError("internal", `the module sent a transducer of the unknown type ${type}`);
    }
    return type;
  }

  /** The `name` property of its HFST3 header, or `""` when there is none. */
  get name(): string {
    return this.use((glue) => glue.name());
  }

  /**
   * The properties of its HFST3 header, such as a pmatch container's
   * settings. `name` and `type` have their own accessors, and the header's
   * `version` belongs to the stream rather than the transducer, so none of
   * the three is here.
   */
  get properties(): TransducerProperties {
    return transducerProperties(this.use((glue) => glue.propertyPairs()));
  }

  /**
   * Every result the hfst library's lookup finds for `input`, lightest
   * first, then by output in code point order, then by symbols. In optimized
   * lookup, flag diacritics are obeyed, `limit` stops the search once it has
   * found that many paths, and `timeLimitMs` stops it after that many
   * milliseconds with what it found. In the other types the results are
   * what `hfst lookup` finds, at most `limit` of them.
   */
  lookup(input: string, options?: LookupOptions): LookupResult[] {
    const [text, limit, timeLimitMs] = lookupArgs(input, options);
    return decodeResults(this.use((glue) => glue.lookup(text, limit, timeLimitMs)));
  }

  /** What `hfst summarize` reports on the transducer. */
  summarize(options?: SummarizeOptions): Summary {
    return this.use((glue) => glue.summarize(given("summarize", options)));
  }

  /** What `hfst check-alpha` reports on the transducer and `other`. */
  checkAlpha(other: TransducerBase): AlphabetComparison {
    return this.withOther("checkAlpha", other, (glue, theirs) => glue.checkAlpha(theirs));
  }

  /**
   * Whether the transducer and `other` are equivalent, as `hfst compare`
   * decides. Like the tool, it compares two transducers of one algebra type.
   */
  compare(other: TransducerBase, options?: CompareOptions): boolean {
    return this.withOther("compare", other, (glue, theirs) => glue.compare(theirs, given("compare", options)));
  }

  /** What `hfst dump-alphabets` reports on the transducer. */
  dumpAlphabets(): Alphabets {
    return this.use((glue) => glue.dumpAlphabets());
  }

  /** The paths `hfst fst2strings` prints, in its order. */
  extractPaths(options?: ExtractPathsOptions): ExtractedPath[] {
    return this.use((glue) => glue.extractPaths(given("extractPaths", options)));
  }

  /**
   * What `hfst guess` prints for each of `words` with the transducer, a
   * guesser `guessify` made. With `modelForms` it generates model forms for
   * each guess with `generator`, or with one compiled from the guesser.
   */
  guess(words: readonly string[], options?: GuessOptions): GuessReport[] {
    const checked = strings("guess", words);
    const { generator, ...rest } = given("guess", options) ?? {};
    if (generator === undefined) return this.use((glue) => glue.guess(checked, rest));
    return this.withOther("guess", generator, (glue, theirs) => glue.guessWith(checked, theirs, rest));
  }

  /**
   * The tokeniser `hfst tokenize` makes of a transducer that is not a pmatch
   * container: each word of the transducer's input side a token, analysed as
   * its outputs.
   */
  naiveTokeniser(): Tokeniser {
    return wrapTokeniser(this.compileCore, this.use((glue) => glue.naiveTokeniser()));
  }

  /**
   * The bytes of a stream holding the transducer, as `hfst fst2fst` writes
   * it: in `type`, converted and named as `--format` converts and names it,
   * or as it is, in its own type, when `type` is left out.
   */
  toBytes(type?: TransducerType, options?: ToBytesOptions): Uint8Array {
    if (type !== undefined && typeof type !== "string") throw invalid("toBytes takes the type as a string");
    return this.use((glue) => glue.toBytes(type, given("toBytes", options)));
  }

  /** The AT&T text `hfst fst2txt` writes of the transducer. */
  toAtt(options?: AttTextOptions): string {
    return this.use((glue) => glue.toAtt(given("toAtt", options)));
  }

  /**
   * The Prolog text `hfst fst2txt -f prolog` writes of the transducer alone
   * in its stream, so a transducer with no name is `NO_NAME_1`.
   */
  toProlog(options?: TextOptions): string {
    return this.use((glue) => glue.toProlog(given("toProlog", options)));
  }

  /** The Graphviz text `hfst fst2txt -f dot` writes of the transducer. */
  toDot(options?: TextOptions): string {
    return this.use((glue) => glue.toDot(given("toDot", options)));
  }

  /** The PC-KIMMO text `hfst fst2txt -f pckimmo` writes of the transducer. */
  toPckimmo(options?: TextOptions): string {
    return this.use((glue) => glue.toPckimmo(given("toPckimmo", options)));
  }

  /**
   * The DHFST acceptor (type 2) `hfst fst2fst -f dhfst --dhfst-type
   * acceptor` writes of the transducer, which `readDhfst` of `@divvun/hfst`
   * reads.
   */
  toDhfstAcceptor(options?: DhfstAcceptorOptions): Uint8Array {
    return this.use((glue) => glue.toDhfstAcceptor(given("toDhfstAcceptor", options)));
  }

  /**
   * The DHFST error model (type 1) `hfst fst2fst -f dhfst --dhfst-type
   * errmodel` writes of the transducer.
   */
  toDhfstErrorModel(options?: DhfstErrorModelOptions): Uint8Array {
    return this.use((glue) => glue.toDhfstErrorModel(given("toDhfstErrorModel", options)));
  }
}

/** `words` checked to be an array of strings, as the module takes it. */
export function strings(call: string, words: unknown): string[] {
  if (!Array.isArray(words) || !words.every((word) => typeof word === "string")) {
    throw invalid(`${call} takes an array of strings`);
  }
  return words;
}

// [spec:hfst:req:javascript.compile-module]
/**
 * Runs `f` with a list of the glue of `transducers` in the module instance
 * `core`, and frees the list once `f` returns.
 */
export function withList<T>(
  core: Core<CompileGlue>,
  call: string,
  transducers: readonly TransducerBase[],
  f: (list: TransducerListGlue) => T,
): T {
  if (!Array.isArray(transducers)) throw invalid(`${call} takes an array of Transducers`);
  const glues = transducers.map((transducer) => glueOf(call, transducer));
  const list = core.call((glue) => new glue.TransducerList());
  try {
    core.call(() => {
      for (const glue of glues) list.push(glue);
    });
    return f(list);
  } finally {
    core.call(() => list.free());
  }
}

