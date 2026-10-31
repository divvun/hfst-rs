// The native expectations of the optimised lookup (javascript.optimised-lookup):
// for every single-transducer optimized-lookup fixture, what native divvun-fst
// gives, through the example crates/hfst-wasm/examples/js_optimised_expected.rs.
// They go into parity.json as `optimised`, beside the hfst library's, so the
// parity scenario (js/test/parity.ts) runs them in every environment. Only
// js/test/fixtures.mjs calls this, last, once parity.json and every
// transducer fixture are built.

import { execFileSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { GRAPHEME_WORDS } from "./parity-fixtures.mjs";

const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

const READ = ["optimized-lookup-weighted", "optimized-lookup-unweighted"];

/** The limits each input is looked up under: none, and the lightest one and two. */
const LIMITS = [null, 1, 2];

/**
 * Inputs for the fixtures js/test/hfst-js-fixtures.mjs builds, written from
 * escapes because a text file's normalisation form does not survive every
 * editor: each source's words, decomposed spellings of its composed letters,
 * characters outside its alphabet, and a symbol that begins like a longer
 * one.
 */
const HFST_JS_INPUTS = {
  weights: ["aaaaa", "aaaa", "x", "xa"],
  nonascii: [
    "\u010d\u00e1hci",
    "\u014buolggat",
    "\u0138",
    "\u{1f600}",
    "\u00e1",
    "\u010d\u00e1hci".normalize("NFD"),
    "\u00e1".normalize("NFD"),
    "\u010d\u00e1hci\u2603",
  ],
  identity: ["=abc", "=", "=\u2603", "abc", "=\u00e1", "=a\u0301"],
  prefix: ["ab", "abc", "a", "abcab"],
  shortflag: ["k", "m"],
  unify: ["ua", "ub", "pa"],
};

function linesOf(text) {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

/**
 * Adds `optimised` to `out/parity.json`. Each entry names the file the
 * package loads and the file divvun-fst reads for it: the same transducer,
 * with the HFST3 header divvun-fst needs where the package's file has none.
 */
export function buildOptimisedFixtures(out) {
  const at = (name) => join(out, name);
  const lookupInputs = linesOf(readFileSync(at("lookup-inputs.txt"), "utf8"));
  const spellerInputs = linesOf(readFileSync(at("speller-inputs.txt"), "utf8"));
  const lookup = (file, inputs, reference = file) => ({ file, reference, inputs, limits: LIMITS });
  const request = {
    optimised: [
      ...["words", "flags", "ties"].flatMap((source) =>
        READ.map((type) => lookup(`${source}.${type}.hfst`, lookupInputs)),
      ),
      ...READ.map((type) => lookup(`words.${type}.bare`, lookupInputs, `words.${type}.hfst`)),
      ...["first.hfst", "second.hfst"].map((file) => lookup(file, lookupInputs)),
      lookup("many.optimized-lookup-weighted.hfst", ["aaaaaaa", "aaa", "a", "b", ""]),
      lookup("speller.optimized-lookup-weighted.hfst", spellerInputs),
      lookup("graphemes.optimized-lookup-weighted.hfst", GRAPHEME_WORDS),
      ...Object.entries(HFST_JS_INPUTS).flatMap(([source, inputs]) =>
        READ.map((type) => lookup(`${source}.${type}.hfst`, inputs)),
      ),
    ],
  };

  const requestFile = at("optimised-request.json");
  const nativeFile = at("optimised-native.json");
  writeFileSync(requestFile, JSON.stringify(request));
  const example = [
    "run",
    "--quiet",
    "-p",
    "hfst-wasm",
    "--example",
    "js_optimised_expected",
    "--",
    out,
    requestFile,
    nativeFile,
  ];
  console.log(`$ cargo ${example.join(" ")}`);
  execFileSync("cargo", example, { cwd: root, stdio: ["ignore", "inherit", "inherit"] });
  const native = JSON.parse(readFileSync(nativeFile, "utf8"));
  rmSync(requestFile);
  rmSync(nativeFile);

  const parity = JSON.parse(readFileSync(at("parity.json"), "utf8"));
  writeFileSync(at("parity.json"), JSON.stringify({ ...parity, optimised: native.optimised }));
}
