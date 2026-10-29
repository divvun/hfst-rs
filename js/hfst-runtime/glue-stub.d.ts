// What deno check sees in place of pkg/hfst_wasm_runtime.js, which only exists after a
// build. js/deno.json maps that path here. tsc compiles against the real one.
import type { RuntimeGlue } from "../shared/glue.ts";

declare const init: RuntimeGlue["default"];
export default init;
export declare const install: RuntimeGlue["install"];
export declare const setLogLevel: RuntimeGlue["setLogLevel"];
export declare const version: RuntimeGlue["version"];
export declare const readTransducers: RuntimeGlue["readTransducers"];
export declare const readTokeniser: import("../shared/glue.ts").TokeniseGlue["readTokeniser"];
