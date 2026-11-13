---
id [dec:hfst:prediction-word-forms]
epitome "The model counts the text's word forms as well as its units. A form the text showed is offered on its letters, or after the words before it, and ranked by its own counts; units, and words built from them, rank below it by a weight, and carry the forms the text never showed."
state @decided
category @existence
scope {
    elements ([arch:hfst:prediction-walk] [arch:hfst:prediction-model])
    rules (
        [spec:hfst:sem:prediction.seen-forms]
        [spec:hfst:req:prediction-model.word-forms]
        [spec:hfst:sem:prediction-model.form-score]
        [spec:hfst:def:prediction-model.file]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-13T00:00:00Z"
edges {
    requires ([dec:hfst:predictive-text] [dec:hfst:prediction-whole-words])
}
alternatives (
    {
        option "Keep units, with whole words built from them, as the only source of words."
        rejected_because "On 51,954 words of open North Sámi text, units with one complete word per stem saved 19.5% of keystrokes with three suggestions, where completing the word forms the training text showed saved 22.3%, and both together 23.6%; with four shown, 24.1%, 24.4% and 27.9%. A form's own count knows which of a stem's forms is common; a stem's score times an ending's score only approximates it. The figures first given here, 25.3% and 26.6% for forms, came from a user who could pick a suggestion that only starts the word, where the user that scored units picks only the whole word."
    }
    {
        option "Offer word forms only, as an ordinary keyboard does."
        rejected_because "A quarter of the North Sámi words were forms the training text never showed. On those, units saved 10.6% with three suggestions, and 19.8% with four where the lemma had been seen, while completing seen forms saved nothing, since a form the text never showed is never offered. Units are also how a Greenlandic or Plains Cree word that no text holds gets built at all."
    }
    {
        option "Ship two predictors, one for forms and one for units, and show both."
        rejected_because "A keyboard shows three or four suggestions; two lists would split them and repeat the same word. Counting forms in the same model file ranks both kinds of candidate in one list, and keeps one file in step with one grammar."
    }
    {
        option "Mix the two scores for every word, with a fixed share for each."
        rejected_because "A seen form already has the better estimate, so mixing in the unit score mostly blurs it. Scoring seen forms by their counts and everything else by units times a weight is simpler, and the weight is measured per language. Mixing remains open if measurement shows a gain."
    }
)
consequences {
    accepted (
        "The model file grows by its word-form tables; the format version goes up."
        "The harness can measure completion of word forms on its own, with units off, on the same folds, so the baselines need no recordings scored outside it."
        "With no letters typed, a word is predicted from the words before it, which units alone could not do."
        "This takes up, in part, the alternative the whole-words decision rejected: a second source of word scores. The North Sámi measurement is the reason."
    )
    deferred (
        "Mixing the two scores for every word."
    )
}
codifies ([spec:hfst:sem:prediction.seen-forms] [spec:hfst:req:prediction-model.word-forms] [spec:hfst:sem:prediction-model.form-score])
establishes ()
---

## Rationale

The North Sámi measurement split the words in two. On forms the training text
had shown, completing the forms themselves beat every unit variant (35.2%
for the unigram of forms against 28.0% for units with one complete word per
stem, three shown). On
forms it had not shown, units won outright: forms save nothing there, units
10.6%, because a stem and an ending the text showed separately can still
make the word. Offered together, forms and units saved 23.6% of keystrokes
with three suggestions and 27.9% with four, against 22.3% and 24.4% for
forms alone and 19.5% and 24.1% for units alone; on that text the unit
weight did best at 1. The first comparison, 25.3% for forms against 19.5%
for units, scored the forms with a user who could pick a suggestion that
only starts the word, which overstated forms by 4.4 points.

Plains Cree and Greenlandic have since been measured the same way. On the
Cree comparison text forms alone saved the most with three suggestions
(36.1%) and units with one complete word per stem with ten (51.5%); on
3,000 Greenlandic tokens units alone saved the most (11.2% and 15.9%) and
forms, on 2,000 tokens of training text, little. Neither gained from the
mix, which is why forms and units are each a setting.

So the model keeps both counts, and the ranking uses each where it is
strong: a seen form by its own count after the words before it, and anything
built from units by the unit model, scaled by a weight each language
measures. The settings that turn forms and units on and off let every
language be measured three ways on the same folds: forms alone, units alone
and both.
