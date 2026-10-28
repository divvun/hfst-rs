import { entryOf, packageTests } from "../test/packages.ts";
import { assertEqual, runTests, test } from "../test/harness.ts";

const dir = new URL("./", import.meta.url);
packageTests(dir, "@divvun/hfst-runtime");

test("the entry module exports the runtime API", async () => {
  const names = Object.keys(await entryOf(dir)).sort();
  assertEqual(names.join(" "), "HfstError init version", "exports");
});

await runTests("hfst-runtime/package");
