// Loading transducers from bytes, against both packages as they ship. The
// fixtures are built by `node js/build.mjs --test` with this checkout's hfst
// tools; js/test/fixtures.mjs says what each one is.

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

const args = scenarioArgs();
const pkg = await loadPackage(args);
await pkg.init(await readBytes(args.wasm));
const runtime = args.package === "@divvun/hfst-runtime";

async function fixture(name: string): Promise<Uint8Array> {
  return await readBytes(new URL(name, args.fixtures).href);
}

function thrown(f: () => unknown): InstanceType<typeof pkg.HfstError> {
  try {
    f();
  } catch (error) {
    assert(error instanceof pkg.HfstError, `an HfstError, not ${String(error)}`);
    return error;
  }
  throw new Error("nothing was thrown");
}

function json(value: unknown): string {
  return JSON.stringify(value);
}

/** Where the payload of a stream's first member starts, after its HFST3 header. */
function payloadStart(bytes: Uint8Array): number {
  return 8 + (bytes[5] ?? 0) + 256 * (bytes[6] ?? 0);
}

function withU32(bytes: Uint8Array, offset: number, value: number): Uint8Array {
  const copy = Uint8Array.from(bytes);
  new DataView(copy.buffer).setUint32(offset, value, true);
  return copy;
}

// [spec:hfst:req:javascript.load-bytes+1/test]
test("weighted and unweighted optimized lookup load with their header's name and properties", async () => {
  for (const type of ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const) {
    const all = pkg.readTransducers(await fixture(`words.${type}.hfst`));
    assertEqual(all.length, 1, `${type}: transducers`);
    const [t] = all;
    assert(t instanceof pkg.Transducer, `${type}: a Transducer`);
    assertEqual(t.type, type, "type");
    assertEqual(t.name, "convert(xre(?))", `${type}: name`);
    assertEqual(json(t.properties), '{"formulaic-definition":"Id ."}', `${type}: properties`);
    t.free();
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("optimized lookup with no HFST3 header loads, with no name or properties", async () => {
  for (const type of ["optimized-lookup-weighted", "optimized-lookup-unweighted"] as const) {
    using t = pkg.readTransducer(await fixture(`words.${type}.bare`));
    assertEqual(t.type, type, "type");
    assertEqual(t.name, "", `${type}: name`);
    assertEqual(json(t.properties), "{}", `${type}: properties`);
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("a stream of several transducers loads as an array in stream order", async () => {
  const ordered = pkg.readTransducers(await fixture("ordered.hfst"));
  assertEqual(ordered.map((t) => t.name).join(" "), "first second", "names");
  const bare = pkg.readTransducers(await fixture("ordered.bare"));
  assertEqual(bare.length, 2, "headerless transducers");
  for (const t of [...ordered, ...bare]) t.free();
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("a pmatch container loads as TOP and its definitions, each with its own properties", async () => {
  const all = pkg.readTransducers(await fixture("animals.pmhfst"));
  assertEqual(all.map((t) => t.name).join(" "), "TOP Animal", "names");
  assert(all.every((t) => t.type === "optimized-lookup-weighted"), "all weighted optimized lookup");
  const [top, animal] = all;
  assertEqual(top?.properties["max-recursion"], "5000", "TOP's settings");
  assertEqual(json(animal?.properties), "{}", "the definition's properties");
  for (const t of all) t.free();
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("readTransducer takes one transducer, and a stream of several throws transducer-count", async () => {
  using one = pkg.readTransducer(await fixture("words.optimized-lookup-weighted.hfst"));
  assertEqual(one.type, "optimized-lookup-weighted", "one transducer");
  const several = await fixture("ordered.hfst");
  const error = thrown(() => pkg.readTransducer(several));
  assertEqual(error.code, "transducer-count", "code");
  assert(error.message.includes("2"), `the message gives the count: ${error.message}`);
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("tropical OpenFst and foma bytes are for @divvun/hfst", async () => {
  for (const [name, type] of [
    ["words.openfst-tropical.hfst", "openfst-tropical"],
    ["words.openfst-tropical.bare", "openfst-tropical"],
    ["words.foma.hfst", "foma"],
    ["words.foma.plain", "foma"],
  ] as const) {
    const bytes = await fixture(name);
    if (runtime) {
      const error = thrown(() => pkg.readTransducers(bytes));
      assertEqual(error.code, "unsupported-format", `${name}: code`);
      assert(error.message.includes(type), `${name}: the message names ${type}: ${error.message}`);
      assert(error.message.includes("@divvun/hfst reads it"), `${name}: the message names @divvun/hfst`);
    } else {
      using t = pkg.readTransducer(bytes);
      assertEqual(String(t.type), type, `${name}: type`);
    }
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("formats no package reads throw unsupported-format naming them", async () => {
  const gzipped = await fixture("words.foma.bare");
  const gzip = thrown(() => pkg.readTransducers(gzipped));
  assertEqual(gzip.code, "unsupported-format", "gzip: code");
  assert(gzip.message.includes("gzip"), `the message names gzip: ${gzip.message}`);
  const dhfst = thrown(() => pkg.readTransducers(new TextEncoder().encode("DHFST\u0001\u0001\u0000 and more")));
  assertEqual(dhfst.code, "unsupported-format", "DHFST: code");
  assert(dhfst.message.includes("DHFST"), `the message names DHFST: ${dhfst.message}`);
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("empty, foreign, truncated and corrupt bytes throw invalid-format", async () => {
  const olw = await fixture("words.optimized-lookup-weighted.hfst");
  const ol = payloadStart(olw);
  const cases: [string, Uint8Array][] = [
    ["empty", new Uint8Array(0)],
    ["text", new TextEncoder().encode("this is not a transducer stream")],
    ["zeros", new Uint8Array(100)],
    ["truncated inside the HFST3 header", olw.subarray(0, 12)],
    ["truncated inside the optimized-lookup header", olw.subarray(0, ol + 30)],
    ["truncated inside the tables", olw.subarray(0, olw.length - 7)],
    ["truncated second member", (await fixture("ordered.hfst")).subarray(0, olw.length + 100)],
    ["a property flag that is not 0 or 1", withU32(olw, ol + 20, 7)],
    ["a header claiming a vast index table", withU32(olw, ol + 4, 0xfffffff0)],
    ["a header claiming a vast transition table", withU32(olw, ol + 8, 0x7ffffff0)],
  ];
  for (const [what, bytes] of cases) {
    const error = thrown(() => pkg.readTransducers(bytes));
    assertEqual(error.code, "invalid-format", `${what} (${error.message})`);
  }
  using after = pkg.readTransducer(olw);
  assertEqual(after.type, "optimized-lookup-weighted", "the module still loads transducers");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("a stream whose transducers differ in type throws type-mismatch", async () => {
  const mixed = await fixture("mixed.hfst");
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readTransducers(mixed)), "type-mismatch", "code");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("bytes that are not a Uint8Array throw invalid-argument", () => {
  for (const notBytes of [null, "HFST", [72, 70, 83, 84], new ArrayBuffer(8)]) {
    const call = () => pkg.readTransducers(notBytes as unknown as Uint8Array);
    assertEqual(thrownCode(pkg.HfstError, call), "invalid-argument", String(notBytes));
  }
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("a freed transducer throws freed, and freeing twice does nothing", async () => {
  const t = pkg.readTransducer(await fixture("words.optimized-lookup-unweighted.hfst"));
  t.free();
  t.free();
  assertEqual(thrownCode(pkg.HfstError, () => t.name), "freed", "name");
  assertEqual(thrownCode(pkg.HfstError, () => t.type), "freed", "type");
});

await runTests(args.label);
