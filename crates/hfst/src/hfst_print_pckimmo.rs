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

use crate::hfst_basic_transducer::{HfstBasicTransducer, HfstState};
use crate::hfst_symbol_defs::{internal_epsilon, internal_unknown};

// Raw byte-faithful stand-in for 'fprintf''s output side: write the
// already-formatted bytes verbatim with 'fwrite'. Symbols may be truncated by
// '%.*s' at an arbitrary byte boundary (mid-UTF-8), so the buffer is '[u8]'
// rather than 'str'.
unsafe fn fwrite_bytes(out: *mut libc::FILE, bytes: &[u8]) {
    unsafe {
        libc::fwrite(bytes.as_ptr() as *const libc::c_void, 1, bytes.len(), out);
    }
}

// C printf '%*s': right-justify 's' in a field of minimum 'width' bytes,
// padding with spaces on the left.
fn fmt_star_s(width: usize, s: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    if s.len() < width {
        for _ in 0..(width - s.len()) {
            out.push(b' ');
        }
    }
    out.extend_from_slice(s);
    out
}

// C printf '%.*s': print at most 'precision' bytes of 's'.
fn fmt_dot_star_s(precision: usize, s: &[u8]) -> Vec<u8> {
    let n = std::cmp::min(precision, s.len());
    s[..n].to_vec()
}

// C printf '%.*d': decimal with a minimum of 'precision' digits, zero-padded on
// the left. The value is read as a signed 'int' (the '%d' conversion). Per ISO
// C, converting a zero value with a precision of zero yields no characters.
fn fmt_dot_star_d(precision: usize, value: i32) -> Vec<u8> {
    if precision == 0 && value == 0 {
        return Vec::new();
    }
    let neg = value < 0;
    let mut digits = (value as i64).unsigned_abs().to_string().into_bytes();
    while digits.len() < precision {
        digits.insert(0, b'0');
    }
    if neg {
        let mut v = vec![b'-'];
        v.extend_from_slice(&digits);
        v
    } else {
        digits
    }
}

// [spec:hfst:def:hfst-print-pc-kimmo.hfst.print-pckimmo-fn]
// [spec:hfst:sem:hfst-print-pc-kimmo.hfst.print-pckimmo-fn]
pub unsafe fn print_pckimmo(out: *mut libc::FILE, t: &crate::hfst_transducer::HfstTransducer) {
    unsafe {
        // C++: 'HfstBasicTransducer mutt {t};' — build the interchange graph from
        // the facade. get_basic_transducer is the HfstBasicTransducer(const
        // HfstTransducer&) conversion.
        let mutt: HfstBasicTransducer = t.get_basic_transducer();
        let mut s: HfstState = 0;
        let mut last: HfstState = 0;
        let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
        for state in mutt.iter() {
            for arc in state.iter() {
                let first: String = arc.get_input_symbol();
                let second: String = arc.get_output_symbol();
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
        {
            let mut buf = fmt_star_s(numwidth as usize, b" ");
            buf.extend_from_slice(b"  ");
            fwrite_bytes(out, &buf);
        }
        for p in &pairs {
            if p.0.as_str() == internal_epsilon {
                let mut buf = fmt_dot_star_s(numwidth as usize, b"0");
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            } else if p.0.as_str() == internal_unknown {
                let mut buf = fmt_dot_star_s(numwidth as usize, b"@");
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            } else {
                let mut buf = fmt_dot_star_s(numwidth as usize, p.0.as_bytes());
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            }
        }
        // second line is output symbols per pair
        fwrite_bytes(out, b"\n");
        // (left corner is digit width + 2)
        {
            let mut buf = fmt_star_s(numwidth as usize, b" ");
            buf.extend_from_slice(b"  ");
            fwrite_bytes(out, &buf);
        }
        for p in &pairs {
            if p.1.as_str() == internal_epsilon {
                let mut buf = fmt_dot_star_s(numwidth as usize, b"0");
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            } else if p.1.as_str() == internal_unknown {
                let mut buf = fmt_dot_star_s(numwidth as usize, b"@");
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            } else {
                let mut buf = fmt_dot_star_s(numwidth as usize, p.1.as_bytes());
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            }
        }
        // the transition table per state
        fwrite_bytes(out, b"\n");
        for state in mutt.iter() {
            if mutt.is_final_state(s) {
                let mut buf = fmt_dot_star_d(numwidth as usize, s.wrapping_add(1) as i32);
                buf.extend_from_slice(b". ");
                fwrite_bytes(out, &buf);
            } else {
                let mut buf = fmt_dot_star_d(numwidth as usize, s.wrapping_add(1) as i32);
                buf.extend_from_slice(b": ");
                fwrite_bytes(out, &buf);
            }
            // map everything to sink state 0 first
            let mut transitions: BTreeMap<(String, String), HfstState> = BTreeMap::new();
            for p in &pairs {
                transitions.insert(p.clone(), (-1i32) as u32);
            }
            for arc in state.iter() {
                let first: String = arc.get_input_symbol();
                let second: String = arc.get_output_symbol();
                transitions.insert((first, second), arc.get_target_state());
            }
            for (_k, v) in &transitions {
                let mut buf = fmt_dot_star_d(numwidth as usize, v.wrapping_add(1) as i32);
                buf.extend_from_slice(b" ");
                fwrite_bytes(out, &buf);
            }
            fwrite_bytes(out, b"\n");
            s += 1;
        } // for each state
    }
}
