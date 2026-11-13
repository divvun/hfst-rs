# divvun-hfst

Divvun HFST for Python. It compiles, changes, inspects and writes
finite-state transducers as the `hfst` command-line tools do, and gives the
results the tools give. It looks words up, tokenises text with pmatch
tokenisers (`.pmhfst`) and checks words against DHFST spellers. It is the
Divvun HFST library, written in Rust, as a native extension module: one wheel
per platform for CPython 3.11 and later. Its surface is that of the npm
package [@divvun/hfst](https://www.npmjs.com/package/@divvun/hfst), in
Python's idioms, and the two run the same code.

```sh
pip install divvun-hfst
```

It is imported as `divvun_hfst`: `hfst` is the HFST project's own Python
package.

## Reading transducers

```python
import divvun_hfst

analyser = divvun_hfst.load_transducer("analyser-gt-desc.hfstol")
analyser.type  # 'optimized-lookup-weighted'
```

`load_transducer(path)` reads a file; `read_transducer(data)` takes its bytes.
Both read every type the hfst library reads, with or without an HFST header:

- `openfst-tropical`: OpenFst with tropical weights, which the compilers make
  unless told otherwise.
- `foma`: foma's unweighted transducers.
- `optimized-lookup-weighted` and `optimized-lookup-unweighted`: HFST's
  optimized-lookup formats, which analysers, generators and pmatch
  tokenisers use.

`divvun_hfst.TransducerType` names them. `load_transducers` and
`read_transducers` take a stream of several and return them in stream order,
a pmatch container as TOP and then its definitions. `load_transducers` also
reads a THFST directory, divvunspell's format, as one weighted
optimized-lookup transducer. Every reader has both forms, and a `load_` form
reads the file in the library, without the GIL.

A transducer's `type` is its type, `name` is the name its header gives it,
and `properties` is a dict of the rest of its header.

## Lookup

```python
analyser.lookup("teach")
# [LookupResult(output='teach+Noun+Masc+Com+Sg',
#               symbols=('t', 'e', 'a', 'c', 'h', '+Noun', '+Masc', '+Com', '+Sg'), weight=0.0),
#  LookupResult(output='teach+Noun+Masc+Com+Sg+DefArt', symbols=(...), weight=0.0),
#  ...]
```

`lookup` returns every result the hfst library finds, lightest first, then by
output. `output` is the output symbols joined; `symbols` lists them, without
flag diacritics and epsilons. It takes two keyword-only limits: `limit` stops
the search after that many paths, and `time_limit`, in seconds, returns the
results found so far once it runs out.

```python
analyser.lookup("teach", limit=1, time_limit=0.5)
```

That is lookup in optimized lookup. In the other types, `lookup` returns what
`hfst lookup` finds, in its order, and at most `limit` of them.

## The optimised lookup

`load_optimised_transducer(path)` and `read_optimised_transducer(data)` read
optimized-lookup files for divvunspell's lookup engine, its `divvun-fst`
crate. Its `lookup` returns what divvunspell returns for the same transducer
and word, in the same shape, and is usually several times faster.

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

## Changing a transducer

Each operation changes the transducer in place and returns `None`, as
`list.sort` does. `copy()` gives a transducer of its own to change.

```python
animals = divvun_hfst.regex("{cat}:{dog} | {mouse}:{mice}")
animals.invert()
animals.minimize()
animals.lookup("dog")  # [LookupResult(output='cat', symbols=('c', 'a', 't'), weight=0.0)]

plurals = animals.copy()
plurals.concatenate(divvun_hfst.regex("s"))
plurals.lookup("dogs")  # [LookupResult(output='cats', symbols=('c', 'a', 't', 's'), weight=0.0)]
animals.lookup("dogs")  # []
```

Each operation is named after the hfst tool that does it, and takes the
tool's options as keyword arguments, in snake_case:

```python
animals.determinize(encode_weights=True)
animals.repeat(from_=1, to=3)
animals.reweight(addition=0.5)
```

- On the transducer alone: `determinize`, `minimize`, `invert`, `reverse`,
  `remove_epsilons`, `prune_alphabet`, `project`, `push_weights`,
  `push_labels`, `realign`, `repeat`, `insert_freely`, `eliminate_flags`,
  `preprocess_for_optimized_lookup_format`, `affix_guessify`,
  `expand_equivalences`, `kill_paths`, `reweight`, `set_name`,
  `edit_metadata`, `guessify` and `substitute`.
- With another transducer: `compose`, `concatenate`, `conjunct`, `subtract`,
  `disjunct`, `priority_disjunct` and `shuffle`, and `compose_intersect`
  with a list of rule transducers.
- `convert(type)` converts it to another type, as `hfst fst2fst` does.

`guessify` also returns the model form generator `hfst guessify` writes
after the guesser. `t.compose(t)` composes `t` with itself as it was before
the call. The operations work on `openfst-tropical` and `foma`. As the tools
do, they refuse an optimized-lookup transducer, raising `HfstError` with code
`type-mismatch`; convert it first.

## Queries

Each query reports what the hfst tool it is named after reports, as frozen
dataclasses:

```python
animals = divvun_hfst.regex("{cat}:{dog} | {mouse}:{mice}")
animals.summarize().states                             # 8
[(p.input, p.output) for p in animals.extract_paths()]  # [('mouse', 'mice'), ('cat', 'dog')]
animals.compare(animals.copy())                        # True
```

A transducer has `summarize`, `check_alpha`, `compare`, `dump_alphabets`,
`extract_paths` (`hfst fst2strings`), `guess` and `naive_tokeniser` (the
tokeniser `hfst tokenize` makes of a transducer that is not a pmatch
container). The module has `lookup_report`, `flookup_report`,
`optimized_lookup_report`, `pair_test` and `grep`, and a tokeniser has
`pmatch`.

## Compiling

```python
lexc = """
Multichar_Symbols +N +Pl
LEXICON Root
Nouns ;
LEXICON Nouns
cat Noun ;
dog Noun ;
LEXICON Noun
+N:0 # ;
+N+Pl:s # ;
"""

nouns = divvun_hfst.compile_lexc([lexc], names=["nouns.lexc"])
nouns.name                # 'lexc(nouns.lexc)'
nouns.lookup("cat+N+Pl")  # [LookupResult(output='cats', ...)]
```

| Function | Tool | Returns | Options |
| --- | --- | --- | --- |
| `regex(source)` | `hfst regexp2fst` | `Transducer` | `RegexOptions` |
| `compile_lexc(sources)` | `hfst lexc` | `Transducer` | `LexcOptions` |
| `compile_twolc(source)` | `hfst twolc` | `list[Transducer]` | `TwolcOptions` |
| `compile_xfst(source, input="")` | `hfst xfst -F` | `XfstResult` | `XfstOptions` |
| `compile_pmatch(source)` | `hfst pmatch2fst` | `bytes`, which `read_tokeniser` reads | `PmatchOptions` |
| `compile_strings(source)` | `hfst strings2fst` | `Transducer` | `StringsOptions` |
| `parse_att(source)` | `hfst txt2fst` | `Transducer` | `AttOptions` |
| `parse_prolog(source)` | `hfst txt2fst --prolog` | `Transducer` | `PrologOptions` |

Each compiler gives what the tool writes of the same source. Its options are
one dataclass, made once and passed to as many calls as need them:

```python
options = divvun_hfst.LexcOptions(format="foma", warn_missing_lexicons=True)
divvun_hfst.compile_lexc([lexc], options)
```

Each field is named after the tool's option, in snake_case. `format` is the
type to compile to, `openfst-tropical` unless set; twolc and xfst work in
`openfst-tropical` or `foma`. The options are frozen, keyword-only
dataclasses, checked when they are made: a value of the wrong type raises
`TypeError`, and one the tool would turn down `ValueError`.
`dataclasses.replace` makes a variant.

`compile_xfst` runs a script as `hfst xfst -F` does, with `input` as its
standard input. Its `XfstResult` holds what the script printed (`output`),
its `stack`, the networks it defined (`definitions`) and the files it wrote
(`written`):

```python
result = divvun_hfst.compile_xfst("regex {cat}:{dog} ;\napply down cat\n")
result.output  # '4 states, 3 arcs\ndog\n'
result.stack   # (<divvun_hfst.Transducer openfst-tropical>,)
```

### Names and files

Every compiler takes `name`, the file name the tool would read the source
from. Diagnostics give it, and results are named after it as the tool names
them. `compile_lexc` takes `names`, one for each source.

`files` maps file names to their text or bytes. When it is given, a compile
reads the files its source names from it and from nowhere else, and an xfst
script's writes go only to `XfstResult.written`. Without it, a compile reads,
and a script writes, the filesystem, as the tool would.

```python
result = divvun_hfst.compile_xfst(
    "load stack nouns.hfst\ninvert net\nsave stack inverted.hfst\n",
    files={"nouns.hfst": nouns.to_bytes()},
)
inverted = divvun_hfst.read_transducer(result.written["inverted.hfst"])
inverted.lookup("cats")  # [LookupResult(output='cat+N+Pl', ...)]
```

A pmatch script finds a relative `@bin"FILE"` or `@txt"FILE"` beside `name`,
as the tool finds one beside its input:

```python
archive = divvun_hfst.compile_pmatch(
    'Define TOP @txt"animals.txt" EndTag(Animal) ;\n',
    name="lang/animals.pmatch",
    files={"lang/animals.txt": "cat\ndog\n"},
)
tokeniser = divvun_hfst.read_tokeniser(archive)
```

### Errors and warnings

A source that does not compile raises `CompileError`, a subclass of
`HfstError`. Its `diagnostics` hold every `Diagnostic` the compile made:

```python
try:
    divvun_hfst.regex("[cat", name="nouns.xre")
except divvun_hfst.CompileError as error:
    [diagnostic] = error.diagnostics
    diagnostic.message  # 'expected RightBracket'
    diagnostic.span     # (4, 4)
    print(diagnostic.rendered)
```

A `Diagnostic` has its `severity` (`error`, `warning` or `info`), `message`,
`source_name`, `span`, `labels`, `notes`, `help`, and `rendered`, the report
the hfst tools print for it, without colour. Spans count code points, as
`str` indices do, so `source[start:end]` is the text a span points at.

A compile that succeeds issues each of its warnings through the `warnings`
module, as a `CompileWarning` whose `diagnostic` is the `Diagnostic`. The
usual filters apply:

```python
import warnings

with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always", divvun_hfst.CompileWarning)
    divvun_hfst.compile_strings("cat\t-1\n")
[w.message.diagnostic.message for w in caught]
# ['Found negative weight -1.000000; negative weights are supported but iffy, ...']
```

What a compile says at the info level, as with `verbose`, goes to the `hfst`
logger.

## Writing

```python
nouns.to_bytes()                             # in its own type, as hfst fst2fst writes it
nouns.to_bytes("optimized-lookup-weighted")  # converted, for lookup
print(nouns.to_att())                        # AT&T text, as hfst fst2txt writes it
```

`to_bytes` writes a stream holding the transducer, in any type, which the
readers read. `use_backend_format=True` leaves out the HFST header. `to_att`,
`to_prolog`, `to_dot` and `to_pckimmo` return the text `hfst fst2txt` writes.
`to_dhfst_acceptor` and `to_dhfst_error_model` write DHFST files, which
`read_dhfst` reads, and `write_thfst(path)` writes a THFST directory.

`pack_bhfst` packs a speller as `hfst bhfst` does, from two DHFST files, or
two transducer streams to convert, and the text of its `meta.json`:

```python
lexicon = divvun_hfst.regex("{cat} | {dog}")
errors = divvun_hfst.regex("[a:b | b:a | a | b]*")
speller = divvun_hfst.pack_bhfst(
    lexicon.to_dhfst_acceptor(),
    errors.to_dhfst_error_model(),
    '{"id": "en"}',
)
```

With `index_xml=True` the metadata is a zhfst `index.xml` to convert, and
`speller_config` adds a speller runtime configuration to `meta.json`.

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

Every call releases the GIL while the library works: reading, lookup,
tokenising, compiling, and every operation, query and writer. Tokenisers,
optimised transducers and DHFST models never change, so threads can share one
and run in parallel:

```python
from concurrent.futures import ThreadPoolExecutor

words = ["teach", "bád", "madra"]
with ThreadPoolExecutor() as pool:
    analyses = list(pool.map(analyser.lookup, words))
```

A `Transducer` can change, so each has a lock of its own. Calls that only
read it run in parallel. An operation holds it for its length, and a call on
a transducer another thread is changing waits until the change is done, so
no thread sees a transducer half changed. To change one in several threads
at once, give each thread a `copy()`.

## Errors

A failure the library reports raises a `divvun_hfst.HfstError`, whose `code`
is the code `@divvun/hfst` gives the same failure (`divvun_hfst.HfstErrorCode`
lists them). A failed compile raises its subclass `CompileError`. Bytes a
reader does not load raise its subclass `divvun_hfst.FormatError`:

- `unsupported-format`: a format the reader does not read, such as a DHFST
  file given to `read_transducers`, or a tropical OpenFst transducer given to
  `read_optimised_transducer`. The message names it.
- `invalid-format`: bytes that are not well formed in the format they claim,
  or that end partway through it. A directory given to `load_transducers` or
  `load_transducer` that is not THFST raises it too.
- `type-mismatch`: a stream mixing transducer types.
- `transducer-count`: more transducers than the reader takes.

An argument of the wrong type or value raises the `TypeError`, `ValueError` or
`OverflowError` Python raises for it, and a file that cannot be read raises
what `open()` would. A failed call leaves the object usable.

## Logging

The library's log events go to the standard `logging` module once the call
returns, on loggers named after the Rust module they came from: under `hfst`
for the hfst library, such as `hfst.pmatch_core`; under `hfst_embed` for the
layer the package calls it through, such as `hfst_embed.compile`, which warns
of what an operation does to flag diacritics; and under `divvun_fst` for the
optimised lookup's engine. Python's own levels apply, with the library's
trace level at 5. Events below the level of `hfst` or `divvun_fst` at the
start of a call are not kept, so set those loggers' levels to see more:

```python
import logging

logging.getLogger("hfst").setLevel(logging.DEBUG)
```

## How it is checked

The suite runs the npm package's parity fixtures through the built package:
every lookup, optimised lookup, token list, tokenise format, acceptor lookup
and DHFST description in them, and compares each with what the native
library gives, value for value, and each format with what `hfst tokenise`
prints, byte for byte. It also gives the built package every input the
repository's tests give a compiler, operation, query or writer, and compares
what it returns with what the hfst tool writes or prints.

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
