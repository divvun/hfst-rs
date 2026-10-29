// Builds the tokeniser fixtures: the pmatch container compiled from
// fixtures/tokenise.pmatch, and for each case in fixtures/tokenise-cases.json
// what this checkout's hfst tokenise prints for the case's text and flags.
// Each case pairs the flags with the API options that stand for them, so the
// tests compare the API against the command line byte for byte.

import { execFileSync } from "node:child_process";
import { copyFileSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export function buildTokeniseFixtures(hfst, sources, out) {
  const archive = join(out, "tokenise.pmhfst");
  execFileSync(hfst, ["pmatch2fst", join(sources, "tokenise.pmatch"), "-o", archive, "-q"], {
    stdio: ["ignore", "ignore", "pipe"],
  });
  copyFileSync(join(sources, "tokenise-cases.json"), join(out, "tokenise-cases.json"));
  const { texts, formats, tokens } = JSON.parse(readFileSync(join(sources, "tokenise-cases.json"), "utf8"));
  for (const { name, text, flags } of [...formats, ...tokens]) {
    const printed = execFileSync(hfst, ["tokenise", ...flags, archive], {
      input: texts[text],
      stdio: ["pipe", "pipe", "pipe"],
    });
    writeFileSync(join(out, `tokenise.${name}.out`), printed);
  }
}
