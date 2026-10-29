// The parity scenario: the package against the native hfst library on the
// shared fixtures. js/test/parity-fixtures.mjs writes parity.json at test-build
// time from the native library and this checkout's hfst tokenise; this module
// starts the package as it ships, runs every fixture in that file through it
// and compares each result with the native one, value for value. It is written
// once for every environment: parity-runtime.ts runs it under Node, Bun and
// Deno, and parity-browser.ts in a browser, on a page and in a Web Worker.

import type * as Hfst from "../hfst/index.ts";
import type {
  AcceptorInfo,
  DhfstType,
  FormatOptions,
  LookupResult,
  Tokeniser,
  TokeniseFormat,
  TokeniseOptions,
  Transducer,
} from "../hfst/index.ts";

type Package = typeof Hfst;

/** Where the scenario finds the package and the fixtures, as URLs the environment can read. */
export interface ParityArgs {
  module: string;
  /** URL of the directory of built fixtures, ending in a slash. */
  fixtures: string;
}

/** Reads the bytes at a URL: with fetch in a browser, from the file system elsewhere. */
export type ReadBytes = (url: string) => Promise<Uint8Array>;

/** One kind of result compared over every fixture that has it, and every difference found. */
export interface ParityCheck {
  name: string;
  compared: number;
  differences: string[];
}

/** A token as the native library gives it: the package's offsets are its own. */
interface NativeToken {
  surface: string;
  analysed: boolean;
  readings: { analysis: string; weight: number }[];
}

/** What parity.json holds: each native result beside the call that gave it. */
interface Expected {
  lookup: { file: string; member: number; input: string; limit: number | null; results: LookupResult[] }[];
  tokens: { file: string; name: string; text: string; options: TokeniseOptions; tokens: NativeToken[] }[];
  formats: {
    file: string;
    name: string;
    text: string;
    format: TokeniseFormat;
    options: FormatOptions;
    printed: string;
  }[];
  acceptors: { file: string; word: string; weight: number | null }[];
  models: {
    file: string;
    type: DhfstType;
    version: number;
    symbols: string[];
    meta: string | null;
    info?: AcceptorInfo;
    stateCount?: number;
  }[];
}

function describe(value: unknown): string {
  const text = JSON.stringify(value) ?? String(value);
  return text.length > 200 ? `${text.slice(0, 200)}...` : text;
}

// [spec:hfst:req:javascript.native-parity+2]
// The package's results and the native ones compare exactly: the same keys,
// the same strings, the same numbers. Both sides carry each f32 weight as the
// JavaScript number that holds it exactly, so no tolerance is needed, and
// any would hide a weight the module computed differently.
/** Where `actual` first differs from `expected`, or null when they are the same. */
function difference(actual: unknown, expected: unknown, at: string): string | null {
  if (typeof expected === "string" && typeof actual === "string") {
    if (actual === expected) return null;
    let i = 0;
    while (i < actual.length && actual[i] === expected[i]) i += 1;
    const around = (s: string) => JSON.stringify(s.slice(Math.max(0, i - 30), i + 50));
    return `${at} differs at code unit ${i}: ${around(actual)}, native ${around(expected)}`;
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) return `${at} is ${describe(actual)}, native ${describe(expected)}`;
    if (actual.length !== expected.length) {
      return `${at} has ${actual.length} entries, native ${expected.length}: ${describe(actual)}, native ${describe(expected)}`;
    }
    for (let i = 0; i < expected.length; i += 1) {
      const found = difference(actual[i], expected[i], `${at}[${i}]`);
      if (found !== null) return found;
    }
    return null;
  }
  if (typeof expected === "object" && expected !== null) {
    if (typeof actual !== "object" || actual === null || Array.isArray(actual)) {
      return `${at} is ${describe(actual)}, native ${describe(expected)}`;
    }
    const keys = Object.keys(expected).sort();
    const actualKeys = Object.keys(actual).sort();
    if (keys.join() !== actualKeys.join()) return `${at} has keys ${actualKeys.join()}, native ${keys.join()}`;
    for (const key of keys) {
      const found = difference(Reflect.get(actual, key), Reflect.get(expected, key), `${at}.${key}`);
      if (found !== null) return found;
    }
    return null;
  }
  return Object.is(actual, expected) ? null : `${at} is ${describe(actual)}, native ${describe(expected)}`;
}

class Check implements ParityCheck {
  readonly name: string;
  compared = 0;
  readonly differences: string[] = [];

  constructor(name: string) {
    this.name = name;
  }

  compare(what: string, actual: unknown, expected: unknown): void {
    this.compared += 1;
    const found = difference(actual, expected, "result");
    if (found !== null) this.differences.push(`${what}: ${found}`);
  }

  /** Runs `body`, counting anything it throws as a difference. */
  async run(body: () => Promise<void>): Promise<Check> {
    try {
      await body();
    } catch (error) {
      this.differences.push(`threw ${error instanceof Error ? (error.stack ?? error.message) : String(error)}`);
    }
    return this;
  }
}

/** The objects `make` loads from each fixture, once each, freed when `body` is done. */
async function withLoaded<T extends { free(): void }>(
  make: (file: string) => Promise<T[]>,
  body: (load: (file: string) => Promise<T[]>) => Promise<void>,
): Promise<void> {
  const loaded = new Map<string, T[]>();
  try {
    await body(async (file) => {
      const known = loaded.get(file);
      if (known !== undefined) return known;
      const made = await make(file);
      loaded.set(file, made);
      return made;
    });
  } finally {
    for (const objects of loaded.values()) for (const object of objects) object.free();
  }
}

function lookups(pkg: Package, fixture: (name: string) => Promise<Uint8Array>, expected: Expected): Promise<Check> {
  const check = new Check("lookup in every optimized-lookup transducer, member by member, with and without a limit");
  return check.run(() =>
    withLoaded<Transducer>(
      async (file) => pkg.readTransducers(await fixture(file)),
      async (load) => {
        const members = new Map<string, number>();
        for (const { file, member } of expected.lookup) {
          members.set(file, Math.max(members.get(file) ?? 0, member + 1));
        }
        for (const [file, count] of members) {
          check.compare(`${file}: transducers in the stream`, (await load(file)).length, count);
        }
        for (const { file, member, input, limit, results } of expected.lookup) {
          const transducer = (await load(file))[member];
          const what = `${file} #${member} ${JSON.stringify(input)} limit ${limit}`;
          if (transducer === undefined) {
            check.compare(what, undefined, results);
            continue;
          }
          check.compare(what, transducer.lookup(input, limit === null ? {} : { limit }), results);
        }
      },
    ),
  );
}

function tokeniser(pkg: Package, fixture: (name: string) => Promise<Uint8Array>) {
  return async (file: string): Promise<Tokeniser[]> => [pkg.readTokeniser(await fixture(file))];
}

function tokens(pkg: Package, fixture: (name: string) => Promise<Uint8Array>, expected: Expected): Promise<Check> {
  const check = new Check("tokens and their weighted readings, under each tokeniser option");
  return check.run(() =>
    withLoaded(tokeniser(pkg, fixture), async (load) => {
      for (const { file, name, text, options, tokens } of expected.tokens) {
        const [loaded] = await load(file);
        const found = loaded?.tokenise(text, options) ?? [];
        const withoutOffsets = found.map(({ surface, analysed, readings }) => ({ surface, analysed, readings }));
        check.compare(`${file} ${name}`, withoutOffsets, tokens);
      }
    }),
  );
}

function formats(pkg: Package, fixture: (name: string) => Promise<Uint8Array>, expected: Expected): Promise<Check> {
  const check = new Check("tokenise output in every format, as hfst tokenise prints it");
  return check.run(() =>
    withLoaded(tokeniser(pkg, fixture), async (load) => {
      for (const { file, name, text, format, options, printed } of expected.formats) {
        const [loaded] = await load(file);
        check.compare(`${file} ${name}`, loaded?.format(text, format, options), printed);
      }
    }),
  );
}

function acceptors(pkg: Package, fixture: (name: string) => Promise<Uint8Array>, expected: Expected): Promise<Check> {
  const check = new Check("DHFST acceptor lookup of every word");
  return check.run(() =>
    withLoaded(
      async (file) => [pkg.readDhfst(await fixture(file))],
      async (load) => {
        for (const { file, word, weight } of expected.acceptors) {
          const [model] = await load(file);
          const found = model?.type === "acceptor" ? model.lookup(word) : `a ${model?.type}`;
          check.compare(`${file} ${JSON.stringify(word)}`, found, weight);
        }
      },
    ),
  );
}

function models(pkg: Package, fixture: (name: string) => Promise<Uint8Array>, expected: Expected): Promise<Check> {
  const check = new Check("what each DHFST file says about itself");
  return check.run(async () => {
    for (const native of expected.models) {
      const model = pkg.readDhfst(await fixture(native.file));
      try {
        const common = {
          file: native.file,
          type: model.type,
          version: model.version,
          symbols: [...model.symbols],
          meta: model.meta,
        };
        const described =
          model.type === "acceptor"
            ? { ...common, info: { ...model.info } }
            : { ...common, stateCount: model.stateCount };
        check.compare(native.file, described, native);
      } finally {
        model.free();
      }
    }
  });
}

// [spec:hfst:req:javascript.native-parity+2/test]
/**
 * Starts the package at `args.module` from the `.wasm` file beside it, as
 * a caller who passes init nothing does, and compares its results for every
 * fixture in parity.json with the native ones. Throws only when the package
 * or parity.json cannot be loaded; a result that differs, or a call that
 * throws, is a difference in the check it belongs to.
 */
export async function runParity(args: ParityArgs, read: ReadBytes): Promise<ParityCheck[]> {
  const pkg: Package = await import(args.module);
  await pkg.init();
  const fixture = (name: string) => read(new URL(name, args.fixtures).href);
  const expected: Expected = JSON.parse(new TextDecoder().decode(await fixture("parity.json")));
  return [
    await lookups(pkg, fixture, expected),
    await tokens(pkg, fixture, expected),
    await formats(pkg, fixture, expected),
    await acceptors(pkg, fixture, expected),
    await models(pkg, fixture, expected),
  ];
}

/** The most differences a failed check prints. */
const SHOWN = 20;

/**
 * The lines that report `checks` under `label`, and whether every check
 * passed. A check that compared nothing fails: the native expectations are
 * missing, not met.
 */
export function report(label: string, checks: ParityCheck[]): { lines: string[]; passed: boolean } {
  const lines: string[] = [];
  let passed = true;
  for (const { name, compared, differences } of checks) {
    if (differences.length === 0 && compared > 0) {
      lines.push(`ok - ${label}: ${name} (${compared} results the same as native)`);
      continue;
    }
    passed = false;
    lines.push(`not ok - ${label}: ${name}: ${differences.length} of ${compared} results differ from native`);
    for (const line of differences.slice(0, SHOWN)) lines.push(`  ${line}`);
    if (differences.length > SHOWN) lines.push(`  and ${differences.length - SHOWN} more`);
  }
  return { lines, passed };
}
