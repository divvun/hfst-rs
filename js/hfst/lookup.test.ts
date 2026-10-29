// What the wrapper checks before a lookup enters the module, against a fake
// glue.

import { FakeGlue, FakeTransducer } from "../test/fake-glue.ts";
import { assert, assertEqual, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import type { LookupOptions } from "./lookup.ts";
import { transducerReaders } from "./transducer.ts";

async function started() {
  const glue = new FakeGlue();
  glue.stream = [["optimized-lookup-weighted", "t"]];
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  const transducer = transducerReaders(core).readTransducer(new Uint8Array(1));
  const [fake] = glue.made;
  assert(fake instanceof FakeTransducer, "the fake glue made a transducer");
  return { glue, fake, transducer };
}

// [spec:hfst:req:javascript.lookup/test]
test("the options reach the module as the limit and the time limit, absent ones as undefined", async () => {
  const { fake, transducer } = await started();
  const results = transducer.lookup("cat");
  assertEqual(JSON.stringify(results), '[{"output":"cat","symbols":["c","a","t"],"weight":0}]', "results");
  transducer.lookup("cat", {});
  transducer.lookup("cat", { limit: 0, timeLimitMs: 0 });
  transducer.lookup("cat", { limit: 3, timeLimitMs: 2.5 });
  transducer.lookup("cat", { limit: Number.MAX_SAFE_INTEGER });
  assertEqual(
    JSON.stringify(fake.lookups),
    '[["cat",null,null],["cat",null,null],["cat",0,0],["cat",3,2.5],["cat",4294967295,null]]',
    "glue arguments",
  );
  const [input, limit, timeLimitMs] = fake.lookups[0] ?? [];
  assertEqual(input, "cat", "input");
  assertEqual(limit, undefined, "no limit");
  assertEqual(timeLimitMs, undefined, "no time limit");
});

// [spec:hfst:req:javascript.lookup/test]
test("bad options and input throw invalid-argument without entering the module", async () => {
  const { glue, fake, transducer } = await started();
  const before = glue.entered;
  const options = [
    { limit: -1 },
    { limit: 1.5 },
    { limit: Number.NaN },
    { limit: Number.POSITIVE_INFINITY },
    { limit: "3" },
    { timeLimitMs: -1 },
    { timeLimitMs: Number.NaN },
    { timeLimitMs: Number.POSITIVE_INFINITY },
    { timeLimitMs: "10" },
    null,
    "fast",
  ] as unknown as LookupOptions[];
  for (const option of options) {
    const call = () => transducer.lookup("cat", option);
    assertEqual(thrownCode(HfstError, call), "invalid-argument", JSON.stringify(option) ?? String(option));
  }
  for (const input of [undefined, 42, ["cat"]]) {
    const call = () => transducer.lookup(input as unknown as string);
    assertEqual(thrownCode(HfstError, call), "invalid-argument", String(input));
  }
  assertEqual(glue.entered, before, "calls into the module");
  assertEqual(fake.lookups.length, 0, "lookups");
});

// [spec:hfst:req:javascript.lookup/test]
test("lookup on a freed transducer throws freed", async () => {
  const { transducer } = await started();
  transducer.free();
  assertEqual(thrownCode(HfstError, () => transducer.lookup("cat")), "freed", "code");
});

await runTests("hfst/lookup");
