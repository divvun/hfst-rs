//! `hfst-bhfst` — pack an acceptor and an error model (each THFST, or DHFST
//! stored as it is) plus speller metadata into a BHFST speller archive,
//! convert a whole zhfst into one, or inspect an existing one. There is NO C++ HFST ancestor: this tool is authored greenfield
//! against `docs/spec/port/back-ends/thfst/thfst-backend.md`, and its contract
//! is divvunspell compatibility — the archives it writes are consumed by
//! `github.com/divvun/divvunspell` (its `src/archive/boxf.rs` loader), and the
//! reference producer it mirrors is divvunspell's `thfst-tools` (`-z` is its
//! `zhfst-to-bhfst`).
//!
//! Option handling is clap 4 derive through [`crate::cli`]. Unlike the
//! algebra/lookup tools it has NO default input/output streams: pack mode
//! requires `-a`, `-e`, `-o` (or `-z`, `-o`); info mode requires only `-I`. It
//! therefore never runs the check-params fragments (no standard in/out stream
//! to resolve).

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    convert_any_with_options, error, hfst_set_program_name, verbose_print,
};
use box_format::{
    BoxPath, Compression, CompressionConfig, HashMap as BoxHashMap, sync::BoxReader,
    sync::BoxWriter,
};
use hfst::dhfst_header::{DhfstType, PREFIX_LEN};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use std::io::{Read, Write};
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

/// The single-file members divvunspell reads a DHFST error model and a
/// DHFST acceptor from, in place of the THFST directories.
// [spec:hfst:def:dhfst.bhfst-member+2]
const DHFST_ERRMODEL_MEMBER: &str = "errmodel.default.dhfst";
// [spec:hfst:def:dhfst.bhfst-member+2]
const DHFST_ACCEPTOR_MEMBER: &str = "acceptor.default.dhfst";

/// The meta.json key the speller runtime configuration rides under, and the
/// zhfst member it comes from.
// [spec:hfst:def:thfst-backend.speller-config]
const SPELLER_CONFIG_KEY: &str = "spellerConfig";
const SPELLER_CONFIG_MEMBER: &str = "speller-config.json";

/// The zhfst members the packer reads: the metadata, plus the fallback names of
/// the two transducers when index.xml names ids the archive lacks.
// [spec:hfst:sem:thfst-backend.zhfst-input]
const INDEX_XML_MEMBER: &str = "index.xml";
const DEFAULT_ACCEPTOR_MEMBER: &str = "acceptor.default.hfst";
const DEFAULT_ERRMODEL_MEMBER: &str = "errmodel.default.hfst";

/// hfst-bhfst's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Pack an acceptor and an error model, each THFST or DHFST (+ speller metadata), into a BHFST archive",
    after_help = "Pack mode needs -a, -e and -o, or -z and -o. Info mode needs only -I."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Acceptor: a DHFST acceptor (type 2, stored as it is), a .thfst
    /// directory, or any transducer file (auto-converted to THFST)
    #[arg(
        short = 'a',
        long = "acceptor",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    acceptor: Option<String>,

    /// Error model: a DHFST error model (type 1, stored as it is), a .thfst
    /// directory, or any transducer file (auto-converted to THFST)
    #[arg(
        short = 'e',
        long = "errmodel",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    errmodel: Option<String>,

    /// zhfst index.xml, converted to meta.json (ids .hfst->.thfst)
    #[arg(
        short = 'X',
        long = "index-xml",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    index_xml: Option<String>,

    /// meta.json embedded verbatim (mutually exclusive with -X)
    #[arg(
        short = 'm',
        long = "meta",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    meta: Option<String>,

    /// A whole zhfst archive to convert (instead of -a/-e/-X/-m)
    #[arg(
        short = 'z',
        long = "zhfst",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    zhfst: Option<String>,

    /// speller-config.json carried into meta.json as "spellerConfig"
    #[arg(
        short = 'c',
        long = "speller-config",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    speller_config: Option<String>,

    /// Print metadata of an existing .bhfst and exit
    #[arg(
        short = 'I',
        long = "info",
        value_name = "FILE",
        allow_hyphen_values = true
    )]
    info: Option<String>,

    /// (rejected: this tool takes no positional arguments)
    #[arg(value_name = "ARG", num_args = 0.., hide = true)]
    infiles: Vec<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    /// No default output transducer stream: mirror only check-params-common's
    /// message routing (stderr when no '-o'), never its "<stdout>" default.
    fn apply_io(&self, opts: &mut CommonOptions) {
        if !opts.output_named {
            opts.message_to_stderr = true;
        }
    }

    fn applies_check_common_params(&self) -> bool {
        false
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The C rejected leftover free arguments right after its getopt loop.
        if !self.infiles.is_empty() {
            error(
                opts,
                1,
                0,
                "hfst-bhfst takes no positional arguments; use -a/-e/-o, -z/-o or -I",
            );
            return Err(1);
        }
        Ok(())
    }
}

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
    /// '-z/--zhfst': a whole zhfst archive to convert (instead of -a/-e/-X/-m).
    zhfst: Option<String>,
    /// '-c/--speller-config': speller runtime config for meta.json.
    speller_config: Option<String>,
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

    /// divvunspell's `SpellerMetadata`, plus the `spellerConfig` key the BHFST
    /// meta.json carries the speller runtime configuration in. The extra key is
    /// skipped entirely when there is no configuration, so an archive built
    /// without one is byte-identical to what this tool wrote before the key
    /// existed; it is never present in the XML this struct also deserializes
    /// from.
    // [spec:hfst:def:thfst-backend.speller-config]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SpellerMetadata {
        pub info: SpellerMetadataInfo,
        pub acceptor: SpellerMetadataAcceptor,
        pub errmodel: SpellerMetadataErrmodel,
        #[serde(
            rename = "spellerConfig",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        pub speller_config: Option<serde_json::Value>,
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

/// Resolve a `-a`/`-e` source that is not DHFST to a THFST directory. A ready
/// `.thfst` dir is used in place; anything else is read via `HfstInputStream`,
/// converted to THFST via the standard format-conversion path, and serialized
/// into a temp dir.
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
    if let Err(e) = outstream.write(&mut thfst) {
        error(common, 1, 0, &format!("cannot write THFST: {e}"));
        return Err(1);
    }
    outstream.close();

    Ok(ThfstSource {
        dir,
        _tempdir: Some(tempdir),
    })
}

/// Whether `path` is a file that starts the way a DHFST file does.
// [spec:hfst:sem:dhfst.bhfst-member+2]
fn is_dhfst_file(path: &Path) -> bool {
    let mut head = [0u8; 5];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|()| hfst::dhfst_header::has_magic(&head))
}

/// The type a DHFST file's header declares. A type or version the reader
/// does not know is refused, naming what the header holds.
// [spec:hfst:sem:dhfst.bhfst-member+2]
fn dhfst_file_type(common: &CommonOptions, path_str: &str) -> Result<DhfstType, i32> {
    let mut head = Vec::with_capacity(PREFIX_LEN);
    let read = std::fs::File::open(path_str)
        .and_then(|f| f.take(PREFIX_LEN as u64).read_to_end(&mut head));
    if let Err(e) = read {
        error(common, 1, 0, &format!("cannot read {path_str}: {e}"));
        return Err(1);
    }
    hfst::dhfst_header::read_type(&head).map_err(|e| {
        error(
            common,
            1,
            0,
            &format!("{path_str} is not a DHFST file this tool reads: {e}"),
        );
        1
    })
}

/// An acceptor or an error model resolved for packing.
enum PackSource {
    /// A THFST directory, entered as `acceptor.default.thfst` or
    /// `errmodel.default.thfst`.
    Thfst(ThfstSource),
    /// A validated DHFST file's bytes, entered as they are as
    /// `acceptor.default.dhfst` or `errmodel.default.dhfst`.
    Dhfst(Vec<u8>),
}

/// The ids meta.json gives the acceptor at `acceptor` and the error model
/// at `errmodel`: their DHFST member names for DHFST files, and none for the
/// rest, which keep the THFST directories' ids.
fn dhfst_meta_ids(acceptor: &str, errmodel: &str) -> (Option<&'static str>, Option<&'static str>) {
    let id = |path: &str, member| is_dhfst_file(Path::new(path)).then_some(member);
    (
        id(acceptor, DHFST_ACCEPTOR_MEMBER),
        id(errmodel, DHFST_ERRMODEL_MEMBER),
    )
}

/// Resolve a `-a` (`wanted` an acceptor) or `-e` (`wanted` an error model)
/// source: a DHFST file, told by its header, must be of the type wanted, and
/// is read and validated as divvunspell validates it before loading;
/// anything else resolves as [`resolve_thfst_source`] does.
// [spec:hfst:sem:dhfst.bhfst-member+2]
fn resolve_source(
    common: &CommonOptions,
    path_str: &str,
    wanted: DhfstType,
) -> Result<PackSource, i32> {
    if !is_dhfst_file(Path::new(path_str)) {
        return resolve_thfst_source(common, path_str).map(PackSource::Thfst);
    }
    let kind = dhfst_file_type(common, path_str)?;
    if kind != wanted {
        error(
            common,
            1,
            0,
            &format!(
                "{path_str} is a DHFST {kind} (type {}); {} is type {}",
                kind.byte(),
                wanted.with_article(),
                wanted.byte()
            ),
        );
        return Err(1);
    }
    verbose_print(
        common,
        &format!("Using DHFST {wanted} {path_str} as it is\n"),
    );
    let bytes = read_file(common, path_str)?;
    let loads = match wanted {
        DhfstType::ErrorModel => hfst::dhfst::DhfstReader::parse(&bytes).map(|_| ()),
        DhfstType::Acceptor => hfst::dhfst_acceptor::AcceptorReader::parse(&bytes).map(|_| ()),
    };
    if let Err(e) = loads {
        error(
            common,
            1,
            0,
            &format!("{path_str} is not a DHFST {wanted} divvunspell can load: {e}"),
        );
        return Err(1);
    }
    Ok(PackSource::Dhfst(bytes))
}

/// Insert a DHFST file as the single member `member`, Stored.
// [spec:hfst:sem:dhfst.bhfst-member+2]
fn insert_dhfst(
    common: &CommonOptions,
    boxfile: &mut BoxWriter,
    bytes: &[u8],
    member: &str,
) -> Result<(), i32> {
    let path = match BoxPath::new(member) {
        Ok(p) => p,
        Err(e) => {
            error(common, 1, 0, &format!("invalid box path '{member}': {e}"));
            return Err(1);
        }
    };
    if let Err(e) = boxfile.insert(
        &CompressionConfig::new(Compression::Stored),
        path,
        std::io::Cursor::new(bytes),
        BoxHashMap::new(),
    ) {
        error(common, 1, 0, &format!("cannot insert '{member}': {e}"));
        return Err(1);
    }
    Ok(())
}

/// Insert an acceptor or error model under its THFST directory `dir_name`
/// or its DHFST member `dhfst_member`.
// [spec:hfst:sem:dhfst.bhfst-member+2]
fn insert_source(
    common: &CommonOptions,
    boxfile: &mut BoxWriter,
    source: &PackSource,
    dir_name: &str,
    dhfst_member: &str,
) -> Result<(), i32> {
    match source {
        PackSource::Thfst(source) => {
            verbose_print(common, &format!("Inserting {dir_name}...\n"));
            insert_thfst_dir(common, boxfile, &source.dir, dir_name)
        }
        PackSource::Dhfst(bytes) => {
            verbose_print(common, &format!("Inserting {dhfst_member}...\n"));
            insert_dhfst(common, boxfile, bytes, dhfst_member)
        }
    }
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

/// Where the metadata for meta.json comes from, with the bytes already in hand
/// (a zhfst member has no path of its own, so the origin travels as a label used
/// only in diagnostics).
enum MetaSource {
    /// Neither `-X` nor `-m` nor `-z`: the archive gets no meta.json.
    None,
    /// A zhfst index.xml, to be parsed and converted per `.meta-json`.
    IndexXml { origin: String, bytes: Vec<u8> },
    /// A caller-supplied meta.json, embedded verbatim.
    Verbatim { origin: String, bytes: Vec<u8> },
}

impl MetaSource {
    /// Resolve `-X`/`-m` to a source, reading the named file.
    fn from_options(common: &CommonOptions, options: &Options) -> Result<MetaSource, i32> {
        match (&options.index_xml, &options.meta) {
            (Some(xml_path), None) => {
                verbose_print(
                    common,
                    &format!("Converting {xml_path} (index.xml) to meta.json...\n"),
                );
                Ok(MetaSource::IndexXml {
                    origin: xml_path.clone(),
                    bytes: read_file(common, xml_path)?,
                })
            }
            (None, Some(meta_path)) => Ok(MetaSource::Verbatim {
                origin: meta_path.clone(),
                bytes: read_file(common, meta_path)?,
            }),
            (Some(_), Some(_)) => {
                error(
                    common,
                    1,
                    0,
                    "--index-xml and --meta are mutually exclusive",
                );
                Err(1)
            }
            (None, None) => Ok(MetaSource::None),
        }
    }
}

/// Read a whole file, reporting failure through the common error path.
fn read_file(common: &CommonOptions, path: &str) -> Result<Vec<u8>, i32> {
    match std::fs::read(path) {
        Ok(b) => Ok(b),
        Err(e) => {
            error(common, 1, 0, &format!("cannot read {path}: {e}"));
            Err(1)
        }
    }
}

/// Parse a speller runtime configuration: well-formed JSON, and a JSON OBJECT.
/// Both failures are fatal — a configuration that divvunspell could not load is
/// caught here, at build time, rather than at speller startup.
// [spec:hfst:sem:thfst-backend.speller-config]
fn parse_speller_config(
    common: &CommonOptions,
    origin: &str,
    bytes: &[u8],
) -> Result<serde_json::Value, i32> {
    let value: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            error(
                common,
                1,
                0,
                &format!("{origin} is not a valid speller configuration: {e}"),
            );
            return Err(1);
        }
    };
    if !value.is_object() {
        error(
            common,
            1,
            0,
            &format!(
                "{origin} is not a valid speller configuration: \
                 expected a JSON object, found {}",
                json_kind(&value)
            ),
        );
        return Err(1);
    }
    Ok(value)
}

/// The JSON type name of `value`, for the "expected an object, found ..."
/// diagnostic.
fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

/// Resolve the speller runtime configuration for this run: an explicit `-c`
/// file wins, otherwise the zhfst's `speller-config.json` member (when `-z`
/// carried one), otherwise none.
// [spec:hfst:sem:thfst-backend.speller-config]
fn resolve_speller_config(
    common: &CommonOptions,
    options: &Options,
    from_zhfst: Option<(String, Vec<u8>)>,
) -> Result<Option<serde_json::Value>, i32> {
    if let Some(path) = &options.speller_config {
        verbose_print(
            common,
            &format!("Reading speller configuration from {path}...\n"),
        );
        let bytes = read_file(common, path)?;
        return Ok(Some(parse_speller_config(common, path, &bytes)?));
    }
    match from_zhfst {
        Some((origin, bytes)) => {
            verbose_print(
                common,
                &format!("Carrying the speller configuration from {origin}...\n"),
            );
            Ok(Some(parse_speller_config(common, &origin, &bytes)?))
        }
        None => Ok(None),
    }
}

/// Build the meta.json bytes from `source`, merging `config` in under
/// `spellerConfig` when there is one. Returns None when there is no metadata.
/// `ids.0` and `ids.1` override the ids an index.xml's acceptor and error
/// model are given, which otherwise have `.hfst` rewritten to `.thfst`.
///
/// A `Verbatim` source is embedded VERBATIM (the raw file bytes), validated only
/// as well-formed JSON (parsed into `serde_json::Value` and discarded) so that
/// unknown/extra fields survive byte-for-byte — a stricter parse into the mirror
/// structs would silently drop fields divvunspell may still carry. Merging a
/// configuration into one is the single exception: the object has to be rebuilt
/// to gain the key, so those bytes are re-serialized.
// [spec:hfst:sem:thfst-backend.meta-json]
// [spec:hfst:sem:thfst-backend.speller-config]
fn build_meta_json(
    common: &CommonOptions,
    source: MetaSource,
    config: Option<serde_json::Value>,
    (acceptor_id, errmodel_id): (Option<&str>, Option<&str>),
) -> Result<Option<Vec<u8>>, i32> {
    match source {
        MetaSource::IndexXml { origin, bytes } => {
            let mut meta = match SpellerMetadata::from_xml_bytes(&bytes) {
                Ok(m) => m,
                Err(e) => {
                    error(common, 1, 0, &format!("cannot parse {origin}: {e}"));
                    return Err(1);
                }
            };
            // Rewrite acceptor.id and errmodel.id: .hfst -> .thfst, unless the
            // transducer is DHFST, whose member name is fixed.
            // [spec:hfst:sem:thfst-backend.meta-json]
            // [spec:hfst:sem:dhfst.bhfst-member+2]
            meta.acceptor.id = match acceptor_id {
                Some(id) => id.to_string(),
                None => meta.acceptor.id.replace(".hfst", ".thfst"),
            };
            meta.errmodel.id = match errmodel_id {
                Some(id) => id.to_string(),
                None => meta.errmodel.id.replace(".hfst", ".thfst"),
            };
            meta.speller_config = config;
            let json = match serde_json::to_string_pretty(&meta) {
                Ok(s) => s,
                Err(e) => {
                    error(common, 1, 0, &format!("cannot serialize meta.json: {e}"));
                    return Err(1);
                }
            };
            Ok(Some(json.into_bytes()))
        }
        MetaSource::Verbatim { origin, bytes } => {
            let parsed = match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{origin} is not valid JSON: {e}"));
                    return Err(1);
                }
            };
            let Some(config) = config else {
                return Ok(Some(bytes));
            };
            let serde_json::Value::Object(mut object) = parsed else {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "cannot add a speller configuration to {origin}: \
                         expected a JSON object, found {}",
                        json_kind(&parsed)
                    ),
                );
                return Err(1);
            };
            object.insert(SPELLER_CONFIG_KEY.to_string(), config);
            match serde_json::to_string_pretty(&object) {
                Ok(s) => Ok(Some(s.into_bytes())),
                Err(e) => {
                    error(common, 1, 0, &format!("cannot serialize meta.json: {e}"));
                    Err(1)
                }
            }
        }
        MetaSource::None => {
            if config.is_some() {
                error(
                    common,
                    1,
                    0,
                    "a speller configuration needs metadata to ride in;\n\
                     pass --index-xml, --meta or --zhfst as well",
                );
                return Err(1);
            }
            Ok(None)
        }
    }
}

// -----------------------------------------------------------------------------
// zhfst input
// -----------------------------------------------------------------------------

/// A zhfst archive unpacked into a temp dir: the two transducer members written
/// out as files (so the ordinary `-a`/`-e` resolution path can read them), plus
/// the index.xml and optional speller-config.json bytes.
// [spec:hfst:def:thfst-backend.zhfst-input]
struct ZhfstInput {
    acceptor: String,
    errmodel: String,
    index_xml: Vec<u8>,
    speller_config: Option<Vec<u8>>,
    // Keeps the extracted transducer files alive until they have been read.
    _tempdir: tempfile::TempDir,
}

/// Read `name` out of the zip, or None when the archive has no such member.
/// Anything other than "not found" is fatal.
fn zhfst_member<R: Read + std::io::Seek>(
    common: &CommonOptions,
    archive: &mut zip::ZipArchive<R>,
    path: &str,
    name: &str,
) -> Result<Option<Vec<u8>>, i32> {
    let mut entry = match archive.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => {
            error(common, 1, 0, &format!("cannot read {path}: {name}: {e}"));
            return Err(1);
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = entry.read_to_end(&mut bytes) {
        error(common, 1, 0, &format!("cannot read {path}: {name}: {e}"));
        return Err(1);
    }
    Ok(Some(bytes))
}

/// Read `name` out of the zip and write it into `dir`, returning the path
/// written. The member is required.
fn extract_zhfst_member<R: Read + std::io::Seek>(
    common: &CommonOptions,
    archive: &mut zip::ZipArchive<R>,
    path: &str,
    name: &str,
    dir: &Path,
) -> Result<String, i32> {
    let Some(bytes) = zhfst_member(common, archive, path, name)? else {
        error(common, 1, 0, &format!("{path} contains no {name}"));
        return Err(1);
    };
    // The member name is only ever one of the ids from index.xml, so take just
    // its file-name component: a member named with a path would otherwise write
    // outside the temp dir.
    let leaf = Path::new(name).file_name().unwrap_or(name.as_ref());
    let out = dir.join(leaf);
    if let Err(e) = std::fs::write(&out, &bytes) {
        error(
            common,
            1,
            0,
            &format!("cannot write {}: {e}", out.display()),
        );
        return Err(1);
    }
    Ok(out.to_string_lossy().into_owned())
}

/// Open a zhfst and lay its members out for packing. index.xml is required: it
/// carries both the metadata and the ids naming the two transducer members; a
/// named id the archive does not carry falls back to the conventional
/// `acceptor.default.hfst` / `errmodel.default.hfst`.
// [spec:hfst:sem:thfst-backend.zhfst-input]
fn read_zhfst(common: &CommonOptions, path: &str) -> Result<ZhfstInput, i32> {
    verbose_print(common, &format!("Opening {path} (zhfst)...\n"));
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            error(common, 1, 0, &format!("cannot open {path}: {e}"));
            return Err(1);
        }
    };
    let mut archive = match zip::ZipArchive::new(std::io::BufReader::new(file)) {
        Ok(a) => a,
        Err(e) => {
            error(common, 1, 0, &format!("cannot open {path}: {e}"));
            return Err(1);
        }
    };

    let Some(index_xml) = zhfst_member(common, &mut archive, path, INDEX_XML_MEMBER)? else {
        error(
            common,
            1,
            0,
            &format!("{path} contains no {INDEX_XML_MEMBER}"),
        );
        return Err(1);
    };
    // The ids as written in the XML — the member names inside the zip, before
    // meta.json's .hfst -> .thfst rewrite.
    let meta = match SpellerMetadata::from_xml_bytes(&index_xml) {
        Ok(m) => m,
        Err(e) => {
            error(
                common,
                1,
                0,
                &format!("cannot parse {path}: {INDEX_XML_MEMBER}: {e}"),
            );
            return Err(1);
        }
    };
    let acceptor_member =
        member_or_default(&mut archive, &meta.acceptor.id, DEFAULT_ACCEPTOR_MEMBER);
    let errmodel_member =
        member_or_default(&mut archive, &meta.errmodel.id, DEFAULT_ERRMODEL_MEMBER);

    let speller_config = zhfst_member(common, &mut archive, path, SPELLER_CONFIG_MEMBER)?;

    let tempdir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            error(common, 1, 0, &format!("cannot create temp directory: {e}"));
            return Err(1);
        }
    };
    verbose_print(
        common,
        &format!("Extracting {acceptor_member} and {errmodel_member}...\n"),
    );
    let acceptor =
        extract_zhfst_member(common, &mut archive, path, &acceptor_member, tempdir.path())?;
    let errmodel =
        extract_zhfst_member(common, &mut archive, path, &errmodel_member, tempdir.path())?;

    Ok(ZhfstInput {
        acceptor,
        errmodel,
        index_xml,
        speller_config,
        _tempdir: tempdir,
    })
}

/// `id` when the archive carries a member under that name, else `fallback`.
fn member_or_default<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    id: &str,
    fallback: &str,
) -> String {
    if archive.by_name(id).is_ok() {
        id.to_string()
    } else {
        fallback.to_string()
    }
}

/// Write the BHFST archive per `.bhfst-layout`: create with alignment 8; insert
/// the acceptor, then the error model, then meta.json (if present), all
/// Stored; finish. [spec:hfst:def:thfst-backend.bhfst-layout]
fn pack(
    common: &CommonOptions,
    meta_json: Option<Vec<u8>>,
    acceptor: &PackSource,
    errmodel: &PackSource,
    output: &str,
) -> i32 {
    // BoxWriter::create_with_alignment refuses an existing file (create_new);
    // the rest of the hfst suite overwrites its outputs, so match that.
    if Path::new(output).is_file()
        && let Err(e) = std::fs::remove_file(output)
    {
        error(common, 1, 0, &format!("cannot overwrite {output}: {e}"));
        return 1;
    }
    let mut boxfile = match BoxWriter::create_with_alignment(output, ALIGNMENT) {
        Ok(v) => v,
        Err(e) => {
            error(common, 1, 0, &format!("cannot create {output}: {e}"));
            return 1;
        }
    };

    let inserted = insert_source(
        common,
        &mut boxfile,
        acceptor,
        ACCEPTOR_DIRNAME,
        DHFST_ACCEPTOR_MEMBER,
    )
    .and_then(|()| {
        insert_source(
            common,
            &mut boxfile,
            errmodel,
            ERRMODEL_DIRNAME,
            DHFST_ERRMODEL_MEMBER,
        )
    });
    if let Err(code) = inserted {
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
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstBhfst");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        acceptor: args.acceptor.clone(),
        errmodel: args.errmodel.clone(),
        index_xml: args.index_xml.clone(),
        meta: args.meta.clone(),
        zhfst: args.zhfst.clone(),
        speller_config: args.speller_config.clone(),
        info: args.info.clone(),
    };

    // Info mode short-circuits everything else.
    if let Some(info_path) = &options.info {
        if options.acceptor.is_some()
            || options.errmodel.is_some()
            || common.output_named
            || options.index_xml.is_some()
            || options.meta.is_some()
            || options.zhfst.is_some()
            || options.speller_config.is_some()
        {
            error(
                &common,
                1,
                0,
                "--info cannot be combined with packing options",
            );
            return Err(1);
        }
        return cli::from_code(info(&common, info_path));
    }

    // '-o' flows through the common option handler, which populates
    // `common.output_filename`/`output_named` (and maps '-o -' to the
    // "<stdout>" sentinel, which a box archive cannot be written to). Both pack
    // modes require it. [spec:hfst:sem:thfst-backend.bhfst-tool]
    if common.output_named && common.output_filename == "<stdout>" {
        error(
            &common,
            1,
            0,
            "writing a .bhfst archive to standard output is not supported,\n\
             use 'hfst-bhfst [--output|-o] OUT.bhfst' instead",
        );
        return Err(1);
    }

    // zhfst mode: one archive in, one archive out.
    // [spec:hfst:sem:thfst-backend.zhfst-input]
    if let Some(zhfst_path) = &options.zhfst {
        if options.acceptor.is_some()
            || options.errmodel.is_some()
            || options.index_xml.is_some()
            || options.meta.is_some()
        {
            error(
                &common,
                1,
                0,
                "--zhfst cannot be combined with --acceptor, --errmodel, \
                 --index-xml or --meta",
            );
            return Err(1);
        }
        if !common.output_named {
            error(&common, 1, 0, "--zhfst also requires --output");
            return Err(1);
        }
        let output = common.output_filename.clone();
        let zhfst = read_zhfst(&common, zhfst_path)?;
        let config = resolve_speller_config(
            &common,
            &options,
            zhfst
                .speller_config
                .clone()
                .map(|bytes| (format!("{zhfst_path}: {SPELLER_CONFIG_MEMBER}"), bytes)),
        )?;
        let meta_json = build_meta_json(
            &common,
            MetaSource::IndexXml {
                origin: format!("{zhfst_path}: {INDEX_XML_MEMBER}"),
                bytes: zhfst.index_xml.clone(),
            },
            config,
            dhfst_meta_ids(&zhfst.acceptor, &zhfst.errmodel),
        )?;
        let acceptor = resolve_source(&common, &zhfst.acceptor, DhfstType::Acceptor)?;
        let errmodel = resolve_source(&common, &zhfst.errmodel, DhfstType::ErrorModel)?;
        return cli::from_code(pack(&common, meta_json, &acceptor, &errmodel, &output));
    }

    // Loose-file pack mode: -a, -e and -o are all required.
    // [spec:hfst:sem:thfst-backend.bhfst-tool]
    let (Some(acceptor_path), Some(errmodel_path), true) =
        (&options.acceptor, &options.errmodel, common.output_named)
    else {
        error(
            &common,
            1,
            0,
            "pack mode requires --acceptor, --errmodel and --output \
             (or --zhfst and --output, or --info)",
        );
        return Err(1);
    };
    let output = common.output_filename.clone();

    if options.index_xml.is_some() && options.meta.is_some() {
        error(
            &common,
            1,
            0,
            "--index-xml and --meta are mutually exclusive",
        );
        return Err(1);
    }

    let meta_source = MetaSource::from_options(&common, &options)?;
    let config = resolve_speller_config(&common, &options, None)?;
    let meta_json = build_meta_json(
        &common,
        meta_source,
        config,
        dhfst_meta_ids(acceptor_path, errmodel_path),
    )?;

    let acceptor = resolve_source(&common, acceptor_path, DhfstType::Acceptor)?;
    let errmodel = resolve_source(&common, errmodel_path, DhfstType::ErrorModel)?;

    cli::from_code(pack(&common, meta_json, &acceptor, &errmodel, &output))
}

#[cfg(test)]
mod tests {
    use super::{dhfst_meta_ids, is_dhfst_file};

    // [spec:hfst:sem:dhfst.bhfst-member+2/test]
    #[test]
    fn tells_dhfst_files_by_their_header() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dhfst = dir.path().join("errmodel.dhfst");
        let hfst = dir.path().join("errmodel.hfst");
        let short = dir.path().join("short");
        std::fs::write(&dhfst, b"DHFST\x01\x01\0").expect("write errmodel.dhfst");
        std::fs::write(&hfst, b"HFST\0\0\0\0").expect("write errmodel.hfst");
        std::fs::write(&short, b"DHF").expect("write short");
        assert!(is_dhfst_file(&dhfst));
        for not_dhfst in [
            &hfst,
            &short,
            &dir.path().join("missing"),
            &dir.path().to_path_buf(),
        ] {
            assert!(!is_dhfst_file(not_dhfst), "{}", not_dhfst.display());
        }
        let dhfst = dhfst.to_string_lossy();
        let hfst = hfst.to_string_lossy();
        assert_eq!(
            dhfst_meta_ids(&dhfst, &hfst),
            (Some("acceptor.default.dhfst"), None)
        );
        assert_eq!(
            dhfst_meta_ids(&hfst, &dhfst),
            (None, Some("errmodel.default.dhfst"))
        );
    }
}
