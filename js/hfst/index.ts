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

export const { init, version } = runtimeApi(core);
export { HfstError } from "../shared/errors.ts";
export type { HfstErrorCode } from "../shared/errors.ts";
export type { InitSource } from "../shared/core.ts";
