import type { CompileGlue, CompileTransducerGlue } from "./compile-glue.ts";
import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import { given, invalid, TransducerBase, type TransducerType, withList } from "./compile-transducer-base.ts";

/** The functions `reweight` applies to a weight, as `hfst reweight --function` names them. */
export type ReweightFunction =
  | "cos"
  | "sin"
  | "tan"
  | "acos"
  | "asin"
  | "atan"
  | "cosh"
  | "sinh"
  | "tanh"
  | "exp"
  | "log"
  | "log10"
  | "sqrt"
  | "floor"
  | "ceil";

export interface ConvertOptions {
  /** Packs optimized-lookup tables less tightly, as `--quick`. */
  quick?: boolean;
}

/** Options of `determinize` and `minimize`. */
export interface DeterminizeOptions {
  /** Treats each weight as part of its arc's label, as `--encode-weights`. */
  encodeWeights?: boolean;
}

export interface PruneAlphabetOptions {
  /** Prunes even when an arc uses the unknown or identity symbol, as `--force`. */
  force?: boolean;
}

export interface ProjectOptions {
  /** The side whose language the transducer becomes the identity on. Default `output`. */
  project?: "input" | "output";
}

/** Options of `pushWeights` and `pushLabels`. */
export interface PushOptions {
  /** Towards the initial state or the final states. Default `final`. */
  push?: "initial" | "final";
}

export interface RepeatOptions {
  /** The fewest repetitions, a whole number or `Infinity`. Default 0. */
  from?: number;
  /** The most repetitions, a whole number or `Infinity`. Default `Infinity`. */
  to?: number;
}

export interface InsertFreelyOptions {
  /** The pair to insert, `IN:OUT`, with `@0@` for the epsilon. */
  symbolPair: string;
  /** Expands the identity and unknown symbols by the pair's symbols. */
  harmonise?: boolean;
}

export interface EliminateFlagsOptions {
  /** The feature whose flags to eliminate; every feature's when left out. */
  flag?: string;
}

export interface AffixGuessifyOptions {
  /** Guess by suffixes or by prefixes. Default `suffix`. */
  direction?: "prefix" | "suffix";
  /** What each guessed character costs. Default 1. */
  weight?: number;
}

/** Options of `expandEquivalences`: `from` with `to`, or `tsv`. */
export interface ExpandEquivalencesOptions {
  from?: string;
  to?: string;
  /** The text of a TSV file, `FROM<TAB>TO<TAB>TO...` a line. */
  tsv?: string;
  /** The side to expand. Default `first`. */
  level?: "first" | "second" | "both";
}

/** Options of `killPaths`: `symbol`, or `tsv`. */
export interface KillPathsOptions {
  symbol?: string;
  /** The text of a file listing the symbols, one a line. */
  tsv?: string;
}

/**
 * Options of `reweight`: each weight in bounds becomes
 * `function(weight * multiplier + addition)`, on the arcs and final states
 * `symbol`, `inputSymbol` and `outputSymbol` select, or by the rules of
 * `tsv`, `SYMBOL<TAB>WEIGHT` a line.
 */
export interface ReweightOptions {
  addition?: number;
  multiplier?: number;
  function?: ReweightFunction;
  lowerBound?: number;
  /** Default: no upper bound. */
  upperBound?: number;
  inputSymbol?: string;
  outputSymbol?: string;
  symbol?: string;
  endStatesOnly?: boolean;
  arcsOnly?: boolean;
  tsv?: string;
}

export interface SetNameOptions {
  /** Cuts the name to this many bytes; 0, the default, does not cut it. */
  truncateLength?: number;
}

export interface EditMetadataOptions {
  /** Each property to add or replace, as `--add NAME=VALUE`. */
  add: Readonly<Record<string, string>>;
  /** Cuts each value to this many bytes; 0, the default, does not cut it. */
  truncateLength?: number;
}

export interface GuessifyOptions {
  /** Default 1. */
  defaultPenalty?: number;
  /** Makes no model form generator, so `guessify` returns `null`. */
  doNotCompileGenerator?: boolean;
}

/**
 * Options of `substitute`: `fromLabel` with `toLabel` or `toTransducer`, or
 * `fromFile`. A label is a symbol or a colon pair, `@0@` for the epsilon.
 */
export interface SubstituteOptions {
  fromLabel?: string;
  toLabel?: string;
  /** The transducer to put in place of each arc `fromLabel` labels. */
  toTransducer?: Transducer;
  /** The text of a relabel file, `FROM<TAB>TO` a line. */
  fromFile?: string;
  /**
   * The name of the file the tool would read `toTransducer` or `fromFile`
   * from, which names the result.
   */
  fileName?: string;
  /** Applies each line of `fromFile` in turn. */
  inOrder?: boolean;
  doNotConvert?: boolean;
  /** Composes symbol-for-symbol substitutions in rather than rewriting arcs. */
  compose?: boolean;
}

/** Options of `concatenate`, `conjunct` and `subtract`. */
export interface BinaryOptions {
  /** Does not expand the identity and unknown symbols by the other's symbols. */
  doNotHarmonize?: boolean;
  /** Harmonizes flag diacritics. */
  harmonizeFlags?: boolean;
  /** Refuses transducers of two types rather than converting one, as the tool would. */
  doNotConvert?: boolean;
}

export interface ComposeOptions extends BinaryOptions {
  /** Flag diacritics are ordinary symbols in composition. */
  xeroxComposition?: boolean;
  /** A one-sided flag diacritic is an epsilon in composition. */
  flagIsEpsilon?: boolean;
}

export interface DisjunctOptions {
  doNotHarmonize?: boolean;
  doNotConvert?: boolean;
}

/** Options of `priorityDisjunct` and `shuffle`. */
export interface ConversionOptions {
  doNotConvert?: boolean;
}

export interface ComposeIntersectOptions {
  invert?: boolean;
  fast?: boolean;
  encodeWeights?: boolean;
  harmonize?: boolean;
  /** The name the result takes the lexicon by when it has none, as the tool takes its file's. */
  lexiconName?: string;
  /** The name the result takes the rules by, as the tool takes their file's. */
  rulesName?: string;
  doNotConvert?: boolean;
}

let wrap: (core: Core<CompileGlue>, glue: CompileTransducerGlue) => Transducer;

/** Wraps a transducer the glue hands back, as the compilers do. */
export let wrapTransducer: (core: Core<CompileGlue>, glue: CompileTransducerGlue) => Transducer;

// [spec:hfst:req:javascript.compile-module]
/**
 * A transducer of any type the hfst library reads: tropical OpenFst, foma,
 * and optimized lookup, weighted and unweighted. Make one with
 * `readTransducers` or a compiler.
 *
 * Its operations change it in place and return it, as `Array.prototype.sort`
 * does, so they chain: `t.invert().minimize()`. `copy()` gives one of its own
 * to change. Its queries return plain objects, and its writers bytes or
 * text, as the hfst tool each is named after writes them. A transducer
 * crosses to `@divvun/hfst` as the bytes `toBytes` writes, which its readers
 * read.
 *
 * It owns module memory: call `free()` when done with it, or declare it with
 * `using`.
 */
export class Transducer extends TransducerBase {
  private constructor(core: Core<CompileGlue>, glue: CompileTransducerGlue) {
    super(core, glue);
  }

  static {
    wrap = (core, glue) => new Transducer(core, glue);
    wrapTransducer = wrap;
  }

  /** A transducer of its own with the same contents, which changes independently of this one. */
  copy(): Transducer {
    return wrap(this.compileCore, this.use((glue) => glue.copy()));
  }

  /**
   * Converts the transducer to `type`, as `hfst fst2fst --format` converts
   * it; `quick` packs optimized-lookup tables less tightly, as `--quick`.
   */
  convert(type: TransducerType, options?: ConvertOptions): this {
    if (typeof type !== "string") throw invalid("convert takes the type to convert to as a string");
    const { quick = false, ...rest } = given("convert", options) ?? {};
    const [unknown] = Object.keys(rest);
    if (unknown !== undefined) throw invalid(`convert has no option called ${unknown}; its options are quick`);
    if (typeof quick !== "boolean") throw invalid("convert's option quick is a boolean");
    this.use((glue) => glue.convert(type, quick));
    return this;
  }

  #change(f: (glue: CompileTransducerGlue) => void): this {
    this.use(f);
    return this;
  }

  /** Determinizes the transducer, as `hfst determinize`. */
  determinize(options?: DeterminizeOptions): this {
    return this.#change((glue) => glue.determinize(given("determinize", options)));
  }

  /** Minimizes the transducer, as `hfst minimize`. */
  minimize(options?: DeterminizeOptions): this {
    return this.#change((glue) => glue.minimize(given("minimize", options)));
  }

  /** Swaps the input and output sides, as `hfst invert`. */
  invert(): this {
    return this.#change((glue) => glue.invert());
  }

  /** Reverses every path, as `hfst reverse`. */
  reverse(): this {
    return this.#change((glue) => glue.reverse());
  }

  /** Removes the epsilon arcs, as `hfst remove-epsilons`. */
  removeEpsilons(): this {
    return this.#change((glue) => glue.removeEpsilons());
  }

  /** Drops the symbols no arc uses from the alphabet, as `hfst prune-alphabet`. */
  pruneAlphabet(options?: PruneAlphabetOptions): this {
    return this.#change((glue) => glue.pruneAlphabet(given("pruneAlphabet", options)));
  }

  /** Makes the transducer the identity on one side's language, as `hfst project`. */
  project(options?: ProjectOptions): this {
    return this.#change((glue) => glue.project(given("project", options)));
  }

  /** Pushes the weights towards the initial state or the final states, as `hfst push-weights`. */
  pushWeights(options?: PushOptions): this {
    return this.#change((glue) => glue.pushWeights(given("pushWeights", options)));
  }

  /** Pushes the labels towards the initial state or the final states, as `hfst push-labels`. */
  pushLabels(options?: PushOptions): this {
    return this.#change((glue) => glue.pushLabels(given("pushLabels", options)));
  }

  /** Realigns the symbols of each path, as `hfst realign`. */
  realign(): this {
    return this.#change((glue) => glue.realign());
  }

  /** Makes the transducer repeat from `from` to `to` times, as `hfst repeat`. */
  repeat(options?: RepeatOptions): this {
    return this.#change((glue) => glue.repeat(given("repeat", options)));
  }

  /** Inserts `symbolPair` freely anywhere along each path, as `hfst insert-freely`. */
  insertFreely(options: InsertFreelyOptions): this {
    return this.#change((glue) => glue.insertFreely(given("insertFreely", options)));
  }

  /** Compiles flag diacritics into the transducer's paths, as `hfst eliminate-flags`. */
  eliminateFlags(options?: EliminateFlagsOptions): this {
    return this.#change((glue) => glue.eliminateFlags(given("eliminateFlags", options)));
  }

  /** Prepares the transducer for conversion to optimized lookup, as `hfst preprocess-for-optimized-lookup-format`. */
  preprocessForOptimizedLookupFormat(): this {
    return this.#change((glue) => glue.preprocessForOptimizedLookupFormat());
  }

  /** Makes the transducer a guesser of unknown words by their affixes, as `hfst affix-guessify`. */
  affixGuessify(options?: AffixGuessifyOptions): this {
    return this.#change((glue) => glue.affixGuessify(given("affixGuessify", options)));
  }

  /** Lets each symbol stand for the symbols it is equivalent to, as `hfst expand-equivalences`. */
  expandEquivalences(options?: ExpandEquivalencesOptions): this {
    return this.#change((glue) => glue.expandEquivalences(given("expandEquivalences", options)));
  }

  /** Removes every arc carrying `symbol`, or any symbol `tsv` lists, as `hfst kill-paths`. */
  killPaths(options?: KillPathsOptions): this {
    return this.#change((glue) => glue.killPaths(given("killPaths", options)));
  }

  /** Changes the weights, as `hfst reweight`. */
  reweight(options?: ReweightOptions): this {
    return this.#change((glue) => glue.reweight(given("reweight", options)));
  }

  /**
   * Sets the transducer's name, as `hfst name --name`. The tool is `name`,
   * which is the accessor that reads the name.
   */
  setName(name: string, options?: SetNameOptions): this {
    if (typeof name !== "string") throw invalid("setName takes the name as a string");
    return this.#change((glue) => glue.setName(name, given("setName", options)));
  }

  /** Adds or replaces each property of `add`, as `hfst edit-metadata --add NAME=VALUE`. */
  editMetadata(options: EditMetadataOptions): this {
    return this.#change((glue) => glue.editMetadata(given("editMetadata", options)));
  }

  /**
   * Makes the transducer, a morphological analyser, a guesser of the
   * analyses of unknown words, as `hfst guessify`: weighted optimized lookup
   * that `guess` takes. Unlike the other operations it returns the model
   * form generator the tool writes after the guesser, or `null` with
   * `doNotCompileGenerator`.
   */
  guessify(options?: GuessifyOptions): Transducer | null {
    const generator = this.use((glue) => glue.guessify(given("guessify", options)));
    return generator === undefined ? null : wrap(this.compileCore, generator);
  }

  /**
   * Replaces labels, as `hfst substitute`. A transducer substituted into
   * itself is read as it was before.
   */
  substitute(options: SubstituteOptions): this {
    const { toTransducer, ...rest } = given("substitute", options) ?? {};
    if (toTransducer === undefined) return this.#change((glue) => glue.substitute(rest));
    return this.withOther("substitute", toTransducer, (glue, to) => {
      glue.substituteTransducer(to, rest);
      return this;
    });
  }

  /**
   * Composes `other` after the transducer, as `hfst compose`. Transducers of
   * two algebra types are converted to one as the tool converts them.
   * `t.compose(t)` composes the transducer with itself as it was.
   */
  compose(other: Transducer, options?: ComposeOptions): this {
    return this.withOther("compose", other, (glue, theirs) => {
      glue.compose(theirs, given("compose", options));
      return this;
    });
  }

  /** Concatenates `other` after the transducer, as `hfst concatenate`. */
  concatenate(other: Transducer, options?: BinaryOptions): this {
    return this.withOther("concatenate", other, (glue, theirs) => {
      glue.concatenate(theirs, given("concatenate", options));
      return this;
    });
  }

  /** Intersects the transducer with `other`, as `hfst conjunct`. */
  conjunct(other: Transducer, options?: BinaryOptions): this {
    return this.withOther("conjunct", other, (glue, theirs) => {
      glue.conjunct(theirs, given("conjunct", options));
      return this;
    });
  }

  /** Subtracts `other` from the transducer, as `hfst subtract`. */
  subtract(other: Transducer, options?: BinaryOptions): this {
    return this.withOther("subtract", other, (glue, theirs) => {
      glue.subtract(theirs, given("subtract", options));
      return this;
    });
  }

  /** Unites the transducer with `other`, as `hfst disjunct`. */
  disjunct(other: Transducer, options?: DisjunctOptions): this {
    return this.withOther("disjunct", other, (glue, theirs) => {
      glue.disjunct(theirs, given("disjunct", options));
      return this;
    });
  }

  /** Unites the transducer with the paths of `other` whose input it does not read, as `hfst priority-disjunct`. */
  priorityDisjunct(other: Transducer, options?: ConversionOptions): this {
    return this.withOther("priorityDisjunct", other, (glue, theirs) => {
      glue.priorityDisjunct(theirs, given("priorityDisjunct", options));
      return this;
    });
  }

  /** Shuffles the transducer with `other`, as `hfst shuffle`. */
  shuffle(other: Transducer, options?: ConversionOptions): this {
    return this.withOther("shuffle", other, (glue, theirs) => {
      glue.shuffle(theirs, given("shuffle", options));
      return this;
    });
  }

  /**
   * Composes the transducer, a lexicon, with the intersection of `rules`, as
   * `hfst compose-intersect`. The result is named after the lexicon, or
   * `lexiconName` when it has none, and `rulesName`.
   */
  composeIntersect(rules: readonly Transducer[], options?: ComposeIntersectOptions): this {
    const checked = given("composeIntersect", options);
    withList(this.compileCore, "composeIntersect", rules, (list) => {
      this.use((glue) => glue.composeIntersect(list, checked));
    });
    return this;
  }

}

export interface TransducerReaders {
  /**
   * Every transducer in an HFST transducer stream, in stream order, of any
   * type the hfst library reads, with an HFST3 header or without one: what
   * `toBytes` writes here or in the tools, and the optimized lookup
   * `@divvun/hfst` reads. A pmatch container reads as TOP followed by its
   * definitions, each under its name. Bytes that are not a transducer
   * stream throw `invalid-format`, and a format the library does not read
   * `unsupported-format`.
   */
  readTransducers(bytes: Uint8Array): Transducer[];
  /**
   * The one transducer in a stream. A stream holding several throws
   * `transducer-count`, and frees what it read.
   */
  readTransducer(bytes: Uint8Array): Transducer;
}

/** {@link TransducerReaders} that read into the module instance `core`. */
export function transducerReaders(core: Core<CompileGlue>): TransducerReaders {
  const readTransducers = (bytes: Uint8Array): Transducer[] => {
    if (!(bytes instanceof Uint8Array)) {
      throw invalid("transducers read from the stream's bytes, as a Uint8Array");
    }
    return core.call((glue) => glue.readTransducers(bytes).map((transducer) => wrap(core, transducer)));
  };
  return {
    readTransducers,
    readTransducer: (bytes) => {
      const all = readTransducers(bytes);
      const [first] = all;
      if (first !== undefined && all.length === 1) return first;
      for (const transducer of all) transducer.free();
      throw new HfstError(
        "transducer-count",
        `readTransducer takes exactly one transducer, and the stream holds ${all.length}`,
      );
    },
  };
}

