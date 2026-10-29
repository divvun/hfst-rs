// Tokenising with the package as it ships, against what this checkout's
// hfst tokenise prints for the same text and flags. The cases, each a set of
// flags beside the options that stand for them, are in
// js/test/fixtures/tokenise-cases.json; js/test/tokenise-fixtures.mjs runs
// them through the command line.

import type {
  FormatOptions,
  Token,
  TokeniseFormat,
  TokeniseOptions,
} from "../hfst/index.ts";
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

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

interface Cases {
  texts: Record<string, string>;
  formats: { name: string; text: string; flags: string[]; format: TokeniseFormat; options: FormatOptions }[];
  tokens: { name: string; text: string; flags: string[]; options: TokeniseOptions }[];
}

const cases: Cases = JSON.parse(new TextDecoder().decode(await fixture("tokenise-cases.json")));
const archive = await fixture("tokenise.pmhfst");

function textOf(name: string): string {
  const text = cases.texts[name];
  assert(text !== undefined, `the cases have a text called ${name}`);
  return text;
}

/** Fails with the first place two strings differ, rather than both whole. */
function assertSameText(actual: string, expected: string, what: string): void {
  if (actual === expected) return;
  let at = 0;
  while (at < actual.length && actual[at] === expected[at]) at += 1;
  const around = (s: string) => JSON.stringify(s.slice(Math.max(0, at - 40), at + 40));
  throw new Error(`${what}: differs at ${at}: expected ${around(expected)}, got ${around(actual)}`);
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
}

/**
 * The cohorts `hfst tokenise --cg --print-weights` prints for `tokens`. A
 * token with no readings is a stretch `printAll` kept, which the command line
 * prints as unknown with no blank line after it.
 */
function cg(tokens: Token[]): string {
  const escaped = (s: string) => s.replaceAll("\\", "\\\\");
  let printed = "";
  for (const { surface, readings } of tokens) {
    printed += `"<${escaped(surface)}>"\n`;
    if (readings.length === 0) {
      printed += `\t"${escaped(surface)}" ?\n`;
      continue;
    }
    for (const { analysis, weight } of readings) {
      printed += analysis.startsWith(surface)
        ? `\t"${escaped(surface)}"${analysis.slice(surface.length)}`
        : `\t${analysis}`;
      printed += `\t${weight.toFixed(10)}\n`;
    }
    printed += "\n";
  }
  return printed;
}

function find(tokens: Token[], surface: string): Token {
  const token = tokens.find((t) => t.surface === surface);
  assert(token !== undefined, `a token ${surface} among ${tokens.map((t) => t.surface).join("|")}`);
  return token;
}

function readingsOf(token: Token): string {
  return token.readings.map((r) => `${r.analysis}/${r.weight}`).join(" ");
}

// [spec:hfst:req:javascript.tokenise/test]
test("format prints what hfst tokenise prints, byte for byte, in every format", async () => {
  const everyFormat: Record<TokeniseFormat, boolean> = {
    segment: false,
    "space-separated": false,
    xerox: false,
    cg: false,
    finnpos: false,
    "giella-cg": false,
    conllu: false,
    visl: false,
  };
  using tokeniser = pkg.readTokeniser(archive);
  for (const { name, text, flags, format, options } of cases.formats) {
    const expected = await fixture(`tokenise.${name}.out`);
    const actual = tokeniser.format(textOf(text), format, options);
    if (!bytesEqual(new TextEncoder().encode(actual), expected)) {
      assertSameText(actual, new TextDecoder().decode(expected), `${name} (hfst tokenise ${flags.join(" ")})`);
      throw new Error(`${name}: the same text in different bytes`);
    }
    everyFormat[format] = true;
  }
  for (const [format, seen] of Object.entries(everyFormat)) {
    assert(seen, `a case prints ${format}`);
  }
});

// [spec:hfst:req:javascript.tokenise/test]
test("tokens hold the cohorts and weighted readings hfst tokenise --cg --print-weights prints", async () => {
  using tokeniser = pkg.readTokeniser(archive);
  for (const { name, text, flags, options } of cases.tokens) {
    const expected = new TextDecoder().decode(await fixture(`tokenise.${name}.out`));
    assertSameText(cg(tokeniser.tokenise(textOf(text), options)), expected, `${name} (hfst tokenise ${flags.join(" ")})`);
  }
});

// [spec:hfst:req:javascript.tokenise/test]
test("UTF-16 offsets pick out each token's surface, in text order, through astral characters", () => {
  using tokeniser = pkg.readTokeniser(archive);
  for (const [name, text] of Object.entries(cases.texts)) {
    const tokens = tokeniser.tokenise(text, { printAll: true });
    let end = 0;
    for (const token of tokens) {
      assertEqual(text.slice(token.start, token.end), token.surface, `${name}: the text at ${token.start}`);
      assertEqual(token.start, end, `${name}: ${token.surface} starts where the token before it ends`);
      end = token.end;
    }
    assertEqual(end, text.length, `${name}: printAll covers the whole text`);
  }
  const line = textOf("line");
  const tokens = tokeniser.tokenise(line);
  const cat = find(tokens, "🐱");
  assertEqual(cat.start, line.indexOf("🐱"), "the cat's start");
  assertEqual(cat.end - cat.start, 2, "an astral character is two UTF-16 code units");
  const word = find(tokens, "blåbær");
  assertEqual(word.start, line.indexOf("blåbær"), "a word with non-ASCII letters starts where indexOf says");
  assertEqual(word.end, word.start + "blåbær".length, "and ends after them");
  const unmatched = find(tokeniser.tokenise(line, { printAll: true }), " 🐱🐱");
  assertEqual(unmatched.start, line.lastIndexOf(" "), "two cats with no space between are unmatched, with the space before them");
  assertEqual(unmatched.end, line.lastIndexOf("."), "and the stretch ends before the full stop");
});

// [spec:hfst:req:javascript.tokenise/test]
test("tokens are plain objects: multiword, weighted, analysed and unknown", () => {
  using tokeniser = pkg.readTokeniser(archive);
  const tokens = tokeniser.tokenise(textOf("line"));
  for (const token of tokens) {
    assertEqual(Object.getPrototypeOf(token), Object.prototype, "a token is a plain object");
    assertEqual(Object.keys(token).join(), "surface,start,end,analysed,readings", "its fields");
    for (const reading of token.readings) {
      assertEqual(Object.keys(reading).join(), "analysis,weight", "a reading's fields");
    }
  }
  assertEqual(readingsOf(find(tokens, "New York")), "New York+Prop/0", "a multiword token");
  assertEqual(readingsOf(find(tokens, "ice cream")), "ice cream+N/0", "another");
  assertEqual(
    readingsOf(find(tokens, "saw")),
    `see+V+Past/0 saw+N/${Math.fround(0.1)} saw ??/${Math.fround(100.000389)}`,
    "readings in weight order, with the library's single-precision weights",
  );
  const known = find(tokens, "cat");
  assert(known.analysed, "a word with an analysis is analysed");
  assertEqual(readingsOf(known), "cat+N/0 cat ??/100", "and keeps its unknown reading");
  const unknown = find(tokens, "blorf");
  assert(!unknown.analysed, "a word with only an unknown reading is not analysed");
  assertEqual(readingsOf(unknown), "blorf ??/100", "its reading");
  assert(find(tokens, ",").analysed, "punctuation is analysed");
  const stretch = find(tokeniser.tokenise(textOf("line"), { printAll: true }), " ");
  assert(!stretch.analysed && stretch.readings.length === 0, "an unmatched stretch has no readings");
});

// [spec:hfst:req:javascript.tokenise/test]
test("the options change which tokens and readings come back", () => {
  using tokeniser = pkg.readTokeniser(archive);
  const line = textOf("line");
  const surfaces = (tokens: Token[]) => tokens.map((t) => t.surface).join("|");
  const plain = tokeniser.tokenise(line);

  assertEqual(readingsOf(find(plain, "dog")), "dog+N/0 dog ??/100", "dedupe drops the second dog+N");
  const duplicates = tokeniser.tokenise(line, { dedupe: false });
  assertEqual(readingsOf(find(duplicates, "dog")), "dog+N/0 dog+N/0 dog ??/100", "dedupe off keeps it");

  const oneClass = tokeniser.tokenise(line, { maxWeightClasses: 1 });
  assertEqual(readingsOf(find(oneClass, "saw")), "see+V+Past/0", "one weight class");
  assertEqual(readingsOf(find(oneClass, "cat")), "cat+N/0 cat ??/100", "an unknown reading counts toward no class");

  const beam = tokeniser.tokenise(line, { beam: 0.05 });
  assertEqual(readingsOf(find(beam, "saw")), "see+V+Past/0", "the beam keeps readings near the best");
  assertEqual(readingsOf(find(beam, "blorf")), "blorf ??/100", "and the best is always within it");

  const cutoff = tokeniser.tokenise(line, { weightCutoff: 50 });
  assert(cutoff.every((t) => t.readings.every((r) => r.weight <= 50)), "no reading is heavier than the cutoff");
  assert(!surfaces(cutoff).includes("blorf"), `an unknown word is no token: ${surfaces(cutoff)}`);
  assertEqual(readingsOf(find(cutoff, "saw")), `see+V+Past/0 saw+N/${Math.fround(0.1)}`, "lighter readings stay");

  const all = tokeniser.tokenise(line, { printAll: true });
  assertEqual(all.filter((t) => t.surface === " ").length, 11, "printAll keeps the spaces between tokens");
  assertEqual(surfaces(all.filter((t) => t.readings.length > 0)), surfaces(plain), "and the same tokens");

  const timed = tokeniser.tokenise(line, { timeLimit: 60_000 });
  assertEqual(JSON.stringify(timed), JSON.stringify(plain), "a time limit that does not pass changes nothing");
  assertEqual(JSON.stringify(tokeniser.tokenise(line)), JSON.stringify(plain), "a tokeniser keeps no state between calls");
});

// [spec:hfst:req:javascript.tokenise/test]
test("bytes that are not a pmatch container throw invalid-format", async () => {
  for (const name of ["words.optimized-lookup-weighted.hfst", "words.openfst-tropical.hfst"]) {
    const bytes = await fixture(name);
    let message = "";
    try {
      pkg.readTokeniser(bytes);
    } catch (error) {
      assert(error instanceof pkg.HfstError, `an HfstError for ${name}`);
      assertEqual(error.code, "invalid-format", `${name}: code`);
      message = error.message;
    }
    assert(message.includes("pmatch container"), `${name}: the message says what was wanted: ${message}`);
  }
  for (const [what, bytes] of [
    ["empty", new Uint8Array(0)],
    ["truncated", archive.subarray(0, archive.length - 9)],
    ["text", new TextEncoder().encode("define TOP {cat} ;")],
  ] as const) {
    assertEqual(thrownCode(pkg.HfstError, () => pkg.readTokeniser(bytes)), "invalid-format", what);
  }
  const notBytes = [1, 2, 3] as unknown as Uint8Array;
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readTokeniser(notBytes)), "invalid-argument", "an array");
});

// [spec:hfst:req:javascript.tokenise/test]
test("option values the command line turns down throw invalid-argument", () => {
  using tokeniser = pkg.readTokeniser(archive);
  const bad: [string, unknown][] = [
    ["no weight classes", { maxWeightClasses: 0 }],
    ["a fraction of a weight class", { maxWeightClasses: 1.5 }],
    ["a negative beam", { beam: -1 }],
    ["a negative time limit", { timeLimit: -5 }],
    ["a NaN cutoff", { weightCutoff: Number.NaN }],
    ["a string for a flag", { dedupe: "yes" }],
    ["a string for a number", { beam: "0.5" }],
    ["options that are not an object", 5],
  ];
  for (const [what, options] of bad) {
    const call = () => tokeniser.tokenise("cat", options as TokeniseOptions);
    assertEqual(thrownCode(pkg.HfstError, call), "invalid-argument", what);
  }
  const formatCall = () => tokeniser.format("cat", "giellacg" as TokeniseFormat);
  assertEqual(thrownCode(pkg.HfstError, formatCall), "invalid-argument", "an unknown format");
  const notText = 42 as unknown as string;
  assertEqual(thrownCode(pkg.HfstError, () => tokeniser.tokenise(notText)), "invalid-argument", "text");
  assertEqual(tokeniser.format("cat", "segment", { beam: undefined }), "cat\n\n", "undefined leaves the default");
});

// [spec:hfst:req:javascript.tokenise/test]
test("a freed tokeniser throws freed, and freeing twice does nothing", () => {
  const tokeniser = pkg.readTokeniser(archive);
  assert(tokeniser instanceof pkg.Tokeniser, "readTokeniser gives a Tokeniser");
  tokeniser.free();
  tokeniser.free();
  assertEqual(thrownCode(pkg.HfstError, () => tokeniser.tokenise("cat")), "freed", "tokenise");
  assertEqual(thrownCode(pkg.HfstError, () => tokeniser.format("cat", "cg")), "freed", "format");
});

await runTests(args.label);
