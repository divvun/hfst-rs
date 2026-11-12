//! `hfst txt2fst`: transducers read from AT&T or Prolog text.

use std::io::BufRead;

use crate::backend::AlgebraBackend;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_transducer::HfstTransducer;
use crate::tools::metadata::{hfst_set_formula, hfst_set_name};

/// `hfst txt2fst`'s options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Txt2FstOptions {
    /// The text is Prolog rather than AT&T: `--prolog`.
    pub prolog: bool,
    /// What AT&T text spells the epsilon as: `--epsilon`.
    pub epsilon: String,
    /// `--check-negative-epsilon-cycles`, whose check is the costly part of
    /// a large transducer's read.
    pub check_negative_epsilon_cycles: bool,
    /// Tell of each negative weight in AT&T text.
    pub warn_negative_weights: bool,
    /// One transducer, the union of the AT&T text's, rather than each:
    /// `--disjunct`.
    pub disjunct: bool,
}

impl Default for Txt2FstOptions {
    fn default() -> Self {
        Txt2FstOptions {
            prolog: false,
            epsilon: "@0@".to_string(),
            check_negative_epsilon_cycles: false,
            warn_negative_weights: true,
            disjunct: false,
        }
    }
}

/// What reading the text hands its caller as it goes.
pub enum TextEvent<B: AlgebraBackend> {
    /// A message the tool prints when verbose.
    Progress(String),
    /// A negative weight on the line numbered `line`, counted through the
    /// whole text.
    NegativeWeight { weight: f32, line: u32 },
    /// The transducer just read has epsilon cycles with a negative weight.
    NegativeEpsilonCycles,
    /// A transducer, named as the tool writes it.
    Read(HfstTransducer<B>),
}

type Each<'e, B> = &'e mut dyn FnMut(TextEvent<B>) -> crate::error::Result<()>;

fn check_epsilon_cycles<B: AlgebraBackend>(
    fsm: &HfstBasicTransducer,
    each: Each<'_, B>,
) -> crate::error::Result<()> {
    each(TextEvent::Progress(
        "Checking if the transducer has epsilon cycles with a negative weight...\n".to_string(),
    ))?;
    if fsm.has_negative_epsilon_cycles() {
        each(TextEvent::NegativeEpsilonCycles)
    } else {
        each(TextEvent::Progress(
            "No epsilon cycles with a negative weight detected...\n".to_string(),
        ))
    }
}

/// Read one AT&T transducer, telling `each` of its negative weights when
/// told to.
fn read_att<B: AlgebraBackend>(
    input: &mut dyn BufRead,
    options: &Txt2FstOptions,
    linecount: &mut u32,
    each: Each<'_, B>,
) -> crate::error::Result<HfstBasicTransducer> {
    let mut negatives = Vec::new();
    let net = HfstBasicTransducer::read_in_att_format_noting(
        input,
        &options.epsilon,
        linecount,
        &mut |weight, line| {
            if options.warn_negative_weights {
                negatives.push((weight, line));
            }
        },
    );
    for (weight, line) in negatives {
        each(TextEvent::NegativeWeight { weight, line })?;
    }
    net
}

/// Whether nothing more is left to read.
fn at_end(input: &mut dyn BufRead) -> bool {
    input.fill_buf().map_or(true, <[u8]>::is_empty)
}

// [spec:hfst:def:hfst-txt2fst.process-stream-fn]
// [spec:hfst:sem:hfst-txt2fst.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
/// Read the transducers of `input`, text from the file `name`, as
/// `hfst txt2fst` reads them, handing `each` every one to write, named and
/// given a formula after `name`, and what the tool prints as it goes. With
/// `disjunct`, the AT&T transducers are joined into one, unnamed.
pub fn read_text<B: AlgebraBackend>(
    input: &mut dyn BufRead,
    name: &str,
    options: &Txt2FstOptions,
    each: Each<'_, B>,
) -> crate::error::Result<()> {
    let mut transducer_n: usize = 0;
    let mut linecount: u32 = 0;
    let named = |mut t: HfstTransducer<B>| {
        hfst_set_name(&mut t, name, "text");
        hfst_set_formula(&mut t, name, "T");
        t
    };
    while !at_end(input) {
        transducer_n += 1;
        each(TextEvent::Progress(if transducer_n < 2 {
            "Reading transducer table...\n".to_string()
        } else {
            format!("Reading transducer table {transducer_n}...\n")
        }))?;
        if options.prolog {
            let fsm = HfstBasicTransducer::read_in_prolog_format(input, &mut linecount)?;
            if options.check_negative_epsilon_cycles {
                check_epsilon_cycles(&fsm, each)?;
            }
            each(TextEvent::Read(named(HfstTransducer::new_from_basic(
                &fsm,
            )?)))?;
        } else if options.disjunct {
            let mut joined: HfstTransducer<B> = HfstTransducer::new();
            while !at_end(input) {
                let net = read_att(input, options, &mut linecount, each)?;
                joined.disjunct(&HfstTransducer::new_from_basic(&net)?, true)?;
            }
            each(TextEvent::Read(joined))?;
        } else {
            let net = read_att(input, options, &mut linecount, each)?;
            let t = named(HfstTransducer::new_from_basic(&net)?);
            if options.check_negative_epsilon_cycles {
                check_epsilon_cycles(&t.to_basic()?, each)?;
            }
            each(TextEvent::Read(t))?;
        }
    }
    Ok(())
}
