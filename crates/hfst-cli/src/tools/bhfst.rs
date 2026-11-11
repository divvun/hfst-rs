//! `hfst-bhfst` — pack an acceptor and an error model (each THFST, or DHFST
//! stored as it is) plus speller metadata into a BHFST speller archive,
//! convert a whole zhfst into one, or inspect an existing one. There is NO
//! C++ HFST ancestor; the packing is the library's [`hfst::tools::bhfst`],
//! and this tool reads the files it names and writes the archive.
//!
//! Option handling is clap 4 derive through [`crate::cli`]. Unlike the
//! algebra/lookup tools it has NO default input/output streams: pack mode
//! requires `-a`, `-e`, `-o` (or `-z`, `-o`); info mode requires only `-I`. It
//! therefore never runs the check-params fragments (no standard in/out stream
//! to resolve).

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
use hfst::dhfst_header::DhfstType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::thfst_io::ThfstMembers;
use hfst::tools::bhfst::{
    BhfstError, INDEX_XML_MEMBER, MetaSource, PackSource, SPELLER_CONFIG_MEMBER, SourceInput,
    SpellerConfig, build_meta_json, dhfst_meta_ids, is_dhfst, meta_json_of, pack,
    parse_speller_config, read_zhfst, resolve_source,
};
use std::io::Write;
use std::path::Path;

/// The three THFST member files.
const THFST_MEMBERS: [&str; 3] = ["alphabet", "index", "transition"];

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

/// Print `e`, the library's failure, and give the exit code.
fn report(common: &CommonOptions, e: BhfstError) -> i32 {
    let message = match e {
        BhfstError::ConfigWithoutMetadata => {
            format!("{e};\npass --index-xml, --meta or --zhfst as well")
        }
        BhfstError::Message(message) => message,
    };
    error(common, 1, 0, &message);
    1
}

/// Read a whole file, reporting failure through the common error path.
fn read_file(common: &CommonOptions, path: &str) -> Result<Vec<u8>, i32> {
    std::fs::read(path).map_err(|e| {
        error(common, 1, 0, &format!("cannot read {path}: {e}"));
        1
    })
}

/// True if `dir` is a directory holding all three THFST member files.
fn is_thfst_dir(dir: &Path) -> bool {
    dir.is_dir() && THFST_MEMBERS.iter().all(|m| dir.join(m).is_file())
}

/// What `path` holds as a pack source: a ready THFST directory's members,
/// or the file's bytes. A file that cannot be read fails as the transducer
/// stream it was taken for.
fn source_input(common: &CommonOptions, path_str: &str) -> Result<SourceInput, i32> {
    let path = Path::new(path_str);
    if is_thfst_dir(path) {
        verbose_print(common, &format!("Using ready THFST directory {path_str}\n"));
        let [alphabet, index, transition] =
            THFST_MEMBERS.map(|m| std::fs::read(path.join(m)).unwrap_or_default());
        return Ok(SourceInput::Thfst(ThfstMembers {
            alphabet,
            index,
            transition,
        }));
    }
    let read = if path.is_dir() {
        HfstInputStream::new_filename(path_str).map(|_| Vec::new())
    } else {
        std::fs::read(path).map_err(|_| hfst::err!(NotTransducerStream, "file could not be opened"))
    };
    read.map(SourceInput::Bytes).map_err(|e| {
        error(common, 1, 0, &format!("cannot open {path_str}: {e}"));
        1
    })
}

/// Ready the source `input`, named `origin`, as an acceptor or error model,
/// reporting a failure.
fn resolve(
    common: &CommonOptions,
    origin: &str,
    input: SourceInput,
    wanted: DhfstType,
) -> Result<PackSource, i32> {
    match resolve_source(origin, input, wanted, &mut |m| verbose_print(common, m)) {
        Ok(source) => Ok(source),
        Err(e) => Err(report(common, e)),
    }
}

/// Resolve `-X`/`-m` to a metadata source, reading the named file.
fn meta_source(common: &CommonOptions, options: &Options) -> Result<MetaSource, i32> {
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

/// The speller runtime configuration for this run: an explicit `-c` file
/// wins, otherwise the zhfst's `speller-config.json` member when `-z`
/// carried one, otherwise none.
// [spec:hfst:sem:thfst-backend.speller-config]
fn speller_config(
    common: &CommonOptions,
    options: &Options,
    from_zhfst: Option<(String, Vec<u8>)>,
) -> Result<Option<SpellerConfig>, i32> {
    let (origin, bytes) = if let Some(path) = &options.speller_config {
        verbose_print(
            common,
            &format!("Reading speller configuration from {path}...\n"),
        );
        (path.clone(), read_file(common, path)?)
    } else if let Some((origin, bytes)) = from_zhfst {
        verbose_print(
            common,
            &format!("Carrying the speller configuration from {origin}...\n"),
        );
        (origin, bytes)
    } else {
        return Ok(None);
    };
    parse_speller_config(&origin, &bytes)
        .map(Some)
        .map_err(|e| report(common, e))
}

/// Pack the archive and write it to `output`.
fn pack_into(
    common: &CommonOptions,
    meta_json: Option<Vec<u8>>,
    (acceptor, errmodel): (&PackSource, &PackSource),
    output: &str,
) -> i32 {
    let packed = pack(meta_json.as_deref(), acceptor, errmodel, &mut |m| {
        verbose_print(common, m)
    });
    let archive = match packed {
        Ok(archive) => archive,
        Err(e) => return report(common, e),
    };
    if let Err(e) = std::fs::write(output, archive) {
        error(common, 1, 0, &format!("cannot create {output}: {e}"));
        return 1;
    }
    verbose_print(common, &format!("Wrote {output}\n"));
    0
}

// [spec:hfst:sem:thfst-backend.zhfst-input]
fn convert_zhfst(common: &CommonOptions, options: &Options, zhfst_path: &str) -> i32 {
    let output = common.output_filename.clone();
    verbose_print(common, &format!("Opening {zhfst_path} (zhfst)...\n"));
    let Ok(bytes) = std::fs::read(zhfst_path).map_err(|e| {
        error(common, 1, 0, &format!("cannot open {zhfst_path}: {e}"));
    }) else {
        return 1;
    };
    let zhfst = match read_zhfst(zhfst_path, &bytes, &mut |m| verbose_print(common, m)) {
        Ok(zhfst) => zhfst,
        Err(e) => return report(common, e),
    };
    let config_member = zhfst
        .speller_config
        .clone()
        .map(|bytes| (format!("{zhfst_path}: {SPELLER_CONFIG_MEMBER}"), bytes));
    let Ok(config) = speller_config(common, options, config_member) else {
        return 1;
    };
    let ids = dhfst_meta_ids(is_dhfst(&zhfst.acceptor.1), is_dhfst(&zhfst.errmodel.1));
    let index_xml = MetaSource::IndexXml {
        origin: format!("{zhfst_path}: {INDEX_XML_MEMBER}"),
        bytes: zhfst.index_xml,
    };
    let meta_json = match build_meta_json(index_xml, config, ids) {
        Ok(meta_json) => meta_json,
        Err(e) => return report(common, e),
    };
    let (acceptor_name, acceptor_bytes) = zhfst.acceptor;
    let (errmodel_name, errmodel_bytes) = zhfst.errmodel;
    let sources = resolve(
        common,
        &acceptor_name,
        SourceInput::Bytes(acceptor_bytes),
        DhfstType::Acceptor,
    )
    .and_then(|acceptor| {
        let errmodel = resolve(
            common,
            &errmodel_name,
            SourceInput::Bytes(errmodel_bytes),
            DhfstType::ErrorModel,
        )?;
        Ok((acceptor, errmodel))
    });
    match sources {
        Ok((acceptor, errmodel)) => pack_into(common, meta_json, (&acceptor, &errmodel), &output),
        Err(code) => code,
    }
}

fn file_is_dhfst(path: &str) -> bool {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| std::io::Read::read_to_end(&mut std::io::Read::take(f, 5), &mut head))
        .is_ok_and(|_| is_dhfst(&head))
}

// [spec:hfst:sem:thfst-backend.bhfst-tool]
fn pack_files(
    common: &CommonOptions,
    options: &Options,
    (acceptor, errmodel): (&str, &str),
) -> i32 {
    let output = common.output_filename.clone();
    let meta_and_config = meta_source(common, options)
        .and_then(|source| Ok((source, speller_config(common, options, None)?)));
    let Ok((source, config)) = meta_and_config else {
        return 1;
    };
    let ids = dhfst_meta_ids(file_is_dhfst(acceptor), file_is_dhfst(errmodel));
    let meta_json = match build_meta_json(source, config, ids) {
        Ok(meta_json) => meta_json,
        Err(e) => return report(common, e),
    };
    let sources = source_input(common, acceptor)
        .and_then(|input| resolve(common, acceptor, input, DhfstType::Acceptor))
        .and_then(|a| {
            let input = source_input(common, errmodel)?;
            Ok((a, resolve(common, errmodel, input, DhfstType::ErrorModel)?))
        });
    match sources {
        Ok((a, e)) => pack_into(common, meta_json, (&a, &e), &output),
        Err(code) => code,
    }
}

// [spec:hfst:sem:thfst-backend.bhfst-tool]
/// Print the meta.json of the archive `path`, ending with a newline.
fn info(common: &CommonOptions, path: &str) -> i32 {
    // The messages are box-format's own for a path it cannot open.
    let archive = match std::fs::canonicalize(path) {
        Err(_) => Err(format!("Invalid path to Box file. Path: '{path}'")),
        Ok(canonical) => std::fs::read(&canonical)
            .map_err(|_| format!("Failed to read Box file. Path: '{}'", canonical.display())),
    };
    let archive = match archive {
        Ok(archive) => archive,
        Err(e) => {
            error(common, 1, 0, &format!("cannot open {path}: {e}"));
            return 1;
        }
    };
    let bytes = match meta_json_of(path, &archive) {
        Ok(bytes) => bytes,
        Err(e) => return report(common, e),
    };
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
        return cli::from_code(convert_zhfst(&common, &options, zhfst_path));
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
    if options.index_xml.is_some() && options.meta.is_some() {
        error(
            &common,
            1,
            0,
            "--index-xml and --meta are mutually exclusive",
        );
        return Err(1);
    }

    cli::from_code(pack_files(
        &common,
        &options,
        (acceptor_path, errmodel_path),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        Options, SourceInput, file_is_dhfst, is_thfst_dir, meta_source, source_input,
        speller_config,
    };
    use crate::globals::CommonOptions;
    use hfst::tools::bhfst::MetaSource;

    fn common() -> CommonOptions {
        crate::hfst_commandline::hfst_set_program_name("hfst-bhfst", "0.1", "HfstBhfst")
    }

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
        assert!(file_is_dhfst(&dhfst.to_string_lossy()));
        for not_dhfst in [&hfst, &short, &dir.path().join("missing")] {
            assert!(!file_is_dhfst(&not_dhfst.to_string_lossy()));
        }
    }

    // [spec:hfst:sem:thfst-backend.bhfst-tool/test]
    #[test]
    fn ready_thfst_directories_are_read_whole() {
        let dir = tempfile::tempdir().expect("tempdir");
        let thfst = dir.path().join("a.thfst");
        std::fs::create_dir(&thfst).expect("mkdir");
        assert!(!is_thfst_dir(&thfst));
        for (member, bytes) in [("alphabet", "{}"), ("index", "ix"), ("transition", "tr")] {
            std::fs::write(thfst.join(member), bytes).expect("write a member");
        }
        assert!(is_thfst_dir(&thfst));
        let Ok(SourceInput::Thfst(members)) = source_input(&common(), &thfst.to_string_lossy())
        else {
            panic!("a ready THFST directory was not read as one");
        };
        assert_eq!(
            (members.alphabet, members.index, members.transition),
            (b"{}".to_vec(), b"ix".to_vec(), b"tr".to_vec())
        );
        let file = dir.path().join("t.hfst");
        std::fs::write(&file, b"bytes").expect("write a file");
        let read = source_input(&common(), &file.to_string_lossy());
        assert!(matches!(read, Ok(SourceInput::Bytes(bytes)) if bytes == b"bytes"));
    }

    // [spec:hfst:sem:thfst-backend.speller-config/test]
    #[test]
    fn explicit_speller_config_wins() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("c.json");
        std::fs::write(&config, br#"{"beam": 1}"#).expect("write config");
        let options = Options {
            speller_config: Some(config.to_string_lossy().into_owned()),
            ..Options::default()
        };
        let zhfst = Some((
            "z: speller-config.json".to_string(),
            br#"{"beam": 2}"#.to_vec(),
        ));
        let chosen = speller_config(&common(), &options, zhfst.clone()).expect("a config");
        assert_eq!(chosen.map(|c| c["beam"].clone()), Some(1.into()));
        let carried = speller_config(&common(), &Options::default(), zhfst).expect("a config");
        assert_eq!(carried.map(|c| c["beam"].clone()), Some(2.into()));
        let source = meta_source(&common(), &Options::default()).expect("a source");
        assert!(matches!(source, MetaSource::None));
    }
}
