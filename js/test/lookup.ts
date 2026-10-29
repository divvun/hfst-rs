// Lookup on optimized-lookup transducers, against the package as it ships.
// Each result list is compared with what this checkout's `hfst lookup` printed
// for the same transducer, input and limit (js/test/lookup-fixtures.mjs).

import { assertLookup, linesOf, parsePrint, printed } from "./lookup-expect.ts";
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

const args = scenarioArgs();
const pkg = await loadPackage(args);
await pkg.init(await readBytes(args.wasm));

const TYPES = ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const;
const SOURCES = ["words", "flags", "ties"] as const;

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

async function text(name: string): Promise<string> {
  return new TextDecoder().decode(await fixture(name));
}

const inputs = linesOf(await text("lookup-inputs.txt"));

function summary(results: readonly { output: string; symbols: string[]; weight: number }[]): string {
  return results.map(({ output, symbols, weight }) => `${weight} ${output} [${symbols.join(" ")}]`).join("; ");
}

// [spec:hfst:req:javascript.lookup/test]
test("optimized lookup returns what hfst lookup prints, for every input", async () => {
  for (const source of SOURCES) {
    for (const type of TYPES) {
      using t = pkg.readTransducer(await fixture(`${source}.${type}.hfst`));
      const expected = parsePrint(await text(`lookup.${source}.${type}.txt`));
      assertEqual(expected.length, inputs.length, `${source} ${type}: inputs printed`);
      inputs.forEach((input, i) => {
        assertLookup(t.lookup(input), expected[i] ?? [], `${source} ${type} ${JSON.stringify(input)}`);
      });
    }
  }
});

// [spec:hfst:req:javascript.lookup/test]
test("a limit caps the results as hfst lookup -n does", async () => {
  for (const source of SOURCES) {
    for (const type of TYPES) {
      using t = pkg.readTransducer(await fixture(`${source}.${type}.hfst`));
      const expected = parsePrint(await text(`lookup.${source}.${type}.limit1.txt`));
      inputs.forEach((input, i) => {
        const what = `${source} ${type} ${JSON.stringify(input)} limit 1`;
        assertLookup(t.lookup(input, { limit: 1 }), expected[i] ?? [], what);
      });
      assertEqual(t.lookup("a", { limit: 0 }).length, 0, `${source} ${type}: limit 0`);
    }
  }
  using words = pkg.readTransducer(await fixture("words.optimized-lookup-weighted.hfst"));
  assertEqual(words.lookup("mouse").length, 2, "mouse without a limit");
  assertEqual(words.lookup("mouse", { limit: 1 }).length, 1, "mouse with limit 1");
});

// [spec:hfst:req:javascript.lookup/test]
test("flag diacritics block a path and are not among the symbols", async () => {
  using weighted = pkg.readTransducer(await fixture("flags.optimized-lookup-weighted.hfst"));
  assertEqual(
    summary(weighted.lookup("talo")),
    "0.5 talo+N+Nom [t a l o +N +Nom]; 1.5 talo+N+Gen [t a l o +N +Gen]",
    "weighted",
  );
  using unweighted = pkg.readTransducer(await fixture("flags.optimized-lookup-unweighted.hfst"));
  assertEqual(
    summary(unweighted.lookup("talo")),
    "0 talo+N+Gen [t a l o +N +Gen]; 0 talo+N+Nom [t a l o +N +Nom]",
    "unweighted",
  );
});

// [spec:hfst:req:javascript.lookup/test]
test("ties on weight are ordered by output, then by symbols, and multichar symbols stay whole", async () => {
  using ties = pkg.readTransducer(await fixture("ties.optimized-lookup-weighted.hfst"));
  assertEqual(
    summary(ties.lookup("a")),
    "0.25 ab [a b]; 0.25 c [c]; 1 b [b]; 1 xy [x y]; 1 xy [xy]",
    "a",
  );
  assertEqual(summary(ties.lookup("ab")), "2 r [r]", "ab reads as the multichar symbol ab");
  using words = pkg.readTransducer(await fixture("words.optimized-lookup-weighted.hfst"));
  assertEqual(summary(words.lookup("ca+Pl")), "0 ca+Pl [c a +Pl]", "ca+Pl");
});

// [spec:hfst:req:javascript.lookup/test]
test("empty input, input with no analyses and input in foreign symbols give empty arrays", async () => {
  for (const type of TYPES) {
    using t = pkg.readTransducer(await fixture(`words.${type}.hfst`));
    for (const input of ["", "mice", "zzz", "ølø", "\ud800"]) {
      const results = t.lookup(input);
      assert(Array.isArray(results) && results.length === 0, `${type} ${JSON.stringify(input)}: ${summary(results)}`);
    }
  }
});

// [spec:hfst:req:javascript.lookup/test]
test("repeated lookups on one transducer give the same results", async () => {
  using t = pkg.readTransducer(await fixture("words.optimized-lookup-weighted.hfst"));
  const first = JSON.stringify(t.lookup("mouse"));
  for (const input of ["ølø", "zzz", "cat", "mouse", "ølø"]) t.lookup(input);
  assertEqual(JSON.stringify(t.lookup("mouse")), first, "mouse after other lookups");
  assertEqual(JSON.stringify(t.lookup("mouse", { timeLimitMs: 0 })), first, "a time limit of 0 sets none");
});

// [spec:hfst:req:javascript.lookup/test]
test("a 1 ms time limit returns fewer than all of many analyses, and does not throw", async () => {
  using many = pkg.readTransducer(await fixture("many.optimized-lookup-weighted.hfst"));
  const input = "aaaaaaa";
  const all = many.lookup(input);
  assertEqual(all.length, 4 ** 7, "analyses without a time limit");
  const outputs = new Set(all.map((result) => result.output));
  const timed = many.lookup(input, { timeLimitMs: 1 });
  assert(timed.length < all.length, `a 1 ms lookup found ${timed.length} of ${all.length}`);
  assert(
    timed.every((result) => outputs.has(result.output) && result.symbols.length === 7),
    "every result found in time is an analysis",
  );
  assertLookup(timed, printed(timed), "the results found in time are in lookup's order");
});

// [spec:hfst:req:javascript.lookup/test]
test("options lookup cannot use throw invalid-argument, and a freed transducer throws freed", async () => {
  const t = pkg.readTransducer(await fixture("words.optimized-lookup-weighted.hfst"));
  const bad = [{ limit: -1 }, { limit: 0.5 }, { timeLimitMs: Number.NaN }, { timeLimitMs: -2 }];
  for (const options of bad) {
    assertEqual(thrownCode(pkg.HfstError, () => t.lookup("cat", options)), "invalid-argument", JSON.stringify(options));
  }
  assertEqual(summary(t.lookup("cat")), "0.5 dog [d o g]", "the transducer still looks up");
  t.free();
  assertEqual(thrownCode(pkg.HfstError, () => t.lookup("cat")), "freed", "lookup after free");
});

await runTests(args.label);
