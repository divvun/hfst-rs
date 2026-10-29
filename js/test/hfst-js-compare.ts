// The three-way lookup comparison behind the hfst-js parity check. This
// package's lookup is compared with hfst-js's, and where the two disagree,
// with what the reference C++ hfst-lookup prints. A disagreement passes only
// when our results are the reference's and the cause is one of the kinds
// js/hfst/MIGRATING.md lists. hfst-js comes from js/bench, which pins it; the
// published package does not depend on it.
//
// Nothing here has node: types, so deno check can check it. The node: modules
// are imported through variable specifiers, as harness.ts does.

import type { LookupResult } from "../hfst/lookup.ts";

/** hfst-js's "no symbol": the end of its input tape, and every input position it could not read. */
const NO_SYMBOL = 0xffff;

/** hfst-js's number for the unification operator, `@U.feature.value@`. */
const UNIFY = 5;

/** A flag diacritic as HFST spells it. hfst-js turns the ones it reads as flags into "" in its symbol table. */
const FLAG_SYNTAX = /^@[PNRDCU]\.[^.@]+(\.[^@]+)?@$/;

// The parts of hfst-js 1.4.0 the comparison uses. Its index.d.ts declares
// them; they are restated here so that deno check needs nothing from js/bench.
interface HfstJsFlag {
  op: number;
  feature: number;
  value: number;
}

interface HfstJsMachine {
  alphabet: { keyTable: string[] };
  header: { getInputSymbolCount(): number };
}

interface HfstJsResult {
  symbols: string[];
  weight: number;
}

interface HfstJsAnalyzer {
  state: { input_string: number[]; state_stack: number[][] };
  analyze(): HfstJsResult[];
  pushState(flag: HfstJsFlag): boolean;
}

type HfstJsAnalyzerClass = new (machine: HfstJsMachine, input: string, timeCutoff?: number) => HfstJsAnalyzer;

/** An hfst-js transducer, as `HfstInputStream.read()` returns it. */
export interface HfstJsTransducer {
  tr: HfstJsMachine;
  lookup(input: string, timeCutoff?: number): [string, number][];
  lookupResults(input: string, timeCutoff?: number): HfstJsResult[];
}

export interface HfstJs {
  HfstInputStream: new (bytes: Uint8Array, cache: boolean) => { read(): HfstJsTransducer };
  Analyzer: HfstJsAnalyzerClass;
}

/** Imports hfst-js from js/bench, saying how to install it when it is not there. */
export async function loadHfstJs(): Promise<HfstJs> {
  const specifier = new URL("../bench/hfst-js.mjs", import.meta.url).href;
  try {
    const hfstJs: HfstJs = await import(specifier);
    return hfstJs;
  } catch (error) {
    throw new Error(
      `cannot import hfst-js from js/bench (${String(error)}): run npm ci in js/bench, which node js/build.mjs --test does`,
    );
  }
}

/** The part of a package `Transducer` the comparison uses. */
export interface OurLookup {
  lookup(input: string): LookupResult[];
}

/**
 * The kinds of disagreement the check accepts. The first five are hfst-js
 * departing from the reference while our results are the reference's. In the
 * last, the input spells a symbol in the other Unicode normalisation form,
 * which only we read, and our results are the reference's for the spelling
 * the transducer has.
 *
 * - `unreadable-input`: hfst-js cannot read a character or symbol, so it
 *   looks up only what comes before it.
 * - `symbol-prefix`: hfst-js cannot read input that begins with a symbol,
 *   because a longer symbol begins the same way, so it looks up only what
 *   comes before it.
 * - `short-flag`: hfst-js does not read a five-character flag, such as
 *   `@C.X@`, as a flag.
 * - `unify-after-negate`: hfst-js lets `@U.F.v@` follow `@N.F.v@`.
 * - `weight-sum`: hfst-js sums weights in 64 bits; rounded to 32 bits, its
 *   sum can still be a bit away from HFST's 32-bit running sum.
 * - `normalisation`: we read a precomposed letter and its decomposed spelling
 *   as the same symbol.
 */
export type Kind =
  | "unreadable-input"
  | "symbol-prefix"
  | "short-flag"
  | "unify-after-negate"
  | "weight-sum"
  | "normalisation";

export const KINDS: readonly Kind[] = [
  "unreadable-input",
  "symbol-prefix",
  "short-flag",
  "unify-after-negate",
  "weight-sum",
  "normalisation",
];

/** One input the libraries disagree on, with all three result sets as `output TAB weight` lines. */
export interface Disagreement {
  input: string;
  kind: Kind | null;
  ours: string[];
  hfstJs: string[];
  reference: string[];
  /** Why the disagreement is unclassified; empty when it has a kind. */
  reason: string;
}

/** The C++ hfst-lookup results for inputs, one set of `output TAB weight` lines per input. */
export interface Reference {
  lookup(inputs: readonly string[]): Promise<Set<string>[]>;
}

function sameSet(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
  return a.size === b.size && [...a].every((x) => b.has(x));
}

function sorted(set: ReadonlySet<string>): string[] {
  return [...set].sort();
}

/** Our results as a set, weights exact. */
function oursExact(results: readonly LookupResult[]): Set<string> {
  return new Set(results.map(({ symbols, weight }) => `${symbols.join("")}\t${weight}`));
}

/** hfst-js results as a set, weights rounded to the 32 bits HFST weights have. */
function hfstJsSet(pairs: readonly (readonly [string, number])[]): Set<string> {
  return new Set(pairs.map(([analysis, weight]) => `${analysis}\t${Math.fround(weight)}`));
}

/**
 * `weight` as C's `%f` prints it, which is how hfst-lookup prints weights:
 * six places, an exact tie rounded to even. A tie at six places has at most
 * seven binary fraction digits, so its seven-place form is exact.
 */
export function printedWeight(weight: number): string {
  const seven = weight.toFixed(7);
  if (!seven.endsWith("5") || Number(seven) !== weight) return weight.toFixed(6);
  const down = seven.slice(0, -1);
  const lastDigit = Number(down.at(-1));
  if (lastDigit % 2 === 0) return down;
  return (Math.sign(weight) * (Math.abs(Number(down)) + 1e-6)).toFixed(6);
}

/** Our results as hfst-lookup would print them. */
function oursPrinted(results: readonly LookupResult[]): Set<string> {
  return new Set(results.map(({ output, weight }) => `${output}\t${printedWeight(weight)}`));
}

/**
 * Whether the same analyses come back with weights a rounding apart: each
 * weight an analysis has on one side is near one it has on the other. Paths
 * that differ only in where an epsilon falls sum the same weights in another
 * order, so one side can have two weights a rounding apart where the other
 * has one. 1e-5 of the weight is far more than 32-bit sums of a few weights
 * drift, and far less than any weight an analyser gives on purpose.
 */
function weightsApart(ours: ReadonlySet<string>, theirs: ReadonlySet<string>): boolean {
  const byAnalysis = (set: ReadonlySet<string>) => {
    const map = new Map<string, number[]>();
    for (const line of set) {
      const tab = line.lastIndexOf("\t");
      const analysis = line.slice(0, tab);
      map.set(analysis, [...(map.get(analysis) ?? []), Number(line.slice(tab + 1))]);
    }
    return map;
  };
  const near = (weight: number, others: readonly number[]) =>
    others.some((other) => Math.abs(weight - other) <= 1e-5 * Math.max(1, Math.abs(weight)));
  const a = byAnalysis(ours);
  const b = byAnalysis(theirs);
  if (a.size !== b.size) return false;
  for (const [analysis, weights] of a) {
    const other = b.get(analysis);
    if (other === undefined) return false;
    if (!weights.every((weight) => near(weight, other)) || !other.every((weight) => near(weight, weights))) return false;
  }
  return true;
}

/** hfst-js's analyser with HFST's unification: `@U.F.v@` fails when F was negatively set to v. */
function unifyCorrected(Analyzer: HfstJsAnalyzerClass): HfstJsAnalyzerClass {
  return class extends Analyzer {
    override pushState(flag: HfstJsFlag): boolean {
      if (flag.op !== UNIFY) return super.pushState(flag);
      const stack = this.state.state_stack;
      const top = stack.at(-1) ?? [];
      const current = top[flag.feature] ?? 0;
      if (current !== 0 && current !== flag.value && !(current < 0 && -current !== flag.value)) return false;
      const next = top.slice();
      next[flag.feature] = flag.value;
      stack.push(next);
      return true;
    }
  };
}

// [spec:hfst:req:javascript.hfst-js-parity+2]
/**
 * Compares lookups on one transducer, loaded both into this package and into
 * hfst-js (with hfst-js's result cache off).
 */
export class ParityCheck {
  readonly #ours: OurLookup;
  readonly #hfstJs: HfstJsTransducer;
  readonly #analyzer: HfstJsAnalyzerClass;
  readonly #unifyCorrected: HfstJsAnalyzerClass;
  readonly #reference: Reference;
  /** The spellings of the transducer's input symbols, for telling which input positions a symbol begins at. */
  readonly #inputSymbols: string[];
  /** Whether the transducer has flags hfst-js reads as ordinary symbols. */
  readonly #shortFlags: boolean;

  constructor(hfstJs: HfstJs, bytes: Uint8Array, ours: OurLookup, reference: Reference) {
    this.#ours = ours;
    this.#hfstJs = new hfstJs.HfstInputStream(bytes, false).read();
    this.#analyzer = hfstJs.Analyzer;
    this.#unifyCorrected = unifyCorrected(hfstJs.Analyzer);
    this.#reference = reference;
    const keyTable = this.#hfstJs.tr.alphabet.keyTable;
    this.#inputSymbols = keyTable.slice(0, this.#hfstJs.tr.header.getInputSymbolCount()).filter((s) => s !== "");
    this.#shortFlags = keyTable.some((symbol) => FLAG_SYNTAX.test(symbol));
  }

  /**
   * Looks every input up in both libraries and returns the inputs they
   * disagree on, each with its kind, or with a null kind and the reason when
   * the disagreement is not one the check accepts.
   */
  async compare(inputs: readonly string[]): Promise<{ agreed: number; disagreements: Disagreement[] }> {
    const pending: { input: string; ours: LookupResult[]; theirs: Set<string> }[] = [];
    let agreed = 0;
    for (const input of inputs) {
      const ours = this.#ours.lookup(input);
      const theirs = hfstJsSet(this.#hfstJs.lookup(input));
      if (sameSet(oursExact(ours), theirs)) {
        agreed += 1;
      } else {
        pending.push({ input, ours, theirs });
      }
    }
    const asked = [...new Set(pending.flatMap(({ input }) => [input, ...normalisedSpellings(input)]))];
    const answers = await this.#reference.lookup(asked);
    const reference = new Map(asked.map((input, i) => [input, answers[i] ?? new Set<string>()]));
    const disagreements = pending.map(({ input, ours, theirs }) =>
      this.#classify(input, ours, theirs, reference),
    );
    return { agreed, disagreements };
  }

  #classify(
    input: string,
    ours: LookupResult[],
    theirs: Set<string>,
    reference: ReadonlyMap<string, Set<string>>,
  ): Disagreement {
    const printed = oursPrinted(ours);
    const expected = reference.get(input) ?? new Set<string>();
    const found = (kind: Kind | null, reason = ""): Disagreement => ({
      input,
      kind,
      ours: sorted(oursExact(ours)),
      hfstJs: sorted(theirs),
      reference: sorted(expected),
      reason,
    });
    if (!sameSet(printed, expected)) {
      const respelt = normalisedSpellings(input).some((spelling) => {
        const other = reference.get(spelling);
        return other !== undefined && sameSet(printed, other);
      });
      if (respelt) return found("normalisation");
      return found(null, "our results are not the reference's");
    }
    const kind = this.#hfstJsKind(input, ours, theirs);
    return kind === null ? found(null, "no known hfst-js defect explains it") : found(kind);
  }

  /** Which hfst-js defect makes it depart from the reference for `input`, if a known one does. */
  #hfstJsKind(input: string, ours: LookupResult[], theirs: Set<string>): Kind | null {
    const tape = new this.#analyzer(this.#hfstJs.tr, input).state.input_string;
    const stop = tape.indexOf(NO_SYMBOL);
    if (stop < tape.length - 1) {
      // hfst-js treats the first position it cannot read as the end of the
      // input. Its results must then be those of the input before it.
      const keyTable = this.#hfstJs.tr.alphabet.keyTable;
      const prefix = tape
        .slice(0, stop)
        .map((symbol) => keyTable[symbol] ?? "")
        .join("");
      if (!sameSet(theirs, hfstJsSet(this.#hfstJs.lookup(prefix)))) return null;
      const symbolBegins = this.#inputSymbols.some((symbol) => input.startsWith(symbol, prefix.length));
      return symbolBegins ? "symbol-prefix" : "unreadable-input";
    }
    if (weightsApart(oursExact(ours), theirs)) return "weight-sum";
    const corrected = new this.#unifyCorrected(this.#hfstJs.tr, input).analyze();
    const correctedSet = new Set(corrected.map(({ symbols, weight }) => `${symbols.join("")}\t${Math.fround(weight)}`));
    if (sameSet(oursExact(ours), correctedSet)) return "unify-after-negate";
    if (this.#shortFlags) return "short-flag";
    return null;
  }

  /** Whether both libraries return the same set of results for `input`. */
  agrees(input: string): boolean {
    return sameSet(oursExact(this.#ours.lookup(input)), hfstJsSet(this.#hfstJs.lookup(input)));
  }

  /**
   * Whether, for `input`, the symbols hfst-js reports, with the empty
   * strings it gives flags and epsilons left out, are the symbols we report:
   * which makes joining ours the same as its joined analysis.
   */
  symbolsAgree(input: string): boolean {
    const key = (symbols: readonly string[], weight: number) => `${JSON.stringify(symbols)}\t${weight}`;
    const ours = new Set(this.#ours.lookup(input).map(({ symbols, weight }) => key(symbols, weight)));
    const theirs = new Set(
      this.#hfstJs
        .lookupResults(input)
        .map(({ symbols, weight }) => key(symbols.filter((symbol) => symbol !== ""), Math.fround(weight))),
    );
    return sameSet(ours, theirs);
  }
}

/** The other Unicode normalisations of `input`, where they differ from it. */
function normalisedSpellings(input: string): string[] {
  return [...new Set([input.normalize("NFC"), input.normalize("NFD")])].filter((spelling) => spelling !== input);
}

interface ChildProcess {
  execFileSync(
    file: string,
    args: readonly string[],
    options: { input?: string; encoding: "utf8"; maxBuffer?: number; stdio?: readonly string[] },
  ): string;
}

interface NodeFs {
  existsSync(path: string): boolean;
}

const childProcessSpecifier = "node:child_process";
const fsSpecifier = "node:fs";

/** The reference hfst-lookup binary: `HFST_LOOKUP_REFERENCE`, or C++ HFST's usual install path. */
export function referenceBinary(): string {
  const env: unknown = Reflect.get(Reflect.get(globalThis, "process"), "env");
  const named = typeof env === "object" && env !== null ? Reflect.get(env, "HFST_LOOKUP_REFERENCE") : undefined;
  return typeof named === "string" && named !== "" ? named : "/opt/hfst/bin/hfst-lookup";
}

/** The reference binary's version line, without the directory it was run from, or null when it is not installed. */
export async function referenceVersion(binary: string): Promise<string | null> {
  const fs: NodeFs = await import(fsSpecifier);
  if (!fs.existsSync(binary)) return null;
  const childProcess: ChildProcess = await import(childProcessSpecifier);
  const text = childProcess.execFileSync(binary, ["--version"], { encoding: "utf8" });
  return (text.split("\n")[0] ?? "").replace(/^\S*\//, "");
}

/**
 * Runs C++ hfst-lookup on the transducer at `path` (a filesystem path) for
 * all inputs at once. It prints a block of `input TAB output TAB weight` lines
 * per input, closed by an empty line, and a single line weighing `inf` for an
 * input with no results.
 */
export function liveReference(binary: string, path: string): Reference {
  return {
    async lookup(inputs) {
      if (inputs.length === 0) return [];
      for (const input of inputs) {
        if (/[\n\r]/.test(input)) throw new Error(`hfst-lookup reads lines, so it cannot look up ${JSON.stringify(input)}`);
      }
      const childProcess: ChildProcess = await import(childProcessSpecifier);
      const text = childProcess.execFileSync(binary, ["-q", path], {
        input: `${inputs.join("\n")}\n`,
        encoding: "utf8",
        maxBuffer: 1 << 30,
        stdio: ["pipe", "pipe", "ignore"],
      });
      const blocks = text.split("\n\n").filter((block) => block !== "");
      if (blocks.length !== inputs.length) {
        throw new Error(`hfst-lookup printed ${blocks.length} result blocks for ${inputs.length} inputs`);
      }
      return blocks.map((block, i) => {
        const results = new Set<string>();
        for (const line of block.split("\n")) {
          const [echoed, output = "", weight = ""] = line.split("\t");
          if (echoed !== inputs[i]) throw new Error(`hfst-lookup echoed ${JSON.stringify(echoed)} for ${JSON.stringify(inputs[i])}`);
          if (weight !== "inf") results.add(`${output}\t${weight}`);
        }
        return results;
      });
    },
  };
}

/** A reference that answers from results recorded earlier, and fails on an input it has none for. */
export function recordedReference(recorded: Readonly<Record<string, readonly string[]>>, what: string): Reference {
  return {
    lookup(inputs) {
      return Promise.resolve(
        inputs.map((input) => {
          const lines = recorded[input];
          if (lines === undefined) throw new Error(`${what} has no recorded reference result for ${JSON.stringify(input)}`);
          return new Set(lines);
        }),
      );
    },
  };
}

/**
 * The inputs the check runs for a word list: the words, and non-words made
 * from some of them. Some non-words are in the transducer's letters and some
 * hold a character no analyser reads, at the end or in the middle; some are a
 * word decomposed into base letters and combining marks.
 */
export function withNonWords(words: readonly string[]): string[] {
  const made: string[] = [];
  words.forEach((word, i) => {
    const letters = Array.from(word);
    if (i % 50 === 0) made.push(letters.toReversed().join(""));
    if (i % 50 === 1) made.push(word + (letters.at(-1) ?? "").repeat(2));
    if (i % 50 === 2 && letters.length > 3) made.push([letters[0], letters[2], letters[1], ...letters.slice(3)].join(""));
    if (i % 50 === 3) made.push(`${word}☃`);
    if (i % 50 === 4) made.push(`${word}\u{1f600}${word}`);
    if (i % 50 === 5) made.push(word.normalize("NFD"));
  });
  return [...new Set([...words, ...made])];
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
  const { input, kind, ours, hfstJs, reference, reason } = disagreement;
  return `${JSON.stringify(input)} ${kind ?? `unclassified (${reason})`}: ours ${JSON.stringify(ours)}, hfst-js ${JSON.stringify(hfstJs)}, reference ${JSON.stringify(reference)}`;
}
