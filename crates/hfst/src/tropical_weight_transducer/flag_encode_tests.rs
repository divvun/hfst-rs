//! The xerox-composition flag encode/decode must leave the two symbol tables
//! spelling every flag the same way.
//!
//! An HFST tropical transducer always carries an input symbol table and MAY
//! carry an output one; when it does, the two are equivalent — the invariant
//! `handle_symbol_tables` (ConvertTropicalWeightTransducer) states outright and
//! `copy_alphabet` then relies on, unioning BOTH tables into the interchange
//! graph's alphabet. `reverse` is one operation that produces that two-table
//! shape (rustfst's Reverse swaps the tables onto the opposite sides, and the
//! tropical wrapper copies the input table back).
//!
//! The C++ implements the flag pass as a whole-graph rebuild through
//! HfstBasicTransducer, which emits one fresh alphabet and so cannot drift.
//! This port keeps a tropical fast path that renames the SymbolTable in place,
//! and that shortcut used to rewrite only the input table — leaving the output
//! table spelling each flag `@X@` while the input table spelled it `%X%`. The
//! union then carried both spellings, which is what let a `%X%` decode land on
//! a name the alphabet already held.

use super::*;
use std::sync::Arc;

use crate::hfst_flag_diacritics::FdOperation;

// Build a two-arc machine `@U.Cap.up@ a` whose symbol table holds a flag, and
// give it BOTH symbol tables the way `reverse` leaves them.
fn flagged_fst(with_output_table: bool) -> StdVectorFst {
    let mut st = SymbolTable::empty();
    st.add_symbol_with_key("@_EPSILON_SYMBOL_@", 0);
    st.add_symbol_with_key("@U.Cap.up@", 1);
    st.add_symbol_with_key("a", 2);

    let mut t = StdVectorFst::new();
    let s0 = t.add_state();
    let s1 = t.add_state();
    let s2 = t.add_state();
    t.set_start(s0).expect("start state");
    t.set_final(s2, TropicalWeight::one()).expect("final state");
    t.add_tr(s0, StdTransition::new(1, 1, TropicalWeight::one(), s1))
        .expect("flag arc");
    t.add_tr(s1, StdTransition::new(2, 2, TropicalWeight::one(), s2))
        .expect("letter arc");

    let st = Arc::new(st);
    t.set_input_symbols(Arc::clone(&st));
    if with_output_table {
        t.set_output_symbols(st);
    }
    t
}

// Every flag name reachable through EITHER symbol table — the same union
// `copy_alphabet` builds when the transducer is converted for harmonization.
fn diacritics_across_both_tables(t: &StdVectorFst) -> Vec<String> {
    let mut names = Vec::new();
    for table in [t.input_symbols(), t.output_symbols()]
        .into_iter()
        .flatten()
    {
        for (_label, sym) in table.iter() {
            if FdOperation::is_diacritic(sym) {
                names.push(sym.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

// [spec:hfst:req:flag-encode-symbol-tables.table-parity/test]
#[test]
fn encode_leaves_no_unencoded_flag_in_either_table() {
    for with_output_table in [false, true] {
        let mut t = flagged_fst(with_output_table);
        TropicalWeightTransducer::encode_flag_diacritics(&mut t);
        assert_eq!(
            diacritics_across_both_tables(&t),
            Vec::<String>::new(),
            "with_output_table={with_output_table}: an encoded transducer must not \
             still spell any flag `@...@` in either symbol table — the un-encoded \
             name is invisible to `?` harmonization and collides with the decode"
        );
    }
}

// [spec:hfst:req:flag-encode-symbol-tables.table-parity/test]
#[test]
fn encode_decode_round_trips_with_both_tables() {
    for with_output_table in [false, true] {
        let before = flagged_fst(with_output_table);
        let mut t = before.clone();
        TropicalWeightTransducer::encode_flag_diacritics(&mut t);
        TropicalWeightTransducer::decode_flag_diacritics(&mut t);

        assert_eq!(
            diacritics_across_both_tables(&t),
            diacritics_across_both_tables(&before),
            "with_output_table={with_output_table}: encode/decode must round-trip \
             the flag names"
        );
        assert_eq!(
            t.output_symbols().is_some(),
            with_output_table,
            "with_output_table={with_output_table}: the flag pass must not add or \
             drop a symbol table"
        );
        if with_output_table {
            let input: Vec<_> = t
                .input_symbols()
                .expect("input table")
                .iter()
                .map(|(l, s)| (l, s.to_string()))
                .collect();
            let output: Vec<_> = t
                .output_symbols()
                .expect("output table")
                .iter()
                .map(|(l, s)| (l, s.to_string()))
                .collect();
            assert_eq!(
                input, output,
                "the two tables must stay equivalent across the flag pass"
            );
        }
    }
}
