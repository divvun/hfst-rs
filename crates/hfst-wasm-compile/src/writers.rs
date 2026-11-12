//! The writers: a transducer as `hfst fst2fst` writes it in each type and as
//! DHFST, as the text `hfst fst2txt` writes, and spellers packed as
//! `hfst bhfst` packs them. THFST, a directory, is not written.

use hfst_embed::Code;
use hfst_embed::compile::{
    self as embed, DEFAULT_FALLBACK_DEPTH, DhfstKind, DhfstOptions, MemoryFiles, MetaFile,
    SpellerFiles, TextFormat, TextOptions, WriteOptions,
};
use hfst_wasm_shared::GlueError;
use wasm_bindgen::prelude::*;

use crate::options::{Options, Value, format_named};
use crate::transducer::Transducer;

/// The text `fst2txt` writes, which is UTF-8 since every symbol is.
fn text(bytes: Vec<u8>) -> Result<String, GlueError> {
    String::from_utf8(bytes).map_err(|_| GlueError::new(Code::Internal, "the text is not UTF-8"))
}

/// The options of a text writer: `toAtt`'s, with `useNumbers`, or another's.
fn text_options(format: TextFormat, o: &Options<'_>) -> Result<TextOptions, GlueError> {
    match format {
        TextFormat::Att => o.only(&["printWeights", "doNotPrintWeights", "useNumbers"])?,
        TextFormat::Prolog | TextFormat::Dot | TextFormat::Pckimmo => {
            o.only(&["printWeights", "doNotPrintWeights"])?
        }
    }
    Ok(TextOptions {
        format,
        print_weights: o.flag("printWeights", false)?,
        do_not_print_weights: o.flag("doNotPrintWeights", false)?,
        use_numbers: o.flag("useNumbers", false)?,
    })
}

/// The options of a DHFST writer. An error model's `maxFallbackDepth` left
/// out is the tool's default, and `null` is `--unbounded-fallback`; an
/// acceptor has no fallbacks.
fn dhfst_options(kind: DhfstKind, o: &Options<'_>) -> Result<DhfstOptions, GlueError> {
    let max_fallback_depth = match kind {
        DhfstKind::Acceptor => {
            o.only(&["quick", "sourceName"])?;
            Some(DEFAULT_FALLBACK_DEPTH)
        }
        DhfstKind::ErrorModel => {
            o.only(&["maxFallbackDepth", "quick", "sourceName"])?;
            match o.get("maxFallbackDepth") {
                Some(Value::Null) => None,
                Some(_) | None => Some(
                    o.count("maxFallbackDepth", 0)?
                        .map_or(DEFAULT_FALLBACK_DEPTH, |n| {
                            u32::try_from(n).unwrap_or(u32::MAX)
                        }),
                ),
            }
        }
    };
    Ok(DhfstOptions {
        kind,
        max_fallback_depth,
        quick: o.flag("quick", false)?,
        source_name: o.text("sourceName")?.unwrap_or_default(),
    })
}

/// `toBytes`' options for the type named `format`, none for its own.
fn write_options(format: Option<&str>, o: &Options<'_>) -> Result<WriteOptions, GlueError> {
    o.only(&["useBackendFormat", "quick"])?;
    Ok(WriteOptions {
        format: format
            .map(|name| format_named("toBytes", name))
            .transpose()?,
        header: !o.flag("useBackendFormat", false)?,
        quick: o.flag("quick", false)?,
    })
}

// [spec:hfst:req:javascript.compile-module]
#[wasm_bindgen]
impl Transducer {
    /// The bytes of a stream holding the transducer, as `hfst fst2fst`
    /// writes it: in the type named `format`, converted and named as
    /// `--format` converts and names it, or as it is when there is none.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(
        &self,
        format: Option<String>,
        options: &JsValue,
    ) -> Result<Vec<u8>, GlueError> {
        let options = write_options(format.as_deref(), &Options::new("toBytes", options)?)?;
        Ok(self.reading()?.to_bytes(&options)?)
    }

    /// The AT&T text `hfst fst2txt` writes of the transducer.
    #[wasm_bindgen(js_name = toAtt)]
    pub fn to_att(&self, options: &JsValue) -> Result<String, GlueError> {
        let options = text_options(TextFormat::Att, &Options::new("toAtt", options)?)?;
        text(self.reading()?.to_text(&options)?)
    }

    /// The Prolog text `hfst fst2txt -f prolog` writes of the transducer
    /// alone in its stream.
    #[wasm_bindgen(js_name = toProlog)]
    pub fn to_prolog(&self, options: &JsValue) -> Result<String, GlueError> {
        let options = text_options(TextFormat::Prolog, &Options::new("toProlog", options)?)?;
        text(self.reading()?.to_text(&options)?)
    }

    /// The Graphviz text `hfst fst2txt -f dot` writes of the transducer.
    #[wasm_bindgen(js_name = toDot)]
    pub fn to_dot(&self, options: &JsValue) -> Result<String, GlueError> {
        let options = text_options(TextFormat::Dot, &Options::new("toDot", options)?)?;
        text(self.reading()?.to_text(&options)?)
    }

    /// The PC-KIMMO text `hfst fst2txt -f pckimmo` writes of the transducer.
    #[wasm_bindgen(js_name = toPckimmo)]
    pub fn to_pckimmo(&self, options: &JsValue) -> Result<String, GlueError> {
        let options = text_options(TextFormat::Pckimmo, &Options::new("toPckimmo", options)?)?;
        text(self.reading()?.to_text(&options)?)
    }

    /// The DHFST acceptor `hfst fst2fst -f dhfst --dhfst-type acceptor`
    /// writes of the transducer.
    #[wasm_bindgen(js_name = toDhfstAcceptor)]
    pub fn to_dhfst_acceptor(&self, options: &JsValue) -> Result<Vec<u8>, GlueError> {
        let o = Options::new("toDhfstAcceptor", options)?;
        let options = dhfst_options(DhfstKind::Acceptor, &o)?;
        Ok(self.reading()?.to_dhfst(&options)?)
    }

    /// The DHFST error model `hfst fst2fst -f dhfst --dhfst-type errmodel`
    /// writes of the transducer.
    #[wasm_bindgen(js_name = toDhfstErrorModel)]
    pub fn to_dhfst_error_model(&self, options: &JsValue) -> Result<Vec<u8>, GlueError> {
        let o = Options::new("toDhfstErrorModel", options)?;
        let options = dhfst_options(DhfstKind::ErrorModel, &o)?;
        Ok(self.reading()?.to_dhfst(&options)?)
    }
}

/// The BHFST archive `hfst bhfst` packs of the speller `acceptor` and
/// `error_model`, with `metadata` as its meta.json, or with `indexXml` as a
/// zhfst index.xml to convert.
fn pack(
    acceptor: &[u8],
    error_model: &[u8],
    metadata: Option<Vec<u8>>,
    o: &Options<'_>,
) -> Result<Vec<u8>, GlueError> {
    o.only(&["indexXml", "spellerConfig"])?;
    let index_xml = o.flag("indexXml", false)?;
    let files = MemoryFiles::default();
    files.insert("acceptor", acceptor.to_vec());
    files.insert("errmodel", error_model.to_vec());
    let meta = match metadata {
        None => MetaFile::None,
        Some(bytes) if index_xml => {
            files.insert("index.xml", bytes);
            MetaFile::IndexXml("index.xml".to_owned())
        }
        Some(bytes) => {
            files.insert("meta.json", bytes);
            MetaFile::Meta("meta.json".to_owned())
        }
    };
    let speller_config = o.contents("spellerConfig")?.map(|bytes| {
        files.insert("speller-config.json", bytes);
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

// [spec:hfst:req:javascript.compile-module]
/// The BHFST archive `hfst bhfst` packs of a speller: `acceptor` and
/// `error_model` each the bytes of a DHFST file or of a transducer stream,
/// and `metadata`, the text or bytes of the archive's meta.json, or with
/// `indexXml` of a zhfst index.xml to convert.
#[wasm_bindgen(js_name = packBhfst)]
pub fn pack_bhfst(
    acceptor: &[u8],
    error_model: &[u8],
    metadata: &JsValue,
    options: &JsValue,
) -> Result<Vec<u8>, GlueError> {
    let metadata = match Value::from_js(metadata) {
        None | Some(Value::Null) => None,
        Some(Value::Text(text)) => Some(text.into_bytes()),
        Some(Value::Bytes(bytes)) => Some(bytes),
        Some(_) => {
            return Err(GlueError::new(
                Code::InvalidArgument,
                "packBhfst takes the metadata as a string or a Uint8Array",
            ));
        }
    };
    pack(
        acceptor,
        error_model,
        metadata,
        &Options::new("packBhfst", options)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::given;
    use crate::tests::machine;

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn each_writer_takes_its_tools_options() {
        let o = given("toAtt", &[("useNumbers", true.into())]);
        assert!(text_options(TextFormat::Att, &o).expect("att").use_numbers);
        assert!(
            text_options(TextFormat::Dot, &o).is_err(),
            "only AT&T writes numbers"
        );
        let o = given("toBytes", &[("useBackendFormat", true.into())]);
        let write = write_options(Some("foma"), &o).expect("foma");
        assert!(!write.header);
        assert_eq!(write.format, Some(hfst_embed::format::Format::Foma));
        assert!(
            write_options(Some("thfst"), &o).is_err(),
            "THFST is not written"
        );
        assert_eq!(
            write_options(None, &given("toBytes", &[]))
                .expect("own")
                .format,
            None
        );
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn fallback_depth_defaults_or_is_unbounded() {
        let depth = |fields: &[(&str, Value)]| {
            dhfst_options(DhfstKind::ErrorModel, &given("toDhfstErrorModel", fields))
                .expect("options")
                .max_fallback_depth
        };
        assert_eq!(depth(&[]), Some(DEFAULT_FALLBACK_DEPTH));
        assert_eq!(depth(&[("maxFallbackDepth", Value::Null)]), None);
        assert_eq!(depth(&[("maxFallbackDepth", 1.0.into())]), Some(1));
        let acceptor = given("toDhfstAcceptor", &[("maxFallbackDepth", 1.0.into())]);
        assert!(dhfst_options(DhfstKind::Acceptor, &acceptor).is_err());
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn a_speller_packs_with_its_metadata_and_configuration() {
        let acceptor = machine("{cat} | {dog}")
            .to_dhfst(&DhfstOptions::new(DhfstKind::Acceptor))
            .expect("an acceptor");
        let errmodel = machine("[?:?]*")
            .to_dhfst(&DhfstOptions::new(DhfstKind::ErrorModel))
            .expect("an error model");
        let config = given("packBhfst", &[("spellerConfig", "{\"beam\": 3}".into())]);
        let archive = pack(
            &acceptor,
            &errmodel,
            Some(b"{\"id\": \"x\"}".to_vec()),
            &config,
        )
        .expect("packed");
        let meta = embed::bhfst_metadata(&archive).expect("meta.json");
        let meta = String::from_utf8(meta).expect("text");
        assert!(
            meta.contains("\"id\": \"x\"") && meta.contains("\"beam\": 3"),
            "{meta}"
        );
        let bare = pack(&acceptor, &errmodel, None, &given("packBhfst", &[])).expect("packed");
        assert!(embed::bhfst_metadata(&bare).is_err(), "no meta.json");
    }
}
