---
id [dec:hfst:prediction-whole-words]
epitome "While a stem can be picked, prediction offers complete words beside the stems: each stem on offer completed by its highest-ranked endings, scored by the same unit model, so a short frequent word costs one pick and a long or rare one can still be built from units."
state @decided
category @existence
scope {
    elements ([arch:hfst:prediction-walk] [arch:hfst:prediction-model])
    rules (
        [spec:hfst:sem:prediction.whole-words]
        [spec:hfst:sem:prediction-model.word-score]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-12T00:00:00Z"
edges {
    requires ([dec:hfst:predictive-text])
}
alternatives (
    {
        option "Offer only units, as specified first."
        rejected_because "On a 5,208-word sample of open North Sámi text, units saved 16.8% of keystrokes with three suggestions where completing whole word forms saved 24.5%. A short frequent word costs a stem and an ending: ja is three taps for two letters, lea three for three."
    }
    {
        option "Rank whole words with a second model of word forms, beside the unit model."
        rejected_because "Two models would have to be built, shipped and kept in step, and a word-form model knows only the forms its text contains. A stem's score times its ending's score is a word's score in the unit model already, and it pools what the text shows across a stem's forms."
    }
    {
        option "Offer only whole words, and drop units."
        rejected_because "A word the text never showed, or a long derived word, can then only be typed letter by letter. Units are what save keystrokes on long North Sámi forms such as Sámedikki (44% saved) and on Greenlandic's chains of derivations."
    }
)
consequences {
    accepted (
        "An offer can hold complete words as well as units, so the walk, the ranker, the session, the harness and hfst-embed carry a candidate that is two units at once."
        "The North Sámi and Plains Cree write-ups measure units, whole words and both together against completion of word forms."
    )
    deferred (
        "Offering whole words after a derivation, where an ending is not yet in reach."
    )
}
codifies ([spec:hfst:sem:prediction.whole-words] [spec:hfst:sem:prediction-model.word-score])
establishes ()
---

## Rationale

Tino Didriksen's Greenlandic interface already works this way: while a root is
typed it shows ordinary word predictions, and only after the root does it
offer morphemes. The first North Sámi measurement shows why it matters.
Most North Sámi words are a stem and one ending, so a pick for each costs as
much as typing a short word, and completion of whole word forms won by seven
to nine points.

A whole word here is a stem and one of its endings, scored as the product of
their scores in the unit model. A stem shown beside its own words keeps only
the score its other endings leave, so the offer fills with the likeliest words
first and keeps a stem only when composing still has something to give.
