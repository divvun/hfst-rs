// Tests for the checks js/build.mjs runs on what it builds.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { assertNoAny } from "./build.mjs";
import { assert, runTests, test } from "./test/harness.ts";

function withDeclarations(files, body) {
  const dir = mkdtempSync(join(tmpdir(), "hfst-build-test-"));
  try {
    for (const [path, text] of Object.entries(files)) {
      mkdirSync(dirname(join(dir, path)), { recursive: true });
      writeFileSync(join(dir, path), text);
    }
    body(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("declarations that mention any only in comments pass", () => {
  withDeclarations(
    {
      "index.d.ts": "/** Takes any string. */\nexport declare function f(x: string): number; // any\n",
    },
    (dir) => assertNoAny(dir),
  );
});

test("a declaration that uses any fails the build and names the file", () => {
  withDeclarations({ "shared/api.d.ts": "export declare const f: (x: any) => void;\n" }, (dir) => {
    let message = "";
    try {
      assertNoAny(dir);
    } catch (error) {
      message = error.message;
    }
    assert(message.includes(join("shared", "api.d.ts")), `the failure names the file: ${message}`);
  });
});

test("the generated glue under pkg is not checked", () => {
  withDeclarations({ "hfst/pkg/glue.d.ts": "export function install(sink: any): void;\n" }, (dir) =>
    assertNoAny(dir),
  );
});

await runTests("build");
