# divvun-hfst

Divvun HFST for Python. It reads optimized-lookup transducers (`.hfstol`),
pmatch tokenisers (`.pmhfst`) and DHFST files. It looks words up, tokenises
text and checks words against DHFST acceptors, and gives the results the
`hfst` command-line tools give. It is the Divvun HFST library, written in Rust,
as a native extension module: one wheel per platform for CPython 3.11 and
later. Its surface is that of the npm package
[@divvun/hfst](https://www.npmjs.com/package/@divvun/hfst), in Python's
idioms, and the two run the same code.

The package reads and looks up. It does not compile, write or change
transducers.

```sh
pip install divvun-hfst
```

It is imported as `divvun_hfst`: `hfst` is the HFST project's own Python
package.

## Lookup

```python
import divvun_hfst

analyser = divvun_hfst.load_transducer("analyser-gt-desc.hfstol")

analyser.lookup("teach")
# [LookupResult(output='teach+Noun+Masc+Com+Sg',
#               symbols=('t', 'e', 'a', 'c', 'h', '+Noun', '+Masc', '+Com', '+Sg'), weight=0.0),
#  LookupResult(output='teach+Noun+Masc+Com+Sg+DefArt', symbols=(...), weight=0.0),
#  ...]
```

`load_transducer(path)` reads a file; `read_transducer(data)` takes its bytes.
Both take a weighted or unweighted optimized-lookup transducer, with or
without an HFST header. `load_transducers` and `read_transducers` take a
stream of several and return them in stream order, a pmatch container as TOP
and then its definitions. Every reader has both forms, and a `load_` form
reads the file in the library, without the GIL.

`lookup` returns every result the hfst library finds, lightest first, then by
output. `output` is the output symbols joined; `symbols` lists them, without
flag diacritics and epsilons. It takes two keyword-only limits: `limit` stops
the search after that many paths, and `time_limit`, in seconds, returns the
results found so far once it runs out.

```python
analyser.lookup("teach", limit=1, time_limit=0.5)
```

A transducer's `type` says which optimized-lookup format it is in, `name` is
the name its header gives it, and `properties` is a dict of the rest of its
header.

## The optimised lookup

`load_optimised_transducer(path)` and `read_optimised_transducer(data)` read
the same files for divvunspell's lookup engine, its `divvun-fst` crate. Its
`lookup` returns what divvunspell returns for the same transducer and word, in
the same shape, and is usually several times faster.

```python
fast = divvun_hfst.load_optimised_transducer("analyser-gt-desc.hfstol")
fast.lookup("teach")           # the same four analyses
fast.lookup("teach", limit=1)  # the lightest one
```

For most words the two lookups agree. They differ on Unicode normalisation,
text outside the alphabet, cycles of epsilon and flag arcs, duplicate paths
and what `limit` means;
[MIGRATING.md](https://github.com/divvun/hfst-rs/blob/main/js/hfst/MIGRATING.md#two-lookups)
gives an example of each. Use `Transducer` where results must match hfst's,
and `OptimisedTransducer` where they must match divvunspell's, or where speed
matters most. The optimised lookup has no time limit.

## Tokenising

```python
tokeniser = divvun_hfst.load_tokeniser("tokeniser-disamb-gt-desc.pmhfst")

text = "Inuit nunaat."
for token in tokeniser.tokenise(text):
    print(text[token.start:token.end], token.analysed, token.readings)
# Inuit True (Reading(analysis='Inuit Sem/Hum Prop Abs Pl', weight=0.0), ...)
# nunaat True (Reading(analysis='nuna N Abs Pl 3PlPoss', weight=0.0), ...)
# . True (Reading(analysis='. CLB', weight=0.0),)

print(tokeniser.format(text, "giella-cg"))
# "<Inuit>"
# 	"Inuit" Sem/Hum Prop Abs Pl <W:0.0>
# 	...
```

`start` and `end` count code points, so `text[start:end]` is the token's
surface. `format` returns, byte for byte, what `hfst tokenise` prints, in
`segment`, `space-separated`, `xerox`, `cg`, `finnpos`, `giella-cg`, `conllu`
or `visl` format; `divvun_hfst.TokeniseFormat` is their type.

Options are one `TokeniseOptions` for `tokenise` and one `FormatOptions` for
`format`, made once and passed to as many calls as need them:

```python
options = divvun_hfst.TokeniseOptions(max_weight_classes=1, print_all=True)
tokeniser.tokenise(text, options)

weights = divvun_hfst.FormatOptions(print_weights=True)
tokeniser.format(text, "cg", weights)
```

Each field is named after the `hfst tokenise` setting it stands for, in
snake_case, with time limits in seconds. `TokeniseOptions` takes `dedupe`,
`max_weight_classes`, `beam`, `time_limit`, `tokenize_multichar`, `print_all`,
and `weight_cutoff`, which `hfst tokenise` lacks. `FormatOptions` takes all of
them but `weight_cutoff`, and `print_weights`, `hack_uncompose`, `newline`,
`keep_newline` and `superblanks`; a field left as `None` has the value the
format's flag gives it. Both are frozen dataclasses, checked when they are
made: a value of the wrong type raises `TypeError`, and one `hfst tokenise`
would turn down `ValueError`. `dataclasses.replace` makes a variant.

## DHFST

```python
model = divvun_hfst.load_dhfst("acceptor.default.dhfst")
if isinstance(model, divvun_hfst.DhfstAcceptor):
    model.lookup("sámi")  # the word's least weight, or None when the lexicon rejects it
```

`load_dhfst` and `read_dhfst` read either type of DHFST file and make the
checks divvunspell makes when it loads one, returning a `DhfstAcceptor` or a
`DhfstErrorModel` as its `type` says. An acceptor, a speller's lexicon,
answers `lookup(word)` as divvunspell's speller checks a word as typed, and
its `info` says how it stores its arcs. An error model gives its `version`,
`state_count`, `symbols` and `meta`. For spelling suggestions, use
divvunspell.

## Threads

Loading, both lookups, tokenising and acceptor lookup release the GIL while
the library works. Every loaded object is immutable, so threads can share
one and run in parallel:

```python
from concurrent.futures import ThreadPoolExecutor

with ThreadPoolExecutor() as pool:
    analyses = list(pool.map(analyser.lookup, words))
```

## Errors

A failure the library reports raises a `divvun_hfst.HfstError`, whose `code`
is the code `@divvun/hfst` gives the same failure (`divvun_hfst.HfstErrorCode`
lists them). Bytes a reader does not load raise its subclass
`divvun_hfst.FormatError`:

- `unsupported-format`: a format the package does not read, such as a tropical
  OpenFst or foma transducer. The message names it.
- `invalid-format`: bytes that are not well formed in the format they claim,
  or that end partway through it.
- `type-mismatch`: a stream mixing transducer types.
- `transducer-count`: more transducers than the reader takes.

An argument of the wrong type or value raises the `TypeError`, `ValueError` or
`OverflowError` Python raises for it, and a file that cannot be read raises
what `open()` would. A failed call leaves the object usable.

## Logging

The library's log events go to the standard `logging` module once the call
returns, on loggers named after the Rust module they came from: under `hfst`
for the hfst library, such as `hfst.pmatch_core`, and under `divvun_fst` for
the optimised lookup's engine. Python's own levels apply, with the library's
trace level at 5. Events below the level of `hfst` or `divvun_fst` at the
start of a call are not kept, so set those loggers' levels to see more:

```python
import logging

logging.getLogger("hfst").setLevel(logging.DEBUG)
```

## Types

The package ships type stubs and the `py.typed` marker, and `mypy --strict`
checks them.

## Developing

The package is built from `crates/hfst-python` in the
[hfst-rs](https://github.com/divvun/hfst-rs) repository, with
[maturin](https://www.maturin.rs) and [uv](https://docs.astral.sh/uv/):

```sh
cd python
uv sync --no-install-project
uv run --no-sync maturin develop --uv   # build the extension into .venv
uv run --no-sync pytest                 # the tests
uv run --no-sync mypy                   # the stubs and the tests
uv run --no-sync maturin build --release
```

The tests read the transducers the npm package's tests read, and the native
library's results for them. They build them with
`node js/test/fixtures.mjs <dir>`, which needs Node 20 or later and cargo, or
take them from the directory `DIVVUN_HFST_FIXTURES` names, such as the
`target/js-test/fixtures` that `node js/build.mjs --test` writes. They check
the wheel `DIVVUN_HFST_WHEEL` names, or build one.

The package's version is the one `crates/hfst-python/Cargo.toml` carries. A
`python/v<version>` tag naming it releases the wheels and the sdist to PyPI.

## Licence

LGPL-3.0-or-later. The package carries the licence in `COPYING.LESSER` and
the GPL it builds on in `COPYING`. Divvun HFST is copyright UiT The Arctic
University of Norway.
