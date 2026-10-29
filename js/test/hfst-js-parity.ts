// Lookup parity with hfst-js 1.4.0, the package @divvun/hfst replaces,
// against the package as it ships. Where the two disagree, our results must be
// what the reference C++ hfst-lookup prints, and the disagreement one of the
// kinds js/hfst/MIGRATING.md lists (js/test/hfst-js-compare.ts).
//
// Two variants:
//
// - `fixtures` runs with the suite, on the lookup fixtures and those
//   js/test/hfst-js-fixtures.mjs builds. When the reference binary is not
//   installed, the reference leg answers from js/test/fixtures/
//   hfst-js-reference.json, recorded with C++ HFST 3.16.2; when it is, the
//   recording must match it. HFST_JS_RECORD_REFERENCE=1 rewrites the
//   recording instead.
// - `analysers` runs on real analysers, when HFST_JS_PARITY lists them as
//   `transducer.hfstol=words.txt` entries separated as PATH separates
//   directories. It needs the reference binary. js/bench/words.mjs makes word
//   lists.
//
// HFST_LOOKUP_REFERENCE names the reference binary; the default is
// /opt/hfst/bin/hfst-lookup.

import {
  type Disagreement,
  describe,
  type Kind,
  KINDS,
  liveReference,
  loadHfstJs,
  ParityCheck,
  type Reference,
  recordedReference,
  referenceBinary,
  referenceVersion,
  tally,
  withNonWords,
} from "./hfst-js-compare.ts";
import { linesOf } from "./lookup-expect.ts";
import { assert, assertEqual, loadPackage, readBytes, runTests, scenarioArgs, test } from "./harness.ts";

const args = scenarioArgs();
const pkg = await loadPackage(args);
await pkg.init(await readBytes(args.wasm));
const hfstJs = await loadHfstJs();
const binary = referenceBinary();
const version = await referenceVersion(binary);

interface NodeUrl {
  fileURLToPath(url: string): string;
  pathToFileURL(path: string): URL;
}

interface NodeFs {
  readFileSync(path: string, encoding: "utf8"): string;
  writeFileSync(path: string, text: string): void;
}

const urlSpecifier = "node:url";
const fsSpecifier = "node:fs";
const nodeUrl: NodeUrl = await import(urlSpecifier);
const nodeFs: NodeFs = await import(fsSpecifier);

function env(name: string): string | undefined {
  const value: unknown = Reflect.get(Reflect.get(Reflect.get(globalThis, "process"), "env"), name);
  return typeof value === "string" && value !== "" ? value : undefined;
}

function summary(disagreements: readonly Disagreement[], agreed: number): string {
  const { kinds } = tally(disagreements);
  const counts = KINDS.filter((kind) => kinds[kind] !== undefined).map((kind) => `${kind} ${kinds[kind]}`);
  return `${agreed} agree with hfst-js${counts.length > 0 ? `; ${counts.join(", ")}` : ""}`;
}

function assertClassified(disagreements: readonly Disagreement[], what: string): void {
  const { unclassified } = tally(disagreements);
  assert(
    unclassified.length === 0,
    `${what}: ${unclassified.length} disagreements of no accepted kind:\n${unclassified.slice(0, 10).map(describe).join("\n")}`,
  );
}

const READ = ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const;

async function fixtureText(name: string): Promise<string> {
  return new TextDecoder().decode(await readBytes(new URL(name, args.fixtures).href));
}

const lookupInputs = linesOf(await fixtureText("lookup-inputs.txt"));

const FIXTURES: { source: string; types: readonly string[]; inputs: string[] }[] = [
  { source: "words", types: READ, inputs: [...lookupInputs, "cat☃", "catz", "mouse\u{1f600}mouse", "ca+Plz"] },
  { source: "flags", types: READ, inputs: [...lookupInputs, "talo☃"] },
  { source: "ties", types: READ, inputs: [...lookupInputs, "abq", "xyab"] },
  { source: "many", types: ["optimized-lookup-weighted"], inputs: ["aaaaaaa", "aaa"] },
  { source: "weights", types: READ, inputs: ["aaaaa", "aaaa", "x", "xa"] },
  {
    source: "nonascii",
    types: READ,
    inputs: ["čáhci", "ŋuolggat", "ĸ", "\u{1f600}", "á", "čáhci".normalize("NFD"), "á".normalize("NFD"), "čáhci☃"],
  },
  { source: "identity", types: READ, inputs: ["=abc", "=", "=☃", "abc"] },
  { source: "prefix", types: READ, inputs: ["ab", "abc", "a", "abcab"] },
  { source: "shortflag", types: READ, inputs: ["k", "m"] },
  { source: "unify", types: READ, inputs: ["ua", "ub", "pa"] },
];

/** The fixtures whose symbols the join check reads: flags, epsilons, multichar symbols and non-ASCII. */
const JOINED = ["words", "flags", "ties", "weights", "nonascii"];

const recordingPath = nodeUrl.fileURLToPath(new URL("fixtures/hfst-js-reference.json", import.meta.url).href);

interface Recording {
  reference: string;
  results: Record<string, Record<string, string[]>>;
}

function readRecording(): Recording {
  const recording: Recording = JSON.parse(nodeFs.readFileSync(recordingPath, "utf8"));
  return recording;
}

/** A reference that keeps every answer it gives in `answers`. */
function keeping(reference: Reference, answers: Record<string, string[]>): Reference {
  return {
    async lookup(inputs) {
      const results = await reference.lookup(inputs);
      inputs.forEach((input, i) => {
        answers[input] = [...(results[i] ?? [])].sort();
      });
      return results;
    },
  };
}

if (args.variant === "fixtures") {
  /** The live reference's answers for each fixture, filled in by the first test. */
  const live: Record<string, Record<string, string[]>> = {};

  // [spec:hfst:req:javascript.hfst-js-parity+2/test]
  test("on every fixture, lookup equals hfst-js, or the C++ reference where hfst-js departs from it", async () => {
    if (version === null) {
      console.log(`# ${binary} is not installed: the reference leg answers from ${recordingPath}`);
    }
    const recording = version === null ? readRecording() : null;
    const seen = new Set<Kind>();
    const all: Disagreement[] = [];
    let agreedTotal = 0;
    for (const { source, types, inputs } of FIXTURES) {
      for (const type of types) {
        const name = `${source}.${type}.hfst`;
        const url = new URL(name, args.fixtures).href;
        const bytes = await readBytes(url);
        using ours = pkg.readTransducer(bytes);
        let reference: Reference;
        if (recording === null) {
          live[name] = {};
          reference = keeping(liveReference(binary, nodeUrl.fileURLToPath(url)), live[name]);
        } else {
          reference = recordedReference(recording.results[name] ?? {}, name);
        }
        const check = new ParityCheck(hfstJs, bytes, ours, reference);
        const { agreed, disagreements } = await check.compare(inputs);
        assertClassified(disagreements, name);
        for (const { kind } of disagreements) if (kind !== null) seen.add(kind);
        all.push(...disagreements);
        agreedTotal += agreed;
      }
    }
    console.log(`# fixtures: ${summary(all, agreedTotal)}`);
    for (const kind of KINDS) assert(seen.has(kind), `no fixture shows the ${kind} disagreement`);
  });

  // [spec:hfst:req:javascript.hfst-js-parity+2/test]
  test("the recorded reference results are what C++ hfst-lookup prints for the fixtures", () => {
    if (version === null) {
      console.log(`# skipped: ${binary} is not installed`);
      return;
    }
    const recorded: Recording = { reference: version, results: live };
    const text = `${JSON.stringify(recorded, null, 2)}\n`;
    if (env("HFST_JS_RECORD_REFERENCE") !== undefined) {
      nodeFs.writeFileSync(recordingPath, text);
      console.log(`# wrote ${recordingPath}`);
      return;
    }
    assertEqual(
      JSON.stringify(readRecording(), null, 2),
      JSON.stringify(recorded, null, 2),
      `${recordingPath}, against ${binary} (HFST_JS_RECORD_REFERENCE=1 rewrites it)`,
    );
  });

  // [spec:hfst:req:javascript.hfst-js-parity+2/test]
  test("joining our symbols gives hfst-js's analysis, on the flag, multichar and non-ASCII fixtures", async () => {
    for (const { source, types, inputs } of FIXTURES.filter(({ source }) => JOINED.includes(source))) {
      for (const type of types) {
        const name = `${source}.${type}.hfst`;
        const bytes = await readBytes(new URL(name, args.fixtures).href);
        using ours = pkg.readTransducer(bytes);
        const check = new ParityCheck(hfstJs, bytes, ours, recordedReference({}, name));
        for (const input of inputs) {
          const what = `${name} ${JSON.stringify(input)}`;
          for (const { output, symbols } of ours.lookup(input)) {
            assertEqual(symbols.join(""), output, `${what}: our output is our symbols joined`);
          }
          if (check.agrees(input)) assert(check.symbolsAgree(input), `${what}: the symbols differ`);
        }
      }
    }
  });
} else if (args.variant === "analysers") {
  const listed = env("HFST_JS_PARITY");
  if (listed === undefined) {
    test("real analysers # SKIP HFST_JS_PARITY lists none", () => {});
  } else {
    const separator = Reflect.get(Reflect.get(globalThis, "process"), "platform") === "win32" ? ";" : ":";
    for (const entry of listed.split(separator).filter((entry) => entry !== "")) {
      const [transducer, words] = entry.split("=");
      // [spec:hfst:req:javascript.hfst-js-parity+2/test]
      test(`${transducer}: lookup equals hfst-js, or the C++ reference where hfst-js departs from it`, async () => {
        assert(
          transducer !== undefined && words !== undefined,
          `HFST_JS_PARITY entries are transducer.hfstol=words.txt, not ${JSON.stringify(entry)}`,
        );
        assert(version !== null, `the real-analyser check needs the reference ${binary}`);
        const bytes = await readBytes(nodeUrl.pathToFileURL(transducer).href);
        using ours = pkg.readTransducer(bytes);
        const list = [...new Set(linesOf(nodeFs.readFileSync(words, "utf8")).filter((word) => word !== ""))];
        const inputs = withNonWords(list);
        const check = new ParityCheck(hfstJs, bytes, ours, liveReference(binary, transducer));
        const started = performance.now();
        const { agreed, disagreements } = await check.compare(inputs);
        const seconds = ((performance.now() - started) / 1000).toFixed(1);
        console.log(
          `# ${transducer}: ${list.length} words and ${inputs.length - list.length} non-words, ${seconds} s: ${summary(disagreements, agreed)}`,
        );
        for (const kind of KINDS) {
          const example = disagreements.find((disagreement) => disagreement.kind === kind);
          if (example !== undefined) console.log(`#   ${describe(example)}`);
        }
        assertClassified(disagreements, transducer);
      });
    }
  }
}

await runTests(args.label);
