//! Port of `libhfst/src/HfstFlagDiacritics.{h,cc}`.
//!
//! Partial: only `FdOperation::is_diacritic` is ported here so far, because
//! `HfstSymbolDefs::remove_flags` depends on it. The remainder of the
//! flag-diacritic machinery (the `FdOperator` enum, `FdState`/`FdTable`
//! templates, `FdOperation`'s instance members, etc.) is ported in the
//! `core.util` layer.

/// \brief Operations defined by a flag diacritic.
///
/// `class FdOperation`. Here represented by a unit struct exposing the static
/// `is_diacritic` predicate; instance state is added when the rest of the file
/// is ported.
pub struct FdOperation;

impl FdOperation {
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.is-diacritic-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.is-diacritic-fn]
    //
    // All diacritics have form @[PNDRCU][.][A-Z]+([.][A-Z]+)?@. Indexing is by
    // byte, matching C++ `std::string::at`/`size`/`find_last_of` over the ASCII
    // diacritic syntax.
    pub fn is_diacritic(diacritic_string: &str) -> bool {
        let bytes = diacritic_string.as_bytes();
        if diacritic_string.len() < 5 {
            return false;
        }
        if bytes[2] != b'.' {
            return false;
        }
        // These two checks probably always succeed...
        if bytes[0] != b'@' {
            return false;
        }
        if bytes[diacritic_string.len() - 1] != b'@' {
            return false;
        }
        match bytes[1] {
            b'P' => {}
            b'N' => {}
            b'D' => {}
            b'R' => {}
            b'C' => {}
            b'U' => {}
            _ => return false,
        }
        if diacritic_string.rfind('.') == Some(2) {
            if bytes[1] != b'R' && bytes[1] != b'D' && bytes[1] != b'C' {
                return false;
            }
        }
        true
    }
}
