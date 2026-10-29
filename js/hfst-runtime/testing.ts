// The entry module of a test build: the package's API and its test hooks.
// js/build.mjs compiles it only with --test, into target/js-test, so no
// published package contains it.
import { testHooks } from "../shared/testing.ts";
import { core } from "./binding.ts";
import * as glue from "./pkg/hfst_wasm_runtime.js";

export * from "./index.ts";
export const testing = testHooks(core, glue);
