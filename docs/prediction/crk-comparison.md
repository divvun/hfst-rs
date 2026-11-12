# Unit prediction and lang-crk's morph-completion speller on Plains Cree

## The question

How many keystrokes does unit prediction save a Plains Cree writer, next to
the morph-completion speller that Lane, Harrigan and Arppe built for Plains
Cree ("Interactive word completion for Plains Cree", ACL 2022)? Both are
scored here by the one measure in `docs/spec/prediction-evaluation.md`, on
the same held-out words, with the keyboard showing 3 suggestions, and also
10.

## The two systems

Unit prediction is `hfst predict-eval` over `hfst::prediction`. It walks
lang-crk's generator (`src/fst/generator-gt-norm.hfstol`) and offers units
cut by the unit grammar `crates/hfst/tests/fixtures/prediction/crk.toml`:
prefixes (preverbs, reduplication, initial change), stems, derivation
boundaries, endings, and two person-first filters, `ni-` and `ki-`. A model
built from the training text ranks the units. The variants scored are model
order 1, 2 and 3, all with stupid backoff and the first reading of each
token.

The morph-completion speller is lang-crk's
`tools/spellcheckers/crk-x-predict_morphs-mobile.zhfst`, run with
divvunspell. For what has been typed it suggests either a complete word or
an incomplete one that runs to the next morpheme boundary, marked with `~`.
Its acceptor weights each start of a word by how often words in a corpus
begin with it (`tools/spellcheckers/prefix_weighting.lexc`). Those weights
were counted from the Ahenakew-Wolfart-Bloomfield corpus, the same corpus
the words here come from.

Versions measured: hfst d472f02 (the harness and the tools), lang-crk
edd4d027 built with hfst 23ad222, divvunspell 1.0.0-beta.13 (e31b55c). The
whole-word numbers were measured with hfst de85176, whose results without
whole words equal d472f02's on this text, row for row.

## The data and its limits

lang-crk has the tagged corpus only as two frequency lists,
`devtools/testdata/ahenakew_wolfart_MGS_tab-sep-anls_freq-sorted.txt` and
`bloomfield_MGS_tab-sep-anls_freq-sorted.txt`. Each line is a count, a
space, a word, and the word's analyses after tabs. Together they hold
246,437 tokens. There is no running text, so there is no word order and no
context across words.

The lists' analyses use an older tag set than today's lang-crk: a `+Prs`
tag, `y` where today's lemmas have `ý`, and other pronoun and particle
tags. Today's generator refuses the lists' analyses for about half of the
word tokens. So each word was
analysed again with today's normative analyser
(`src/fst/analyser-gt-norm.hfstol`), and the generator's own check kept the
readings it accepts. A reading that agrees with one of the list's analyses,
once `+Prs` is dropped and `ý` is read as `y`, comes first. For 103,414 of
the kept tokens the first reading agrees with the list.

Tokens left out:

| why | tokens |
|---|---:|
| punctuation | 80,346 |
| the list marks every analysis as English (`+Eng`) or misspelt (`+Err/`) | 26,671 |
| today's normative analyser does not know the word | 15,310 |
| kept | 124,110 (22,737 distinct words) |

For 584 kept tokens the generator's first form of the first reading is
another spelling of the word, so unit prediction cannot write them and
types them in full.

The speller's weights were counted from this corpus, including the words it
is scored on. That favours it. To measure by how much, its weights were
counted again without each held-out part (see the setup).

## The setup

The kept tokens were expanded by their counts and shuffled with seed 1.
After each word comes the token `.` with no reading. The model builder and
the harness both read a token with no reading as a break, so no run of
units spans two words and every word is typed with no units before it.
Real order is unknown, so none is invented. This is fair to both systems in
the same way: neither gets context. It does take away what unit
prediction's model could learn from context across words.

The harness splits the text into k = 10 contiguous parts and scores each
with a model built from the other nine.

Typing the whole text was out of reach. A timed pilot of 300 typed words
with one variant took 10 s (45 CPU seconds, most of it building the ten
models). The main run of 5,000 typed words with six variants then took 19
minutes and 5,040 CPU seconds on a shared machine, about 0.17 CPU seconds
per word per variant. The whole text, 124,110 tokens, would take about 35
CPU hours for unit prediction. divvunspell needs about 0.4 CPU seconds per
start of a word it is asked about, and the whole text has 110,084 distinct
starts: about 12 CPU hours for the shipped speller alone, before the ten
speller rebuilds.

So 5,000 tokens, drawn at random with the seed, are typed, 470 to 560 in
each part. Every other token is written with the surface `_`, which has no
character the harness counts: the model builder counts its reading, the
harness does not type it, and divvunspell is not asked about it. Each
part's model is built from all the text outside the part, about 111,700
tokens, and a random sample of the part is scored. Both systems are scored
on exactly these 5,000 tokens, 36,719 characters.

The speller is scored through the harness's external recording
(`prediction-evaluation.external`). For every typed word and every start of
it, divvunspell is asked once with `-S --json -n 10 --continuation-marker
'~'`. Its plain-text output marks complete words as incomplete, so the JSON
is read. The first 3 of the 10 are taken as what 3 shown would be; on 300
starts drawn at random, `-n 3` gave exactly those. divvunspell skips an
empty line, so the speller offers nothing before the first letter.

A suggestion ending in `~` writes its text without the `~`. Any other is a
complete word: picking it writes the word and a space, so it helps only
when it is the word being typed. It is recorded with the trailing space
unless it is the word, so the harness cannot pick it as the start of a
longer word. The lenient numbers, with complete words recorded bare, are
also given.

Three speller acceptors were scored:

- shipped: the zhfst as lang-crk built it.
- recount: weights counted again from the whole text by
  `scripts/prediction/crk_speller.py`, which follows
  `src/derivation/morpheme-completion-weighting.sh` (that script reads a
  file not in lang-crk). Against the shipped `prefix_weighting.lexc` it has
  33,769 starts to 33,902, 32,477 shared, with a mean gap of 0.021 in
  weight on the shared ones. Compiling the shipped `prefix_weighting.lexc`
  with the script's build gives an acceptor byte-identical to the shipped
  one, which checks the build.
- held out: for each part, weights counted from the other nine parts only,
  compiled the same way. Each build takes about 80 s and 1.4 GB.

A recording that depends on the part cannot be scored in one harness run,
since a recording is scored on the whole text. Each part was scored on its
own, and `scripts/prediction/crk_sum_folds.py` sums the parts by
characters. The harness rounds keystrokes saved to a tenth of a percent, so
a sum is within 0.05 points.

### Reproducing

```sh
HFST=$PWD/target/release/hfst      # cargo build --release -p hfst-cli
CRK=~/git/divvun/lang-crk          # built, at edd4d027
DS=~/git/divvun/divvunspell/target/release/divvunspell
G=$CRK/src/fst/generator-gt-norm.hfstol
U=crates/hfst/tests/fixtures/prediction/crk.toml
Z=$CRK/tools/spellcheckers/crk-x-predict_morphs-mobile.zhfst
T=$CRK/devtools/testdata
W=/tmp/crk-cmp                     # any work directory
mkdir -p $W/rec $W/runs

# The text, its ten parts, and the 5,000 typed tokens.
python3 -I scripts/prediction/crk_corpus.py --hfst $HFST \
    --analyser $CRK/src/fst/analyser-gt-norm.hfstol --generator $G \
    --out $W/corpus --seed 1 --folds 10 --typed 5000 \
    $T/ahenakew_wolfart_MGS_tab-sep-anls_freq-sorted.txt \
    $T/bloomfield_MGS_tab-sep-anls_freq-sorted.txt

# Unit prediction.
$HFST predict-eval -g $G -G $U --folds 10 --order 1,2,3 -n 3,10 \
    $W/corpus/text.txt

# The shipped speller. With --seconds the script stops early and exits 3;
# run it again and it goes on from its cache.
python3 -I scripts/prediction/crk_record.py $W/corpus/text.txt \
    --divvunspell $DS --archive $Z --out $W/rec/shipped \
    --cache $W/rec/shipped.cache.jsonl --jobs 8
$HFST predict-eval -g $G -G $U --folds 10 --order 3 -n 3 \
    --recording "arppe n3=$W/rec/shipped-n3.tsv" \
    --recording "arppe n10=$W/rec/shipped-n10.tsv" \
    --recording "arppe n3 lenient=$W/rec/shipped-n3-lenient.tsv" \
    --recording "arppe n10 lenient=$W/rec/shipped-n10-lenient.tsv" \
    $W/corpus/text.txt

# The recount, and the held-out speller for each part.
mkdir -p $W/unzipped && unzip -o $Z -d $W/unzipped
E=$W/unzipped/errmodel.default.hfst
python3 -I scripts/prediction/crk_speller.py $W/corpus/text.txt \
    --hfst $HFST --lang-crk $CRK --out $W/weights-all --build \
    --compare $CRK/tools/spellcheckers/prefix_weighting.lexc
python3 -I scripts/prediction/crk_record.py $W/corpus/text.txt \
    --divvunspell $DS --lexicon $W/weights-all/acceptor.hfstol $E \
    --out $W/rec/recount --cache $W/rec/recount.cache.jsonl --jobs 8
for F in 1 2 3 4 5 6 7 8 9 10; do
    python3 -I scripts/prediction/crk_speller.py $W/corpus/text.txt \
        --hfst $HFST --lang-crk $CRK --out $W/weights-$F \
        --folds 10 --leave-out $F --build
    python3 -I scripts/prediction/crk_record.py $W/corpus/fold-$F.txt \
        --divvunspell $DS --lexicon $W/weights-$F/acceptor.hfstol $E \
        --out $W/rec/fold-$F --cache $W/rec/fold-$F.cache.jsonl --jobs 8
    # The unit variant here is only there because the tool needs one.
    $HFST predict-eval -g $G -G $U --folds 2 --order 1 -n 3 \
        --recording "shipped n3=$W/rec/shipped-n3.tsv" \
        --recording "shipped n10=$W/rec/shipped-n10.tsv" \
        --recording "recount n3=$W/rec/recount-n3.tsv" \
        --recording "recount n10=$W/rec/recount-n10.tsv" \
        --recording "held-out n3=$W/rec/fold-$F-n3.tsv" \
        --recording "held-out n10=$W/rec/fold-$F-n10.tsv" \
        --recording "held-out n3 lenient=$W/rec/fold-$F-n3-lenient.tsv" \
        --recording "held-out n10 lenient=$W/rec/fold-$F-n10-lenient.tsv" \
        -o $W/runs/fold-$F.txt $W/corpus/fold-$F.txt
done
python3 -I scripts/prediction/crk_sum_folds.py $W/runs/fold-*.txt
```

Nothing in lang-crk is written. `crk_speller.py` copies the acceptor's
hfscript into its output directory with the generator's path made absolute.
The scripts' own logic is tested by
`python3 -I scripts/prediction/test_crk_scripts.py`.

## Results

All numbers are on the same 5,000 held-out tokens, 36,719 characters. Each
word in this text has at least two units: a word with no inflection still
has a stem and a tag such as `+Ipc`.

### Keystrokes saved

Unit prediction is order 3. The speller's held-out numbers are summed over
the ten parts.

| units | words | chars | units, 3 shown | speller held out, 3 | speller shipped, 3 | units, 10 shown | speller held out, 10 | speller shipped, 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 5000 | 36719 | 25.9% | 29.4% | 34.0% | 43.8% | 41.2% | 44.5% |
| 2 | 3839 | 22710 | 28.3% | 34.7% | 35.3% | 40.2% | 45.4% | 45.9% |
| 3 | 818 | 9066 | 23.2% | 21.1% | 30.1% | 50.0% | 33.2% | 39.9% |
| 4 | 283 | 3841 | 19.6% | 21.4% | 33.6% | 50.0% | 35.8% | 45.4% |
| 5 | 49 | 855 | 15.1% | 19.3% | 37.9% | 43.0% | 36.2% | 48.8% |
| 6 | 10 | 224 | 40.6% | 17.9% | 38.4% | 59.8% | 36.2% | 46.9% |
| 7 | 1 | 23 | 21.7% | 13.0% | 34.8% | 47.8% | 34.8% | 52.2% |

With complete words recorded bare (lenient), the speller saves 30.4% (3
shown) and 42.0% (10 shown) held out, and 34.8% and 45.2% shipped.

### Unit prediction by order

`saved` is keystrokes saved. `first` and `shown` are how often the word's
next unit was the first suggestion, or among those shown, when the user
looked for it. `full` counts words typed in full.

3 shown:

| units | words | o1 saved | o1 first | o1 shown | o1 full | o2 saved | o2 first | o2 shown | o2 full | o3 saved | o3 first | o3 shown | o3 full |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 5000 | 22.1% | 43.4% | 64.6% | 40 | 25.8% | 48.3% | 67.3% | 27 | 25.9% | 48.7% | 67.4% | 24 |
| 2 | 3839 | 26.6% | 47.6% | 68.6% | 19 | 28.3% | 53.4% | 69.9% | 19 | 28.3% | 53.4% | 69.9% | 19 |
| 3 | 818 | 17.4% | 32.1% | 53.9% | 19 | 23.1% | 34.5% | 59.4% | 7 | 23.2% | 35.9% | 60.1% | 5 |
| 4 | 283 | 11.2% | 41.3% | 62.0% | 1 | 18.7% | 44.0% | 66.2% | 0 | 19.6% | 45.1% | 66.1% | 0 |
| 5 | 49 | -1.6% | 38.0% | 59.2% | 1 | 17.8% | 45.3% | 69.0% | 1 | 15.1% | 44.9% | 67.3% | 0 |
| 6 | 10 | 33.0% | 40.0% | 65.0% | 0 | 43.8% | 46.7% | 71.7% | 0 | 40.6% | 45.0% | 70.0% | 0 |
| 7 | 1 | 34.8% | 42.9% | 85.7% | 0 | 21.7% | 42.9% | 71.4% | 0 | 21.7% | 42.9% | 71.4% | 0 |

10 shown:

| units | words | o1 saved | o1 first | o1 shown | o1 full | o2 saved | o2 first | o2 shown | o2 full | o3 saved | o3 first | o3 shown | o3 full |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 5000 | 42.6% | 43.4% | 81.3% | 3 | 43.7% | 48.3% | 82.4% | 3 | 43.8% | 48.7% | 82.5% | 3 |
| 2 | 3839 | 39.9% | 47.6% | 83.0% | 3 | 40.2% | 53.5% | 83.3% | 3 | 40.2% | 53.5% | 83.3% | 3 |
| 3 | 818 | 47.5% | 31.9% | 76.5% | 0 | 49.7% | 34.5% | 78.9% | 0 | 50.0% | 35.9% | 79.1% | 0 |
| 4 | 283 | 47.4% | 41.3% | 79.9% | 0 | 49.5% | 44.0% | 83.2% | 0 | 50.0% | 45.1% | 84.2% | 0 |
| 5 | 49 | 39.4% | 38.0% | 78.4% | 0 | 43.2% | 45.3% | 82.4% | 0 | 43.0% | 44.9% | 82.0% | 0 |
| 6 | 10 | 59.4% | 40.0% | 83.3% | 0 | 60.3% | 46.7% | 90.0% | 0 | 59.8% | 45.0% | 88.3% | 0 |
| 7 | 1 | 47.8% | 42.9% | 85.7% | 0 | 47.8% | 42.9% | 85.7% | 0 | 47.8% | 42.9% | 85.7% | 0 |

Order 2 and 3 are close; order 1 is behind at 3 shown, mostly on words of
three or more units.

### The speller's top-1 and top-N rates

The harness gives no first or shown rate for a recording, since the
speller's suggestions are not units. `crk_record.py` reports a nearby
measure: over every typed token and every start of it from one letter to
all but the last (33,322 starts), how often a suggestion shown is useful,
one the word starts with that is longer than the start (a complete word
only when it is the word).

| speller | first | among 3 | among 10 |
|---|---:|---:|---:|
| shipped | 45.5% | 65.4% | 77.9% |
| recount | 45.9% | 66.4% | 78.8% |
| held out | 40.1% | 61.1% | 75.6% |

These are not comparable with unit prediction's `first` and `shown`, which
are judged once per unit the user looks for.

### The weight leak, part by part

Keystrokes saved by the speller on each part's typed words.

| part | words | chars | shipped, 3 | recount, 3 | held out, 3 | shipped, 10 | recount, 10 | held out, 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 496 | 3614 | 33.6% | 33.6% | 27.6% | 43.6% | 44.0% | 40.1% |
| 2 | 506 | 3667 | 33.7% | 34.3% | 29.8% | 45.0% | 45.0% | 41.8% |
| 3 | 560 | 4181 | 33.2% | 33.8% | 28.0% | 43.9% | 44.4% | 40.1% |
| 4 | 474 | 3538 | 34.5% | 35.4% | 30.0% | 44.8% | 45.4% | 41.3% |
| 5 | 515 | 3840 | 34.0% | 34.6% | 29.1% | 44.6% | 45.3% | 41.1% |
| 6 | 470 | 3376 | 34.9% | 35.0% | 30.6% | 44.9% | 44.9% | 42.1% |
| 7 | 482 | 3503 | 34.5% | 34.9% | 31.6% | 44.6% | 45.2% | 42.8% |
| 8 | 502 | 3714 | 33.7% | 34.2% | 29.9% | 43.9% | 44.2% | 41.4% |
| 9 | 475 | 3512 | 33.4% | 34.1% | 28.3% | 44.7% | 45.0% | 40.2% |
| 10 | 520 | 3774 | 34.4% | 35.1% | 29.8% | 44.7% | 44.9% | 41.0% |
| all | 5000 | 36719 | 34.0% | 34.5% | 29.4% | 44.5% | 44.8% | 41.2% |

The recount is within a point of the shipped speller on every part, so the
recount follows the shipped weights closely. Leaving the scored part out of
the count costs the speller 5.1 points with 3 shown and 3.6 points with 10.
That is the size of the leak. The held-out speller ranges from 27.6% to
31.6% across parts with 3 shown.

### Against the paper's figure

The paper reports that with its best ranking 64.9% of its queries had a
useful completion in the top 10. That counts how often help is on screen,
query by query. Keystrokes saved counts what a user who takes that help
saves, word by word, and it rewards a long useful suggestion more than a
short one. The nearest figure here is the speller's share of starts with a
useful suggestion among 10: 77.9% shipped, 75.6% held out. That figure uses
other queries (every start of every token), another lexicon version and
our notion of useful, so it does not test the paper's number.

## The person-first filter

The grammar's `ni-` and `ki-` filters let a user start a verb with its
person prefix, as Plains Cree writes it. Picking `ni-` sets the flag
`@U.person.NI@`, so the generator then offers only endings that agree.

The harness cannot measure this. Its simulated user picks only units from
the word's analysis, and the unit grammar's `segment()` never yields a
filter, since a filter is not a symbol of the analysis. The harness was not
changed for this comparison.

The filter was measured directly instead, with a throwaway program over the
walker (not kept). For every held-out token of the whole text (all 124,110,
with each part's order-3 model built as the harness builds it) whose
surface starts with `ni` or `ki`, the word was built from its analysis up
to its ending, once from the empty word and once after picking the filter.
Then all units the generator offers there were ranked by the model, and
the rank of the word's own ending noted. A verb counts when the generator
still offers its ending after the filter, that is, when its ending agrees;
673 `ni` and 638 `ki` verb tokens did not agree, such as words whose stem
starts with those letters.

| words | tokens (distinct) | | ending first | in top 3 | in top 10 | median rank | units offered (mean) |
|---|---|---|---:|---:|---:|---:|---:|
| verbs, `ni-` | 3222 (1659) | without | 27.6% | 61.0% | 90.3% | 3 | 58.7 |
| | | with | 67.2% | 91.8% | 99.8% | 1 | 7.8 |
| verbs, `ki-` | 1025 (694) | without | 10.4% | 28.2% | 62.0% | 7 | 69.7 |
| | | with | 49.0% | 78.1% | 93.1% | 2 | 17.6 |
| nouns, `ni-` | 2183 (261) | without | 51.2% | 83.6% | 96.0% | 1 | 25.7 |
| | | with | 51.2% | 83.6% | 96.0% | 1 | 25.7 |
| nouns, `ki-` | 1031 (225) | without | 41.5% | 68.3% | 92.1% | 2 | 31.1 |
| | | with | 41.8% | 68.6% | 92.1% | 2 | 31.1 |

For verbs the filter cuts what is offered after the stem from about 60 to
70 units to 8 or 18, and puts the right ending among 3 suggestions for most
words. It costs one pick, which this table does not count.

For nouns the filter does nothing: 3,204 of the 3,214 noun tokens are
offered exactly the same units with it as without. This is the known gap.
lang-crk's noun possessor prefixes set the person flag with `@P`, which
overwrites the `@U` value the filter set, so every possessor stays open.

## Whole-word offers

The spec offers complete words beside the stems (`prediction.whole-words`):
while a stem can be picked, each stem on offer also comes as up to K
complete words, the stem with each of its K highest-ranked endings. Picking
one writes the word. This was measured on the same text, parts and 5,000
typed tokens, with order 3, K = 0, 1 and 2, and 3 and 10 shown:

```sh
$HFST predict-eval -g $G -G $U --folds 10 --order 3 -n 3,10 \
    --whole-words 0,1,2 $W/corpus/text.txt
```

K = 0 offers no complete words and gives the order-3 numbers above
exactly. Keystrokes saved, with the held-out speller beside them:

| units | words | chars | K=0, 3 shown | K=1, 3 | K=2, 3 | speller held out, 3 | K=0, 10 shown | K=1, 10 | K=2, 10 | speller held out, 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 5000 | 36719 | 25.9% | 32.4% | 31.8% | 29.4% | 43.8% | 51.5% | 51.5% | 41.2% |
| 2 | 3839 | 22710 | 28.3% | 40.4% | 40.3% | 34.7% | 40.2% | 52.9% | 53.2% | 45.4% |
| 3 | 818 | 9066 | 23.2% | 20.4% | 18.8% | 21.1% | 50.0% | 49.6% | 49.0% | 33.2% |
| 4 | 283 | 3841 | 19.6% | 18.0% | 17.2% | 21.4% | 50.0% | 49.3% | 48.7% | 35.8% |
| 5 | 49 | 855 | 15.1% | 14.2% | 11.6% | 19.3% | 43.0% | 43.2% | 43.2% | 36.2% |
| 6 | 10 | 224 | 40.6% | 36.6% | 32.6% | 17.9% | 59.8% | 56.2% | 55.4% | 36.2% |
| 7 | 1 | 23 | 21.7% | 21.7% | 13.0% | 13.0% | 47.8% | 34.8% | 39.1% | 34.8% |

Words typed in full: 24, 41 and 47 with 3 shown; 3, 5 and 5 with 10.

The harness's first and shown rates fall with whole words (with 3 shown,
first 48.7%, 27.3% and 27.0%; shown 67.4%, 52.2% and 49.2%). They do not
compare across K: a complete word picked ends the word, so the look for
its ending is never made, and the rates count a different set of looks.

Reading:

- Whole words help Cree overall. K = 1 adds 6.5 points with 3 shown and 7.7
  with 10. With 3 shown, unit prediction then saves more than the held-out
  speller (32.4% to 29.4%), though less than the shipped speller with its
  leaked weights (34.0%). With 10 shown it saves 51.5%, against 41.2% held
  out and 44.5% shipped.
- K = 2 is no better than K = 1: 0.6 points worse with 3 shown, equal with
  10.
- The whole gain is on two-unit words, 77% of the tokens and 62% of the
  characters: 12.1 points with 3 shown and 12.7 with 10 at K = 1. These are
  words of a stem and one ending, with no prefix or derivation, where a
  complete word saves the pick of the ending.
- On words of three or more units Cree behaves like Greenlandic, where
  whole words cost 0.6 to 1.6 points: they lose here too. Taken together
  (14,009 characters, worked out from the rows above), those words save
  22.0% at K = 0, 19.6% at K = 1 and 18.1% at K = 2 with 3 shown, and
  49.7%, 49.2% and 48.7% with 10. The cost is larger with 3 shown, where
  places are scarce, and grows with K.
- The Greenlandic reason holds for Cree: a stem ranks below its own words
  and drops out of what is shown. At the 462 distinct starts of one to three
  letters of the typed words, with 3 shown and a model built from the whole
  text, 1,290 stems are shown without complete words. With K = 1, 602 of
  them are still shown, 459 have given their place to one of their own
  complete words, and 229 have been pushed out by other suggestions. With
  K = 2 the numbers are 477, 507 and 306. A longer word on such a stem has
  to be found with more letters, which fits the losses above. Cree differs
  from Greenlandic only in that most of its running words are two units
  long, so the gain there outweighs the loss.

The starts were counted with `hfst predict -j -m MODEL -n 3 --whole-words K`
over a file of lines each holding a tab and one start, lower-cased, the
model built by `hfst predict-model --order 3` from the whole text.

## What this does and does not show

It shows, on words of this corpus typed one at a time:

- With 3 suggestions shown, the speller with weights counted without the
  scored words saves 29.4% of keystrokes, and unit prediction at order 3
  saves 25.9%.
- With 10 shown, unit prediction saves 43.8% and the held-out speller
  41.2%.
- Without complete words, unit prediction gains most over the speller on
  words of three or more units when 10 are shown, and is behind on
  two-unit words.
- With one complete word for each stem (K = 1), unit prediction saves
  32.4% with 3 shown and 51.5% with 10, more than the held-out speller in
  both. The gain is on two-unit words; words of three or more units lose a
  little, as on Greenlandic.
- Order 2 and order 3 are within 0.1 point of each other here; order 1 is
  3.8 points behind at 3 shown.
- The shipped speller scores 4.6 points more than the held-out one at 3
  shown, and 3.3 more at 10, because its weights were counted from the
  scored words. Counted the same way with and without the scored words,
  the gap is 5.1 and 3.6 points.

It does not show:

- How either system does with context across words. There was none to
  give. Unit prediction's model can use the units of the words before; the
  speller cannot.
- What users would do. The simulated user takes the best offer every time,
  never mistypes, never backs out of a pick, and keeps using units even
  where typing would be cheaper (order 1 with 3 shown loses 1.6% on
  five-unit words).
- Anything about the words left out: the misspelt and English words, and
  the 15,310 tokens today's normative analyser does not know.
- How the filter pays off in taps, which the harness cannot count.
- Small differences. Only 5,000 tokens are typed, and the groups of five
  units or more are small.

## Open problems

- The noun filters do nothing, because the possessor prefixes in lang-crk
  set the person flag with `@P` and so overwrite the filter's `@U` value.
- The harness cannot score filters: its user picks only units from the
  analysis, and `segment()` never yields a filter. The model never counts a
  filter either, so at the start of a word a filter ranks after every
  prefix the model has seen.
- A recording is scored on the whole text, not part by part. A system whose
  suggestions depend on its training part has to be scored part by part in
  separate runs and summed outside the harness.
- A recording cannot mark a suggestion as a complete word, and the
  recording rule lets the user pick a complete word as the start of a
  longer one. Here complete words were recorded with a trailing space; the
  lenient numbers show the rule as written adds about a point.
- Speed: on d472f02 the harness took about 0.17 CPU seconds per word per
  variant on this text, and divvunspell about 0.4 CPU seconds per start.
  On de85176 the six whole-word variants took under six minutes together.
- With complete words on offer, a stem often gives its place to one of its
  own complete words, and words of three or more units lose points. How a
  stem should rank against its own words is open, as on Greenlandic.
- Some complete words do not start with the letters typed, because the
  word a Cree stem writes can begin with a person prefix or another form of
  the stem: after `apw` the offer holds `nitapwêsin`, after `at` it holds
  `otêma`. At the starts counted above, 31 of the 702 complete words shown
  at K = 1, and 47 of 832 at K = 2, are like this. Whether a keyboard
  should offer them in place of what was typed is a question for speakers.
- The tagged frequency lists use an older tag set than today's lang-crk and
  had to be analysed again. Today's normative analyser does not know some
  frequent words in them, such as `êwako`, `namôya`, `êkonik` and `kotak`.
