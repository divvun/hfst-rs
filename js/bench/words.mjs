// Makes the word lists the hfst-js comparison and benchmark run on
// (js/test/hfst-js-parity.ts, js/bench/bench.mjs). It prints one input per
// line, each once, in the order first met.
//
//   node js/bench/words.mjs surface FILE...           word forms in text files, and the surface forms of Giella yaml tests
//   node js/bench/words.mjs keys YAML...              the analyses Giella yaml tests give, as generator inputs
//   node js/bench/words.mjs analyses HFSTOL WORDS [N] the analyses a transducer gives for a word list, at most N of them
//   node js/bench/words.mjs paths HFST N              the input sides of N random paths through a transducer
//
// analyses runs @divvun/hfst from js/hfst/dist, and paths the checkout's hfst
// tool in target/debug, so both need `node js/build.mjs --test` first.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const js = dirname(dirname(fileURLToPath(import.meta.url)));

/** The `key: value` and `key: [value, ...]` entries under a Giella yaml test file's `Tests:`. */
export function yamlTests(text) {
  const entries = [];
  let inTests = false;
  for (const line of text.split("\n")) {
    if (/^\S/.test(line)) inTests = /^Tests:/.test(line);
    if (!inTests) continue;
    // A comment starts at a # after a space; a # inside an analysis is a compound boundary.
    const match = /^\s+(?:"([^"]+)"|([^\s#"][^:]*?))\s*:\s*(.+?)\s*$/.exec(line.replace(/\s+#.*$/, ""));
    if (match === null) continue;
    const value = match[3];
    const values = value.startsWith("[") ? value.replace(/^\[|\]$/g, "").split(",") : [value];
    entries.push({
      key: (match[1] ?? match[2]).trim(),
      values: values.map((v) => v.trim().replace(/^["']|["']$/g, "")).filter((v) => v !== ""),
    });
  }
  return entries;
}

/** Word forms in running text: runs of letters, marks and digits, with inner hyphens and apostrophes. */
export function textWords(text) {
  return text
    .split(/[^\p{L}\p{M}\p{N}'-]+/u)
    .map((word) => word.replace(/^['-]+|['-]+$/g, ""))
    .filter((word) => /[\p{L}\p{N}]/u.test(word));
}

async function analyses(transducerPath, wordsPath, most) {
  const hfst = await import(join(js, "hfst", "dist", "index.js"));
  await hfst.init(readFileSync(join(js, "hfst", "dist", "pkg", "hfst_wasm_bg.wasm")));
  using transducer = hfst.readTransducer(readFileSync(transducerPath));
  const found = new Set();
  for (const word of readFileSync(wordsPath, "utf8").split("\n")) {
    if (word === "") continue;
    for (const { output } of transducer.lookup(word)) {
      if (found.size >= most) return [...found];
      found.add(output);
    }
  }
  return [...found];
}

/**
 * The input sides of the `input:output` lines `hfst fst2strings` prints. A
 * line with another colon cannot be split safely, so it is left out.
 */
export function inputSides(printed) {
  return printed
    .split("\n")
    .map((line) => line.split(":"))
    .filter((sides) => sides.length === 2)
    .map(([input]) => input);
}

function paths(transducerPath, count) {
  const hfst = join(dirname(js), "target", "debug", "hfst");
  const printed = execFileSync(hfst, ["fst2strings", "-q", "-r", String(count), "-X", "obey-flags", transducerPath], {
    encoding: "utf8",
    maxBuffer: 1 << 30,
  });
  return inputSides(printed);
}

async function main([mode, ...rest]) {
  let words;
  if (mode === "surface") {
    words = rest.flatMap((file) => {
      const text = readFileSync(file, "utf8");
      return file.endsWith(".yaml") ? yamlTests(text).flatMap(({ values }) => values) : textWords(text);
    });
  } else if (mode === "keys") {
    words = rest.flatMap((file) => yamlTests(readFileSync(file, "utf8")).map(({ key }) => key));
  } else if (mode === "analyses" && rest.length >= 2) {
    words = await analyses(rest[0], rest[1], rest[2] === undefined ? Infinity : Number(rest[2]));
  } else if (mode === "paths" && rest.length === 2) {
    words = paths(rest[0], Number(rest[1]));
  } else {
    console.error("usage: words.mjs surface FILE... | keys YAML... | analyses HFSTOL WORDS [N] | paths HFST N");
    process.exit(2);
  }
  process.stdout.write([...new Set(words.filter((word) => word !== ""))].map((word) => `${word}\n`).join(""));
}

// The tests import this module for its functions, so it runs only when run.
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main(process.argv.slice(2));
}
