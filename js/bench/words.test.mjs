// Tests for how js/bench/words.mjs reads Giella yaml tests, running text and
// fst2strings output into word lists.

import { assertEqual, runTests, test } from "../test/harness.ts";
import { inputSides, textWords, yamlTests } from "./words.mjs";

const json = (value) => JSON.stringify(value);

const YAML = `Config:
  hfst:
    Gen: ../../src/generator-gt-desc.hfst
    Morph: ../../src/analyser-gt-desc.hfst

Tests:
  Noun - bietna: # -a stem, root vowel -ie-
    bietna+N+Sg+Nom: bietna
    bietna+N+Sg+Gen: [bietnaga, bietnaga ]
    "beana#gáddi+N+Sg+Nom": beanagáddi  # a compound
    bietna+N+Pl+Nom: ["bietnagat", 'bietnagat']
  Empty+N: []

Other:
  ignored+N: ignored
`;

test("yaml tests give each analysis with its surface forms, under Tests: only", () => {
  assertEqual(
    json(yamlTests(YAML)),
    json([
      { key: "bietna+N+Sg+Nom", values: ["bietna"] },
      { key: "bietna+N+Sg+Gen", values: ["bietnaga", "bietnaga"] },
      { key: "beana#gáddi+N+Sg+Nom", values: ["beanagáddi"] },
      { key: "bietna+N+Pl+Nom", values: ["bietnagat", "bietnagat"] },
      { key: "Empty+N", values: [] },
    ]),
    "entries",
  );
});

test("a group heading followed by a comment is not an entry", () => {
  assertEqual(json(yamlTests("Tests:\n  Proper - Oslo: # place\n    Oslo+N+Prop: Oslo\n").map(({ key }) => key)), json(["Oslo+N+Prop"]), "keys");
});

test("running text gives its word forms, inner hyphens and apostrophes kept", () => {
  assertEqual(
    json(textWords("Oqaasileriffik. 'S é an t-uisce, (1984) -- ’ ... o'clock!\nčáhci-")),
    json(["Oqaasileriffik", "S", "é", "an", "t-uisce", "1984", "o'clock", "čáhci"]),
    "words",
  );
  assertEqual(json(textWords(" -- ' ... ")), json([]), "punctuation alone gives no words");
});

test("fst2strings lines give their input sides, and a line with a colon in it is left out", () => {
  assertEqual(
    json(inputSides("cat:cat+N\nMBÁC:BÁC+Noun\n:::+Punct\na:b:c\n\nnuna:nuna+N+Abs+Sg\n")),
    json(["cat", "MBÁC", "nuna"]),
    "inputs",
  );
});

await runTests("bench/words");
