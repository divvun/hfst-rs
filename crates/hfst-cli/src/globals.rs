//! Shared tool state, ported from tools/src/inc/globals-common.h +
//! globals-unary.h + globals-binary.h.
//!
//! In C these fragments are '#include'd once per tool .cc as file-scope globals.
//! The idiomatic Rust port has NO globals: the state lives in a [`CommonOptions`]
//! value that each tool's `parse_options` builds and threads into its
//! processing functions (the getopt parser state lives likewise in
//! [`crate::hfst_getopt::Getopt`]). The "<stdin>"/"<stdout>" sentinels (or "-",
//! or an unset name) select the standard streams.

use std::io::{BufRead, BufReader, Write};

// colour tristate (hfst-commandline.h enum colour_tristate). Variant names kept
// SCREAMING bug-for-bug (matches the crate's ImplementationType style).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColourTristate {
    COLOUR_NEVER,
    COLOUR_ALWAYS,
    COLOUR_AUTO,
}

/// The shared command-line state every tool carries: the common (`-v/-q/-o/…`),
/// unary (`-i`) and binary (`-1/-2/-C`) option fields plus the resolved
/// filenames and tool metadata. Built by `parse_options` (via the
/// `crate::inc` case handlers) and threaded into the tool's processing
/// functions — the idiomatic replacement for the former file-scope `static mut`
/// globals.
#[derive(Clone, Debug)]
pub struct CommonOptions {
    // common (globals-common.h)
    pub verbose: bool,
    pub silent: bool,
    pub debug: bool,
    pub colour: ColourTristate,
    /// C: `FILE *message_out = stdout`, switched to stderr when the tool's data
    /// output is itself stdout so messages do not corrupt the data stream.
    pub message_to_stderr: bool,
    pub output_named: bool,
    // unary (globals-unary.h)
    pub input_named: bool,
    // binary (globals-binary.h)
    pub first_named: bool,
    pub second_named: bool,
    pub is_input_stdin: bool,
    pub allow_transducer_conversion: bool,
    // tool metadata + resolved filenames. An empty / "<stdin>" / "<stdout>" /
    // "-" filename selects the standard stream.
    pub program_name: String,
    pub hfst_tool_version: String,
    pub hfst_tool_wikiname: String,
    pub output_filename: String,
    pub input_filename: String,
    pub first_filename: String,
    pub second_filename: String,
}

impl Default for CommonOptions {
    fn default() -> CommonOptions {
        CommonOptions {
            verbose: false,
            silent: false,
            debug: false,
            colour: ColourTristate::COLOUR_AUTO,
            message_to_stderr: false,
            output_named: false,
            input_named: false,
            first_named: false,
            second_named: false,
            is_input_stdin: true,
            allow_transducer_conversion: true,
            program_name: String::new(),
            hfst_tool_version: String::new(),
            hfst_tool_wikiname: String::new(),
            output_filename: String::new(),
            input_filename: String::new(),
            first_filename: String::new(),
            second_filename: String::new(),
        }
    }
}

impl CommonOptions {
    /// Primary text input (`input_filename`). The std counterpart of the old
    /// `inputfile()` FILE*.
    pub fn input_reader(&self) -> std::io::Result<Box<dyn BufRead>> {
        reader_for(&self.input_filename)
    }

    /// First binary-tool input (`first_filename`), the counterpart of `firstfile()`.
    pub fn first_reader(&self) -> std::io::Result<Box<dyn BufRead>> {
        reader_for(&self.first_filename)
    }

    /// Second binary-tool input (`second_filename`), the counterpart of `secondfile()`.
    pub fn second_reader(&self) -> std::io::Result<Box<dyn BufRead>> {
        reader_for(&self.second_filename)
    }

    /// Primary text output (`output_filename`): stdout for the "<stdout>"/"-"/unset
    /// name, else the named file. The std counterpart of the old `outfile()` FILE*.
    pub fn output_writer(&self) -> std::io::Result<Box<dyn Write>> {
        let name = &self.output_filename;
        // Buffered in both arms: raw Stdout locks and newline-scans per write, a
        // raw File is a syscall per write; the C FILE* these replace buffered.
        if name == "<stdout>" || name == "-" || name.is_empty() {
            Ok(Box::new(std::io::BufWriter::new(std::io::stdout())))
        } else {
            Ok(Box::new(std::io::BufWriter::new(std::fs::File::create(
                name,
            )?)))
        }
    }

    /// Diagnostic/help output: stderr when the tool's data output is stdout (so
    /// the streams don't mix), otherwise stdout. Counterpart of `message_out()`.
    pub fn message_writer(&self) -> Box<dyn Write> {
        if self.message_to_stderr {
            Box::new(std::io::stderr())
        } else {
            Box::new(std::io::stdout())
        }
    }
}

// Open a named text input as a buffered reader; the "<stdin>"/"-"/unset names
// select stdin.
fn reader_for(name: &str) -> std::io::Result<Box<dyn BufRead>> {
    if name == "<stdin>" || name == "-" || name.is_empty() {
        Ok(Box::new(BufReader::new(std::io::stdin())))
    } else {
        Ok(Box::new(BufReader::new(std::fs::File::open(name)?)))
    }
}
