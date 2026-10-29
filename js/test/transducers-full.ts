// What only @divvun/hfst does with transducers: write them, convert them,
// and read and print AT&T text, each checked against what this checkout's
// hfst tools made of the same transducers (js/test/fixtures.mjs).

import type * as Full from "../hfst/index.ts";
import {
  assert,
  assertEqual,
  readBytes,
  runTests,
  scenarioArgs,
  test,
  thrownCode,
} from "./harness.ts";

const args = scenarioArgs();
const pkg: typeof Full = await import(args.module);
await pkg.init(await readBytes(args.wasm));

const TYPES = [
  "openfst-tropical",
  "optimized-lookup-weighted",
  "optimized-lookup-unweighted",
  "foma",
] as const;

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

async function text(name: string): Promise<string> {
  return new TextDecoder().decode(await fixture(name));
}

/**
 * The bytes after the HFST3 header of a one-member stream. The tools name
 * what they make after the tool and the source file, so two transducers
 * built the same way differ in their headers and agree in their payloads.
 */
function payload(bytes: Uint8Array): Uint8Array {
  return bytes.subarray(8 + (bytes[5] ?? 0) + 256 * (bytes[6] ?? 0));
}

async function gunzip(bytes: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([Uint8Array.from(bytes)]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/**
 * Whether two payloads of `type` hold the same transducer, byte for byte. A
 * foma payload is gzip, and the hfst tools compress with flate2's zlib-rs
 * backend, which their zip dependency turns on, while the module links
 * miniz_oxide: the compressed bytes differ, so the text they hold is
 * compared.
 */
async function assertPayload(
  type: string,
  actual: Uint8Array,
  expected: Uint8Array,
  what: string,
): Promise<void> {
  if (type !== "foma") return assertBytes(actual, expected, what);
  assertBytes(await gunzip(actual), await gunzip(expected), `${what}, uncompressed`);
}

/** {@link assertPayload} for a one-member stream, whose header must match too. */
async function assertStream(
  type: string,
  actual: Uint8Array,
  expected: Uint8Array,
  what: string,
): Promise<void> {
  const header = expected.length - payload(expected).length;
  assertBytes(actual.subarray(0, header), expected.subarray(0, header), `${what}: header`);
  await assertPayload(type, payload(actual), payload(expected), what);
}

function assertBytes(actual: Uint8Array, expected: Uint8Array, what: string): void {
  const length = Math.min(actual.length, expected.length);
  for (let i = 0; i < length; i += 1) {
    if (actual[i] !== expected[i]) throw new Error(`${what}: the bytes differ at ${i}`);
  }
  assertEqual(actual.length, expected.length, `${what}: length`);
}

// [spec:hfst:req:javascript.load-bytes+1/test]
// A pmatch container is left out: its definitions load with TOP's alphabet,
// so they are written as transducers of their own and the bytes differ from
// the container's.
test("toBytes and writeTransducers write what the hfst library wrote", async () => {
  for (const type of TYPES) {
    const bytes = await fixture(`words.${type}.hfst`);
    using t = pkg.readTransducer(bytes);
    await assertStream(type, t.toBytes(), bytes, `${type}: toBytes`);
  }
  for (const name of ["two.openfst-tropical.hfst", "two.optimized-lookup-weighted.hfst", "ordered.hfst"]) {
    const bytes = await fixture(name);
    const all = pkg.readTransducers(bytes);
    assertEqual(all.length, 2, `${name}: transducers`);
    assertBytes(pkg.writeTransducers(all), bytes, `${name}: writeTransducers`);
    for (const t of all) t.free();
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("optimized lookup with no header is written with one, around the same payload", async () => {
  for (const type of ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const) {
    const bare = await fixture(`words.${type}.bare`);
    using t = pkg.readTransducer(bare);
    assertBytes(payload(t.toBytes()), bare, type);
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("convert reaches every type as hfst fst2fst does, and leaves its receiver", async () => {
  for (const from of TYPES) {
    using source = pkg.readTransducer(await fixture(`words.${from}.hfst`));
    const before = source.toBytes();
    for (const to of TYPES) {
      using converted = source.convert(to);
      assertEqual(converted.type, to, `${from} -> ${to}: type`);
      assertEqual(converted.name, source.name, `${from} -> ${to}: name`);
      const native = await fixture(`convert.${from}.${to}.hfst`);
      await assertPayload(to, payload(converted.toBytes()), payload(native), `${from} -> ${to}`);
    }
    assertEqual(source.type, from, `${from}: the receiver's type`);
    assertBytes(source.toBytes(), before, `${from}: the receiver`);
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("readAtt makes what hfst txt2fst makes, and toAtt prints what hfst fst2txt prints", async () => {
  const att = await text("small.att");
  for (const type of TYPES) {
    using read = pkg.readAtt(att, type);
    assertEqual(read.type, type, `${type}: type`);
    assertEqual(read.name, "", `${type}: name`);
    const native = await fixture(`att.${type}.hfst`);
    await assertPayload(type, payload(read.toBytes()), payload(native), `readAtt ${type}`);
    using t = pkg.readTransducer(await fixture(`words.${type}.hfst`));
    assertEqual(t.toAtt(), await text(`words.${type}.att`), `toAtt ${type}`);
  }
  using tropical = pkg.readAtt(att);
  assertEqual(tropical.type, "openfst-tropical", "the default type");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("readAtt takes one well-formed transducer", async () => {
  const att = await text("small.att");
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readAtt(`${att}--\n${att}`)), "transducer-count", "two");
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readAtt("0\t1\ta\n")), "invalid-format", "malformed");
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readAtt("")), "invalid-format", "empty");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("the full package reads tropical and foma with or without a header", async () => {
  for (const [name, type] of [
    ["words.openfst-tropical.bare", "openfst-tropical"],
    ["words.foma.plain", "foma"],
  ] as const) {
    using t = pkg.readTransducer(await fixture(name));
    assertEqual(t.type, type, name);
    assert(t.toAtt().length > 0, `${name}: prints`);
  }
});

await runTests(args.label);
