//! Faithful 1:1 port of tools/src/hfst-tokenize.cc — a replacement for
//! hfst-proc using pmatch: perform matching/lookup/tokenization on text
//! streams. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments) and the hfst optimized-lookup pmatch
//! tokenizer ('hfst::pmatch_tokenize', 'hfst::pmatch', 'hfst::pmatch_compiler').
//!
//! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
//! but like hfst-pmatch it does not use the usual unary
//! HfstInputStream/HfstOutputStream pipeline for output: it reads its single
//! positional argument as the ruleset archive filename, reads lines of stdin
//! (via 'inputfile'), and prints to stdout.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_tokenize::{
    OutputFormat, TokenizeSettings, match_and_print, print_nonmatching_sequence,
};
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_getdelim, hfst_getline, hfst_set_program_name,
    hfst_setlocale, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, hfst_getopt_common_long, print_common_program_options,
};
use hfst_cli::inc::{CaseResult, handle_common_case, handle_error_case};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::io::Write;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// 'inputfile' as it is reached through the globals-unary.h include.
fn inputfile() -> *mut libc::FILE {
    globals::inputfile()
}

// File-scope tool state (the C++ file-scope statics).
static mut SUPERBLANKS: bool = false; // Input is apertium-style superblanks
// (overrides blankline_separated)
static mut BLANKLINE_SEPARATED: bool = true; // Input is separated by blank lines
// (as opposed to single newlines)
static mut KEEP_NEWLINES: bool = false;
#[allow(dead_code)]
static mut TOKEN_NUMBER: c_int = 1;
static mut TOKENIZER_FILENAME: String = String::new();
const DEFAULT_FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

// 'static TokenizeSettings settings;' — held as a process-global. Default()
// mirrors the C++ default-constructed TokenizeSettings.
static mut SETTINGS: Option<TokenizeSettings> = None;

fn settings() -> &'static mut TokenizeSettings {
    unsafe {
        let ptr = &raw mut SETTINGS;
        if (*ptr).is_none() {
            *ptr = Some(TokenizeSettings::default());
        }
        (*ptr).as_mut().unwrap()
    }
}

// [spec:hfst:def:hfst-tokenize.print-usage-fn]
// [spec:hfst:sem:hfst-tokenize.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [--segment | --xerox | --cg | --giella-cg] [OPTIONS...] RULESET\nperform matching/lookup on text streams\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "  -n, --newline            Newline as input separator (default is blank line)\n\
             \x20 -a, --print-all          Print nonmatching text\n\
             \x20 -w, --print-weight       Print weights (overrides earlier -W option)\n\
             \x20 -W, --no-weights         Don't print weights (default; overrides earlier -w, or -w implied by -g, options)\n\
             \x20 -m, --tokenize-multichar Tokenize multicharacter symbols\n\
             \x20                          (by default only one grapheme is tokenized at a time\n\
             \x20                          regardless of what is present in the alphabet)\n\
             \x20 -b, --beam=B             Output only analyses whose weight is within B from best result\n\
             \x20 -tS, --time-cutoff=S     Limit search after having used S seconds per input\n\
             \x20 -lN, --weight-classes=N  Output no more than N best weight classes\n\
             \x20                          (where analyses with equal weight constitute a class\n\
             \x20 -u, --unique             Remove duplicate analyses\n\
             \x20 -z, --segment            Segmenting / tokenization mode (default)\n\
             \x20 -i, --space-separated    Tokenization with one sentence per line, space-separated tokens\n\
             \x20 -x, --xerox              Xerox output\n\
             \x20 -c, --cg                 Constraint Grammar output\n\
             \x20 -S, --superblanks        Ignore contents of unescaped [] (cf. apertium-destxt); flush on NUL\n\
             \x20 -g, --giella-cg          CG format used in Giella infrastructure (implies -w and -l2,\n\
             \x20                          treats @PMATCH_INPUT_MARK@ as subreading separator,\n\
             \x20                          expects tags to be Multichar_symbols, flush on NUL)\n\
             \x20 -C  --conllu             CoNLL-U format\n\
             \x20 -f, --finnpos            FinnPos output\n\
             \x20 -L, --visl               VISL input and output (implies -W, handles <s> as blocks and <STYLE> inline)\n",
        );
        fput(
            globals::message_out(),
            "Use standard streams for input and output (for now).\n\n",
        );

        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
        fput(globals::message_out(), "\n");
    }
}

// [spec:hfst:def:hfst-tokenize.make-naive-tokenizer-fn]
// [spec:hfst:sem:hfst-tokenize.make-naive-tokenizer-fn]
unsafe fn make_naive_tokenizer(dictionary: &mut HfstTransducer) -> PmatchContainer {
    unsafe {
        let mut word_boundary =
            hfst::pmatch_compiler::PmatchUtilityTransducers::make_latin1_whitespace_acceptor(
                DEFAULT_FORMAT,
            );
        let punctuation =
            hfst::pmatch_compiler::PmatchUtilityTransducers::make_latin1_punct_acceptor(
                DEFAULT_FORMAT,
            );
        word_boundary.disjunct(&punctuation, true);
        let mut others = hfst::pmatch_compiler::make_exc_list(&word_boundary, DEFAULT_FORMAT);
        others.repeat_plus();
        // make the default token less likely than any dictionary token
        others.set_final_weights(f32::MAX, false);
        let mut word_boundary_list =
            hfst::pmatch_compiler::make_list(&word_boundary, DEFAULT_FORMAT);
        // @BOUNDARY@ is pmatch's special input boundary marker
        let boundary = HfstTransducer::new_symbol("@BOUNDARY@", DEFAULT_FORMAT);
        word_boundary_list.disjunct(&boundary, true);
        let mut left_context = HfstTransducer::new_symbol_pair(
            hfst::hfst_symbol_defs::internal_epsilon,
            hfst::pmatch_compiler::LC_ENTRY_SYMBOL,
            DEFAULT_FORMAT,
        );
        let mut right_context = HfstTransducer::new_symbol_pair(
            hfst::hfst_symbol_defs::internal_epsilon,
            hfst::pmatch_compiler::RC_ENTRY_SYMBOL,
            DEFAULT_FORMAT,
        );
        left_context.concatenate(&word_boundary_list, true);
        right_context.concatenate(&word_boundary_list, true);
        let left_context_exit = HfstTransducer::new_symbol_pair(
            hfst::hfst_symbol_defs::internal_epsilon,
            hfst::pmatch_compiler::LC_EXIT_SYMBOL,
            DEFAULT_FORMAT,
        );
        let right_context_exit = HfstTransducer::new_symbol_pair(
            hfst::hfst_symbol_defs::internal_epsilon,
            hfst::pmatch_compiler::RC_EXIT_SYMBOL,
            DEFAULT_FORMAT,
        );
        left_context.concatenate(&left_context_exit, true);
        right_context.concatenate(&right_context_exit, true);
        let mut dict_name = dictionary.get_name();
        if dict_name.is_empty() {
            dict_name = "unknown_pmatch_tokenized_dict".to_string();
            dictionary.set_name(&dict_name);
        }
        let dict_name_c = CString::new(dict_name.clone()).unwrap();
        let dict_ins_arc = HfstTransducer::new_symbol(
            &hfst::pmatch_compiler::get_Ins_transition(dict_name_c.as_ptr()),
            DEFAULT_FORMAT,
        );
        // We now make the center of the tokenizer
        others.disjunct(&dict_ins_arc, true);
        // And combine it with the context conditions
        left_context.concatenate(&others, true);
        left_context.concatenate(&right_context, true);
        // Because there are context conditions we need delimiter markers
        let mut tokenizer = hfst::pmatch_compiler::add_pmatch_delimiters(&left_context);
        tokenizer.set_name("TOP");
        tokenizer.minimize();
        // Convert the dictionary to olw if it wasn't already
        dictionary.convert(ImplementationType::HFST_OLW_TYPE, String::new());
        // Get the alphabets
        let dict_syms = dictionary.get_alphabet();
        let tokenizer_syms = tokenizer.get_alphabet();
        // What to add to the dictionary
        let tokenizer_minus_dict: Vec<String> =
            tokenizer_syms.difference(&dict_syms).cloned().collect();
        for it in tokenizer_minus_dict.iter() {
            dictionary.insert_to_alphabet(it.as_str());
        }
        let tokenizer_basic =
            hfst::convert_transducer_format::ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(
                &tokenizer,
            );
        // The C++ 'hfst_basic_transducer_to_hfst_ol' takes the HfstTransducer
        // dictionary directly as its harmonizer and converts it internally; the
        // Rust port shifts that to the caller, so we first obtain the dictionary's
        // optimized-lookup backend ('hfst_transducer_to_hfst_ol' converts the
        // dictionary in place to HFST_OLW and returns its ol::Transducer) and pass
        // that as the harmonizer. This is also the same backend used for add_rtn
        // below.
        let dict_backend =
            hfst::convert_transducer_format::ConversionFunctions::hfst_transducer_to_hfst_ol(
                dictionary,
            );
        let tokenizer_ol =
            hfst::convert_transducer_format::ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &*tokenizer_basic,
                true,                 // weighted
                "",                   // no special options
                Some(&*dict_backend), // harmonize with the dictionary
            );
        drop(Box::from_raw(tokenizer_basic));
        let mut retval = PmatchContainer::new_from_transducer(Box::new(tokenizer_ol));
        retval.add_rtn(&*dict_backend, &dict_name);
        retval
    }
}

// TODO: lambda this when C++11 available everywhere
// [spec:hfst:def:hfst-tokenize.process-input-0delim-print-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-0delim-print-fn]
fn process_input_0delim_print(
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
    cur: &mut String,
) {
    let input_text = cur.clone();
    if !input_text.is_empty() {
        match_and_print(container, outstream, &input_text, settings());
    }
    cur.clear();
}

// [spec:hfst:def:hfst-tokenize.trim-fn]
// [spec:hfst:sem:hfst-tokenize.trim-fn]
fn trim(s: &mut String) {
    while !s.is_empty() {
        let c = *s.as_bytes().last().unwrap();
        if (c as char).is_ascii_whitespace() || c == 0 {
            s.pop();
        } else {
            break;
        }
    }
    while !s.is_empty() {
        let c = s.as_bytes()[0];
        if (c as char).is_ascii_whitespace() || c == 0 {
            s.remove(0);
        } else {
            break;
        }
    }
}

// [spec:hfst:def:hfst-tokenize.process-input-visl-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-visl-fn]
unsafe fn process_input_visl(container: &mut PmatchContainer, outstream: &mut dyn Write) -> c_int {
    unsafe {
        let mut bufsize: usize = 0;
        let mut buffer: *mut c_char = std::ptr::null_mut();

        let mut len: isize;
        loop {
            len = hfst_getline(&mut buffer, &mut bufsize, inputfile());
            if !(len > 0) {
                break;
            }
            let mut line = bytes_to_string(buffer, len as usize);
            trim(&mut line);
            if !line.is_empty() {
                if line.as_bytes()[0] == b'<' && *line.as_bytes().last().unwrap() == b'>' {
                    print_nonmatching_sequence(&line, outstream, settings());
                } else {
                    match_and_print(container, outstream, &line, settings());
                }
            } else {
                let _ = write!(outstream, "\n");
            }
            let _ = outstream.flush();

            *buffer = 0;
            len = 0;

            if libc::feof(inputfile()) != 0 {
                break;
            }
        }

        if len < 0 {
            len = 0;
        }

        let mut line = bytes_to_string(buffer, len as usize);
        trim(&mut line);
        if !line.is_empty() {
            if line.as_bytes()[0] == b'<' && *line.as_bytes().last().unwrap() == b'>' {
                print_nonmatching_sequence(&line, outstream, settings());
            } else {
                match_and_print(container, outstream, &line, settings());
            }
        }
        let _ = outstream.flush();

        libc::free(buffer as *mut libc::c_void);
        libc::EXIT_SUCCESS
    }
}

// Build a Rust String from the first 'len' bytes of a C buffer (mirrors
// std::string::assign(buffer, buffer + len)).
unsafe fn bytes_to_string(buffer: *const c_char, len: usize) -> String {
    if buffer.is_null() || len == 0 {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(buffer as *const u8, len) };
    String::from_utf8_lossy(slice).into_owned()
}

// 'template <bool do_superblank>'
// [spec:hfst:def:hfst-tokenize.process-input-0delim-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-0delim-fn]
unsafe fn process_input_0delim(
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
    do_superblank: bool,
) -> c_int {
    unsafe {
        let mut line: *mut c_char = std::ptr::null_mut();
        let mut bufsize: usize = 0;
        let mut in_blank = false;
        let mut cur = String::new();
        loop {
            let len = hfst_getdelim(&mut line, &mut bufsize, b'\0' as c_int, inputfile());
            if !(len > 0) {
                break;
            }
            let bytes = std::slice::from_raw_parts(line as *const u8, len as usize);
            let mut escaped = false; // beginning of line is necessarily unescaped
            let mut i: isize = 0;
            while i < len {
                let ch = bytes[i as usize];
                if escaped {
                    cur.push(ch as char);
                    escaped = false;
                    i += 1;
                    continue;
                } else if do_superblank && !in_blank && ch == b'[' {
                    process_input_0delim_print(container, outstream, &mut cur);
                    cur.push(ch as char);
                    in_blank = true;
                } else if do_superblank && in_blank && ch == b']' {
                    cur.push(ch as char);
                    if i + 1 < len && bytes[(i + 1) as usize] == b'[' {
                        // Join consecutive superblanks
                        i += 1;
                        cur.push(bytes[i as usize] as char);
                    } else {
                        in_blank = false;
                        print_nonmatching_sequence(&cur, outstream, settings());
                        cur.clear();
                    }
                } else if !in_blank && ch == b'\n' {
                    cur.push(ch as char);
                    if globals::VERBOSE {
                        println!("processing: {}\\n", cur);
                    }
                    process_input_0delim_print(container, outstream, &mut cur);
                } else if ch == b'\0' {
                    if globals::VERBOSE {
                        println!("processing: {}\\0", cur);
                    }
                    process_input_0delim_print(container, outstream, &mut cur);
                    let _ = writeln!(outstream, "<STREAMCMD:FLUSH>"); // CG format uses this instead of \0
                    if outstream.flush().is_err() {
                        eprintln!("hfst-tokenize: Could not flush file");
                    }
                } else {
                    cur.push(ch as char);
                }
                escaped = ch == b'\\';
                i += 1;
            }
            libc::free(line as *mut libc::c_void);
            line = std::ptr::null_mut();
            if libc::feof(inputfile()) != 0 {
                break;
            }
        }
        if in_blank {
            print_nonmatching_sequence(&cur, outstream, settings());
        } else {
            process_input_0delim_print(container, outstream, &mut cur);
        }
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-tokenize.maybe-erase-newline-fn]
// [spec:hfst:sem:hfst-tokenize.maybe-erase-newline-fn]
fn maybe_erase_newline(input_text: &mut String) {
    unsafe {
        if !KEEP_NEWLINES
            && !input_text.is_empty()
            && *input_text.as_bytes().last().unwrap() == b'\n'
        {
            // Remove final newline
            input_text.pop();
        }
    }
}

// [spec:hfst:def:hfst-tokenize.process-input-fn]
// [spec:hfst:sem:hfst-tokenize.process-input-fn]
unsafe fn process_input(container: &mut PmatchContainer, outstream: &mut dyn Write) -> c_int {
    unsafe {
        // (The C++ sets std::fixed/setprecision(10) on the stream for cg/giellacg/
        // visl; the library print functions format weights themselves, so there is
        // no stream-wide formatting flag to mirror here.)
        if settings().output_format == OutputFormat::giellacg || SUPERBLANKS {
            if SUPERBLANKS {
                verbose_printf("Processign giellacg with superblanks\n");
                return process_input_0delim(container, outstream, true);
            } else {
                verbose_printf("Processign giellacg without superblanks\n");
                return process_input_0delim(container, outstream, false);
            }
        }
        if settings().output_format == OutputFormat::visl {
            verbose_printf("Processign VISL CG 3\n");
            return process_input_visl(container, outstream);
        }
        let mut input_text = String::new();
        let mut line: *mut c_char = std::ptr::null_mut();
        let mut bufsize: usize = 0;
        if BLANKLINE_SEPARATED {
            verbose_printf("Processing blankline separated input\n");
            while hfst_getline(&mut line, &mut bufsize, inputfile()) > 0 {
                if *(line as *const u8) == b'\n' {
                    maybe_erase_newline(&mut input_text);
                    match_and_print(container, outstream, &input_text, settings());
                    input_text.clear();
                } else {
                    input_text.push_str(&cstr(line));
                }
                libc::free(line as *mut libc::c_void);
                line = std::ptr::null_mut();
            }
            if !input_text.is_empty() {
                maybe_erase_newline(&mut input_text);
                match_and_print(container, outstream, &input_text, settings());
            }
        } else {
            // newline or non-separated
            verbose_printf("Processing non-separated input\n");
            while hfst_getline(&mut line, &mut bufsize, inputfile()) > 0 {
                input_text = cstr(line);
                maybe_erase_newline(&mut input_text);
                match_and_print(container, outstream, &input_text, settings());
                libc::free(line as *mut libc::c_void);
                line = std::ptr::null_mut();
            }
        }

        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-tokenize.parse-options-fn]
// [spec:hfst:sem:hfst-tokenize.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            // tool-specific options
            let names: &[(&str, c_int, c_int)] = &[
                ("newline", 0, b'n' as c_int),
                ("keep-newline", 0, b'k' as c_int),
                ("print-all", 0, b'a' as c_int),
                ("print-weights", 0, b'w' as c_int),
                ("no-weights", 0, b'W' as c_int),
                ("tokenize-multichar", 0, b'm' as c_int),
                ("beam", 1, b'b' as c_int),
                ("time-cutoff", 1, b't' as c_int),
                ("weight-classes", 1, b'l' as c_int),
                ("unique", 0, b'u' as c_int),
                ("segment", 0, b'z' as c_int),
                ("space-separated", 0, b'd' as c_int),
                ("xerox", 0, b'x' as c_int),
                ("cg", 0, b'c' as c_int),
                ("superblanks", 0, b'S' as c_int),
                ("giella-cg", 0, b'g' as c_int),
                ("gtd", 0, b'g' as c_int),
                ("conllu", 0, b'C' as c_int),
                ("finnpos", 0, b'f' as c_int),
                ("visl", 0, b'L' as c_int),
            ];
            let name_storage: Vec<CString> = names
                .iter()
                .map(|(n, _, _)| CString::new(*n).unwrap())
                .collect();
            for (i, (_, has_arg, val)) in names.iter().enumerate() {
                long_options.push(getopt::Option {
                    name: name_storage[i].as_ptr(),
                    has_arg: *has_arg,
                    flag: std::ptr::null_mut(),
                    val: *val,
                });
            }
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}nkawWmub:t:l:zixcSgCfL",
                HFST_GETOPT_COMMON_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'k' as c_int {
                KEEP_NEWLINES = true;
                BLANKLINE_SEPARATED = false;
            } else if c == b'n' as c_int {
                BLANKLINE_SEPARATED = false;
            } else if c == b'a' as c_int {
                settings().print_all = true;
            } else if c == b'w' as c_int {
                settings().print_weights = true;
            } else if c == b'W' as c_int {
                settings().print_weights = false;
            } else if c == b'm' as c_int {
                settings().tokenize_multichar = true;
            } else if c == b't' as c_int {
                settings().time_cutoff = libc::atof(getopt::OPTARG);
                if settings().time_cutoff < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b'u' as c_int {
                settings().dedupe = true;
            } else if c == b'b' as c_int {
                settings().beam = libc::atof(getopt::OPTARG) as f32;
                if settings().beam < 0.0 {
                    eprint!("Invalid argument for --beam\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b'l' as c_int {
                settings().max_weight_classes = libc::atoi(getopt::OPTARG);
                if settings().max_weight_classes < 1 {
                    eprint!("Invalid or no argument --weight-classes count\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b'z' as c_int {
                settings().output_format = OutputFormat::tokenize;
            } else if c == b'i' as c_int {
                settings().output_format = OutputFormat::space_separated;
            } else if c == b'x' as c_int {
                settings().output_format = OutputFormat::xerox;
            } else if c == b'c' as c_int {
                settings().output_format = OutputFormat::cg;
            } else if c == b'C' as c_int {
                settings().output_format = OutputFormat::conllu;
            } else if c == b'S' as c_int {
                SUPERBLANKS = true;
            } else if c == b'g' as c_int {
                settings().output_format = OutputFormat::giellacg;
                settings().print_weights = true;
                settings().print_all = true;
                settings().dedupe = true;
                settings().hack_uncompose = true;
                settings().verbose = false;
                if settings().max_weight_classes == i32::MAX {
                    settings().max_weight_classes = 2;
                }
            } else if c == b'L' as c_int {
                settings().output_format = OutputFormat::visl;
                settings().print_weights = false;
                settings().print_all = true;
                settings().dedupe = true;
                settings().verbose = false;
            } else if c == b'f' as c_int {
                settings().output_format = OutputFormat::finnpos;
            } else {
                return handle_error_case(c);
            }

            if globals::VERBOSE {
                settings().verbose = true;
            }
        }

        // no more options, we should now be at the input filename
        if (getopt::OPTIND + 1) < argc {
            eprint!("More than one input file given\n");
            libc::EXIT_FAILURE
        } else if (getopt::OPTIND + 1) == argc {
            let ptr = &raw mut TOKENIZER_FILENAME;
            *ptr = cstr(*argv.offset(getopt::OPTIND as isize));
            EXIT_CONTINUE
        } else {
            eprint!("No input file given\n");
            libc::EXIT_FAILURE
        }
    }
}

// [spec:hfst:def:hfst-tokenize.first-transducer-is-called-top-fn]
// [spec:hfst:sem:hfst-tokenize.first-transducer-is-called-top-fn]
// (Defined in the C++ source but never called there; kept for fidelity.)
#[allow(dead_code)]
fn first_transducer_is_called_top(dictionary: &HfstTransducer) -> bool {
    dictionary.get_name() == "TOP"
}

// [spec:hfst:def:hfst-tokenize.main-fn]
// [spec:hfst:sem:hfst-tokenize.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstTokenize");
        hfst_setlocale();
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        let tokenizer_filename = {
            let ptr = &raw const TOKENIZER_FILENAME;
            (*ptr).clone()
        };
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            tokenizer_filename,
            cstr(globals::OUTFILENAME)
        ));
        let mut file = match std::fs::File::open(&tokenizer_filename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", tokenizer_filename);
                return libc::EXIT_FAILURE;
            }
        };
        // The C wraps the rest in try/catch on HfstException (and a nested catch
        // on TransducerHeaderException around parse_hfst3_header); the Rust ports
        // currently panic rather than throw, so those catch arms are not
        // reproduced here.
        //
        // To decide whether we're working with something produced by a pmatch
        // ruleset, we want to know whether the first transducer is named TOP. To
        // do this, rather than load the whole thing into a HfstTransducer, we read
        // just the header variables with parse_hfst3_header, then rewind.
        let first_header_attributes = {
            let mut hdr_stream =
                hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
            PmatchContainer::parse_hfst3_header(&mut hdr_stream)
        };
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));

        let mut stdout = std::io::stdout();
        if first_header_attributes.get("name").map(|s| s.as_str()) != Some("TOP") {
            verbose_printf("No TOP automaton found, using naive tokeniser?\n");
            let mut is = HfstInputStream::new_filename(&tokenizer_filename);
            let mut dictionary = HfstTransducer::new_from_stream(&mut is);
            let mut container = make_naive_tokenizer(&mut dictionary);
            container.set_verbose(globals::VERBOSE);
            container.set_single_codepoint_tokenization(!settings().tokenize_multichar);
            process_input(&mut container, &mut stdout)
        } else {
            verbose_printf("TOP automaton seen, treating as pmatch script...\n");
            let mut is = hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
            let mut container = PmatchContainer::new_from_stream(&mut is);
            container.set_verbose(globals::VERBOSE);
            container.set_single_codepoint_tokenization(!settings().tokenize_multichar);
            process_input(&mut container, &mut stdout)
        }
    }
}
