# @divvun/hfst against hfst-js

@divvun/hfst replaces [hfst-js](https://www.npmjs.com/package/hfst-js) 1.4.0. These figures compare the two on the same transducers, word lists and machine. `node js/bench/bench.mjs` measured them and wrote this file.

Machine: Apple M5 Pro, 18 cores, 64 GiB, macOS 26.5.1. Measured under Node v26.11.1 on 2026-10-31, with @divvun/hfst built from commit 0cd8884 (456.1 KiB of WebAssembly); Bun 1.4.2 on 2026-10-31, with @divvun/hfst built from commit 0cd8884 (456.1 KiB of WebAssembly). hfst-js is 1.4.0.

## Method

- Each transducer is measured in 3 fresh processes per runtime, and each figure is the median of the 3. The machine was shared with other work: its one-minute load average was 6.3 to 11.0 under Node and 5.5 to 40.0 under Bun. Load slows both libraries; the ratios, from the two taking turns in one process, move less than the times.
- **Load** is the time from a transducer's bytes in memory to a transducer ready to look up: `readTransducer(bytes)` in @divvun/hfst (`readOptimisedTransducer(bytes)` for the optimised lookup) and `new HfstInputStream(bytes, false).read()` in hfst-js. In each process it is the median of 7 loads of each, the loaders taking turns. @divvun/hfst is measured after `init` has compiled its WebAssembly module, which happens once per process and took a median of 14.0 ms under Node and 2.46 ms under Bun.
- **Lookup** is the time per word over the first 500 words of the transducer's word list (fewer when the list is shorter), in each process the median of 3 passes after one untimed pass. hfst-js runs with its result cache off, so every lookup searches. @divvun/hfst keeps no result cache. **hfst-js cached** is the time per word when every word is already in hfst-js's cache, for context: it is the cost of a repeated word.
- Neither library is tuned for the benchmark.

## Node v26.11.1

| Transducer | Size | Words timed | Load, hfst-js (ms) | Load, ours (ms) | Ours ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, ours (µs/word) | Ours ÷ hfst-js | Lookup, hfst-js cached (µs/word) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-disamb-gt-desc | 14.5 MiB | 500 | 8.74 | 18.2 | 2.08× | 355 | 765 | 2.15× | 2.08 |
| lang-kal analyser-gt-desc | 14.2 MiB | 500 | 7.78 | 17.9 | 2.31× | 353 | 752 | 2.13× | 2.13 |
| lang-kal analyser-gt-norm | 13.9 MiB | 500 | 9.65 | 17.6 | 1.82× | 357 | 742 | 2.08× | 2.21 |
| lang-kal analyser-learner-gt-norm | 12.8 MiB | 500 | 4.25 | 17.3 | 4.06× | 201 | 337 | 1.67× | 0.19 |
| lang-kal generator-gt-desc | 14.9 MiB | 500 | 9.34 | 20.2 | 2.16× | 6200 | 11948 | 1.93× | 0.80 |
| lang-kal generator-gt-norm | 14.6 MiB | 500 | 9.64 | 19.9 | 2.06× | 5946 | 11552 | 1.94× | 0.78 |
| lang-kal generator-learner-gt-norm | 13.4 MiB | 500 | 5.29 | 18.3 | 3.46× | 5001 | 9464 | 1.89× | 0.81 |
| lang-gle analyser-disamb-gt-desc | 13.9 MiB | 500 | 4.29 | 16.9 | 3.93× | 54 | 41 | 0.77× | 0.69 |
| lang-gle analyser-gt-desc | 13.9 MiB | 500 | 3.96 | 16.6 | 4.18× | 50 | 41 | 0.83× | 0.60 |
| lang-gle analyser-gt-norm | 12.8 MiB | 500 | 3.53 | 15.2 | 4.32× | 48 | 37 | 0.76× | 0.54 |
| lang-gle generator-gt-desc | 12.9 MiB | 500 | 6.46 | 15.4 | 2.38× | 26 | 12 | 0.47× | 0.55 |
| lang-gle generator-gt-norm | 12.9 MiB | 500 | 6.25 | 15.6 | 2.50× | 25 | 11 | 0.44× | 0.61 |
| se-zcheck generator-gramcheck-gt-norm | 36.7 MiB | 500 | 13.3 | 42.7 | 3.22× | 85 | 174 | 2.05× | 6.88 |

@divvun/hfst loads every transducer more slowly than hfst-js, taking 1.8 to 4.3 times as long. It looks words up more slowly than uncached hfst-js on 8 of 13 transducers, taking up to 2.2 times as long. It is faster on lang-gle analyser-disamb-gt-desc, lang-gle analyser-gt-desc, lang-gle analyser-gt-norm, lang-gle generator-gt-desc and lang-gle generator-gt-norm, in as little as 0.4 times the time.

## Bun 1.4.2

| Transducer | Size | Words timed | Load, hfst-js (ms) | Load, ours (ms) | Ours ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, ours (µs/word) | Ours ÷ hfst-js | Lookup, hfst-js cached (µs/word) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-disamb-gt-desc | 14.5 MiB | 500 | 4.67 | 17.9 | 3.83× | 475 | 679 | 1.43× | 1.61 |
| lang-kal analyser-gt-desc | 14.2 MiB | 500 | 4.15 | 17.7 | 4.26× | 463 | 661 | 1.43× | 1.51 |
| lang-kal analyser-gt-norm | 13.9 MiB | 500 | 4.07 | 17.2 | 4.22× | 493 | 659 | 1.34× | 1.57 |
| lang-kal analyser-learner-gt-norm | 12.8 MiB | 500 | 4.14 | 17.4 | 4.20× | 288 | 299 | 1.04× | 0.23 |
| lang-kal generator-gt-desc | 14.9 MiB | 500 | 4.83 | 20.3 | 4.22× | 8303 | 10543 | 1.27× | 0.62 |
| lang-kal generator-gt-norm | 14.6 MiB | 500 | 4.66 | 19.8 | 4.25× | 8579 | 9754 | 1.14× | 0.59 |
| lang-kal generator-learner-gt-norm | 13.4 MiB | 500 | 6.18 | 25.3 | 4.09× | 9582 | 10837 | 1.13× | 0.88 |
| lang-gle analyser-disamb-gt-desc | 13.9 MiB | 500 | 3.98 | 17.1 | 4.30× | 65 | 38 | 0.58× | 0.40 |
| lang-gle analyser-gt-desc | 13.9 MiB | 500 | 3.90 | 18.0 | 4.61× | 64 | 38 | 0.60× | 0.41 |
| lang-gle analyser-gt-norm | 12.8 MiB | 500 | 3.36 | 15.6 | 4.64× | 51 | 33 | 0.64× | 0.37 |
| lang-gle generator-gt-desc | 12.9 MiB | 500 | 3.36 | 16.1 | 4.78× | 27 | 11 | 0.42× | 0.37 |
| lang-gle generator-gt-norm | 12.9 MiB | 500 | 3.34 | 15.7 | 4.70× | 22 | 11 | 0.49× | 0.29 |
| se-zcheck generator-gramcheck-gt-norm | 36.7 MiB | 500 | 9.19 | 43.8 | 4.76× | 125 | 148 | 1.19× | 6.45 |

@divvun/hfst loads every transducer more slowly than hfst-js, taking 3.8 to 4.8 times as long. It looks words up more slowly than uncached hfst-js on 8 of 13 transducers, taking up to 1.4 times as long. It is faster on lang-gle analyser-disamb-gt-desc, lang-gle analyser-gt-desc, lang-gle analyser-gt-norm, lang-gle generator-gt-desc and lang-gle generator-gt-norm, in as little as 0.4 times the time.

## The optimised lookup

`readOptimisedTransducer(bytes)` and its `lookup`, the engine of divvunspell's `divvun-fst`, measured in the same processes as the figures above and in the same way, taking turns with the other two. **Ours** is @divvun/hfst's `lookup`.

### Node v26.11.1

| Transducer | Load, hfst-js (ms) | Load, optimised (ms) | Optimised ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, optimised (µs/word) | Optimised ÷ hfst-js | Lookup, ours (µs/word) | Optimised ÷ ours |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-disamb-gt-desc | 8.74 | 1.16 | 0.13× | 355 | 151 | 0.42× | 765 | 0.20× |
| lang-kal analyser-gt-desc | 7.78 | 1.11 | 0.14× | 353 | 151 | 0.43× | 752 | 0.20× |
| lang-kal analyser-gt-norm | 9.65 | 1.18 | 0.12× | 357 | 148 | 0.42× | 742 | 0.20× |
| lang-kal analyser-learner-gt-norm | 4.25 | 1.22 | 0.29× | 201 | 96 | 0.48× | 337 | 0.29× |
| lang-kal generator-gt-desc | 9.34 | 1.29 | 0.14× | 6200 | 3978 | 0.64× | 11948 | 0.33× |
| lang-kal generator-gt-norm | 9.64 | 1.23 | 0.13× | 5946 | 3734 | 0.63× | 11552 | 0.32× |
| lang-kal generator-learner-gt-norm | 5.29 | 1.18 | 0.22× | 5001 | 3018 | 0.60× | 9464 | 0.32× |
| lang-gle analyser-disamb-gt-desc | 4.29 | 0.47 | 0.11× | 54 | 31 | 0.57× | 41 | 0.74× |
| lang-gle analyser-gt-desc | 3.96 | 0.45 | 0.11× | 50 | 31 | 0.61× | 41 | 0.74× |
| lang-gle analyser-gt-norm | 3.53 | 0.42 | 0.12× | 48 | 30 | 0.62× | 37 | 0.81× |
| lang-gle generator-gt-desc | 6.46 | 0.43 | 0.07× | 26 | 11 | 0.42× | 12 | 0.90× |
| lang-gle generator-gt-norm | 6.25 | 0.40 | 0.06× | 25 | 10 | 0.41× | 11 | 0.93× |
| se-zcheck generator-gramcheck-gt-norm | 13.3 | 1.06 | 0.08× | 85 | 49 | 0.57× | 174 | 0.28× |

The optimised lookup loads every transducer faster than hfst-js, in 0.1 to 0.3 times the time. It is faster on every transducer, in as little as 0.4 times the time.

### Bun 1.4.2

| Transducer | Load, hfst-js (ms) | Load, optimised (ms) | Optimised ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, optimised (µs/word) | Optimised ÷ hfst-js | Lookup, ours (µs/word) | Optimised ÷ ours |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-disamb-gt-desc | 4.67 | 1.22 | 0.26× | 475 | 155 | 0.33× | 679 | 0.23× |
| lang-kal analyser-gt-desc | 4.15 | 1.15 | 0.28× | 463 | 155 | 0.33× | 661 | 0.23× |
| lang-kal analyser-gt-norm | 4.07 | 1.12 | 0.28× | 493 | 153 | 0.31× | 659 | 0.23× |
| lang-kal analyser-learner-gt-norm | 4.14 | 1.21 | 0.29× | 288 | 102 | 0.35× | 299 | 0.34× |
| lang-kal generator-gt-desc | 4.83 | 1.21 | 0.25× | 8303 | 3969 | 0.48× | 10543 | 0.38× |
| lang-kal generator-gt-norm | 4.66 | 1.11 | 0.24× | 8579 | 4572 | 0.53× | 9754 | 0.47× |
| lang-kal generator-learner-gt-norm | 6.18 | 1.66 | 0.27× | 9582 | 4303 | 0.45× | 10837 | 0.40× |
| lang-gle analyser-disamb-gt-desc | 3.98 | 0.75 | 0.19× | 65 | 32 | 0.49× | 38 | 0.85× |
| lang-gle analyser-gt-desc | 3.90 | 0.85 | 0.22× | 64 | 32 | 0.50× | 38 | 0.84× |
| lang-gle analyser-gt-norm | 3.36 | 0.63 | 0.19× | 51 | 30 | 0.58× | 33 | 0.90× |
| lang-gle generator-gt-desc | 3.36 | 0.58 | 0.17× | 27 | 11 | 0.40× | 11 | 0.95× |
| lang-gle generator-gt-norm | 3.34 | 0.66 | 0.20× | 22 | 10 | 0.45× | 11 | 0.93× |
| se-zcheck generator-gramcheck-gt-norm | 9.19 | 1.27 | 0.14× | 125 | 48 | 0.39× | 148 | 0.33× |

The optimised lookup loads every transducer faster than hfst-js, in 0.1 to 0.3 times the time. It is faster on every transducer, in as little as 0.3 times the time.

## Word lists

The transducers are the analysers and generators built in GiellaLT's lang-kal (Greenlandic) and lang-gle (Irish) repositories, and the North Sámi grammar checker's generator. `node js/bench/words.mjs` made the word lists, which the real-analyser parity check also reads:

- lang-kal analysers: the 11,199 word forms of `tools/analysers/test/corpus.txt` and the surface forms of the yaml tests (`words.mjs surface`).
- lang-kal generators: the analyses of the yaml tests, then the first 5,000 analyses that `analyser-gt-desc` (for `generator-gt-desc`) or `analyser-gt-norm` (for the other two) gives for those words (`words.mjs keys`, `words.mjs analyses`).
- lang-gle analysers: the input sides of 3,000 random paths through `analyser-gt-desc.hfst` and the surface forms of the yaml tests, 2,696 words (`words.mjs paths`, `words.mjs surface`). About a quarter of them spell accented letters decomposed, as the analyser's input side allows; hfst-js stops reading those words at the combining accent.
- lang-gle generators: as for lang-kal, from `analyser-gt-desc` and `analyser-gt-norm`.
- se-zcheck `generator-gramcheck-gt-norm`: the 3,478 analyses of lang-sme's yaml tests (`words.mjs keys`).
