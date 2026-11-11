//! The checks the OpenFst read runs before rustfst sees the bytes, and after
//! it has built a transducer from them.

use super::*;

const FST_MAGIC: i32 = 2_125_659_606;
const SYMBOL_TABLE_MAGIC: i32 = 2_125_658_996;

fn string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// A header with an input symbol table of the rows given as (name, key).
fn header(rows: &[(&[u8], i64)], row_count: i64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&FST_MAGIC.to_le_bytes());
    string(&mut out, b"vector");
    string(&mut out, b"standard");
    out.extend_from_slice(&2i32.to_le_bytes());
    out.extend_from_slice(&1i32.to_le_bytes());
    out.extend_from_slice(&[0u8; 32]);
    out.extend_from_slice(&SYMBOL_TABLE_MAGIC.to_le_bytes());
    string(&mut out, b"");
    out.extend_from_slice(&0i64.to_le_bytes());
    out.extend_from_slice(&row_count.to_le_bytes());
    for (name, key) in rows {
        string(&mut out, name);
        out.extend_from_slice(&key.to_le_bytes());
    }
    out
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn header_check_refuses_what_rustfst_mistrusts() {
    let good = header(&[(b"@_EPSILON_SYMBOL_@", 0), (b"a", 1)], 2);
    assert!(check_openfst_header(&good).is_ok());
    for bad in [
        header(&[(b"\xff", 0)], 1),
        header(&[(b"a", -1)], 1),
        header(&[(b"a", 1 << 40)], 1),
        header(&[], -3),
        good[..good.len() - 3].to_vec(),
    ] {
        assert!(check_openfst_header(&bad).is_err());
    }
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn state_check_refuses_a_dangling_arc() {
    let mut fst = StdVectorFst::new();
    let start = fst.add_state();
    fst.set_start(start).expect("the state exists");
    assert!(check_openfst_states(&fst).is_ok());
    fst.add_tr(start, Tr::new(1, 1, TropicalWeight::one(), 7))
        .expect("the source state exists");
    assert!(check_openfst_states(&fst).is_err());
}
