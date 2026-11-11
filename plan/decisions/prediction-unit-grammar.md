---
id [dec:hfst:prediction-unit-grammar]
epitome "What a unit is in a language is data: a TOML unit grammar kept in the language's repository beside its generator, read by both the walk and the model builder. A model records the grammar's hash, so a model and a grammar that disagree cannot be loaded together."
state @decided
category @existence
scope {
    elements ([arch:hfst:prediction-units])
    rules (
        [spec:hfst:def:prediction-units.grammar]
        [spec:hfst:syn:prediction-units.format]
        [spec:hfst:sem:prediction-units.stem]
        [spec:hfst:sem:prediction-units.prefix]
        [spec:hfst:sem:prediction-units.boundary]
        [spec:hfst:sem:prediction-units.ending]
        [spec:hfst:sem:prediction-units.enclitic]
        [spec:hfst:sem:prediction-units.filter]
        [spec:hfst:sem:prediction-units.hidden]
        [spec:hfst:sem:prediction-units.labels]
        [spec:hfst:sem:prediction-units.classes]
        [spec:hfst:req:prediction-model.same-grammar]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-11T00:00:00Z"
edges {
    requires ([dec:hfst:predictive-text])
}
alternatives (
    {
        option "Write each language's units into the code."
        rejected_because "A change to a language's tags would need an hfst release, and the people who know the tags maintain the language repositories, not hfst."
    }
    {
        option "Infer units from the shape of the tags alone, with no file."
        rejected_because "The shape finds North Sámi's Der/ tags, but not which tags are enclitics or prefixes, and nothing in a tag's shape says that Plains Cree's person prefix is chosen by a flag."
    }
    {
        option "Keep the grammars in this repository."
        rejected_because "They would change with the languages but be released with hfst, and a language's generator and its grammar would live in two places."
    }
    {
        option "Write the grammar in Styx."
        rejected_because "The language repositories already use TOML, in manifest.toml, and their maintainers would otherwise learn a new syntax for one file."
    }
)
consequences {
    accepted (
        "Each language repository carries a unit grammar. This repository keeps grammars for kal, crk and sme only as test fixtures."
        "A grammar with a key, pattern or flag the generator lacks is refused, not ignored."
        "Changing a grammar means rebuilding the language's model."
    )
    deferred (
        "Building the grammar's model in giella-core's language builds."
    )
}
codifies (
    [spec:hfst:def:prediction-units.grammar]
    [spec:hfst:syn:prediction-units.format]
    [spec:hfst:req:prediction-model.same-grammar]
)
establishes ([arch:hfst:prediction-units])
---

## Rationale

The walk and the model builder must cut analyses into the same units, or the
model counts units the walk never offers and ranks nothing. Both read one
file, and the model records that file's hash, so the two cannot drift apart
without an error.

What a unit is differs between languages in ways no shared rule captures.
Greenlandic chains derivations, each ending in a Der/ tag. North Sámi has
derivations and compounds. Plains Cree has preverb prefixes, and its person
prefix is set by a flag that its endings must agree with. A grammar per
language states this once, in the repository where the language's tags are
kept, by the people who maintain them.

A grammar that names something the generator lacks is refused. A pattern
that matches nothing is almost always a typo or a renamed tag, and ignoring
it would make units disappear without a word.
