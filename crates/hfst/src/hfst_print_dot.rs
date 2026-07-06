//! Port of 'libhfst/src/HfstPrintDot.{cc,h}' — render an ['HfstTransducer'] as
//! a Graphviz 'dot' graph.
//!
//! The two C++ 'print_dot' overloads ('FILE*' and 'std::ostream&') become the
//! distinct names ['print_dot_file'] and ['print_dot_os'].

use std::collections::BTreeMap;
use std::io::Write;

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::Symbol;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};
use crate::hfst_transducer::HfstTransducer;
use crate::hfst_tropical_transducer_transition_data::SymbolCoder;
use crate::string_utils::replace_all;

// '#define DOT_MAX_LABEL_SIZE 64'
const DOT_MAX_LABEL_SIZE: usize = 64;

// The MSVC ('_MSC_VER < 1900') section that '#define's 'snprintf'/'vsnprintf' to
// 'c99_snprintf'/'c99_vsnprintf' is a Windows-only compatibility shim that
// forwards to '_vsnprintf_s'/'_vscprintf'. On this target the standard
// 'snprintf'/'vsnprintf' (here, Rust 'format!') are used directly, so the shims
// are not ported. Their spec ids are carried verbatim:
// [spec:hfst:def:hfst-print-dot.hfst.c99-vsnprintf-fn]
// [spec:hfst:sem:hfst-print-dot.hfst.c99-vsnprintf-fn]
// [spec:hfst:def:hfst-print-dot.hfst.c99-snprintf-fn]
// [spec:hfst:sem:hfst-print-dot.hfst.c99-snprintf-fn]

// C++ 'HfstBasicTransducer mutt {t};' invokes the
// 'HfstBasicTransducer(const HfstTransducer&)' conversion constructor. The facade
// exposes it as 'HfstTransducer::get_basic_transducer'.
fn hfst_transducer_to_basic<B: crate::backend::Backend>(
    t: &HfstTransducer<B>,
) -> HfstBasicTransducer {
    t.get_basic_transducer()
        .expect("get_basic_transducer on a valid transducer cannot fail")
}

// [spec:hfst:def:hfst-print-dot.hfst.trim-to-valid-utf8-fn]
// [spec:hfst:sem:hfst-print-dot.hfst.trim-to-valid-utf8-fn]
fn trim_to_valid_utf8(inp: &mut Vec<u8>) {
    let len = inp.len();
    // C++ 'for (int i=1; i<4 && (len-i>0); i++)'. With 'len' a 'size_t',
    // 'len-i>0' equals 'len>i' for every non-empty buffer (the only inputs that
    // occur in practice); using 'len > i' avoids the C 'len==0' underflow OOB.
    let mut i: usize = 1;
    while i < 4 && len > i {
        if i < 2 && (inp[len - i] & 0xc0) == 0xc0 {
            // 'inp[len-i] = '\0'' truncates the C string at that byte.
            inp.truncate(len - i);
            return;
        } else if i < 3 && (inp[len - i] & 0xe0) == 0xe0 {
            inp.truncate(len - i);
            return;
        } else if i < 4 && (inp[len - i] & 0xf0) == 0xf0 {
            inp.truncate(len - i);
            return;
        }
        i += 1;
    }
}

// Build the target-state arc label for one transition, given the previously
// accumulated label for that same target. This is the per-arc body shared
// byte-for-byte by both 'print_dot' overloads (the 'snprintf' family always
// uses '%.2f' for the arc weight in both functions).
fn arc_label(old_label: &str, arc: &HfstBasicTransition, coder: &SymbolCoder) -> String {
    let mut first = arc.get_input_symbol(coder);
    let mut second = arc.get_output_symbol(coder);
    if first == internal_epsilon {
        first = Symbol::new_static("00");
    } else if first == internal_identity {
        first = Symbol::new_static("??");
    } else if first == internal_unknown {
        first = Symbol::new_static("?1");
    }
    if second == internal_epsilon {
        second = Symbol::new_static("00");
    } else if second == internal_identity {
        second = Symbol::new_static("??");
    } else if second == internal_unknown {
        second = Symbol::new_static("?2");
    }
    // The C++ allocates a 'DOT_MAX_LABEL_SIZE' byte buffer and 'snprintf's into
    // it; 'snprintf' never returns < 0 for these arguments, so the
    // 'HFST_THROW_MESSAGE(HfstException, "sprinting dot arc label")' branches are
    // unreachable. Truncation to 'DOT_MAX_LABEL_SIZE - 1' bytes is preserved.
    let formatted: String = if first == second {
        if arc.get_weight() > 0.0 {
            if !old_label.is_empty() {
                format!("{}, {}/{:.2}", old_label, first, arc.get_weight())
            } else {
                format!("{}/{:.2}", first, arc.get_weight())
            } // if old label empty
        } else {
            if !old_label.is_empty() {
                format!("{}, {}", old_label, first)
            } else {
                format!("{}", first)
            } // if old label empty
        } // if weighted
    } else {
        if arc.get_weight() > 0.0 {
            if !old_label.is_empty() {
                format!(
                    "{}, {}:{}/{:.2}",
                    old_label,
                    first,
                    second,
                    arc.get_weight()
                )
            } else {
                format!("{}:{}/{:.2}", first, second, arc.get_weight())
            } // old label empty
        } else {
            if !old_label.is_empty() {
                format!("{}, {}:{}", old_label, first, second)
            } else {
                format!("{}:{}", first, second)
            } // if old label empty
        } // if weighted
    }; // if id pair

    let mut l: Vec<u8> = formatted.into_bytes();
    l.truncate(DOT_MAX_LABEL_SIZE - 1);
    trim_to_valid_utf8(&mut l);
    // 'trim_to_valid_utf8' just dropped any partial trailing code point, so the
    // bytes are valid UTF-8 by construction (C++ 'string sl(l)' kept them raw
    // because 'std::string' isn't validated).
    let mut sl = String::from_utf8(l).expect("trim_to_valid_utf8 leaves valid UTF-8");
    replace_all(&mut sl, "\"", "\\\"");
    sl
}

// [spec:hfst:def:hfst-print-dot.hfst.print-dot-fn]
// [spec:hfst:sem:hfst-print-dot.hfst.print-dot-fn]
pub fn print_dot_file<B: crate::backend::Backend>(
    out: &mut dyn Write,
    t: &mut HfstTransducer<B>,
) -> std::io::Result<()> {
    //fprintf(out, "// This graph generated with hfst-fst2txt\n");
    if t.get_name() != "" {
        writeln!(out, "digraph \"{}\" {{", t.get_name())?;
    } else {
        writeln!(out, "digraph H {{")?;
    }
    writeln!(out, "charset = UTF8;")?;
    writeln!(out, "rankdir = LR;")?;
    writeln!(out, "node [shape=circle,style=filled,fillcolor=yellow]")?;
    let mutt: HfstBasicTransducer = hfst_transducer_to_basic(t);
    let mut s: HfstState = 0;
    // for some reason, dot works nicer if I first have all nodes, then arcs
    for _state in mutt.iter() {
        if mutt.is_final_state(s) {
            if mutt
                .get_final_weight(s)
                .expect("state was confirmed final via is_final_state")
                > 0.0
            {
                writeln!(
                    out,
                    "q{} [shape=doublecircle,label=\"q{}/\\n{:.2}\"] ",
                    s,
                    s,
                    mutt.get_final_weight(s)
                        .expect("state was confirmed final via is_final_state")
                )?;
            } else {
                writeln!(out, "q{} [shape=doublecircle,label=\"q{}\"] ", s, s)?;
            }
        } else {
            writeln!(out, "q{} [label=\"q{}\"] ", s, s)?;
        }
        s += 1;
    } // each state
    s = 0;
    for state in mutt.iter() {
        let mut target_labels: BTreeMap<HfstState, String> = BTreeMap::new();
        for arc in state {
            let old_label = target_labels
                .entry(arc.get_target_state())
                .or_default()
                .clone();
            let sl = arc_label(&old_label, arc, mutt.coder());
            target_labels.insert(arc.get_target_state(), sl);
        } // each arc
        for (key, value) in &target_labels {
            write!(out, "q{} -> q{} ", s, key)?;
            writeln!(out, "[label=\"{} \"];", value)?;
        }
        s += 1;
    } // each state
    writeln!(out, "}}")
}

pub fn print_dot_os<B: crate::backend::Backend>(out: &mut dyn Write, t: &mut HfstTransducer<B>) {
    // C++ 'out.precision(2)' sets the stream's general-format precision to 2
    // significant figures for '<<'-printed floats (the final-state weight
    // below). That is neither '%.2f' nor Rust '{}'; per the port convention we
    // print that weight with '{}' and note the divergence. (The arc weights go
    // through 'arc_label''s 'snprintf'-equivalent '%.2f' in both overloads.)

    //out << "// This graph generated with hfst-fst2txt" << std::endl;
    if t.get_name() != "" {
        let _ = writeln!(out, "digraph \"{}\" {{", t.get_name());
    } else {
        let _ = writeln!(out, "digraph H {{");
    }
    let _ = writeln!(out, "charset = UTF8;");
    let _ = writeln!(out, "rankdir = LR;");
    let _ = writeln!(out, "node [shape=circle,style=filled,fillcolor=yellow]");
    let mutt: HfstBasicTransducer = hfst_transducer_to_basic(t);
    let mut s: HfstState = 0;
    // for some reason, dot works nicer if I first have all nodes, then arcs
    for _state in mutt.iter() {
        if mutt.is_final_state(s) {
            if mutt
                .get_final_weight(s)
                .expect("state was confirmed final via is_final_state")
                > 0.0
            {
                let _ = writeln!(
                    out,
                    "q{} [shape=doublecircle,label=\"q{}/\\n{}\"] ",
                    s,
                    s,
                    mutt.get_final_weight(s)
                        .expect("state was confirmed final via is_final_state")
                );
            } else {
                let _ = writeln!(out, "q{} [shape=doublecircle,label=\"q{} \"] ", s, s);
            }
        } else {
            let _ = writeln!(out, "q{} [label=\"q{}\"] ", s, s);
        }
        s += 1;
    } // each state
    s = 0;
    for state in mutt.iter() {
        let mut target_labels: BTreeMap<HfstState, String> = BTreeMap::new();
        for arc in state {
            let old_label = target_labels
                .entry(arc.get_target_state())
                .or_default()
                .clone();
            let sl = arc_label(&old_label, arc, mutt.coder());
            target_labels.insert(arc.get_target_state(), sl);
        } // each arc
        for (key, value) in &target_labels {
            let _ = write!(out, "q{} -> q{} ", s, key);
            let _ = writeln!(out, "[label=\"{} \"];", value);
        }
        s += 1;
    } // each state
    let _ = writeln!(out, "}}");
}
