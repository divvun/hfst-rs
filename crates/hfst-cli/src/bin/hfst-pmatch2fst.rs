//! Faithful 1:1 port of tools/src/hfst-pmatch2fst.cc — the pmatch regular
//! expression compiling command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options) plus the hfst pmatch
//! compiler and the OL conversion functions.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::pmatch_compiler::{CLOCKS_PER_SEC, clock, clock_t};
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, hfst_strdup, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

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

// C: static char *epsilonname = NULL;
static mut EPSILONNAME: *mut c_char = std::ptr::null_mut();
// C: static bool flatten = false;
static mut FLATTEN: bool = false;
// C: static bool include_cosine_distances = false;
static mut INCLUDE_COSINE_DISTANCES: bool = false;
// C: static clock_t timer;
static mut TIMER: clock_t = 0;

// C: the compilation format, chosen at compile time from the available
// back-ends. The Rust crate links the tropical OpenFST back-end.
const COMPILATION_FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

// [spec:hfst:def:hfst-pmatch2fst.print-usage-fn]
// [spec:hfst:sem:hfst-pmatch2fst.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nCompile regular expressions into transducer(s)\n (Experimental version)\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "String and format options:\n  -e, --epsilon=EPS         Map EPS as zero\n      --flatten             Compile in all RTNs\n      --cosine-distances    When compiling Like() operations, include cosine distance info\n",
        );
        fput(globals::message_out(), "\n");

        fput(
            globals::message_out(),
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\nIf EPS is not defined, the default representation of 0 is used\nWeights are currently not implemented.\n\n",
        );

        fput(
            globals::message_out(),
            &format!(
                "Examples:\n  echo \"Define TOP  UppercaseAlpha Alpha* LC({{professor}}) EndTag(ProfName);\" | {} \n  create matcher that tags \"professor Chomsky\" as \"professor <ProfName>Chomsky</ProfName>\"\n\n",
                program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
        fput(globals::message_out(), "\n");
    }
}

// [spec:hfst:def:hfst-pmatch2fst.parse-options-fn]
// [spec:hfst:sem:hfst-pmatch2fst.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: c"epsilon".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'e' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"flatten".as_ptr(),
                has_arg: getopt::NO_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: '1' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"cosine-distances".as_ptr(),
                has_arg: getopt::NO_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: '2' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}e:",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
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

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 as char {
                'e' => {
                    EPSILONNAME = hfst_strdup(getopt::OPTARG);
                    continue;
                }
                '1' => {
                    FLATTEN = true;
                    continue;
                }
                '2' => {
                    INCLUDE_COSINE_DISTANCES = true;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-pmatch2fst.get-current-dir-name-fn]
// [spec:hfst:sem:hfst-pmatch2fst.get-current-dir-name-fn]
unsafe fn get_current_dir_name() -> String {
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
unsafe fn process_stream(outstream: &mut HfstOutputStream) -> c_int {
    unsafe {
        let mut comp = PmatchCompiler::new(COMPILATION_FORMAT);
        comp.set_verbose(globals::VERBOSE);
        comp.set_flatten(FLATTEN);
        comp.set_include_cosine_distances(INCLUDE_COSINE_DISTANCES);
        let mut file_bytes: Vec<u8> = Vec::new();
        let mut definitions: std::collections::HashMap<String, *mut HfstTransducer> =
            std::collections::HashMap::new();

        let inputfile = globals::inputfile();

        let mut includedir = String::new();
        let inputfilename_str = cstr(globals::INPUTFILENAME);
        // C: 'inputfile != stdin'. In the foundation, INPUTFILE is non-null only
        // when a real input file was opened (else stdin is used).
        if !globals::INPUTFILE.is_null() && !inputfilename_str.is_empty() {
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

        loop {
            let c = libc::fgetc(inputfile);
            if c == libc::EOF {
                break;
            }
            // C: std::string::push_back(c) — accumulate raw bytes.
            file_bytes.push(c as u8);
        }
        // C: std::string holds bytes; reinterpret the collected bytes as UTF-8.
        let file_contents = String::from_utf8_lossy(&file_bytes).into_owned();
        if file_contents.len() > 1 {
            // C wraps comp.compile in try/catch on HfstException; on a thrown
            // exception it prints e.name and returns EXIT_FAILURE. The Rust
            // compiler panics rather than throwing, so the catch arm is not
            // reproduced (any panic propagates).
            definitions = comp.compile(&file_contents);
        }

        if globals::VERBOSE {
            TIMER = clock();
            eprint!("Building hfst-ol alphabet... ");
        }

        // A dummy transducer with an alphabet with all the symbols
        let mut harmonizer = HfstTransducer::new_type(COMPILATION_FORMAT);
        // First we need to collect a unified alphabet from all the transducers.
        let mut symbols_seen: hfst::hfst_symbol_defs::StringSet = std::collections::BTreeSet::new();
        // Iterate in key order to mirror std::map's ordered iteration.
        let mut keys: Vec<&String> = definitions.keys().collect();
        keys.sort();
        for key in &keys {
            let t = definitions[*key];
            let string_set = (*t).get_alphabet();
            for sym in string_set.iter() {
                if !symbols_seen.contains(sym) {
                    harmonizer.insert_to_alphabet(sym);
                    symbols_seen.insert(sym.clone());
                }
            }
        }
        if symbols_seen.is_empty() {
            // We don't recognise anything, go home early
            eprintln!(
                "{}: Empty ruleset, nothing to write",
                cstr(globals::PROGRAM_NAME)
            );
            return libc::EXIT_FAILURE;
        }

        // Then we convert it...
        harmonizer.convert(ImplementationType::HFST_OLW_TYPE, String::new());
        // Use these for naughty intermediate steps to make sure
        // everything has the same alphabet
        // C passes 'HfstTransducer* harmonizer' to the conversion functions,
        // which read its hfst_ol backend; the Rust signature takes that backend
        // directly, so unwrap it here (harmonizer is now HFST_OLW_TYPE).
        let harmonizer_ol = ConversionFunctions::hfst_transducer_to_hfst_ol(&mut harmonizer);

        if globals::VERBOSE {
            let duration = (clock() - TIMER) as f64 / CLOCKS_PER_SEC as f64;
            TIMER = clock();
            eprintln!("built in {:.2} seconds", duration);
            eprint!("Converting TOP... ");
        }

        // When done compiling everything, look for TOP and output it first.
        if definitions.contains_key("TOP") {
            let properties: std::collections::BTreeMap<String, String> =
                (*definitions["TOP"]).get_properties().clone();
            let intermediate_tmp =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&*definitions["TOP"]);
            let harmonized_tmp = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &intermediate_tmp,
                true,                  // weighted
                "",                    // no special options
                Some(&*harmonizer_ol), // harmonize with this
            );
            let mut output_tmp = ConversionFunctions::hfst_ol_to_hfst_transducer(&harmonized_tmp);
            output_tmp.set_name("TOP");
            for (k, v) in properties.iter() {
                output_tmp.set_property(k, v);
            }
            outstream.redirect(&mut output_tmp);
            drop(Box::from_raw(definitions["TOP"]));
            definitions.remove("TOP");

            if globals::VERBOSE {
                let duration = (clock() - TIMER) as f64 / CLOCKS_PER_SEC as f64;
                TIMER = clock();
                eprintln!("converted in {:.2} seconds", duration);
            }

            let mut rest_keys: Vec<String> = definitions.keys().cloned().collect();
            rest_keys.sort();
            for key in &rest_keys {
                let t = definitions[key];
                if globals::VERBOSE {
                    eprintln!("Converting {}... ", key);
                    TIMER = clock();
                }
                let intermediate_tmp =
                    ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&*t);
                let harmonized_tmp = if !key.contains("UNCOMPOSE") {
                    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                        &intermediate_tmp,
                        true,                  // weighted
                        "empty_alphabet",      // empty alphabet in RTNs, they'll use the main one
                        Some(&*harmonizer_ol), // harmonize with this
                    )
                } else {
                    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                        &intermediate_tmp,
                        true,                  // weighted
                        "",                    // alphabet in UNCs,
                        Some(&*harmonizer_ol), // harmonize with this
                    )
                };
                let mut output_tmp =
                    ConversionFunctions::hfst_ol_to_hfst_transducer(&harmonized_tmp);
                output_tmp.set_name(key);
                outstream.redirect(&mut output_tmp);
                drop(Box::from_raw(t));
                if globals::VERBOSE {
                    let duration = (clock() - TIMER) as f64 / CLOCKS_PER_SEC as f64;
                    eprintln!("converted in {:.2} seconds", duration);
                }
            }
        } else {
            eprintln!(
                "{}: Empty ruleset, nothing to write",
                cstr(globals::PROGRAM_NAME)
            );
            return libc::EXIT_FAILURE;
        }
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-pmatch2fst.main-fn]
// [spec:hfst:sem:hfst-pmatch2fst.main-fn]
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
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argc: c_int = c_args.len() as c_int;
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "Pmatch2Fst");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let output_opened = !globals::OUTFILE.is_null();
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(
                &cstr(globals::OUTFILENAME),
                ImplementationType::HFST_OLW_TYPE,
                true,
            )
        } else {
            HfstOutputStream::new(ImplementationType::HFST_OLW_TYPE, true)
        };
        process_stream(&mut outstream);
        libc::EXIT_SUCCESS
    }
}
