import { entryOf, packageTests } from "../test/packages.ts";
import { assert, runTests, test } from "../test/harness.ts";

const dir = new URL("./", import.meta.url);
packageTests(dir, "@divvun/hfst");

test("the entry module exports everything the runtime package does", async () => {
  const full = await entryOf(dir);
  const runtime = await entryOf(new URL("../hfst-runtime/", import.meta.url));
  for (const name of Object.keys(runtime)) {
    assert(typeof full[name] === typeof runtime[name], `@divvun/hfst exports ${name}`);
  }
});

await runTests("hfst/package");
