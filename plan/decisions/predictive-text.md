---
id [dec:hfst:predictive-text]
epitome "Predict text a unit at a time: the language's generator says which stems, derivations, endings and enclitics can come next, a model built from the language's analysed text ranks those units, and the generator writes the word. This is Oqaasileriffik's Greenlandic method without its lists of morpheme names."
state @decided
category @existence
scope {
    elements ([arch:hfst:prediction] [arch:hfst:prediction-walk] [arch:hfst:prediction-model])
    rules (
        [spec:hfst:def:prediction.unit]
        [spec:hfst:def:prediction.state]
        [spec:hfst:req:prediction.generator-decides]
        [spec:hfst:req:prediction.flags]
        [spec:hfst:req:prediction.bounded]
        [spec:hfst:sem:prediction.complete]
        [spec:hfst:req:prediction.realise]
        [spec:hfst:sem:prediction.preview]
        [spec:hfst:req:prediction.script-edges]
        [spec:hfst:sem:prediction.entry]
        [spec:hfst:sem:prediction.session]
        [spec:hfst:req:prediction.views]
        [spec:hfst:def:prediction-model.file]
        [spec:hfst:syn:prediction-model.format]
        [spec:hfst:req:prediction-model.input]
        [spec:hfst:sem:prediction-model.order]
        [spec:hfst:req:prediction-model.faces]
        [spec:hfst:req:prediction-model.budget]
        [spec:hfst:req:prediction-model.rank-only]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-11T00:00:00Z"
edges {
    requires ([dec:hfst:compile-surface] [dec:hfst:librarify-cli])
}
alternatives (
    {
        option "Try a fixed list of morpheme names and derivation types where the input ends, as the predict branch of the C++ hfst-optimized-lookup does."
        rejected_because "The list repeats what the generator already holds and has to be written again for every language. It looks one morpheme ahead and never checks endings, so its client offers endings the generator may refuse."
    }
    {
        option "Complete typed letters to the next morpheme boundary with one transducer weighted by the prefixes seen in a corpus, as lang-crk's morph-completion speller does (Lane, Harrigan and Arppe, 2022)."
        rejected_because "Every word form that starts with the typed letters is a candidate: the paper reports up to 10^5 for short prefixes and a median above 100 until 12 letters are typed. Its best ranking puts the target in the top 10 for 64.9% of queries, and a keyboard shows 3. A prefix count mixes stem and ending, so in a corpus of 243,000 words most prefixes are never seen. The person prefix reads as the start of a stem, which is the paper's worst failure. The weighted transducer can pass 100 MB."
    }
    {
        option "Rank whole word forms by how often they occur."
        rejected_because "A language with many forms for each stem sees most forms rarely or never in the text there is. Counting stems and endings apart pools what is seen."
    }
    {
        option "Rank with a neural language model."
        rejected_because "In the Cree paper a transformer ranked worse than prefix counts on the same data, and our corpora are smaller than such models need. The ranker takes candidates and returns them in order, so a neural ranker can be added later if measurement shows it helps."
    }
)
consequences {
    accepted (
        "Each language needs a unit grammar and analysed text to build its model from."
        "The walk and the ranker belong to hfst-embed's read-only surface, so @divvun/hfst and divvun-hfst predict without the compile entry point. The model builder sits behind the compile feature."
        "hfst gains predict, predict-model and predict-eval tools, each a thin driver over library calls. predict takes the place of the patched C++ optimized lookup."
    )
    deferred (
        "The keyboards: showing suggestions and faces and switching between them."
        "Completing typed letters to a morpheme boundary, unless the Cree comparison shows letter entry needs it."
        "Where prediction runs on a phone, hfst-embed or divvunspell, until a language's measurements show it is worth shipping."
    )
}
codifies (
    [spec:hfst:def:prediction.unit]
    [spec:hfst:req:prediction.generator-decides]
    [spec:hfst:req:prediction.realise]
    [spec:hfst:req:prediction-model.rank-only]
)
establishes ([arch:hfst:prediction] [arch:hfst:prediction-walk] [arch:hfst:prediction-model])
---

## Rationale

Oqaasileriffik's Greenlandic prediction shows the method works for a
polysynthetic language. Words are built from units, and a simple model over
units reaches the keystroke savings of analytic languages. Its weak point is
how it finds the units that can come next. A patched lookup tries a list of
462 morpheme names after the input, and the endings it offers are taken
from the corpus without asking the generator. The generator already holds
the answer, so the walk reads it there. That removes the per-language lists
and makes every offered unit one the generator accepts.

The Cree work completes letters, not units. Its own results show where that
leads: very many candidates, sparse prefix counts and a stem that the person
prefix hides. Picking a stem and an ending avoids all three, and the
generator spells the word.

The walk only reads a transducer, so it belongs with lookup in hfst-embed's
read-only surface, and the packages get it without loading the compilers.
Building a model reads a corpus and writes a file, as the compilers do, so it
sits behind the compile feature. The tools stay thin drivers over library
calls, as `[dec:hfst:librarify-cli]` asks.
