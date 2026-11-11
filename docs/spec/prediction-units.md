# Units

A unit grammar says how the analysis side of one language's generator
divides into units. The walk applies it to what the generator accepts, and
the model builder applies it to analysed text, so the units the walk offers
are the units the model counted (`prediction-model.same-grammar`).

## The grammar

> [spec:hfst:def:prediction-units.grammar+1]
> A unit grammar is a TOML file kept in a language's repository beside its
> generator. On the generator's analysis side, a symbol of one character is a
> letter unless the grammar lists it under `tags`, a flag diacritic is a
> flag, and any other symbol is a tag. The grammar sorts tags into
> boundaries, prefixes, enclitics and hidden tags, and it holds the
> language's filters, labels and word classes. A grammar's hash is the
> SHA-256 of its bytes.

> [spec:hfst:syn:prediction-units.format+1]
> A grammar is a TOML document with these keys, each optional:
>
> - `tags`: an array of symbols of one character that the grammar treats as
>   tags.
> - `boundary`, `prefix`, `enclitic` and `hidden`: arrays of patterns.
> - `filter`: an array of tables, each with a `name`, a `label` and `flags`,
>   an array of flag diacritics written as the generator writes them.
> - `labels`: a table from a unit's key to its label.
> - `class`: an array of tables, each with a `name` and `patterns`, an array
>   of patterns.
>
> A pattern matches a whole tag. In a pattern, `*` matches any run of
> characters and every other character matches itself. A grammar MUST be
> refused, with an error that names the cause, when it has a key not listed
> here, when an entry of `tags` is not exactly one character or is not in the
> generator's alphabet, when one of its patterns matches no tag in the
> generator's alphabet, or when one of its filters names a flag diacritic the
> generator lacks.

## Where units start and end

> [spec:hfst:sem:prediction-units.stem]
> A stem is a run of letters, read where a word starts, after its prefixes
> or after a boundary unit, up to the first tag. A stem is one unit, however
> many letters it has.

> [spec:hfst:sem:prediction-units.prefix]
> Before a word's first stem, a tag that matches a `prefix` pattern is a
> prefix unit on its own, as Plains Cree's preverbs (`PV/e+`) are.

> [spec:hfst:sem:prediction-units.boundary+1]
> After a stem or a boundary unit, a run of tags that ends with a tag
> matching a `boundary` pattern is one unit, a derivation or a compound join,
> and the next unit starts after it. Greenlandic's `Der/nv` and North Sámi's
> `Der/` tags and the `#` that closes a compound join are boundaries.

> [spec:hfst:sem:prediction-units.ending]
> After a stem or a boundary unit, a run of tags with no boundary tag and no
> enclitic tag in it, after which some configuration is final, is an ending. Every such run is
> offered, so an ending and a longer ending that extends it are both offered,
> such as `+N+Sg+Nom` and `+N+Sg+Nom+PxSg1`.

> [spec:hfst:sem:prediction-units.enclitic]
> After a complete word, a run of tags whose first tag matches an `enclitic`
> pattern, after which some configuration is final, is an enclitic. The word
> is still complete after it, and further enclitics can follow.

> [spec:hfst:sem:prediction-units.filter]
> A filter is a unit that adds nothing to the analysis. Picking one applies
> its flag diacritics to every configuration of the word in progress, as if
> the generator had read them there, and drops the configurations on which
> they fail. A filter is offered only before the word's first stem, and only
> when some configuration survives it. Plains Cree's `ni-` is a filter that
> sets the person flag the generator's own `ni-` sets, so after the stem only
> the endings that agree with it are offered.

## What the user sees

> [spec:hfst:sem:prediction-units.hidden]
> A tag that matches a `hidden` pattern stays in the unit's symbols, so the
> walk and the generator still read it, but is left out of the unit's key and
> label. Units that differ only in hidden tags are one choice, and the model
> counts them as one.

> [spec:hfst:sem:prediction-units.labels]
> The grammar's `labels` table gives a unit's label by its key. A stem without
> an entry is shown as its letters. An ending without an entry is shown as the
> complete word it makes. A filter is shown as its `label`. Any other unit
> without an entry is shown as its key.

> [spec:hfst:sem:prediction-units.classes]
> A unit's class is the first class, in the grammar's order, with a pattern
> that matches one of the unit's tags. A word in progress has the class of
> its latest unit that has one. A keyboard shows the face for the word's
> class (`prediction-model.faces`).
