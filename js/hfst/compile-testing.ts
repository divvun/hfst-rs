// The compile entry module of a test build: the compile entry point's API
// and its module's test hooks. js/build.mjs compiles it only with --test,
// into target/js-test, so the published package never contains it.

import { core } from "./compile-binding.ts";
import * as glue from "./pkg-compile/hfst_wasm_compile.js";
import { type TestHooks, testHooks } from "./test-hooks.ts";

export * from "./compile.ts";

export const testing: TestHooks = testHooks(core, glue);
