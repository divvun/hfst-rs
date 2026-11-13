# Unit prediction for Greenlandic

## The question

Greenlandic is the language unit prediction was first built for: one word
can hold a stem, a chain of derivations and an ending, and Oqaasileriffik's
keyboard offers morphemes one at a time. How many keystrokes does
`hfst::prediction` save on lang-kal, with units, with the word forms the
training text showed, and with both, measured as the other languages are
(`docs/spec/prediction-evaluation.md`)?

## The walk on lang-kal

Before measuring, the walk was checked against the prediction Oqaasileriffik
built into hfst-optimized-lookup on hfst's C++ predict branch. When the
analysis typed so far runs out, that lookup reads each of 462 morpheme
names, then each of five Der tags, and offers every pair it can read; it
never asks the generator about endings. The check is
`crates/hfst/tests/prediction_kal.rs`, ignored by default because it needs
lang-kal's built transducers and runs for minutes.

On 2,000 corpus words, at every eighth word's words in progress (796 of
5,526), the emulated lookup finds 96,211 (morpheme, Der) pairs, and the
walk offers every one of them. The walk offers 276,864 endings, and each
makes a complete word that the generator writes a form for. On denser
samples (1,409 and 1,001 words in progress) 46 and 67 pairs were missed,
all after one word each, where the walk's bound of 1,000,000 configurations
cut it; with 4 or 20 million every one is offered. The test walks such
words again with 20 times the bound and fails only on misses the bound does
not explain.

Since then the walk keeps what it learns of the generator between calls.
With a kept walker the check runs in 232 s (it took 4 to 8 minutes), and
its 797 `next()` walks take 24.9 ms on average, 13.4 ms at the median and
341 ms at most; 6 of them meet the bound, where 16 did before. Entry walks
are another matter: after one or two letters the stems that can start with
them are so many that the walk does not finish within any bound tried (100
million configurations still cut after 30 s on `a`).

## The setup

- Text: the first 3,000 tokens of lang-kal's
  `tools/analysers/test/corpus.txt` (lang-kal be8b5f912), running text from
  Oqaasileriffik's pages. `scripts/prediction/kal_corpus.py` splits it at
  spaces, makes each character that is neither a letter nor a digit at the
  edge of a piece a token of its own, and analyses each token with
  `src/fst/analyser-gt-norm.hfstol`, lightest reading first. 2,313 tokens
  are words the measure types (28,835 characters), 81 of them with no
  reading; the rest are punctuation.
- Generator `src/fst/generator-gt-norm.hfstol`, unit grammar
  `crates/hfst/tests/fixtures/prediction/kal.toml`.
- k = 3 parts, so each model is built from about 2,000 tokens. Order 3,
  stupid backoff, the first reading. 3 and 4 suggestions shown.
- hfst 0f5590d. With forms on, the harness's user types the word's own
  letters until it picks a unit, each in the stem's case while the two
  agree, and picks a counted form only when it is the whole word.

```sh
HFST=$PWD/target/release/hfst      # cargo build --release -p hfst-cli
KAL=~/git/divvun/lang-kal
python3 -I scripts/prediction/kal_corpus.py $KAL/tools/analysers/test/corpus.txt \
    --hfst $HFST --analyser $KAL/src/fst/analyser-gt-norm.hfstol \
    --tokens 3000 --out /tmp/kal-3000.txt
$HFST predict-eval -g $KAL/src/fst/generator-gt-norm.hfstol \
    -G crates/hfst/tests/fixtures/prediction/kal.toml --folds 3 --order 3 \
    -n 3,4 --whole-words 0,1 --forms off,on --units on,off --unit-weight 1,0.1 \
    /tmp/kal-3000.txt
```

The script's own logic is tested by
`python3 -I scripts/prediction/test_kal_scripts.py`.

## Results

Keystrokes saved. K is the number of complete words each stem brings, w the
unit weight.

| system | 3 shown | 4 shown |
|---|---:|---:|
| units, K = 0 | 11.2% | 15.9% |
| units, K = 1 | 10.6% | 15.1% |
| forms | 6.2% | 6.6% |
| both, K = 0, w 1 | 10.3% | 14.8% |
| both, K = 0, w 0.1 | 8.4% | 12.0% |
| both, K = 1, w 1 | 10.0% | 13.9% |
| both, K = 1, w 0.1 | 8.3% | 11.4% |

Units alone give the figures measured before the walk kept its memo (11.2%
and 15.9%, 10.6% and 15.1%).

By units in the word, 3 shown:

| units | words | chars | units, K = 0 | units, K = 1 | forms | both, K = 0, w 1 | both, K = 0, w 0.1 | both, K = 1, w 1 | both, K = 1, w 0.1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 2313 | 28835 | 11.2% | 10.6% | 6.2% | 10.3% | 8.4% | 10.0% | 8.3% |
| 2 | 704 | 5306 | 21.1% | 21.5% | 19.3% | 21.5% | 20.6% | 21.7% | 21.1% |
| 3 | 540 | 5697 | 13.8% | 12.6% | 7.2% | 13.4% | 10.1% | 12.5% | 10.0% |
| 4 | 396 | 5385 | 12.5% | 10.6% | 5.0% | 11.4% | 11.1% | 10.8% | 10.5% |
| 5 | 285 | 4871 | 7.0% | 6.6% | 0.2% | 4.5% | 1.0% | 3.9% | 0.6% |
| 6 | 167 | 3250 | 6.2% | 6.1% | 0.0% | 4.0% | 2.1% | 4.2% | 1.9% |
| 7 | 89 | 1912 | 7.1% | 6.6% | 4.6% | 8.4% | 5.0% | 9.4% | 6.1% |
| 8 | 35 | 825 | 1.9% | 3.3% | 0.0% | 0.6% | -2.7% | 0.7% | -2.5% |
| 9 | 11 | 306 | -11.1% | -10.8% | 0.0% | -17.3% | -15.7% | -17.3% | -15.7% |
| 10 | 3 | 79 | 1.3% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| 11 | 2 | 60 | -20.0% | -20.0% | 0.0% | -20.0% | 0.0% | -20.0% | 0.0% |
| no reading | 81 | 1144 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |

4 shown:

| units | words | chars | units, K = 0 | units, K = 1 | forms | both, K = 0, w 1 | both, K = 0, w 0.1 | both, K = 1, w 1 | both, K = 1, w 0.1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 2313 | 28835 | 15.9% | 15.1% | 6.6% | 14.8% | 12.0% | 13.9% | 11.4% |
| 2 | 704 | 5306 | 24.8% | 25.7% | 20.5% | 25.7% | 24.6% | 25.3% | 24.4% |
| 3 | 540 | 5697 | 17.9% | 16.4% | 7.7% | 18.0% | 14.3% | 16.2% | 13.0% |
| 4 | 396 | 5385 | 18.0% | 15.4% | 5.4% | 16.3% | 14.6% | 14.7% | 13.1% |
| 5 | 285 | 4871 | 11.9% | 11.2% | 0.2% | 9.4% | 5.1% | 8.5% | 4.4% |
| 6 | 167 | 3250 | 11.3% | 11.0% | 0.0% | 7.6% | 4.1% | 7.8% | 4.0% |
| 7 | 89 | 1912 | 13.5% | 13.1% | 4.6% | 13.7% | 9.6% | 13.9% | 10.2% |
| 8 | 35 | 825 | 9.6% | 10.4% | 0.0% | 6.1% | 2.5% | 5.7% | 2.5% |
| 9 | 11 | 306 | -1.0% | -1.0% | 0.0% | -7.8% | -10.1% | -7.8% | -10.1% |
| 10 | 3 | 79 | 11.4% | 10.1% | 0.0% | 11.4% | 8.9% | 10.1% | 8.9% |
| 11 | 2 | 60 | -18.3% | -18.3% | 0.0% | -18.3% | 0.0% | -18.3% | 0.0% |
| no reading | 81 | 1144 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |

Reading:

- Units alone with no complete words save the most: 11.2% with 3 shown and
  15.9% with 4.
- Forms alone save 6.2% and 6.6%. With about 2,000 tokens of training text
  per part, most Greenlandic words typed were never seen in it; forms help
  only the two-unit words (19.3% and 20.5%) and save nothing on words of
  five units or more.
- Both together cost points against units alone: 0.9 and 1.1 points at
  weight 1, 2.8 and 3.9 at weight 0.1. A counted form takes a place a stem
  would have had, and the user, who types the word's own letters with forms
  on, loses the stem where the word does not start with it.
- Words of nine and eleven units cost more taps than their characters:
  the user picks every unit it is offered, and some units save fewer
  letters than the pick costs.
- 1,594 of the 2,313 words (1,583 to 1,599 with forms on) met the bound of
  1,000,000 configurations in some walk. The faster walk's `next()` seldom
  meets it (6 of 797 walks in the check above), while entry walks after the
  first letters never finish within it, so most of these are entry walks.

The run of all 14 variants took 5.5 minutes (331 s) and 3,235 CPU seconds
with 18 threads, and at most 22.5 GB of memory.

## Against Oqaasileriffik's 46 to 48%

Oqaasileriffik reports savings of 46% to 48% for its Greenlandic
prediction. These numbers cannot be set beside it, for two reasons
(`plan/decisions/prediction-measured-choices.md`):

- The measure differs. Its figure counts keystrokes in morpheme names and
  leaves the endings out. Keystrokes saved here counts every character of
  the word as written, endings included, against the taps of a keyboard
  that shows 3 or 4 suggestions.
- The text differs by four orders of magnitude. Its model was built from
  20.3 million words; each model here from about 2,000 tokens, the other
  two parts of 3,000. A unit or a form the training text never showed is
  ranked by the generator's weight alone, and most are, here.

## What to ship

For Greenlandic, as measured: units alone, with no complete words, order 3
with stupid backoff, 4 suggestions shown (15.9%). Forms and complete words
lower the savings on this text. This rests on a training text of about
2,000 tokens, far too little for forms, which need most words to have been
seen; the choice should be measured again on a larger analysed text before
it is fixed.

## Limits

- 3,000 tokens of one text, and models of about 2,000 tokens. The order,
  the weight and whether forms help will move with more text.
- No constraint-grammar disambiguation: the first reading is the target and
  the one counted.
- Entry walks after one or two letters do not finish within the bound, so
  the stems offered there may be missing some.
- The unit grammar's keys keep orthography, usage and loan tags (such as
  `+Orth/Alt` and `+OLang/DAN`), which split counts that belong together.
- The simulated user picks every unit it is offered and never mistypes.
- Memory: 22.5 GB at most for a run of 18 threads, against 7.0 GB for
  North Sámi's 51,954 words.
