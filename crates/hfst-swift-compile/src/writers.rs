//! The writers: a transducer as `hfst fst2fst` writes it in each type and as
//! DHFST and THFST, as the text `hfst fst2txt` writes, and spellers packed as
//! `hfst bhfst` packs them.

use hfst_embed::compile::{
    self as embed, DhfstKind, DhfstOptions, FsResolver, MemoryFiles, MetaFile, SpellerFiles,
    TextFormat, TextOptions, WriteOptions,
};
use hfst_embed::{Code, Error};
use hfst_swift_runtime::GlueError;
use hfst_swift_runtime::error::invalid;

use crate::transducer::{Transducer, format_named};

fn text(bytes: Vec<u8>) -> Result<String, Error> {
    String::from_utf8(bytes).map_err(|_| Error::new(Code::Internal, "the text is not UTF-8"))
}

// [spec:hfst:req:swift.writers]
// [spec:hfst:req:swift.threads]
#[uniffi::export]
impl Transducer {
    /// The bytes of a stream holding the transducer, in the type `format`
    /// names, or in its own type for `None`.
    pub fn to_bytes(
        &self,
        format: Option<String>,
        use_backend_format: bool,
        quick: bool,
    ) -> Result<Vec<u8>, GlueError> {
        let options = WriteOptions {
            format: format.as_deref().map(format_named).transpose()?,
            header: !use_backend_format,
            quick,
        };
        self.read(|t| t.to_bytes(&options))
    }

    /// The text `hfst fst2txt` writes in the format `format` names: `att`,
    /// `prolog`, `dot` or `pckimmo`.
    pub fn to_text(
        &self,
        format: String,
        print_weights: bool,
        do_not_print_weights: bool,
        use_numbers: bool,
    ) -> Result<String, GlueError> {
        let format = match format.as_str() {
            "att" => TextFormat::Att,
            "prolog" => TextFormat::Prolog,
            "dot" => TextFormat::Dot,
            "pckimmo" => TextFormat::Pckimmo,
            other => return Err(invalid(format!("there is no text format called {other}"))),
        };
        let options = TextOptions {
            format,
            print_weights,
            do_not_print_weights,
            use_numbers,
        };
        self.read(|t| text(t.to_text(&options)?))
    }

    /// What `hfst fst2fst -f dhfst --dhfst-type acceptor` writes: a type 2
    /// file, for divvunspell to check words against.
    pub fn to_dhfst_acceptor(
        &self,
        quick: bool,
        source_name: String,
    ) -> Result<Vec<u8>, GlueError> {
        let options = DhfstOptions {
            quick,
            source_name,
            ..DhfstOptions::new(DhfstKind::Acceptor)
        };
        self.read(|t| t.to_dhfst(&options))
    }

    /// The DHFST error model `hfst fst2fst -f dhfst --dhfst-type errmodel`
    /// writes, its fallback chains at most `max_fallback_depth` long, or
    /// unbounded for `None`.
    pub fn to_dhfst_error_model(
        &self,
        max_fallback_depth: Option<u32>,
        quick: bool,
        source_name: String,
    ) -> Result<Vec<u8>, GlueError> {
        let options = DhfstOptions {
            kind: DhfstKind::ErrorModel,
            max_fallback_depth,
            quick,
            source_name,
        };
        self.read(|t| t.to_dhfst(&options))
    }

    /// Writes the THFST directory at `path`, as `hfst fst2fst -f thfst -o
    /// PATH` writes it.
    pub fn write_thfst(&self, path: String, quick: bool) -> Result<(), GlueError> {
        self.read(|t| t.write_thfst(&path, quick, &FsResolver))
    }
}

// [spec:hfst:req:swift.writers]
/// The BHFST archive `hfst bhfst` packs of a speller: `acceptor` and
/// `error_model` each the bytes of a DHFST file or of a transducer stream,
/// and `metadata` the text of its meta.json, or with `index_xml` of a zhfst
/// index.xml.
#[uniffi::export]
pub fn pack_bhfst(
    acceptor: Vec<u8>,
    error_model: Vec<u8>,
    metadata: Option<Vec<u8>>,
    index_xml: bool,
    speller_config: Option<Vec<u8>>,
) -> Result<Vec<u8>, GlueError> {
    let files = MemoryFiles::default();
    files.insert("acceptor", acceptor);
    files.insert("errmodel", error_model);
    let meta = match metadata {
        None => MetaFile::None,
        Some(metadata) => {
            let name = if index_xml { "index.xml" } else { "meta.json" };
            files.insert(name, metadata);
            if index_xml {
                MetaFile::IndexXml(name.to_owned())
            } else {
                MetaFile::Meta(name.to_owned())
            }
        }
    };
    let speller_config = speller_config.map(|config| {
        files.insert("speller-config.json", config);
        "speller-config.json".to_owned()
    });
    let speller = SpellerFiles {
        acceptor: "acceptor".to_owned(),
        errmodel: "errmodel".to_owned(),
        meta,
        speller_config,
    };
    Ok(embed::pack_bhfst(&speller, &files)?)
}
