// The lifetime of objects that own module memory.

import { FakeGlue, type FakeObject } from "../test/fake-glue.ts";
import { assert, assertEqual, collectUntil, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import { Handle } from "./handle.ts";

class Thing extends Handle<FakeObject> {
  constructor(core: Core, glue: FakeObject) {
    super(core, glue);
  }

  label(): string {
    return this.use((glue) => glue.label);
  }
}

async function started(): Promise<{ core: Core; glue: FakeGlue }> {
  const glue = new FakeGlue();
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  return { core, glue };
}

// [spec:hfst:req:javascript.handle-lifetime/test]
test("free releases the memory, a second free does nothing, and a call after it throws freed", async () => {
  const { core, glue } = await started();
  const thing = new Thing(core, glue.object("a"));
  assertEqual(thing.label(), "a", "label");
  thing.free();
  assertEqual(glue.freed.join(), "a", "freed");
  thing.free();
  assertEqual(glue.freed.join(), "a", "freed after a second free");
  assertEqual(thrownCode(HfstError, () => thing.label()), "freed", "a call after free");
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("Symbol.dispose frees, so a using declaration frees at the end of its block", async () => {
  const { core, glue } = await started();
  const kept = new Thing(core, glue.object("kept"));
  {
    using scoped = new Thing(core, glue.object("scoped"));
    assertEqual(scoped.label(), "scoped", "label inside the block");
  }
  assertEqual(glue.freed.join(), "scoped", "freed at the end of the block");
  kept[Symbol.dispose]();
  assertEqual(glue.freed.join(), "scoped,kept", "freed by Symbol.dispose");
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("an object dropped without free is released after garbage collection", async () => {
  const { core, glue } = await started();
  (() => {
    new Thing(core, glue.object("dropped"));
  })();
  assert(await collectUntil(() => glue.freed.includes("dropped")), "the dropped object is freed");
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("after a trap, free enters nothing and does not throw", async () => {
  const { core, glue } = await started();
  const thing = new Thing(core, glue.object("survivor"));
  try {
    core.call(() => glue.trap(null));
  } catch {
    // The trap is what this test sets up.
  }
  const entered = glue.entered;
  thing.free();
  thing.free();
  (() => {
    new Thing(core, glue.object("late"));
  })();
  assertEqual(glue.entered, entered + 1, "only making the late object entered the module");
  assertEqual(thrownCode(HfstError, () => thing.label()), "poisoned", "a call on it");
});

// [spec:hfst:req:javascript.handle-lifetime/test]
test("a free that traps throws poisoned once, and freeing again does nothing", async () => {
  const { core, glue } = await started();
  const object = glue.object("fragile");
  object.trapOnFree = true;
  const thing = new Thing(core, object);
  assertEqual(thrownCode(HfstError, () => thing.free()), "poisoned", "the trapping free");
  thing.free();
});

await runTests("shared/handle");
