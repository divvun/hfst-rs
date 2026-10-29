// The full package's compilers against this checkout's hfst tools: for the
// same source and options, each writes the bytes the tool wrote
// (js/test/compile-fixtures.mjs), with the files the source names taken from
// a map of the same files.

import type * as Full from "../hfst/index.ts";
import { assertBytes, assertStream } from "./bytes.ts";
import { assertEqual, readBytes, runTests, scenarioArgs, test } from "./harness.ts";

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
  return await readBytes(new URL(`compile/${name}`, args.fixtures).href);
}

async function text(name: string): Promise<string> {
  return new TextDecoder().decode(await fixture(name));
}

/** The files the sources name, as the tools read them. */
async function files(): Promise<Record<string, Uint8Array | string>> {
  return {
    "animals.hfst": await fixture("animals.hfst"),
    "farm.txt": await text("farm.txt"),
    "pets.pmatch": await text("pets.pmatch"),
  };
}

// [spec:hfst:req:javascript.compile/test]
test("compileRegex writes what hfst regexp2fst writes, in every type and with its options", async () => {
  const source = await text("refs.regexp");
  for (const type of TYPES) {
    using t = pkg.compileRegex(source, { type, files: await files() }).transducer;
    assertEqual(t.type, type, "type");
    await assertStream(type, t.toBytes(), await fixture(`regex.${type}.hfst`), `regex ${type}`);
  }
  const options = { semicolon: true, disjunct: true, minimize: false } as const;
  using t = pkg.compileRegex(await text("several.regexp"), options).transducer;
  assertBytes(t.toBytes(), await fixture("regex.options.hfst"), "regexp2fst -S -j -M");
});

// [spec:hfst:req:javascript.compile/test]
test("compileLexc writes what hfst lexc writes, in every type and with its options", async () => {
  const source = await text("words.lexc");
  for (const type of TYPES) {
    const compiled = pkg.compileLexc(source, { type, sourceName: "words.lexc", files: await files() });
    using t = compiled.transducer;
    assertEqual(t.name, "lexc(words.lexc)", "name");
    await assertStream(type, t.toBytes(), await fixture(`lexc.${type}.hfst`), `lexc ${type}`);
  }
  const options = { withFlags: true, alignStrings: true, sourceName: "words.lexc", files: await files() };
  using t = pkg.compileLexc(source, options).transducer;
  assertBytes(t.toBytes(), await fixture("lexc.options.hfst"), "lexc -F -A");
});

/**
 * Each rule of a foma stream the tool wrote, written again by the module, so
 * the compression on both sides is the module's.
 */
function rewritten(bytes: Uint8Array): Uint8Array[] {
  const all = pkg.readTransducers(bytes);
  const written = all.map((t) => t.toBytes());
  for (const t of all) t.free();
  return written;
}

// [spec:hfst:req:javascript.compile/test]
test("compileTwolc writes the rules hfst twolc writes, at both types and with its options", async () => {
  const source = await text("rules.twolc");
  const tropical = pkg.compileTwolc(source).transducers;
  const names = (all: Full.Transducer[]) => all.map((t) => t.name).join(", ");
  assertEqual(names(tropical), '"e raising before c", "no raising after b"', "the =>-conflict joined");
  assertBytes(pkg.writeTransducers(tropical), await fixture("twolc.openfst-tropical.hfst"), "twolc");
  const foma = pkg.compileTwolc(source, { type: "foma" }).transducers;
  const native = rewritten(await fixture("twolc.foma.hfst"));
  assertEqual(foma.length, native.length, "foma rules");
  foma.forEach((t, i) => assertBytes(t.toBytes(), native[i] ?? new Uint8Array(), `twolc -f foma, rule ${i}`));
  const resolved = pkg.compileTwolc(source, { resolveLeft: true, resolveRight: false }).transducers;
  assertEqual(names(resolved), '"e raising before c", "e raising before d", "no raising after b"', "unjoined");
  assertBytes(pkg.writeTransducers(resolved), await fixture("twolc.options.hfst"), "twolc -R -D");
  for (const t of [...tropical, ...foma, ...resolved]) t.free();
});

// [spec:hfst:req:javascript.compile/test]
test("compilePmatch writes the container hfst pmatch2fst writes, with and without --flatten", async () => {
  const source = await text("tokens.pmatch");
  for (const [options, name] of [
    [{}, "tokens.pmhfst"],
    [{ flatten: true }, "tokens.flatten.pmhfst"],
  ] as const) {
    const compiled = pkg.compilePmatch(source, { ...options, files: await files() });
    const native = await fixture(name);
    assertBytes(compiled.bytes, native, name);
    const read = pkg.readTransducers(native);
    assertEqual(compiled.transducers.map((t) => t.name).join(" "), read.map((t) => t.name).join(" "), `${name}: names`);
    for (const t of [...compiled.transducers, ...read]) t.free();
  }
});

await runTests(args.label);
