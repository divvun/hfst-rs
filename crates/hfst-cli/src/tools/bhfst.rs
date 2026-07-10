//! `hfst-bhfst` — pack a THFST acceptor/errmodel pair (+ speller metadata) into
//! a BHFST speller archive, or inspect an existing one. There is NO C++ HFST
//! ancestor: this tool is authored greenfield against
//! `docs/spec/port/back-ends/thfst/thfst-backend.md`, and its contract is
//! divvunspell compatibility — the archives it writes are consumed by
//! `github.com/divvun/divvunspell` (its `src/archive/boxf.rs` loader), and the
//! reference producer it mirrors is divvunspell's `thfst-tools`.
//!
//! It follows the house getopt/[`CommonOptions`] pattern (like the other tools),
//! not clap. Unlike the algebra/lookup tools it has NO default input/output
//! streams: pack mode requires `-a`, `-e`, `-o`; info mode requires only `-I`.
//! It therefore uses only [`hfst_getopt_common_long`] plus its own options (no
//! unary/binary `inc` helpers, which assume a standard in/out stream).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    convert_any_with_options, error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt, REQUIRED_ARGUMENT};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::inc::{CaseResult, handle_common_case, handle_error_case};
use box_format::{
    BoxPath, Compression, CompressionConfig, HashMap as BoxHashMap, sync::BoxReader,
    sync::BoxWriter,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Box data alignment: divvunspell mmaps the member files at their raw archive
/// offsets, so 8-byte alignment is a hard requirement
/// [spec:hfst:sem:thfst-backend.bhfst-layout].
const ALIGNMENT: u32 = 8;

/// The canonical box entry directory names divvunspell hard-codes
/// [spec:hfst:sem:thfst-backend.bhfst-layout].
const ACCEPTOR_DIRNAME: &str = "acceptor.default.thfst";
const ERRMODEL_DIRNAME: &str = "errmodel.default.thfst";

/// The three THFST member files, in the order they must enter the archive.
const THFST_MEMBERS: [&str; 3] = ["alphabet", "index", "transition"];

/// hfst-bhfst's own options.
#[derive(Default)]
struct Options {
    /// '-a/--acceptor': acceptor source (a .thfst dir or any transducer file).
    acceptor: Option<String>,
    /// '-e/--errmodel': error-model source (same).
    errmodel: Option<String>,
    /// '-X/--index-xml': zhfst index.xml converted to meta.json (ids rewritten).
    index_xml: Option<String>,
    /// '-m/--meta': meta.json embedded verbatim (mutually exclusive with -X).
    meta: Option<String>,
    /// '-I/--info': print metadata of an existing .bhfst and exit.
    info: Option<String>,
}

// -----------------------------------------------------------------------------
// Metadata mirror structs — the serde shapes of divvunspell's SpellerMetadata
// (src/archive/meta.rs). Field names, the "$value" rename, the `type` default,
// and the Option fields are mirrored EXACTLY so the JSON round-trips byte-for-
// byte with divvunspell. [spec:hfst:def:thfst-backend.meta-json]
// -----------------------------------------------------------------------------
mod bhfst_meta {
    use serde::{Deserialize, Serialize};
    use serde_xml_rs::ParserConfig;

    /// divvunspell's `SpellerMetadata`.
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SpellerMetadata {
        pub info: SpellerMetadataInfo,
        pub acceptor: SpellerMetadataAcceptor,
        pub errmodel: SpellerMetadataErrmodel,
    }

    /// divvunspell's `SpellerTitle`.
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SpellerTitle {
        pub lang: Option<String>,
        #[serde(rename = "$value")]
        pub value: String,
    }

    /// divvunspell's `SpellerMetadataInfo`.
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SpellerMetadataInfo {
        pub locale: String,
        pub title: Vec<SpellerTitle>,
        pub description: String,
        pub producer: String,
    }

    /// divvunspell's `SpellerMetadataAcceptor`. `type_` carries the `type` XML
    /// attribute, defaulting to "" when absent, exactly like divvunspell.
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SpellerMetadataAcceptor {
        #[serde(rename = "type", default)]
        pub type_: String,
        pub id: String,
        pub title: Vec<SpellerTitle>,
        pub description: String,
        pub continuation: Option<String>,
    }

    /// divvunspell's `SpellerMetadataErrmodel`.
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SpellerMetadataErrmodel {
        pub id: String,
        pub title: Vec<SpellerTitle>,
        pub description: String,
    }

    impl SpellerMetadata {
        /// Parse a zhfst index.xml with the exact serde-xml-rs 0.6 parser
        /// configuration divvunspell uses (whitespace trimming, comment
        /// skipping, character coalescing).
        /// [spec:hfst:sem:thfst-backend.meta-json]
        pub fn from_xml_bytes(bytes: &[u8]) -> Result<SpellerMetadata, serde_xml_rs::Error> {
            let mut reader = ParserConfig::new()
                .trim_whitespace(true)
                .ignore_comments(true)
                .coalesce_characters(true)
                .create_reader(bytes)
                .into_inner();
            serde_xml_rs::from_reader(&mut reader)
        }
    }
}

use bhfst_meta::SpellerMetadata;

// -----------------------------------------------------------------------------
// usage / option parsing
// -----------------------------------------------------------------------------

fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...]\n\
         Pack a THFST acceptor/errmodel pair (+ speller metadata) into a BHFST archive\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Packing options:\n\
         \u{20}\u{20}-a, --acceptor=FILE   Acceptor: a .thfst directory, or any transducer file\n\
         \u{20}\u{20}                        (auto-converted to THFST)\n\
         \u{20}\u{20}-e, --errmodel=FILE   Error model: same as --acceptor\n\
         \u{20}\u{20}-X, --index-xml=FILE  zhfst index.xml, converted to meta.json (ids .hfst->.thfst)\n\
         \u{20}\u{20}-m, --meta=FILE       meta.json embedded verbatim (mutually exclusive with -X)\n\
         \u{20}\u{20}-o, --output=FILE     Output .bhfst archive (required for packing; stdout unsupported)\n\
         \u{20}\u{20}-I, --info=FILE       Print metadata of an existing .bhfst and exit\n\
         \n\
         Pack mode needs -a, -e and -o. Info mode needs only -I.\n"
    );
    let _ = write!(msg, "\n");
}

/// Parse argv into the shared + tool options; `Err(code)` is an exit code the
/// caller should return.
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
        long_options.push(getopt::GetOpt {
            name: "acceptor",
            has_arg: REQUIRED_ARGUMENT,
            val: b'a' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "errmodel",
            has_arg: REQUIRED_ARGUMENT,
            val: b'e' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "index-xml",
            has_arg: REQUIRED_ARGUMENT,
            val: b'X' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "meta",
            has_arg: REQUIRED_ARGUMENT,
            val: b'm' as i32,
        });
        // '-o/--output' is recognized here but routed to `handle_common_case`,
        // which populates `common.output_filename`/`output_named` (there is no
        // `b'o'` arm below). No `inc` unary/binary helpers are chained: this
        // tool has no default input/output transducer stream.
        long_options.push(getopt::GetOpt {
            name: "output",
            has_arg: REQUIRED_ARGUMENT,
            val: b'o' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "info",
            has_arg: REQUIRED_ARGUMENT,
            val: b'I' as i32,
        });

        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        let ch = c as u8;
        match ch {
            b'a' => {
                options.acceptor = Some(opt.optarg());
                continue;
            }
            b'e' => {
                options.errmodel = Some(opt.optarg());
                continue;
            }
            b'X' => {
                options.index_xml = Some(opt.optarg());
                continue;
            }
            b'm' => {
                options.meta = Some(opt.optarg());
                continue;
            }
            b'I' => {
                options.info = Some(opt.optarg());
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    // No default output stream: mirror check-params-common's default so
    // message routing (stderr) matches the rest of the suite.
    if !common.output_named {
        common.message_to_stderr = true;
    }

    // No positional operands are accepted.
    let optind = opt.optind;
    if args.len() > optind {
        error(
            &common,
            1,
            0,
            "hfst-bhfst takes no positional arguments; use -a/-e/-o or -I",
        );
        return Err(1);
    }

    Ok((common, options))
}

// -----------------------------------------------------------------------------
// pack mode
// -----------------------------------------------------------------------------

/// A THFST source resolved to an on-disk directory holding the three member
/// files. When the source was a ready `.thfst` dir it is used in place (and the
/// optional `_tempdir` is None); when it was any other transducer it is
/// converted to THFST and serialized into a `tempfile::TempDir` kept alive here.
struct ThfstSource {
    dir: PathBuf,
    // The temp dir keeps the serialized THFST alive until the archive is
    // written; None when the source was a ready .thfst directory used in place.
    _tempdir: Option<tempfile::TempDir>,
}

/// True if `dir` is a directory holding all three THFST member files.
fn is_thfst_dir(dir: &Path) -> bool {
    dir.is_dir() && THFST_MEMBERS.iter().all(|m| dir.join(m).is_file())
}

/// Resolve a `-a`/`-e` source to a THFST directory. A ready `.thfst` dir is used
/// in place; anything else is read via `HfstInputStream`, converted to THFST via
/// the standard format-conversion path, and serialized into a temp dir.
/// [spec:hfst:sem:thfst-backend.bhfst-tool]
fn resolve_thfst_source(common: &CommonOptions, path_str: &str) -> Result<ThfstSource, i32> {
    let path = Path::new(path_str);
    if is_thfst_dir(path) {
        verbose_print(common, &format!("Using ready THFST directory {path_str}\n"));
        return Ok(ThfstSource {
            dir: path.to_path_buf(),
            _tempdir: None,
        });
    }

    verbose_print(
        common,
        &format!("Reading {path_str} and converting to THFST...\n"),
    );
    let mut instream = match HfstInputStream::new_filename(path_str) {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("cannot open {path_str}: {e}"));
            return Err(1);
        }
    };
    let orig: AnyTransducer = match instream.read() {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("cannot read {path_str}: {e}"));
            return Err(1);
        }
    };
    let converted = match convert_any_with_options(orig, ImplementationType::THFST_TYPE, "") {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("cannot convert {path_str}: {e}"));
            return Err(1);
        }
    };

    // convert_any_with_options into THFST_TYPE always yields the Thfst variant;
    // any other variant would be a bug in the conversion path.
    let mut thfst = match converted {
        AnyTransducer::Thfst(t) => t,
        AnyTransducer::Tropical(_) | AnyTransducer::OlW(_) | AnyTransducer::OlU(_) => {
            error(
                common,
                1,
                0,
                "internal error: THFST conversion did not yield a THFST transducer",
            );
            return Err(1);
        }
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(_) => {
            error(
                common,
                1,
                0,
                "internal error: THFST conversion did not yield a THFST transducer",
            );
            return Err(1);
        }
    };

    let tempdir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            error(common, 1, 0, &format!("cannot create temp directory: {e}"));
            return Err(1);
        }
    };
    // Serialize the converted THFST into the temp dir through the directory
    // sink (the same path `hfst fst2fst -f thfst` writes). hfst_format is forced
    // off by the THFST arm of new_filename.
    let dir = tempdir.path().join("converted.thfst");
    let dir_str = dir.to_string_lossy().into_owned();
    let mut outstream =
        match HfstOutputStream::new_filename(&dir_str, ImplementationType::THFST_TYPE, false) {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("cannot open THFST sink: {e}"));
                return Err(1);
            }
        };
    if let Err(e) = outstream.redirect(&mut thfst) {
        error(common, 1, 0, &format!("cannot write THFST: {e}"));
        return Err(1);
    }
    outstream.close();

    Ok(ThfstSource {
        dir,
        _tempdir: Some(tempdir),
    })
}

/// Insert the three THFST member files of `source.dir` into the archive under
/// the canonical `dir_name`, each Stored. Unlike thfst-tools (which reuses the
/// on-disk directory name), ours ALWAYS builds the box paths from the canonical
/// name — the divvunspell reader hard-codes `acceptor.default.thfst` /
/// `errmodel.default.thfst`, so re-homing differently-named inputs here is the
/// robust behaviour. [spec:hfst:sem:thfst-backend.bhfst-layout]
fn insert_thfst_dir(
    common: &CommonOptions,
    boxfile: &mut BoxWriter,
    source: &Path,
    dir_name: &str,
) -> Result<(), i32> {
    let dir_path = match BoxPath::new(dir_name) {
        Ok(p) => p,
        Err(e) => {
            error(common, 1, 0, &format!("invalid box path '{dir_name}': {e}"));
            return Err(1);
        }
    };
    if let Err(e) = boxfile.mkdir(dir_path, BoxHashMap::new()) {
        error(common, 1, 0, &format!("cannot mkdir '{dir_name}': {e}"));
        return Err(1);
    }
    for member in THFST_MEMBERS {
        let entry_path = source.join(member);
        let file = match std::fs::File::open(&entry_path) {
            Ok(f) => f,
            Err(e) => {
                error(
                    common,
                    1,
                    0,
                    &format!("cannot open '{}': {e}", entry_path.display()),
                );
                return Err(1);
            }
        };
        // The box entry path is the canonical dir name joined with the member,
        // regardless of the source directory name on disk.
        let member_box_path = match BoxPath::new(Path::new(dir_name).join(member)) {
            Ok(p) => p,
            Err(e) => {
                error(
                    common,
                    1,
                    0,
                    &format!("invalid box path '{dir_name}/{member}': {e}"),
                );
                return Err(1);
            }
        };
        if let Err(e) = boxfile.insert(
            &CompressionConfig::new(Compression::Stored),
            member_box_path,
            std::io::BufReader::new(file),
            BoxHashMap::new(),
        ) {
            error(
                common,
                1,
                0,
                &format!("cannot insert '{dir_name}/{member}': {e}"),
            );
            return Err(1);
        }
    }
    Ok(())
}

/// Build the meta.json bytes from `-X` (parse index.xml, rewrite ids) or `-m`
/// (verbatim). Returns None when neither option was given.
///
/// `-m` is embedded VERBATIM (the raw file bytes), validated only as
/// well-formed JSON (parsed into `serde_json::Value` and discarded) so that
/// unknown/extra fields survive byte-for-byte — a stricter parse into the mirror
/// structs would silently drop fields divvunspell may still carry.
/// [spec:hfst:sem:thfst-backend.meta-json]
fn build_meta_json(common: &CommonOptions, options: &Options) -> Result<Option<Vec<u8>>, i32> {
    match (&options.index_xml, &options.meta) {
        (Some(xml_path), None) => {
            verbose_print(
                common,
                &format!("Converting {xml_path} (index.xml) to meta.json...\n"),
            );
            let xml = match std::fs::read(xml_path) {
                Ok(b) => b,
                Err(e) => {
                    error(common, 1, 0, &format!("cannot read {xml_path}: {e}"));
                    return Err(1);
                }
            };
            let mut meta = match SpellerMetadata::from_xml_bytes(&xml) {
                Ok(m) => m,
                Err(e) => {
                    error(common, 1, 0, &format!("cannot parse {xml_path}: {e}"));
                    return Err(1);
                }
            };
            // Rewrite acceptor.id and errmodel.id: .hfst -> .thfst.
            // [spec:hfst:sem:thfst-backend.meta-json]
            meta.acceptor.id = meta.acceptor.id.replace(".hfst", ".thfst");
            meta.errmodel.id = meta.errmodel.id.replace(".hfst", ".thfst");
            let json = match serde_json::to_string_pretty(&meta) {
                Ok(s) => s,
                Err(e) => {
                    error(common, 1, 0, &format!("cannot serialize meta.json: {e}"));
                    return Err(1);
                }
            };
            Ok(Some(json.into_bytes()))
        }
        (None, Some(meta_path)) => {
            let bytes = match std::fs::read(meta_path) {
                Ok(b) => b,
                Err(e) => {
                    error(common, 1, 0, &format!("cannot read {meta_path}: {e}"));
                    return Err(1);
                }
            };
            // Validate as well-formed JSON (Value), but embed the raw bytes so
            // any extra fields survive verbatim.
            if let Err(e) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                error(common, 1, 0, &format!("{meta_path} is not valid JSON: {e}"));
                return Err(1);
            }
            Ok(Some(bytes))
        }
        (Some(_), Some(_)) => {
            error(
                common,
                1,
                0,
                "--index-xml and --meta are mutually exclusive",
            );
            Err(1)
        }
        (None, None) => Ok(None),
    }
}

/// Write the BHFST archive per `.bhfst-layout`: create with alignment 8; insert
/// acceptor dir, then errmodel dir, then meta.json (if present), all Stored;
/// finish. [spec:hfst:def:thfst-backend.bhfst-layout]
fn pack(
    common: &CommonOptions,
    options: &Options,
    acceptor: &ThfstSource,
    errmodel: &ThfstSource,
    output: &str,
) -> i32 {
    let meta_json = match build_meta_json(common, options) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // BoxWriter::create_with_alignment refuses an existing file (create_new);
    // the rest of the hfst suite overwrites its outputs, so match that.
    if Path::new(output).is_file() {
        if let Err(e) = std::fs::remove_file(output) {
            error(common, 1, 0, &format!("cannot overwrite {output}: {e}"));
            return 1;
        }
    }
    let mut boxfile = match BoxWriter::create_with_alignment(output, ALIGNMENT) {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("cannot create {output}: {e}"));
            return 1;
        }
    };

    verbose_print(common, &format!("Inserting {ACCEPTOR_DIRNAME}...\n"));
    if let Err(code) = insert_thfst_dir(common, &mut boxfile, &acceptor.dir, ACCEPTOR_DIRNAME) {
        return code;
    }
    verbose_print(common, &format!("Inserting {ERRMODEL_DIRNAME}...\n"));
    if let Err(code) = insert_thfst_dir(common, &mut boxfile, &errmodel.dir, ERRMODEL_DIRNAME) {
        return code;
    }

    if let Some(bytes) = meta_json {
        verbose_print(common, "Inserting meta.json...\n");
        let meta_path = match BoxPath::new("meta.json") {
            Ok(p) => p,
            Err(e) => {
                error(common, 1, 0, &format!("invalid box path 'meta.json': {e}"));
                return 1;
            }
        };
        if let Err(e) = boxfile.insert(
            &CompressionConfig::new(Compression::Stored),
            meta_path,
            std::io::Cursor::new(bytes),
            BoxHashMap::new(),
        ) {
            error(common, 1, 0, &format!("cannot insert meta.json: {e}"));
            return 1;
        }
    }

    if let Err(e) = boxfile.finish() {
        error(common, 1, 0, &format!("cannot finalise {output}: {e}"));
        return 1;
    }
    verbose_print(common, &format!("Wrote {output}\n"));
    0
}

// -----------------------------------------------------------------------------
// info mode
// -----------------------------------------------------------------------------

/// Open `path` as a box archive and print its meta.json to stdout. When
/// meta.json is absent the tool errors (nonzero exit), matching the reference
/// producer's `thfst-tools bhfst-info` behaviour (which bails when metadata is
/// missing). [spec:hfst:sem:thfst-backend.bhfst-tool]
fn info(common: &CommonOptions, path: &str) -> i32 {
    let reader = match BoxReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            error(common, 1, 0, &format!("cannot open {path}: {e}"));
            return 1;
        }
    };
    let meta_box_path = match BoxPath::new("meta.json") {
        Ok(p) => p,
        Err(e) => {
            error(common, 1, 0, &format!("invalid box path 'meta.json': {e}"));
            return 1;
        }
    };
    let record = match reader.find(&meta_box_path) {
        Ok(r) => r,
        Err(_) => {
            error(
                common,
                1,
                0,
                &format!("{path} contains no meta.json metadata"),
            );
            return 1;
        }
    };
    let file = match record.as_file() {
        Some(f) => f,
        None => {
            error(common, 1, 0, &format!("{path}: meta.json is not a file"));
            return 1;
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = reader.decompress(file, &mut bytes) {
        error(common, 1, 0, &format!("cannot read meta.json: {e}"));
        return 1;
    }
    // Print the meta.json text verbatim (the converted metadata).
    let mut out = std::io::stdout();
    if out.write_all(&bytes).is_err() {
        return 1;
    }
    if !bytes.ends_with(b"\n") {
        let _ = out.write_all(b"\n");
    }
    let _ = out.flush();
    0
}

// -----------------------------------------------------------------------------
// entry point
// -----------------------------------------------------------------------------

// [spec:hfst:def:thfst-backend.bhfst-tool]
// [spec:hfst:sem:thfst-backend.bhfst-tool]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstBhfst");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // Info mode short-circuits everything else.
    if let Some(info_path) = &options.info {
        if options.acceptor.is_some()
            || options.errmodel.is_some()
            || common.output_named
            || options.index_xml.is_some()
            || options.meta.is_some()
        {
            error(
                &common,
                1,
                0,
                "--info cannot be combined with packing options",
            );
            return 1;
        }
        return info(&common, info_path);
    }

    // Pack mode: -a, -e and -o are all required. '-o' flows through the common
    // option handler, which populates `common.output_filename`/`output_named`
    // (and maps '-o -' to the "<stdout>" sentinel, which a box archive cannot
    // be written to). [spec:hfst:sem:thfst-backend.bhfst-tool]
    let (Some(acceptor_path), Some(errmodel_path), true) =
        (&options.acceptor, &options.errmodel, common.output_named)
    else {
        error(
            &common,
            1,
            0,
            "pack mode requires --acceptor, --errmodel and --output (or use --info)",
        );
        return 1;
    };
    if common.output_filename == "<stdout>" {
        error(
            &common,
            1,
            0,
            "writing a .bhfst archive to standard output is not supported,\n\
             use 'hfst-bhfst [--output|-o] OUT.bhfst' instead",
        );
        return 1;
    }
    let output = common.output_filename.clone();

    if options.index_xml.is_some() && options.meta.is_some() {
        error(
            &common,
            1,
            0,
            "--index-xml and --meta are mutually exclusive",
        );
        return 1;
    }

    let acceptor = match resolve_thfst_source(&common, acceptor_path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let errmodel = match resolve_thfst_source(&common, errmodel_path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    pack(&common, &options, &acceptor, &errmodel, &output)
}
