// The transducer wrapper both packages share, against a fake glue.

import { FakeGlue, type FakeTransducer } from "../test/fake-glue.ts";
import { assert, assertEqual, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { HfstError } from "./errors.ts";
import {
  RUNTIME_TRANSDUCER_TYPES,
  type RuntimeTransducerType,
  TransducerBase,
  transducerReaders,
} from "./transducer.ts";

class Loaded extends TransducerBase<RuntimeTransducerType, FakeTransducer> {
  constructor(core: Core, glue: FakeTransducer) {
    super(core, glue, RUNTIME_TRANSDUCER_TYPES);
  }
}

async function started(stream: string[][]) {
  const glue = new FakeGlue();
  glue.stream = stream;
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  const readers = transducerReaders((bytes) =>
    core.call((g) => g.readTransducers(bytes).map((t) => new Loaded(core, t as FakeTransducer))),
  );
  return { glue, ...readers };
}

// [spec:hfst:req:javascript.load-bytes+1/test]
test("a transducer's type, name and properties are plain values", async () => {
  const { readTransducer } = await started([
    ["optimized-lookup-weighted", "TOP", "max-recursion", "5000", "mark-patterns", "on"],
  ]);
  const t = readTransducer(new Uint8Array(1));
  assertEqual(t.type, "optimized-lookup-weighted", "type");
  assertEqual(t.name, "TOP", "name");
  assertEqual(JSON.stringify(t.properties), '{"max-recursion":"5000","mark-patterns":"on"}', "properties");
  assert(Object.isFrozen(t.properties), "the properties cannot be changed");
  assert(t.properties === t.properties, "the same object each time");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("readTransducer on a stream of several throws transducer-count and frees them all", async () => {
  const { glue, readTransducer, readTransducers } = await started([
    ["optimized-lookup-weighted", "a"],
    ["optimized-lookup-weighted", "b"],
  ]);
  assertEqual(readTransducers(new Uint8Array(1)).length, 2, "readTransducers reads both");
  assertEqual(thrownCode(HfstError, () => readTransducer(new Uint8Array(1))), "transducer-count", "code");
  assertEqual(glue.freed.join(), "a,b", "freed");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("bytes that are not a Uint8Array throw invalid-argument without entering the module", async () => {
  const { glue, readTransducers } = await started([]);
  const before = glue.entered;
  const notBytes = [1, 2, 3] as unknown as Uint8Array;
  assertEqual(thrownCode(HfstError, () => readTransducers(notBytes)), "invalid-argument", "code");
  assertEqual(glue.entered, before, "calls into the module");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("a type the wrapper does not know is an internal error", async () => {
  const { readTransducers } = await started([["thfst", "x"]]);
  assertEqual(thrownCode(HfstError, () => readTransducers(new Uint8Array(1))), "internal", "code");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("the accessors of a freed transducer throw freed", async () => {
  const { readTransducer } = await started([["optimized-lookup-unweighted", "x"]]);
  const t = readTransducer(new Uint8Array(1));
  t.free();
  for (const [what, read] of [
    ["type", () => t.type],
    ["name", () => t.name],
    ["properties", () => t.properties],
  ] as const) {
    assertEqual(thrownCode(HfstError, read), "freed", what);
  }
});

await runTests("shared/transducer");
