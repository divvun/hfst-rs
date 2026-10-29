// The runtime contract against the real module of a test build: errors from
// the library, object lifetime, and logging fed by the library's tracing.

import type * as Testing from "../hfst-runtime/testing.ts";
import {
  assert,
  assertEqual,
  collectUntil,
  readBytes,
  runTests,
  scenarioArgs,
  test,
  thrownCode,
  withConsole,
} from "./harness.ts";

const args = scenarioArgs();
const pkg: typeof Testing = await import(args.module);
const { testing } = pkg;
await pkg.init(await readBytes(args.wasm));

type LogEvent = Parameters<Parameters<typeof pkg.setLogger>[0] & {}>[0];

/** Sends events at `level` and above to the returned array. */
function capture(level: Parameters<typeof pkg.setLogLevel>[0]): LogEvent[] {
  const events: LogEvent[] = [];
  pkg.setLogger((event) => events.push(event));
  pkg.setLogLevel(level);
  return events;
}

function reset(): void {
  pkg.setLogger(null);
  pkg.setLogLevel("warn");
}

function dropped(events: LogEvent[], label: string): number {
  return events.filter((event) => event.message === `dropped ${label}`).length;
}

function thrown(f: () => unknown): unknown {
  try {
    f();
  } catch (error) {
    return error;
  }
  throw new Error("nothing was thrown");
}

// [spec:hfst:req:javascript.errors/test]
test("a library error is thrown as an HfstError with its code and the library's message", () => {
  const error = thrown(() => testing.libraryError("left is foma"));
  assert(error instanceof pkg.HfstError, "an HfstError");
  assert(error instanceof Error && !(error instanceof pkg.HfstCompileError), "a plain HfstError");
  assertEqual(error.name, "HfstError", "name");
  assertEqual(error.code, "type-mismatch", "code");
  assertEqual(error.message, "transducer type mismatch: left is foma", "message");
});

// [spec:hfst:req:javascript.errors/test]
test("a failed compile is thrown as an HfstCompileError with the diagnostics", () => {
  const error = thrown(() => testing.compileError("lexc failed"));
  assert(error instanceof pkg.HfstCompileError, "an HfstCompileError");
  assert(error instanceof pkg.HfstError, "which is an HfstError");
  assertEqual(error.code, "compile-error", "code");
  assertEqual(error.message, "lexc failed", "message");
  const [diagnostic] = error.diagnostics;
  assertEqual(diagnostic?.message, "unexpected ;", "the diagnostic's message");
  assertEqual(diagnostic?.start, 4, "its span");
  assertEqual(diagnostic?.help, "remove it", "its help");
  assertEqual(testing.createObject("after").label(), "after", "the module carries on");
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("free releases the object's memory once, and a call after it throws freed", () => {
  const events = capture("debug");
  const object = testing.createObject("a");
  assertEqual(object.label(), "a", "label");
  object.free();
  object.free();
  assertEqual(dropped(events, "a"), 1, "times the module dropped it");
  assertEqual(thrownCode(pkg.HfstError, () => object.label()), "freed", "a call after free");
  reset();
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("a using declaration and Symbol.dispose free the object", () => {
  const events = capture("debug");
  {
    using scoped = testing.createObject("scoped");
    assertEqual(scoped.label(), "scoped", "label inside the block");
  }
  assertEqual(dropped(events, "scoped"), 1, "dropped at the end of the block");
  const disposed = testing.createObject("disposed");
  disposed[Symbol.dispose]();
  assertEqual(dropped(events, "disposed"), 1, "dropped by Symbol.dispose");
  reset();
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("an object dropped without free is released after garbage collection", async () => {
  const events = capture("debug");
  (() => {
    testing.createObject("collected");
  })();
  assert(await collectUntil(() => dropped(events, "collected") === 1), "the module dropped it");
  reset();
});

// [spec:hfst:req:javascript.init/test]
test("init again keeps the instance and the objects already made in it", async () => {
  const object = testing.createObject("before");
  await pkg.init();
  assertEqual(object.label(), "before", "the object made before the second init");
  object.free();
});

// [spec:hfst:req:javascript.logging/test]
test("by default warnings and errors from the library reach the console, and lower levels do not", async () => {
  reset();
  await withConsole((lines) => {
    for (const level of ["error", "warn", "info", "debug", "trace"] as const) {
      testing.log(level, `at ${level}`);
    }
    const seen = lines.map(({ method, text }) => `${method} ${text}`);
    assertEqual(
      seen.join("; "),
      "error hfst_wasm_runtime::test_hooks: at error; warn hfst_wasm_runtime::test_hooks: at warn",
      "console lines",
    );
  });
});

// [spec:hfst:req:javascript.logging/test]
test("a replaced logger gets each event's level, target and message", () => {
  const events = capture("info");
  testing.log("info", "hello");
  assertEqual(
    JSON.stringify(events),
    '[{"level":"info","target":"hfst_wasm_runtime::test_hooks","message":"hello"}]',
    "events",
  );
  reset();
});

// [spec:hfst:req:javascript.logging/test]
test("the level set is the level the module sends, and off sends nothing", () => {
  const events = capture("info");
  for (const level of ["error", "warn", "info", "debug", "trace"] as const) testing.log(level, "x");
  pkg.setLogLevel("trace");
  testing.log("trace", "y");
  pkg.setLogLevel("off");
  testing.log("error", "z");
  assertEqual(events.map((event) => event.level).join(" "), "error warn info trace", "levels");
  reset();
});

await runTests(args.label);
