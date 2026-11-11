//! `hfst substitute`: relabelling arcs, by a label, a label file or a
//! transducer, and the name the tool gives what it relabelled. The engine
//! that does the relabelling is [`crate::substitute_driver`].

use crate::backend::AlgebraBackend;
use crate::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
use crate::hfst_transducer::HfstTransducer;
use crate::substitute_driver::{SubstituteEngine, SubstituteReporter, SubstituteRequest};
use crate::tools::metadata::{hfst_set_formula_unary, hfst_set_name_unary};

/// Why a line of a relabel file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelabelLineError {
    /// No tab between the two labels.
    NoTab,
    /// The label to replace is empty.
    EmptyFrom,
    EmptyTo,
}

impl std::fmt::Display for RelabelLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelabelLineError::NoTab => write!(f, "At least one tab required per line"),
            RelabelLineError::EmptyFrom => write!(
                f,
                "First field is empty;\nif you REALLY want to replace epsilons withsomething, use @0@ or {}",
                internal_epsilon
            ),
            RelabelLineError::EmptyTo => write!(
                f,
                "Second field seems empty;\nif you want to substitute something with epsilons, use @0@ or {}",
                internal_epsilon
            ),
        }
    }
}

/// A relabel file's refused line: its number, from 1, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelabelError {
    pub line: u32,
    pub error: RelabelLineError,
}

// [spec:hfst:req:embed.tools-in-library]
/// The `(from, to)` label pairs of a relabel file, `FROM<TAB>TO` a line, in
/// file order: empty lines and tab-less `#` comments are skipped, and the
/// first refused line ends the reading.
pub fn relabel_entries(text: &str) -> Result<Vec<(String, String)>, RelabelError> {
    let mut entries = Vec::new();
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let refuse = |error| RelabelError {
            line: index as u32 + 1,
            error,
        };
        if line.starts_with('\n') {
            continue;
        }
        let Some(tab) = line.find('\t') else {
            if line.starts_with('#') {
                continue;
            }
            return Err(refuse(RelabelLineError::NoTab));
        };
        let rest = &line[tab + 1..];
        let end = rest.find('\n').unwrap_or(rest.len());
        let (from, to) = (&line[..tab], &rest[..end]);
        if from.is_empty() {
            return Err(refuse(RelabelLineError::EmptyFrom));
        }
        if to.is_empty() {
            return Err(refuse(RelabelLineError::EmptyTo));
        }
        entries.push((from.to_string(), to.to_string()));
    }
    Ok(entries)
}

/// A label as `hfst substitute -f` and `-t` read one: `@0@` is the epsilon.
pub fn substitute_label(label: &str) -> String {
    if label == "@0@" {
        internal_epsilon.to_string()
    } else {
        label.to_string()
    }
}

/// What to substitute, and with what.
pub enum Substitution<'a, B: AlgebraBackend> {
    /// `-f FROM -t TO`: each a symbol or a colon pair. With `compose`, a
    /// symbol-for-symbol substitution is composed in rather than rewriting
    /// arcs.
    Label {
        from: &'a str,
        to: &'a str,
        compose: bool,
    },
    /// `-F FILE`: each entry of a relabel file, as [`relabel_entries`] reads
    /// it. They are applied together, unless `in_order`, which applies each
    /// in turn. `file_name` names the result.
    Relabel {
        entries: &'a [(String, String)],
        in_order: bool,
        compose: bool,
        file_name: &'a str,
    },
    /// `-f FROM -T FILE`: each arc FROM, a symbol or a pair, replaced by
    /// `to`. `file_name` names the result, and `to` in progress messages
    /// when it has no name.
    Transducer {
        from: &'a str,
        to: HfstTransducer<B>,
        file_name: &'a str,
    },
}

fn request(
    from: &str,
    to: Option<&str>,
    compose: bool,
    file_name: Option<&str>,
) -> SubstituteRequest {
    SubstituteRequest {
        from_label: Some(from.to_string()),
        from_pair: label_to_stringpair(from),
        to_label: to.map(str::to_string),
        to_pair: to.and_then(label_to_stringpair),
        to_transducer_filename: file_name.map(str::to_string),
        compose,
    }
}

/// What `hfst substitute` names its result after: the kind of substitution
/// and the labels or file it named.
#[derive(Clone, Copy, Debug)]
pub enum SubstitutionName<'a> {
    Label { from: &'a str, to: &'a str },
    Relabel { file_name: &'a str },
    Transducer { from: &'a str, file_name: &'a str },
}

impl<B: AlgebraBackend> Substitution<'_, B> {
    pub fn name(&self) -> SubstitutionName<'_> {
        match self {
            Substitution::Label { from, to, .. } => SubstitutionName::Label { from, to },
            Substitution::Relabel { file_name, .. } => SubstitutionName::Relabel { file_name },
            Substitution::Transducer {
                from, file_name, ..
            } => SubstitutionName::Transducer { from, file_name },
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// Name `t` as `hfst substitute` names what it relabelled.
pub fn name_substituted<B: AlgebraBackend>(t: &mut HfstTransducer<B>, name: SubstitutionName<'_>) {
    let (name, formula) = match name {
        SubstitutionName::Relabel { file_name } => (
            format!("substitute-from-{}", file_name),
            format!("\u{2672}{}", file_name),
        ),
        SubstitutionName::Label { from, to } => (
            format!("substitute-{}-with-{}", from, to),
            format!("{} \u{2672} {}", from, to),
        ),
        SubstitutionName::Transducer { from, file_name } => (
            format!("substitute-{}-with-net-{}", from, file_name),
            format!("{} \u{2672} {}", from, file_name),
        ),
    };
    hfst_set_name_unary(t, &name);
    hfst_set_formula_unary(t, &formula);
}

// [spec:hfst:def:hfst-substitute.process-stream-fn]
// [spec:hfst:sem:hfst-substitute.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// `hfst substitute` on one transducer: `t` relabelled by `substitution`
/// and named as the tool names it. Labels are taken as given, so `@0@` is
/// for the caller to read as the epsilon, as [`substitute_label`] does.
pub fn substitute<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    substitution: Substitution<'_, B>,
    reporter: &dyn SubstituteReporter,
) -> crate::error::Result<()> {
    match &substitution {
        Substitution::Label { from, to, compose } => {
            let mut engine = SubstituteEngine::new(None);
            engine.begin_transducer();
            engine.do_substitute(&request(from, Some(to), *compose, None), t, 1, reporter)?;
            if engine.is_delayed() {
                engine.perform_delayed(t, reporter)?;
            }
        }
        Substitution::Relabel {
            entries,
            in_order,
            compose,
            ..
        } => {
            let mut engine = SubstituteEngine::new(None);
            engine.begin_transducer();
            for (from, to) in entries.iter() {
                engine.apply_relabel_entry(
                    &request(from, Some(to), *compose, None),
                    t,
                    1,
                    *in_order,
                    reporter,
                )?;
            }
            engine.flush_batched(t, *in_order)?;
            if engine.is_delayed() {
                engine.perform_delayed(t, reporter)?;
            }
        }
        Substitution::Transducer {
            from,
            to,
            file_name,
        } => {
            let mut engine = SubstituteEngine::new(Some(HfstTransducer::new_copy(to)?));
            engine.begin_transducer();
            engine.do_substitute(&request(from, None, false, Some(file_name)), t, 1, reporter)?;
        }
    }
    name_substituted(t, substitution.name());
    Ok(())
}
