# @divvun/hfst

Divvun HFST for JavaScript. It reads optimized-lookup transducers
(`.hfstol`), pmatch tokenisers (`.pmhfst`) and DHFST files. It looks words
up, tokenises text and checks words against DHFST acceptors. It is the Divvun
HFST library, written in Rust and built as WebAssembly, so it gives the
results the `hfst` command-line tools give. It runs in browsers, Web Workers,
Deno, Node 20 and later, and Bun, as one ES module and one `.wasm` file.

It replaces [hfst-js](https://www.npmjs.com/package/hfst-js). Its optimised
lookup loads and looks words up faster than hfst-js on every transducer
measured ([figures below](#against-hfst-js)).
[MIGRATING.md](https://github.com/divvun/hfst-rs/blob/main/js/hfst/MIGRATING.md)
maps hfst-js calls to this package and lists every way the results differ.
The package reads and looks up. It does not compile, write or change
transducers.

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
with code `unsupported-format`.

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

## Starting the module

`init()` loads the `.wasm` file that ships beside the package. Call it once
and wait for it before anything else; calling it again returns the same
promise. It also takes a URL, a `Response`, the module's bytes or a compiled
`WebAssembly.Module`, for bundlers and for workers that are handed one.
`version()` gives the package's version. The package is versioned on its own,
apart from the hfst library inside it.

## Memory

Transducers, tokenisers and DHFST models hold WebAssembly memory. Call
`free()` when done with one, or declare it with `using`:

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
`not-automaton`, `cyclic`, `invalid-argument`, `internal` and `failed`. A
panic inside the module traps it. The instance is poisoned from then on and
every call throws `poisoned`, because its memory can no longer be trusted.
Load the page or process again to recover.

## Logging

The library's log events go to the console, warnings and errors only by
default. `setLogger((event) => ...)` takes them instead, each with its
`level`, `target` and `message`. `setLogLevel("debug")` changes how much is
sent, and `setLogLevel("off")` sends nothing.

## Developing

The package is built from `crates/hfst-wasm` in the
[hfst-rs](https://github.com/divvun/hfst-rs) repository:

```sh
node js/build.mjs           # js/hfst/dist, ready for npm publish from js/hfst
node js/build.mjs --test    # and the test build and fixtures
node js/test/run.mjs --require-all
```

The build needs the `wasm32-unknown-unknown` target, the `wasm-bindgen` CLI
at the version `Cargo.lock` pins, and `npx`; `wasm-opt` is used when it is
installed. The suite runs under Node, Bun, Deno and a headless Chromium, on a
page and in a Web Worker, and compares every result with the native hfst
library's. `node js/bench/bench.mjs` measures the package against hfst-js and
writes BENCHMARKS.md.

The package's version is the one `crates/hfst-wasm/Cargo.toml` and
`js/hfst/package.json` both carry. A `wasm/v<version>` tag naming it releases
the package to npm.

## Licence

LGPL-3.0-or-later. The package carries the licence in `dist/COPYING.LESSER`
and the GPL it builds on in `dist/COPYING`. Divvun HFST is copyright UiT The
Arctic University of Norway.
