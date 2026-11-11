# Predictive text

Prediction offers the next part of a word, not the next letter. A word is
built from units: a stem, then derivations, an ending and enclitics, each
picked from a short list. The language's generator, the transducer from
analyses to words, decides which units can come next, and writes the word
once they are picked. A model built from the language's analysed text ranks
the units (`prediction-model.md`). The language's unit grammar says what a
unit is (`prediction-units.md`).

This is how Oqaasileriffik's Greenlandic prediction works, except that the
generator, not a list of morpheme names, decides what can come next.

## Units and words in progress

> [spec:hfst:def:prediction.unit]
> A unit is one choice the user makes while building a word: a run of one or
> more symbols on the generator's analysis side that the unit grammar treats
> as one piece. A unit is a stem, a prefix, a derivation, an ending, an
> enclitic or a filter, as `prediction-units.md` defines them. A unit's key is
> its symbols with the grammar's hidden tags left out; two units with the same
> key are one choice.

> [spec:hfst:def:prediction.state]
> A word in progress is the sequence of units picked so far, together with
> every configuration it reaches in the generator. A configuration is a state,
> the flag values set on the way to it, the surface symbols written on the way
> to it and the weight of that path. An empty word starts from the
> configurations the generator reaches from its start state without reading
> an analysis symbol.

## What can come next

> [spec:hfst:req:prediction.generator-decides]
> The units offered for a word in progress MUST be exactly the units, as the
> language's unit grammar draws them, that the generator accepts next from the
> word's configurations. No list of morpheme names, endings or enclitics MAY
> add a unit the generator does not accept there, or remove one it does.
> Picking a unit MUST continue every configuration from which the generator
> accepts some unit with that key.

> [spec:hfst:req:prediction.flags]
> The walk MUST apply flag diacritics on the generator's analysis side as
> lookup does, so a unit is offered only where some path that reaches it
> keeps every flag test true, and the flag values a path sets carry into the
> units after it.

> [spec:hfst:req:prediction.bounded]
> A walk MUST end after a bounded number of steps, whatever cycles the
> generator has. The bounds are the number of symbols in one unit and the
> number of configurations visited, and both are settings. When a bound stops
> a walk early, the result MUST say so, and MUST NOT present the units it
> found as the whole set.

> [spec:hfst:sem:prediction.complete]
> A word in progress is complete when one of its configurations is final.
> The walk of a complete word returns, besides the units that can still
> follow it, every surface form its final configurations wrote, each with the
> lowest weight among the paths that wrote it, final weight included.

## Writing the word

> [spec:hfst:req:prediction.realise]
> The word a session commits MUST be a surface form the generator writes for
> the units picked. When the language has an output transliterator, the word
> MUST be passed through it after the generator.

> [spec:hfst:sem:prediction.preview]
> The preview of a word in progress is the surface form of the word completed
> by its highest-ranked ending, or the word's own surface form when it is
> complete. A word with no ending in reach of the walk has no preview.

> [spec:hfst:req:prediction.script-edges]
> A language written in a script its generator does not write, such as
> syllabics for a generator that writes Roman letters, MUST convert only at
> two edges: the typed letters before entry matching, and the committed word
> after realising. Units, the model and the walk MUST see only the
> generator's own spelling.

## Entering and ending a word

> [spec:hfst:sem:prediction.entry]
> Letters typed before the first stem of a word is picked are matched, by
> prefix, against the labels of the units the generator accepts next. When
> the language has a relaxation transducer, a label matches when the typed
> letters match it through that transducer, so letters typed without their
> diacritics still match. The units that match are offered.

> [spec:hfst:sem:prediction.session]
> A session holds a word in progress and the units of the words before it.
> Picking a unit appends it to the word. Backspace removes the last letter
> typed, or the last unit picked when no letter is typed after it. Committing
> a complete word writes it, keeps its units as context and starts an empty
> word. The context keeps as many of the latest units as the model's order
> uses.

## Memory

> [spec:hfst:req:prediction.views]
> The walk MUST read the generator's tables where they are loaded, without
> copying them, so that a host can map the file into memory and pay only for
> the pages a walk touches. This is `table-residency.views-not-copies` applied
> to prediction.
