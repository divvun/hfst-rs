// The DHFST wrapper, against a fake glue.

import { FakeDhfstAcceptor, FakeGlue } from "../test/fake-glue.ts";
import { assert, assertEqual, runTests, test, thrownCode } from "../test/harness.ts";
import { Core } from "./core.ts";
import { dhfstReader } from "./dhfst.ts";
import { HfstError } from "./errors.ts";

async function started(dhfstType: string) {
  const glue = new FakeGlue();
  glue.dhfstType = dhfstType;
  const core = new Core(glue, new URL("http://127.0.0.1:9/never.wasm"));
  await core.init(new Uint8Array(0));
  return { glue, ...dhfstReader(core) };
}

// [spec:hfst:req:javascript.dhfst+2/test]
test("an acceptor's type, metadata and info are plain values, and lookup gives null for no path", async () => {
  const { glue, readDhfst } = await started("acceptor");
  const model = readDhfst(new Uint8Array(1));
  assert(model.type === "acceptor", "the acceptor says it is one");
  assertEqual(model.version, 1, "version");
  assertEqual(model.symbols.join(" "), "@_EPSILON_SYMBOL_@ a b", "symbols");
  assert(Object.isFrozen(model.symbols) && Object.isFrozen(model.info), "the values cannot be changed");
  assertEqual(model.meta, null, "no meta section is null");
  assertEqual(model.info.labelMax, 5, "info");
  assertEqual(model.lookup("ab"), 2, "an accepted word");
  assertEqual(model.lookup("abc"), null, "a rejected word");
  const [fake] = glue.made;
  assert(fake instanceof FakeDhfstAcceptor, "the glue made an acceptor");
  assertEqual(fake.lookups.join(","), "ab,abc", "the words that reached the module");
});

// [spec:hfst:req:javascript.dhfst+2/test]
test("an error model has its metadata and no lookup", async () => {
  const { readDhfst } = await started("error-model");
  const model = readDhfst(new Uint8Array(1));
  assert(model.type === "error-model", "the error model says it is one");
  assertEqual(model.stateCount, 2, "state count");
  assertEqual(model.meta, '{"writer":"fake"}', "meta");
  assert(!("lookup" in model), "an error model has no lookup");
});

// [spec:hfst:req:javascript.dhfst+2/test]
test("a word that is not a string and bytes that are not a Uint8Array throw invalid-argument", async () => {
  const { glue, readDhfst } = await started("acceptor");
  const before = glue.entered;
  const notBytes = [1, 2, 3] as unknown as Uint8Array;
  assertEqual(thrownCode(HfstError, () => readDhfst(notBytes)), "invalid-argument", "bytes");
  assertEqual(glue.entered, before, "calls into the module");
  const model = readDhfst(new Uint8Array(1));
  if (model.type !== "acceptor") throw new Error("not an acceptor");
  const notWord = 7 as unknown as string;
  assertEqual(thrownCode(HfstError, () => model.lookup(notWord)), "invalid-argument", "word");
});

// [spec:hfst:req:javascript.dhfst+2/test]
test("a type the wrapper does not know is an internal error, and its model is freed", async () => {
  const { glue, readDhfst } = await started("thesaurus");
  assertEqual(thrownCode(HfstError, () => readDhfst(new Uint8Array(1))), "internal", "code");
  assertEqual(glue.freed.join(), "thesaurus", "freed");
});

// [spec:hfst:req:javascript.handle-lifetime+1/test]
test("every accessor of a freed DHFST model throws freed, and freeing twice frees once", async () => {
  const acceptor = (await started("acceptor")).readDhfst(new Uint8Array(1));
  if (acceptor.type !== "acceptor") throw new Error("not an acceptor");
  const { glue, readDhfst } = await started("error-model");
  const errorModel = readDhfst(new Uint8Array(1));
  if (errorModel.type !== "error-model") throw new Error("not an error model");
  acceptor.free();
  errorModel.free();
  errorModel[Symbol.dispose]();
  assertEqual(glue.freed.join(), "error-model", "freed once");
  const reads: [string, () => unknown][] = [
    ["type", () => acceptor.type],
    ["version", () => acceptor.version],
    ["symbols", () => acceptor.symbols],
    ["meta", () => acceptor.meta],
    ["info", () => acceptor.info],
    ["error model type", () => errorModel.type],
    ["state count", () => errorModel.stateCount],
    ["lookup", () => acceptor.lookup("a")],
  ];
  for (const [what, read] of reads) assertEqual(thrownCode(HfstError, read), "freed", what);
});

await runTests("hfst/dhfst");
