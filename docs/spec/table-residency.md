# Transducer table residency

An optimized-lookup archive is dominated by its two tables. What the loader
does with them decides the resident cost of every loaded transducer: the
tokeniser `.pmhfst` is the largest asset in a Giella bundle, and a lookup
service loads one per pipeline. The C++ shape this code was ported from hands
tables around by value (`get_vector` returning `std::vector<T>`), and a literal
translation of that shape turns each such call into a full heap copy. Measured
on the pmatch load path, the ported code stacked up to three copies of a table
that the archive only needed once.

These rules pin the loader to the minimum: one copy, sized exactly, with the
existing corrupt-header defence kept intact.

## Size fields off disk are claims, not facts

> [spec:hfst:req:table-residency.untrusted-size-fields]
> A table entry count read from an archive header MUST NOT be converted into a
> single up-front allocation ask. The reader's outstanding allocation ahead of
> bytes actually read MUST stay bounded by a fixed batch size regardless of the
> claimed count, and a short read MUST stop the load with a clean error rather
> than an allocator abort. A corrupt header may claim any count; the cost of
> believing it must be bounded before the bytes disprove it.

## One copy, sized exactly

> [spec:hfst:req:table-residency.single-copy-load]
> Loading a table pair from an archive MUST leave exactly one live copy of each
> table, whose resident capacity equals its entry count. Growth overshoot from
> incremental reading MUST be released before the load returns, and no
> full-table copy beyond the bounded read batch may be made on the load path.
> Constructing a runtime transducer from an already-loaded one MUST add at most
> the one copy being constructed.

> [spec:hfst:req:table-residency.views-not-copies]
> Read access to a loaded table MUST be by reference. An owned copy MUST only
> be produced by an operation whose name says so at the call site — a consuming
> conversion or an explicit copy — never by an accessor that looks like a
> getter. The C++ `get_vector` shape, an accessor returning an owned vector, is
> retired rather than translated.
