// DHFST files, against the package as it ships. The fixtures are written by
// `node js/build.mjs --test` with this checkout's hfst tools;
// js/test/dhfst-fixtures.mjs says what each one is.

import type { DhfstAcceptor } from "../hfst/dhfst.ts";
import {
  assert,
  assertEqual,
  loadPackage,
  readBytes,
  runTests,
  scenarioArgs,
  test,
  thrownCode,
} from "./harness.ts";
import { linesOf } from "./lookup-expect.ts";

const args = scenarioArgs();
const pkg = await loadPackage(args);
await pkg.init(await readBytes(args.wasm));

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

async function text(name: string): Promise<string> {
  return new TextDecoder().decode(await fixture(name));
}

function thrown(f: () => unknown): InstanceType<typeof pkg.HfstError> {
  try {
    f();
  } catch (error) {
    assert(error instanceof pkg.HfstError, `an HfstError, not ${String(error)}`);
    return error;
  }
  throw new Error("nothing was thrown");
}

async function acceptor(name: string): Promise<DhfstAcceptor> {
  const model = pkg.readDhfst(await fixture(name));
  assert(model instanceof pkg.DhfstAcceptor && model.type === "acceptor", `${name} is an acceptor`);
  return model;
}

/**
 * The least weight `hfst lookup -X print-space` printed for each input, as it
 * prints weights, or null for an input it found nothing for. A block of
 * `input TAB output TAB weight` lines and an empty line follows each input;
 * nothing found prints the weight `inf`, and a lookup that met a cycle ends
 * its results with a line that has no weight.
 */
function leastPrinted(print: string): (string | null)[] {
  const least: (string | null)[] = [];
  let best: number | null = null;
  for (const line of linesOf(print)) {
    if (line === "") {
      least.push(best === null ? null : best.toFixed(6));
      best = null;
      continue;
    }
    const weight = line.split("\t")[2];
    if (weight === undefined || weight === "inf") continue;
    best = best === null ? Number(weight) : Math.min(best, Number(weight));
  }
  return least;
}

function withByte(bytes: Uint8Array, at: number, value: number): Uint8Array {
  const copy = Uint8Array.from(bytes);
  copy[at] = value;
  return copy;
}

// [spec:hfst:req:javascript.dhfst+2/test]
// [spec:hfst:req:javascript.load-bytes+2/test]
test("an acceptor loads with its metadata, and its info agrees with what the writer recorded", async () => {
  using speller = await acceptor("speller.acceptor.dhfst");
  assertEqual(speller.version, 1, "version");
  assertEqual(speller.symbols[0], "@_EPSILON_SYMBOL_@", "symbol 0");
  for (const symbol of ["@P.CASE.NOM@", "@U.NUM.PL@", "t", "m"]) {
    assert(speller.symbols.includes(symbol), `the symbols hold ${symbol}`);
  }
  assert(Object.isFrozen(speller.symbols) && Object.isFrozen(speller.info), "the values cannot be changed");
  const meta: unknown = JSON.parse(speller.meta ?? "null");
  assert(typeof meta === "object" && meta !== null, `meta is a JSON object: ${String(speller.meta)}`);
  assertEqual(Reflect.get(meta, "source"), "speller.openfst-tropical.hfst", "the source the writer names");
  assertEqual(Reflect.get(meta, "arcs"), speller.info.arcs, "arcs");
  assertEqual(Reflect.get(meta, "free_arcs"), speller.info.freeArcs, "free arcs");
  assertEqual(Reflect.get(meta, "distances"), speller.info.distances, "distances");
  assert(speller.info.ids >= Number(Reflect.get(meta, "states")), "every state has a number");
  assert(speller.info.slots >= speller.info.ids + speller.info.labelMax, "slots");
});

// [spec:hfst:req:javascript.dhfst+2/test]
test("lookup gives the least weight hfst lookup prints for every input, or null when it prints none", async () => {
  using speller = await acceptor("speller.acceptor.dhfst");
  const inputs = linesOf(await text("speller-inputs.txt"));
  const expected = leastPrinted(await text("lookup.speller.txt"));
  assertEqual(expected.length, inputs.length, "inputs printed");
  let accepted = 0;
  inputs.forEach((input, i) => {
    const weight = speller.lookup(input);
    assertEqual(weight === null ? null : weight.toFixed(6), expected[i] ?? null, JSON.stringify(input));
    accepted += weight === null ? 0 : 1;
  });
  assertEqual(accepted, 8, "the inputs the speller accepts");
  assertEqual(speller.lookup("talo"), 0.875, "talo, through two flags and a cycle of each kind");
  assertEqual(speller.lookup(`t${"a".repeat(20000)}`), null, "a long word ends");
});

// [spec:hfst:req:javascript.dhfst+2/test]
test("a multichar symbol is read only from its graphemes' own symbols, and a symbol matches only its own form", async () => {
  using graphemes = await acceptor("graphemes.acceptor.dhfst");
  using lookup = pkg.readTransducer(await fixture("graphemes.optimized-lookup-weighted.hfst"));
  const best = (input: string) => lookup.lookup(input)[0]?.weight ?? null;
  assertEqual(best("ija"), 0.5, "hfst lookup reads ij as one symbol");
  assertEqual(graphemes.lookup("ija"), 2, "the speller reads i, j and a");
  assertEqual(best("xy"), 0, "hfst lookup reads xy as one symbol");
  assertEqual(graphemes.lookup("xy"), null, "no grapheme spells xy");
  assertEqual(graphemes.lookup("\u00e9"), 0.25, "é as stored, composed");
  assertEqual(graphemes.lookup("o\u0308"), 0.75, "ö as stored, decomposed");
  assertEqual(best("e\u0301"), 0.25, "hfst lookup matches é decomposed");
  assertEqual(graphemes.lookup("e\u0301"), null, "the speller does not");
  assertEqual(best("\u00f6"), 0.75, "hfst lookup matches ö composed");
  assertEqual(graphemes.lookup("\u00f6"), null, "the speller does not");
});

// [spec:hfst:req:javascript.dhfst+2/test]
// [spec:hfst:req:javascript.load-bytes+2/test]
test("an error model loads with its metadata and has no lookup", async () => {
  using model = pkg.readDhfst(await fixture("errmodel.error-model.dhfst"));
  assert(model instanceof pkg.DhfstErrorModel && model.type === "error-model", "an error model");
  assertEqual(model.version, 1, "version");
  const letters = model.symbols.filter((symbol) => !symbol.startsWith("@"));
  assertEqual([...letters].sort().join(" "), "a b c", "the symbols that are not special");
  assertEqual(model.symbols[0], "@_EPSILON_SYMBOL_@", "symbol 0");
  const meta: unknown = JSON.parse(model.meta ?? "null");
  assert(typeof meta === "object" && meta !== null, `meta is a JSON object: ${String(model.meta)}`);
  assertEqual(Reflect.get(meta, "source"), "errmodel.openfst-tropical.hfst", "the source the writer names");
  assertEqual(model.stateCount, Reflect.get(meta, "source-states"), "states");
  assert(!("lookup" in model), "an error model has no lookup");
});

// [spec:hfst:req:javascript.dhfst+2/test]
test("files that fail divvunspell's validation or are truncated throw invalid-format with the reason", async () => {
  const speller = await fixture("speller.acceptor.dhfst");
  const errmodel = await fixture("errmodel.error-model.dhfst");
  const cases: [string, Uint8Array, string][] = [
    ["empty", new Uint8Array(0), "empty"],
    ["only the magic", speller.subarray(0, 5), "truncated"],
    ["only the header's first eight bytes", speller.subarray(0, 8), "truncated"],
    ["truncated in the sections", speller.subarray(0, speller.length >> 1), "not a valid DHFST file"],
    ["one byte short", speller.subarray(0, speller.length - 1), "not a valid DHFST file"],
    ["an unknown type", withByte(speller, 5, 9), "type 9"],
    ["a later version", withByte(speller, 6, 2), "version 2"],
    ["a reserved byte set", withByte(speller, 7, 1), "reserved"],
    ["an acceptor relabelled an error model", withByte(speller, 5, 1), "not a valid DHFST file"],
    ["an error model relabelled an acceptor", withByte(errmodel, 5, 2), "not a valid DHFST file"],
    ["a truncated error model", errmodel.subarray(0, errmodel.length - 9), "not a valid DHFST file"],
  ];
  for (const [what, bytes, reason] of cases) {
    const error = thrown(() => pkg.readDhfst(bytes));
    assertEqual(error.code, "invalid-format", `${what}: code (${error.message})`);
    assert(error.message.includes(reason), `${what}: the message says ${reason}: ${error.message}`);
  }
  using after = await acceptor("speller.acceptor.dhfst");
  assertEqual(after.lookup("ki"), 0.5, "the module still loads and looks up");
});

// [spec:hfst:req:javascript.load-bytes+2/test]
test("bytes that are not DHFST throw invalid-format, or unsupported-format naming a format the package does not read", async () => {
  const olw = await fixture("words.optimized-lookup-weighted.hfst");
  const ol = thrown(() => pkg.readDhfst(olw));
  assertEqual(ol.code, "invalid-format", `an optimized-lookup transducer: ${ol.message}`);
  assert(ol.message.includes("readTransducers"), `the message says what reads it: ${ol.message}`);
  for (const [name, format] of [
    ["words.foma.hfst", "foma"],
    ["words.openfst-tropical.hfst", "openfst-tropical"],
  ] as const) {
    const bytes = await fixture(name);
    const error = thrown(() => pkg.readDhfst(bytes));
    assertEqual(error.code, "unsupported-format", `${name}: code`);
    assert(error.message.includes(format), `${name}: the message names ${format}: ${error.message}`);
  }
  const prose = thrown(() => pkg.readDhfst(new TextEncoder().encode("a speller, honestly")));
  assertEqual(prose.code, "invalid-format", "text");
  for (const notBytes of [null, "DHFST", [68, 72, 70, 83, 84], new ArrayBuffer(8)]) {
    const call = () => pkg.readDhfst(notBytes as unknown as Uint8Array);
    assertEqual(thrownCode(pkg.HfstError, call), "invalid-argument", String(notBytes));
  }
  const dhfst = await fixture("speller.acceptor.dhfst");
  const asTransducers = thrown(() => pkg.readTransducers(dhfst));
  assertEqual(asTransducers.code, "unsupported-format", "readTransducers on DHFST");
  assert(asTransducers.message.includes("readDhfst"), `the message says what reads it: ${asTransducers.message}`);
});

// [spec:hfst:req:javascript.dhfst+2/test]
// [spec:hfst:req:javascript.handle-lifetime+1/test]
test("a freed acceptor or error model throws freed, freeing twice does nothing, and using frees", async () => {
  const speller = await acceptor("speller.acceptor.dhfst");
  const model = pkg.readDhfst(await fixture("errmodel.error-model.dhfst"));
  assert(model.type === "error-model", "an error model");
  speller.free();
  speller.free();
  model[Symbol.dispose]();
  model.free();
  for (const [what, read] of [
    ["lookup", () => speller.lookup("ki")],
    ["symbols", () => speller.symbols],
    ["info", () => speller.info],
    ["version", () => model.version],
    ["state count", () => model.stateCount],
  ] as const) {
    assertEqual(thrownCode(pkg.HfstError, read), "freed", what);
  }
  let scoped: DhfstAcceptor | null = null;
  {
    using inside = await acceptor("speller.acceptor.dhfst");
    scoped = inside;
    assertEqual(inside.lookup("ki"), 0.5, "inside the block");
  }
  const freed = scoped;
  assertEqual(thrownCode(pkg.HfstError, () => freed.lookup("ki")), "freed", "after the block");
});

await runTests(args.label);
