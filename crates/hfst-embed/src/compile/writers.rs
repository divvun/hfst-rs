//! The writers of the compile surface besides [`Transducer::to_bytes`]:
//! DHFST and THFST as `hfst fst2fst` writes them, the text `hfst fst2txt`
//! writes, and the speller archives `hfst bhfst` packs.

use hfst::file_resolver::FileResolver;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_transducer::AnyTransducer;
#[cfg(feature = "bhfst")]
use hfst::tools::bhfst::BhfstError;
use hfst::tools::fst2txt::{TextError, write_text};

use super::transducer::Transducer;
use crate::error::{Code, Error, code_of};

#[cfg(feature = "bhfst")]
pub use hfst::tools::bhfst::{MetaFile, SpellerFiles};
pub use hfst::tools::dhfst::{DEFAULT_FALLBACK_DEPTH, DhfstKind, DhfstOptions};
pub use hfst::tools::fst2txt::{TextFormat, TextOptions};

fn text_error(error: TextError) -> Error {
    let code = match &error {
        TextError::Prolog(e) => code_of(&e.kind),
        TextError::Write(_) => Code::Failed,
    };
    Error::new(code, error.to_string())
}

#[cfg(feature = "bhfst")]
fn bhfst_error(error: BhfstError) -> Error {
    let code = match error {
        BhfstError::ConfigWithoutMetadata => Code::InvalidArgument,
        BhfstError::Message(_) => Code::Failed,
    };
    Error::new(code, error.to_string())
}

/// Write `t`, the `n`th of its stream, as text, ignoring the notes the tool
/// prints on standard error: the Prolog name they announce is in the text.
fn text_of(
    t: &AnyTransducer,
    n: usize,
    options: &TextOptions,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    write_text(t, n, options, out, &mut |_| {}).map_err(text_error)
}

impl Transducer {
    // [spec:hfst:req:embed.writers]
    /// This transducer as the DHFST file `hfst fst2fst -f dhfst` writes of
    /// it.
    pub fn to_dhfst(&self, options: &DhfstOptions) -> Result<Vec<u8>, Error> {
        let written = hfst::tools::dhfst::write_dhfst(self.inner.copy()?, options)?;
        Ok(written.bytes)
    }

    // [spec:hfst:req:embed.writers]
    /// This transducer as `hfst fst2txt` writes it when it is alone in its
    /// stream; a Prolog network is named `NO_NAME_1`.
    pub fn to_text(&self, options: &TextOptions) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        text_of(&self.inner, 1, options, &mut out)?;
        Ok(out)
    }

    // [spec:hfst:req:embed.writers]
    /// Write this transducer as the THFST directory `dir` through `files`,
    /// as `hfst fst2fst -f thfst -o DIR` writes it. `quick` relaxes the
    /// table packing, as `--quick`.
    pub fn write_thfst(
        &self,
        dir: &str,
        quick: bool,
        files: &dyn FileResolver,
    ) -> Result<(), Error> {
        let converted = self.inner.converted(
            ImplementationType::THFST_TYPE,
            if quick { "quick" } else { "" },
        )?;
        let AnyTransducer::Thfst(thfst) = converted else {
            return Err(Error::new(
                Code::Internal,
                "the conversion did not yield THFST",
            ));
        };
        thfst.thfst_members()?.write_to(dir, files)?;
        Ok(())
    }
}

// [spec:hfst:req:embed.writers]
/// The text `hfst fst2txt` writes of the stream `transducers`: each after a
/// `--` line but the first, and each Prolog network named for its place.
pub fn text(transducers: &[&Transducer], options: &TextOptions) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    for (i, t) in transducers.iter().enumerate() {
        text_of(&t.inner, i + 1, options, &mut out)?;
    }
    Ok(out)
}

#[cfg(feature = "bhfst")]
// [spec:hfst:req:embed.writers]
/// The BHFST archive `hfst bhfst` packs from the files `speller` names, read
/// through `files`.
pub fn pack_bhfst(speller: &SpellerFiles, files: &dyn FileResolver) -> Result<Vec<u8>, Error> {
    hfst::tools::bhfst::pack_files(speller, files, &mut |_| {}).map_err(bhfst_error)
}

#[cfg(feature = "bhfst")]
// [spec:hfst:req:embed.writers]
/// The BHFST archive `hfst bhfst --zhfst` makes of the zhfst file `zhfst`,
/// with the speller configuration in the file `speller_config` in place of
/// the zhfst's own, each read through `files`.
pub fn pack_zhfst(
    zhfst: &str,
    speller_config: Option<&str>,
    files: &dyn FileResolver,
) -> Result<Vec<u8>, Error> {
    hfst::tools::bhfst::pack_zhfst(zhfst, speller_config, files, &mut |_| {}).map_err(bhfst_error)
}

#[cfg(feature = "bhfst")]
/// The meta.json of the BHFST archive `archive`, as `hfst bhfst --info`
/// prints it but for the newline it ends a line with.
pub fn bhfst_metadata(archive: &[u8]) -> Result<Vec<u8>, Error> {
    hfst::tools::bhfst::meta_json_of("the archive", archive).map_err(bhfst_error)
}
