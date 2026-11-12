import { entryOf, packageTests } from "../test/packages.ts";
import { assertEqual, runTests, test } from "../test/harness.ts";

const dir = new URL("./", import.meta.url);
packageTests(dir, "@divvun/hfst");

test("the entry module exports the API", async () => {
  const names = Object.keys(await entryOf(dir)).sort();
  assertEqual(
    names.join(" "),
    "DhfstAcceptor DhfstErrorModel HfstError OptimisedTransducer Tokeniser Transducer init readDhfst readOptimisedTransducer readTokeniser readTransducer readTransducers setLogLevel setLogger version",
    "exports",
  );
});

// [spec:hfst:req:javascript.compile-module/test]
test("the compile entry module exports the compile API", async () => {
  const names = Object.keys(await entryOf(dir, "./compile")).sort();
  assertEqual(
    names.join(" "),
    "HfstError Tokeniser Transducer compileLexc compilePmatch compileStrings compileTwolc compileXfst flookupReport grep init lookupReport optimizedLookupReport packBhfst pairTest parseAtt parseProlog readTokeniser readTransducer readTransducers regex setLogLevel setLogger version",
    "exports",
  );
});

await runTests("hfst/package");
