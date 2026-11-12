// The compile entry point's API contract against its real module, as it
// ships: errors and warnings carrying diagnostics, the files a compile reads
// and writes, operations that change and return their transducer, what each
// compiler returns, and transducers crossing to the main entry point as
// bytes.

import type * as Compile from "../hfst/compile.ts";
import type * as Main from "../hfst/index.ts";
import {
  assert,
  assertEqual,
  readBytes,
  runTests,
  scenarioArgs,
  test,
  thrownCode,
} from "./harness.ts";

const args = scenarioArgs();
const pkg: typeof Compile = await import(args.module);
const main: typeof Main = await import(args.entries.main.module);
await pkg.init(await readBytes(args.wasm));
await main.init(await readBytes(args.entries.main.wasm));

function thrown(f: () => unknown): unknown {
  try {
    f();
  } catch (error) {
    return error;
  }
  throw new Error("nothing was thrown");
}

function compileError(f: () => unknown): InstanceType<typeof pkg.HfstError> {
  const error = thrown(f);
  assert(error instanceof pkg.HfstError, `an HfstError, not ${String(error)}`);
  return error;
}

function paths(t: Compile.Transducer): string[] {
  return t.extractPaths().map((p) => (p.input === p.output ? p.input : `${p.input}:${p.output}`));
}

// [spec:hfst:req:javascript.compile-errors/test]
test("a compile that fails throws compile-failed with its diagnostics as plain objects", () => {
  const error = compileError(() => pkg.regex("[a"));
  assertEqual(error.code, "compile-failed", "code");
  assert(error.diagnostics.length > 0, "it carries diagnostics");
  for (const d of error.diagnostics) {
    assertEqual(Object.getPrototypeOf(d), Object.prototype, "a plain object");
    assertEqual(
      Object.keys(d).sort().join(" "),
      "help labels message notes rendered severity sourceName span",
      "its fields",
    );
    assertEqual(JSON.stringify(JSON.parse(JSON.stringify(d))), JSON.stringify(d), "it is plain data");
  }
  const [first] = error.diagnostics;
  assert(first !== undefined, "a first diagnostic");
  assertEqual(first.severity, "error", "severity");
  assertEqual(first.sourceName, "<stdin>", "the source's name");
  assert(!first.rendered.includes("\u001b["), "the rendered text has no colour");
  assert(first.rendered.includes(first.message), "the rendered text says what the message says");
});

// [spec:hfst:req:javascript.compile-errors/test]
test("a diagnostic's spans count UTF-16 units, as JavaScript indexes the source", () => {
  const source = "{\u{1F600}å} [";
  const error = compileError(() => pkg.regex(source, { name: "smile.xre" }));
  const [first] = error.diagnostics;
  assertEqual(first?.sourceName, "smile.xre", "the name given");
  assertEqual(first?.span?.start, source.length, "the error is at the end of the source");
  assertEqual(first?.labels[0]?.span.start, source.length, "and so is its label");
});

// [spec:hfst:req:javascript.compile-errors/test]
test("a compile that succeeds returns its warnings in the same shape", () => {
  const source = "cat\ndog\t-1.5\n";
  const { transducer, warnings } = pkg.compileStrings(source, { disjunctStrings: true, name: "w.txt" });
  assertEqual(transducer.type, "openfst-tropical", "the transducer");
  const [warning] = warnings;
  assert(warning !== undefined, `a warning, of ${warnings.length}`);
  assertEqual(warning.severity, "warning", "severity");
  assertEqual(warning.sourceName, "w.txt", "the source's name");
  const span = warning.span;
  assert(span !== null, "a span");
  assertEqual(source.slice(span.start, span.end), "dog\t-1.5", "the span is the line it warns of");
});

// [spec:hfst:req:javascript.compile-errors/test]
test("an xfst command that fails is a warning of a script that runs on", () => {
  const ran = pkg.compileXfst("set quit-on-fail OFF\npop stack\nregex a ;\nprint words\n");
  assert(ran.output.endsWith("\na\n"), `the script ran on: ${JSON.stringify(ran.output)}`);
  const errors = ran.warnings.filter((w) => w.severity === "error");
  assert(errors.some((w) => w.message.includes("empty stack")), "the failed command is among the warnings");
  assertEqual(ran.stack.length, 1, "the stack");
});

// [spec:hfst:req:javascript.compile-errors/test]
test("a source of several transducers where one is wanted throws transducer-count", () => {
  const error = compileError(() => pkg.regex("a\nb\n"));
  assertEqual(error.code, "transducer-count", "code");
  assert(Array.isArray(error.diagnostics), "it carries the compile's diagnostics");
  assertEqual(paths(pkg.regex("a\nb\n", { disjunct: true }).transducer).sort().join(" "), "a b", "disjunct");
});

// [spec:hfst:req:javascript.compile-files/test]
test("the files a compile reads come from a record, a Map or a function", () => {
  const source = '@txt"words.txt"';
  const record = pkg.regex(source, { files: { "words.txt": "cat\ndog\n" } }).transducer;
  assertEqual(paths(record).sort().join(" "), "cat dog", "a record of text");
  const map = new Map([["words.txt", new TextEncoder().encode("mouse\n")]]);
  assertEqual(paths(pkg.regex(source, { files: map }).transducer).join(" "), "mouse", "a Map of bytes");
  const asked: string[] = [];
  const fromFunction = pkg.regex(source, {
    files: (name) => {
      asked.push(name);
      return name === "words.txt" ? "horse\n" : undefined;
    },
  }).transducer;
  assertEqual(paths(fromFunction).join(" "), "horse", "a function");
  assertEqual(asked.join(" "), "words.txt", "the function is asked for the file by name, once");
});

// [spec:hfst:req:javascript.compile-files/test]
test("a name the files do not supply fails the compile and names the file", () => {
  for (const files of [undefined, { "other.txt": "x\n" }, () => undefined]) {
    const error = compileError(() => pkg.regex('@txt"missing.txt"', { files }));
    assertEqual(error.code, "compile-failed", "code");
    const said = [error.message, ...error.diagnostics.map((d) => d.message)].join("\n");
    assert(said.includes("missing.txt"), `the error names the file: ${said}`);
  }
});

// [spec:hfst:req:javascript.compile-files/test]
test("what the files function throws is thrown as it was", () => {
  const mine = new TypeError("no files here");
  const error = thrown(() =>
    pkg.regex('@txt"words.txt"', {
      files: () => {
        throw mine;
      },
    }),
  );
  assert(error === mine, `the function's own error, not ${String(error)}`);
  const wrong = compileError(() => pkg.regex('@txt"words.txt"', { files: () => 3 as unknown as string }));
  assertEqual(wrong.code, "invalid-argument", "a value that is no file");
  assertEqual(pkg.regex("a").transducer.type, "openfst-tropical", "the module carries on");
});

// [spec:hfst:req:javascript.compile-files/test]
test("the files an xfst script writes come back in its result, and it reads them back", () => {
  const script = "regex c a t ;\nsave stack out.hfst\nclear stack\nload stack out.hfst\nprint words\n";
  const ran = pkg.compileXfst(script, { files: {} });
  assertEqual(Object.keys(ran.written).join(" "), "out.hfst", "the file written");
  const written = ran.written["out.hfst"];
  assert(written instanceof Uint8Array, "its bytes");
  assertEqual(paths(pkg.readTransducer(written)).join(" "), "cat", "what it holds");
  assert(ran.output.endsWith("\ncat\n"), `the script read it back: ${JSON.stringify(ran.output)}`);
  const viaFunction = pkg.compileXfst(script, { files: () => undefined });
  assertEqual(Object.keys(viaFunction.written).join(" "), "out.hfst", "with a files function too");
});

// [spec:hfst:req:javascript.compile-module/test]
test("an operation changes its transducer and returns it, and copy gives one of its own", () => {
  const t = pkg.regex("{cat}:{dog} | {mouse}").transducer;
  const copy = t.copy();
  assert(copy !== t, "a copy is another transducer");
  assert(t.invert() === t, "invert returns the transducer");
  assert(t.minimize().removeEpsilons() === t, "operations chain");
  assertEqual(t.lookup("dog")[0]?.output, "cat", "the transducer was changed");
  assertEqual(copy.lookup("cat")[0]?.output, "dog", "the copy was not");
  assert(copy.convert("foma") === copy, "convert returns the transducer");
  assertEqual(copy.type, "foma", "and changes its type");
  assertEqual(t.type, "openfst-tropical", "the original keeps its own");
  assert(t.compose(t) === t, "a transducer composes with itself");
  assert(t.setName("renamed") === t && t.name === "renamed", "setName names it");
  const guesser = pkg.regex("{cat}:{cat+N}").transducer;
  const generator = guesser.guessify();
  assert(generator !== null && generator !== guesser, "guessify returns the generator");
  assertEqual(guesser.type, "optimized-lookup-weighted", "and makes the transducer a guesser");
});

// [spec:hfst:req:javascript.compile-module/test]
test("every compiler returns what its divvun-hfst counterpart does", () => {
  const lexc = pkg.compileLexc(["LEXICON Root\ncat # ;\n"], { names: ["a.lexc"] });
  assertEqual(paths(lexc.transducer).join(" "), "cat", "compileLexc: one transducer");
  const twolc = pkg.compileTwolc('Alphabet a b ;\nRules\n"R1"\na:b <=> _ # ;\n');
  assert(Array.isArray(twolc.transducers) && twolc.transducers.length === 1, "compileTwolc: a list");
  const archive = pkg.compilePmatch("Define TOP {cat} EndTag(animal) ;").archive;
  assert(archive instanceof Uint8Array, "compilePmatch: bytes");
  const marked = pkg.readTokeniser(archive).pmatch("a cat")[0]?.marked;
  assertEqual(marked, "a <animal>cat</animal>", "which readTokeniser reads");
  const att = pkg.parseAtt("0\t1\ta\tb\t0.5\n1\t0\n", { name: "t.att" }).transducer;
  assertEqual(att.lookup("a")[0]?.output, "b", "parseAtt");
  const prolog = pkg.parseProlog(att.toProlog()).transducer;
  assertEqual(prolog.lookup("a")[0]?.output, "b", "parseProlog reads what toProlog writes");
  const ran = pkg.compileXfst("define X a ;\nregex X b ;\n");
  assertEqual(Object.keys(ran.definitions).join(" "), "X", "compileXfst: its definitions");
  assertEqual(paths(ran.definitions.X as Compile.Transducer).join(" "), "a", "by name");
});

// [spec:hfst:req:javascript.compile-module/test]
test("transducers cross between the entry points as bytes", () => {
  const t = pkg.regex("{cat}:{dog}").transducer;
  const read = main.readTransducer(t.toBytes("optimized-lookup-weighted"));
  assertEqual(read.lookup("cat")[0]?.output, "dog", "the main entry point reads what toBytes writes");
  const back = pkg.readTransducer(t.toBytes("optimized-lookup-weighted"));
  assertEqual(back.type, "optimized-lookup-weighted", "the compile entry point reads optimized lookup");
  assertEqual(back.lookup("cat")[0]?.output, "dog", "and looks up in it");
  assertEqual(thrownCode(main.HfstError, () => main.readTransducer(t.toBytes())), "unsupported-format", "tropical");
  const code = thrownCode(pkg.HfstError, () => t.compose(read as unknown as Compile.Transducer));
  assertEqual(code, "invalid-argument", "a transducer of the other entry point is not an operand");
  assert(main.HfstError === pkg.HfstError, "both entry points throw the one HfstError");
});

// [spec:hfst:req:javascript.compile-module/test]
test("THFST, a directory, is not written", () => {
  const t = pkg.regex("a").transducer;
  assertEqual(Reflect.get(t, "writeThfst"), undefined, "there is no THFST writer");
  const code = thrownCode(pkg.HfstError, () => t.toBytes("thfst" as Compile.TransducerType));
  assertEqual(code, "invalid-argument", "toBytes takes no THFST");
});

test("an option a call does not know is refused, and names it", () => {
  const t = pkg.regex("a").transducer;
  const error = compileError(() => t.determinize({ encodeWieghts: true } as Compile.DeterminizeOptions));
  assertEqual(error.code, "invalid-argument", "code");
  assert(error.message.includes("encodeWieghts"), error.message);
  const regex = compileError(() => pkg.regex("a", { formt: "foma" } as Compile.RegexOptions));
  assert(regex.message.includes("formt"), regex.message);
});

test("what a compile says at the info level reaches the logger", () => {
  const events: Compile.LogEvent[] = [];
  pkg.setLogger((event) => events.push(event));
  pkg.setLogLevel("info");
  pkg.compileTwolc('Alphabet a b ;\nRules\n"R1"\na:b <=> _ # ;\n', { verbose: true });
  pkg.setLogger(null);
  pkg.setLogLevel("warn");
  assert(events.some((e) => e.level === "info" && e.target === "hfst"), `info events: ${JSON.stringify(events)}`);
});

test("a freed transducer throws freed", () => {
  const t = pkg.regex("a").transducer;
  t.free();
  assertEqual(thrownCode(pkg.HfstError, () => t.invert()), "freed", "an operation");
  assertEqual(thrownCode(pkg.HfstError, () => t.name), "freed", "a property");
});

test("the queries over several transducers take arrays of them", () => {
  const t = pkg.regex("{cat}:{dog}").transducer;
  const [report] = pkg.lookupReport([t], ["cat"]);
  assertEqual(report?.results?.[0]?.output, "dog", "lookupReport");
  const [olu] = pkg.optimizedLookupReport([t.copy().convert("optimized-lookup-unweighted")], ["cat"]);
  assertEqual(olu?.analyses?.[0]?.[0], "dog", "optimizedLookupReport");
  assertEqual(pkg.grep("{cat}", "a cat\nno\n").lines.map((l) => l.number).join(" "), "1", "grep");
});

await runTests(args.label);
