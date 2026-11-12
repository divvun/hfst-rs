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
use hfst::file_resolver::FsResolver;
use hfst::tools::bhfst::{
    BhfstError, MetaFile, SpellerFiles, meta_json_of, pack_files, pack_zhfst,
};
use std::io::Write;

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

/// Write the archive `packed` to the output file, or report why there is
/// none.
fn write_archive(common: &CommonOptions, packed: Result<Vec<u8>, BhfstError>) -> i32 {
    let archive = match packed {
        Ok(archive) => archive,
        Err(e) => return report(common, e),
    };
    let output = &common.output_filename;
    if let Err(e) = std::fs::write(output, archive) {
        error(common, 1, 0, &format!("cannot create {output}: {e}"));
        return 1;
    }
    verbose_print(common, &format!("Wrote {output}\n"));
    0
}

// [spec:hfst:sem:thfst-backend.zhfst-input]
fn convert_zhfst(common: &CommonOptions, options: &Options, zhfst_path: &str) -> i32 {
    let packed = pack_zhfst(
        zhfst_path,
        options.speller_config.as_deref(),
        &FsResolver,
        &mut |m| verbose_print(common, m),
    );
    write_archive(common, packed)
}

// [spec:hfst:sem:thfst-backend.bhfst-tool]
fn pack_speller(
    common: &CommonOptions,
    options: &Options,
    (acceptor, errmodel): (&str, &str),
) -> i32 {
    let meta = match (&options.index_xml, &options.meta) {
        (Some(xml), _) => MetaFile::IndexXml(xml.clone()),
        (None, Some(meta)) => MetaFile::Meta(meta.clone()),
        (None, None) => MetaFile::None,
    };
    let speller = SpellerFiles {
        acceptor: acceptor.to_string(),
        errmodel: errmodel.to_string(),
        meta,
        speller_config: options.speller_config.clone(),
    };
    let packed = pack_files(&speller, &FsResolver, &mut |m| verbose_print(common, m));
    write_archive(common, packed)
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

    cli::from_code(pack_speller(
        &common,
        &options,
        (acceptor_path, errmodel_path),
    ))
}

#[cfg(test)]
mod tests {
    use super::{Options, convert_zhfst, pack_speller};
    use crate::globals::CommonOptions;
    use std::io::Write;
    use std::path::Path;

    const INDEX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hfstspeller dtdversion="1.0" hfstversion="3">
  <info><locale>se</locale><title>T</title><description>D</description><producer>p</producer></info>
  <acceptor type="general" id="acceptor.default.hfst"><title>A</title><description>a</description></acceptor>
  <errmodel id="errmodel.default.hfst"><title>E</title><description>e</description></errmodel>
</hfstspeller>
"#;

    fn fixture(name: &str) -> String {
        format!("{}/tests/fixtures/dhfst/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    fn writing_to(output: &Path) -> CommonOptions {
        let mut common =
            crate::hfst_commandline::hfst_set_program_name("hfst-bhfst", "0.1", "HfstBhfst");
        common.output_filename = output.to_string_lossy().into_owned();
        common.output_named = true;
        common
    }

    // [spec:hfst:sem:thfst-backend.bhfst-tool/test]
    // The archive the library packs from the named files is written to the
    // output file.
    #[test]
    fn loose_files_pack_into_the_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let index_xml = dir.path().join("index.xml");
        std::fs::write(&index_xml, INDEX_XML).expect("write index.xml");
        let out = dir.path().join("s.bhfst");
        let options = Options {
            index_xml: Some(index_xml.to_string_lossy().into_owned()),
            ..Options::default()
        };
        let (acceptor, errmodel) = (fixture("acceptor.hfst"), fixture("errmodel.dhfst"));
        let code = pack_speller(&writing_to(&out), &options, (&acceptor, &errmodel));
        assert_eq!(code, 0);
        let archive = std::fs::read(&out).expect("the archive");
        let meta = hfst::tools::bhfst::meta_json_of("s.bhfst", &archive).expect("meta.json");
        let meta: serde_json::Value = serde_json::from_slice(&meta).expect("JSON");
        assert_eq!(meta["errmodel"]["id"], "errmodel.default.dhfst");
        assert_eq!(meta["acceptor"]["id"], "acceptor.default.thfst");
    }

    // [spec:hfst:sem:thfst-backend.zhfst-input/test]
    // A zhfst is packed whole, its speller configuration carried.
    #[test]
    fn a_zhfst_packs_into_the_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zhfst = dir.path().join("s.zhfst");
        let acceptor = std::fs::read(fixture("acceptor.hfst")).expect("the acceptor");
        let errmodel = std::fs::read(fixture("errmodel.hfst")).expect("the error model");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&zhfst).expect("create"));
        for (name, bytes) in [
            ("index.xml", INDEX_XML.as_bytes()),
            ("acceptor.default.hfst", &acceptor),
            ("errmodel.default.hfst", &errmodel),
            ("speller-config.json", br#"{"beam": 2}"#),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .expect("a member");
            zip.write_all(bytes).expect("written");
        }
        zip.finish().expect("a zip");
        let out = dir.path().join("s.bhfst");
        let code = convert_zhfst(
            &writing_to(&out),
            &Options::default(),
            &zhfst.to_string_lossy(),
        );
        assert_eq!(code, 0);
        let archive = std::fs::read(&out).expect("the archive");
        let meta = hfst::tools::bhfst::meta_json_of("s.bhfst", &archive).expect("meta.json");
        let meta: serde_json::Value = serde_json::from_slice(&meta).expect("JSON");
        assert_eq!(meta["spellerConfig"]["beam"], 2);
    }
}
