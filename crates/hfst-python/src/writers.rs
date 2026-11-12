//! The writers: a transducer as `hfst fst2fst` writes it in each type and as
//! DHFST and THFST, as the text `hfst fst2txt` writes, and spellers packed as
//! `hfst bhfst` packs them.

use std::path::PathBuf;

use hfst_embed::compile::{
    self as embed, DEFAULT_FALLBACK_DEPTH, DhfstKind, DhfstOptions, FsResolver, MemoryFiles,
    MetaFile, SpellerFiles, TextFormat, TextOptions, WriteOptions,
};
use hfst_embed::{Code, Error};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::{PyByteArray, PyBytes, PyString};

use crate::args::Value;
use crate::call::without_gil;
use crate::error::hfst_error;
use crate::transducer::{Transducer, format_named};

/// The text `fst2txt` writes, which is UTF-8 since every symbol is.
fn text(bytes: Vec<u8>) -> Result<String, Error> {
    String::from_utf8(bytes).map_err(|_| Error::new(Code::Internal, "the text is not UTF-8"))
}

fn text_options(
    format: TextFormat,
    print_weights: bool,
    do_not_print_weights: bool,
    use_numbers: bool,
) -> TextOptions {
    TextOptions {
        format,
        print_weights,
        do_not_print_weights,
        use_numbers,
    }
}

// [spec:hfst:req:python.writers]
// [spec:hfst:req:python.threads+1]
#[pymethods]
impl Transducer {
    /// The bytes of a stream holding the transducer, as `hfst fst2fst`
    /// writes it: in `type`, converted and named as `--format` converts and
    /// names it, or as it is, in its own type, when `type` is None.
    /// `use_backend_format` leaves out the HFST3 header, and `quick` packs
    /// optimized-lookup tables less tightly.
    #[pyo3(signature = (r#type = None, /, *, use_backend_format = false, quick = false))]
    fn to_bytes<'py>(
        &self,
        py: Python<'py>,
        r#type: Option<&str>,
        use_backend_format: bool,
        quick: bool,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let format = r#type
            .map(|name| format_named("to_bytes()", name))
            .transpose()?;
        let options = WriteOptions {
            format,
            header: !use_backend_format,
            quick,
        };
        let bytes = self.read(py, |t| t.to_bytes(&options))?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// The AT&T text `hfst fst2txt` writes of the transducer. Weights are
    /// written for every type but foma, which has none, unless
    /// `do_not_print_weights`; `print_weights` writes them for foma too.
    /// `use_numbers` writes symbol numbers rather than names.
    #[pyo3(signature = (*, print_weights = false, do_not_print_weights = false, use_numbers = false))]
    fn to_att(
        &self,
        py: Python<'_>,
        print_weights: bool,
        do_not_print_weights: bool,
        use_numbers: bool,
    ) -> PyResult<String> {
        let options = text_options(
            TextFormat::Att,
            print_weights,
            do_not_print_weights,
            use_numbers,
        );
        self.read(py, |t| text(t.to_text(&options)?))
    }

    /// The Prolog text `hfst fst2txt -f prolog` writes of the transducer
    /// alone in its stream, so a transducer with no name is `NO_NAME_1`.
    #[pyo3(signature = (*, print_weights = false, do_not_print_weights = false))]
    fn to_prolog(
        &self,
        py: Python<'_>,
        print_weights: bool,
        do_not_print_weights: bool,
    ) -> PyResult<String> {
        let options = text_options(
            TextFormat::Prolog,
            print_weights,
            do_not_print_weights,
            false,
        );
        self.read(py, |t| text(t.to_text(&options)?))
    }

    /// The Graphviz text `hfst fst2txt -f dot` writes of the transducer.
    #[pyo3(signature = (*, print_weights = false, do_not_print_weights = false))]
    fn to_dot(
        &self,
        py: Python<'_>,
        print_weights: bool,
        do_not_print_weights: bool,
    ) -> PyResult<String> {
        let options = text_options(TextFormat::Dot, print_weights, do_not_print_weights, false);
        self.read(py, |t| text(t.to_text(&options)?))
    }

    /// The PC-KIMMO text `hfst fst2txt -f pckimmo` writes of the transducer.
    #[pyo3(signature = (*, print_weights = false, do_not_print_weights = false))]
    fn to_pckimmo(
        &self,
        py: Python<'_>,
        print_weights: bool,
        do_not_print_weights: bool,
    ) -> PyResult<String> {
        let options = text_options(
            TextFormat::Pckimmo,
            print_weights,
            do_not_print_weights,
            false,
        );
        self.read(py, |t| text(t.to_text(&options)?))
    }

    /// The DHFST acceptor (type 2) `hfst fst2fst -f dhfst --dhfst-type
    /// acceptor` writes of the transducer. `source_name` is the file name,
    /// without its directory, the tool would have read it from, which the
    /// file's meta section names; `quick` packs the conversion's tables
    /// less tightly.
    #[pyo3(signature = (*, quick = false, source_name = ""))]
    fn to_dhfst_acceptor<'py>(
        &self,
        py: Python<'py>,
        quick: bool,
        source_name: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let options = DhfstOptions {
            quick,
            source_name: source_name.to_owned(),
            ..DhfstOptions::new(DhfstKind::Acceptor)
        };
        let bytes = self.read(py, |t| t.to_dhfst(&options))?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// The DHFST error model (type 1) `hfst fst2fst -f dhfst --dhfst-type
    /// errmodel` writes of the transducer, its fallback chains at most
    /// `max_fallback_depth` long, as `--max-fallback-depth`, or unbounded
    /// when it is None, as `--unbounded-fallback`. `source_name` and `quick`
    /// are to_dhfst_acceptor's.
    #[pyo3(
        signature = (*, max_fallback_depth = Some(DEFAULT_FALLBACK_DEPTH), quick = false, source_name = ""),
        text_signature = "($self, /, *, max_fallback_depth=4, quick=False, source_name='')"
    )]
    fn to_dhfst_error_model<'py>(
        &self,
        py: Python<'py>,
        max_fallback_depth: Option<u32>,
        quick: bool,
        source_name: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let options = DhfstOptions {
            kind: DhfstKind::ErrorModel,
            max_fallback_depth,
            quick,
            source_name: source_name.to_owned(),
        };
        let bytes = self.read(py, |t| t.to_dhfst(&options))?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Writes the transducer as the THFST directory at `path`, divvunspell's
    /// format, as `hfst fst2fst -f thfst -o PATH` writes it: the directory
    /// and its three files `alphabet`, `index` and `transition`. `quick`
    /// packs the tables less tightly. A directory that cannot be written
    /// raises HfstError.
    #[pyo3(signature = (path, /, *, quick = false))]
    fn write_thfst(&self, py: Python<'_>, path: PathBuf, quick: bool) -> PyResult<()> {
        let Some(dir) = path.to_str() else {
            return Err(PyValueError::new_err(format!(
                "write_thfst() argument 'path' must be named in UTF-8, not {path:?}"
            )));
        };
        self.read(py, |t| t.write_thfst(dir, quick, &FsResolver))
    }
}

/// The bytes of `value`, a `str` as UTF-8, or the TypeError for argument
/// `name` of `pack_bhfst()`.
fn contents(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(text.to_str()?.as_bytes().to_vec());
    }
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(bytes.as_bytes().to_vec());
    }
    if let Ok(bytes) = value.cast::<PyByteArray>() {
        return Ok(bytes.to_vec());
    }
    Err(Value {
        call: "pack_bhfst()",
        name,
        value,
    }
    .wrong_type("str, bytes or bytearray"))
}

// [spec:hfst:req:python.writers]
/// The BHFST archive `hfst bhfst` packs of a speller: `acceptor` and
/// `error_model` each the bytes of a DHFST file, packed as they are, or of a
/// transducer stream, converted to THFST; and `metadata`, the text of the
/// archive's meta.json, or with `index_xml` of a zhfst index.xml to convert,
/// as `--meta` and `--index-xml` take them. `speller_config` is the text of
/// the speller runtime configuration `--speller-config` adds to meta.json.
#[pyfunction]
#[pyo3(signature = (acceptor, error_model, metadata = None, /, *, index_xml = false, speller_config = None))]
pub fn pack_bhfst<'py>(
    py: Python<'py>,
    acceptor: PyBackedBytes,
    error_model: PyBackedBytes,
    metadata: Option<&Bound<'_, PyAny>>,
    index_xml: bool,
    speller_config: Option<&Bound<'_, PyAny>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let files = MemoryFiles::default();
    files.insert("acceptor", acceptor.to_vec());
    files.insert("errmodel", error_model.to_vec());
    let meta = match metadata.filter(|m| !m.is_none()) {
        None => MetaFile::None,
        Some(metadata) => {
            let name = if index_xml { "index.xml" } else { "meta.json" };
            files.insert(name, contents("metadata", metadata)?);
            if index_xml {
                MetaFile::IndexXml(name.to_owned())
            } else {
                MetaFile::Meta(name.to_owned())
            }
        }
    };
    let speller_config = match speller_config.filter(|c| !c.is_none()) {
        None => None,
        Some(config) => {
            files.insert("speller-config.json", contents("speller_config", config)?);
            Some("speller-config.json".to_owned())
        }
    };
    let speller = SpellerFiles {
        acceptor: "acceptor".to_owned(),
        errmodel: "errmodel".to_owned(),
        meta,
        speller_config,
    };
    let packed = without_gil(py, || embed::pack_bhfst(&speller, &files))?
        .map_err(|error| hfst_error(py, error))?;
    Ok(PyBytes::new(py, &packed))
}
