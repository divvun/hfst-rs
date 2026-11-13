# @divvun/hfst

Divvun HFST for JavaScript. It reads optimized-lookup transducers
(`.hfstol`), pmatch tokenisers (`.pmhfst`) and DHFST files. It looks words
up, tokenises text and checks words against DHFST acceptors. It is the Divvun
HFST library, written in Rust and built as WebAssembly, so it gives the
results the `hfst` command-line tools give. It runs in browsers, Web Workers,
Deno, Node 20 and later, and Bun.

The package has two entry points. `@divvun/hfst` reads and looks up.
[`@divvun/hfst/compile`](#compiling) compiles, changes, inspects and writes
transducers, from a WebAssembly module of its own, which only code that
imports it loads.

It replaces [hfst-js](https://www.npmjs.com/package/hfst-js). Its optimised
lookup loads and looks words up faster than hfst-js on every transducer
measured ([figures below](#against-hfst-js)).
[MIGRATING.md](https://github.com/divvun/hfst-rs/blob/main/js/hfst/MIGRATING.md)
maps hfst-js calls to this package and lists every way the results differ.

```sh
npm install @divvun/hfst
```

## Lookup

```js
import { init, readTransducer } from "@divvun/hfst";

await init();

const response = await fetch("/fst/analyser-gt-desc.hfstol");
const analyser = readTransducer(new Uint8Array(await response.arrayBuffer()));

analyser.lookup("teach");
// [
//   { output: "teach+Noun+Masc+Com+Sg",
//     symbols: ["t", "e", "a", "c", "h", "+Noun", "+Masc", "+Com", "+Sg"], weight: 0 },
//   { output: "teach+Noun+Masc+Com+Sg+DefArt", symbols: [...], weight: 0 },
//   { output: "teagh+Noun+Masc+Com+Sg", symbols: [...], weight: 0 },
//   { output: "teagh+Noun+Masc+Com+Sg+DefArt", symbols: [...], weight: 0 },
// ]

analyser.free();
```

`readTransducer` takes a weighted or unweighted optimized-lookup transducer,
with or without an HFST header. `readTransducers` takes a stream of several
and returns them in stream order. Bytes in any other format throw `HfstError`
with code `unsupported-format`; [`@divvun/hfst/compile`](#compiling) reads
every type.

`lookup` returns every result the hfst library finds, ordered by weight, then
by output. `output` is the output symbols joined; `symbols` lists them,
without flag diacritics and epsilons. It takes `{ limit, timeLimitMs }`:
`limit` stops the search after that many paths, and a time limit that runs out
returns the results found so far.

## The optimised lookup

`readOptimisedTransducer(bytes)` loads the same files for divvunspell's lookup
engine, its `divvun-fst` crate. Its `lookup` returns what divvunspell returns
for the same transducer and word, in the same shape. It loads in a millisecond
or two, and a lookup takes as little as a fifth of the time `lookup` takes.

```js
import { readOptimisedTransducer } from "@divvun/hfst";

const fast = readOptimisedTransducer(bytes);
fast.lookup("teach"); // the same four analyses
fast.lookup("teach", { limit: 1 }); // the lightest one
```

For most words the two lookups agree. They differ on Unicode normalisation,
text outside the alphabet, cycles of epsilon and flag arcs, duplicate paths
and what `limit` means; [MIGRATING.md](https://github.com/divvun/hfst-rs/blob/main/js/hfst/MIGRATING.md#two-lookups)
gives an example of each. Use `readTransducer` where results must match
hfst's, and `readOptimisedTransducer` where they must match divvunspell's, or
where speed matters most.

## Against hfst-js

Load time and time per word under Node 26 on an Apple M5 Pro, each the median
of three processes. hfst-js runs with its result cache off, so every lookup
searches. [BENCHMARKS.md](https://github.com/divvun/hfst-rs/blob/main/js/hfst/BENCHMARKS.md)
has all 13 transducers, under Node and Bun, and says how they were measured.

| Transducer | Load (ms): hfst-js | `readTransducer` | `readOptimisedTransducer` | Lookup (µs/word): hfst-js | `lookup` | optimised |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-gt-desc | 7.78 | 17.9 | 1.11 | 353 | 752 | 151 |
| lang-kal generator-gt-desc | 9.34 | 20.2 | 1.29 | 6200 | 11948 | 3978 |
| lang-gle analyser-gt-desc | 3.96 | 16.6 | 0.45 | 50 | 41 | 31 |
| lang-gle generator-gt-desc | 6.46 | 15.4 | 0.43 | 26 | 12 | 11 |
| se-zcheck generator-gramcheck-gt-norm | 13.3 | 42.7 | 1.06 | 85 | 174 | 49 |

The optimised lookup is faster than hfst-js on every transducer, at both
loading and lookup.

## Tokenising

```js
import { readTokeniser } from "@divvun/hfst";

const tokeniser = readTokeniser(pmhfstBytes); // lang-kal's tokeniser-disamb-gt-desc.pmhfst

tokeniser.tokenise("Inuit nunaat.");
// [
//   { surface: "Inuit", start: 0, end: 5, analysed: true,
//     readings: [{ analysis: "Inuit Sem/Hum Prop Abs Pl", weight: 0 }, ...] },
//   { surface: "nunaat", start: 6, end: 12, analysed: true,
//     readings: [{ analysis: "nuna N Abs Pl 3PlPoss", weight: 0 }, ...] },
//   { surface: ".", start: 12, end: 13, analysed: true,
//     readings: [{ analysis: ". CLB", weight: 0 }] },
// ]

tokeniser.format("Inuit nunaat.", "giella-cg");
// "<Inuit>"
// 	"Inuit" Sem/Hum Prop Abs Pl <W:0.0>
// 	...
```

`start` and `end` are UTF-16 offsets, so `text.slice(start, end)` is the
token's surface. `tokenise` takes the settings of `hfst tokenise` that change
which tokens and readings come back: `dedupe`, `maxWeightClasses`, `beam`,
`timeLimit` (in milliseconds), `tokenizeMultichar` and `printAll`, and
`weightCutoff`, which `hfst tokenise` lacks. `format` returns, byte for byte,
what `hfst tokenise` prints, in `segment`, `space-separated`, `xerox`, `cg`,
`finnpos`, `giella-cg`, `conllu` or `visl` format. It also takes
`printWeights`, `hackUncompose`, `newline`, `keepNewline` and `superblanks`.

## DHFST

```js
import { readDhfst } from "@divvun/hfst";

const model = readDhfst(bytes);
if (model.type === "acceptor") {
  model.lookup("sámi"); // the word's least weight, or null when the lexicon rejects it
}
```

`readDhfst` loads either type of DHFST file and makes the checks divvunspell
makes when it loads one. An acceptor, a speller's lexicon, answers
`lookup(word)` as divvunspell's speller checks a word as typed. An error model
gives its `version`, `stateCount`, `symbols` and `meta`. For spelling
suggestions, use [@divvun/fst](https://www.npmjs.com/package/@divvun/fst),
which is divvunspell itself.

## Compiling

`@divvun/hfst/compile` compiles regular expressions, lexc, twolc, xfst
scripts, pmatch scripts, strings, and AT&T and Prolog text. It reads
transducers of every type the hfst library reads, and changes, inspects and
writes them as the hfst tools do, giving the bytes and reports the tools
give. Its WebAssembly module is about 3.6 MiB, against about 460 KiB for
the main entry point's, and only code that imports this entry point loads
it.

```js
import { compileLexc, init } from "@divvun/hfst/compile";

await init();

const lexc = `
Multichar_Symbols +N +Pl
LEXICON Root
Nouns ;
LEXICON Nouns
cat Noun ;
dog Noun ;
LEXICON Noun
+N:0 # ;
+N+Pl:s # ;
`;

const { transducer: nouns } = compileLexc([lexc], { names: ["nouns.lexc"] });
nouns.name; // "lexc(nouns.lexc)"
nouns.lookup("cat+N+Pl"); // [{ output: "cats", symbols: ["c", "a", "t", "s"], weight: 0 }]
```

This entry point has its own `init`, which loads its own `.wasm` file. Call
it before anything else here: the main entry point's `init` does not start
it.

| Function | Tool | Returns |
| --- | --- | --- |
| `regex(source)` | `hfst regexp2fst` | `{ transducer, warnings }` |
| `compileLexc(sources)` | `hfst lexc` | `{ transducer, warnings }` |
| `compileTwolc(source)` | `hfst twolc` | `{ transducers, warnings }` |
| `compileXfst(source)` | `hfst xfst -F` | `{ output, stack, definitions, written, warnings }` |
| `compilePmatch(source)` | `hfst pmatch2fst` | `{ archive, warnings }`; `readTokeniser(archive)` reads the archive |
| `compileStrings(source)` | `hfst strings2fst` | `{ transducer, warnings }` |
| `parseAtt(source)` | `hfst txt2fst` | `{ transducer, warnings }` |
| `parseProlog(source)` | `hfst txt2fst --prolog` | `{ transducer, warnings }` |

Each compiler gives what the tool writes of the same source. Its second
argument holds the tool's options in camelCase, such as
`{ format: "foma", warnMissingLexicons: true }`. `format` is the type to
compile to, `openfst-tropical` unless set; twolc and xfst work in
`openfst-tropical` or `foma`. `name` is the file name the tool would read the
source from: diagnostics give it, and results are named after it as the tool
names them. `compileLexc` takes `names`, one for each source, and
`compileXfst` takes `input`, the script's standard input. An option a call
does not know throws `invalid-argument`, naming it.

### Files

A compile reads the files its source names from `files`, and from nowhere
else. `files` is a record or a `Map` from names to text or a `Uint8Array`, or
a function that takes a name and returns the file's text or bytes, or
`undefined` when it has no such file. The function is called while the
compile runs, once for each file. A file `files` does not give fails the
compile, naming the file. The files an xfst script writes come back in
`written`.

```js
regex('@txt"words.txt"', { files: { "words.txt": "cat\ndog\n" } });
regex('@txt"words.txt"', { files: new Map([["words.txt", bytes]]) });
regex('@txt"words.txt"', { files: (name) => cache.get(name) });

const ran = compileXfst("load stack nouns.hfst\ninvert net\nsave stack inverted.hfst\n", {
  files: { "nouns.hfst": nouns.toBytes() },
});
readTransducer(ran.written["inverted.hfst"]).lookup("cats"); // [{ output: "cat+N+Pl", ... }]
```

A pmatch script finds a relative `@bin"FILE"` or `@txt"FILE"` beside
`name`, as the tool finds one beside its input: with
`name: "lang/animals.pmatch"`, `@txt"animals.txt"` reads
`lang/animals.txt`.

### Changing a transducer

A `Transducer` of this entry point is of any type the hfst library reads:
`openfst-tropical`, `foma`, `optimized-lookup-weighted` or
`optimized-lookup-unweighted`. Its `readTransducer` and `readTransducers`
read all four. Each operation changes the transducer in place and returns
it, as `Array.prototype.sort` does, so operations chain. `copy()` gives a
transducer of its own to change.

```js
const { transducer: animals } = regex("{cat}:{dog} | {mouse}:{mice}");
animals.invert().minimize();
animals.lookup("dog"); // [{ output: "cat", symbols: ["c", "a", "t"], weight: 0 }]

const plurals = animals.copy().concatenate(regex("s").transducer);
plurals.lookup("dogs"); // [{ output: "cats", symbols: ["c", "a", "t", "s"], weight: 0 }]
animals.lookup("dogs"); // []
```

Each operation is named after the hfst tool that does it, in camelCase, and
takes the tool's options as an object, such as
`t.determinize({ encodeWeights: true })`:

- On the transducer alone: `determinize`, `minimize`, `invert`, `reverse`,
  `removeEpsilons`, `pruneAlphabet`, `project`, `pushWeights`,
  `pushLabels`, `realign`, `repeat`, `insertFreely`, `eliminateFlags`,
  `preprocessForOptimizedLookupFormat`, `affixGuessify`,
  `expandEquivalences`, `killPaths`, `reweight`, `setName`,
  `editMetadata`, `guessify` and `substitute`.
- With another transducer: `compose`, `concatenate`, `conjunct`,
  `subtract`, `disjunct`, `priorityDisjunct` and `shuffle`, and
  `composeIntersect` with an array of rule transducers.
- `convert(type)` converts it to another type, as `hfst fst2fst` does.

`guessify` returns the model form generator `hfst guessify` writes after
the guesser, or `null`, rather than the transducer. `t.compose(t)` composes
`t` with itself as it was. The operations work on `openfst-tropical` and
`foma`. As the tools do, they refuse optimized lookup, throwing
`type-mismatch`; convert it first.

### Queries

A transducer's `summarize`, `checkAlpha`, `compare`, `dumpAlphabets`,
`extractPaths` (`hfst fst2strings`), `guess` and `naiveTokeniser` report
what the hfst tool each is named after reports, as plain objects. The entry
point also exports `lookupReport`, `flookupReport`, `optimizedLookupReport`,
`pairTest` and `grep`, and its `Tokeniser` adds `pmatch`.

```js
animals.extractPaths().map((p) => [p.input, p.output]); // [["dog", "cat"], ["mice", "mouse"]]
```

### Errors and warnings

A source that does not compile throws `HfstError` with code
`compile-failed`. Its `diagnostics` hold every diagnostic the compile made,
as plain objects:

```js
try {
  regex("[cat", { name: "nouns.xre" });
} catch (error) {
  error.code; // "compile-failed"
  const [diagnostic] = error.diagnostics;
  diagnostic.message; // "expected RightBracket"
  diagnostic.span; // { start: 4, end: 4 }
  console.log(diagnostic.rendered);
}
```

A diagnostic has its `severity` (`error`, `warning` or `info`), `message`,
`sourceName`, `span`, `labels`, `notes`, `help`, and `rendered`, the report
the hfst tools print for it, without colour. Spans count UTF-16 code units,
as JavaScript indexes a string, so `source.slice(span.start, span.end)` is
the text a span points at.

A compile that succeeds returns its warnings beside what it made:

```js
const { warnings } = compileStrings("cat\t-1\n");
warnings[0].message; // "Found negative weight -1.000000; negative weights are supported but iffy, ..."
```

An xfst command that fails without stopping the script is among its
warnings, with severity `error`. What a compile says at the info level, as
with `verbose`, goes to the logger.

### Writing

```js
nouns.toBytes(); // in its own type, as hfst fst2fst writes it
nouns.toBytes("optimized-lookup-weighted"); // converted, for lookup
nouns.toAtt(); // AT&T text, as hfst fst2txt writes it
```

`toBytes` writes a stream holding the transducer, in any type, and
`useBackendFormat` leaves out the HFST header. `toAtt`, `toProlog`, `toDot`
and `toPckimmo` return the text `hfst fst2txt` writes. `toDhfstAcceptor`
and `toDhfstErrorModel` write DHFST files, which the main entry point's
`readDhfst` reads. `packBhfst` packs a speller as `hfst bhfst` does, from
two DHFST files, or two transducer streams to convert, and the text of its
`meta.json`:

```js
const lexicon = regex("{cat} | {dog}").transducer;
const errorModel = regex("[a:b | b:a | a | b]*").transducer;
const speller = packBhfst(lexicon.toDhfstAcceptor(), errorModel.toDhfstErrorModel(), '{"id": "en"}');
```

This entry point does not write THFST, which is a directory of files rather
than bytes; `packBhfst` converts a transducer stream it packs to THFST inside
the archive.

### Between the entry points

A transducer crosses between the entry points as the bytes `toBytes`
writes. The main entry point reads optimized lookup, so write that:

```js
import * as hfst from "@divvun/hfst";

await hfst.init();
const generator = hfst.readTransducer(nouns.toBytes("optimized-lookup-weighted"));
generator.lookup("cat+N+Pl"); // [{ output: "cats", symbols: ["c", "a", "t", "s"], weight: 0 }]
```

This entry point's readers read what the main one reads. A transducer of
one entry point is not an operand of the other's: passing one throws
`invalid-argument`. Both throw the same `HfstError`.

## Starting the module

`init()` loads the `.wasm` file that ships beside the package. Call it once
and wait for it before anything else; calling it again returns the same
promise. It also takes a URL, a `Response`, the module's bytes or a compiled
`WebAssembly.Module`, for bundlers and for workers that are handed one.
`@divvun/hfst/compile`'s `init` does the same for its own `.wasm` file. Each
entry point starts on its own, and only the ones you call `init` on are
loaded. `version()` gives the package's version, from either entry point.
The package is versioned on its own, apart from the hfst library inside it.

## Memory

Transducers, tokenisers and DHFST models, of either entry point, hold
WebAssembly memory. Call `free()` when done with one, or declare it with
`using`:

```js
{
  using analyser = readTransducer(bytes);
  analyser.lookup("teach");
} // freed here
```

An object dropped without `free()` is released after it is garbage collected.
A call on a freed object throws `HfstError` with code `freed`.

## Errors

Every error is an `HfstError` with a `code` that code can test:
`not-initialised`, `init-failed`, `poisoned`, `freed`, `transducer-count`,
`type-mismatch`, `unsupported-format`, `invalid-format`, `io`,
`not-automaton`, `cyclic`, `invalid-argument`, `internal`, `failed` and,
from `@divvun/hfst/compile`, `compile-failed`. A panic inside a module traps
it. The instance is poisoned from then on and every call into it throws
`poisoned`, because its memory can no longer be trusted. The other entry
point's module carries on. Load the page or process again to recover.

## Logging

The library's log events go to the console, warnings and errors only by
default. `setLogger((event) => ...)` takes them instead, each with its
`level`, `target` and `message`. `setLogLevel("debug")` changes how much is
sent, and `setLogLevel("off")` sends nothing. Each entry point logs on its
own: the `setLogger` and `setLogLevel` it exports reach only its module.

## Developing

The package is built from `crates/hfst-wasm`, and its compile entry point
from `crates/hfst-wasm-compile`, in the
[hfst-rs](https://github.com/divvun/hfst-rs) repository:

```sh
node js/build.mjs           # js/hfst/dist, ready for npm publish from js/hfst
node js/build.mjs --test    # and the test build and fixtures
node js/test/run.mjs --require-all
```

The build needs the `wasm32-unknown-unknown` target, the `wasm-bindgen` CLI
at the version `Cargo.lock` pins, and `npx`; `wasm-opt` is used when it is
installed. The suite runs under Node, Bun, Deno and a headless Chromium, on a
page and in a Web Worker. It compares every result of the main entry point
with the native hfst library's, and every result of the compile entry point
with what the hfst tools write or print of the same input.
`node js/bench/bench.mjs` measures the package against hfst-js and writes
BENCHMARKS.md.

The package's version is the one `crates/hfst-wasm/Cargo.toml` and
`js/hfst/package.json` both carry. A `wasm/v<version>` tag naming it releases
the package to npm.

## Licence

LGPL-3.0-or-later. The package carries the licence in `dist/COPYING.LESSER`
and the GPL it builds on in `dist/COPYING`. Divvun HFST is copyright UiT The
Arctic University of Norway.
