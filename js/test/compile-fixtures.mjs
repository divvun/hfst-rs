// Builds what the hfst tools write of the inputs the repository's tests give
// them, for the suites that check a package compiles, transforms, inspects
// and writes transducers as the tools do. The inputs and cases are those of
// crates/hfst-cli/tests/embed_parity.rs, which checks hfst-embed against the
// same tools.
//
// Everything goes under compile/ in the fixture directory, and the manifest
// compile-parity.json lists each case: what a package calls, the tool's
// arguments, whether the tool succeeded, and the file holding what it wrote.
// Each call is named as divvun-hfst names it, its options in snake_case. The
// tools run in the fixture directory on paths relative to it, so the names
// they give their results, which some results carry, are the names the
// suites pass, wherever the directory is.

import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** The flag `--format` takes for each type, as divvun-hfst names it. */
const FORMATS = {
  "openfst-tropical": "openfst-tropical",
  foma: "foma",
  "optimized-lookup-weighted": "olw",
  "optimized-lookup-unweighted": "olu",
};

/** The lines the lookup tools read. */
const WORDS = ["cat", "mouse", "ca", "mice", "dog", "talo", "xyz", "cats", ""];

/** Each operation that takes one transducer: the tool's arguments, and the call. */
const UNARY = [
  [["determinize"], "determinize", {}],
  [["determinize", "-E"], "determinize", { encode_weights: true }],
  [["minimize"], "minimize", {}],
  [["minimize", "-E"], "minimize", { encode_weights: true }],
  [["invert"], "invert", {}],
  [["reverse"], "reverse", {}],
  [["remove-epsilons"], "remove_epsilons", {}],
  [["prune-alphabet"], "prune_alphabet", {}],
  [["prune-alphabet", "-f"], "prune_alphabet", { force: true }],
  [["project", "-p", "input"], "project", { project: "input" }],
  [["project"], "project", {}],
  [["push-weights", "-p", "initial"], "push_weights", { push: "initial" }],
  [["push-weights"], "push_weights", {}],
  [["push-labels", "-p", "start"], "push_labels", { push: "initial" }],
  [["push-labels"], "push_labels", {}],
  [["realign"], "realign", {}],
  [["repeat", "-f", "1", "-t", "3"], "repeat", { from_: 1, to: 3 }],
  [["repeat", "-f", "2"], "repeat", { from_: 2 }],
  [["repeat", "-f", "inf"], "repeat", { from_: { $float: "inf" } }],
  [["insert-freely", "-a", "x:y"], "insert_freely", { symbol_pair: "x:y" }],
  [["eliminate-flags"], "eliminate_flags", {}],
  [["preprocess-for-optimized-lookup-format"], "preprocess_for_optimized_lookup_format", {}],
  [["affix-guessify"], "affix_guessify", {}],
  [["affix-guessify", "-D", "prefix", "-w", "2"], "affix_guessify", { direction: "prefix", weight: 2 }],
  [["kill-paths", "-S", "c"], "kill_paths", { symbol: "c" }],
  [["reweight", "-a", "1", "-b", "2"], "reweight", { addition: 1, multiplier: 2 }],
  [["reweight", "-S", "a", "-F", "sqrt"], "reweight", { symbol: "a", function: "sqrt" }],
  [
    ["edit-metadata", "-a", "foo=bar", "-a", "baz=quux", "-t", "2"],
    "edit_metadata",
    { add: { foo: "bar", baz: "quux" }, truncate_length: 2 },
  ],
  [["name", "-n", "renamed"], "set_name", { $args: ["renamed"] }],
  [["expand-equivalences", "-f", "a", "-t", "A"], "expand_equivalences", { from_: "a", to: "A" }],
  [["substitute", "-f", "a", "-t", "A"], "substitute", { from_label: "a", to_label: "A" }],
  [["substitute", "-f", "a", "-t", "A", "-9"], "substitute", { from_label: "a", to_label: "A", compose: true }],
];

/** Each operation that takes two transducers. */
const BINARY = [
  [["compose"], "compose", {}],
  [["compose", "-F"], "compose", { harmonize_flags: true }],
  [["compose", "-H"], "compose", { do_not_harmonize: true }],
  [["compose", "--xerox-composition=yes"], "compose", { xerox_composition: true }],
  [["concatenate"], "concatenate", {}],
  [["concatenate", "-F"], "concatenate", { harmonize_flags: true }],
  [["conjunct"], "conjunct", {}],
  [["conjunct", "-F"], "conjunct", { harmonize_flags: true }],
  [["disjunct"], "disjunct", {}],
  [["priority-disjunct"], "priority_disjunct", {}],
  [["subtract"], "subtract", {}],
  [["subtract", "-F"], "subtract", { harmonize_flags: true }],
];

/** Writes the compile fixtures and their manifest into `out`, using the hfst binary at `hfst`. */
export function buildCompileFixtures(hfst, out) {
  const root = "compile";
  for (const sub of ["in", "src", "out", "src/dhfst"]) mkdirSync(join(out, root, sub), { recursive: true });
  const run = (args, input = "", cwd = out) => {
    const done = spawnSync(hfst, args, { cwd, input, maxBuffer: 1 << 28 });
    return { ok: done.status === 0, stdout: done.stdout, stderr: done.stderr.toString() };
  };
  const must = (args, input = "") => {
    const done = run(args, input);
    if (!done.ok) throw new Error(`hfst ${args.join(" ")} failed: ${done.stderr}`);
    return done.stdout;
  };
  const at = (name) => join(out, name);
  const src = (name) => `${root}/src/${name}`;
  const input = (name) => `${root}/in/${name}.hfst`;
  const copy = (from, name) => copyFileSync(join(repo, from), at(src(name)));
  const write = (name, text) => writeFileSync(at(src(name)), text);

  for (const name of ["words", "flags", "weights", "identity", "many", "nonascii", "unify"]) {
    copy(`js/test/fixtures/${name}.regexp`, `${name}.regexp`);
  }
  for (const name of ["animals", "tokenise"]) copy(`js/test/fixtures/${name}.pmatch`, `${name}.pmatch`);
  copy("crates/hfst/tests/fixtures/test_transducers.att", "test_transducers.att");
  copy("crates/hfst/tests/fixtures/test_lexc.lexc", "test_lexc.lexc");
  for (const name of ["acceptor.hfst", "acceptor-wide.hfst", "errmodel.hfst", "errmodel.dhfst"]) {
    copy(`crates/hfst-cli/tests/fixtures/dhfst/${name}`, `dhfst/${name}`);
  }
  write("more.lexc", "LEXICON Root\nmouse # ;\n");
  write("strings.txt", "cat\ndog\t1.5\nc:d a t\t2\n");
  write("relabel.tsv", "c\tC\n# a comment\nd\tD\n");
  write("words.txt", WORDS.join("\n") + "\n");
  write(
    "index.xml",
    `<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info><locale>se</locale><title>T</title><description>D</description><producer>p</producer></info>
  <acceptor type="general" id="acceptor.default.hfst"><title>A</title><description>a</description></acceptor>
  <errmodel id="errmodel.default.hfst"><title>E</title><description>e</description></errmodel>
</hfstspeller>
`,
  );
  write("meta.json", '{"id": "x"}');
  write("config.json", '{"beam": 3}');

  // The transducers the operations, queries and writers read.
  const inputs = {};
  for (const name of ["words", "flags", "weights", "identity", "many", "nonascii"]) {
    must(["regexp2fst", src(`${name}.regexp`), "-o", input(name)]);
    for (const [suffix, format] of [["foma", "foma"], ["olw", "olw"], ["olu", "olu"]]) {
      must(["fst2fst", "-f", format, input(name), "-o", input(`${name}.${suffix}`)]);
      inputs[`${name}.${suffix}`] = input(`${name}.${suffix}`);
    }
    inputs[name] = input(name);
  }
  for (const [name, regex] of [
    ["abcd", "a b c d"],
    ["cyc", "[a:b c]* d"],
    ["animals", "{cat}|{dog}|{cats}"],
    ["flagsx", '{cat} "@P.X.Y@" | {dog} "@R.X.Y@"'],
    ["rule", "[c|d|f] a ? s"],
  ]) {
    must(["regexp2fst", "-o", input(name)], `${regex}\n`);
    inputs[name] = input(name);
  }
  must(["txt2fst", src("test_transducers.att"), "-o", input("att")]);
  must(["lexc", src("test_lexc.lexc"), "-o", input("lexc")]);
  inputs.att = input("att");
  inputs.lexc = input("lexc");
  // A stream's transducers one by one, for the writers, which write one.
  const pieces = {};
  for (const name of ["att"]) {
    const count = (must(["summarize", input(name)]).toString().match(/^name: /gm) ?? []).length;
    pieces[name] = [];
    for (let n = 1; n <= count; n += 1) {
      const piece = input(`${name}.${n}`);
      writeFileSync(at(piece), must(["tail", "-n", "1"], must(["head", "-n", String(n), input(name)])));
      pieces[name].push(piece);
      inputs[`${name}.${n}`] = piece;
    }
  }
  must(["pmatch2fst", src("animals.pmatch"), "-o", input("animals-pmatch")]);
  inputs["animals-pmatch"] = input("animals-pmatch");
  const grammar = 'Alphabet a b ;\nRules\n"R1"\na:b <=> _ # ;\n';
  must(["twolc", "-o", input("rules")], grammar);
  inputs.rules = input("rules");
  must(["guessify", input("words"), "-o", input("guesser")]);
  inputs.guesser = input("guesser");

  const cases = [];
  let counter = 0;
  const next = (suffix = "") => `${root}/out/${String((counter += 1)).padStart(4, "0")}${suffix}`;
  /** A case whose expectation is what the tool prints. */
  const printed = (entry, args, stdin = "") => {
    const done = run(args, stdin);
    const expected = next();
    writeFileSync(at(expected), done.stdout);
    cases.push({ ...entry, tool: args, ok: done.ok, expected });
  };
  /** A case whose expectation is the file the tool writes to `-o`. */
  const written = (entry, args, suffix = "") => {
    const expected = next(suffix);
    const done = run([...args, "-o", expected]);
    cases.push({ ...entry, tool: [...args, "-o", expected], ok: done.ok, expected });
  };

  // fst2fst: every type, with and without a header.
  for (const name of ["words", "words.foma", "words.olw", "words.olu"]) {
    for (const [type, flag] of Object.entries(FORMATS)) {
      if (name === "words.olu" && flag !== "olu") continue;
      for (const header of [true, false]) {
        const args = ["fst2fst", "-f", flag, ...(header ? [] : ["-b"]), input(name)];
        printed({ kind: "to_bytes", input: name, type, use_backend_format: !header }, args);
      }
    }
  }

  for (const name of ["words", "weights", "flags", "words.foma", "att", "lexc"]) {
    for (const [args, method, kwargs] of UNARY) {
      if (name === "att" && method === "insert_freely") continue;
      printed({ kind: "unary", input: name, method, kwargs }, [...args, input(name)]);
    }
  }
  const pairs = [
    ["words", "many"],
    ["animals", "words"],
    ["flags", "flagsx"],
    ["flags", "flags"],
    ["words", "words.foma"],
    ["words.foma", "many"],
    ["abcd", "animals"],
  ];
  for (const [first, second] of pairs) {
    for (const [args, method, kwargs] of BINARY) {
      printed({ kind: "binary", first, second, method, kwargs }, [...args, input(first), input(second)]);
    }
  }
  for (const [args, method, kwargs] of BINARY) {
    printed({ kind: "binary", first: "words", second: null, method, kwargs }, [...args, input("words"), input("words")]);
  }
  printed({ kind: "binary", first: "abcd", second: "animals", method: "shuffle", kwargs: {} }, [
    "shuffle",
    input("abcd"),
    input("animals"),
  ]);
  for (const [args, kwargs] of [
    [[], {}],
    [["-f"], { fast: true }],
    [["-I"], { invert: true }],
    [["-a", "-e"], { harmonize: true, encode_weights: true }],
  ]) {
    printed(
      {
        kind: "compose_intersect",
        input: "lexc",
        rules: ["rule"],
        kwargs: { ...kwargs, lexicon_name: input("lexc"), rules_name: input("rule") },
      },
      ["compose-intersect", ...args, input("lexc"), input("rule")],
    );
  }
  printed(
    {
      kind: "unary",
      input: "words",
      method: "substitute",
      kwargs: { from_label: "c", to_transducer: { $transducer: "abcd" }, file_name: input("abcd") },
    },
    ["substitute", "-f", "c", "-T", input("abcd"), input("words")],
  );
  for (const inOrder of [false, true]) {
    printed(
      {
        kind: "unary",
        input: "words",
        method: "substitute",
        kwargs: { from_file: { $text: src("relabel.tsv") }, file_name: src("relabel.tsv"), in_order: inOrder },
      },
      ["substitute", "-F", src("relabel.tsv"), ...(inOrder ? ["-R"] : []), input("words")],
    );
  }
  for (const [args, kwargs] of [
    [[], {}],
    [["-G", "-p", "2"], { do_not_compile_generator: true, default_penalty: 2 }],
  ]) {
    printed({ kind: "guessify", input: "words", kwargs }, ["guessify", ...args, input("words")]);
  }

  // The queries.
  for (const name of ["words", "weights", "flags", "words.foma", "words.olw"]) {
    for (const [args, kwargs] of [
      [[], {}],
      [["-n", "3"], { max_strings: 3 }],
      [["-N", "2"], { nbest: 2 }],
      [["-b", "0.5"], { beam: 0.5 }],
      [["-X", "obey-flags"], { obey_flags: true }],
      [["-p", "c"], { in_prefix: "c" }],
      [["-l", "3"], { max_in_length: 3 }],
    ]) {
      printed({ kind: "extract_paths", input: name, kwargs }, ["fst2strings", "-w", ...args, input(name)]);
    }
  }
  printed({ kind: "extract_paths", input: "cyc", kwargs: {} }, ["fst2strings", "-w", input("cyc")]);
  const words = WORDS.join("\n") + "\n";
  for (const name of ["words", "weights", "flags", "words.foma", "words.olw", "words.olu", "flags.olw"]) {
    for (const [args, kwargs] of [
      [[], {}],
      [["-b", "0.6"], { beam: 0.6 }],
      [["-X", "show-flags"], { show_flags: true }],
    ]) {
      printed({ kind: "lookup_report", inputs: [name], lines: WORDS, kwargs }, ["lookup", "-q", ...args, input(name)], words);
    }
  }
  for (const name of ["words", "weights", "words.foma"]) {
    for (const [args, kwargs] of [
      [[], {}],
      [["-R"], { invert: true }],
    ]) {
      printed({ kind: "flookup_report", inputs: [name], lines: WORDS, kwargs }, ["flookup", "-q", ...args, input(name)], words);
    }
  }
  for (const name of ["words.olw", "weights.olw", "words.olu", "flags.olw"]) {
    for (const [args, kwargs] of [
      [[], {}],
      [["-u"], { unique: true }],
      [["-n", "1"], { analyses: 1 }],
      [["-b", "0.5"], { beam: 0.5 }],
    ]) {
      printed(
        { kind: "optimized_lookup_report", inputs: [name], lines: WORDS, kwargs },
        ["optimized-lookup", "-q", ...args, input(name)],
        words,
      );
    }
  }
  for (const [first, second] of [
    ["words", "words"],
    ["words", "many"],
    ["flags", "flags"],
    ["words", "words.foma"],
  ]) {
    printed({ kind: "compare", first, second }, ["compare", input(first), input(second)]);
  }
  for (const name of ["words", "lexc", "words.olw"]) {
    printed({ kind: "dump_alphabets", input: name }, ["dump-alphabets", input(name)]);
  }
  printed({ kind: "check_alpha", first: "words", second: "many" }, ["check-alpha", input("words"), input("many")]);
  for (const name of ["words", "words.olw"]) {
    printed({ kind: "summarize", input: name }, ["summarize", "-v", input(name)]);
  }
  const grepped = "the cat sat\nno match here\n\ncats and dogs\n";
  for (const [args, pattern, kwargs] of [
    [["cat"], "cat", {}],
    [["{cat}"], "{cat}", {}],
    [["--invert-match", "{cat}"], "{cat}", { invert_match: true }],
    [["-F", "cat"], "cat", { fixed_strings: true }],
    [["-F", "-w", "cat"], "cat", { fixed_strings: true, word_regexp: true }],
  ]) {
    printed({ kind: "grep", pattern, text: grepped, kwargs }, ["grep", ...args], grepped);
  }
  for (const [tests, negative] of [
    ["a a:b\n", false],
    ["a:b a\n", false],
    ["a:b a\n", true],
  ]) {
    printed(
      { kind: "pair_test", rules: "rules", tests, kwargs: { negative_test: negative } },
      ["pair-test", input("rules"), ...(negative ? ["-N"] : [])],
      tests,
    );
  }
  const matched = "the cat and the dog\nsaw a horse\n\nand a cat\n";
  for (const [args, kwargs] of [
    [[], {}],
    [["--newline"], { newline: true }],
    [["--locate"], { locate: true }],
  ]) {
    printed({ kind: "pmatch", archive: "animals-pmatch", text: matched, kwargs }, ["pmatch", ...args, input("animals-pmatch")], matched);
  }
  const guessed = "cat\nmouses\nxyz\n";
  printed({ kind: "guess", guesser: "guesser", words: guessed.split("\n").slice(0, -1) }, ["guess", input("guesser")], guessed);
  const tokenised = "the cat and the mouse.\n";
  printed({ kind: "naive_tokeniser", input: "words", text: tokenised }, ["tokenize", input("words")], tokenised);

  // The writers.
  for (const name of ["words", "weights", "flags", "words.foma", "words.olw", "lexc", ...pieces.att.map((p) => basename(p, ".hfst"))]) {
    for (const [args, method, kwargs] of [
      [[], "to_att", {}],
      [["-n"], "to_att", { use_numbers: true }],
      [["-w"], "to_att", { print_weights: true }],
      [["-D"], "to_att", { do_not_print_weights: true }],
      [["-f", "dot"], "to_dot", {}],
      [["-f", "prolog"], "to_prolog", {}],
      [["-f", "pckimmo"], "to_pckimmo", {}],
    ]) {
      printed({ kind: "text", input: name, method, kwargs }, ["fst2txt", ...args, input(name)]);
    }
  }
  for (const [name, args, method, kwargs] of [
    ["errmodel.hfst", ["--dhfst-type", "errmodel"], "to_dhfst_error_model", {}],
    ["errmodel.hfst", ["--dhfst-type", "errmodel", "--max-fallback-depth", "1"], "to_dhfst_error_model", { max_fallback_depth: 1 }],
    ["errmodel.hfst", ["--dhfst-type", "errmodel", "--unbounded-fallback"], "to_dhfst_error_model", { max_fallback_depth: null }],
    ["acceptor.hfst", ["--dhfst-type", "acceptor"], "to_dhfst_acceptor", {}],
    ["acceptor-wide.hfst", ["--dhfst-type", "acceptor", "-Q"], "to_dhfst_acceptor", { quick: true }],
  ]) {
    written(
      { kind: "dhfst", source: src(`dhfst/${name}`), method, kwargs: { ...kwargs, source_name: name } },
      ["fst2fst", "-f", "dhfst", ...args, "-i", src(`dhfst/${name}`)],
    );
  }
  for (const name of ["words", "weights", "words.foma", "words.olw"]) {
    written({ kind: "thfst", input: name }, ["fst2fst", "-f", "thfst", input(name)], ".thfst");
  }
  for (const [args, metadata, indexXml] of [
    [["-X", src("index.xml")], src("index.xml"), true],
    [["-m", src("meta.json")], src("meta.json"), false],
  ]) {
    written(
      {
        kind: "bhfst",
        acceptor: src("dhfst/acceptor.hfst"),
        error_model: src("dhfst/errmodel.dhfst"),
        metadata,
        kwargs: { index_xml: indexXml, speller_config: { $text: src("config.json") } },
      },
      ["bhfst", "-a", src("dhfst/acceptor.hfst"), "-e", src("dhfst/errmodel.dhfst"), ...args, "-c", src("config.json")],
    );
  }

  // The compilers.
  for (const name of ["words", "flags", "weights", "many", "unify", "nonascii"]) {
    for (const [args, options] of [
      [[], {}],
      [["-j"], { disjunct: true }],
      [["-S"], { semicolon: true }],
      [["-H", "-F", "-E", "-M"], { do_not_harmonize: true, harmonize_flags: true, encode_weights: true, do_not_minimize: true }],
      [["--xerox-composition=yes", "-X", "flag-is-epsilon"], { xerox_composition: true, flag_is_epsilon: true }],
    ]) {
      for (const [format, flag] of Object.entries(FORMATS)) {
        printed(
          { kind: "regex", sources: [src(`${name}.regexp`)], options: { ...options, format } },
          ["regexp2fst", "-f", flag, ...args, src(`${name}.regexp`)],
        );
      }
    }
  }
  for (const files of [[src("test_lexc.lexc")], [src("test_lexc.lexc"), src("more.lexc")]]) {
    for (const [args, options] of [
      [[], {}],
      [["-A", "-F"], { align_strings: true, with_flags: true }],
      [["-F", "-M", "-R"], { with_flags: true, minimize_flags: true, rename_flags: true }],
    ]) {
      for (const [format, flag] of Object.entries(FORMATS)) {
        printed(
          { kind: "lexc", sources: files, options: { ...options, format, quiet: true } },
          ["lexc", "-q", "-f", flag, ...args, ...files],
        );
      }
    }
  }
  for (const grammarText of [
    'Alphabet a b ;\nRules\n"R1"\na:b <=> _ # ;\n',
    'Alphabet a b c a:b ;\nSets\nCns = a b ;\nRules\n"R1"\nCns:0 <=> _ c ;\n"R2"\na:b => c _ ;\n',
  ]) {
    for (const [args, options] of [
      [["-f", "openfst"], { format: "openfst-tropical" }],
      [["-f", "foma"], { format: "foma" }],
      [["-f", "openfst", "-R", "-D"], { format: "openfst-tropical", resolve: true, dont_resolve_right: true }],
    ]) {
      printed({ kind: "twolc", text: grammarText, options: { ...options, quiet: true } }, ["twolc", "-q", ...args], grammarText);
    }
  }
  for (const name of ["animals", "tokenise"]) {
    for (const [args, options] of [
      [[], {}],
      [["--flatten"], { flatten: true }],
    ]) {
      written({ kind: "pmatch2fst", sources: [src(`${name}.pmatch`)], options }, ["pmatch2fst", ...args, src(`${name}.pmatch`)]);
    }
  }
  for (const [format, flag] of Object.entries(FORMATS)) {
    for (const [args, options] of [
      [[], {}],
      [["-j", "--norm"], { disjunct_strings: true, norm: true }],
      [["-p", "-S"], { pairstrings: true, has_spaces: true }],
    ]) {
      printed(
        { kind: "strings", sources: [src("strings.txt")], options: { ...options, format } },
        ["strings2fst", "-f", flag, ...args, src("strings.txt")],
      );
    }
  }
  writeFileSync(at(src("words.prolog")), must(["fst2txt", "-f", "prolog", input("words")]));
  for (const [format, flag] of Object.entries(FORMATS)) {
    for (const [kind, name, args, options] of [
      ["att", "test_transducers.att", [], {}],
      ["att", "test_transducers.att", ["-j"], { disjunct: true }],
      ["prolog", "words.prolog", ["-p"], {}],
    ]) {
      printed(
        { kind, sources: [src(name)], options: { ...options, format } },
        ["txt2fst", "-f", flag, ...args, src(name)],
      );
    }
  }
  const script =
    "regex a b c ;\nprint words\ndefine X [d|e] ;\nregex X ;\nprint net\n" +
    "push defined X\nprint stack\nsave stack out.hfst\nsave defined defs.hfst\n" +
    "set quit-on-fail OFF\nregex a:b ;\nset print-weight ON\nprint words\n" +
    "pop stack\npop stack\npop stack\npop stack\npop stack\nregex c ;\nprint size\n";
  for (const [args, options] of [
    [["-f", "openfst-tropical"], { format: "openfst-tropical" }],
    [["-f", "foma"], { format: "foma" }],
    [["-f", "openfst-tropical", "-w"], { format: "openfst-tropical", print_weight: true }],
  ]) {
    const expected = next(".xfst");
    mkdirSync(at(expected), { recursive: true });
    writeFileSync(join(at(expected), "s.xfst"), script);
    const done = run(["xfst", ...args, "-F", "s.xfst"], "", at(expected));
    writeFileSync(join(at(expected), "stdout"), done.stdout);
    const wrote = readdirSync(at(expected)).filter((f) => f !== "s.xfst" && f !== "stdout").sort();
    cases.push({ kind: "xfst", script, options, tool: ["xfst", ...args, "-F", "s.xfst"], ok: done.ok, expected, written: wrote });
  }

  writeFileSync(at("compile-parity.json"), JSON.stringify({ inputs, pieces, cases }, null, 1));
}
