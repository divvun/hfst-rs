// The entry module of a test build: the package's API and its test hooks.
// js/build.mjs compiles it only with --test, into target/js-test, so the
// published package never contains it.

import { core } from "./binding.ts";
import * as glue from "./pkg/hfst_wasm.js";
import { type TestHooks, testHooks } from "./test-hooks.ts";

export * from "./index.ts";
export { TestObject } from "./test-hooks.ts";

export const testing: TestHooks = testHooks(core, glue);
