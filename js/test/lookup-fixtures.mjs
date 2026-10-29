// The lookup fixtures: transducers built from the regular expressions in
// js/test/fixtures, and what this checkout's `hfst lookup` prints for every
// line of lookup-inputs.txt in each of them. js/test/lookup.ts compares the
// package's lookups against those prints. Only js/test/fixtures.mjs calls
// this, after it has built words.*.hfst.

import { copyFileSync } from "node:fs";
import { join } from "node:path";

/** The regular expressions whose transducers every lookup test reads. */
const LOOKUP_SOURCES = ["words", "flags", "ties"];

/**
 * Writes the lookup fixtures to `out` with `tool`, which runs one hfst
 * subcommand quietly. `types` are the optimized-lookup types to build; fst2fst
 * makes each from the tropical OpenFst transducer regexp2fst writes. For each
 * source and type this writes
 *
 * - `<source>.<type>.hfst`, the transducer;
 * - `lookup.<source>.<type>.txt`, what `hfst lookup -X print-space` prints for
 *   every input, a block of result lines per input;
 * - `lookup.<source>.<type>.limit1.txt`, the same under `-n 1`.
 *
 * `many.optimized-lookup-weighted.hfst` maps `aaaaaaa` to its 4^7 analyses,
 * for a time limit to cut short.
 */
export function buildLookupFixtures(tool, out, sources, types) {
  const at = (name) => join(out, name);
  const inputs = at("lookup-inputs.txt");
  copyFileSync(join(sources, "lookup-inputs.txt"), inputs);
  for (const source of LOOKUP_SOURCES.filter((name) => name !== "words")) {
    tool("regexp2fst", join(sources, `${source}.regexp`), "-o", at(`${source}.openfst-tropical.hfst`));
    for (const type of types) {
      tool("fst2fst", "-f", type, at(`${source}.openfst-tropical.hfst`), "-o", at(`${source}.${type}.hfst`));
    }
  }
  for (const source of LOOKUP_SOURCES) {
    for (const type of types) {
      const transducer = at(`${source}.${type}.hfst`);
      const lookup = (name, ...options) =>
        tool("lookup", "-X", "print-space", ...options, "-I", inputs, transducer, "-o", at(name));
      lookup(`lookup.${source}.${type}.txt`);
      lookup(`lookup.${source}.${type}.limit1.txt`, "-n", "1");
    }
  }
  tool("regexp2fst", join(sources, "many.regexp"), "-o", at("many.openfst-tropical.hfst"));
  tool("fst2fst", "-w", at("many.openfst-tropical.hfst"), "-o", at("many.optimized-lookup-weighted.hfst"));
}
