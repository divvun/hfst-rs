// Lookup on the tropical and foma transducers only @divvun/hfst reads. Each
// result list is compared with what this checkout's `hfst lookup` printed for
// the same transducer and input (js/test/lookup-fixtures.mjs).

import type * as Full from "../hfst/index.ts";
import { assertLookup, linesOf, parsePrint, printed } from "./lookup-expect.ts";
import { assert, assertEqual, readBytes, runTests, scenarioArgs, test } from "./harness.ts";

const args = scenarioArgs();
const pkg: typeof Full = await import(args.module);
await pkg.init(await readBytes(args.wasm));

const TYPES = ["openfst-tropical", "foma"] as const;
const SOURCES = ["words", "flags", "ties"] as const;

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

async function text(name: string): Promise<string> {
  return new TextDecoder().decode(await fixture(name));
}

const inputs = linesOf(await text("lookup-inputs.txt"));

function summary(results: readonly Full.LookupResult[]): string {
  return results.map(({ output, symbols, weight }) => `${weight} ${output} [${symbols.join(" ")}]`).join("; ");
}

// [spec:hfst:req:javascript.lookup/test]
test("tropical and foma lookup returns what hfst lookup prints, for every input", async () => {
  for (const source of SOURCES) {
    for (const type of TYPES) {
      using t = pkg.readTransducer(await fixture(`${source}.${type}.hfst`));
      assertEqual(t.type, type, `${source}: type`);
      const expected = parsePrint(await text(`lookup.${source}.${type}.txt`));
      assertEqual(expected.length, inputs.length, `${source} ${type}: inputs printed`);
      for (let round = 0; round < 2; round += 1) {
        inputs.forEach((input, i) => {
          const what = `${source} ${type} ${JSON.stringify(input)}, round ${round}`;
          assertLookup(t.lookup(input), expected[i] ?? [], what);
        });
      }
    }
  }
});

// [spec:hfst:req:javascript.lookup/test]
test("flag diacritics block a path in the general lookup too", async () => {
  using tropical = pkg.readTransducer(await fixture("flags.openfst-tropical.hfst"));
  assertEqual(
    summary(tropical.lookup("talo")),
    "0.5 talo+N+Nom [t a l o +N +Nom]; 1.5 talo+N+Gen [t a l o +N +Gen]",
    "tropical",
  );
  using foma = pkg.readTransducer(await fixture("flags.foma.hfst"));
  assertEqual(summary(foma.lookup("talo")), "0 talo+N+Gen [t a l o +N +Gen]; 0 talo+N+Nom [t a l o +N +Nom]", "foma");
});

// [spec:hfst:req:javascript.lookup/test]
test("a limit caps the general lookup's results", async () => {
  for (const type of TYPES) {
    using t = pkg.readTransducer(await fixture(`ties.${type}.hfst`));
    const all = printed(t.lookup("a"));
    assertEqual(all.length, 5, `${type}: results without a limit`);
    for (const limit of [0, 1, 3]) {
      const capped = t.lookup("a", { limit });
      assertEqual(capped.length, limit, `${type}: results under limit ${limit}`);
      const known = new Set(all.map((result) => JSON.stringify(result)));
      assert(
        printed(capped).every((result) => known.has(JSON.stringify(result))),
        `${type}: every capped result is a result`,
      );
      assertLookup(capped, printed(capped), `${type}: capped results are in lookup's order`);
    }
  }
});

// [spec:hfst:req:javascript.lookup/test]
test("the general lookup has no time limit, so it returns every result under one", async () => {
  for (const type of TYPES) {
    using t = pkg.readTransducer(await fixture(`words.${type}.hfst`));
    assertEqual(
      JSON.stringify(t.lookup("mouse", { timeLimitMs: 1 })),
      JSON.stringify(t.lookup("mouse")),
      `${type}: mouse`,
    );
  }
});

// [spec:hfst:req:javascript.lookup/test]
test("a transducer converted to another type looks up as the one it came from", async () => {
  using source = pkg.readTransducer(await fixture("flags.optimized-lookup-weighted.hfst"));
  const expected = JSON.stringify(source.lookup("talo"));
  for (const type of ["openfst-tropical", "optimized-lookup-weighted"] as const) {
    using converted = source.convert(type);
    assertEqual(JSON.stringify(converted.lookup("talo")), expected, type);
  }
});

await runTests(args.label);
