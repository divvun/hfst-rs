// Compiles at the depth limits hfst::depth sets for wasm32, against the
// compile module as it ships. crates/hfst-wasm-compile/build.rs links the
// module with a stack these fit in: at a limit a compile succeeds, and past
// it fails with compile-failed rather than trapping, so the module is never
// poisoned.

import type * as Compile from "../hfst/compile.ts";
import { assert, assertEqual, readBytes, runTests, scenarioArgs, test } from "./harness.ts";

const args = scenarioArgs();
const pkg: typeof Compile = await import(args.module);
await pkg.init(await readBytes(args.wasm));

/** Brackets nested `n` deep around `a`. */
const brackets = (n: number) => `${"[".repeat(n)}a${"]".repeat(n)}`;
/** `n` complements of `a`, each a level of evaluation. */
const complements = (n: number) => `${"~".repeat(n)}a`;
/** A union of `n + 1` symbols: `n` operators in a chain. */
const union = (n: number) => Array.from({ length: n + 1 }, () => "a").join(" | ");

/**
 * Each construct at the deepest the wasm32 limits let a compiler take it,
 * and the compile one level deeper. The nesting limit is 100 brackets, the
 * expression limit 500 levels of evaluation, of which the source's top level
 * takes one, the tree limit 3000 operators, and the path limit 1000 arcs.
 */
const cases: [string, (n: number) => unknown, number][] = [
  ["regex brackets", (n) => pkg.regex(brackets(n)), 100],
  ["regex operators", (n) => pkg.regex(complements(n)), 499],
  ["regex chain", (n) => pkg.regex(union(n)), 2999],
  ["regex path", (n) => pkg.regex(`{${"a".repeat(n)}}`), 1000],
  ["lexc brackets", (n) => pkg.compileLexc([`LEXICON Root\n< ${brackets(n)} > # ;\n`]), 100],
  ["twolc operators", (n) => pkg.compileTwolc(`Alphabet a b ;\nRules\n"r"\na:b <=> ${complements(n)} _ ;\n`), 499],
  ["pmatch brackets", (n) => pkg.compilePmatch(`Define TOP ${brackets(n)} ;\n`), 100],
  ["pmatch chain", (n) => pkg.compilePmatch(`Define TOP ${union(n)} ;\n`), 498],
  ["xfst brackets", (n) => pkg.compileXfst(`regex ${brackets(n)} ;\n`), 100],
  ["xfst operators", (n) => pkg.compileXfst(`regex ${complements(n)} ;\n`), 499],
];

for (const [name, compile, deepest] of cases) {
  // [spec:hfst:req:javascript.compile-errors/test]
  test(`${name}: ${deepest} deep compiles, and ${deepest + 1} fails with compile-failed`, () => {
    compile(deepest);
    let code: string | undefined;
    try {
      compile(deepest + 1);
    } catch (error) {
      assert(error instanceof pkg.HfstError, `an HfstError, not ${String(error)}`);
      code = error.code;
    }
    assertEqual(code, "compile-failed", "past the limit");
    assertEqual(pkg.version(), args.version, "the module carries on");
  });
}

await runTests(args.label);
