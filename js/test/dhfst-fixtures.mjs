// The DHFST fixtures: acceptors and an error model that this checkout's
// `hfst fst2fst -f dhfst` writes from AT&T sources, and what `hfst lookup`
// prints for every line of speller-inputs.txt in the weighted optimized-lookup
// transducer the speller acceptor is written from. js/test/dhfst.ts compares
// the package's acceptor lookups against that print. Only js/test/fixtures.mjs
// calls this.

import { copyFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/**
 * An acceptor whose symbols are not all one grapheme cluster in the form the
 * input spells them: `ij` and `xy` are multicharacter symbols, `é` is stored
 * composed and `ö` decomposed. It is written out from escapes here because a
 * text file's normalisation form does not survive every editor and checkout.
 */
const GRAPHEMES = [
  "0\t1\tij\tij\t0.5",
  "1\t2\ta\ta\t0",
  "2\t0",
  "0\t3\ti\ti\t1",
  "3\t4\tj\tj\t1",
  "4\t5\ta\ta\t0",
  "5\t0",
  "0\t6\txy\txy\t0",
  "6\t0",
  "0\t7\t\u00e9\t\u00e9\t0.25",
  "7\t0",
  "0\t8\to\u0308\to\u0308\t0.75",
  "8\t0",
];

/**
 * Writes the DHFST fixtures to `out` with `tool`, which runs one hfst
 * subcommand quietly, from the sources in `sources`. For `speller.att` and
 * the grapheme acceptor above this writes `<name>.acceptor.dhfst` and
 * `<name>.optimized-lookup-weighted.hfst`, the transducer it is written from;
 * `lookup.speller.txt`, what `hfst lookup -X print-space` prints for each line
 * of `speller-inputs.txt` in that transducer; and `errmodel.error-model.dhfst`
 * from `errmodel.att`.
 */
export function buildDhfstFixtures(tool, out, sources) {
  const at = (name) => join(out, name);
  writeFileSync(at("graphemes.att"), GRAPHEMES.map((line) => `${line}\n`).join(""));
  copyFileSync(join(sources, "speller.att"), at("speller.att"));
  copyFileSync(join(sources, "errmodel.att"), at("errmodel.att"));
  copyFileSync(join(sources, "speller-inputs.txt"), at("speller-inputs.txt"));
  for (const name of ["speller", "graphemes", "errmodel"]) {
    tool("txt2fst", at(`${name}.att`), "-o", at(`${name}.openfst-tropical.hfst`));
  }
  for (const name of ["speller", "graphemes"]) {
    const source = at(`${name}.openfst-tropical.hfst`);
    tool("fst2fst", "-f", "dhfst", "--dhfst-type", "acceptor", source, "-o", at(`${name}.acceptor.dhfst`));
    tool("fst2fst", "-w", source, "-o", at(`${name}.optimized-lookup-weighted.hfst`));
  }
  const speller = at("speller.optimized-lookup-weighted.hfst");
  tool("lookup", "-X", "print-space", "-I", at("speller-inputs.txt"), speller, "-o", at("lookup.speller.txt"));
  const errmodel = at("errmodel.openfst-tropical.hfst");
  tool("fst2fst", "-f", "dhfst", "--dhfst-type", "errmodel", errmodel, "-o", at("errmodel.error-model.dhfst"));
}
