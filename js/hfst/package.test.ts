import { entryOf, packageTests } from "../test/packages.ts";
import { assertEqual, runTests, test } from "../test/harness.ts";

const dir = new URL("./", import.meta.url);
packageTests(dir, "@divvun/hfst");

test("the entry module exports the API", async () => {
  const names = Object.keys(await entryOf(dir)).sort();
  assertEqual(
    names.join(" "),
    "HfstError Tokeniser Transducer init readTokeniser readTransducer readTransducers setLogLevel setLogger version",
    "exports",
  );
});

await runTests("hfst/package");
