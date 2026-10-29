// Builds the transducer fixtures the JavaScript tests read, with this
// checkout's hfst command-line tools, from the text sources in
// js/test/fixtures. They are made at test-build time rather than committed,
// so the bytes a test compares against are what the checkout's own native
// library writes. Only `node js/build.mjs --test` calls this.

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { buildDhfstFixtures } from "./dhfst-fixtures.mjs";
import { buildHfstJsFixtures } from "./hfst-js-fixtures.mjs";
import { buildLookupFixtures } from "./lookup-fixtures.mjs";
import { buildTokeniseFixtures } from "./tokenise-fixtures.mjs";

const sources = join(dirname(fileURLToPath(import.meta.url)), "fixtures");

/** The transducer types the package reads, under the names both the tools and the API use. */
const READ = ["optimized-lookup-weighted", "optimized-lookup-unweighted"];

/** Writes every fixture to `out`, using the hfst binary at `hfst`. */
export function buildFixtures(hfst, out) {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const at = (name) => join(out, name);
  const source = (name) => join(sources, name);
  const tool = (...args) => execFileSync(hfst, [...args, "-q"], { stdio: ["ignore", "ignore", "pipe"] });
  const concat = (name, ...parts) => writeFileSync(at(name), Buffer.concat(parts.map((p) => readFileSync(at(p)))));

  // One transducer in each type, headered and headerless: the two the package
  // reads, and tropical OpenFst (what regexp2fst writes) and foma, which the
  // package turns down.
  tool("regexp2fst", source("words.regexp"), "-o", at("words.openfst-tropical.hfst"));
  for (const type of [...READ, "foma"]) {
    tool("fst2fst", "-f", type, at("words.openfst-tropical.hfst"), "-o", at(`words.${type}.hfst`));
  }
  for (const type of ["openfst-tropical", ...READ, "foma"]) {
    tool("fst2fst", "-b", "-f", type, at("words.openfst-tropical.hfst"), "-o", at(`words.${type}.bare`));
  }
  // A native foma file is gzip-compressed, and foma also reads it uncompressed.
  writeFileSync(at("words.foma.plain"), gunzipSync(readFileSync(at("words.foma.bare"))));

  // Several transducers concatenated, which an HFST3 stream is.
  for (const name of ["first", "second"]) {
    tool("edit-metadata", "-a", `name=${name}`, at("words.optimized-lookup-weighted.hfst"), "-o", at(`${name}.hfst`));
  }
  concat("ordered.hfst", "first.hfst", "second.hfst");
  concat("ordered.bare", "words.optimized-lookup-weighted.bare", "words.optimized-lookup-weighted.bare");
  concat("mixed.hfst", "words.optimized-lookup-weighted.hfst", "words.optimized-lookup-unweighted.hfst");

  tool("pmatch2fst", source("animals.pmatch"), "-o", at("animals.pmhfst"));

  buildLookupFixtures(tool, out, sources, READ);
  buildTokeniseFixtures(hfst, sources, out);
  buildDhfstFixtures(tool, out, sources);
  buildHfstJsFixtures(tool, out, sources, READ);
}

/** Builds the hfst tools with cargo and returns the binary's path. */
export function buildTools(root, run) {
  run("cargo", ["build", "-p", "hfst-cli"]);
  return join(root, "target", "debug", "hfst");
}
