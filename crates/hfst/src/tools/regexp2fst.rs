//! `hfst regexp2fst`: splitting a file of regular expressions, one per line
//! or separated by semicolons, and compiling each, or their disjunction.

use std::io::BufRead;

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::HfstTransducer;
use crate::tools::metadata::hfst_set_name;
use crate::xre::XreCompiler;

/// How the expressions in a file are separated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Separation {
    Lines,
    Semicolons,
}

/// What a compile hands its caller as it goes: a progress message the tool
/// prints when verbose, or a transducer to write.
pub enum RegexpEvent<B: AlgebraBackend> {
    Progress(String),
    Compiled(HfstTransducer<B>),
}

/// Why the expressions did not compile. Each `Display`s as the message the
/// tool prints for it.
#[derive(Clone, Debug)]
pub enum RegexpError {
    /// No line held an expression.
    OnlyComments { source: String },
    /// The first expression is only whitespace and comments.
    FirstOnlyComments { source: String },
    /// The expression on this line does not compile.
    LineFailed { line: u32 },
    /// The expression this many semicolons in does not compile.
    ExpressionFailed { source: String, number: u32 },
    /// An operation on the compiled transducers failed, or the caller's
    /// handling of one did.
    Operation(crate::error::Error),
}

impl std::fmt::Display for RegexpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegexpError::OnlyComments { source } => write!(
                f,
                "{}: XRE parsing failed: input contains only whitespace or comments",
                source
            ),
            RegexpError::FirstOnlyComments { source } => write!(
                f,
                "{}: XRE parsing failed: expression #1 contains only whitespace or comments",
                source
            ),
            RegexpError::LineFailed { .. } => writeln!(f, "XRE parsing failed"),
            RegexpError::ExpressionFailed { source, number } => write!(
                f,
                "{}: XRE parsing failed in expression #{} separated by semicolons",
                source, number
            ),
            RegexpError::Operation(e) => write!(f, "{}", e),
        }
    }
}

impl From<crate::error::Error> for RegexpError {
    fn from(e: crate::error::Error) -> Self {
        RegexpError::Operation(e)
    }
}

/// The transducers compiled so far, joined into one when they are disjuncted.
struct Results<'r, B: AlgebraBackend> {
    disjunction: Option<HfstTransducer<B>>,
    harmonize: bool,
    each: &'r mut dyn FnMut(RegexpEvent<B>) -> crate::error::Result<()>,
}

impl<B: AlgebraBackend> Results<'_, B> {
    fn add(&mut self, mut compiled: HfstTransducer<B>) -> crate::error::Result<()> {
        match &mut self.disjunction {
            Some(disjunction) => {
                disjunction.disjunct(&compiled, self.harmonize)?;
                Ok(())
            }
            None => {
                hfst_set_name(&mut compiled, "?", "xre");
                (self.each)(RegexpEvent::Compiled(compiled))
            }
        }
    }

    fn finish(mut self) -> crate::error::Result<()> {
        match self.disjunction.take() {
            Some(mut disjunction) => {
                hfst_set_name(&mut disjunction, "?", "xre");
                (self.each)(RegexpEvent::Compiled(disjunction))
            }
            None => Ok(()),
        }
    }
}

fn compile_lines<B: AlgebraBackend>(
    xre: &mut XreCompiler<B>,
    input: &mut dyn BufRead,
    results: &mut Results<'_, B>,
) -> Result<(), RegexpError> {
    let mut any_expression = false;
    let mut line_count: u32 = 0;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let expression = line.trim_start_matches(['\n', '\r', ' ']);
        line_count += 1;
        if expression.is_empty() {
            (results.each)(RegexpEvent::Progress(format!(
                "Skipping whitespace expression #{}",
                line_count
            )))?;
            continue;
        }
        (results.each)(RegexpEvent::Progress(format!(
            "Compiling expression {}\n",
            line_count
        )))?;
        let Some(compiled) = xre.compile(expression) else {
            if !xre.contained_only_comments() {
                return Err(RegexpError::LineFailed { line: line_count });
            }
            continue;
        };
        any_expression = true;
        results.add(compiled)?;
    }
    if !any_expression {
        return Err(RegexpError::OnlyComments {
            source: xre.source_name.clone(),
        });
    }
    Ok(())
}

fn compile_semicolon_separated<B: AlgebraBackend>(
    xre: &mut XreCompiler<B>,
    input: &mut dyn BufRead,
    results: &mut Results<'_, B>,
) -> Result<(), RegexpError> {
    let mut content = String::new();
    let _ = input.read_to_string(&mut content);
    let mut offset: usize = 0;
    let mut number: u32 = 0;
    loop {
        number += 1;
        (results.each)(RegexpEvent::Progress(format!(
            "Compiling expression #{}\n",
            number as i32
        )))?;
        let mut chars_read: u32 = 0;
        let compiled = xre.compile_first_at(&content, offset, &mut chars_read);
        match compiled {
            Some(compiled) => results.add(compiled)?,
            None if !xre.contained_only_comments() => {
                return Err(RegexpError::ExpressionFailed {
                    source: xre.source_name.clone(),
                    number,
                });
            }
            None if number == 1 => {
                return Err(RegexpError::FirstOnlyComments {
                    source: xre.source_name.clone(),
                });
            }
            None => break,
        }
        offset += chars_read as usize;
        if offset >= content.len() {
            break;
        }
    }
    Ok(())
}

// [spec:hfst:def:hfst-regexp2fst.process-stream-fn]
// [spec:hfst:sem:hfst-regexp2fst.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Compile the expressions `input` holds with `xre`, as `hfst regexp2fst`
/// does, handing `each` every transducer to write, named `xre(?)`, as it is
/// compiled: one per expression, or with `disjunct` their disjunction at the
/// end, harmonized when `harmonize`. `each` hears the tool's progress
/// messages too. Failures name the source as `xre`'s source name.
pub fn compile_regexps<B: AlgebraBackend>(
    xre: &mut XreCompiler<B>,
    input: &mut dyn BufRead,
    separation: Separation,
    (disjunct, harmonize): (bool, bool),
    each: &mut dyn FnMut(RegexpEvent<B>) -> crate::error::Result<()>,
) -> Result<(), RegexpError> {
    let mut results = Results {
        disjunction: disjunct.then(HfstTransducer::new),
        harmonize,
        each,
    };
    match separation {
        Separation::Lines => compile_lines(xre, input, &mut results)?,
        Separation::Semicolons => compile_semicolon_separated(xre, input, &mut results)?,
    }
    results.finish()?;
    Ok(())
}
