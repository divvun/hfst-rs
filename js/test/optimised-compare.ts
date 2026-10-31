// The comparison of the optimised lookup with lookup on one transducer, behind
// js/test/optimised.ts. The two agree on an input when they return the same
// outputs at the same weights. Where they do not, the difference must be one
// of the kinds js/hfst/MIGRATING.md lists; any other is unclassified, and
// fails the check.
//
// Nothing here has node: types, so deno check can check it.

import type { LookupResult } from "../hfst/lookup.ts";

/**
 * The kinds of difference the check accepts.
 *
 * - `normalisation`: the input spells a symbol in the other Unicode
 *   normalisation form, which hfst lookup reads as that symbol. The optimised
 *   lookup gives lookup's results for a spelling in the other form.
 * - `outside-alphabet`: the input holds text no input symbol matches, which
 *   hfst lookup reads a character at a time and through identity arcs, and
 *   the optimised lookup reads a grapheme cluster at a time as the unknown
 *   symbol. Only a transducer with an identity or unknown symbol reads such
 *   text at all.
 * - `epsilon-cycle`: the transducer's header says it has a cycle of arcs that
 *   read no input, epsilons and flag diacritics, and lookup finds every
 *   result the optimised lookup finds and more: paths that go round such a
 *   cycle back to a state with the same flags, which the optimised lookup
 *   never follows.
 */
export type Kind = "normalisation" | "outside-alphabet" | "epsilon-cycle";

export const KINDS: readonly Kind[] = ["normalisation", "outside-alphabet", "epsilon-cycle"];

/** One input the two lookups disagree on, with both result sets as `output TAB weight` lines. */
export interface Disagreement {
  input: string;
  kind: Kind | null;
  optimised: string[];
  lookup: string[];
  /** Why the disagreement is unclassified; empty when it has a kind. */
  reason: string;
}

/** The part of a package transducer the comparison uses. */
export interface Lookups {
  lookup(input: string): LookupResult[];
}

/** What the comparison reads of an optimized-lookup transducer's header and alphabet. */
export interface OlHeader {
  symbols: string[];
  /** How many of `symbols`, numbered first, its input side uses. */
  inputCount: number;
  /** Whether the header says the transducer has a cycle of arcs that read no input. */
  inputEpsilonCycles: boolean;
}

/** Where the optimized-lookup tables start in `bytes`: after an HFST3 header, a pre-release one, or none. */
function payloadStart(bytes: Uint8Array): number {
  const begins = (magic: string) => [...magic].every((c, i) => bytes[i] === c.charCodeAt(0));
  if (begins("HFST\0")) return 8 + (bytes[5] ?? 0) + 256 * (bytes[6] ?? 0);
  if (begins("HFST3\0")) return bytes.indexOf(0, 6) + 1;
  return 0;
}

/** Bytes of an optimized-lookup header, before the alphabet. */
const OL_HEADER = 56;

/** Where the header holds its `has_input_epsilon_cycles` property: the eighth of nine after six counts. */
const INPUT_EPSILON_CYCLES = 20 + 7 * 4;

/** The header and alphabet of the optimized-lookup transducer in `bytes`, its symbols read as divvun-fst reads them. */
export function readOlHeader(bytes: Uint8Array): OlHeader {
  const start = payloadStart(bytes);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const inputCount = view.getUint16(start, true);
  const count = view.getUint16(start + 2, true);
  const inputEpsilonCycles = view.getUint32(start + INPUT_EPSILON_CYCLES, true) !== 0;
  const decoder = new TextDecoder();
  const symbols: string[] = [];
  let at = start + OL_HEADER;
  for (let i = 0; i < count; i += 1) {
    const end = bytes.indexOf(0, at);
    if (end < 0) throw new Error(`the alphabet ends inside symbol ${i}`);
    symbols.push(decoder.decode(bytes.subarray(at, end)));
    at = end + 1;
  }
  return { symbols, inputCount, inputEpsilonCycles };
}

/** A symbol divvun-fst sets apart by its spelling: epsilon, flag diacritics and the other `@...@` names. */
function isSpecial(symbol: string): boolean {
  return symbol.length > 1 && symbol.startsWith("@") && symbol.endsWith("@");
}

/**
 * Whether reading `input` by longest match over `symbols`, as divvun-fst
 * does, reaches text that none of them matches. A slice that splits a
 * surrogate pair matches no symbol, so stepping in code units finds what
 * divvun-fst's steps over character boundaries find.
 */
export function meetsUnmatchedText(input: string, symbols: ReadonlySet<string>): boolean {
  const longest = Math.max(0, ...[...symbols].map((symbol) => symbol.length));
  let at = 0;
  while (at < input.length) {
    let matched = 0;
    for (let length = Math.min(longest, input.length - at); length > 0; length -= 1) {
      if (symbols.has(input.slice(at, at + length))) {
        matched = length;
        break;
      }
    }
    if (matched === 0) return true;
    at += matched;
  }
  return false;
}

function resultSet(results: readonly LookupResult[]): Set<string> {
  return new Set(results.map(({ output, weight }) => `${output}\t${weight}`));
}

function sameSet(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
  return a.size === b.size && [...a].every((x) => b.has(x));
}

/** The other Unicode normalisations of `input`, where they differ from it. */
function respellings(input: string): string[] {
  return [...new Set([input.normalize("NFC"), input.normalize("NFD")])].filter((spelling) => spelling !== input);
}

/** Compares the optimised lookup with lookup on one transducer, loaded both ways from `bytes`. */
export class LookupComparison {
  readonly #optimised: Lookups;
  readonly #lookup: Lookups;
  /** The text the optimised lookup reads as symbols. */
  readonly #readable: Set<string>;
  /** Whether the transducer has an identity or unknown symbol, without which text outside the alphabet has no path. */
  readonly #readsOutside: boolean;
  readonly #inputEpsilonCycles: boolean;

  constructor(optimised: Lookups, lookup: Lookups, bytes: Uint8Array) {
    this.#optimised = optimised;
    this.#lookup = lookup;
    const { symbols, inputCount, inputEpsilonCycles } = readOlHeader(bytes);
    this.#inputEpsilonCycles = inputEpsilonCycles;
    this.#readable = new Set(symbols.slice(0, inputCount).filter((symbol) => symbol !== "" && !isSpecial(symbol)));
    this.#readsOutside = symbols.some((symbol) => symbol === "@_IDENTITY_SYMBOL_@" || symbol === "@_UNKNOWN_SYMBOL_@");
  }

  /** Looks every input up both ways and returns how many agree and the inputs that do not, each classified. */
  compare(inputs: readonly string[]): { agreed: number; disagreements: Disagreement[] } {
    let agreed = 0;
    const disagreements: Disagreement[] = [];
    for (const input of inputs) {
      const optimised = resultSet(this.#optimised.lookup(input));
      const lookup = resultSet(this.#lookup.lookup(input));
      if (sameSet(optimised, lookup)) {
        agreed += 1;
      } else {
        disagreements.push(this.#classify(input, optimised, lookup));
      }
    }
    return { agreed, disagreements };
  }

  #classify(input: string, optimised: Set<string>, lookup: Set<string>): Disagreement {
    const found = (kind: Kind | null, reason = ""): Disagreement => ({
      input,
      kind,
      optimised: [...optimised].sort(),
      lookup: [...lookup].sort(),
      reason,
    });
    const respelt = respellings(input).some((spelling) =>
      sameSet(resultSet(this.#optimised.lookup(spelling)), lookup),
    );
    if (respelt) return found("normalisation");
    if (meetsUnmatchedText(input, this.#readable)) {
      if (this.#readsOutside) return found("outside-alphabet");
      return found(null, "the input reaches text outside the alphabet, which neither lookup reads without an identity or unknown symbol");
    }
    if (this.#inputEpsilonCycles && [...optimised].every((result) => lookup.has(result))) {
      return found("epsilon-cycle");
    }
    return found(null, "the two read the input as the same symbols");
  }
}

/** How many of `disagreements` have each kind, and the unclassified ones. */
export function tally(disagreements: readonly Disagreement[]): {
  kinds: Partial<Record<Kind, number>>;
  unclassified: Disagreement[];
} {
  const kinds: Partial<Record<Kind, number>> = {};
  const unclassified: Disagreement[] = [];
  for (const disagreement of disagreements) {
    if (disagreement.kind === null) {
      unclassified.push(disagreement);
    } else {
      kinds[disagreement.kind] = (kinds[disagreement.kind] ?? 0) + 1;
    }
  }
  return { kinds, unclassified };
}

/** A disagreement as one line of test output. */
export function describe(disagreement: Disagreement): string {
  const { input, kind, optimised, lookup, reason } = disagreement;
  return `${JSON.stringify(input)} ${kind ?? `unclassified (${reason})`}: optimised ${JSON.stringify(optimised)}, lookup ${JSON.stringify(lookup)}`;
}
