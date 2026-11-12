import type { CompileGlue } from "./compile-glue.ts";
import { Core } from "./core.ts";
import * as glue from "./pkg-compile/hfst_wasm_compile.js";

// [spec:hfst:def:javascript.packages+3]
// The compile entry point's .wasm file ships in pkg-compile/, beside the
// compiled entry module. Its glue is generated into a directory of its own,
// so its copy of the sink snippet is a module of its own, and the two module
// instances never share a sink. Nothing here names the main module's.
/** The compile entry point's module instance. Making it loads nothing. */
export const core: Core<CompileGlue> = new Core<CompileGlue>(
  glue,
  new URL("./pkg-compile/hfst_wasm_compile_bg.wasm", import.meta.url),
  "@divvun/hfst/compile",
);
