# @divvun/hfst against hfst-js

@divvun/hfst replaces [hfst-js](https://www.npmjs.com/package/hfst-js) 1.4.0. These figures compare the two on the same transducers, word lists and machine. `node js/bench/bench.mjs` measured them and wrote this file.

Machine: Apple M5 Pro, 18 cores, 64 GiB, macOS 26.5.1. Measured under Node v26.11.1 on 2026-10-29, with @divvun/hfst built from commit 9ea78e4 (381.2 KiB of WebAssembly); Bun 1.4.2 on 2026-10-29, with @divvun/hfst built from commit 9ea78e4 (381.2 KiB of WebAssembly). hfst-js is 1.4.0.

## Method

- Each transducer is measured in 3 fresh processes per runtime, and each figure is the median of the 3. The machine was shared with other work: its one-minute load average was 3.2 to 97.3 under Node and 9.2 to 93.2 under Bun. Load slows both libraries; the ratios, from the two taking turns in one process, move less than the times.
- **Load** is the time from a transducer's bytes in memory to a transducer ready to look up: `readTransducer(bytes)` in @divvun/hfst and `new HfstInputStream(bytes, false).read()` in hfst-js. In each process it is the median of 7 loads, the two libraries taking turns. @divvun/hfst is measured after `init` has compiled its WebAssembly module, which happens once per process and took a median of 13.6 ms under Node and 2.72 ms under Bun.
- **Lookup** is the time per word over the first 500 words of the transducer's word list (fewer when the list is shorter), in each process the median of 3 passes after one untimed pass. hfst-js runs with its result cache off, so every lookup searches. @divvun/hfst keeps no result cache. **hfst-js cached** is the time per word when every word is already in hfst-js's cache, for context: it is the cost of a repeated word.
- Neither library is tuned for the benchmark.

## Node v26.11.1

| Transducer | Size | Words timed | Load, hfst-js (ms) | Load, ours (ms) | Ours ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, ours (µs/word) | Ours ÷ hfst-js | Lookup, hfst-js cached (µs/word) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-disamb-gt-desc | 14.5 MiB | 500 | 7.80 | 18.5 | 2.37× | 344 | 721 | 2.09× | 2.23 |
| lang-kal analyser-gt-desc | 14.2 MiB | 500 | 7.36 | 18.5 | 2.51× | 346 | 706 | 2.04× | 2.16 |
| lang-kal analyser-gt-norm | 13.9 MiB | 500 | 7.36 | 17.7 | 2.40× | 346 | 700 | 2.02× | 2.10 |
| lang-kal analyser-learner-gt-norm | 12.8 MiB | 500 | 4.35 | 17.6 | 4.04× | 192 | 330 | 1.73× | 0.18 |
| lang-kal generator-gt-desc | 14.9 MiB | 500 | 6.23 | 20.0 | 3.21× | 6311 | 11610 | 1.84× | 0.76 |
| lang-kal generator-gt-norm | 14.6 MiB | 500 | 5.60 | 19.8 | 3.54× | 5923 | 11246 | 1.90× | 0.73 |
| lang-kal generator-learner-gt-norm | 13.4 MiB | 500 | 5.04 | 20.2 | 4.00× | 5219 | 9643 | 1.85× | 1.21 |
| lang-gle analyser-disamb-gt-desc | 13.9 MiB | 500 | 4.22 | 17.2 | 4.06× | 55 | 46 | 0.84× | 0.64 |
| lang-gle analyser-gt-desc | 13.9 MiB | 500 | 4.42 | 17.5 | 3.95× | 54 | 46 | 0.85× | 0.64 |
| lang-gle analyser-gt-norm | 12.8 MiB | 500 | 3.76 | 15.8 | 4.20× | 52 | 41 | 0.79× | 0.51 |
| lang-gle generator-gt-desc | 12.9 MiB | 500 | 4.20 | 16.5 | 3.92× | 27 | 14 | 0.54× | 0.49 |
| lang-gle generator-gt-norm | 12.9 MiB | 500 | 4.39 | 16.2 | 3.69× | 26 | 14 | 0.54× | 0.46 |
| se-zcheck generator-gramcheck-gt-norm | 36.7 MiB | 500 | 14.8 | 45.1 | 3.04× | 90 | 225 | 2.49× | 7.78 |

@divvun/hfst loads every transducer more slowly than hfst-js, taking 2.4 to 4.2 times as long. It looks words up more slowly than uncached hfst-js on 8 of 13 transducers, taking up to 2.5 times as long. It is faster on lang-gle analyser-disamb-gt-desc, lang-gle analyser-gt-desc, lang-gle analyser-gt-norm, lang-gle generator-gt-desc and lang-gle generator-gt-norm, in as little as 0.5 times the time.

## Bun 1.4.2

| Transducer | Size | Words timed | Load, hfst-js (ms) | Load, ours (ms) | Ours ÷ hfst-js | Lookup, hfst-js (µs/word) | Lookup, ours (µs/word) | Ours ÷ hfst-js | Lookup, hfst-js cached (µs/word) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lang-kal analyser-disamb-gt-desc | 14.5 MiB | 500 | 4.15 | 17.6 | 4.24× | 457 | 631 | 1.38× | 1.65 |
| lang-kal analyser-gt-desc | 14.2 MiB | 500 | 4.03 | 16.8 | 4.18× | 458 | 616 | 1.35× | 1.59 |
| lang-kal analyser-gt-norm | 13.9 MiB | 500 | 3.98 | 17.0 | 4.28× | 485 | 625 | 1.29× | 1.63 |
| lang-kal analyser-learner-gt-norm | 12.8 MiB | 500 | 6.25 | 22.8 | 3.64× | 318 | 318 | 1.00× | 0.20 |
| lang-kal generator-gt-desc | 14.9 MiB | 500 | 4.62 | 20.2 | 4.38× | 8374 | 11057 | 1.32× | 0.64 |
| lang-kal generator-gt-norm | 14.6 MiB | 500 | 6.07 | 26.5 | 4.37× | 9720 | 15114 | 1.55× | 0.74 |
| lang-kal generator-learner-gt-norm | 13.4 MiB | 500 | 4.38 | 18.4 | 4.21× | 6793 | 7511 | 1.11× | 0.65 |
| lang-gle analyser-disamb-gt-desc | 13.9 MiB | 500 | 7.85 | 29.4 | 3.74× | 113 | 70 | 0.62× | 0.66 |
| lang-gle analyser-gt-desc | 13.9 MiB | 500 | 9.31 | 35.0 | 3.76× | 133 | 84 | 0.63× | 0.72 |
| lang-gle analyser-gt-norm | 12.8 MiB | 500 | 7.40 | 27.6 | 3.72× | 90 | 65 | 0.72× | 0.60 |
| lang-gle generator-gt-desc | 12.9 MiB | 500 | 7.48 | 27.2 | 3.64× | 50 | 23 | 0.45× | 0.57 |
| lang-gle generator-gt-norm | 12.9 MiB | 500 | 6.57 | 23.7 | 3.61× | 38 | 19 | 0.50× | 0.57 |
| se-zcheck generator-gramcheck-gt-norm | 36.7 MiB | 500 | 15.0 | 57.1 | 3.80× | 181 | 234 | 1.29× | 11 |

@divvun/hfst loads every transducer more slowly than hfst-js, taking 3.6 to 4.4 times as long. It looks words up more slowly than uncached hfst-js on 8 of 13 transducers, taking up to 1.6 times as long. It is faster on lang-gle analyser-disamb-gt-desc, lang-gle analyser-gt-desc, lang-gle analyser-gt-norm, lang-gle generator-gt-desc and lang-gle generator-gt-norm, in as little as 0.5 times the time.

## Word lists

The transducers are the analysers and generators built in GiellaLT's lang-kal (Greenlandic) and lang-gle (Irish) repositories, and the North Sámi grammar checker's generator. `node js/bench/words.mjs` made the word lists, which the real-analyser parity check also reads:

- lang-kal analysers: the 11,199 word forms of `tools/analysers/test/corpus.txt` and the surface forms of the yaml tests (`words.mjs surface`).
- lang-kal generators: the analyses of the yaml tests, then the first 5,000 analyses that `analyser-gt-desc` (for `generator-gt-desc`) or `analyser-gt-norm` (for the other two) gives for those words (`words.mjs keys`, `words.mjs analyses`).
- lang-gle analysers: the input sides of 3,000 random paths through `analyser-gt-desc.hfst` and the surface forms of the yaml tests, 2,696 words (`words.mjs paths`, `words.mjs surface`). About a quarter of them spell accented letters decomposed, as the analyser's input side allows; hfst-js stops reading those words at the combining accent.
- lang-gle generators: as for lang-kal, from `analyser-gt-desc` and `analyser-gt-norm`.
- se-zcheck `generator-gramcheck-gt-norm`: the 3,478 analyses of lang-sme's yaml tests (`words.mjs keys`).
