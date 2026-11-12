// Builds the transducer fixtures the JavaScript tests read, with this
// checkout's hfst command-line tools, from the text sources in
// js/test/fixtures. They are made at test-build time rather than committed,
// so the bytes a test compares against are what the checkout's own native
// library writes. `node js/build.mjs --test` writes them to
// target/js-test/fixtures. The Python package's tests read the same ones, and
// build them where they want them by running this module:
//
//   node js/test/fixtures.mjs <out>

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gunzipSync } from "node:zlib";
import { buildCompileFixtures } from "./compile-fixtures.mjs";
import { buildDhfstFixtures } from "./dhfst-fixtures.mjs";
import { buildHfstJsFixtures } from "./hfst-js-fixtures.mjs";
import { buildLookupFixtures } from "./lookup-fixtures.mjs";
import { buildOptimisedFixtures } from "./optimised-fixtures.mjs";
import { buildParityFixtures } from "./parity-fixtures.mjs";
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
  // THFST, divvunspell's format, is a directory of three files.
  tool("fst2fst", "-f", "thfst", at("words.openfst-tropical.hfst"), "-o", at("words.thfst"));

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
  buildParityFixtures(hfst, out);
  buildHfstJsFixtures(tool, out, sources, READ);
  buildOptimisedFixtures(out);
  buildCompileFixtures(hfst, out);
}

/**
 * Builds the hfst tools with cargo and returns the binary's path, under the
 * target directory cargo uses, which CARGO_TARGET_DIR or a cargo config may
 * move out of the checkout.
 */
export function buildTools(root, run) {
  run("cargo", ["build", "-p", "hfst-cli"]);
  const metadata = execFileSync("cargo", ["metadata", "--format-version=1", "--no-deps"], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  const suffix = process.platform === "win32" ? ".exe" : "";
  return join(JSON.parse(metadata).target_directory, "debug", `hfst${suffix}`);
}

// The JS build imports this module for its functions, so it builds only when run.
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [out] = process.argv.slice(2);
  if (out === undefined) {
    console.error("usage: node js/test/fixtures.mjs <out>");
    process.exit(2);
  }
  const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
  const run = (command, args) => {
    console.log(`$ ${command} ${args.join(" ")}`);
    execFileSync(command, args, { cwd: root, stdio: "inherit" });
  };
  buildFixtures(buildTools(root, run), resolve(out));
}
