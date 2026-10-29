import type { TokeniserGlue } from "../shared/glue.ts";
import { TokeniserBase, tokeniserReader } from "../shared/tokenise.ts";
import { core } from "./binding.ts";
import * as glue from "./pkg/hfst_wasm.js";

let wrap: (glue: TokeniserGlue) => Tokeniser;

// [spec:hfst:req:javascript.tokenise]
/**
 * A tokeniser loaded from a pmatch container, as `hfst tokenise` loads one.
 * It owns module memory: call `free()` when done with it, or declare it with
 * `using`.
 */
export class Tokeniser extends TokeniserBase {
  private constructor(glue: TokeniserGlue) {
    super(core, glue);
  }

  static {
    wrap = (glue) => new Tokeniser(glue);
  }
}

export const { readTokeniser } = tokeniserReader((bytes) =>
  core.call(() => wrap(glue.readTokeniser(bytes))),
);
