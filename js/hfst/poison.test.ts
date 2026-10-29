// How a call into the module fails: glue errors, other exceptions, and traps,
// which poison the instance for good.

import { FakeGlue } from "../test/fake-glue.ts";
import { assert, assertEqual, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";

async function started(): Promise<{ core: Core; glue: FakeGlue }> {
  const glue = new FakeGlue();
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  return { core, glue };
}

function thrown(f: () => unknown): unknown {
  try {
    f();
  } catch (error) {
    return error;
  }
  throw new Error("nothing was thrown");
}

// [spec:hfst:req:javascript.errors+1/test]
test("an error a glue call returns is thrown as an HfstError with its code", async () => {
  const { core, glue } = await started();
  const error = thrown(() => core.call(() => glue.fail("cyclic", "transducer is cyclic")));
  assert(error instanceof HfstError, "an HfstError");
  assertEqual(error.code, "cyclic", "code");
  assertEqual(error.message, "transducer is cyclic", "message");
  assertEqual(core.call(() => glue.version()), "0.0.0-fake", "the next call");
});

test("an exception that is neither a glue error nor a trap passes through", async () => {
  const { core } = await started();
  const error = thrown(() =>
    core.call(() => {
      throw new TypeError("not a string");
    }),
  );
  assert(error instanceof TypeError, "the TypeError itself");
});

// [spec:hfst:req:javascript.errors+1/test]
test("a panic poisons the instance, and the error carries the panic message", async () => {
  const { core, glue } = await started();
  const error = thrown(() => core.call(() => glue.trap("index out of bounds")));
  assert(error instanceof HfstError, "an HfstError");
  assertEqual(error.code, "poisoned", "code");
  assert(error.message.includes("index out of bounds"), `the panic message: ${error.message}`);
  assert(error.cause instanceof WebAssembly.RuntimeError, "the trap is the cause");
});

// [spec:hfst:req:javascript.errors+1/test]
test("every later call throws poisoned without entering the module", async () => {
  const { core, glue } = await started();
  thrown(() => core.call(() => glue.trap(null)));
  const entered = glue.entered;
  assertEqual(thrownCode(HfstError, () => core.call(() => glue.version())), "poisoned", "a call");
  assertEqual(thrownCode(HfstError, () => core.setLogLevel("info")), "poisoned", "setLogLevel");
  assertEqual(thrownCode(HfstError, () => core.setLogger(null)), "poisoned", "setLogger");
  assertEqual(glue.entered, entered, "calls that entered the module after the trap");
  const message = thrown(() => core.call(() => glue.version()));
  assert(message instanceof HfstError && message.message.includes("unreachable"), "trap named");
});

// [spec:hfst:req:javascript.errors+1/test]
test("a trap in a nested call poisons the call around it", async () => {
  const { core, glue } = await started();
  const error = thrown(() =>
    core.call(() => {
      thrown(() => core.call(() => glue.trap("inner")));
      return glue.version();
    }),
  );
  assert(error instanceof HfstError && error.code === "poisoned", "the outer call throws poisoned");
});

test("init after poisoning returns the same promise", async () => {
  const glue = new FakeGlue();
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  const first = core.init(new Uint8Array(0));
  await first;
  thrown(() => core.call(() => glue.trap(null)));
  assert(core.init() === first, "the same promise");
});

await runTests("hfst/poison");
