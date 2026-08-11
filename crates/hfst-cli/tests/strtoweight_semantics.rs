//! `hfst_strtoweight` must reproduce C++'s `strtod` + `*endptr == '\0'`, which
//! `str::parse::<f64>` does not.
//!
//! The case that matters in practice is the empty field. Giella's speller
//! rules build final-string edits with
//!
//!     grep -v '^#' $< | grep -v '^$' | cut -f1-2 | hfst-strings2fst -j ...
//!
//! so a hand-maintained `final_strings.*.txt` line carrying a stray second tab
//! (`uuni:on\t\t1`) reaches the tool as `uuni:on\t` — field 2 empty. C++
//! strtod performs no conversion there, leaves endptr on the NUL, passes the
//! `*endptr == '\0'` test and yields weight 0. Rejecting it instead fails the
//! whole lang-kal speller build.
//!
//! Expectations below were taken from an installed C++ hfst 3.17.1.

use hfst_cli::globals::CommonOptions;
use hfst_cli::hfst_commandline::hfst_strtoweight;

fn opts() -> CommonOptions {
    CommonOptions::default()
}

#[test]
fn empty_weight_field_is_zero_not_an_error() {
    assert_eq!(hfst_strtoweight(&opts(), ""), 0.0);
}

#[test]
fn leading_whitespace_is_skipped_like_strtod() {
    assert_eq!(hfst_strtoweight(&opts(), " 1.5"), 1.5);
    assert_eq!(hfst_strtoweight(&opts(), "\t2"), 2.0);
}

#[test]
fn plain_weights_parse() {
    assert_eq!(hfst_strtoweight(&opts(), "8"), 8.0);
    assert_eq!(hfst_strtoweight(&opts(), "1.5"), 1.5);
    assert_eq!(hfst_strtoweight(&opts(), "-3"), -3.0);
}
