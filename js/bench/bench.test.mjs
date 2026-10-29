// Tests for how js/bench/bench.mjs names transducers, combines the figures of
// its processes and words its verdict.

import { assert, assertEqual, runTests, test } from "../test/harness.ts";
import { combined, label, verdict } from "./bench.mjs";

function result(transducer, loadMs, lookupUs) {
  return {
    transducer,
    bytes: 1,
    words: 500,
    analyses: 0,
    initMs: 1,
    loadMs: { ours: loadMs[0], hfstJs: loadMs[1] },
    lookupUs: { ours: lookupUs[0], hfstJs: lookupUs[1], hfstJsCached: 1 },
  };
}

test("a transducer is named by its language repository, or else its directory", () => {
  assertEqual(label("/home/u/git/lang-kal/src/fst/analyser-gt-desc.hfstol"), "lang-kal analyser-gt-desc", "lang");
  assertEqual(label("/data/se-zcheck/generator-gramcheck-gt-norm.hfstol"), "se-zcheck generator-gramcheck-gt-norm", "other");
});

test("each figure of several processes is their median, and the rest is the first's", () => {
  const all = combined([
    { ...result("a", [30, 10], [500, 200]), initMs: 9 },
    { ...result("a", [10, 30], [100, 400]), initMs: 1 },
    { ...result("a", [20, 20], [300, 300]), initMs: 5 },
  ]);
  assertEqual(JSON.stringify(all.loadMs), JSON.stringify({ ours: 20, hfstJs: 20 }), "load");
  assertEqual(JSON.stringify(all.lookupUs), JSON.stringify({ ours: 300, hfstJs: 300, hfstJsCached: 1 }), "lookup");
  assertEqual(all.initMs, 5, "init");
  assertEqual(all.transducer, "a", "transducer");
});

test("the verdict says plainly where ours is slower and names where it is faster", () => {
  const text = verdict([
    result("/x/lang-kal/a.hfstol", [20, 10], [300, 100]),
    result("/x/lang-gle/b.hfstol", [40, 10], [50, 100]),
    result("/x/lang-gle/c.hfstol", [30, 10], [25, 100]),
  ]);
  assertEqual(
    text,
    "@divvun/hfst loads every transducer more slowly than hfst-js, taking 2.0 to 4.0 times as long. " +
      "It looks words up more slowly than uncached hfst-js on 1 of 3 transducers, taking up to 3.0 times as long. " +
      "It is faster on lang-gle b and lang-gle c, in as little as 0.3 times the time.",
    "verdict",
  );
});

test("the verdict has no slower sentence when ours is faster everywhere, and counts slower loads", () => {
  const text = verdict([result("/x/lang-gle/b.hfstol", [5, 10], [50, 100]), result("/x/lang-gle/c.hfstol", [20, 10], [25, 100])]);
  assert(text.startsWith("@divvun/hfst loads 1 of 2 transducers more slowly than hfst-js, taking up to 2.0 times as long."), text);
  assert(!text.includes("more slowly than uncached"), text);
  assert(text.endsWith("It is faster on every transducer, in as little as 0.3 times the time."), text);
});

await runTests("bench/bench");
