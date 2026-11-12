# The prediction model

A model ranks the units the walk offers. It is built offline from a
language's analysed text and read where prediction runs. It only puts the
walk's units in order: the generator decides which units are offered
(`prediction.generator-decides`).

## The file

> [spec:hfst:def:prediction-model.file]
> A model is one file. It holds the hash of the unit grammar it was built
> with; its order, the length of the longest run of units it counts; its
> backoff method; how its builder counted a token with more than one reading;
> the unit keys it counts, each with its count; for each length from 2 to the
> order, the count of each run of that many units seen in the text; and, for
> each class in the grammar, that class's face (`prediction-model.faces`).

> [spec:hfst:syn:prediction-model.format]
> A model file starts with the eight bytes `HFSTPRED`, a format version and
> the grammar's hash. Every table after this header is an array of
> fixed-size records sorted by key, located by an offset from the start of
> the file, so a reader looks a count up in place by binary search, without
> parsing or copying the file. Integers are little-endian.

> [spec:hfst:req:prediction-model.same-grammar]
> A model MUST record the hash of the unit grammar it was built with. Loading
> a model with a grammar whose hash differs MUST fail with an error that gives
> both hashes.

## Building

> [spec:hfst:req:prediction-model.input]
> The builder MUST read analysed text as hfst-lookup writes it: one block per
> token, each line holding the token's surface form, a tab, one reading in
> the generator's analysis notation, a tab and a weight, and a blank line
> after each block. It MUST cut each reading into units with the unit
> grammar, and it MUST report and leave out a reading the generator does not
> accept, so the model counts only units the walk can offer. A token with
> more than one reading MUST be counted by the rule the build is given:
> either its first reading only, or each reading at an equal share of one.

> [spec:hfst:sem:prediction-model.order]
> A model's order and backoff method are build settings. With `drop-oldest`
> backoff, the candidates are scored by their counts after the longest
> context, ending at the word in progress, after which at least one candidate
> was seen; shorter contexts add nothing to those counts. With
> `stupid-backoff`, a candidate's score is its count after the full context
> divided by the count of the context, or 0.4 times its score after the
> context less its oldest unit when that count is zero, down to its share of
> all units counted.

> [spec:hfst:req:prediction-model.faces]
> For each class in the unit grammar, the builder MUST write into the model
> that class's most frequent unit keys, as many as the build asks for, most
> frequent first. A keyboard face for the class shows these.

> [spec:hfst:req:prediction-model.budget]
> The builder MUST take a size limit in bytes and write a model no larger
> than it, by dropping the least frequent runs of the longest length first,
> then of the next length down. It MUST NOT drop single unit keys. When the
> single keys alone exceed the limit, the build MUST fail and say by how many
> bytes.

## Ranking

> [spec:hfst:req:prediction-model.rank-only]
> The ranker MUST return the candidates it is given and only those, in a new
> order. Candidates the model scores come first, highest first. Candidates
> the model never saw come after them, by the generator's weight, lowest
> first. Remaining ties are broken by the bytes of the unit key. No
> candidate MAY be dropped for being unseen.

> [spec:hfst:sem:prediction-model.word-score]
> A complete word on offer (`prediction.whole-words`) is scored as its stem's
> score times its ending's score after the context and the stem. A stem
> offered beside its own complete words is scored as its own score less the
> sum of theirs, and never below zero, so a stem whose likely endings are all
> on offer as words ranks below them, and one with likely endings left over
> keeps its place. A complete word the model cannot score is unseen.
