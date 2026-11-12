//! `hfst bhfst`: packing an acceptor and an error model, each THFST or DHFST,
//! with speller metadata into a BHFST archive, converting a zhfst into one,
//! and reading the metadata back. Everything here works on bytes; the tool
//! reads and writes the files.
//!
//! The archives are what divvunspell loads (its `src/archive/boxf.rs`), and
//! the producer they mirror is its `thfst-tools`.

use box_format::core::{ArchiveReader, ArchiveWriter};
use box_format::{BoxPath, Compression, HashMap as BoxHashMap};
use std::io::Read;
use std::num::NonZeroU64;

use crate::dhfst_header::DhfstType;
use crate::file_resolver::FileResolver;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_input_stream::HfstInputStream;
use crate::hfst_transducer::AnyTransducer;
use crate::thfst_io::ThfstMembers;

/// Box data alignment: divvunspell maps the member files at their raw
/// archive offsets, so 8-byte alignment is a hard requirement.
// [spec:hfst:sem:thfst-backend.bhfst-layout]
const ALIGNMENT: u32 = 8;

/// The directory names divvunspell looks the THFST members up under.
// [spec:hfst:sem:thfst-backend.bhfst-layout]
pub const ACCEPTOR_DIRNAME: &str = "acceptor.default.thfst";
pub const ERRMODEL_DIRNAME: &str = "errmodel.default.thfst";

/// The single-file members divvunspell reads a DHFST acceptor and a DHFST
/// error model from, in place of the THFST directories.
// [spec:hfst:def:dhfst.bhfst-member+2]
pub const DHFST_ACCEPTOR_MEMBER: &str = "acceptor.default.dhfst";
pub const DHFST_ERRMODEL_MEMBER: &str = "errmodel.default.dhfst";

/// The meta.json key the speller runtime configuration rides under, and the
/// zhfst member it comes from.
// [spec:hfst:def:thfst-backend.speller-config]
const SPELLER_CONFIG_KEY: &str = "spellerConfig";
pub const SPELLER_CONFIG_MEMBER: &str = "speller-config.json";

/// The zhfst members read: the metadata, and the names the two transducers
/// fall back to when index.xml names ids the archive lacks.
// [spec:hfst:sem:thfst-backend.zhfst-input]
pub const INDEX_XML_MEMBER: &str = "index.xml";
const DEFAULT_ACCEPTOR_MEMBER: &str = "acceptor.default.hfst";
const DEFAULT_ERRMODEL_MEMBER: &str = "errmodel.default.hfst";

const META_JSON: &str = "meta.json";

/// Why a pack, a conversion or a read failed. The message is the one the
/// tool prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BhfstError {
    /// A speller configuration was given with no metadata to carry it.
    ConfigWithoutMetadata,
    Message(String),
}

impl std::fmt::Display for BhfstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BhfstError::ConfigWithoutMetadata => {
                write!(f, "a speller configuration needs metadata to ride in")
            }
            BhfstError::Message(message) => write!(f, "{}", message),
        }
    }
}

/// `Err` of the failure `message`.
macro_rules! fail {
    ($message:expr $(,)?) => {
        Err(BhfstError::Message($message))
    };
}

/// A progress message for a tool to print when verbose.
pub type Progress<'p> = &'p mut dyn FnMut(&str);

// -----------------------------------------------------------------------------
// Metadata: the serde shapes of divvunspell's SpellerMetadata
// (src/archive/meta.rs). Field names, the "$value" rename, the `type` default
// and the optional fields are mirrored exactly, so the JSON round-trips with
// divvunspell byte for byte.
// [spec:hfst:def:thfst-backend.meta-json]
// -----------------------------------------------------------------------------

/// divvunspell's `SpellerMetadata`, plus the `spellerConfig` key a BHFST
/// meta.json carries the speller runtime configuration in. The key is left
/// out when there is no configuration, and never appears in index.xml.
// [spec:hfst:def:thfst-backend.speller-config]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SpellerTitle {
    pub lang: Option<String>,
    #[serde(rename = "$value")]
    pub value: String,
}

/// divvunspell's `SpellerMetadataInfo`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SpellerMetadataInfo {
    pub locale: String,
    pub title: Vec<SpellerTitle>,
    pub description: String,
    pub producer: String,
}

/// divvunspell's `SpellerMetadataAcceptor`; `type_` carries the `type`
/// attribute, "" when absent.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SpellerMetadataAcceptor {
    #[serde(rename = "type", default)]
    pub type_: String,
    pub id: String,
    pub title: Vec<SpellerTitle>,
    pub description: String,
    pub continuation: Option<String>,
}

/// divvunspell's `SpellerMetadataErrmodel`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SpellerMetadataErrmodel {
    pub id: String,
    pub title: Vec<SpellerTitle>,
    pub description: String,
}

impl SpellerMetadata {
    /// Parse a zhfst index.xml with the serde-xml-rs 0.6 parser configuration
    /// divvunspell uses: whitespace trimmed, comments skipped, characters
    /// coalesced.
    // [spec:hfst:sem:thfst-backend.meta-json]
    pub fn from_xml_bytes(bytes: &[u8]) -> Result<SpellerMetadata, serde_xml_rs::Error> {
        let mut reader = serde_xml_rs::ParserConfig::new()
            .trim_whitespace(true)
            .ignore_comments(true)
            .coalesce_characters(true)
            .create_reader(bytes)
            .into_inner();
        serde_xml_rs::from_reader(&mut reader)
    }
}

// -----------------------------------------------------------------------------
// The acceptor and the error model
// -----------------------------------------------------------------------------

/// An acceptor or error model as given: a ready THFST directory's members,
/// or the bytes of a file, which is DHFST when its header says so and is
/// otherwise a transducer stream to convert to THFST.
pub enum SourceInput {
    Thfst(ThfstMembers),
    Bytes(Vec<u8>),
}

/// An acceptor or error model ready to pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackSource {
    /// Entered as `acceptor.default.thfst/` or `errmodel.default.thfst/`.
    Thfst(ThfstMembers),
    /// A validated DHFST file, entered as it is as `acceptor.default.dhfst`
    /// or `errmodel.default.dhfst`.
    Dhfst(Vec<u8>),
}

/// Whether bytes start the way a DHFST file does.
// [spec:hfst:sem:dhfst.bhfst-member+2]
pub use crate::dhfst_header::has_magic as is_dhfst;

// [spec:hfst:req:embed.tools-in-library]
/// The ids meta.json gives the acceptor and the error model, as each is
/// DHFST or not: their DHFST member names, or `None` to keep the THFST ids.
pub fn dhfst_meta_ids(
    acceptor_is_dhfst: bool,
    errmodel_is_dhfst: bool,
) -> (Option<&'static str>, Option<&'static str>) {
    (
        acceptor_is_dhfst.then_some(DHFST_ACCEPTOR_MEMBER),
        errmodel_is_dhfst.then_some(DHFST_ERRMODEL_MEMBER),
    )
}

/// The THFST members of the first transducer in the stream `bytes`.
// [spec:hfst:sem:thfst-backend.bhfst-tool]
fn thfst_of_stream(origin: &str, bytes: &[u8]) -> Result<ThfstMembers, BhfstError> {
    let mut cursor = std::io::Cursor::new(bytes);
    let mut instream = HfstInputStream::read_from(&mut cursor)
        .map_err(|e| BhfstError::Message(format!("cannot open {origin}: {e}")))?;
    let orig: AnyTransducer = instream
        .read()
        .map_err(|e| BhfstError::Message(format!("cannot read {origin}: {e}")))?;
    let converted = orig
        .convert(ImplementationType::THFST_TYPE, "")
        .map_err(|e| BhfstError::Message(format!("cannot convert {origin}: {e}")))?;
    let AnyTransducer::Thfst(thfst) = converted else {
        return fail!(
            "internal error: THFST conversion did not yield a THFST transducer".to_string(),
        );
    };
    thfst
        .thfst_members()
        .map_err(|e| BhfstError::Message(format!("cannot write THFST: {e}")))
}

/// Check DHFST `bytes` are of the type `wanted` and load as divvunspell loads
/// them.
// [spec:hfst:sem:dhfst.bhfst-member+2]
fn validate_dhfst(origin: &str, bytes: &[u8], wanted: DhfstType) -> Result<(), BhfstError> {
    let prefix = &bytes[..bytes.len().min(crate::dhfst_header::PREFIX_LEN)];
    let kind = crate::dhfst_header::read_type(prefix).map_err(|e| {
        BhfstError::Message(format!("{origin} is not a DHFST file this tool reads: {e}"))
    })?;
    if kind != wanted {
        return fail!(format!(
            "{origin} is a DHFST {kind} (type {}); {} is type {}",
            kind.byte(),
            wanted.with_article(),
            wanted.byte()
        ));
    }
    let loads = match wanted {
        DhfstType::ErrorModel => crate::dhfst::DhfstReader::parse(bytes).map(|_| ()),
        DhfstType::Acceptor => crate::dhfst_acceptor::AcceptorReader::parse(bytes).map(|_| ()),
    };
    loads.map_err(|e| {
        BhfstError::Message(format!(
            "{origin} is not a DHFST {wanted} divvunspell can load: {e}"
        ))
    })
}

// [spec:hfst:sem:dhfst.bhfst-member+2]
// [spec:hfst:sem:thfst-backend.bhfst-tool]
// [spec:hfst:req:embed.tools-in-library]
/// Ready `input`, the source named `origin`, to pack as an acceptor
/// (`wanted` an acceptor) or an error model. A DHFST file must be of the
/// type wanted and load; any other file is converted to THFST.
pub fn resolve_source(
    origin: &str,
    input: SourceInput,
    wanted: DhfstType,
    progress: Progress<'_>,
) -> Result<PackSource, BhfstError> {
    match input {
        SourceInput::Thfst(members) => Ok(PackSource::Thfst(members)),
        SourceInput::Bytes(bytes) if is_dhfst(&bytes) => {
            validate_dhfst(origin, &bytes, wanted)?;
            progress(&format!("Using DHFST {wanted} {origin} as it is\n"));
            Ok(PackSource::Dhfst(bytes))
        }
        SourceInput::Bytes(bytes) => {
            progress(&format!("Reading {origin} and converting to THFST...\n"));
            thfst_of_stream(origin, &bytes).map(PackSource::Thfst)
        }
    }
}

// -----------------------------------------------------------------------------
// meta.json
// -----------------------------------------------------------------------------

/// Where meta.json comes from. The origin names the source in messages.
pub enum MetaSource {
    /// The archive gets no meta.json.
    None,
    /// A zhfst index.xml, converted to meta.json.
    IndexXml { origin: String, bytes: Vec<u8> },
    /// A meta.json embedded as it is.
    Verbatim { origin: String, bytes: Vec<u8> },
}

/// A speller runtime configuration: a JSON object.
pub type SpellerConfig = serde_json::Value;

/// The JSON type name of `value`, for an "expected an object, found ..."
/// message.
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

// [spec:hfst:sem:thfst-backend.speller-config]
// [spec:hfst:req:embed.tools-in-library]
/// The speller runtime configuration in `bytes`, from `origin`: well-formed
/// JSON and an object, so a configuration divvunspell could not load fails
/// here rather than when the speller starts.
pub fn parse_speller_config(origin: &str, bytes: &[u8]) -> Result<SpellerConfig, BhfstError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| {
        BhfstError::Message(format!(
            "{origin} is not a valid speller configuration: {e}"
        ))
    })?;
    if !value.is_object() {
        return fail!(format!(
            "{origin} is not a valid speller configuration: expected a JSON object, found {}",
            json_kind(&value)
        ));
    }
    Ok(value)
}

fn pretty(value: &impl serde::Serialize) -> Result<Vec<u8>, BhfstError> {
    serde_json::to_string_pretty(value)
        .map(String::into_bytes)
        .map_err(|e| BhfstError::Message(format!("cannot serialize meta.json: {e}")))
}

// [spec:hfst:sem:thfst-backend.meta-json]
// [spec:hfst:sem:thfst-backend.speller-config]
// [spec:hfst:sem:dhfst.bhfst-member+2]
// [spec:hfst:req:embed.tools-in-library]
/// The meta.json bytes from `source`, with `config` added under
/// `spellerConfig`; `None` when there is no metadata. An index.xml's
/// acceptor and error model ids become `ids`, or have `.hfst` become
/// `.thfst`. A verbatim meta.json is checked to be JSON and kept byte for
/// byte, so fields divvunspell may still carry survive, unless a
/// configuration has to be added to it.
pub fn build_meta_json(
    source: MetaSource,
    config: Option<SpellerConfig>,
    (acceptor_id, errmodel_id): (Option<&str>, Option<&str>),
) -> Result<Option<Vec<u8>>, BhfstError> {
    match source {
        MetaSource::IndexXml { origin, bytes } => {
            let mut meta = SpellerMetadata::from_xml_bytes(&bytes)
                .map_err(|e| BhfstError::Message(format!("cannot parse {origin}: {e}")))?;
            meta.acceptor.id = match acceptor_id {
                Some(id) => id.to_string(),
                None => meta.acceptor.id.replace(".hfst", ".thfst"),
            };
            meta.errmodel.id = match errmodel_id {
                Some(id) => id.to_string(),
                None => meta.errmodel.id.replace(".hfst", ".thfst"),
            };
            meta.speller_config = config;
            pretty(&meta).map(Some)
        }
        MetaSource::Verbatim { origin, bytes } => {
            let parsed = serde_json::from_slice::<serde_json::Value>(&bytes)
                .map_err(|e| BhfstError::Message(format!("{origin} is not valid JSON: {e}")))?;
            let Some(config) = config else {
                return Ok(Some(bytes));
            };
            let serde_json::Value::Object(mut object) = parsed else {
                return fail!(format!(
                    "cannot add a speller configuration to {origin}: expected a JSON object, found {}",
                    json_kind(&parsed)
                ));
            };
            object.insert(SPELLER_CONFIG_KEY.to_string(), config);
            pretty(&object).map(Some)
        }
        MetaSource::None if config.is_some() => Err(BhfstError::ConfigWithoutMetadata),
        MetaSource::None => Ok(None),
    }
}

// -----------------------------------------------------------------------------
// zhfst input
// -----------------------------------------------------------------------------

/// What a zhfst holds for packing: its two transducer members, by name, and
/// its index.xml and optional speller-config.json.
// [spec:hfst:def:thfst-backend.zhfst-input]
pub struct Zhfst {
    pub acceptor: (String, Vec<u8>),
    pub errmodel: (String, Vec<u8>),
    pub index_xml: Vec<u8>,
    pub speller_config: Option<Vec<u8>>,
}

type ZipArchive<'b> = zip::ZipArchive<std::io::Cursor<&'b [u8]>>;

/// The member `name`, or `None` when the archive has none.
fn zhfst_member(
    archive: &mut ZipArchive<'_>,
    origin: &str,
    name: &str,
) -> Result<Option<Vec<u8>>, BhfstError> {
    let mut entry = match archive.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return fail!(format!("cannot read {origin}: {name}: {e}")),
    };
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| BhfstError::Message(format!("cannot read {origin}: {name}: {e}")))?;
    Ok(Some(bytes))
}

fn required_member(
    archive: &mut ZipArchive<'_>,
    origin: &str,
    name: &str,
) -> Result<(String, Vec<u8>), BhfstError> {
    match zhfst_member(archive, origin, name)? {
        Some(bytes) => Ok((name.to_string(), bytes)),
        None => fail!(format!("{origin} contains no {name}")),
    }
}

/// `id` when the archive has a member of that name, else `fallback`.
fn member_or_default(archive: &mut ZipArchive<'_>, id: &str, fallback: &str) -> String {
    if archive.by_name(id).is_ok() {
        id.to_string()
    } else {
        fallback.to_string()
    }
}

// [spec:hfst:sem:thfst-backend.zhfst-input]
// [spec:hfst:req:embed.tools-in-library]
/// The members of the zhfst `bytes`, from `origin`. index.xml is required:
/// it carries the metadata and the ids naming the two transducers, and an id
/// the archive lacks falls back to `acceptor.default.hfst` or
/// `errmodel.default.hfst`.
pub fn read_zhfst(origin: &str, bytes: &[u8], progress: Progress<'_>) -> Result<Zhfst, BhfstError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| BhfstError::Message(format!("cannot open {origin}: {e}")))?;
    let (_, index_xml) = required_member(&mut archive, origin, INDEX_XML_MEMBER)?;
    let meta = SpellerMetadata::from_xml_bytes(&index_xml).map_err(|e| {
        BhfstError::Message(format!("cannot parse {origin}: {INDEX_XML_MEMBER}: {e}"))
    })?;
    let acceptor = member_or_default(&mut archive, &meta.acceptor.id, DEFAULT_ACCEPTOR_MEMBER);
    let errmodel = member_or_default(&mut archive, &meta.errmodel.id, DEFAULT_ERRMODEL_MEMBER);
    let speller_config = zhfst_member(&mut archive, origin, SPELLER_CONFIG_MEMBER)?;
    progress(&format!("Extracting {acceptor} and {errmodel}...\n"));
    Ok(Zhfst {
        acceptor: required_member(&mut archive, origin, &acceptor)?,
        errmodel: required_member(&mut archive, origin, &errmodel)?,
        index_xml,
        speller_config,
    })
}

// -----------------------------------------------------------------------------
// The archive
// -----------------------------------------------------------------------------

/// A box archive built in memory, laid out as box-format's file writer lays
/// one out: the header, each file's data at the next aligned offset, then
/// the metadata.
struct ArchiveBytes {
    core: ArchiveWriter,
    bytes: Vec<u8>,
}

fn box_path(path: &str) -> Result<BoxPath<'static>, BhfstError> {
    BoxPath::new(path).map_err(|e| BhfstError::Message(format!("invalid box path '{path}': {e}")))
}

impl ArchiveBytes {
    fn new() -> Self {
        let core = ArchiveWriter::with_alignment(ALIGNMENT);
        let bytes = core.encode_header().to_vec();
        Self { core, bytes }
    }

    fn mkdir(&mut self, name: &str) -> Result<(), BhfstError> {
        self.core
            .mkdir(box_path(name)?, BoxHashMap::new())
            .map_err(|e| BhfstError::Message(format!("cannot mkdir '{name}': {e}")))?;
        Ok(())
    }

    fn insert(&mut self, name: &str, data: &[u8]) -> Result<(), BhfstError> {
        let at = self.core.next_write_addr();
        self.bytes.resize(at.get() as usize, 0);
        self.bytes.extend_from_slice(data);
        self.core.advance_position(data.len() as u64);
        let len = data.len() as u64;
        self.core
            .insert_file(
                box_path(name)?,
                Compression::Stored,
                at,
                len,
                len,
                BoxHashMap::new(),
            )
            .map_err(|e| BhfstError::Message(format!("cannot insert '{name}': {e}")))?;
        Ok(())
    }

    fn insert_source(
        &mut self,
        source: &PackSource,
        (dir_name, dhfst_member): (&str, &str),
        progress: Progress<'_>,
    ) -> Result<(), BhfstError> {
        match source {
            PackSource::Thfst(members) => {
                progress(&format!("Inserting {dir_name}...\n"));
                self.mkdir(dir_name)?;
                for (member, data) in [
                    ("alphabet", &members.alphabet),
                    ("index", &members.index),
                    ("transition", &members.transition),
                ] {
                    self.insert(&format!("{dir_name}/{member}"), data)?;
                }
                Ok(())
            }
            PackSource::Dhfst(bytes) => {
                progress(&format!("Inserting {dhfst_member}...\n"));
                self.insert(dhfst_member, bytes)
            }
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, BhfstError> {
        let (trailer, meta) = self
            .core
            .finish()
            .map_err(|e| BhfstError::Message(format!("cannot finalise the archive: {e}")))?;
        self.bytes.resize(trailer as usize, 0);
        self.bytes.extend_from_slice(&meta);
        let header = self.core.encode_header();
        self.bytes[..header.len()].copy_from_slice(&header);
        Ok(self.bytes)
    }
}

// [spec:hfst:def:thfst-backend.bhfst-layout]
// [spec:hfst:req:embed.tools-in-library]
/// The BHFST archive of `acceptor`, `errmodel` and `meta_json`: 8-byte
/// aligned, the acceptor first, then the error model, then meta.json when
/// there is one, every member stored uncompressed.
pub fn pack(
    meta_json: Option<&[u8]>,
    acceptor: &PackSource,
    errmodel: &PackSource,
    progress: Progress<'_>,
) -> Result<Vec<u8>, BhfstError> {
    let mut archive = ArchiveBytes::new();
    archive.insert_source(
        acceptor,
        (ACCEPTOR_DIRNAME, DHFST_ACCEPTOR_MEMBER),
        progress,
    )?;
    archive.insert_source(
        errmodel,
        (ERRMODEL_DIRNAME, DHFST_ERRMODEL_MEMBER),
        progress,
    )?;
    if let Some(bytes) = meta_json {
        progress("Inserting meta.json...\n");
        archive.insert(META_JSON, bytes)?;
    }
    archive.finish()
}

// [spec:hfst:sem:thfst-backend.bhfst-tool]
// [spec:hfst:req:embed.tools-in-library]
/// The meta.json of the BHFST archive `archive`, from `origin`. An archive
/// without one is an error, as for divvunspell's `thfst-tools bhfst-info`.
pub fn meta_json_of(origin: &str, archive: &[u8]) -> Result<Vec<u8>, BhfstError> {
    // The messages are box-format's own for a file it cannot open.
    let Ok(header) = ArchiveReader::parse_header(archive) else {
        return fail!(format!(
            "cannot open {origin}: Could not read header. Is this a valid Box archive?"
        ));
    };
    let Some(trailer) = header
        .trailer
        .map(NonZeroU64::get)
        .and_then(|at| archive.get(at as usize..))
    else {
        return fail!(format!(
            "cannot open {origin}: Could not find trailer (the end of the file is missing)."
        ));
    };
    let Ok(meta) = ArchiveReader::parse_metadata(trailer, header.version) else {
        return fail!(format!(
            "cannot open {origin}: Invalid trailer data (the data that describes where all the files are is invalid)."
        ));
    };
    let reader = ArchiveReader::new(header, meta, 0);
    let Some(record) = reader.find(&box_path(META_JSON)?) else {
        return fail!(format!("{origin} contains no meta.json metadata"));
    };
    let Some(file) = record.as_file() else {
        return fail!(format!("{origin}: meta.json is not a file"));
    };
    let (at, len) = reader.file_location(file);
    let data = archive
        .get(at as usize..(at + len) as usize)
        .map_or_else(|| fail!("cannot read meta.json: cut short".to_string()), Ok)?;
    reader
        .decompress(file, data)
        .map_err(|e| BhfstError::Message(format!("cannot read meta.json: {e}")))
}

// -----------------------------------------------------------------------------
// Packing from files
// -----------------------------------------------------------------------------

/// The three files of a THFST directory.
pub const THFST_MEMBERS: [&str; 3] = ["alphabet", "index", "transition"];

/// The file meta.json comes from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MetaFile {
    /// No meta.json.
    #[default]
    None,
    /// A zhfst index.xml, converted: `--index-xml`.
    IndexXml(String),
    /// A meta.json embedded as it is: `--meta`.
    Meta(String),
}

/// The files `hfst bhfst` packs a speller from, by the names its file
/// resolver knows them by.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpellerFiles {
    /// A DHFST acceptor, a THFST directory, or any transducer stream:
    /// `--acceptor`.
    pub acceptor: String,
    /// The same for the error model: `--errmodel`.
    pub errmodel: String,
    pub meta: MetaFile,
    /// The speller runtime configuration: `--speller-config`.
    pub speller_config: Option<String>,
}

fn read_named(files: &dyn FileResolver, name: &str) -> Result<Vec<u8>, BhfstError> {
    files
        .read(name)
        .map_err(|e| BhfstError::Message(format!("cannot read {name}: {e}")))
}

/// The members of the THFST directory `name`, or `None` when it is not one
/// holding all three.
fn thfst_dir(files: &dyn FileResolver, name: &str) -> Option<ThfstMembers> {
    let [alphabet, index, transition] =
        THFST_MEMBERS.map(|member| files.read(&format!("{name}/{member}")).ok());
    Some(ThfstMembers {
        alphabet: alphabet?,
        index: index?,
        transition: transition?,
    })
}

/// What the file `name` holds as a pack source. A file that does not read
/// fails as the transducer stream it was taken for.
fn source_file(files: &dyn FileResolver, name: &str) -> Result<SourceInput, BhfstError> {
    if let Some(members) = thfst_dir(files, name) {
        return Ok(SourceInput::Thfst(members));
    }
    let read = match files.read(name) {
        Ok(bytes) => Ok(bytes),
        Err(_) => match files.local_path(name).filter(|path| path.is_dir()) {
            Some(_) => HfstInputStream::new_filename(name).map(|_| Vec::new()),
            None => Err(crate::err!(NotTransducerStream, "file could not be opened")),
        },
    };
    read.map(SourceInput::Bytes)
        .map_err(|e| BhfstError::Message(format!("cannot open {name}: {e}")))
}

/// Whether `input` is a DHFST file, which meta.json names by its member.
fn input_is_dhfst(input: &Result<SourceInput, BhfstError>) -> bool {
    matches!(input, Ok(SourceInput::Bytes(bytes)) if is_dhfst(bytes))
}

/// Ready `input`, from the file `name`, to pack as `wanted`.
fn resolve_file(
    name: &str,
    input: Result<SourceInput, BhfstError>,
    wanted: DhfstType,
    progress: Progress<'_>,
) -> Result<PackSource, BhfstError> {
    let input = input?;
    if matches!(input, SourceInput::Thfst(_)) {
        progress(&format!("Using ready THFST directory {name}\n"));
    }
    resolve_source(name, input, wanted, progress)
}

/// The speller runtime configuration to pack: the file `explicit` names
/// when there is one, else `carried`, a zhfst's own, else none.
// [spec:hfst:sem:thfst-backend.speller-config]
fn chosen_config(
    files: &dyn FileResolver,
    explicit: Option<&str>,
    carried: Option<(String, Vec<u8>)>,
    progress: Progress<'_>,
) -> Result<Option<SpellerConfig>, BhfstError> {
    let (origin, bytes) = match (explicit, carried) {
        (Some(name), _) => {
            progress(&format!("Reading speller configuration from {name}...\n"));
            (name.to_string(), read_named(files, name)?)
        }
        (None, Some((origin, bytes))) => {
            progress(&format!(
                "Carrying the speller configuration from {origin}...\n"
            ));
            (origin, bytes)
        }
        (None, None) => return Ok(None),
    };
    parse_speller_config(&origin, &bytes).map(Some)
}

// [spec:hfst:sem:thfst-backend.bhfst-tool]
// [spec:hfst:req:embed.tools-in-library]
// [spec:hfst:req:embed.writers]
/// The BHFST archive `hfst bhfst` packs from `speller`, each file read
/// through `files`.
pub fn pack_files(
    speller: &SpellerFiles,
    files: &dyn FileResolver,
    progress: Progress<'_>,
) -> Result<Vec<u8>, BhfstError> {
    let source = match &speller.meta {
        MetaFile::None => MetaSource::None,
        MetaFile::IndexXml(name) => {
            progress(&format!("Converting {name} (index.xml) to meta.json...\n"));
            MetaSource::IndexXml {
                origin: name.clone(),
                bytes: read_named(files, name)?,
            }
        }
        MetaFile::Meta(name) => MetaSource::Verbatim {
            origin: name.clone(),
            bytes: read_named(files, name)?,
        },
    };
    let config = chosen_config(files, speller.speller_config.as_deref(), None, progress)?;
    let acceptor = source_file(files, &speller.acceptor);
    let errmodel = source_file(files, &speller.errmodel);
    let ids = dhfst_meta_ids(input_is_dhfst(&acceptor), input_is_dhfst(&errmodel));
    let meta_json = build_meta_json(source, config, ids)?;
    let acceptor = resolve_file(&speller.acceptor, acceptor, DhfstType::Acceptor, progress)?;
    let errmodel = resolve_file(&speller.errmodel, errmodel, DhfstType::ErrorModel, progress)?;
    pack(meta_json.as_deref(), &acceptor, &errmodel, progress)
}

// [spec:hfst:sem:thfst-backend.zhfst-input]
// [spec:hfst:req:embed.tools-in-library]
// [spec:hfst:req:embed.writers]
/// The BHFST archive `hfst bhfst --zhfst` makes of the zhfst `zhfst`, with
/// the speller configuration in the file `speller_config` in place of the
/// zhfst's own, each file read through `files`.
pub fn pack_zhfst(
    zhfst: &str,
    speller_config: Option<&str>,
    files: &dyn FileResolver,
    progress: Progress<'_>,
) -> Result<Vec<u8>, BhfstError> {
    progress(&format!("Opening {zhfst} (zhfst)...\n"));
    let bytes = files
        .read(zhfst)
        .map_err(|e| BhfstError::Message(format!("cannot open {zhfst}: {e}")))?;
    let archive = read_zhfst(zhfst, &bytes, progress)?;
    let carried = archive
        .speller_config
        .map(|bytes| (format!("{zhfst}: {SPELLER_CONFIG_MEMBER}"), bytes));
    let config = chosen_config(files, speller_config, carried, progress)?;
    let ids = dhfst_meta_ids(is_dhfst(&archive.acceptor.1), is_dhfst(&archive.errmodel.1));
    let index_xml = MetaSource::IndexXml {
        origin: format!("{zhfst}: {INDEX_XML_MEMBER}"),
        bytes: archive.index_xml,
    };
    let meta_json = build_meta_json(index_xml, config, ids)?;
    let (acceptor_name, acceptor_bytes) = archive.acceptor;
    let (errmodel_name, errmodel_bytes) = archive.errmodel;
    let acceptor = resolve_source(
        &acceptor_name,
        SourceInput::Bytes(acceptor_bytes),
        DhfstType::Acceptor,
        progress,
    )?;
    let errmodel = resolve_source(
        &errmodel_name,
        SourceInput::Bytes(errmodel_bytes),
        DhfstType::ErrorModel,
        progress,
    )?;
    pack(meta_json.as_deref(), &acceptor, &errmodel, progress)
}
