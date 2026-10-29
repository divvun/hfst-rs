// The native expectations of the parity suite (js/test/parity.ts): for every
// shared fixture, what the native hfst library gives, in parity.json beside
// the fixtures. Lookups, tokens, DHFST acceptor lookups and DHFST metadata
// come from the library itself, through the example
// crates/hfst/examples/js_parity_expected.rs; the tokenise formats come from
// this checkout's hfst tokenise, whose output the package's format promises
// byte for byte. Only js/test/fixtures.mjs calls this, after every other
// fixture is built.

import { execFileSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

const READ = ["optimized-lookup-weighted", "optimized-lookup-unweighted"];

/**
 * Words for the grapheme acceptor and its transducer, written from escapes
 * because a text file's normalisation form does not survive every editor:
 * multicharacter symbols spelt whole and in parts, and é and ö composed and
 * decomposed.
 */
const GRAPHEME_WORDS = ["ija", "ij", "i", "ja", "xy", "x", "\u00e9", "e\u0301", "o\u0308", "\u00f6", ""];

/** The text the animals container tokenises, with each of its patterns and a word none matches. */
const ANIMALS = "a cat, a dog and a mouse\n";

/** The tokeniser options each text is tokenised under, besides none. */
const TOKEN_OPTIONS = [
  { printAll: true },
  { dedupe: false },
  { maxWeightClasses: 1 },
  { tokenizeMultichar: true },
  { weightCutoff: 50 },
];

function linesOf(text) {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

function optionsName(options) {
  return Object.entries(options)
    .map(([key, value]) => `${key}=${value}`)
    .join(",");
}

/** Writes `out/parity.json`, running the hfst tools at `hfst` and the library example with cargo. */
export function buildParityFixtures(hfst, out) {
  const at = (name) => join(out, name);
  const lines = (name) => linesOf(readFileSync(at(name), "utf8"));
  const lookupInputs = lines("lookup-inputs.txt");
  const spellerInputs = lines("speller-inputs.txt");
  const cases = JSON.parse(readFileSync(at("tokenise-cases.json"), "utf8"));

  const lookup = (file, inputs) => ({ file, inputs, limits: [null, 1] });
  const request = {
    lookup: [
      ...["words", "flags", "ties"].flatMap((source) =>
        READ.map((type) => lookup(`${source}.${type}.hfst`, lookupInputs)),
      ),
      ...READ.map((type) => lookup(`words.${type}.bare`, lookupInputs)),
      ...["first.hfst", "second.hfst", "ordered.hfst", "ordered.bare"].map((file) => lookup(file, lookupInputs)),
      lookup("many.optimized-lookup-weighted.hfst", ["aaaaaaa", "aaa", "a", "b", ""]),
      lookup("speller.optimized-lookup-weighted.hfst", spellerInputs),
      lookup("graphemes.optimized-lookup-weighted.hfst", GRAPHEME_WORDS),
      lookup("animals.pmhfst", [...lookupInputs, "dog"]),
      lookup("tokenise.pmhfst", ["cat", "saw", "dog", "New York", "ice cream", "blorf", "Øl", "🐱", ","]),
    ],
    tokens: [
      ...Object.entries(cases.texts).flatMap(([name, text]) =>
        [{}, ...TOKEN_OPTIONS].map((options) => ({
          file: "tokenise.pmhfst",
          name: `${name} ${optionsName(options)}`.trim(),
          text,
          options,
        })),
      ),
      ...[{}, { printAll: true }].map((options) => ({
        file: "animals.pmhfst",
        name: `animals ${optionsName(options)}`.trim(),
        text: ANIMALS,
        options,
      })),
    ],
    acceptors: [
      { file: "speller.acceptor.dhfst", words: spellerInputs },
      { file: "graphemes.acceptor.dhfst", words: GRAPHEME_WORDS },
    ],
    models: ["speller.acceptor.dhfst", "graphemes.acceptor.dhfst", "errmodel.error-model.dhfst"],
  };

  const requestFile = at("parity-request.json");
  const nativeFile = at("parity-native.json");
  writeFileSync(requestFile, JSON.stringify(request));
  const example = ["run", "--quiet", "-p", "hfst", "--example", "js_parity_expected", "--", out, requestFile, nativeFile];
  console.log(`$ cargo ${example.join(" ")}`);
  execFileSync("cargo", example, { cwd: root, stdio: ["ignore", "inherit", "inherit"] });
  const native = JSON.parse(readFileSync(nativeFile, "utf8"));
  rmSync(requestFile);
  rmSync(nativeFile);

  const printed = (file, flags, text) =>
    execFileSync(hfst, ["tokenise", ...flags, at(file)], { input: text, stdio: ["pipe", "pipe", "pipe"] }).toString(
      "utf8",
    );
  const formats = [
    ...cases.formats.map(({ name, text, format, options }) => ({
      file: "tokenise.pmhfst",
      name,
      text: cases.texts[text],
      format,
      options,
      printed: readFileSync(at(`tokenise.${name}.out`), "utf8"),
    })),
    ...[
      { name: "animals segment", flags: [], format: "segment", options: {} },
      { name: "animals xerox", flags: ["-x"], format: "xerox", options: {} },
      { name: "animals cg-weights", flags: ["-c", "-w"], format: "cg", options: { printWeights: true } },
      { name: "animals giella-cg", flags: ["-g"], format: "giella-cg", options: {} },
    ].map(({ name, flags, format, options }) => ({
      file: "animals.pmhfst",
      name,
      text: ANIMALS,
      format,
      options,
      printed: printed("animals.pmhfst", flags, ANIMALS),
    })),
  ];

  writeFileSync(at("parity.json"), JSON.stringify({ ...native, formats }));
}
