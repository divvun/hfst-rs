// Builds what js/test/compile.ts compares the compilers against: the bytes
// this checkout's hfst tools write for the sources in js/test/fixtures/compile.
// The tools run in a copy of that directory, so the files a source names are
// read under the same names the test's file maps give them, and lexc names
// its transducer after words.lexc as the test's sourceName does.

import { execFileSync } from "node:child_process";
import { cpSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const sources = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "compile");

/** `types` are the transducer types the tools and the API both name. */
export function buildCompileFixtures(hfst, out, types) {
  const dir = join(out, "compile");
  cpSync(sources, dir, { recursive: true });
  const tool = (...args) => execFileSync(hfst, [...args, "-q"], { cwd: dir, stdio: ["ignore", "ignore", "pipe"] });

  tool("regexp2fst", "animals.regexp", "-o", "animals.hfst");
  for (const type of types) {
    tool("regexp2fst", "-f", type, "refs.regexp", "-o", `regex.${type}.hfst`);
    tool("lexc", "-f", type, "words.lexc", "-o", `lexc.${type}.hfst`);
  }
  tool("regexp2fst", "-S", "-j", "-M", "several.regexp", "-o", "regex.options.hfst");
  tool("lexc", "-F", "-A", "words.lexc", "-o", "lexc.options.hfst");
  for (const type of ["openfst-tropical", "foma"]) {
    tool("twolc", "-f", type, "rules.twolc", "-o", `twolc.${type}.hfst`);
  }
  tool("twolc", "-R", "-D", "rules.twolc", "-o", "twolc.options.hfst");
  tool("pmatch2fst", "tokens.pmatch", "-o", "tokens.pmhfst");
  tool("pmatch2fst", "--flatten", "tokens.pmatch", "-o", "tokens.flatten.pmhfst");
}
