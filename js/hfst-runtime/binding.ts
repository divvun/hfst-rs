import { Core } from "../shared/core.ts";
import * as glue from "./pkg/hfst_wasm_runtime.js";

/** This package's module instance. Making it loads nothing. */
export const core: Core = new Core(
  glue,
  new URL("./pkg/hfst_wasm_runtime_bg.wasm", import.meta.url),
);
