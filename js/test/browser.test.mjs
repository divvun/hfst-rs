// Tests for how browser.mjs finds the Chromium the parity suite runs in.

import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { findBrowser } from "./browser.mjs";
import { assertEqual, runTests, test } from "./harness.ts";

/** A directory holding `files` (path to whether it is executable), removed after `body`. */
function withFiles(files, body) {
  const dir = mkdtempSync(join(tmpdir(), "hfst-browser-test-"));
  try {
    for (const [path, executable] of Object.entries(files)) {
      mkdirSync(dirname(join(dir, path)), { recursive: true });
      writeFileSync(join(dir, path), "");
      chmodSync(join(dir, path), executable ? 0o755 : 0o644);
    }
    body(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("HFST_JS_TEST_BROWSER names the browser, whatever else is installed", () => {
  assertEqual(findBrowser({ HFST_JS_TEST_BROWSER: "/opt/chrome", PATH: "" }), "/opt/chrome", "the browser");
});

test("Playwright's newest headless shell comes before its full Chromium builds", () => {
  const files = {
    "chromium-1300/chrome-linux64/chrome": true,
    "chromium_headless_shell-1100/chrome-headless-shell-linux64/chrome-headless-shell": true,
    "chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell": true,
  };
  withFiles(files, (cache) => {
    const found = findBrowser({ PLAYWRIGHT_BROWSERS_PATH: cache, PATH: "" });
    assertEqual(found, join(cache, "chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell"), "the browser");
  });
});

test("a build whose browser is not executable is passed over", () => {
  const files = {
    "chromium_headless_shell-1243/chrome-headless-shell-linux64/chrome-headless-shell": false,
    "chromium-1243/chrome-linux64/chrome": true,
  };
  withFiles(files, (cache) => {
    const found = findBrowser({ PLAYWRIGHT_BROWSERS_PATH: cache, PATH: "" });
    assertEqual(found, join(cache, "chromium-1243/chrome-linux64/chrome"), "the browser");
  });
});

await runTests("parity browser");
