// The optimised lookup, against the package as it ships: how its transducers
// load, what its lookup takes, and how its results compare with lookup's.
// The parity scenario (parity.ts) compares its results with native
// divvun-fst's in every environment; this one compares them with the hfst
// library's lookup, where every difference must be a kind
// js/test/optimised-compare.ts classifies.
//
// Two variants:
//
// - `main` runs with the suite, on the fixtures and inputs of parity.json's
//   optimised lookups.
// - `analysers` runs the comparison on real analysers, when HFST_JS_PARITY
//   lists them as `transducer.hfstol=words.txt` entries separated as PATH
//   separates directories, as js/test/hfst-js-parity.ts reads them.

import { withNonWords } from "./hfst-js-compare.ts";
import { linesOf } from "./lookup-expect.ts";
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
import { type Disagreement, describe, type Kind, KINDS, LookupComparison, tally } from "./optimised-compare.ts";

const args = scenarioArgs();
const pkg = await loadPackage(args);
await pkg.init(await readBytes(args.wasm));

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

function json(value: unknown): string {
  return JSON.stringify(value);
}

/** The code of what `f` throws, or `returned` when it returns. */
function outcome(f: () => { free(): void } | { free(): void }[]): string {
  try {
    const made = f();
    for (const object of Array.isArray(made) ? made : [made]) object.free();
    return "returned";
  } catch (error) {
    assert(error instanceof pkg.HfstError, `an HfstError, not ${String(error)}`);
    return error.code;
  }
}

function summary(disagreements: readonly Disagreement[], agreed: number): string {
  const { kinds } = tally(disagreements);
  const counts = KINDS.filter((kind) => kinds[kind] !== undefined).map((kind) => `${kind} ${kinds[kind]}`);
  return `${agreed} agree with lookup${counts.length > 0 ? `; ${counts.join(", ")}` : ""}`;
}

function assertClassified(disagreements: readonly Disagreement[], what: string): void {
  const { unclassified } = tally(disagreements);
  assert(
    unclassified.length === 0,
    `${what}: ${unclassified.length} disagreements of no accepted kind:\n${unclassified.slice(0, 10).map(describe).join("\n")}`,
  );
}

const READ = ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const;

if (args.variant === "main") {
  // [spec:hfst:req:javascript.optimised-lookup+1/test]
  test("a transducer loads with or without its header, with the type, name and properties a Transducer has", async () => {
    for (const type of READ) {
      for (const file of [`words.${type}.hfst`, `words.${type}.bare`, "first.hfst"]) {
        const bytes = await fixture(file);
        using optimised = pkg.readOptimisedTransducer(bytes);
        using transducer = pkg.readTransducer(bytes);
        assert(optimised instanceof pkg.OptimisedTransducer, `${file}: an OptimisedTransducer`);
        assertEqual(optimised.type, transducer.type, `${file}: type`);
        assertEqual(optimised.name, transducer.name, `${file}: name`);
        assertEqual(json(optimised.properties), json(transducer.properties), `${file}: properties`);
        assert(Object.isFrozen(optimised.properties), `${file}: the properties cannot be changed`);
      }
    }
  });

  // [spec:hfst:req:javascript.optimised-lookup+1/test]
  test("bytes in other formats, or not a transducer, are turned down as readTransducers turns them down", async () => {
    const weighted = await fixture("words.optimized-lookup-weighted.hfst");
    const cases: [string, Uint8Array][] = [
      ["openfst-tropical", await fixture("words.openfst-tropical.hfst")],
      ["openfst-tropical, headerless", await fixture("words.openfst-tropical.bare")],
      ["foma", await fixture("words.foma.hfst")],
      ["foma, headerless", await fixture("words.foma.bare")],
      ["foma, uncompressed", await fixture("words.foma.plain")],
      ["DHFST", await fixture("speller.acceptor.dhfst")],
      ["text", new TextEncoder().encode("just text")],
      ["nothing", new Uint8Array(0)],
      ["truncated", weighted.subarray(0, weighted.length - 7)],
      ["truncated in the alphabet", weighted.subarray(0, 120)],
    ];
    for (const [what, bytes] of cases) {
      const theirs = outcome(() => pkg.readTransducers(bytes));
      assert(theirs !== "returned", `${what}: readTransducers turns it down`);
      assertEqual(outcome(() => pkg.readOptimisedTransducer(bytes)), theirs, what);
    }
    const notBytes = [1, 2] as unknown as Uint8Array;
    assertEqual(thrownCode(pkg.HfstError, () => pkg.readOptimisedTransducer(notBytes)), "invalid-argument", "not bytes");
  });

  // [spec:hfst:req:javascript.optimised-lookup+1/test]
  test("a stream of several transducers throws transducer-count", async () => {
    for (const file of ["ordered.hfst", "ordered.bare", "mixed.hfst", "animals.pmhfst"]) {
      const bytes = await fixture(file);
      assertEqual(outcome(() => pkg.readOptimisedTransducer(bytes)), "transducer-count", file);
    }
  });

  // [spec:hfst:req:javascript.optimised-lookup+1/test]
  test("a limit keeps the lightest results; a time limit is refused", async () => {
    using t = pkg.readOptimisedTransducer(await fixture("ties.optimized-lookup-weighted.hfst"));
    const all = t.lookup("a");
    assert(all.length > 2, `"a" has more than two results: ${json(all)}`);
    for (let i = 1; i < all.length; i += 1) {
      const [before, after] = [all[i - 1], all[i]];
      assert(
        before !== undefined && after !== undefined && (before.weight < after.weight || (before.weight === after.weight && before.output < after.output)),
        `ordered by weight, then output, each once: ${json(all)}`,
      );
    }
    assertEqual(json(t.lookup("a", { limit: 2 })), json(all.slice(0, 2)), "limit 2");
    assertEqual(json(t.lookup("a", { limit: 0 })), "[]", "limit 0");
    assertEqual(json(t.lookup("a", {})), json(all), "no limit");
    const timed = { timeLimitMs: 10 } as unknown as { limit?: number };
    assertEqual(thrownCode(pkg.HfstError, () => t.lookup("a", timed)), "invalid-argument", "a time limit");
    for (const bad of [{ limit: -1 }, { limit: 1.5 }, null] as unknown as { limit?: number }[]) {
      assertEqual(thrownCode(pkg.HfstError, () => t.lookup("a", bad)), "invalid-argument", json(bad));
    }
    assertEqual(thrownCode(pkg.HfstError, () => t.lookup(7 as unknown as string)), "invalid-argument", "a number");
  });

  // [spec:hfst:req:javascript.optimised-lookup+1/test]
  test("a freed optimised transducer throws freed", async () => {
    const t = pkg.readOptimisedTransducer(await fixture("words.optimized-lookup-weighted.hfst"));
    t.free();
    t.free();
    for (const [what, use] of [
      ["lookup", () => t.lookup("cat")],
      ["type", () => t.type],
      ["name", () => t.name],
      ["properties", () => t.properties],
    ] as const) {
      assertEqual(thrownCode(pkg.HfstError, use), "freed", what);
    }
  });

  // [spec:hfst:req:javascript.optimised-lookup+1/test]
  test("on every fixture, the optimised lookup agrees with lookup but where a documented difference explains it", async () => {
    const expected: { optimised: { file: string; input: string; limit: number | null }[] } = JSON.parse(
      new TextDecoder().decode(await fixture("parity.json")),
    );
    const inputs = new Map<string, string[]>();
    for (const { file, input, limit } of expected.optimised) {
      if (limit === null) inputs.set(file, [...(inputs.get(file) ?? []), input]);
    }
    assert(inputs.size > 0, "parity.json has optimised lookups");
    const seen = new Set<Kind>();
    const all: Disagreement[] = [];
    let agreedTotal = 0;
    for (const [file, list] of inputs) {
      const bytes = await fixture(file);
      using optimised = pkg.readOptimisedTransducer(bytes);
      using transducer = pkg.readTransducer(bytes);
      const { agreed, disagreements } = new LookupComparison(optimised, transducer, bytes).compare(list);
      assertClassified(disagreements, file);
      for (const { kind } of disagreements) if (kind !== null) seen.add(kind);
      all.push(...disagreements);
      agreedTotal += agreed;
    }
    console.log(`# fixtures: ${summary(all, agreedTotal)}`);
    for (const kind of KINDS) {
      const example = all.find((disagreement) => disagreement.kind === kind);
      assert(example !== undefined, `no fixture shows the ${kind} difference`);
      console.log(`#   ${describe(example)}`);
    }
  });
} else if (args.variant === "analysers") {
  const env: unknown = Reflect.get(Reflect.get(globalThis, "process"), "env");
  const listed: unknown = typeof env === "object" && env !== null ? Reflect.get(env, "HFST_JS_PARITY") : undefined;
  if (typeof listed !== "string" || listed === "") {
    test("real analysers # SKIP HFST_JS_PARITY lists none", () => {});
  } else {
    interface NodeFs {
      readFileSync(path: string, encoding: "utf8"): string;
    }
    interface NodeUrl {
      pathToFileURL(path: string): URL;
    }
    const fsSpecifier = "node:fs";
    const urlSpecifier = "node:url";
    const nodeFs: NodeFs = await import(fsSpecifier);
    const nodeUrl: NodeUrl = await import(urlSpecifier);
    const separator = Reflect.get(Reflect.get(globalThis, "process"), "platform") === "win32" ? ";" : ":";
    for (const entry of listed.split(separator).filter((entry) => entry !== "")) {
      const [transducer, words] = entry.split("=");
      // [spec:hfst:req:javascript.optimised-lookup+1/test]
      test(`${transducer}: the optimised lookup agrees with lookup but where a documented difference explains it`, async () => {
        assert(
          transducer !== undefined && words !== undefined,
          `HFST_JS_PARITY entries are transducer.hfstol=words.txt, not ${JSON.stringify(entry)}`,
        );
        const bytes = await readBytes(nodeUrl.pathToFileURL(transducer).href);
        using optimised = pkg.readOptimisedTransducer(bytes);
        using ours = pkg.readTransducer(bytes);
        const list = [...new Set(linesOf(nodeFs.readFileSync(words, "utf8")).filter((word) => word !== ""))];
        const inputs = withNonWords(list);
        const started = performance.now();
        const { agreed, disagreements } = new LookupComparison(optimised, ours, bytes).compare(inputs);
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
