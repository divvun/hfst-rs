// Measures one transducer in @divvun/hfst and in hfst-js, in a process of
// its own, and prints the figures as one line of JSON. js/bench/bench.mjs runs
// it under each runtime; it takes its settings as JSON in its first argument:
// { transducer, words, loads, passes, sample }.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { HfstInputStream } from "./hfst-js.mjs";

const js = dirname(dirname(fileURLToPath(import.meta.url)));
const { transducer, words: wordsPath, loads, passes, sample } = JSON.parse(process.argv[2]);

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
};

const time = (f) => {
  const start = performance.now();
  f();
  return performance.now() - start;
};

const hfst = await import(join(js, "hfst", "dist", "index.js"));
const wasm = readFileSync(join(js, "hfst", "dist", "pkg", "hfst_wasm_bg.wasm"));
const initStart = performance.now();
await hfst.init(wasm);
const initMs = performance.now() - initStart;

const bytes = new Uint8Array(readFileSync(transducer));
const words = [...new Set(readFileSync(wordsPath, "utf8").split("\n").filter((word) => word !== ""))].slice(0, sample);

// Loads alternate between the libraries, so a slow spell of the machine falls
// on both rather than on one.
const oursLoad = [];
const theirsLoad = [];
for (let i = 0; i < loads; i += 1) {
  oursLoad.push(time(() => hfst.readTransducer(bytes).free()));
  theirsLoad.push(time(() => new HfstInputStream(bytes, false).read()));
}

// One untimed pass warms both libraries' code and fills hfst-js's cache.
const ours = hfst.readTransducer(bytes);
const uncached = new HfstInputStream(bytes, false).read();
const cached = new HfstInputStream(bytes, true).read();
let analyses = 0;
for (const word of words) {
  analyses += ours.lookup(word).length;
  cached.lookup(word);
}
const oursPass = [];
const theirsPass = [];
const cachedPass = [];
for (let i = 0; i < passes; i += 1) {
  oursPass.push(time(() => words.forEach((word) => ours.lookup(word))));
  theirsPass.push(time(() => words.forEach((word) => uncached.lookup(word))));
  cachedPass.push(time(() => words.forEach((word) => cached.lookup(word))));
}
ours.free();

const perWordUs = (passMs) => (1000 * median(passMs)) / words.length;
console.log(
  JSON.stringify({
    transducer,
    bytes: bytes.length,
    words: words.length,
    analyses,
    initMs,
    loadMs: { ours: median(oursLoad), hfstJs: median(theirsLoad) },
    lookupUs: { ours: perWordUs(oursPass), hfstJs: perWordUs(theirsPass), hfstJsCached: perWordUs(cachedPass) },
  }),
);
