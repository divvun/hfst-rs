// [spec:hfst:def:javascript.packages]
/**
 * `@divvun/hfst-runtime`: the Divvun HFST runtime for JavaScript. It loads
 * optimized-lookup transducers and pmatch containers and runs lookup and
 * tokenisation, in browsers, Deno, Node and Bun. `@divvun/hfst` exports all
 * of this under the same names.
 *
 * @module
 */
import { runtimeApi } from "../shared/api.ts";
import { core } from "./binding.ts";

export const { init, version, setLogger, setLogLevel } = runtimeApi(core);
export { HfstCompileError, HfstError } from "../shared/errors.ts";
export type {
  Diagnostic,
  DiagnosticLabel,
  HfstErrorCode,
  Span,
} from "../shared/errors.ts";
export type { InitSource } from "../shared/core.ts";
export type { LogEvent, Logger, LogLevel } from "../shared/logging.ts";
