// The fixtures js/test/hfst-js-parity.ts compares this package with hfst-js
// on, beyond the lookup fixtures. Each is built in both optimized-lookup types
// from a regular expression in js/test/fixtures:
//
// - weights: weights that are not short binary fractions, epsilons and
//   multichar output, for the 32-bit weight sums;
// - nonascii: Sámi and Greenlandic letters and a symbol outside the BMP;
// - identity: an identity arc, which hfst-js never follows;
// - prefix: a symbol that begins like a longer one;
// - shortflag: a flag of five characters;
// - unify: unification after a negative set.
//
// It also installs hfst-js in js/bench when it is not there, which needs the
// network once. Only js/test/fixtures.mjs calls this.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HFST_JS_SOURCES = ["weights", "nonascii", "identity", "prefix", "shortflag", "unify"];

const bench = join(dirname(dirname(fileURLToPath(import.meta.url))), "bench");

/** Writes `<source>.<type>.hfst` to `out` for every source and each of `types`, with `tool`. */
export function buildHfstJsFixtures(tool, out, sources, types) {
  for (const source of HFST_JS_SOURCES) {
    const tropical = join(out, `${source}.openfst-tropical.hfst`);
    tool("regexp2fst", join(sources, `${source}.regexp`), "-o", tropical);
    for (const type of types) {
      tool("fst2fst", "-f", type, tropical, "-o", join(out, `${source}.${type}.hfst`));
    }
  }
  if (!existsSync(join(bench, "node_modules", "hfst-js", "package.json"))) {
    console.log(`$ npm ci (in ${bench})`);
    execFileSync("npm", ["ci"], { cwd: bench, stdio: "inherit" });
  }
}
