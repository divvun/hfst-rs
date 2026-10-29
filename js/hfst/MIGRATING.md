# Moving from hfst-js

`@divvun/hfst` replaces [hfst-js](https://www.npmjs.com/package/hfst-js)
1.4.0 for looking words up in optimized-lookup transducers (`.hfstol`). For
the same transducer and word it returns what hfst-js returns, except where
hfst-js departs from the reference C++ `hfst-lookup`. There it returns what
`hfst-lookup` returns. It also reads accented letters in either Unicode
normalisation form, which neither of the others does. This file lists every
kind of difference a test has found. [BENCHMARKS.md](BENCHMARKS.md) compares the two for speed.

## Calls

| hfst-js | @divvun/hfst |
| --- | --- |
| `new HfstInputStream(bytes, cache).read()` | `await init()` once, then `readTransducer(bytes)` |
| `HfstInputStream.fromUrl(url)` | `readTransducer(new Uint8Array(await (await fetch(url)).arrayBuffer()))` |
| `new HfstInputStream(path)` | read the file yourself: the package loads bytes, never paths |
| `transducer.lookup(word)` returns `[analysis, weight][]` | `transducer.lookup(word)` returns `{ output, symbols, weight }[]` |
| `transducer.lookup(word, seconds)` | `transducer.lookup(word, { timeLimitMs })` |
| result cache, on unless turned off | no result cache |
| none | `transducer.free()`, or declare it with `using` |

hfst-js returns analyses in the order its search finds them and can return
one twice. `@divvun/hfst` returns each analysis once, ordered by weight, then
by output. `output` is what hfst-js calls the analysis.

## Differences

Each example gives the transducer, the input, and what each library returns
as `analysis weight` pairs.

### hfst-js stops at input it cannot read

When a character, or a run of characters, is not a symbol of the
transducer, hfst-js looks up only the input before it and returns those
analyses. `@divvun/hfst`, like `hfst-lookup`, returns nothing for a word it
cannot read whole. This also covers a tag a generator does not know, and it
means hfst-js never follows an identity (`?`) arc, since any character outside
the alphabet ends its input.

- lang-kal `analyser-gt-desc.hfstol`, input `tassaavoq☃`:
  hfst-js `tassa+U+Der/nv+Gram/IV+V+Ind+3Sg 0`; `@divvun/hfst` nothing.

### hfst-js cannot read a symbol that begins like a longer one

When the input at some point begins with a symbol, and a longer symbol begins
the same way but does not match, hfst-js gives up on that point instead of
reading the shorter symbol. It then looks up only the input before it, as
above.

- lang-gle `generator-gt-norm.hfstol`, input `<text>+XMLTag+Corr+Start`
  (the tag `+Corr` is not a symbol, while `+Com` and `+Cond` are):
  hfst-js `text 0`; `@divvun/hfst` `corr 0`.

### hfst-js does not read five-character flags as flags

A flag diacritic of five characters, such as `@C.X@` or `@R.Y@`, is an
ordinary symbol to hfst-js, so it blocks every path through it. hfst-js also
stops reading the other flags that leave the same state.

- `"@C.X@" {k} 0:%+Short`, input `k`:
  hfst-js nothing; `@divvun/hfst` `k+Short 0`.

### hfst-js lets unification follow a negative set of the same value

After `@N.F.a@`, which sets F to anything but a, hfst-js lets `@U.F.a@`
succeed. HFST does not.

- `"@N.F.a@" {ua} "@U.F.a@" 0:%+NU`, input `ua`:
  hfst-js `ua+NU 0`; `@divvun/hfst` nothing.

### Weights can differ in the last bits

HFST stores weights as 32-bit floats and adds them up in 32 bits. hfst-js
adds them up in 64 bits, so its sums carry digits HFST's do not, and rounding
its sum to 32 bits does not always give HFST's sum. The difference is around
the seventh significant digit. `hfst-lookup` prints six decimal places, where
the two agree.

- A transducer whose paths for `aaaaa` sum five of the weights 0.1, 0.37,
  1.3, 0.011 and 3.14159, input `aaaaa`, analysis `bbdedede`:
  hfst-js 4.100000098347664; `@divvun/hfst` 4.100000381469727.

### `@divvun/hfst` finds analyses for decomposed accents

A letter such as `ú` can be written as one character, or as `u` followed by
a combining accent. A transducer's alphabet holds one of the two spellings.
hfst-js and `hfst-lookup` read only that one, so a word typed in the other
spelling gets no analyses from either of them. `@divvun/hfst` reads both
spellings as the same symbol and returns the analyses the word has in the
transducer's own spelling.

- lang-gle `analyser-gt-norm.hfstol`, input `@=brúcht` with `ú` written as
  `u` and U+0301: hfst-js nothing, `hfst-lookup` nothing; `@divvun/hfst`
  `@=brúcht+Event 0`.
