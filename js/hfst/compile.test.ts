// The full package's compilers against the built package: what each returns
// and throws, where its diagnostics point, and the files it reads from the
// map. js/test/compile.ts checks the bytes they write against the hfst tools.

import { assert, assertEqual, readBytes, runTests, test, thrownCode } from "../test/harness.ts";
import { entryOf } from "../test/packages.ts";
import type * as Full from "./index.ts";

const dir = new URL("./", import.meta.url);
const pkg = (await entryOf(dir)) as unknown as typeof Full;
await pkg.init(await readBytes(new URL("dist/hfst/pkg/hfst_wasm_bg.wasm", dir).href));
// The library logs the warnings it has no diagnostic for; these tests read
// only the diagnostics.
pkg.setLogLevel("off");

function compileError(f: () => unknown): Full.HfstCompileError {
  try {
    f();
  } catch (error) {
    assert(error instanceof pkg.HfstCompileError, `an HfstCompileError, not ${String(error)}`);
    assertEqual(error.code, "compile-error", "code");
    return error;
  }
  throw new Error("the compile succeeded");
}

/** Checks `d` against the severity, message, source name and UTF-16 span expected. */
function assertDiagnostic(
  d: Full.Diagnostic | undefined,
  expected: Pick<Full.Diagnostic, "severity" | "message" | "sourceName" | "start" | "end">,
  what: string,
): asserts d is Full.Diagnostic {
  assert(d !== undefined, `${what}: a diagnostic`);
  for (const key of ["severity", "message", "sourceName", "start", "end"] as const) {
    assertEqual(d[key], expected[key], `${what}: ${key}`);
  }
  assertEqual(d.labels[0]?.start, d.start, `${what}: the primary label`);
}

function hasArc(t: Full.Transducer, symbol: string): boolean {
  return t.toAtt().split("\n").some((line) => line.split("\t")[2] === symbol);
}

const NOUNS = "{dog} | {cat}";

// [spec:hfst:req:javascript.compile/test]
test("compileRegex makes one transducer named as hfst regexp2fst names it", () => {
  const compiled = pkg.compileRegex(NOUNS);
  using t = compiled.transducer;
  assertEqual(t.type, "openfst-tropical", "the default type");
  assertEqual(t.name, "xre(?)", "name");
  assertEqual(compiled.diagnostics.length, 0, "diagnostics");
  using foma = pkg.compileRegex(NOUNS, { type: "foma" }).transducer;
  assertEqual(foma.type, "foma", "type");
});

// [spec:hfst:req:javascript.compile/test]
test("compileRegex returns its warnings, with UTF-16 offsets after non-ASCII text", () => {
  const source = 'á "<x>" b';
  const compiled = pkg.compileRegex(source, { verbose: true, sourceName: "w.re" });
  compiled.transducer.free();
  const [d] = compiled.diagnostics;
  assertDiagnostic(
    d,
    { severity: "warning", message: "'<x>' is an ordinary symbol in hfst", sourceName: "w.re", start: 2, end: 7 },
    "warning",
  );
  assertEqual(source.slice(d.start, d.end), '"<x>"', "the span marks the symbol");
  assert(d.rendered.startsWith("Warning: '<x>'") && d.rendered.includes("w.re:1:3"), d.rendered);
  using quiet = pkg.compileRegex(source).transducer;
  assertEqual(quiet.name, "xre(?)", "without verbose, no warning stops it");
});

// [spec:hfst:req:javascript.compile/test]
test("a regular expression that does not compile throws every diagnostic, placed in the source", () => {
  const error = compileError(() => pkg.compileRegex("a\nö [b\n", { sourceName: "w.re" }));
  assertEqual(error.message, "w.re did not compile: expected RightBracket", "message");
  const [d] = error.diagnostics;
  assertDiagnostic(
    d,
    { severity: "error", message: "expected RightBracket", sourceName: "w.re", start: 6, end: 6 },
    "error",
  );
  assert(d.rendered.includes("w.re:2:5"), d.rendered);
  const semicolons = compileError(() => pkg.compileRegex("a ;\nö [b ;\n", { semicolon: true }));
  assertDiagnostic(
    semicolons.diagnostics[0],
    { severity: "error", message: "expected RightBracket", sourceName: "<regex>", start: 9, end: 10 },
    "semicolon-separated",
  );
  const comments = compileError(() => pkg.compileRegex("! only a comment\n"));
  assert(comments.message.includes("only whitespace and comments"), comments.message);
});

// [spec:hfst:req:javascript.compile/test]
test("compileRegex reads lines or semicolons, and takes several expressions only to unite them", () => {
  assertEqual(thrownCode(pkg.HfstError, () => pkg.compileRegex("a\nb\n")), "transducer-count", "two lines");
  using lines = pkg.compileRegex("a\nb\n", { disjunct: true }).transducer;
  assert(hasArc(lines, "a") && hasArc(lines, "b"), "the union of the lines");
  const spanning = "[a |\n b] ;\nc ;\n";
  assertEqual(
    thrownCode(pkg.HfstError, () => pkg.compileRegex(spanning, { semicolon: true })),
    "transducer-count",
    "two expressions",
  );
  using union = pkg.compileRegex(spanning, { semicolon: true, disjunct: true }).transducer;
  assert(["a", "b", "c"].every((s) => hasArc(union, s)), "the union of the expressions");
});

// [spec:hfst:req:javascript.compile/test]
test("@bin and @txt read their files from the map, and a missing one is named", () => {
  using animals = pkg.compileRegex(NOUNS).transducer;
  const files = { "animals.hfst": animals.toBytes(), "farm.txt": "cow\nhorse\n" };
  using t = pkg.compileRegex('@bin"animals.hfst" | @txt"farm.txt"', { files }).transducer;
  assert(hasArc(t, "d") && hasArc(t, "h"), "both files are in it");
  const error = compileError(() => pkg.compileRegex('@txt"farm.txt"'));
  assertEqual(error.message, "<regex> did not compile: File cannot be opened: 'farm.txt'", "message");
  assertDiagnostic(
    error.diagnostics[0],
    { severity: "error", message: "File cannot be opened: 'farm.txt'", sourceName: "<regex>", start: 0, end: 14 },
    "missing file",
  );
});

// [spec:hfst:req:javascript.compile/test]
test("a diagnostic in a file of the map names the file and points into it", () => {
  const error = compileError(() => pkg.compileRegex('@re"x.re"', { files: { "x.re": "a ö [" } }));
  const [d] = error.diagnostics;
  assertEqual(d?.sourceName, "x.re", "the file's name");
  assertEqual(d?.start, 5, "an offset in the file's text");
});

const LEXC = `Multichar_Symbols +N
LEXICON Root
cat N ;
öga N ;
LEXICON N
+N:0 # ;
`;

// [spec:hfst:req:javascript.compile/test]
test("compileLexc makes one transducer named after its source, as hfst lexc names it", () => {
  const compiled = pkg.compileLexc(LEXC, { sourceName: "nouns.lexc", type: "optimized-lookup-weighted" });
  using t = compiled.transducer;
  assertEqual(t.type, "optimized-lookup-weighted", "type");
  assertEqual(t.name, "lexc(nouns.lexc)", "name");
  assertEqual(t.properties["formulaic-definition"], "L n", "formula");
  assertEqual(compiled.diagnostics.length, 0, "diagnostics");
});

// [spec:hfst:req:javascript.compile/test]
test("compileLexc returns its warnings, with UTF-16 offsets after non-ASCII text", () => {
  const source = "LEXICON Root\nö # ;\nx́y # ;\n";
  const compiled = pkg.compileLexc(source);
  compiled.transducer.free();
  const [d] = compiled.diagnostics;
  assertDiagnostic(
    d,
    {
      severity: "warning",
      message: "undeclared multi-code-point grapheme 'x́' (2 code points)",
      sourceName: "<lexc>",
      start: 19,
      end: 21,
    },
    "warning",
  );
  assertEqual(source.slice(d.start, d.end), "x́", "the span marks the grapheme");
  const switched = pkg.compileLexc("LEXICON Root\nab # ;\n", { warnings: ["missing-alphabets"] });
  switched.transducer.free();
  assertEqual(switched.diagnostics.map((w) => w.severity).join(" "), "warning warning", "-Wmissing-alphabets");
  const error = compileError(() =>
    pkg.compileLexc("LEXICON Root\nab # ;\n", { warnings: ["missing-alphabets", "error"] }),
  );
  assertEqual(error.diagnostics.length, 2, "-Werror makes both errors");
});

// [spec:hfst:req:javascript.compile/test]
test("a lexc source that does not compile throws every diagnostic, placed in the source", () => {
  const error = compileError(() => pkg.compileLexc("LEXICON Root\nöga:ögon # ;\n< a ] > # ;\n", { sourceName: "x.lexc" }));
  assertDiagnostic(
    error.diagnostics[0],
    {
      severity: "error",
      message: "error in xre lexicon entry: unexpected trailing input near Some(RightBracket)",
      sourceName: "x.lexc",
      start: 29,
      end: 30,
    },
    "error",
  );
  const empty = compileError(() => pkg.compileLexc(""));
  assert(empty.message.includes("at least one LEXICON"), empty.message);
});

// [spec:hfst:req:javascript.compile/test]
test("lexc regular expressions read their files from the map, and a missing one is named", () => {
  using animals = pkg.compileRegex(NOUNS).transducer;
  const source = 'LEXICON Root\n< @bin"animals.hfst" > # ;\n';
  using t = pkg.compileLexc(source, { files: { "animals.hfst": animals.toBytes() } }).transducer;
  assert(hasArc(t, "d"), "the file is in it");
  const error = compileError(() => pkg.compileLexc(source));
  assert(error.message.includes("'animals.hfst'"), error.message);
  const [inRegex, inEntry] = error.diagnostics;
  assertEqual(inRegex?.sourceName, "<regex>", "the regular expression lexc printed again");
  assertDiagnostic(
    inEntry,
    { severity: "error", message: "Unable to parse regular expression", sourceName: "<lexc>", start: 13, end: 39 },
    "the entry",
  );
});

const TWOLC = 'Alphabet a b c:d ;\nRules\n"ö rule"\nc:d <=> a _ ;\n';

// [spec:hfst:req:javascript.compile/test]
test("compileTwolc makes a transducer for each rule, at either type it takes", () => {
  for (const type of ["openfst-tropical", "foma"] as const) {
    const compiled = pkg.compileTwolc(TWOLC, { type });
    assertEqual(compiled.transducers.map((t) => `${t.type} ${t.name}`).join(), `${type} "ö rule"`, type);
    assertEqual(compiled.diagnostics.length, 0, "diagnostics");
    for (const t of compiled.transducers) t.free();
  }
  const badType = { type: "optimized-lookup-weighted" } as unknown as Full.TwolcOptions;
  assertEqual(thrownCode(pkg.HfstError, () => pkg.compileTwolc(TWOLC, badType)), "invalid-argument", "type");
  assertEqual(pkg.compileTwolc("Alphabet a ;\nRules\n").transducers.length, 0, "no rules, no transducers");
});

// [spec:hfst:req:javascript.compile/test]
test("compileTwolc returns its warnings and throws its errors, with UTF-16 offsets after non-ASCII text", () => {
  const compiled = pkg.compileTwolc(TWOLC.replace("a _", "x _"));
  for (const t of compiled.transducers) t.free();
  const [warning] = compiled.diagnostics;
  assertDiagnostic(
    warning,
    {
      severity: "warning",
      message: "Symbol 'x' is not declared in the Alphabet; treating it as a literal symbol.",
      sourceName: "<twolc>",
      start: 42,
      end: 43,
    },
    "warning",
  );
  assertEqual(warning.labels[0]?.message, "not declared", "the label's own message");
  assertEqual(warning.help, "If 'x' is meant, declare it in the Alphabet.", "help");
  const error = compileError(() => pkg.compileTwolc('Alphabet a b c:d ;\nRules\n"ö rule"\nc:d <=> a _\n'));
  assertDiagnostic(
    error.diagnostics[0],
    { severity: "error", message: "expected `;`, got end of input", sourceName: "<twolc>", start: 45, end: 45 },
    "error",
  );
});

const PMATCH = "define TOP {cat} EndTag(Animal) ;\n";

// [spec:hfst:req:javascript.compile/test]
test("compilePmatch makes the container, which readTransducers reads back", () => {
  const compiled = pkg.compilePmatch(PMATCH);
  assertEqual(compiled.diagnostics.length, 0, "diagnostics");
  assertEqual(compiled.transducers.map((t) => `${t.type} ${t.name}`).join(), "optimized-lookup-weighted TOP", "TOP");
  const read = pkg.readTransducers(compiled.bytes);
  assertEqual(read.length, compiled.transducers.length, "transducers in the bytes");
  for (const t of [...compiled.transducers, ...read]) t.free();
});

// [spec:hfst:req:javascript.compile/test]
test("@include and @bin read their files from the map, and a missing one is named", () => {
  using animals = pkg.compileRegex(NOUNS).transducer;
  const files = { "pets.pmatch": "define Pet {cat} ;\n", "animals.hfst": animals.toBytes() };
  const source = '@include"pets.pmatch"\ndefine Farm @bin"animals.hfst" ;\ndefine TOP Pet | Farm ;\n';
  const compiled = pkg.compilePmatch(source, { files });
  assertEqual(compiled.transducers.map((t) => t.name).join(), "TOP", "TOP");
  for (const t of compiled.transducers) t.free();
  const include = compileError(() => pkg.compilePmatch(source, { files: { "animals.hfst": files["animals.hfst"] } }));
  assertEqual(include.message, "<pmatch> did not compile: could not open file pets.pmatch for @include", "include");
  const bin = compileError(() => pkg.compilePmatch(source, { files: { "pets.pmatch": files["pets.pmatch"] } }));
  assert(bin.message.includes("animals.hfst"), bin.message);
});

// [spec:hfst:req:javascript.compile/test]
test("a pmatch script that does not compile, or has nothing to write, throws", () => {
  const error = compileError(() => pkg.compilePmatch("define TOP {ö} [ ;\n", { sourceName: "bad.pmatch" }));
  const [d] = error.diagnostics;
  assertDiagnostic(
    d,
    {
      severity: "error",
      message: "pmatch: syntax error: unexpected token in expression: Some(EndOfWeightedExpression(0.0))",
      sourceName: "bad.pmatch",
      start: 17,
      end: 18,
    },
    "error",
  );
  assert(d.rendered.includes("bad.pmatch:1:18"), d.rendered);
  const empty = compileError(() => pkg.compilePmatch(""));
  assert(empty.message.includes("nothing to write"), empty.message);
});

// [spec:hfst:req:javascript.compile/test]
test("the compilers check their source and options", () => {
  const cases: [string, () => unknown][] = [
    ["a source that is not a string", () => pkg.compileLexc(3 as unknown as string)],
    ["an option no compiler has", () => pkg.compileTwolc(TWOLC, { resolve: true } as unknown as Full.TwolcOptions)],
    ["a flag that is not a boolean", () => pkg.compileRegex("a", { harmonize: "no" as unknown as boolean })],
    ["a type there is not", () => pkg.compileRegex("a", { type: "sfst" as Full.TransducerType })],
    ["an empty source name", () => pkg.compileRegex("a", { sourceName: "" })],
    ["a warning switch there is not", () => pkg.compileLexc(LEXC, { warnings: ["loud" as Full.LexcWarningSwitch] })],
    ["a file that is neither bytes nor text", () => pkg.compilePmatch(PMATCH, { files: { a: 3 as unknown as string } })],
  ];
  for (const [what, f] of cases) assertEqual(thrownCode(pkg.HfstError, f), "invalid-argument", what);
});

await runTests("hfst/compile");
