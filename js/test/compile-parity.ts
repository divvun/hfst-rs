// The compile parity scenario: @divvun/hfst/compile against the hfst tools,
// on what the tools write of the inputs the repository's tests give them.
// js/test/compile-fixtures.mjs builds those at test-build time into compile/
// and lists each case in compile-parity.json: its call as divvun-hfst names
// it, the tool's arguments, whether the tool succeeded, and what the tool
// wrote. divvun-hfst's suite (python/tests/test_compile_parity.py and
// test_query_parity.py) checks the same cases the same way, so the package
// giving what the tools give is the package giving what divvun-hfst gives.
//
// A compiler, operation or writer gives the tool's bytes or text, byte for
// byte; a query gives data, which is printed as the tool prints it and
// compared with what the tool printed. Where the tool failed, the package
// throws an HfstError. THFST, a directory, is not written by the package, so
// its cases are not run. Calls and options are named here as the package
// names them: divvun-hfst's snake_case names, in camelCase.
//
// It is written once for every environment: parity-runtime.ts runs it under
// Node, Bun and Deno, and parity-browser.ts in a browser, on a page and in a
// Web Worker.

import type * as Compile from "../hfst/compile.ts";
import type { LookupReport, Transducer, TransducerType } from "../hfst/compile.ts";

type Package = typeof Compile;

/** Where the scenario finds the compile entry point and the fixtures, as URLs the environment can read. */
export interface CompileParityArgs {
  module: string;
  /** URL of the directory of built fixtures, ending in a slash. */
  fixtures: string;
}

/** Reads the bytes at a URL: with fetch in a browser, from the file system elsewhere. */
export type ReadBytes = (url: string) => Promise<Uint8Array>;

/** One group of cases, run against the tools, and every difference found. */
export interface ParityGroup {
  name: string;
  cases: number;
  differences: string[];
}

/** A call's options as the manifest names them, in snake_case. */
type Kwargs = Record<string, unknown>;

/**
 * One case of compile-parity.json. Which fields it has depends on its kind,
 * as divvun-hfst's suite reads them; each is read only where its kind is run.
 */
interface Case {
  kind: string;
  /** The tool's arguments. */
  tool: string[];
  /** Whether the tool exited 0. */
  ok: boolean;
  /** The file holding what the tool wrote or printed. */
  expected: string;
  input: string;
  type: TransducerType;
  use_backend_format: boolean;
  method: string;
  kwargs: Kwargs;
  first: string;
  second: string | null;
  /** The rules of a compose-intersect, or the input holding a pair test's rules. */
  rules: string[] | string;
  source: string;
  acceptor: string;
  error_model: string;
  metadata: string;
  sources: string[];
  options: Kwargs;
  text: string;
  script: string;
  written: string[];
  inputs: string[];
  lines: string[];
  pattern: string;
  archive: string;
  guesser: string;
  words: string[];
  tests: string;
}

interface Manifest {
  inputs: Record<string, string>;
  cases: Case[];
}

/** The kinds the package does not run: THFST is a directory, which it does not write. */
const NOT_RUN = new Set(["thfst"]);

/** The files the cases read, read once before any runs. */
class Files {
  readonly #bytes = new Map<string, Uint8Array>();
  readonly #decoder = new TextDecoder("utf-8", { fatal: true });

  async load(names: Iterable<string>, read: (name: string) => Promise<Uint8Array>): Promise<void> {
    const wanted = [...new Set(names)].filter((name) => !this.#bytes.has(name));
    const loaded = await Promise.all(wanted.map((name) => read(name)));
    wanted.forEach((name, i) => this.#bytes.set(name, loaded[i] as Uint8Array));
  }

  bytes(name: string): Uint8Array {
    const bytes = this.#bytes.get(name);
    if (bytes === undefined) throw new Error(`${name} was not loaded`);
    return bytes;
  }

  text(name: string): string {
    const bytes = this.bytes(name);
    return this.#decoder.decode(bytes);
  }
}

/** Every file a case reads, by its name in the fixture directory. */
function* namesOf(manifest: Manifest): Generator<string> {
  yield* Object.values(manifest.inputs);
  const values = (value: unknown): string[] => {
    if (typeof value !== "object" || value === null) return [];
    const text: unknown = Reflect.get(value, "$text");
    if (typeof text === "string") return [text];
    return Object.values(value).flatMap(values);
  };
  for (const c of manifest.cases) {
    if (NOT_RUN.has(c.kind)) continue;
    if (c.kind === "xfst") {
      yield `${c.expected}/stdout`;
      for (const name of c.written) yield `${c.expected}/${name}`;
    } else {
      yield c.expected;
    }
    if (Array.isArray(c.sources)) yield* c.sources;
    for (const name of [c.source, c.acceptor, c.error_model, c.metadata]) if (typeof name === "string") yield name;
    yield* values(c.kwargs);
  }
}

/** A name as the package spells it: `from_label` is `fromLabel`, and `from_` is `from`. */
export function camel(name: string): string {
  const bare = name.replace(/_+$/, "");
  return bare.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase());
}

/**
 * `text`, a number as `toPrecision` writes it, in plain decimal notation with
 * no exponent and no trailing zeros, as Python's Decimal formats it.
 */
function plainDecimal(text: string): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?(?:e([+-]\d+))?$/.exec(text);
  if (match === null) return text;
  const [, sign = "", whole = "", fraction = "", exponent = "0"] = match;
  const digits = whole + fraction;
  const point = whole.length + Number(exponent);
  let out: string;
  if (point <= 0) out = `0.${"0".repeat(-point)}${digits}`;
  else if (point >= digits.length) out = digits + "0".repeat(point - digits.length);
  else out = `${digits.slice(0, point)}.${digits.slice(point)}`;
  out = out.replace(/^0+(?=\d)/, "");
  if (out.includes(".")) out = out.replace(/0+$/, "").replace(/\.$/, "");
  return sign + out;
}

/** A weight as the tools print a 32-bit float on its own: the fewest digits that read back as it, never with an exponent. */
export function shown(weight: number): string {
  if (weight === Infinity) return "inf";
  if (weight === -Infinity) return "-inf";
  let digits = String(weight);
  for (let precision = 1; precision < 18; precision += 1) {
    digits = weight.toPrecision(precision);
    if (Math.fround(Number(digits)) === weight) break;
  }
  return plainDecimal(digits);
}

/** A weight as `%.6f` prints it. */
function fixed6(weight: number): string {
  if (weight === Infinity) return "inf";
  if (weight === -Infinity) return "-inf";
  return weight.toFixed(6);
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
}

/** The stream `ts` make, each written as it is. */
function stream(ts: Transducer[]): Uint8Array {
  const parts: Uint8Array[] = [];
  for (const t of ts) parts.push(t.toBytes());
  return concat(parts);
}

/** What `hfst lookup` prints for `reports`, in its Xerox format. */
function lookupText(reports: LookupReport[]): string {
  let text = "";
  for (const report of reports) {
    if (report.results === null || report.results.length === 0) {
      text += `${report.input}\t${report.input}+?\tinf\n\n`;
      continue;
    }
    for (const r of report.results) text += `${report.input}\t${r.output}\t${fixed6(r.weight)}\n`;
    text += report.infinite ? `${report.input}\t[...cyclic...]\n\n` : "\n";
  }
  return text;
}

/** The AT&T text of each transducer, as the tool reads them one by one, each with the `--` line that ends it. */
function attPieces(text: string): string[] {
  const pieces = [""];
  for (const line of text.split(/(?<=\n)/)) {
    pieces[pieces.length - 1] += line;
    if (line.trim() === "--") pieces.push("");
  }
  return pieces.filter((piece) => piece !== "");
}

function describe(value: unknown): string {
  if (value instanceof Uint8Array) return `${value.length} bytes`;
  const text = typeof value === "string" ? JSON.stringify(value) : String(value);
  return text.length > 300 ? `${text.slice(0, 300)}...` : text;
}

/** The package's side of each case, from the fixtures' files. */
class Runner {
  readonly #pkg: Package;
  readonly #files: Files;
  readonly inputs: Record<string, string>;

  constructor(pkg: Package, files: Files, manifest: Manifest) {
    this.#pkg = pkg;
    this.#files = files;
    this.inputs = manifest.inputs;
  }

  all(name: string): Transducer[] {
    const file = this.inputs[name];
    if (file === undefined) throw new Error(`compile-parity.json names no input ${name}`);
    return this.#pkg.readTransducers(this.#files.bytes(file));
  }

  one(name: string): Transducer {
    const [t, ...more] = this.all(name);
    if (t === undefined || more.length > 0) throw new Error(`${name} holds ${more.length + 1} transducers`);
    return t;
  }

  get files(): Files {
    return this.#files;
  }

  /** A value of the manifest as the call takes it: a transducer, a file's text or an infinity where it names one. */
  value(given: unknown): unknown {
    if (typeof given !== "object" || given === null || Array.isArray(given)) return given;
    const tagged = (key: string): unknown => Reflect.get(given, key);
    if (typeof tagged("$transducer") === "string") return this.one(tagged("$transducer") as string);
    if (typeof tagged("$text") === "string") return this.#files.text(tagged("$text") as string);
    if (tagged("$float") === "inf") return Infinity;
    return Object.fromEntries(Object.entries(given).map(([key, value]) => [key, this.value(value)]));
  }

  /** The options of a call: the manifest's snake_case names in camelCase, each value as the call takes it. */
  options(kwargs: Record<string, unknown>): Record<string, unknown> {
    return Object.fromEntries(
      Object.entries(kwargs)
        .filter(([key]) => key !== "$args")
        .map(([key, value]) => [camel(key), this.value(value)]),
    );
  }

  /** Calls `method` on `target` as the case names it, its `$args` first and its options last. */
  call(target: object, method: string, kwargs: Record<string, unknown>, args: unknown[] = []): unknown {
    const name = camel(method);
    const f: unknown = Reflect.get(target, name);
    if (typeof f !== "function") throw new Error(`the package has no ${name}`);
    const positional = Array.isArray(kwargs.$args) ? kwargs.$args : [];
    const options = this.options(kwargs);
    const rest = Object.keys(options).length > 0 ? [options] : [];
    return Reflect.apply(f, target, [...args, ...positional, ...rest]);
  }
}

type Check = (c: Case) => Uint8Array | string | null;

/**
 * Runs each case of `kinds`. A case the tool did not fail gives what
 * `expected` reads; one it failed throws an HfstError. A case `run` checks
 * itself gives null. A kind with no cases fails: the expectations are
 * missing, not met.
 */
function group(
  pkg: Package,
  manifest: Manifest,
  name: string,
  kinds: string[],
  run: Check,
  expected: (c: Case) => Uint8Array | string,
): ParityGroup {
  const cases = manifest.cases.filter((c) => kinds.includes(c.kind));
  const differences: string[] = [];
  if (cases.length === 0) differences.push(`compile-parity.json holds no ${kinds.join(", ")} cases`);
  for (const c of cases) {
    const what = `hfst ${c.tool.join(" ")}`;
    let got: Uint8Array | string | null;
    try {
      got = run(c);
    } catch (error) {
      if (!(error instanceof pkg.HfstError)) {
        differences.push(`${what}: threw ${error instanceof Error ? (error.stack ?? error.message) : String(error)}`);
      } else if (c.ok) {
        differences.push(`${what}: threw ${error.code}: ${error.message}`);
      }
      continue;
    }
    if (!c.ok) {
      differences.push(`${what}: the tool failed, and the package did not`);
      continue;
    }
    if (got === null) continue;
    const want = expected(c);
    const same = got instanceof Uint8Array && want instanceof Uint8Array ? sameBytes(got, want) : got === want;
    if (!same) differences.push(`${what}: ${describe(got)}\n    tool: ${describe(want)}`);
  }
  return { name, cases: cases.length, differences };
}

/**
 * Runs each case of `kind` through `run`, which checks it itself and throws
 * where it differs: for the tools whose exit status reports what they found,
 * not whether they ran.
 */
function own(manifest: Manifest, name: string, kind: string, run: (c: Case) => void): ParityGroup {
  const cases = manifest.cases.filter((c) => c.kind === kind);
  const differences: string[] = [];
  if (cases.length === 0) differences.push(`compile-parity.json holds no ${kind} cases`);
  for (const c of cases) {
    try {
      run(c);
    } catch (error) {
      differences.push(`hfst ${c.tool.join(" ")}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
  return { name, cases: cases.length, differences };
}

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function writes(pkg: Package, r: Runner, m: Manifest): ParityGroup[] {
  const expected = (c: Case) => r.files.bytes(c.expected);
  const printed = (c: Case) => r.files.text(c.expected);
  const toBytes = group(
    pkg,
    m,
    "each type is written as fst2fst writes it",
    ["to_bytes"],
    (c) => {
      for (const name of ["words", "words.foma", "words.olw", "words.olu"]) {
        const file = r.inputs[name] as string;
        assert(sameBytes(r.one(name).toBytes(), r.files.bytes(file)), `${name} reads and writes back as it was`);
      }
      return r.one(c.input).toBytes(c.type, { useBackendFormat: c.use_backend_format });
    },
    expected,
  );
  const operations = group(
    pkg,
    m,
    "operations write what the tools write",
    ["unary", "binary", "compose_intersect", "guessify"],
    (c) => {
      switch (c.kind) {
        case "unary": {
          const ts = r.all(c.input);
          for (const t of ts) assert(r.call(t, c.method, c.kwargs) === t, `${c.method} returns the transducer`);
          return stream(ts);
        }
        case "binary": {
          const t = r.one(c.first);
          const other = c.second === null ? t : r.one(c.second);
          assert(r.call(t, c.method, c.kwargs, [other]) === t, `${c.method} returns the transducer`);
          return t.toBytes();
        }
        case "compose_intersect": {
          const t = r.one(c.input);
          const rules = (c.rules as string[]).map((name) => r.one(name));
          r.call(t, "compose_intersect", c.kwargs, [rules]);
          return t.toBytes();
        }
        default: {
          const t = r.one(c.input);
          const generator = r.call(t, "guessify", c.kwargs) as Transducer | null;
          return concat([t.toBytes(), generator === null ? new Uint8Array() : generator.toBytes()]);
        }
      }
    },
    expected,
  );
  const text = group(pkg, m, "text writers write what fst2txt writes", ["text"], (c) => {
    return r.call(r.one(c.input), c.method, c.kwargs) as string;
  }, printed);
  const dhfst = group(pkg, m, "DHFST is written as fst2fst writes it", ["dhfst"], (c) => {
    return r.call(pkg.readTransducer(r.files.bytes(c.source)), c.method, c.kwargs) as Uint8Array;
  }, expected);
  const bhfst = group(pkg, m, "BHFST is packed as hfst bhfst packs it", ["bhfst"], (c) => {
    const options = r.options(c.kwargs);
    return pkg.packBhfst(r.files.bytes(c.acceptor), r.files.bytes(c.error_model), r.files.text(c.metadata), options);
  }, expected);
  return [toBytes, operations, text, dhfst, bhfst];
}

function compiles(pkg: Package, r: Runner, m: Manifest): ParityGroup[] {
  const sources = (c: Case): [string, string][] => c.sources.map((name) => [name, r.files.text(name)]);
  const only = (c: Case): [string, string] => {
    const [first] = sources(c);
    assert(first !== undefined, "the case has a source");
    return first;
  };
  const compilers = group(
    pkg,
    m,
    "compilers write what the tools write",
    ["regex", "lexc", "twolc", "pmatch2fst", "strings", "att", "prolog"],
    (c) => {
      const options = r.options(c.options);
      switch (c.kind) {
        case "regex": {
          const [name, text] = only(c);
          return pkg.regex(text, { ...options, name }).transducer.toBytes();
        }
        case "lexc": {
          const named = sources(c);
          const names = named.map(([name]) => name);
          return pkg.compileLexc(named.map(([, text]) => text), { ...options, names }).transducer.toBytes();
        }
        case "twolc":
          return stream(pkg.compileTwolc(c.text, options).transducers);
        case "pmatch2fst": {
          const [name, text] = only(c);
          return pkg.compilePmatch(text, { ...options, name }).archive;
        }
        case "strings": {
          const [name, text] = only(c);
          if (options.disjunctStrings === true) return pkg.compileStrings(text, { ...options, name }).transducer.toBytes();
          const lines = text.split("\n").filter((line) => line !== "").map((line) => `${line}\n`);
          return stream(lines.map((line) => pkg.compileStrings(line, { ...options, name }).transducer));
        }
        case "att": {
          const [name, text] = only(c);
          if (options.disjunct === true) return pkg.parseAtt(text, { ...options, name }).transducer.toBytes();
          return stream(attPieces(text).map((piece) => pkg.parseAtt(piece, { ...options, name }).transducer));
        }
        default: {
          const [name, text] = only(c);
          return pkg.parseProlog(text, { ...options, name }).transducer.toBytes();
        }
      }
    },
    (c) => r.files.bytes(c.expected),
  );
  const xfst = group(pkg, m, "xfst scripts run as the tool runs them", ["xfst"], (c) => {
    const result = pkg.compileXfst(c.script, { ...r.options(c.options), name: "s.xfst", files: {} });
    assert(result.output === r.files.text(`${c.expected}/stdout`), `the output: ${describe(result.output)}`);
    const written = Object.keys(result.written).sort();
    assert(written.join(" ") === c.written.join(" "), `the files written: ${written.join(" ")}`);
    for (const name of written) {
      const bytes = result.written[name] as Uint8Array;
      assert(sameBytes(bytes, r.files.bytes(`${c.expected}/${name}`)), `${name} holds what the tool wrote`);
    }
    assert(
      result.warnings.some((w) => w.message.includes("empty stack")),
      "a command on the empty stack is a warning",
    );
    return null;
  }, (c) => r.files.bytes(c.expected));
  return [compilers, xfst];
}

function queries(pkg: Package, r: Runner, m: Manifest): ParityGroup[] {
  const printed = (c: Case) => r.files.text(c.expected);
  const paths = group(pkg, m, "extractPaths finds what fst2strings prints", ["extract_paths"], (c) => {
    const found = r.call(r.one(c.input), "extract_paths", c.kwargs) as ReturnType<Transducer["extractPaths"]>;
    return found
      .map((p) => {
        const identity = p.pairs.every(([i, o]) => i === o);
        return `${identity ? p.input : `${p.input}:${p.output}`}\t${shown(p.weight)}\n`;
      })
      .join("");
  }, printed);
  const lookups = group(
    pkg,
    m,
    "the lookup reports are what the lookup tools print",
    ["lookup_report", "flookup_report", "optimized_lookup_report"],
    (c) => {
      const ts = c.inputs.map((name) => r.one(name));
      const options = r.options(c.kwargs);
      if (c.kind === "lookup_report") return lookupText(pkg.lookupReport(ts, c.lines, options));
      if (c.kind === "flookup_report") {
        let text = "";
        for (const report of pkg.flookupReport(ts, c.lines, options)) {
          if (report.results !== null && report.results.length > 0) {
            for (const res of report.results) text += `${report.input}\t${res.output}\t${fixed6(res.weight)}\n`;
            text += "\n";
          } else {
            text += `${report.input}\t${report.input}+?\tinf\n\n`;
          }
        }
        return text;
      }
      const weighted = ts[0]?.type === "optimized-lookup-weighted";
      let text = "";
      for (const report of pkg.optimizedLookupReport(ts, c.lines, options)) {
        const word = report.input;
        if (report.analyses === null) text += `${word}\t${word}\t+?\n\n\n\n`;
        else if (report.analyses.length === 0) text += `${word}\t${word}\t+?${weighted ? "\n\n" : "\n\n\n\n"}`;
        else {
          for (const [output, weight] of report.analyses) {
            text += `${word}\t${output}${weighted ? `\t${shown(weight)}` : ""}\n`;
          }
          text += "\n";
        }
      }
      return text;
    },
    printed,
  );
  // compare exits 1 for transducers that differ, so a case it printed for is
  // a comparison made, and one it printed nothing for one it refused.
  const compare = own(m, "compare decides as hfst compare does", "compare", (c) => {
    const first = r.one(c.first);
    const second = r.one(c.second as string);
    const text = r.files.text(c.expected);
    if (text === "") {
      let threw = false;
      try {
        first.compare(second);
      } catch (error) {
        if (!(error instanceof pkg.HfstError)) throw error;
        threw = true;
      }
      assert(threw, "the tool refused these, and the package did not throw");
      return;
    }
    const equal = first.compare(second);
    const names = [first.name || r.inputs[c.first], second.name || r.inputs[c.second as string]];
    const said = `${names[0]} ${equal ? "==" : "!="} ${names[1]}\n`;
    assert(said === text, `${JSON.stringify(said)}, and the tool printed ${JSON.stringify(text)}`);
  });
  const alphabets = group(pkg, m, "dumpAlphabets reports what dump-alphabets prints", ["dump_alphabets"], (c) => {
    const a = r.one(c.input).dumpAlphabets();
    return [...a.declared, ...a.seen].map((s) => `${s}\n`).join("");
  }, printed);
  const checkAlpha = group(pkg, m, "checkAlpha reports what check-alpha prints", ["check_alpha"], (c) => {
    const first = r.one(c.first);
    const second = r.one(c.second as string);
    const report = first.checkAlpha(second);
    const text = r.files.text(c.expected);
    const firstOnly = report.firstUsed.filter((s) => !report.secondUsed.includes(s));
    const secondOnly = report.secondDeclared.filter((s) => !report.firstDeclared.includes(s));
    assert(report.mismatch, "a mismatch");
    assert(
      text.includes(`In first ${first.name} but not in second ${second.name}:${firstOnly.join(", ")}\n`),
      "what the first has",
    );
    assert(
      text.includes(`Second ${second.name} has but first ${first.name} does not: ${secondOnly.join(", ")}\n`),
      "what the second has",
    );
    return null;
  }, printed);
  const summaries = group(pkg, m, "summarize counts what summarize prints", ["summarize"], (c) => {
    const summary = r.one(c.input).summarize({ verbose: true });
    const text = r.files.text(c.expected);
    assert(text.includes(`# of states: ${summary.states}\n`), `${summary.states} states`);
    assert(text.includes(`# of arcs: ${summary.arcs}\n`), `${summary.arcs} arcs`);
    return null;
  }, printed);
  const text = group(
    pkg,
    m,
    "the text queries are what the tools print",
    ["grep", "pmatch", "guess", "naive_tokeniser"],
    (c) => {
      switch (c.kind) {
        case "grep":
          return pkg.grep(c.pattern, c.text, r.options(c.kwargs)).lines.map((line) => `${line.text}\n`).join("");
        case "pmatch": {
          const tokeniser = pkg.readTokeniser(r.files.bytes(r.inputs[c.archive] as string));
          let out = "";
          for (const piece of tokeniser.pmatch(c.text, r.options(c.kwargs))) {
            if (piece.marked !== null) out += `${piece.marked}\n${c.kwargs.newline ? "" : "\n"}`;
            else if (piece.located !== null && piece.located.length > 0) {
              for (const l of piece.located) out += `${l.start}|${l.length}|${l.output}|${l.tag}|${shown(l.weight)}\n`;
              out += "\n";
            }
          }
          return out;
        }
        case "guess": {
          const [guesser, generator] = r.all(c.guesser);
          assert(guesser !== undefined && generator !== undefined, "a guesser and its generator");
          let out = "";
          for (const report of guesser.guess(c.words, { generator })) {
            out += `${report.guesses.map((g) => `${report.input}\t${g}\n`).join("")}\n`;
          }
          return out;
        }
        default:
          return r.one(c.input).naiveTokeniser().format(c.text, "segment");
      }
    },
    printed,
  );
  // pair-test exits 1 when a test fails, so `ok` says whether every test
  // passed.
  const pairs = own(m, "pairTest reports what pair-test prints", "pair_test", (c) => {
    const results = pkg.pairTest(r.all(c.rules as string), c.tests, r.options(c.kwargs));
    const text = r.files.text(c.expected);
    assert(results.some((res) => res.failed) !== c.ok, "the tests fail where the tool's did");
    for (const res of results) {
      for (const failure of res.failures) assert(text.includes(`Rule ${failure.rule} fails:`), `${failure.rule} fails`);
    }
  });
  return [paths, lookups, compare, alphabets, checkAlpha, summaries, text, pairs];
}

// [spec:hfst:req:javascript.compile-parity/test]
/**
 * Starts the compile entry point at `args.module` from the `.wasm` file
 * beside it, and runs every case of compile-parity.json but THFST's through
 * it. Throws only when the package or the manifest cannot be loaded; a case
 * that differs, or throws where the tool did not fail, is a difference in
 * its group.
 */
export async function runCompileParity(args: CompileParityArgs, read: ReadBytes): Promise<ParityGroup[]> {
  const pkg: Package = await import(args.module);
  await pkg.init();
  const fixture = (name: string) => read(new URL(name, args.fixtures).href);
  const manifest: Manifest = JSON.parse(new TextDecoder().decode(await fixture("compile-parity.json")));
  const files = new Files();
  await files.load(namesOf(manifest), fixture);
  const runner = new Runner(pkg, files, manifest);
  return [...writes(pkg, runner, manifest), ...compiles(pkg, runner, manifest), ...queries(pkg, runner, manifest)];
}

/** The most differences a failed group prints. */
const SHOWN = 20;

/**
 * The lines that report `groups` under `label`, and whether every group
 * passed. A group that ran no case fails: the expectations are missing, not
 * met.
 */
export function report(label: string, groups: ParityGroup[]): { lines: string[]; passed: boolean } {
  const lines: string[] = [];
  let passed = true;
  for (const { name, cases, differences } of groups) {
    if (differences.length === 0 && cases > 0) {
      lines.push(`ok - ${label}: ${name} (${cases} cases as the tools give them)`);
      continue;
    }
    passed = false;
    lines.push(`not ok - ${label}: ${name}: ${differences.length} of ${cases} cases differ from the tools`);
    for (const line of differences.slice(0, SHOWN)) lines.push(`  ${line}`);
    if (differences.length > SHOWN) lines.push(`  and ${differences.length - SHOWN} more`);
  }
  return { lines, passed };
}
