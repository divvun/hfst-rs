---
id [dec:hfst:independent-fork]
epitome "Divvun HFST is an independent fork, not a port under parity obligation: compatibility is kept where it is free, and divergence for correctness needs no upstream sanction."
state @decided
category @executive
scope {
    elements ([arch:hfst:backend-dispatch])
}
author "brendan@necessary.nu"
alternatives (
    {
        option "Keep byte- and behaviour-parity with upstream C++ HFST as the governing constraint, fixing bugs only where upstream has fixed them."
        rejected_because "Parity was the right instrument while the port was being built — it made every difference a defect and gave the work an oracle. It stopped paying once the port passed upstream: it pinned the Helsinki/GPLv3 banners in spec rules so they could not be deleted, and it left the hfst-info feature checks inverted because upstream inverts them. An obligation that forbids fixing a known-wrong answer is no longer buying correctness."
    }
    {
        option "Diverge freely and stop tracking upstream behaviour at all."
        rejected_because "Upstream behaviour is what Giella build systems, .hfst consumers and downstream tooling actually depend on. Gratuitous divergence would break real users for no gain, and it would throw away the oracle that is still the cheapest way to check any behaviour nobody has thought hard about."
    }
)
consequences {
    accepted (
        "Byte-identical output is evidence, not a requirement: a diff against C++ is a question to answer, not a bug by construction."
        "A faithfully ported upstream bug is a bug in this project. It gets fixed here and, where worthwhile, reported upstream."
        "Spec rules under docs/spec/port/ describe what this project does. Where that departs from the C++ they carry a PORT DIVERGENCE note with the reason; a rule that merely records upstream behaviour does not bind us."
        "Sibling crates (nfst, foma) may gain primitives their upstreams lack when that is the honest fix, published as normal releases."
    )
    deferred (
        "The C++ oracle stays useful and stays installed, but it is now advisory. Each divergence has to be argued on its merits, which is more work per decision than 'match upstream' was."
        "Divergences accumulate into a compatibility surface somebody must eventually document for downstream users; there is no such document yet."
    )
}
---

## Rationale

This project began as a 1:1 port, and parity with upstream C++ HFST was the
instrument that made that possible: it supplied an oracle for every function,
turned every behavioural difference into a defect, and let correctness be
checked mechanically rather than argued. That instrument has done its work.

It now costs more than it returns, and the costs are not hypothetical. Parity
pinned the University of Helsinki copyright and a GPLv3 licence banner into
spec rules, so the strings could not be corrected without the code falling out
of conformance with its own spec — for a tree that is LGPL and whose CLI was
never a literal port. It reproduced an inverted feature check in `hfst-info`,
where upstream guards "Required foma support not present" with `#if HAVE_FOMA`,
so the tool reports failure for backends that work. Neither is a case where
matching upstream serves a user.

The governing rule is therefore: **compatibility where possible, divergence for
correctness where necessary.**

Compatibility is not abandoned, because it is what downstream actually consumes.
On-disk formats, the `.hfst` header, tool names and their flags, and the shape
of tool output are contracts with Giella build systems, divvunspell, and every
existing archive. Those change only for a reason that outweighs breaking them.
Where behaviour is merely incidental — an internal map key, a diagnostic, a
symbol-table entry count, a compiler's parse of an ambiguous construct — this
project answers to correctness alone.

The C++ build remains installed and remains valuable: for any behaviour nobody
has reasoned about carefully, comparing against it is still the cheapest way to
find out what should happen. The change is in its authority. A difference from
C++ is now a question — *which of us is right?* — rather than a verdict. When
the answer is that upstream is wrong, this project fixes it and says so in the
spec rule, and the fix is worth reporting upstream.

This extends to the sibling crates. `nfst` and `foma` are ports too, and the
same reasoning applies: when the honest fix is a primitive the upstream lacks,
adding it is correct, and it ships as a normal release rather than as a local
patch.

The practical test for any proposed divergence is whether it makes the software
answer a question more truthfully. Fixing an inverted check, deleting a
copyright that was never ours to claim, adding a parser rule that resolves an
ambiguity the way the language actually means it — all yes. Renaming things
because a different name reads better, or changing an output format because it
would be tidier, are not: they impose migration cost on users to buy taste.
