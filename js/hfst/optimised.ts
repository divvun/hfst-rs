import type { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { OptimisedTransducerGlue } from "./glue.ts";
import { Handle } from "./handle.ts";
import { decodeResults, type LookupResult, lookupArgs } from "./lookup.ts";
import {
  isTransducerType,
  type TransducerProperties,
  type TransducerType,
  transducerProperties,
} from "./transducer.ts";

// [spec:hfst:req:javascript.optimised-lookup+2]
/** Options of one optimised lookup. It has no time limit: the search always runs to its end. */
export interface OptimisedLookupOptions {
  /**
   * The most results to return, an integer 0 or more. Every result is found
   * first, so these are the lightest, in the order the lookup returns them.
   * Leave it out for every result.
   */
  limit?: number;
}

let wrap: (core: Core, glue: OptimisedTransducerGlue) => OptimisedTransducer;

// [spec:hfst:req:javascript.optimised-lookup+2]
/**
 * An optimized-lookup transducer loaded for the optimised lookup, which is
 * divvunspell's: the `divvun-fst` crate reads the transducer and looks words
 * up in it, and its results are what divvunspell gives for the same
 * transducer and word. A {@link Transducer} looks words up with the hfst
 * library, and its results are what `hfst lookup` prints.
 *
 * Use this one where results must agree with divvunspell, as in a speller's
 * analyses or a page beside `@divvun/fst`, and {@link Transducer} where they
 * must agree with hfst. For most words in most transducers the two return the
 * same analyses at the same weights. Both read a word by longest match: at
 * each point the longest input symbol the rest of the word begins with, fixed
 * before the search, so a longer symbol that leads nowhere is not tried again
 * as shorter ones. They differ in these ways:
 *
 * - Unicode normalisation. hfst lookup also reads a symbol spelt in the other
 *   normalisation form, such as `u` and a combining acute for a transducer's
 *   `ú`. The optimised lookup reads a symbol only from its own code points.
 * - Text outside the alphabet. hfst lookup reads each character no symbol
 *   matches as a symbol of its own, which identity (`?`) arcs read and write
 *   back. The optimised lookup reads such text an extended grapheme cluster at
 *   a time as the transducer's `@_UNKNOWN_SYMBOL_@`, which only arcs on that
 *   symbol read; identity arcs never do. In a transducer with no unknown
 *   symbol, such a word has no results.
 * - Cycles of arcs that read no input, epsilons and flag diacritics. A path
 *   that comes back to a state at the same point of the input with the same
 *   flag values is not followed further by the optimised lookup, so it never
 *   goes round such a cycle. hfst lookup can return paths that go once round
 *   it, at the weight the cycle adds.
 * - Duplicates. hfst lookup returns a result for each path, so two paths that
 *   differ only in flag diacritics or epsilons give the same result twice.
 *   The optimised lookup returns each output at each weight once.
 * - Limits. hfst lookup's `limit` stops the search once it has found that
 *   many paths; the optimised lookup's returns the lightest results. The
 *   optimised lookup has no time limit.
 *
 * Its type, name and properties are read once, when it loads; reading them
 * after `free()` throws `freed`, as any other use does. It owns module
 * memory: call `free()` when done with it, or declare it with `using`.
 */
export class OptimisedTransducer extends Handle<OptimisedTransducerGlue> {
  readonly #type: TransducerType;
  readonly #name: string;
  readonly #properties: TransducerProperties;

  private constructor(core: Core, glue: OptimisedTransducerGlue) {
    super(core, glue);
    const type = this.use((g) => g.format());
    if (!isTransducerType(type)) {
      throw new HfstError("internal", `the module sent a transducer of the unknown type ${type}`);
    }
    this.#type = type;
    this.#name = this.use((g) => g.name());
    this.#properties = transducerProperties(this.use((g) => g.propertyPairs()));
  }

  static {
    wrap = (core, glue) => new OptimisedTransducer(core, glue);
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
   * The properties of its HFST3 header, as {@link Transducer.properties} has
   * them: without `name`, `type` and `version`. A transducer with no header
   * has none.
   */
  get properties(): TransducerProperties {
    return this.use(() => this.#properties);
  }

  // [spec:hfst:req:javascript.optimised-lookup+2]
  /**
   * What divvunspell's lookup returns for `input`: every output of a path
   * that reads it, with the path's weight, final weight included, ordered by
   * weight, lightest first, then by output in code point order. Each output
   * comes once at each weight it has. Flag diacritics are obeyed as
   * divvunspell's speller obeys them, and never read from the input. Input
   * with no analyses gives an empty array. The search keeps its tables from
   * one lookup to the next, so it does not allocate them again.
   */
  lookup(input: string, options?: OptimisedLookupOptions): LookupResult[] {
    const [text, limit, timeLimitMs] = lookupArgs(input, options);
    if (timeLimitMs !== undefined) {
      throw new HfstError("invalid-argument", "the optimised lookup takes no time limit, only a limit on results");
    }
    return decodeResults(this.use((g) => g.lookup(text, limit)));
  }
}

export interface OptimisedTransducerReader {
  // [spec:hfst:req:javascript.optimised-lookup+2]
  /**
   * The one optimized-lookup transducer in `bytes`, weighted or unweighted,
   * with or without an HFST3 header, loaded for the optimised lookup. Bytes
   * are copied into the module once and not kept. Bytes in any other format
   * throw `unsupported-format`, naming it, and bytes that are not a
   * transducer stream, or end partway through one, throw `invalid-format`, as
   * `readTransducers` does. A stream of several transducers, such as a pmatch
   * container, throws `transducer-count`.
   */
  readOptimisedTransducer(bytes: Uint8Array): OptimisedTransducer;
}

/** The {@link OptimisedTransducerReader} that loads into the module instance `core`. */
export function optimisedTransducerReader(core: Core): OptimisedTransducerReader {
  return {
    readOptimisedTransducer: (bytes) => {
      if (!(bytes instanceof Uint8Array)) {
        throw new HfstError("invalid-argument", "a transducer loads from its bytes, as a Uint8Array");
      }
      return core.call((glue) => wrap(core, glue.readOptimisedTransducer(bytes)));
    },
  };
}
