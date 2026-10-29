// The full package's checks on what it is handed, against the built package.
// They need no fixtures: every transducer here is read from AT&T text.

import { assertEqual, readBytes, runTests, test, thrownCode } from "../test/harness.ts";
import { entryOf } from "../test/packages.ts";
import type * as Full from "./index.ts";

const dir = new URL("./", import.meta.url);
const pkg = (await entryOf(dir)) as unknown as typeof Full;
await pkg.init(await readBytes(new URL("dist/hfst/pkg/hfst_wasm_bg.wasm", dir).href));

const ATT = "0\t1\ta\tb\t0.5\n1\t0\n";

// [spec:hfst:req:javascript.load-bytes+1/test]
test("writeTransducers takes Transducers of one type, and at least one", () => {
  using tropical = pkg.readAtt(ATT);
  using foma = pkg.readAtt(ATT, "foma");
  using again = pkg.readAtt(ATT);
  assertEqual(thrownCode(pkg.HfstError, () => pkg.writeTransducers([])), "invalid-argument", "none");
  const notTransducers = [{}] as unknown as Full.Transducer[];
  assertEqual(thrownCode(pkg.HfstError, () => pkg.writeTransducers(notTransducers)), "invalid-argument", "objects");
  assertEqual(thrownCode(pkg.HfstError, () => pkg.writeTransducers([tropical, foma])), "type-mismatch", "mixed");
  const both = pkg.readTransducers(pkg.writeTransducers([tropical, again]));
  assertEqual(both.length, 2, "transducers written");
  for (const t of both) t.free();
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("readAtt and convert take only the type names", () => {
  using t = pkg.readAtt(ATT);
  assertEqual(t.type, "openfst-tropical", "the default type");
  const badType = "thfst" as Full.TransducerType;
  assertEqual(thrownCode(pkg.HfstError, () => t.convert(badType)), "invalid-argument", "convert");
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readAtt(ATT, badType)), "invalid-argument", "readAtt");
  const notText = 42 as unknown as string;
  assertEqual(thrownCode(pkg.HfstError, () => pkg.readAtt(notText)), "invalid-argument", "text");
});

// [spec:hfst:req:javascript.load-bytes+1/test]
test("toAtt prints weights for every type but foma", () => {
  for (const type of ["openfst-tropical", "optimized-lookup-weighted", "foma"] as const) {
    using t = pkg.readAtt(ATT, type);
    const text = t.toAtt();
    assertEqual(text.includes("0.500000"), type !== "foma", `${type}: ${text}`);
  }
  using freed = pkg.readAtt(ATT);
  freed.free();
  assertEqual(thrownCode(pkg.HfstError, () => freed.toAtt()), "freed", "toAtt after free");
});

await runTests("hfst/transducer");
