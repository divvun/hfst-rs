//! Faithful 1:1 port of tools/src/hfst-pmatch2fst.cc — the pmatch regular
//! expression compiling command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options) plus the hfst pmatch
//! compiler and the OL conversion functions.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch_compiler::PmatchCompiler;
use std::io::{Read, Write};

/// hfst-pmatch2fst's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// C: `static char *epsilonname = NULL;` ('-e, --epsilon').
    epsilonname: Option<String>,
    /// C: `static bool flatten = false;` ('--flatten').
    flatten: bool,
    /// C: `static bool include_cosine_distances = false;` ('--cosine-distances').
    include_cosine_distances: bool,
}

// C: the compilation format, chosen at compile time from the available
// back-ends. The Rust crate links the tropical OpenFST back-end.

// [spec:hfst:def:hfst-pmatch2fst.print-usage-fn]
// [spec:hfst:sem:hfst-pmatch2fst.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nCompile regular expressions into transducer(s)\n (Experimental version)\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "String and format options:\n  -e, --epsilon=EPS         Map EPS as zero\n      --flatten             Compile in all RTNs\n      --cosine-distances    When compiling Like() operations, include cosine distance info\n"
    );
    let _ = writeln!(msg);

    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\nIf EPS is not defined, the default representation of 0 is used\nWeights are currently not implemented.\n\n"
    );

    let _ = write!(
        msg,
        "Examples:\n  echo \"Define TOP  UppercaseAlpha Alpha* LC({{professor}}) EndTag(ProfName);\" | {} \n  create matcher that tags \"professor Chomsky\" as \"professor <ProfName>Chomsky</ProfName>\"\n\n",
        common.program_name
    );
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-pmatch2fst.parse-options-fn]
// [spec:hfst:sem:hfst-pmatch2fst.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "epsilon",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'e' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "flatten",
            has_arg: getopt::NO_ARGUMENT,
            val: '1' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "cosine-distances",
            has_arg: getopt::NO_ARGUMENT,
            val: '2' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own, then the terminal
        // error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match c as u8 as char {
            'e' => {
                options.epsilonname = opt.optarg_opt();
                continue;
            }
            '1' => {
                options.flatten = true;
                continue;
            }
            '2' => {
                options.include_cosine_distances = true;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-pmatch2fst.get-current-dir-name-fn]
// [spec:hfst:sem:hfst-pmatch2fst.get-current-dir-name-fn]
fn get_current_dir_name() -> String {
    // The C++ allocates a growing buffer and calls getcwd(); the Rust standard
    // library does the equivalent. On failure (the C++ EACCES throw, or any
    // other error) we return the empty string, matching the C++ fallback path.
    match std::env::current_dir() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => String::new(),
    }
}

// [spec:hfst:def:hfst-pmatch2fst.process-stream-fn]
// [spec:hfst:sem:hfst-pmatch2fst.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn Read,
) -> i32 {
    // pmatch is pinned to the tropical backend (the C++ compilation_format);
    // the former format argument is the type parameter now.
    let mut comp = PmatchCompiler::<hfst_openfst::StdVectorFst>::new();
    comp.set_verbose(common.verbose);
    comp.set_flatten(options.flatten);
    comp.set_include_cosine_distances(options.include_cosine_distances);
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut definitions: std::collections::HashMap<
        String,
        HfstTransducer<hfst_openfst::StdVectorFst>,
    > = std::collections::HashMap::new();

    let mut includedir = String::new();
    let inputfilename_str = &common.input_filename;
    // C: 'inputfile != stdin'. A real input file is in use only when the
    // input filename is a real name (not the "<stdin>" sentinel).
    if inputfilename_str != "<stdin>" && !inputfilename_str.is_empty() {
        if inputfilename_str.starts_with('/') {
            // absolute path
            includedir = inputfilename_str.clone();
        } else {
            let pwd = get_current_dir_name();
            includedir = format!("{}/{}", pwd, inputfilename_str);
        }
        match includedir.rfind('/') {
            None => {
                // mysterious, we'll just use the working dir
                includedir = String::new();
            }
            Some(slashpos) => {
                includedir = includedir[..slashpos + 1].to_string();
            }
        }
    }
    comp.set_include_path(includedir);

    // C: fgetc loop reading the whole input; read_to_end is the equivalent.
    let _ = input.read_to_end(&mut file_bytes);
    // C: std::string holds bytes; reinterpret the collected bytes as UTF-8.
    let file_contents = String::from_utf8_lossy(&file_bytes).into_owned();
    if file_contents.len() > 1 {
        // C wraps comp.compile in try/catch on HfstException; on a thrown
        // exception it prints e.name and returns EXIT_FAILURE. The Rust
        // compiler panics rather than throwing, so the catch arm is not
        // reproduced (any panic propagates).
        definitions = match comp.compile(&file_contents) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
    }

    // Harmonization + archive writing live in the library
    // ('hfst::pmatch_compiler::write_archive'); verbose progress goes to
    // stderr as before.
    match hfst::pmatch_compiler::write_archive(
        &mut definitions,
        outstream,
        common.verbose,
        &mut std::io::stderr(),
    ) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("{}: Empty ruleset, nothing to write", common.program_name);
            return 1;
        }
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    }
    outstream.close();
    0
}

// [spec:hfst:def:hfst-pmatch2fst.main-fn]
// [spec:hfst:sem:hfst-pmatch2fst.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "Pmatch2Fst");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    // close buffers, we use streams
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(
            &common.output_filename,
            ImplementationType::HFST_OLW_TYPE,
            true,
        )
    } else {
        HfstOutputStream::new(ImplementationType::HFST_OLW_TYPE, true)
    } {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-pmatch2fst: cannot open output: {e}");
            return 1;
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-pmatch2fst: cannot open input: {e}");
            return 1;
        }
    };
    process_stream(&common, &options, &mut outstream, &mut *input);
    0
}
