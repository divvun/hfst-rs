//! Faithful 1:1 port of tools/src/hfst-tool-metadata.cc — common functions for
//! adding tool-related metadata (name, formula, commandline definition) to
//! automata. The tools use these to edit the metadata of created automata.

use hfst::backend::Backend;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};

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
pub fn hfst_set_name_maybe_truncate(dest: &mut impl ToolTransducer, s: &str) {
    if s.len() > 1024 {
        dest.set_name(&format!("truncated({}...)", &s[0..1000]));
    } else {
        dest.set_name(s);
    }
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
    } else if lhs.get_name().is_empty() && rhs.get_name().is_empty() {
        hfst_set_name_maybe_truncate(dest, &format!("{}(UNNAMED, UNNAMED)", op));
    } else {
        std::panic::panic_any(String::from(
            "!(a && b) || (!a && b) || (a && !b) || (!a && !b)",
        ));
    }
}

pub fn hfst_set_formula(dest: &mut impl ToolTransducer, src: &str, op: &str) {
    let c = src.as_bytes()[0] as i8 as i32;
    if (0 < c) && (c < 128) {
        hfst_set_formula_maybe_truncate(dest, &format!("{} {}", op, &src[0..1]));
    } else {
        hfst_set_formula_maybe_truncate(dest, &format!("{} U8", op));
    }
}

pub fn hfst_set_formula_unary(dest: &mut impl ToolTransducer, src: &impl ToolTransducer, op: &str) {
    if src.get_property("formulaic-definition") != "" {
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
    if (lhs.get_property("formulaic-definition") != "")
        && (rhs.get_property("formulaic-definition") != "")
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
        && (rhs.get_property("formulaic-definition") != "")
    {
        hfst_set_formula_maybe_truncate(
            dest,
            &format!(". {} {}", op, rhs.get_property("formulaic-definition")),
        );
    } else if (lhs.get_property("formulaic-definition") != "")
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

// Replicates POSIX 'basename(argv[0])' under the HAVE_BASENAME arm: the file
// component after the last '/'. (The non-HAVE_BASENAME arm used argv[0] whole.)
fn basename(s: &str) -> &str {
    match s.rfind('/') {
        Some(i) => &s[i + 1..],
        None => s,
    }
}

pub fn hfst_set_commandline_def(dest: &mut impl ToolTransducer, argv: &[String]) {
    let argc = argv.len();
    let mut cmdline = String::from("");
    let mut o = false;
    cmdline += basename(&argv[0]);
    for i in 1..=argc {
        if (argv[i] == "-v") || (argv[i] == "--verbose") {
            continue;
        } else if (argv[i] == "-o") || (argv[i] == "--output") {
            o = true;
        }
        cmdline += &argv[i];
    }
    if !o {
        cmdline += " > ??? ";
    }
    dest.set_property("commandline-definition", &cmdline);
}

pub fn hfst_set_commandline_def_unary(
    dest: &mut impl ToolTransducer,
    src: &impl ToolTransducer,
    argv: &[String],
) {
    let argc = argv.len();
    let mut cmdline = src.get_property("commandline-definition");
    if cmdline != "" {
        cmdline += "; ";
    }
    let mut o = false;
    cmdline += basename(&argv[0]);
    for i in 1..=argc {
        if (argv[i] == "-v") || (argv[i] == "--verbose") {
            continue;
        } else if (argv[i] == "-o") || (argv[i] == "--output") {
            o = true;
        }
        cmdline += &argv[i];
    }
    if !o {
        cmdline += " > ??? ";
    }
    dest.set_property("commandline-definition", &cmdline);
}

// [spec:hfst:def:hfst-tool-metadata.hfst-set-commandline-def-fn]
// [spec:hfst:sem:hfst-tool-metadata.hfst-set-commandline-def-fn]
pub fn hfst_set_commandline_def_binary(
    dest: &mut impl ToolTransducer,
    lhs: &impl ToolTransducer,
    rhs: &impl ToolTransducer,
    argv: &[String],
) {
    let argc = argv.len();
    let mut cmdline = lhs.get_property("commandline-definition");
    if cmdline != "" {
        cmdline += "&& ";
    }
    if rhs.get_property("commandline-definition") != "" {
        cmdline += &rhs.get_property("commandline-definition");
    }
    if cmdline != "" {
        cmdline += "; ";
    }
    let mut o = false;
    cmdline += basename(&argv[0]);
    for i in 1..=argc {
        if (argv[i] == "-v") || (argv[i] == "--verbose") {
            continue;
        } else if (argv[i] == "-o") || (argv[i] == "--output") {
            o = true;
        }
        cmdline += &argv[i];
    }
    if !o {
        cmdline += " > ??? ";
    }
    dest.set_property("commandline-definition", &cmdline);
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
