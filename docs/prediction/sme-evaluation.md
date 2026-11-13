# Unit prediction for North Sámi

## The question

Most North Sámi words are a stem and one ending, sometimes with a
derivation or a compound part. Chaining units saves less than in
Greenlandic, where one word can hold several derivations. Unit prediction
was hoped to help North Sámi in other ways. Counting stems and endings
apart pools sparse data: a stem seen in one form helps all its forms. And
the generator writes consonant gradation for the user: pick `girji` and
`+N+Pl+Acc` and get `girjjiid`.

Does unit prediction save enough keystrokes, next to ordinary completion of
word forms, to be worth shipping for North Sámi?

## The systems

Unit prediction is `hfst predict-eval` over `hfst::prediction`. It walks
lang-sme's generator (`src/fst/generator-gt-norm.hfstol`) and offers units
cut by the unit grammar `crates/hfst/tests/fixtures/prediction/sme.toml`:
stems, derivation and compound boundaries, endings and enclitics. A model
built from the training text ranks them. With whole words on
(`prediction.whole-words`), each stem on offer is also offered as complete
words: the stem with each of its K best endings, scored as stem times
ending. The variants differ in model order (1, 2, 3), backoff (drop-oldest,
stupid backoff), how ambiguous tokens are counted (first reading, equal
shares), the number of suggestions shown (N = 3, 4), K (0, 1, 2) and
keyboard faces.

Two baselines complete word forms. Both are built from the training text
of each fold only, and both match letters without regard to capitals.

- unigram: for what has been typed, the N most frequent word forms of the
  training text that start with it. Ties go by the letters of the form.
- bigram: the N most frequent forms that followed the previous token in the
  training text and start with what is typed, then the unigram's to make
  up N. The previous token may be punctuation.

A suggestion is shown with the capitals the user typed, so a capital is
the keyboard's shift state in both systems, as the unit user treats it.
The baselines also show words before the first letter.

The baselines are scored by the measure's rule for recorded suggestions
(`prediction-evaluation.external`): the user picks the longest suggestion
the word starts with, or types the next letter.

That rule lets the user pick a suggestion that only starts the word, such
as `ja` while typing `jagi`, or `ráhkesvuođa` while typing
`ráhkesvuođabáhkkomis`, and then type on. The measure's own user
(`prediction-evaluation.keystrokes`), which types every unit variant, picks
only the word's next unit or the whole word, as a keyboard does that ends a
picked word with a space. The word-form figures this write-up first gave,
25.3% (unigram) and 26.6% (bigram) with 3 shown, assume the generous user,
and the finding first drawn from them rested on that unequal comparison.
With the measure's own user the same baselines save 20.9% and 22.4%. Each
word-form figure below says which user it assumes: prefix picks for the
recording rule, whole-word picks for the measure's user.
`sme_eval.py baselines --picks whole` scores the baselines with the
measure's user.

Versions measured: hfst de85176 (the harness and the tools), lang-sme's
analyser and generator `analyser-gt-norm.hfstol` and
`generator-gt-norm.hfstol` as built on 2026-08-24, corpus-sme d55a4a8c6.
The word forms the model counts (section *Word forms in the model*) were
measured with hfst 0f5590d.

## Data

The text comes from the open corpus only, `corpus-sme/converted`, from
eight genre directories: admin, bible, blogs, facta, ficti, laws, news and
science. `scripts/prediction/sme_corpus.py extract` lists each genre's
documents, puts them in a random order with seed 1, and takes documents
until the genre has 7,500 words, at most 1,500 from one document. It keeps
the body's paragraphs with no `type` (running text, not titles, list items
or table cells) that are not marked as another language, and leaves out
any element inside them that is. ficti and news have fewer words than
that. The hist directory was left out: its documents are mostly
newspapers from 1906 to 1912 in the old orthography, which the analyser
does not read and nobody types today. The chosen documents are then
shuffled across genres, so each of the harness's contiguous folds holds
some of every genre.

| genre | documents | words |
|---|---:|---:|
| admin | 35 | 7,508 |
| bible | 53 | 7,513 |
| blogs | 13 | 7,508 |
| facta | 12 | 7,517 |
| ficti | 16 | 2,955 |
| laws | 6 | 7,566 |
| news | 4 | 3,368 |
| science | 7 | 7,962 |
| all | 146 | 51,897 |

The text is cut into tokens simply: a word is a run of letters and digits,
which may hold a hyphen, an apostrophe or a colon between two of them
(`2010-logu`, `NRK:s`) and may end with a hyphen (`álbmot-` in
`álbmot- ja servodat`). Any other character that is not a space is a token
of its own. This gives 61,753 tokens, of which 51,954 are words the measure
types (353,579 characters) and the rest punctuation. lang-sme has no built
tokeniser to use instead.

`sme_corpus.py analyse` analyses each distinct form with `hfst lookup` on
`analyser-gt-norm.hfstol` and writes hfst-lookup format. A reading the
analyser gives twice, by two paths, is kept once. A form it does not know
is looked up again in small letters (34 tokens). 1,980 word tokens have no
analysis and are typed in full by unit prediction.

Each token's target, the reading the simulated user builds the word from,
is its first reading that the generator accepts and the grammar cuts into
units. The generator accepts all 48,047 distinct readings. Words by units
in their target:

| units | 0 (no reading) | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| word tokens | 1,980 | 44,845 | 3,906 | 898 | 258 | 40 | 13 | 14 |
| characters | 17,764 | 279,255 | 38,093 | 12,752 | 4,276 | 782 | 283 | 374 |

Punctuation tokens have readings too (`.+CLB`), so they stay in the
context the model ranks by, but they are not typed.

### Ambiguity

There was no constraint-grammar disambiguation. 74.0% of the analysed word
tokens have more than one reading, 3.5 on average, and 31.6% have readings
with more than one lemma. Two rules for counting them were run:

- first reading: the model counts only each token's first reading, the
  first one hfst-lookup writes. For `girjjiid` that is `girji+N+Pl+Acc`,
  though `+Pl+Gen` and the lemma `girje` are also there.
- equal shares: the model counts every reading, each at a share of one,
  so the plausible and the implausible count the same.

The rule changes only what the model counts. The target the user types is
the first reading either way.

lang-sme's disambiguator (`src/cg3/disambiguator.cg3`) is not compiled in
the checkout, and no vislcg3 binary is installed. Running it would also
need the hfst-lookup output converted to CG's stream format and back. That
was not done, so there is no disambiguated run.

## The setup

The harness splits the token list into k = 5 contiguous parts, part p from
token p·n/5 to (p+1)·n/5, and scores each part with a model built from the
other four. `scripts/prediction/sme_eval.py` splits the text the same way
for the baselines and for the groups below.

Two things the harness does not do were done around it.

**Scoring some tokens only.** `sme_eval.py mask` writes a copy of the text
in which every token not to be scored keeps its readings but has its
surface form replaced by `·`. The model builder reads only the readings,
so every model is built exactly as from the plain text. The harness types
only tokens with a character it counts, and `·` is punctuation, so those
tokens are not typed, but their units stay in the context. This scores a
group of words (the gradation and novelty groups below), a random sample
of words, or a single word, with the same models and contexts as the whole
text.

**Baselines built per fold.** A recording is read as one list of
suggestions for each word and prefix. Two lines for one word and prefix
must agree (see the open problems), so a baseline that differs between
folds, or with the previous word, cannot be one recording. The baselines
are therefore scored by `sme_eval.py baselines`, which follows
`Recording::type_word`. To check it, `sme_eval.py recordings` writes, for
each fold, an input with the fold's training text masked as above and its
scored words without readings, and a recording of that fold's unigram
suggestions. Scored with `hfst predict-eval --recording`, all five folds
give exactly the script's words, characters and keystrokes saved, with 3
and with 4 shown. The script's count of units in each word also gives
exactly the harness's words and characters for every number of units.

### Reproducing

```sh
HFST=$PWD/target/release/hfst      # cargo build --release -p hfst-cli
SME=~/git/divvun/lang-sme/src/fst  # analyser and generator as built 2026-08-24
G=$SME/generator-gt-norm.hfstol
U=crates/hfst/tests/fixtures/prediction/sme.toml
W=/tmp/sme-eval                    # any work directory
mkdir -p $W/out

# The text and its analyses.
python3 -I scripts/prediction/sme_corpus.py extract \
    --corpus ~/git/divvun/corpus-sme/converted \
    --genres admin,bible,blogs,facta,ficti,laws,news,science \
    --words-per-genre 7500 --max-doc-words 1500 --seed 1 \
    --tokens $W/tokens.txt --manifest $W/manifest.tsv
python3 -I scripts/prediction/sme_corpus.py analyse --hfst $HFST \
    --analyser $SME/analyser-gt-norm.hfstol \
    --tokens $W/tokens.txt --out $W/analysed.txt
python3 -I scripts/prediction/sme_eval.py targets --analysed $W/analysed.txt \
    --hfst $HFST --generator $G --out $W/targets.tsv

# Inputs: the whole text and a 10% sample, each also cut into groups.
for R in 1.0 0.1; do for S in none gradation novelty; do
    python3 -I scripts/prediction/sme_eval.py mask --analysed $W/analysed.txt \
        --targets $W/targets.tsv --folds 5 --sample $R --seed 1 --split $S \
        --out-dir $W/in$R
done; done

# Unit prediction.
p() { $HFST predict-eval -g $G -G $U --folds 5 "$@"; }
p --order 3 --backoff stupid-backoff -n 3,4 --whole-words 0,1,2 \
    $W/in1.0/none-all.txt
p --order 1,2,3 --backoff drop-oldest,stupid-backoff \
    --ambiguity first-reading,equal-shares -n 3,4 $W/in0.1/none-all.txt
p --order 3 --backoff stupid-backoff -n 3,4 --whole-words 0,1 \
    --faces 8+4 $W/in0.1/none-all.txt
for F in gradation-lemma-changed gradation-lemma-written \
    novelty-form-seen novelty-lemma-seen novelty-both-unseen; do
    p --order 3 --backoff stupid-backoff -n 3,4 --whole-words 0,1,2 \
        $W/in1.0/$F.txt
done

# The baselines, overall and by group.
for S in none gradation novelty; do
    python3 -I scripts/prediction/sme_eval.py baselines \
        --analysed $W/analysed.txt --targets $W/targets.tsv \
        --folds 5 --sample 1.0 --seed 1 --shown 3,4 --split $S
done

# The check of the baselines' scoring against the harness, fold by fold.
for N in 3 4; do
    python3 -I scripts/prediction/sme_eval.py recordings \
        --analysed $W/analysed.txt --folds 5 --shown $N --out-dir $W/check$N
    for F in 0 1 2 3 4; do
        # The unit variant is there only because the tool needs one.
        $HFST predict-eval -g $G -G $U --folds 2 --order 1 -n 3 \
            --recording unigram-n$N=$W/check$N/fold$F.tsv $W/check$N/fold$F.txt
    done
done

# Single words, by their token index in analysed.txt.
python3 -I scripts/prediction/sme_eval.py mask --analysed $W/analysed.txt \
    --targets $W/targets.tsv --split token \
    --tokens 30893,30968,31367,32500,32667 --out-dir $W/trace
for F in $W/trace/*.txt; do
    p --order 3 --backoff stupid-backoff -n 3,4 --whole-words 0,1,2 $F
done

# Model sizes.
for A in first-reading equal-shares; do for O in 1 2 3; do
    $HFST predict-model -g $G -G $U --order $O --ambiguity $A \
        --face-size 12 -o $W/out/o$O-$A.pred $W/analysed.txt
done; done
```

The `none` split of the whole text is the analysed text itself. A group's
input differs only in which surfaces are masked. The scripts' own logic is
tested by `python3 -I scripts/prediction/test_sme_scripts.py`.

## Results

### The main variants, on the whole text

All 51,954 words of the text (353,579 characters), each part scored by a
model of order 3 with stupid backoff and the first reading, built from the
other four parts. K is the number of complete words each stem on offer
brings (0: units only).

| system | 3 shown | 4 shown |
|---|---:|---:|
| units, K = 0 | 17.3% | 21.8% |
| units, K = 1 | 19.5% | 24.1% |
| units, K = 2 | 18.8% | 23.6% |
| word forms, unigram, prefix picks | 25.3% | 27.8% |
| word forms, bigram, prefix picks | 26.6% | 29.0% |
| word forms, unigram, whole-word picks | 20.9% | 23.1% |
| word forms, bigram, whole-word picks | 22.4% | 24.6% |

Only the last two rows are scored by the user that scores units. Section
*Word forms in the model* adds the forms the model itself counts, alone and
beside units.

Keystrokes saved by the number of units in the word, 3 shown, with the
baselines' prefix picks:

| units | words | chars | K = 0 | K = 1 | K = 2 | unigram | bigram |
|---|---:|---:|---:|---:|---:|---:|---:|
| all | 51954 | 353579 | 17.3% | 19.5% | 18.8% | 25.3% | 26.6% |
| 2 | 44845 | 279255 | 17.7% | 21.2% | 20.6% | 27.0% | 28.3% |
| 3 | 3906 | 38093 | 26.4% | 22.3% | 20.1% | 27.5% | 28.8% |
| 4 | 898 | 12752 | 10.6% | 9.0% | 8.4% | 13.7% | 14.0% |
| 5 | 258 | 4276 | 3.5% | 2.4% | 2.3% | 5.6% | 5.5% |
| 6 | 40 | 782 | 4.6% | 3.1% | 2.9% | 7.3% | 6.9% |
| 7 | 13 | 283 | 0.0% | 0.0% | 0.0% | 6.7% | 6.7% |
| 8 | 14 | 374 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| no reading | 1980 | 17764 | 0.0% | 0.0% | 0.0% | 8.9% | 9.5% |

4 shown, prefix picks:

| units | words | chars | K = 0 | K = 1 | K = 2 | unigram | bigram |
|---|---:|---:|---:|---:|---:|---:|---:|
| all | 51954 | 353579 | 21.8% | 24.1% | 23.6% | 27.8% | 29.0% |
| 2 | 44845 | 279255 | 22.4% | 26.0% | 25.7% | 29.7% | 30.9% |
| 3 | 3906 | 38093 | 32.5% | 28.4% | 26.3% | 30.2% | 31.2% |
| 4 | 898 | 12752 | 13.1% | 11.5% | 10.7% | 15.0% | 15.1% |
| 5 | 258 | 4276 | 9.0% | 7.2% | 5.3% | 6.7% | 6.2% |
| 6 | 40 | 782 | 9.3% | 9.0% | 8.1% | 9.7% | 8.7% |
| 7 | 13 | 283 | 0.0% | 0.0% | 0.0% | 7.1% | 7.1% |
| 8 | 14 | 374 | 0.0% | 0.0% | 0.0% | 0.3% | 0.3% |
| no reading | 1980 | 17764 | 0.0% | 0.0% | 0.0% | 9.6% | 10.4% |

Unit prediction's top-1 and top-N rates. `first` and `shown` are how often
the word's next unit was the first suggestion, or among those shown, when
the user looked for it (a stem after its first letter). `full` counts
words typed in full.

3 shown:

| units | K = 0 first | shown | full | K = 1 first | shown | full | K = 2 first | shown | full |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 41.6% | 58.1% | 4042 | 23.9% | 43.8% | 4969 | 22.8% | 38.9% | 5290 |
| 2 | 43.2% | 59.3% | 1042 | 22.5% | 42.8% | 1823 | 21.0% | 36.9% | 2110 |
| 3 | 36.5% | 56.2% | 214 | 34.6% | 53.4% | 345 | 34.2% | 52.7% | 369 |
| 4 | 20.2% | 35.7% | 549 | 18.4% | 33.9% | 560 | 18.0% | 32.8% | 569 |
| 5 | 11.5% | 25.9% | 197 | 9.6% | 24.1% | 200 | 9.2% | 24.0% | 201 |

4 shown:

| units | K = 0 first | shown | full | K = 1 first | shown | full | K = 2 first | shown | full |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 41.6% | 62.2% | 3636 | 22.7% | 48.1% | 4330 | 22.9% | 44.5% | 4662 |
| 2 | 43.2% | 63.3% | 756 | 21.0% | 47.0% | 1309 | 21.2% | 42.6% | 1603 |
| 3 | 36.5% | 61.4% | 171 | 34.7% | 58.5% | 297 | 34.2% | 57.6% | 326 |
| 4 | 20.4% | 40.4% | 511 | 18.3% | 37.6% | 524 | 18.1% | 36.5% | 531 |
| 5 | 12.0% | 34.6% | 167 | 10.0% | 31.2% | 169 | 10.2% | 31.2% | 170 |

Words of 6 to 8 units (67 in all) are left out of these two tables. The
1,980 words with no reading are typed in full in every variant.

A complete word that writes the user's word counts as the unit looked
for. But a word picked as one complete word is looked for once, at its
stem, after its first letter. Without whole words it is looked for twice,
and the second look, for the ending, often finds it first. So with K above
0 the rates fall although taps fall. They count looks, not words.

The word-form baselines' rates, judged per word: how often the word itself
was the first suggestion, or among those shown, before its first letter
and after it.

| baseline | first, before | shown, before | first, after one letter | shown, after one letter |
|---|---:|---:|---:|---:|
| unigram, 3 shown | 4.6% | 8.6% | 15.3% | 24.4% |
| unigram, 4 shown | 4.6% | 9.7% | 15.3% | 26.5% |
| bigram, 3 shown | 6.5% | 11.2% | 18.5% | 27.4% |
| bigram, 4 shown | 6.5% | 12.6% | 18.5% | 29.7% |

These count words, where unit prediction's rates count units, so the two
are not comparable.

The run took 42 minutes (2,529 s) on 12 threads of a shared 18-core
machine, with 21,070 CPU seconds, about 0.068 CPU seconds per word for each
variant, and 3.6 GB at most in memory. The baselines take 12 s and 190 MB.

### Order, backoff and ambiguity, on a 10% sample

The full grid of 24 variants was run on a sample, to keep the time down:
5,208 words (35,434 characters) drawn at random with seed 1, 1,013 to
1,074 in each part. Every model is still built from the whole of the other
four parts, and the sampled words keep their context (see the setup). All
with K = 0. Whole-text figures for the same variant run half a point to a point
higher (o3 stupid backoff: 17.3% and 21.8% on the whole text, 16.8% and
21.4% here), and the baselines likewise (with prefix picks unigram 25.3%
and 27.8%, here 24.5% and 27.0%; bigram 26.6% and 29.0%, here 25.7% and
28.1%; with whole-word picks the sample's unigram saves 20.3% and 22.6%,
its bigram 21.7% and 23.9%).

Keystrokes saved, all words:

| order | backoff | first reading, 3 | first reading, 4 | equal shares, 3 | equal shares, 4 |
|---|---|---:|---:|---:|---:|
| 1 | drop-oldest | 12.9% | 17.7% | 8.6% | 13.5% |
| 1 | stupid | 12.9% | 17.7% | 8.6% | 13.5% |
| 2 | drop-oldest | 12.9% | 16.4% | 8.7% | 13.1% |
| 2 | stupid | 16.1% | 20.9% | 10.5% | 15.8% |
| 3 | drop-oldest | 10.9% | 13.4% | 8.0% | 11.1% |
| 3 | stupid | 16.8% | 21.4% | 11.6% | 16.3% |

By units in the word, first reading, 3 shown:

| units | words | o1 | o2 drop | o2 stupid | o3 drop | o3 stupid |
|---|---:|---:|---:|---:|---:|---:|
| all | 5208 | 12.9% | 12.9% | 16.1% | 10.9% | 16.8% |
| 2 | 4503 | 14.5% | 13.1% | 16.7% | 11.1% | 17.3% |
| 3 | 383 | 11.9% | 21.0% | 24.6% | 17.2% | 25.8% |
| 4 | 85 | 4.9% | 7.4% | 7.9% | 9.3% | 10.4% |
| 5 | 35 | 0.0% | 1.5% | 2.6% | -0.2% | 3.2% |

By units in the word, o3 stupid backoff:

| units | words | first reading, 3 | equal shares, 3 | first reading, 4 | equal shares, 4 |
|---|---:|---:|---:|---:|---:|
| all | 5208 | 16.8% | 11.6% | 21.4% | 16.3% |
| 2 | 4503 | 17.3% | 11.6% | 22.1% | 16.4% |
| 3 | 383 | 25.8% | 18.6% | 31.6% | 23.7% |
| 4 | 85 | 10.4% | 9.1% | 11.8% | 15.8% |
| 5 | 35 | 3.2% | 10.5% | 7.4% | 15.5% |

`first` and `shown` for o3 stupid backoff are 41.4% and 57.8% with the
first reading and 3 shown, 34.8% and 49.0% with equal shares. Order 1
gets 34.0% and 50.4%.

At order 1 the two backoffs are the same model, and they score the same to
the tenth. Stupid backoff beats drop-oldest at orders 2 and 3, and
drop-oldest gets worse as the order grows: once a long context has been
seen at all, it ranks by that context alone. Order 3 with stupid backoff is
best, 0.5 to 0.7 points ahead of order 2. Equal shares costs about 5 points
overall. It does better on words of four and five units, mostly compounds
and derivations, but there are few of them.

The grid took 7 minutes (431 s) with 3,132 CPU seconds and 3.0 GB.

### Keyboard faces, on the same sample

With faces on, each class (noun, verb, adjective) has a face of its 12
most frequent unit keys: 8 take one tap and 4 a long press (two taps). A
key on another class's face costs one more tap for the switch. Order 3,
stupid backoff, first reading.

| | 3 shown | 3 shown, faces 8+4 | 4 shown | 4 shown, faces 8+4 |
|---|---:|---:|---:|---:|
| K = 0 | 16.8% | 19.4% | 21.4% | 22.8% |
| K = 1 | 18.9% | 21.2% | 23.6% | 24.8% |
| unigram (no faces), prefix picks | 24.5% | | 27.0% | |
| bigram (no faces), prefix picks | 25.7% | | 28.1% | |
| unigram (no faces), whole-word picks | 20.3% | | 22.6% | |
| bigram (no faces), whole-word picks | 21.7% | | 23.9% | |

By units, faces 8+4:

| units | words | K = 0, 3 | K = 1, 3 | K = 0, 4 | K = 1, 4 |
|---|---:|---:|---:|---:|---:|
| all | 5208 | 19.4% | 21.2% | 22.8% | 24.8% |
| 2 | 4503 | 19.6% | 22.7% | 23.2% | 26.6% |
| 3 | 383 | 29.8% | 25.5% | 33.7% | 29.3% |
| 4 | 85 | 14.3% | 11.2% | 17.0% | 12.6% |
| 5 | 35 | 11.9% | 8.9% | 16.0% | 13.1% |

Faces add 1.2 to 2.6 points, most where fewer suggestions are shown, and
most on long words, where an ending that is not among the suggestions can
still be on the face. The run took 4 minutes (222 s), 2,022 CPU seconds
and 3.0 GB.

### Single words

Five words traced alone, each with every model built as for the whole
text and its own context (`mask --split token`). Taps are what the
simulated user spends; the word's characters are its length here.

| word | target | chars | K = 0, 3 | K = 1, 3 | K = 2, 3 | K = 0, 4 | K = 1, 4 |
|---|---|---:|---:|---:|---:|---:|---:|
| `ja` | `ja+CC` | 2 | 3 | 2 | 2 | 3 | 2 |
| `lea` | `leat+V+IV+Ind+Prs+Sg3` | 3 | 3 | 2 | 2 | 3 | 2 |
| `mánáid` | `mánná+N+Pl+Acc` | 6 | 5 | 5 | 7 | 5 | 5 |
| `girjjiid` | `girji+N+Pl+Acc` | 8 | 7 | 9 | 9 | 6 | 8 |
| `Sámedikki` | `sámediggi+N+Sg+Acc` | 9 | 5 | 7 | 6 | 5 | 7 |

Without whole words, `ja` costs a letter, a pick for the stem and a pick
for the ending: three taps for two letters, though both units were the
first suggestion. With K = 1 the word `ja` is offered after `j`, for two
taps. `girjjiid` and `Sámedikki` cost two taps more with K = 1. The harness
does not log the steps, but this fits how whole words are ranked: a stem
offered beside its own words keeps only the score they leave, so complete
words take the places and the stem shows later.

## Gradation and pooling

These groups were scored on the whole text, each in a run of its own with
the other words masked, so every word keeps the model and context it had
in the main run. Order 3, stupid backoff, first reading.

### Words whose stem changes

A word is in the group `lemma changed` when its lemma, the letters of its
target's first stem, is not where the word starts (capitals aside):
`girjjiid` from `girji`, `lea` from `leat`, `mánáid` from `mánná`. Here the
unit user types the lemma's letters and the generator writes the form, so
gradation and other stem changes cost nothing. A word-form user types the
form's own letters. Words whose lemma starts them, such as `ja`, `ahte` and
`girji`, are `lemma written`. The 1,980 words with no reading are in
neither.

| group | words | chars | chars per word |
|---|---:|---:|---:|
| lemma changed | 24,719 | 190,951 | 7.7 |
| lemma written | 25,255 | 144,864 | 5.7 |

Keystrokes saved:

| group | units | K = 0, 3 | K = 1, 3 | unigram, 3 | bigram, 3 | K = 0, 4 | K = 1, 4 | unigram, 4 | bigram, 4 |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| lemma changed | all | 20.7% | 19.4% | 25.1% | 26.3% | 26.5% | 25.2% | 27.7% | 28.8% |
| | 2 | 19.8% | 19.2% | 25.4% | 26.6% | 25.5% | 24.9% | 28.1% | 29.2% |
| | 3 | 26.8% | 22.5% | 27.1% | 28.4% | 33.1% | 28.9% | 29.7% | 30.8% |
| | 4 | 18.8% | 16.4% | 20.0% | 20.2% | 22.6% | 20.2% | 21.5% | 21.6% |
| | 5 | 4.5% | 2.8% | 5.4% | 5.1% | 12.3% | 9.4% | 6.7% | 6.0% |
| lemma written | all | 14.8% | 22.1% | 27.5% | 29.0% | 18.2% | 25.7% | 30.1% | 31.5% |
| | 2 | 15.4% | 23.4% | 28.7% | 30.3% | 19.0% | 27.2% | 31.4% | 32.8% |
| | 3 | 22.6% | 20.2% | 32.5% | 33.2% | 25.6% | 23.0% | 35.6% | 36.1% |
| | 4 | 2.9% | 2.1% | 7.8% | 8.0% | 4.1% | 3.2% | 8.8% | 9.0% |
| | 5 | 1.7% | 1.5% | 5.9% | 6.1% | 2.9% | 3.1% | 5.9% | 6.6% |

K = 2 (not shown) is 1.1 to 1.3 points below K = 1 on changed stems
(18.1% and 24.1%), and within 0.2 points of it on written ones (22.0% and
25.8%).

The unigram and bigram columns use prefix picks. With whole-word picks
they save 20.1% and 21.7% with 3 shown and 22.5% and 23.9% with 4 on
changed stems, and 23.9% and 25.5%, 26.2% and 27.8% on written lemmas.

Units do much better, against word forms, where the stem changes. There,
units alone with 4 shown come within 1.2 points of the unigram baseline
with prefix picks, and on the three- to five-unit words of the group they
beat both baselines with 4 shown, by 1.0 to 6.3 points. Against whole-word
picks, units alone lead on changed stems: 20.7% against 20.1% (unigram)
with 3 shown, and 26.5% against 22.5% with 4. Where the lemma is written,
units alone are 12 to 13 points behind with prefix picks, 8 to 11 with
whole-word picks. These are the short, frequent words, where a
stem and an ending cost two picks. The words whose stem changes are also
longer, 7.7 characters against 5.7, so these numbers cannot separate the
gain from gradation from the gain from length. Against prefix picks,
gradation does not carry unit prediction past word forms; against
whole-word picks it does, on these words.

Whole words close more than half of the gap on written lemmas, and cost
1.3 points on changed stems.

The runs took 22 and 19 minutes, 10,737 and 10,151 CPU seconds, and 3.6
and 3.8 GB.

### Words the training text has not shown

Each word is grouped by its own part's training text, the other four
parts: `form seen` when the word form, in small letters, is in it; `only
lemma seen` when the form is not but the word's lemma is the lemma of some
token there; `neither seen` otherwise. A word in the last group still has
an analysis: the generator's lexicon holds it, and the model ranks its stem
as unseen, by the generator's weight.

| group | words | chars | K = 0, 3 | K = 1, 3 | unigram, 3 | bigram, 3 | K = 0, 4 | K = 1, 4 | unigram, 4 | bigram, 4 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| form seen | 37,515 | 207,584 | 22.8% | 28.0% | 38.3% | 40.4% | 26.9% | 32.5% | 41.9% | 43.9% |
| only lemma seen | 6,067 | 60,558 | 12.6% | 8.0% | 8.3% | 8.3% | 19.8% | 15.0% | 9.7% | 9.5% |
| neither seen | 6,392 | 67,673 | 8.9% | 8.7% | 4.9% | 4.8% | 13.7% | 12.8% | 5.6% | 5.3% |
| no reading | 1,980 | 17,764 | 0.0% | 0.0% | 8.9% | 9.5% | 0.0% | 0.0% | 9.6% | 10.4% |

`only lemma seen` by units in the word:

| units | words | K = 0, 3 | K = 1, 3 | unigram, 3 | K = 0, 4 | K = 1, 4 | unigram, 4 |
|---|---:|---:|---:|---:|---:|---:|---:|
| 2 | 4128 | 12.7% | 7.2% | 9.0% | 20.4% | 14.6% | 10.5% |
| 3 | 1173 | 17.7% | 12.7% | 6.8% | 26.1% | 21.6% | 7.9% |
| 4 | 508 | 8.5% | 6.5% | 9.0% | 11.7% | 9.5% | 10.1% |
| 5 | 195 | 6.1% | 4.7% | 6.5% | 12.5% | 10.4% | 7.7% |

The unigram and bigram columns use prefix picks. With whole-word picks,
word forms save nothing on forms the training text never showed (0.0% in
both groups, with both N, unigram and bigram), and 35.2% and 39.0%
(unigram), 37.7% and 41.4% (bigram), on forms it did show.

This is pooling. A word form the training text never showed is completed by
word forms only where another form shares its start, and they save 5 to 10
percent, but only for a user who picks a suggestion that starts the word;
with whole-word picks they save nothing. Units save 12.6% with 3 shown and 19.8% with 4 when the lemma was
seen in another form, and 8.9% and 13.7% when it was not seen at all. On
all 12,459 unseen forms together (128,231 characters), units alone save
10.6% with 3 shown and 16.6% with 4, against 6.5% and 7.5% for the
unigram with prefix picks and nothing with whole-word picks. On forms the
training text did show, word forms win by 10 points or more.

Whole words cost most on unseen forms: 4.6 and 4.8 points where only the
lemma was seen. That fits how they are built: a stem is completed by its
highest-ranked endings, and a form the text never showed is less likely to
have one of them.

The groups add up to the main run. The taps their rounded figures imply
give 17.25%, 19.47%, 21.81% and 24.10% for K = 0 and 1 with 3 and 4 shown
(gradation groups: 17.24%, 19.53%, 21.77%, 24.14%), against the main run's
17.3%, 19.5%, 21.8% and 24.1%. The runs took 30, 8 and 9 minutes,
14,263, 3,124 and 3,595 CPU seconds, and 3.4, 3.0 and 4.0 GB.

## Word forms in the model

Since hfst bf519f5 the model also counts the text's word forms
(`prediction-model.word-forms`), and before a word's first unit the
keyboard offers the counted forms that start with the letters typed, or
with none typed the likeliest after the words before
(`prediction.seen-forms`). A form is scored by its count after the forms of
the words before it; units, and words built from them, by the unit model
times a unit weight from 0 to 1 (`prediction-model.form-score`). Forms,
units and the weight are settings, so the harness scores forms alone, units
alone and both on the same parts, with its own user throughout: it picks a
form only when it is the whole word. With forms on, the user types the
word's own letters until it picks a unit, each in the stem's case while the
two agree, so entry still matches the stem. A pick before the first letter
costs one tap, and the word is then judged at that look.

### Checked against the baselines

With units off and order 1 the harness completes the forms its model
counted, which is the unigram baseline. The two agree to the tap once the
baseline is scored the same way. Whole text, keystrokes saved:

| unigram | 3 shown | 4 shown |
|---|---:|---:|
| `sme_eval.py`, prefix picks | 25.29% | 27.81% |
| whole-word picks | 20.89% | 23.12% |
| and forms counted only for tokens with a reading the model counts, punctuation counted but not offered | 20.76% | 23.01% |
| harness, units off, order 1 | 20.76% | 23.01% |

The harness and the baseline row above it both spend 280,164 taps with 3
shown and 272,238 with 4. That row was scored by a variant of
`sme_eval.py`'s baselines that is not kept; `--picks whole` gives the row
before it. The step between the two costs 0.13 points: the 1,980 words
with no reading give the model no form, as `prediction-model.word-forms`
says;
punctuation tokens do have readings, so they are counted as context but
never offered as words. At order 2 the harness ranks by stupid backoff,
where the bigram baseline puts every form seen after the previous token
before the rest: the bigram rule saves 22.11% and 24.29%, stupid backoff
22.24% and 24.40%, the same as the harness to the tap (274,954 and 267,304
taps). On the 10% sample the steps agree to the tap as well. Picks of a
suggestion that only starts the word are 4.4 points of the published 25.3%.

### The results

All 51,954 words, order 3, stupid backoff, the first reading, the same five
parts. `units` offers units alone, `forms` the counted forms alone, `both`
both in one list; K is the number of complete words each stem brings and w
the unit weight.

3 shown:

| units | words | chars | units, K = 0 | units, K = 1 | forms | both, K = 0, w 1 | both, K = 0, w 0.1 | both, K = 1, w 1 | both, K = 1, w 0.1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 51954 | 353579 | 17.3% | 19.5% | 22.3% | 22.7% | 22.9% | 23.6% | 23.4% |
| 2 | 44845 | 279255 | 17.7% | 21.2% | 24.5% | 24.3% | 25.0% | 25.6% | 25.6% |
| 3 | 3906 | 38093 | 26.4% | 22.3% | 24.7% | 28.3% | 26.2% | 27.5% | 26.2% |
| 4 | 898 | 12752 | 10.6% | 9.0% | 7.2% | 10.5% | 8.3% | 10.1% | 8.8% |
| 5 | 258 | 4276 | 3.5% | 2.4% | 0.0% | 3.9% | 1.9% | 2.9% | 1.2% |
| 6 | 40 | 782 | 4.6% | 3.1% | 0.0% | 3.6% | 2.9% | 2.9% | 2.2% |
| 7 | 13 | 283 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| 8 | 14 | 374 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| no reading | 1980 | 17764 | 0.0% | 0.0% | 0.4% | 0.4% | 0.4% | 0.4% | 0.4% |

4 shown:

| units | words | chars | units, K = 0 | units, K = 1 | forms | both, K = 0, w 1 | both, K = 0, w 0.1 | both, K = 1, w 1 | both, K = 1, w 0.1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| all | 51954 | 353579 | 21.8% | 24.1% | 24.4% | 27.3% | 27.1% | 27.9% | 27.3% |
| 2 | 44845 | 279255 | 22.4% | 26.0% | 26.9% | 29.2% | 29.6% | 30.2% | 29.8% |
| 3 | 3906 | 38093 | 32.5% | 28.4% | 26.8% | 34.1% | 30.9% | 32.9% | 30.6% |
| 4 | 898 | 12752 | 13.1% | 11.5% | 7.9% | 12.7% | 9.7% | 12.1% | 9.9% |
| 5 | 258 | 4276 | 9.0% | 7.2% | 0.0% | 8.2% | 5.1% | 6.8% | 4.0% |
| 6 | 40 | 782 | 9.3% | 9.0% | 0.0% | 8.2% | 5.1% | 7.9% | 4.7% |
| 7 | 13 | 283 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| 8 | 14 | 374 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| no reading | 1980 | 17764 | 0.0% | 0.0% | 0.4% | 0.4% | 0.4% | 0.4% | 0.4% |

Units alone give the earlier runs' figures to the tenth. Forms alone save
22.3% and 24.4%, at order 3; at order 1 they save 20.8% and 23.0%, and at
order 2 22.2% and 24.4%.

Reading:

- With 3 shown, forms alone lead units with one complete word per stem by
  2.8 points (22.3% against 19.5%), not the 5.8 points the prefix-pick
  baseline gave. With 4 shown the two are level (24.4% and 24.1%).
- Both together beat either alone. The best is K = 1 at weight 1: 23.6%
  with 3 shown and 27.9% with 4, 1.3 and 3.5 points over forms alone. Weight
  0.1 is 0.2 points behind with 3 shown and 0.6 with 4.
- Forms carry the two-unit words, the short frequent ones. Units carry the
  longer ones: on words of four units and more, forms alone save 0 to 8%,
  units 2 to 13%, and both at weight 1 keep most of what units save there.
  At weight 0.1 units rank below the forms and lose those places.
- On the forms the training text never showed, forms alone save nothing
  (the novelty groups above), and units save 8.0% to 19.8%.
- The 1,980 words with no reading gain 0.4% from forms another token
  showed in another case.

The run of all 14 variants took 26 minutes (1,557 s), 18,379 CPU seconds
and 7.0 GB. With units on, 27,390 of the 51,954 words (24,700 with both)
met the walk's bound of 1,000,000 configurations in some walk; an entry
walk after one or two letters seldom finishes within it.

The groups of the previous sections were not run with forms. They are
scored by masking the other words, and masking replaces the surfaces that
the form counts read, so a masked run would count `·` and almost no real
form.

```sh
$HFST predict-eval -g $G -G $U --folds 5 --order 3 --backoff stupid-backoff \
    -n 3,4 --whole-words 0,1 --forms off,on --units on,off --unit-weight 1,0.1 \
    $W/analysed.txt
for S in none gradation novelty; do
    python3 -I scripts/prediction/sme_eval.py baselines \
        --analysed $W/analysed.txt --targets $W/targets.tsv \
        --folds 5 --sample 1.0 --seed 1 --shown 3,4 --split $S --picks whole
done
```

## Time, memory and model size

The machine had 18 cores and was shared with other work (load 20 to
50 throughout), so wall times vary. Each run used 12 threads.

| run | words typed | variants | wall | CPU s | CPU s per word and variant | memory |
|---|---:|---:|---:|---:|---:|---:|
| main, whole text | 51,954 | 6 | 42 min | 21,070 | 0.068 | 3.6 GB |
| grid, sample | 5,208 | 24 | 7 min | 3,132 | 0.025 | 3.0 GB |
| faces, sample | 5,208 | 4 | 4 min | 2,022 | 0.097 | 3.0 GB |
| lemma changed | 24,719 | 6 | 22 min | 10,737 | 0.072 | 3.6 GB |
| lemma written | 25,255 | 6 | 19 min | 10,151 | 0.067 | 3.8 GB |
| form seen | 37,515 | 6 | 30 min | 14,263 | 0.063 | 3.4 GB |
| only lemma seen | 6,067 | 6 | 8 min | 3,124 | 0.086 | 3.0 GB |
| neither seen | 6,392 | 6 | 9 min | 3,595 | 0.094 | 4.0 GB |
| forms and units, whole text | 51,954 | 14 | 26 min | 18,379 | 0.025 | 7.0 GB |
| baselines (script) | 51,954 | 4 | 12 s | 8 | | 0.2 GB |

The grid has no whole words; runs with K above 0 cost more per word, since
each stem on offer is completed by its own walk. The forms-and-units run
used hfst 0f5590d, whose walk keeps what it learns of the generator between
calls, with 18 threads. Preparing the text takes
under half a minute: extraction under 1 s, analysis 3 to 17 s, targets 2
to 5 s.

Model sizes, built from the whole text with faces of 12 keys for each class
by `hfst predict-model`, each in under 3 s and 150 MB. The first two
columns are de85176's models, of units only; the last two are 0f5590d's,
which also hold the word forms and their runs:

| order | first reading | equal shares | with forms, first reading | with forms, equal shares |
|---|---:|---:|---:|---:|
| 1 | 166,694 bytes | 219,913 bytes | 655,685 bytes | 720,616 bytes |
| 2 | 592,414 bytes | 1,607,805 bytes | 1,574,049 bytes | 2,601,152 bytes |
| 3 | 1,686,710 bytes | 7,628,661 bytes | 3,489,553 bytes | 9,443,216 bytes |

The text has 14,745 distinct word forms; the model counts 12,274, with
capitals folded and only from tokens with a reading it counts.

## The finding

On 51,954 words of open North Sámi text, scored throughout by the user the
measure defines, who picks a suggestion only when it is the word's next
unit or the whole word:

- Completing the word forms the training text showed saves 22.3% of
  keystrokes with 3 suggestions shown and 24.4% with 4 (order 3; the
  unigram baseline saves 20.9% and 23.1%). Units with one complete word per
  stem save 19.5% and 24.1%. Forms lead by 2.8 points with 3 shown and the
  two are level with 4.
- Forms and units together, in one list, save 23.6% and 27.9% (one
  complete word per stem, unit weight 1), more than either alone.
- Units win where the training text never showed the form, where forms
  alone save nothing, and on words of four units and more. Forms win on the
  short frequent words of two units.

The finding first written here, that unit prediction saves 5.8 points
fewer than completing word forms with 3 shown and is not worth shipping on
its own, rested on word-form baselines scored with prefix picks (25.3% and
27.8% for the unigram, 26.6% and 29.0% for the bigram). Those assume a
more generous user than the one that scored units. With the same user the
gap with 3 shown is 2.8 points, and with 4 shown there is none.

Whole words did what they were meant to for short words and no more. They
add 3.5 points on two-unit words and 7.3 to 7.5 points on words whose lemma
is written, the short frequent words such as `ja` and `lea`. They cost about
4 points on three-unit words, 1.3 points on words whose stem changes, and
4.6 to 4.8 points on forms the training text never showed but whose lemma
it did. Net, they add 2.2 points with 3 shown and 2.3 with 4. K = 2 is
worse than K = 1. Beside forms they still add 0.9 points with 3 shown and
0.6 with 4 at weight 1.

What to ship for North Sámi, as measured: forms and units together, one
complete word per stem, unit weight 1, order 3 with stupid backoff. With 4
suggestions shown the combination gains most (27.9% against 24.4% for
forms alone and 24.1% for units); with 3 shown it gains 1.3 points over
forms alone. This holds for one text of 52,000 words and the simulated
user below; weight 1 and 0.1 are within 0.6 points of each other, so the
weight is not settled by this text.

The limits:

- One text of 52,000 words from eight genres, 41,500 words of training text
  per part. A word list from a larger text would have seen more forms, which
  shrinks the group where units win. A unit model would also gain.
- No constraint-grammar disambiguation. 74% of the analysed words have
  more than one reading, and the user's target and the model's counts take
  the first. Counting all readings at equal shares cost about 5 points.
- The simulated user picks every unit it is offered, even where typing
  would be cheaper, and never mistypes. `ja` costs three taps with units
  alone.
- Keystrokes saved is the only measure. Whether a North Sámi writer would
  rather pick a lemma and an ending than a word is a question for writers.
- The groups (gradation, novelty) were not scored with forms on, so how the
  combination does on changed stems and unseen forms is inferred from the
  totals and from forms and units alone.

## Open problems

- **A recording cannot depend on the fold or the context.** `Recording`
  holds one list of suggestions for each word and prefix, and refuses a
  second, different one. A baseline built per fold, or a bigram, therefore
  cannot be scored in one harness run beside the unit variants. A minimal
  reproduction: the text `ja⇥ja+CC⇥0.000000`, a blank line, the same token
  again, and the recording lines `ja⇥⇥ja` and `ja⇥⇥jas` give
  `recording "forms", line 2: a second line for "ja" after "", with other
  suggestions`. The baselines here are scored by `sme_eval.py`, checked
  fold by fold against the harness.
- **Whole words push the stem down.** A stem offered beside its own words
  keeps only the score they leave, so long words, words with changed stems
  and unseen forms lose 1 to 5 points. On Greenlandic they cost 0.6 to 1.6
  points for the same reason. Keeping the stem visible beside its words was
  not tried.
- **Units offer nothing before the first letter.** Stems are offered only
  for letters typed. With forms on, the likeliest counted forms after the
  words before are offered there.
- **Long words are typed in full.** All 27 words of seven and eight units
  are typed in full in every variant, as are the 1,980 words the analyser
  does not know.
- **The rates do not compare across systems.** The harness counts `first`
  and `shown` per unit looked for. With whole words, a word picked whole is
  looked for once instead of twice, so the rates fall while taps fall too.
  A recording gets no rates. A rate per word would serve both.
- **No per-word output, and no way to score some words with forms on.**
  The harness reports totals only. Groups and single words were scored by
  masking the other words, one run for each, and masking replaces the
  surfaces that the form counts read. Scoring a group or a sample with
  forms on needs a harness setting that types only some tokens.
- **No disambiguated run.** lang-sme's CG disambiguator is not compiled
  here and no vislcg3 was at hand.
