---
id [dec:hfst:prediction-measured-choices]
epitome "A language's prediction settings, the model's order and backoff, its unit boundaries, how many suggestions show and what a keyboard face holds, are chosen by keystrokes saved on the language's own held-out text. What users see and how they start a word are settled with the language's speakers."
state @decided
category @executive
scope {
    elements ([arch:hfst:prediction-evaluation])
    rules (
        [spec:hfst:def:prediction-evaluation.keystrokes]
        [spec:hfst:req:prediction-evaluation.held-out]
        [spec:hfst:req:prediction-evaluation.external]
        [spec:hfst:req:prediction-evaluation.report]
        [spec:hfst:sem:prediction-model.order]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-11T00:00:00Z"
edges {
    requires ([dec:hfst:predictive-text])
}
alternatives (
    {
        option "Use the settings Oqaasileriffik found for Greenlandic in every language, such as trigrams."
        rejected_because "They were found on 20.3 million words of one language. The tagged Plains Cree corpus is about 243,000 words, and an order that suits one need not suit the other."
    }
    {
        option "Judge by the papers' own measures."
        rejected_because "The Greenlandic paper counts keystrokes in morpheme names and leaves endings out. The Cree paper counts how often a useful completion is in the top 10. Neither counts what a user types on a keyboard that shows three suggestions."
    }
    {
        option "Settle labels and how a word is started by measurement as well."
        rejected_because "Keystrokes saved cannot say whether a user understands a morpheme name, or wants to start a Cree word with its person prefix. Neither paper tested with speakers, and the Cree authors warn against wearing out willing community collaborators."
    }
)
consequences {
    accepted (
        "Every language gets the harness's report before its settings are fixed, and other systems, such as lang-crk's morph-completion speller, are scored by the same measure."
        "The settings a language uses are kept in its repository beside its unit grammar, with the report that chose them."
    )
    deferred (
        "Testing with speakers, which needs a keyboard to test with."
    )
}
codifies (
    [spec:hfst:def:prediction-evaluation.keystrokes]
    [spec:hfst:req:prediction-evaluation.held-out]
    [spec:hfst:req:prediction-evaluation.external]
)
establishes ([arch:hfst:prediction-evaluation])
---

## Rationale

The two papers this work builds on measure different things on very
different amounts of text, so their numbers cannot choose settings for a
third language. One measure, applied to each language's own held-out text,
can: keystrokes saved on the words as written, with endings counted and the
number of suggestions a keyboard actually shows.

Some choices are not about keystrokes. Whether a Greenlandic morpheme name
or a generated word is the better label, and whether Cree speakers want to
start a verb with its person prefix, are questions for the people who will
use the keyboard. The harness informs those choices but does not make them.
