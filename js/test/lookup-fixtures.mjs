// The lookup fixtures: transducers built from the regular expressions in
// js/test/fixtures, and what this checkout's `hfst lookup` prints for every
// line of lookup-inputs.txt in each of them. js/test/lookup.ts and
// js/test/lookup-full.ts compare the packages' lookups against those prints.
// Only js/test/fixtures.mjs calls this, after it has built words.*.hfst.

import { copyFileSync } from "node:fs";
import { join } from "node:path";

/** The regular expressions whose transducers every lookup test reads. */
const LOOKUP_SOURCES = ["words", "flags", "ties"];

/**
 * Writes the lookup fixtures to `out` with `tool`, which runs one hfst
 * subcommand quietly. `types` are the transducer types to build, the first
 * being the one regexp2fst writes. For each source and type this writes
 *
 * - `<source>.<type>.hfst`, the transducer;
 * - `lookup.<source>.<type>.txt`, what `hfst lookup -X print-space` prints for
 *   every input, a block of result lines per input;
 * - for the optimized-lookup types, `lookup.<source>.<type>.limit1.txt`, the
 *   same under `-n 1`, which `hfst lookup` honours only for them.
 *
 * `many.optimized-lookup-weighted.hfst` maps `aaaaaaa` to its 4^7 analyses,
 * for a time limit to cut short.
 */
export function buildLookupFixtures(tool, out, sources, types) {
  const at = (name) => join(out, name);
  const inputs = at("lookup-inputs.txt");
  copyFileSync(join(sources, "lookup-inputs.txt"), inputs);
  const [compiled, ...converted] = types;
  for (const source of LOOKUP_SOURCES.filter((name) => name !== "words")) {
    tool("regexp2fst", join(sources, `${source}.regexp`), "-o", at(`${source}.${compiled}.hfst`));
    for (const type of converted) {
      tool("fst2fst", "-f", type, at(`${source}.${compiled}.hfst`), "-o", at(`${source}.${type}.hfst`));
    }
  }
  for (const source of LOOKUP_SOURCES) {
    for (const type of types) {
      const transducer = at(`${source}.${type}.hfst`);
      const lookup = (name, ...options) =>
        tool("lookup", "-X", "print-space", ...options, "-I", inputs, transducer, "-o", at(name));
      lookup(`lookup.${source}.${type}.txt`);
      if (type.startsWith("optimized-lookup")) lookup(`lookup.${source}.${type}.limit1.txt`, "-n", "1");
    }
  }
  tool("regexp2fst", join(sources, "many.regexp"), "-o", at(`many.${compiled}.hfst`));
  tool("fst2fst", "-w", at(`many.${compiled}.hfst`), "-o", at("many.optimized-lookup-weighted.hfst"));
}
