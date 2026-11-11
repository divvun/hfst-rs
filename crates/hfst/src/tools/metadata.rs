//! Port of tools/src/hfst-tool-metadata.cc — the name and formula a tool gives
//! the transducer it writes. They are part of the bytes written, so a caller
//! that wants what a tool writes names its result the same way.

use crate::backend::Backend;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};

/// The metadata surface these helpers touch, implemented both for the typed
/// facade and for the stream-boundary sum ([dec:hfst:monomorphic-backends]):
/// tools call them before or after the one per-read dispatch.
pub trait ToolTransducer {
    fn get_name(&self) -> String;
    fn set_name(&mut self, name: &str);
    fn get_property(&self, property: &str) -> String;
    fn set_property(&mut self, property: &str, name: &str);
}

impl<B: Backend> ToolTransducer for HfstTransducer<B> {
    fn get_name(&self) -> String {
        HfstTransducer::get_name(self)
    }
    fn set_name(&mut self, name: &str) {
        HfstTransducer::set_name(self, name)
    }
    fn get_property(&self, property: &str) -> String {
        HfstTransducer::get_property(self, property)
    }
    fn set_property(&mut self, property: &str, name: &str) {
        HfstTransducer::set_property(self, property, name)
    }
}

impl ToolTransducer for AnyTransducer {
    fn get_name(&self) -> String {
        AnyTransducer::get_name(self)
    }
    fn set_name(&mut self, name: &str) {
        AnyTransducer::set_name(self, name)
    }
    fn get_property(&self, property: &str) -> String {
        AnyTransducer::get_property(self, property)
    }
    fn set_property(&mut self, property: &str, name: &str) {
        AnyTransducer::set_property(self, property, name)
    }
}

// [spec:hfst:def:hfst-tool-metadata.hfst-set-formula-maybe-truncate-fn]
// [spec:hfst:sem:hfst-tool-metadata.hfst-set-formula-maybe-truncate-fn]
pub fn hfst_set_formula_maybe_truncate(dest: &mut impl ToolTransducer, s: &str) {
    if s.len() > 1024 {
        dest.set_property("formulaic-definition", "TRUNC");
    } else {
        dest.set_property("formulaic-definition", s);
    }
}

// [spec:hfst:def:hfst-tool-metadata.hfst-set-name-maybe-truncate-fn]
// [spec:hfst:sem:hfst-tool-metadata.hfst-set-name-maybe-truncate-fn]
// [spec:hfst:req:embed.no-panic]
pub fn hfst_set_name_maybe_truncate(dest: &mut impl ToolTransducer, s: &str) {
    if s.len() > 1024 {
        dest.set_name(&format!("truncated({}...)", prefix_within(s, 1000)));
    } else {
        dest.set_name(s);
    }
}

/// The longest prefix of `s` at most `len` bytes long that ends on a
/// character boundary.
fn prefix_within(s: &str, len: usize) -> &str {
    let end = (0..=len.min(s.len()))
        .rev()
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(0);
    &s[..end]
}

pub fn hfst_set_name(dest: &mut impl ToolTransducer, src: &str, op: &str) {
    hfst_set_name_maybe_truncate(dest, &format!("{}({})", op, src));
}

pub fn hfst_set_name_unary(dest: &mut impl ToolTransducer, src: &impl ToolTransducer, op: &str) {
    if src.get_name() != "" {
        hfst_set_name_maybe_truncate(dest, &format!("{}({})", op, src.get_name()));
    } else {
        hfst_set_name_maybe_truncate(dest, &format!("{}(UNNAMED)", op));
    }
}

// [spec:hfst:def:hfst-tool-metadata.hfst-set-name-fn]
// [spec:hfst:sem:hfst-tool-metadata.hfst-set-name-fn]
pub fn hfst_set_name_binary(
    dest: &mut impl ToolTransducer,
    lhs: &impl ToolTransducer,
    rhs: &impl ToolTransducer,
    op: &str,
) {
    if (lhs.get_name() != "") && (rhs.get_name() != "") {
        hfst_set_name_maybe_truncate(
            dest,
            &format!("{}({}, {})", op, lhs.get_name(), rhs.get_name()),
        );
    } else if lhs.get_name().is_empty() && (rhs.get_name() != "") {
        hfst_set_name_maybe_truncate(dest, &format!("{}(UNNAMED, {})", op, rhs.get_name()));
    } else if (lhs.get_name() != "") && rhs.get_name().is_empty() {
        hfst_set_name_maybe_truncate(dest, &format!("{}({}, UNNAMED)", op, lhs.get_name()));
    } else {
        hfst_set_name_maybe_truncate(dest, &format!("{}(UNNAMED, UNNAMED)", op));
    }
}

// [spec:hfst:req:embed.no-panic]
// The C++ read the first byte unconditionally; an empty name has none, and
// takes the same "U8" as a name that does not start with ASCII.
pub fn hfst_set_formula(dest: &mut impl ToolTransducer, src: &str, op: &str) {
    match src.as_bytes().first() {
        Some(&c) if (1..128).contains(&c) => {
            hfst_set_formula_maybe_truncate(dest, &format!("{} {}", op, c as char));
        }
        Some(_) | None => hfst_set_formula_maybe_truncate(dest, &format!("{} U8", op)),
    }
}

pub fn hfst_set_formula_unary(dest: &mut impl ToolTransducer, src: &impl ToolTransducer, op: &str) {
    if !src.get_property("formulaic-definition").is_empty() {
        hfst_set_formula_maybe_truncate(
            dest,
            &format!("{} {}", op, src.get_property("formulaic-definition")),
        );
    } else {
        hfst_set_formula_maybe_truncate(dest, &format!("{} .", op));
    }
}

// [spec:hfst:def:hfst-tool-metadata.hfst-set-formula-fn]
// [spec:hfst:sem:hfst-tool-metadata.hfst-set-formula-fn]
pub fn hfst_set_formula_binary(
    dest: &mut impl ToolTransducer,
    lhs: &impl ToolTransducer,
    rhs: &impl ToolTransducer,
    op: &str,
) {
    if (!lhs.get_property("formulaic-definition").is_empty())
        && (!rhs.get_property("formulaic-definition").is_empty())
    {
        hfst_set_formula_maybe_truncate(
            dest,
            &format!(
                "{} {} {}",
                lhs.get_property("formulaic-definition"),
                op,
                rhs.get_property("formulaic-definition")
            ),
        );
    } else if lhs.get_property("formulaic-definition").is_empty()
        && (!rhs.get_property("formulaic-definition").is_empty())
    {
        hfst_set_formula_maybe_truncate(
            dest,
            &format!(". {} {}", op, rhs.get_property("formulaic-definition")),
        );
    } else if (!lhs.get_property("formulaic-definition").is_empty())
        && rhs.get_property("formulaic-definition").is_empty()
    {
        hfst_set_formula_maybe_truncate(
            dest,
            &format!("{} {} .", lhs.get_property("formulaic-definition"), op),
        );
    } else {
        hfst_set_formula_maybe_truncate(dest, &format!(". {} .", op));
    }
}

// [spec:hfst:def:hfst-tool-metadata.hfst-get-name-fn]
// [spec:hfst:sem:hfst-tool-metadata.hfst-get-name-fn]
pub fn hfst_get_name(arg: &impl ToolTransducer, filename: &str) -> String {
    if arg.get_name() != "" {
        arg.get_name()
    } else {
        filename.to_string()
    }
}
