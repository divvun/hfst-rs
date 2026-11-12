// A trap in one entry point's module poisons that module only: each entry
// point has a module instance, a sink and a poison of its own. `variant`
// names the entry point whose module traps. Each variant runs alone, since a
// poisoned instance stays poisoned.

import type * as CompileTesting from "../hfst/compile-testing.ts";
import type * as Testing from "../hfst/testing.ts";
import { assert, assertEqual, readBytes, runTests, scenarioArgs, test, thrownCode } from "./harness.ts";

const args = scenarioArgs();
const main: typeof Testing = await import(args.entries.main.module);
const compile: typeof CompileTesting = await import(args.entries.compile.module);
await main.init(await readBytes(args.entries.main.wasm));
await compile.init(await readBytes(args.entries.compile.wasm));

const [trapped, other] = args.variant === "compile" ? [compile, main] : [main, compile];

// [spec:hfst:req:javascript.compile-errors/test]
test(`a panic in the ${args.variant} module poisons it`, () => {
  assertEqual(thrownCode(trapped.HfstError, () => trapped.testing.panic("boom")), "poisoned", "the panic");
  assertEqual(thrownCode(trapped.HfstError, () => trapped.version()), "poisoned", "a later call");
});

// [spec:hfst:req:javascript.compile-errors/test]
test("the other module carries on", () => {
  assertEqual(other.version(), args.version, "version()");
  assertEqual(other.testing.createObject("alive").label(), "alive", "an object of its own");
  if (other === compile) {
    assertEqual(compile.regex("{cat}").transducer.lookup("cat")[0]?.output, "cat", "a compile");
  }
});

test("both entry points throw the one HfstError", () => {
  assert(main.HfstError === compile.HfstError, "one class");
});

await runTests(args.label);
