# Flag encoding and the symbol tables

The xerox flag-diacritic pass that `eliminate flag` and the xerox rule
compilations rely on rewrites every flag name to a form ordinary symbol
harmonization will not treat specially — `@U.Cap.up@` becomes `%U.Cap.up%` for
the duration of an operation, and the matching decode restores it.  The
reference implementation performs each half as a whole-graph rebuild through
`HfstBasicTransducer`, which emits one fresh alphabet per rebuild.  This port
keeps an in-place fast path for the tropical backend that edits the symbol table
directly, because the rebuild costs about 14% of a Giella speller's xfst script
and the rename is pure metadata.

A tropical transducer always carries an input symbol table and MAY carry an
output one.  When both are present they are equivalent — the invariant
`handle_symbol_tables` states outright and `copy_alphabet` then depends on,
since it unions *both* tables into the interchange graph's alphabet.  Operations
that swap the tables onto opposite sides produce that two-table shape; reversal
is the common one, and `minimize` reaches for the reverse orientation on its own
whenever the determinization budget overruns, so the shape appears on exactly
the large machines nobody probes by hand.

The measured case is the Lule Sámi (`smj`) speller.  A 3.8M-state generator
overran the determinization budget, minimization retried in the reverse
orientation as [spec:hfst:req:determinize-envelope.relation-preserved] permits,
and the resulting two-table machine reached `eliminate flag Der1`.  The encode
renamed only the input table, so the alphabet union carried `%U.Cap.up%` from
one table beside `@U.Cap.up@` from the stale other; the decode then renamed the
encoded name onto a name the alphabet already held, and the arcs carrying it
were dropped — 774,901 flag transitions to zero in a single composition.  Flags
*are* the casing constraint, so with them gone the speller accepted every
lowercase spelling of a proper noun: 191 false negatives against the smj typo
corpus, and a silent one, since the machine was well-formed and every other
reading survived.

## Both tables spell every symbol the same way

> [spec:hfst:req:flag-encode-symbol-tables.table-parity]
> A symbol rename applied in place to a transducer's symbol table MUST be
> applied to every symbol table that transducer carries, so that a name reachable
> through one table is reachable through all of them.  Encoding MUST leave no
> table still spelling a flag in the un-encoded form, and decoding MUST restore
> the original spelling in every table it encoded.  The requirement is not
> cosmetic: alphabet construction unions all of a transducer's tables, so a table
> left un-renamed contributes the pre-rename name to the same alphabet as its
> encoding, and the inverse rename then collides with a name already present.  A
> collision of that kind is silent — the operation reports success and returns a
> well-formed machine that has lost every transition carrying the colliding
> symbol.
>
> An in-place metadata edit substituted for a whole-graph rebuild MUST be
> observationally equivalent to that rebuild for every transducer shape the
> library can produce, including shapes no single operation constructs
> deliberately — a machine carrying a second symbol table because an
> equivalence-preserving retry left it there is an ordinary input, not a
> malformed one.
