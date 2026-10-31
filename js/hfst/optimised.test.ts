// The optimised transducer wrapper, against a fake glue.

import { FakeGlue, FakeOptimisedTransducer } from "../test/fake-glue.ts";
import { assert, assertEqual, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import { optimisedTransducerReader } from "./optimised.ts";

async function started(stream: string[][]) {
  const glue = new FakeGlue();
  glue.stream = stream;
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  const transducer = optimisedTransducerReader(core).readOptimisedTransducer(new Uint8Array(1));
  const [fake] = glue.made;
  assert(fake instanceof FakeOptimisedTransducer, "the fake glue made an optimised transducer");
  return { glue, fake, transducer };
}

// [spec:hfst:req:javascript.optimised-lookup+2/test]
test("its type, name and properties are plain values read once", async () => {
  const { transducer } = await started([["optimized-lookup-unweighted", "lexicon", "a", "1"]]);
  assertEqual(transducer.type, "optimized-lookup-unweighted", "type");
  assertEqual(transducer.name, "lexicon", "name");
  assertEqual(JSON.stringify(transducer.properties), '{"a":"1"}', "properties");
  assert(transducer.properties === transducer.properties, "the same object each time");
});

// [spec:hfst:req:javascript.optimised-lookup+2/test]
test("a limit reaches the module, and the JSON it answers becomes the results", async () => {
  const { fake, transducer } = await started([["optimized-lookup-weighted", "t"]]);
  assertEqual(
    JSON.stringify(transducer.lookup("ab")),
    '[{"output":"ab","symbols":["a","b"],"weight":0}]',
    "results",
  );
  transducer.lookup("ab", {});
  transducer.lookup("ab", { limit: 3 });
  transducer.lookup("ab", { limit: Number.MAX_SAFE_INTEGER });
  assertEqual(JSON.stringify(fake.lookups), '[["ab",null],["ab",null],["ab",3],["ab",4294967295]]', "glue arguments");
  assertEqual(fake.lookups[0]?.[1], undefined, "no limit");
});

// [spec:hfst:req:javascript.optimised-lookup+2/test]
test("weights JSON cannot hold come back as numbers", async () => {
  const { fake, transducer } = await started([["optimized-lookup-weighted", "t"]]);
  fake.answer =
    '[{"output":"a","symbols":["a"],"weight":0.10000000149011612},' +
    '{"output":"b","symbols":["b"],"weight":"Infinity"},' +
    '{"output":"c","symbols":["c"],"weight":"-Infinity"},' +
    '{"output":"d","symbols":["d"],"weight":"NaN"}]';
  const weights = transducer.lookup("x").map(({ weight }) => weight);
  assertEqual(weights[0], Math.fround(0.1), "an f32 weight, exactly");
  assertEqual(weights[1], Number.POSITIVE_INFINITY, "infinity");
  assertEqual(weights[2], Number.NEGATIVE_INFINITY, "negative infinity");
  assert(Number.isNaN(weights[3]), "NaN");
});

// [spec:hfst:req:javascript.optimised-lookup+2/test]
test("a time limit and bad arguments throw invalid-argument without entering the module", async () => {
  const { glue, fake, transducer } = await started([["optimized-lookup-weighted", "t"]]);
  const before = glue.entered;
  const options = [{ timeLimitMs: 5 }, { timeLimitMs: 0 }, { limit: -1 }, { limit: "2" }, null] as unknown as {
    limit?: number;
  }[];
  for (const option of options) {
    assertEqual(thrownCode(HfstError, () => transducer.lookup("ab", option)), "invalid-argument", JSON.stringify(option));
  }
  assertEqual(thrownCode(HfstError, () => transducer.lookup(1 as unknown as string)), "invalid-argument", "input");
  assertEqual(glue.entered, before, "calls into the module");
  assertEqual(fake.lookups.length, 0, "lookups");
});

// [spec:hfst:req:javascript.optimised-lookup+2/test]
test("a type the wrapper does not know is an internal error; a freed transducer throws freed", async () => {
  const glue = new FakeGlue();
  glue.stream = [["thfst", "x"]];
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  const { readOptimisedTransducer } = optimisedTransducerReader(core);
  assertEqual(thrownCode(HfstError, () => readOptimisedTransducer(new Uint8Array(1))), "internal", "unknown type");
  const { transducer } = await started([["optimized-lookup-weighted", "t"]]);
  transducer.free();
  assertEqual(thrownCode(HfstError, () => transducer.lookup("ab")), "freed", "lookup");
  assertEqual(thrownCode(HfstError, () => transducer.type), "freed", "type");
});

await runTests("hfst/optimised");
