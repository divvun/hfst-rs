// [spec:hfst:def:javascript.packages]
/**
 * `@divvun/hfst`: Divvun HFST for JavaScript. Everything
 * `@divvun/hfst-runtime` exports, under the same names, with the whole hfst
 * library behind it, in browsers, Deno, Node and Bun.
 *
 * @module
 */
import { runtimeApi } from "../shared/api.ts";
import { core } from "./binding.ts";

export const { init, version, setLogger, setLogLevel } = runtimeApi(core);
export {
  readAtt,
  readTransducer,
  readTransducers,
  Transducer,
  writeTransducers,
} from "./transducer.ts";
export type { TransducerType } from "./transducer.ts";
export type { TransducerProperties } from "../shared/transducer.ts";
export { HfstCompileError, HfstError } from "../shared/errors.ts";
export type {
  Diagnostic,
  DiagnosticLabel,
  HfstErrorCode,
  Span,
} from "../shared/errors.ts";
export type { InitSource } from "../shared/core.ts";
export type { LogEvent, Logger, LogLevel } from "../shared/logging.ts";
export type { LookupOptions, LookupResult } from "../shared/lookup.ts";
