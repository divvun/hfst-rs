//! Faithful 1:1 port of tools/src/hfst-commandline.{h,cc} — the shared
//! command-line infrastructure for every HFST tool: message printers (verbose,
//! debug, error, warning, colourised variants), the wrapped libc mem/IO helpers
//! that print an error and exit on failure, transducer-format parsing/printing,
//! string-to-number parsers, program-name/version bookkeeping, and the
//! HFST_OPTIONS environment-variable option extension.
//!
//! The shipped C portability shims that are compiled out on a normal POSIX
//! build (the '#ifndef HAVE_GETLINE' / HAVE_GETDELIM / HAVE_STRNDUP / HAVE_ISATTY
//! fallbacks and the WINDOWS-only arms) are omitted here, exactly as the SFST
//! backend and the real readline UI were dropped: the wrappers call the system
//! libc functions directly. The non-readline 'readline' fallback IS kept,
//! because the real readline library path is the one that is #if'd out.
//!
//! Globals live once in 'crate::globals'; in C they were '#include'd per tool.

use crate::globals::{ColourTristate, CommonOptions};
use hfst::backend::Backend;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer, is_safe_conversion};
use std::io::{IsTerminal, Write};

// ---------------------------------------------------------------------------
// constants (header #defines / constexpr)
// ---------------------------------------------------------------------------

/// option "character" for colour
pub const GETOPT_COLOUR: i32 = 27;

/// successful return value for argument parsing routine
pub const EXIT_CONTINUE: i32 = 42;

pub const COLOUR_BOLD: &str = "\x1b[01m";
pub const COLOUR_RED: &str = "\x1b[31m";
pub const COLOUR_GREEN: &str = "\x1b[32m";
pub const COLOUR_YELLOW: &str = "\x1b[33m";
pub const COLOUR_BLUE: &str = "\x1b[34m";
pub const COLOUR_MAGENTA: &str = "\x1b[35m";
pub const COLOUR_CYAN: &str = "\x1b[36m";
pub const COLOUR_RESET: &str = "\x1b[0m";

/// Build provenance, "(YYYY-MM-DD, <short-hash>)", stamped in by build.rs.
/// The hash carries a `-dirty` suffix when the tree had uncommitted changes,
/// so a hand-patched binary cannot pass for the commit it names; both fields
/// read `unknown` when built without a git checkout.
pub const BUILD_STAMP: &str = concat!(
    "(",
    env!("HFST_BUILD_DATE"),
    ", ",
    env!("HFST_BUILD_REV"),
    ")"
);

/// The one --version identity line: "Divvun <cmd> v<version> (date, ref)".
///
/// There is a single version here, the crate's. Upstream carried a per-tool
/// version alongside the package version, so the banner showed two different
/// numbers ("hfst-compose 0.1 (hfst 3.17.1)"); in this port every tool ships
/// out of one crate at one version, so the per-tool number was noise.
pub fn version_line(program_name: &str) -> String {
    format!(
        "Divvun {} v{} {}",
        program_name,
        env!("CARGO_PKG_VERSION"),
        BUILD_STAMP
    )
}

/// The copyright/licence block every tool's --version prints.
///
/// UiT holds copyright on the Rust work; the University of Helsinki line is
/// retained because this remains a derivative work of the C++ HFST, whose
/// LGPL terms require the original notice to be preserved. The licence named
/// here is the one this project actually ships under (LGPLv3-or-later, see
/// COPYING) — upstream's banners said GPLv3, which was never right for a
/// library-licensed tree.
pub const VERSION_COPYRIGHT_BLOCK: &str = "\
Copyright (C) 2026 UiT The Arctic University of Norway
Copyright (C) 2017 University of Helsinki
License LGPLv3+: GNU LGPL version 3 or later <https://gnu.org/licenses/lgpl.html>
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.
";

// ---------------------------------------------------------------------------
// internal helpers
// ---------------------------------------------------------------------------

// Current value of errno, as the C read it after a failing libc call.
fn last_os_error_code() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

// The OS strerror text for `errnum` (C: os_error_string(e)).
fn os_error_string(errnum: i32) -> String {
    std::io::Error::from_raw_os_error(errnum).to_string()
}

// ---------------------------------------------------------------------------
// error / warning printers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.error-at-line-fn]
// [spec:hfst:sem:hfst-commandline.error-at-line-fn]
pub fn error_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    let _ = write!(f, "{}.{}: {}", filename, linenum, msg);
    if errnum != 0 {
        let _ = write!(f, "{}", os_error_string(errnum));
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-error-at-line-fn]
// [spec:hfst:sem:hfst-commandline.hfst-error-at-line-fn]
pub fn hfst_error_at_line(
    opts: &CommonOptions,
    status: i32,
    errnum: i32,
    filename: &str,
    linenum: u32,
    msg: &str,
) {
    let f = &mut std::io::stderr();
    maybe_print_colour(opts, f, COLOUR_BOLD);
    let _ = write!(f, "{}.{}: ", filename, linenum);
    maybe_print_colour(opts, f, COLOUR_RED);
    let _ = write!(f, "Error: ");
    maybe_print_colour(opts, f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(opts, f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", os_error_string(errnum));
        maybe_print_colour(opts, f, COLOUR_RESET);
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-warning-at-line-fn]
// [spec:hfst:sem:hfst-commandline.hfst-warning-at-line-fn]
pub fn hfst_warning_at_line(
    opts: &CommonOptions,
    status: i32,
    errnum: i32,
    filename: &str,
    linenum: u32,
    msg: &str,
) {
    let f = &mut std::io::stderr();
    maybe_print_colour(opts, f, COLOUR_BOLD);
    let _ = write!(f, "{}.{}: ", filename, linenum);
    maybe_print_colour(opts, f, COLOUR_YELLOW);
    let _ = write!(f, "Warning: ");
    maybe_print_colour(opts, f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(opts, f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", os_error_string(errnum));
        maybe_print_colour(opts, f, COLOUR_RESET);
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.error-fn]
// [spec:hfst:sem:hfst-commandline.error-fn]
pub fn error(opts: &CommonOptions, status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    let _ = write!(f, "{}: {}", opts.program_name, msg);
    if errnum != 0 {
        let _ = write!(f, "{}", os_error_string(errnum));
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-error-fn]
// [spec:hfst:sem:hfst-commandline.hfst-error-fn]
pub fn hfst_error(opts: &CommonOptions, status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    maybe_print_colour(opts, f, COLOUR_BOLD);
    let _ = write!(f, "{}: ", opts.program_name);
    maybe_print_colour(opts, f, COLOUR_RED);
    let _ = write!(f, "Error: ");
    maybe_print_colour(opts, f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(opts, f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", os_error_string(errnum));
        maybe_print_colour(opts, f, COLOUR_RESET);
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.warning-fn]
// [spec:hfst:sem:hfst-commandline.warning-fn]
pub fn warning(opts: &CommonOptions, status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    let _ = write!(f, "{}: warning: {}", opts.program_name, msg);
    if errnum != 0 {
        let _ = write!(f, "{}", os_error_string(errnum));
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-warning-fn]
// [spec:hfst:sem:hfst-commandline.hfst-warning-fn]
pub fn hfst_warning(opts: &CommonOptions, status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    maybe_print_colour(opts, f, COLOUR_BOLD);
    let _ = write!(f, "{}: ", opts.program_name);
    maybe_print_colour(opts, f, COLOUR_YELLOW);
    let _ = write!(f, "Warning: ");
    maybe_print_colour(opts, f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(opts, f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", os_error_string(errnum));
        maybe_print_colour(opts, f, COLOUR_RESET);
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// deprecated; everything's compatible
// [spec:hfst:def:hfst-commandline.get-compatible-fst-format-fn]
// [spec:hfst:sem:hfst-commandline.get-compatible-fst-format-fn]
pub fn get_compatible_fst_format() -> i32 {
    unreachable!("get_compatible_fst_format is deprecated; all formats are compatible")
}

// ---------------------------------------------------------------------------
// conditional printf wrappers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.debug-save-transducer-fn]
// [spec:hfst:sem:hfst-commandline.debug-save-transducer-fn]
pub fn debug_save_transducer<B: Backend>(
    opts: &CommonOptions,
    t: &HfstTransducer<B>,
    name: &str,
) -> hfst::error::Result<()> {
    if opts.debug {
        // C built "DEBUG %s" with sprintf; that always succeeds here.
        let mut t = t.clone();
        let debug_name = format!("DEBUG {}", name);
        t.set_name(&debug_name);
        let Ok(mut debug_out) = HfstOutputStream::new_filename(name, t.get_type(), true) else {
            return Ok(());
        };
        debug_print(
            opts,
            &format!(
                "*** DEBUG ({}): saving current transducer to {}\n",
                opts.program_name, name
            ),
        );
        debug_out.redirect(&mut t)?;
        debug_out.close();
    }
    Ok(())
}

// [spec:hfst:def:hfst-commandline.debug-printf-fn]
// [spec:hfst:sem:hfst-commandline.debug-printf-fn]
pub fn debug_print(opts: &CommonOptions, msg: &str) {
    if opts.debug {
        let f = &mut std::io::stderr();
        let _ = write!(f, "\nDEBUG: {}\n", msg);
    }
}

// [spec:hfst:def:hfst-commandline.verbose-printf-fn]
// [spec:hfst:sem:hfst-commandline.verbose-printf-fn]
pub fn verbose_print(opts: &CommonOptions, msg: &str) {
    if opts.verbose {
        let _ = write!(opts.message_writer(), "{}", msg);
    }
}

// ---------------------------------------------------------------------------
// format conversion helpers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.conversion-type-fn]
// [spec:hfst:sem:hfst-commandline.conversion-type-fn]
pub fn conversion_type(type1: ImplementationType, type2: ImplementationType) -> i32 {
    if type1 == type2 {
        return 0;
    }
    if is_safe_conversion(type2, type1) {
        1
    } else if is_safe_conversion(type1, type2) {
        2
    } else {
        -1
    }
}

/// The typed replacement for the C++ 'HfstTransducer::convert(type)' at the
/// CLI stream boundary ([dec:hfst:monomorphic-backends]): re-type a stream
/// sum to 'ty' through the interchange transducer. OL targets build
/// weighted-shaped tables whatever the requested weightedness — exactly as
/// the C++ convert did — with the header flag carrying the OL/OLW tag.
pub fn convert_any(t: AnyTransducer, ty: ImplementationType) -> hfst::error::Result<AnyTransducer> {
    convert_any_with_options(t, ty, "")
}

/// ['convert_any'] with the C++ convert's options string ("quick" relaxes the
/// optimized-lookup table packing; only fst2fst's -Q sets it).
pub fn convert_any_with_options(
    t: AnyTransducer,
    ty: ImplementationType,
    options: &str,
) -> hfst::error::Result<AnyTransducer> {
    if t.get_type() == ty {
        return Ok(t);
    }
    Ok(match ty {
        ImplementationType::TROPICAL_OPENFST_TYPE => AnyTransducer::Tropical(t.into_typed()?),
        #[cfg(feature = "foma")]
        ImplementationType::FOMA_TYPE => AnyTransducer::Foma(t.into_typed()?),
        ImplementationType::HFST_OLW_TYPE | ImplementationType::HFST_OL_TYPE => {
            let weighted = ty == ImplementationType::HFST_OLW_TYPE;
            match t {
                AnyTransducer::Tropical(x) => AnyTransducer::OlW(x.to_ol(weighted, options)?),
                // OL -> OL retag (only the header weightedness differs):
                // through the algebra, as the C++ went through basic.
                other @ AnyTransducer::OlW(_) | other @ AnyTransducer::OlU(_) => {
                    let x: HfstTransducer<hfst_openfst::StdVectorFst> = other.into_typed()?;
                    AnyTransducer::OlW(x.to_ol(weighted, options)?)
                }
                // Foma routes through the tropical algebra into weighted OL
                // tables, exactly as the OlU arm does.
                #[cfg(feature = "foma")]
                other @ AnyTransducer::Foma(_) => {
                    let x: HfstTransducer<hfst_openfst::StdVectorFst> = other.into_typed()?;
                    AnyTransducer::OlW(x.to_ol(weighted, options)?)
                }
                // THFST -> OLW: O(1) table move back to the weighted engine,
                // then the same weightedness retag as the OLW/OlU arms (through
                // the tropical algebra, as the C++ went through basic).
                // [spec:hfst:sem:thfst-backend.olw-moves]
                AnyTransducer::Thfst(t) => {
                    let x: HfstTransducer<hfst_openfst::StdVectorFst> =
                        AnyTransducer::OlW(t.into_olw()).into_typed()?;
                    AnyTransducer::OlW(x.to_ol(weighted, options)?)
                }
            }
        }
        // THFST target: O(1) table move from the weighted engine (OLW), or a
        // build-through-OLW-then-move for everything else. THFST is a
        // lookup-tier citizen like OLW; its target is always the weighted
        // engine retagged as THFST.
        // [spec:hfst:def:thfst-backend.olw-moves]
        // [spec:hfst:sem:thfst-backend.olw-moves]
        ImplementationType::THFST_TYPE => match t {
            // O(1) table move: OLW is already the weighted engine THFST wraps.
            AnyTransducer::OlW(x) => AnyTransducer::Thfst(x.into_thfst()),
            // Build weighted OL tables through the algebra, then move.
            AnyTransducer::Tropical(x) => {
                AnyTransducer::Thfst(x.to_ol(true, options)?.into_thfst())
            }
            // OlU (and any foma source) routes through the tropical algebra
            // into weighted OL tables, exactly as the OLW target's OlU arm does.
            other @ AnyTransducer::OlU(_) => {
                let x: HfstTransducer<hfst_openfst::StdVectorFst> = other.into_typed()?;
                AnyTransducer::Thfst(x.to_ol(true, options)?.into_thfst())
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => {
                let x: HfstTransducer<hfst_openfst::StdVectorFst> = other.into_typed()?;
                AnyTransducer::Thfst(x.to_ol(true, options)?.into_thfst())
            }
            // Thfst -> Thfst cannot reach here (early return on equal types),
            // but the match must be exhaustive.
            AnyTransducer::Thfst(x) => AnyTransducer::Thfst(x),
        },
        other => hfst::bail!(ImplementationTypeNotAvailable(other)),
    })
}

// [spec:hfst:def:hfst-commandline.convert-transducers-fn]
// [spec:hfst:sem:hfst-commandline.convert-transducers-fn]
pub fn convert_transducers(
    opts: &CommonOptions,
    first: AnyTransducer,
    second: AnyTransducer,
) -> hfst::error::Result<(AnyTransducer, AnyTransducer)> {
    let type1 = first.get_type();
    let type2 = second.get_type();
    let ct = conversion_type(type1, type2);

    if ct == 0 {
        Ok((first, second))
    } else if ct == 1 {
        hfst_warning(
            opts,
            0,
            0,
            &format!(
                "transducers have different types, converting to format {}\n",
                hfst_strformat(type1)
            ),
        );
        let second = convert_any(second, type1)?;
        Ok((first, second))
    } else if ct == 2 {
        hfst_warning(
            opts,
            0,
            0,
            &format!(
                "transducers have different types, converting to format {}\n",
                hfst_strformat(type2)
            ),
        );
        let first = convert_any(first, type2)?;
        Ok((first, second))
    } else if ct == -1 {
        hfst_warning(
            opts,
            0,
            0,
            &format!(
                "transducers have different types, converting to format {}, loss of information is possible\n",
                hfst_strformat(type1)
            ),
        );
        let second = convert_any(second, type1)?;
        Ok((first, second))
    } else {
        // This should not happen.
        hfst::HFST_THROW_MESSAGE!(
            Fatal,
            "convert_transducers: conversion_type returned an invalid integer"
        );
    }
}

/// Write an algebra-backend result into 'outstream', converting to
/// optimized-lookup first when the stream was opened with an OL '--format'
/// (the C++ facade converted inside 'operator<<' type plumbing; the typed
/// conversion of [dec:hfst:monomorphic-backends] happens here instead).
pub fn redirect_converting<B: hfst::backend::AlgebraBackend>(
    outstream: &mut HfstOutputStream,
    t: &mut HfstTransducer<B>,
) -> hfst::error::Result<()> {
    match outstream.get_type() {
        ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
            let mut ol = t.to_ol(
                outstream.get_type() == ImplementationType::HFST_OLW_TYPE,
                "",
            )?;
            outstream.redirect(&mut ol)?;
        }
        #[cfg(feature = "foma")]
        ImplementationType::FOMA_TYPE => {
            let mut foma = t.to_foma()?;
            outstream.redirect(&mut foma)?;
        }
        _ => {
            outstream.redirect(t)?;
        }
    }
    Ok(())
}

// [spec:hfst:def:hfst-commandline.is-input-stream-in-ol-format-fn]
// [spec:hfst:sem:hfst-commandline.is-input-stream-in-ol-format-fn]
pub fn is_input_stream_in_ol_format(is: &HfstInputStream<'_>, program: &str) -> bool {
    if is.get_type() == ImplementationType::HFST_OL_TYPE
        || is.get_type() == ImplementationType::HFST_OLW_TYPE
        || is.get_type() == ImplementationType::THFST_TYPE
    {
        let _ = writeln!(
            std::io::stderr(),
            "Error: {} cannot process transducers that are in optimized lookup format.",
            program
        );
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// string -> number parsers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-strtoweight-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtoweight-fn]
pub fn hfst_strtoweight(opts: &CommonOptions, s: &str) -> f32 {
    // C++ is `strtod(s, &endptr)` accepted when `*endptr == '\0'`, which
    // str::parse does not reproduce in two ways that reach real data:
    //   - "" performs no conversion and leaves endptr on the NUL, so the test
    //     passes and the weight is 0. An empty field means "no weight", not an
    //     error — Giella's speller rules pipe through `cut -f1-2`, so a stray
    //     second tab hands us exactly this.
    //   - strtod skips leading whitespace. Trailing whitespace still fails,
    //     since endptr then rests on the space rather than the NUL, and a
    //     whitespace-only string fails too (no conversion, so endptr is left
    //     at the start, not past the blanks).
    if s.is_empty() {
        return 0.0;
    }
    let s = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    match s.parse::<f64>() {
        Ok(rv) => rv as f32,
        Err(_) => {
            hfst_error(
                opts,
                1,
                last_os_error_code(),
                &format!("{} not a weight", s),
            );
            0.0
        }
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtonumber-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtonumber-fn]
pub fn hfst_strtonumber(opts: &CommonOptions, s: &str, infinite: Option<&mut bool>) -> i32 {
    let mut infinite = infinite;
    if let Some(ref mut b) = infinite {
        **b = false;
    }
    let rv = match s.parse::<f64>() {
        Ok(rv) => rv,
        Err(_) => {
            hfst_error(
                opts,
                1,
                last_os_error_code(),
                &format!("{} not a number", s),
            );
            return 0;
        }
    };
    if rv.is_infinite() && infinite.is_some() {
        if let Some(b) = infinite {
            *b = true;
        }
        // std::signbit(rv): 1 if negative, 0 otherwise.
        return if rv.is_sign_negative() { 1 } else { 0 };
    } else if rv > i32::MAX as f64 {
        return i32::MAX;
    } else if rv < i32::MIN as f64 {
        return i32::MIN;
    }
    rv.floor() as i32
}

// [spec:hfst:def:hfst-commandline.hfst-strtoul-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtoul-fn]
pub fn parse_u64(opts: &CommonOptions, s: &str, base: i32) -> u64 {
    match u64::from_str_radix(s, base as u32) {
        Ok(rv) => rv,
        Err(_) => {
            hfst_error(
                opts,
                1,
                last_os_error_code(),
                &format!("{} is not a valid unsigned number string", s),
            );
            0
        }
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtol-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtol-fn]
pub fn parse_i64(opts: &CommonOptions, s: &str, base: i32) -> i64 {
    match i64::from_str_radix(s, base as u32) {
        Ok(rv) => rv,
        Err(_) => {
            hfst_error(
                opts,
                1,
                last_os_error_code(),
                &format!("{} is not a valid signed number string", s),
            );
            0
        }
    }
}

// ---------------------------------------------------------------------------
// transducer-format name parsing / printing
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-parse-format-name-fn]
// [spec:hfst:sem:hfst-commandline.hfst-parse-format-name-fn]
pub fn hfst_parse_format_name(opts: &CommonOptions, s: &str) -> ImplementationType {
    let lower = s.to_ascii_lowercase();
    let rv;
    if lower == "sfst" {
        rv = ImplementationType::SFST_TYPE;
    } else if lower == "openfst-tropical" || lower == "ofst-tropical" {
        rv = ImplementationType::TROPICAL_OPENFST_TYPE;
    } else if lower == "openfst" || lower == "ofst" {
        rv = ImplementationType::TROPICAL_OPENFST_TYPE;
        hfst_warning(
            opts,
            0,
            0,
            &format!("Ambiguous format name {}, guessing openfst-tropical", s),
        );
    } else if lower == "foma" {
        rv = ImplementationType::FOMA_TYPE;
    } else if lower == "xfsm" {
        rv = ImplementationType::XFSM_TYPE;
    } else if lower == "optimized-lookup-unweighted" || lower == "olu" {
        rv = ImplementationType::HFST_OL_TYPE;
    } else if lower == "optimized-lookup-weighted" || lower == "olw" {
        rv = ImplementationType::HFST_OLW_TYPE;
    } else if lower == "thfst" {
        rv = ImplementationType::THFST_TYPE;
    } else if lower == "optimized-lookup" || lower == "ol" {
        rv = ImplementationType::HFST_OLW_TYPE;
        hfst_warning(
            opts,
            0,
            0,
            &format!(
                "Ambiguous format name {}, guessing optimized-lookup-weighted",
                s
            ),
        );
    } else {
        hfst_error(
            opts,
            1,
            0,
            &format!("Could not parse format name from string {}", s),
        );
        return ImplementationType::UNSPECIFIED_TYPE;
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-strformat-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strformat-fn]
pub fn hfst_strformat(format: ImplementationType) -> &'static str {
    match format {
        ImplementationType::SFST_TYPE => "SFST (1.4 compatible)",
        ImplementationType::TROPICAL_OPENFST_TYPE => "OpenFST, std arc, tropical semiring",
        ImplementationType::FOMA_TYPE => "foma",
        ImplementationType::XFSM_TYPE => "xfsm",
        ImplementationType::HFST_OL_TYPE => "Hfst's lookup optimized, unweighted",
        ImplementationType::HFST_OLW_TYPE => "Hfst's lookup optimized, weighted",
        ImplementationType::THFST_TYPE => {
            "thfst (divvunspell optimized-lookup, weighted, directory format)"
        }
        ImplementationType::HFST2_TYPE => "Hfst 2 legacy (deprecated)",
        ImplementationType::ERROR_TYPE | ImplementationType::UNSPECIFIED_TYPE => {
            "ERROR (not a HFST supported transducer)"
        }
    }
}

// ---------------------------------------------------------------------------
// file functions
// ---------------------------------------------------------------------------
//
// The FILE*-based helpers (hfst_fopen / hfst_fseek / hfst_ftell / hfst_fread /
// hfst_fwrite / hfst_tmpfile) were removed in the io-foundation de-C-ism: tools
// open their I/O as std streams (globals::input_reader / output_writer / first_
// reader / second_reader, or std::fs::File) and use std::io::{Read, Write, Seek}.
// The fd-level wrappers (hfst_close / hfst_open / hfst_read / hfst_write /
// hfst_mkstemp / hfst_remove) had no callers and were dropped with the libc nuke.

// ---------------------------------------------------------------------------
// interactive line input
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-readline-fn]
// [spec:hfst:sem:hfst-commandline.hfst-readline-fn]
//
// The non-readline fallback (the real readline-library path is #if'd out): print
// the prompt, then read a line (trailing '\n' kept, as getline did). None at EOF.
pub fn hfst_readline(opts: &CommonOptions, prompt: &str) -> Option<String> {
    {
        let mut mw = opts.message_writer();
        let _ = write!(mw, "{}", prompt);
        let _ = mw.flush();
    }
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line),
    }
}

// [spec:hfst:def:hfst-commandline.set-program-name-fn]
// [spec:hfst:sem:hfst-commandline.set-program-name-fn]
fn program_name_from_argv0(argv0: &str) -> String {
    // this's gnulib
    let bytes = argv0.as_bytes();
    let slash = bytes.iter().rposition(|&c| c == b'/');
    let base = match slash {
        Some(i) => i + 1, // slash + 1
        None => 0,        // argv0
    };
    // base - argv0 >= 7 && strncmp(base - 7, "/.libs/", 7) == 0
    let mut start = 0usize;
    if base >= 7 && &bytes[base - 7..base] == b"/.libs/" {
        start = base;
        // strncmp(base, "lt-", 3) == 0
        if bytes.len() >= base + 3 && &bytes[base..base + 3] == b"lt-" {
            start = base + 3;
        }
    }
    let name = &argv0[start..];
    if name == "hfst-calculate" {
        "hfst-sfstpl2fst".to_string()
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// customized default printouts for HFST tools
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-set-program-name-fn]
// [spec:hfst:sem:hfst-commandline.hfst-set-program-name-fn]
pub fn hfst_set_program_name(argv0: &str, version_vector: &str, wikiname: &str) -> CommonOptions {
    // Install the shared `tracing` subscriber once, idempotently. Library
    // diagnostics are already gated at their call sites (silent / verbose), so a
    // permissive (TRACE) subscriber renders exactly what the code chooses to
    // emit. Replaces the former hfst::set_warning_stream(&std::cerr).
    //
    // Foreign crates do NOT gate at their call sites, so the known-noisy
    // third-party targets are clamped to WARN: serde-xml-rs / xml-rs (log
    // records, bridged by tracing-log) and box-format (native tracing) would
    // otherwise flood hfst-bhfst's stderr with parser/archive traces.
    {
        use tracing::level_filters::LevelFilter;
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        let _ = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .without_time()
                    .with_target(false),
            )
            .with(
                tracing_subscriber::filter::Targets::new()
                    .with_default(LevelFilter::TRACE)
                    .with_target("serde_xml_rs", LevelFilter::WARN)
                    .with_target("xml", LevelFilter::WARN)
                    .with_target("box_format", LevelFilter::WARN),
            )
            .try_init();
    }
    // Seed the tool's CommonOptions with its identity; parse_options fills the
    // rest. The idiomatic replacement for the former program-name/version
    // globals.
    CommonOptions {
        program_name: program_name_from_argv0(argv0),
        hfst_tool_version: version_vector.to_string(),
        hfst_tool_wikiname: wikiname.to_string(),
        ..CommonOptions::default()
    }
}

// [spec:hfst:def:hfst-commandline.print-short-help-fn]
// [spec:hfst:sem:hfst-commandline.print-short-help-fn]
pub fn print_short_help(opts: &CommonOptions) {
    let mut mw = opts.message_writer();
    let _ = writeln!(
        mw,
        "Try ``{} --help'' for more information.",
        opts.program_name
    );
}

// print version message
// [spec:hfst:def:hfst-commandline.print-version-fn]
// [spec:hfst:sem:hfst-commandline.print-version-fn]
pub fn print_version(opts: &CommonOptions) {
    let mut mw = opts.message_writer();
    let _ = writeln!(mw, "{}", version_line(&opts.program_name));
    let _ = write!(mw, "{VERSION_COPYRIGHT_BLOCK}");
}

// [spec:hfst:def:hfst-commandline.extend-options-getenv-fn]
// [spec:hfst:sem:hfst-commandline.extend-options-getenv-fn]
//
// Append the space-separated tokens of $HFST_OPTIONS to the program arguments
// (consecutive spaces collapse, as the C strtok loop did); getopt then permutes
// them into place.
pub fn extend_options_from_env(args: &mut Vec<String>) {
    if let Ok(hfstopts) = std::env::var("HFST_OPTIONS") {
        for t in hfstopts.split(' ').filter(|t| !t.is_empty()) {
            args.push(t.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// colour
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.should-colourise-fn]
// [spec:hfst:sem:hfst-commandline.should-colourise-fn]
pub fn should_colourise(opts: &CommonOptions) -> bool {
    let colour = opts.colour;
    if colour == ColourTristate::COLOUR_AUTO {
        // this is not the best heuristic but wfm
        std::io::stdout().is_terminal()
    } else if colour == ColourTristate::COLOUR_ALWAYS {
        true
    } else if colour == ColourTristate::COLOUR_NEVER {
        false
    } else {
        unreachable!("colour tristate has no value beyond AUTO/ALWAYS/NEVER")
    }
}

// [spec:hfst:def:hfst-commandline.maybe-print-colour-fn]
// [spec:hfst:sem:hfst-commandline.maybe-print-colour-fn]
pub fn maybe_print_colour(opts: &CommonOptions, f: &mut dyn Write, colour: &str) {
    if should_colourise(opts) {
        let _ = write!(f, "{}", colour);
    }
}
