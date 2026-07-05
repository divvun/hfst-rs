//! Port of 'libhfst/src/HfstPrintPCKimmo.{cc,h}' — writes a transducer in the
//! PC-KIMMO transition-table text format to a C 'FILE*'.
//!
//! The only deferred dependency is the entry point's construction of the
//! interchange graph from the facade: C++ 'HfstBasicTransducer mutt {t};', which
//! needs both the facade 'crate::hfst_transducer::HfstTransducer' and the
//! 'HfstBasicTransducer(const HfstTransducer&)' conversion constructor (itself
//! deferred in 'crate::hfst_basic_transducer'). Everything after that point is
//! ported literally against the already-available 'HfstBasicTransducer'.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use crate::hfst_basic_transducer::{HfstBasicTransducer, HfstState};
use crate::hfst_symbol_defs::{internal_epsilon, internal_unknown};

// C printf '%.*s' of a symbol: at most 'precision' bytes, so a multibyte
// symbol may be cut mid-UTF-8. Rust's '{:.n$}' truncates by chars, not bytes,
// so the byte-exact truncation is done here and written raw.
fn write_symbol_truncated(
    out: &mut dyn Write,
    precision: usize,
    symbol: &str,
) -> std::io::Result<()> {
    let bytes = symbol.as_bytes();
    out.write_all(&bytes[..precision.min(bytes.len())])?;
    out.write_all(b" ")
}

// C printf '%.*d' for the non-negative values used here: zero-pad to a minimum
// of 'precision' digits. Per ISO C, converting a zero value with a precision of
// zero yields no characters — that case is not expressible with '{:0n$}'.
fn write_state_number(out: &mut dyn Write, precision: usize, value: i32) -> std::io::Result<()> {
    if precision == 0 && value == 0 {
        return Ok(());
    }
    write!(out, "{value:0precision$}")
}

// [spec:hfst:def:hfst-print-pc-kimmo.hfst.print-pckimmo-fn]
// [spec:hfst:sem:hfst-print-pc-kimmo.hfst.print-pckimmo-fn]
pub fn print_pckimmo<B: crate::backend::Backend>(
    out: &mut dyn Write,
    t: &crate::hfst_transducer::HfstTransducer<B>,
) -> std::io::Result<()> {
    // C++: 'HfstBasicTransducer mutt {t};' — build the interchange graph from
    // the facade. get_basic_transducer is the HfstBasicTransducer(const
    // HfstTransducer&) conversion.
    let mutt: HfstBasicTransducer = t
        .get_basic_transducer()
        .expect("get_basic_transducer on a valid transducer cannot fail");
    let mut s: HfstState = 0;
    let mut last: HfstState = 0;
    let mut pairs: BTreeSet<(
        crate::hfst_data_types::Symbol,
        crate::hfst_data_types::Symbol,
    )> = BTreeSet::new();
    for state in mutt.iter() {
        for arc in state.iter() {
            let first = arc.get_input_symbol(mutt.coder());
            let second = arc.get_output_symbol(mutt.coder());
            pairs.insert((first, second));
        }
        last += 1;
    }
    // width of the first column
    let mut numwidth: u32 = 0;
    {
        let mut i: u32 = 1;
        while i < last {
            i = i.wrapping_mul(10);
            numwidth += 1;
        }
    }
    // first line is input symbols per pair
    // (left corner is digit width + 2)
    write!(out, "{:>width$}  ", " ", width = numwidth as usize)?;
    for p in &pairs {
        if p.0.as_str() == internal_epsilon {
            write_symbol_truncated(out, numwidth as usize, "0")?;
        } else if p.0.as_str() == internal_unknown {
            write_symbol_truncated(out, numwidth as usize, "@")?;
        } else {
            write_symbol_truncated(out, numwidth as usize, &p.0)?;
        }
    }
    // second line is output symbols per pair
    writeln!(out)?;
    // (left corner is digit width + 2)
    write!(out, "{:>width$}  ", " ", width = numwidth as usize)?;
    for p in &pairs {
        if p.1.as_str() == internal_epsilon {
            write_symbol_truncated(out, numwidth as usize, "0")?;
        } else if p.1.as_str() == internal_unknown {
            write_symbol_truncated(out, numwidth as usize, "@")?;
        } else {
            write_symbol_truncated(out, numwidth as usize, &p.1)?;
        }
    }
    // the transition table per state
    writeln!(out)?;
    for state in mutt.iter() {
        write_state_number(out, numwidth as usize, s.wrapping_add(1) as i32)?;
        if mutt.is_final_state(s) {
            write!(out, ". ")?;
        } else {
            write!(out, ": ")?;
        }
        // map everything to sink state 0 first
        let mut transitions: BTreeMap<
            (
                crate::hfst_data_types::Symbol,
                crate::hfst_data_types::Symbol,
            ),
            HfstState,
        > = BTreeMap::new();
        for p in &pairs {
            transitions.insert(p.clone(), (-1i32) as u32);
        }
        for arc in state.iter() {
            let first = arc.get_input_symbol(mutt.coder());
            let second = arc.get_output_symbol(mutt.coder());
            transitions.insert((first, second), arc.get_target_state());
        }
        for (_k, v) in &transitions {
            write_state_number(out, numwidth as usize, v.wrapping_add(1) as i32)?;
            write!(out, " ")?;
        }
        writeln!(out)?;
        s += 1;
    } // for each state
    Ok(())
}
