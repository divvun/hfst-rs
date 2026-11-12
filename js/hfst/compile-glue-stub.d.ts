// What deno check sees in place of pkg-compile/hfst_wasm_compile.js, which
// only exists after a build, as glue-stub.d.ts stands in for the main
// module's. tsc compiles against the real one.
import type { CompileGlue } from "./compile-glue.ts";

declare const init: CompileGlue["default"];
export default init;
export declare const install: CompileGlue["install"];
export declare const setLogLevel: CompileGlue["setLogLevel"];
export declare const version: CompileGlue["version"];
export declare const TransducerList: CompileGlue["TransducerList"];
export declare const readTransducers: CompileGlue["readTransducers"];
export declare const readTokeniser: CompileGlue["readTokeniser"];
export declare const regex: CompileGlue["regex"];
export declare const compileLexc: CompileGlue["compileLexc"];
export declare const compileTwolc: CompileGlue["compileTwolc"];
export declare const compileXfst: CompileGlue["compileXfst"];
export declare const compilePmatch: CompileGlue["compilePmatch"];
export declare const compileText: CompileGlue["compileText"];
export declare const lookupReport: CompileGlue["lookupReport"];
export declare const flookupReport: CompileGlue["flookupReport"];
export declare const optimizedLookupReport: CompileGlue["optimizedLookupReport"];
export declare const pairTest: CompileGlue["pairTest"];
export declare const grep: CompileGlue["grep"];
export declare const packBhfst: CompileGlue["packBhfst"];
