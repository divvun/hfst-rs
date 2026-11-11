//! The transducer type of the compile surface, and reading and writing it.

use std::io::Cursor;

use hfst::file_resolver::{FileResolver, MemoryFiles};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use hfst::tools::conversion::name_converted;

use crate::error::{Code, Error};
use crate::format::Format;
use crate::stream;

// [spec:hfst:req:embed.transducer]
/// A transducer of any type the library reads: tropical OpenFst, foma, and
/// optimized lookup, weighted and unweighted. A THFST transducer is the
/// weighted optimized-lookup engine in a directory, and is held as weighted
/// optimized lookup.
///
/// Operations change it in place. Nothing in it is shared with another
/// transducer, so it is `Send` and `Sync`, and [`Transducer::copy`] makes one
/// that changes on its own.
pub struct Transducer {
    pub(crate) inner: AnyTransducer,
}

const _: () = {
    const fn send_and_sync<T: Send + Sync>() {}
    send_and_sync::<Transducer>();
};

/// The second transducer of a binary operation.
#[derive(Clone, Copy)]
pub enum Operand<'a> {
    Other(&'a Transducer),
    /// The transducer the operation is called on, as it was before the
    /// operation, as in `t.compose(t)`.
    Itself,
}

/// How [`Transducer::to_bytes`] writes.
#[derive(Clone, Copy, Debug)]
pub struct WriteOptions {
    /// The type to write in, converted and named as `hfst fst2fst --format`
    /// converts and names it; `None` writes the transducer as it is, in its
    /// own type, as the compilers and operations write their results.
    pub format: Option<Format>,
    /// Write the HFST3 header; `false` writes the backend's own format, as
    /// `--use-backend-format` does.
    pub header: bool,
    /// Do not try hard to compress optimized-lookup tables, as `--quick`.
    pub quick: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            format: None,
            header: true,
            quick: false,
        }
    }
}

pub(crate) fn implementation_type(format: Format) -> ImplementationType {
    match format {
        Format::OptimizedLookupWeighted => ImplementationType::HFST_OLW_TYPE,
        Format::OptimizedLookupUnweighted => ImplementationType::HFST_OL_TYPE,
        Format::OpenFstTropical => ImplementationType::TROPICAL_OPENFST_TYPE,
        Format::Foma => ImplementationType::FOMA_TYPE,
    }
}

/// THFST held as what it is, weighted optimized lookup.
fn held(t: AnyTransducer) -> AnyTransducer {
    match t {
        AnyTransducer::Thfst(t) => AnyTransducer::OlW(t.into_olw()),
        other @ (AnyTransducer::Tropical(_)
        | AnyTransducer::OlW(_)
        | AnyTransducer::OlU(_)
        | AnyTransducer::Foma(_)) => other,
    }
}

/// The bytes `write` writes to a stream of type `ty`.
fn stream_bytes(
    ty: ImplementationType,
    header: bool,
    write: impl FnOnce(&mut HfstOutputStream) -> hfst::error::Result<()>,
) -> Result<Vec<u8>, Error> {
    let files = MemoryFiles::default();
    let sink = files
        .create("stream")
        .map_err(|e| Error::new(Code::Io, e.to_string()))?;
    let mut out = HfstOutputStream::from_writer(sink, ty, header)?;
    write(&mut out)?;
    out.close();
    files
        .get("stream")
        .ok_or_else(|| Error::new(Code::Internal, "the stream was not written"))
}

// [spec:hfst:req:embed.transducer]
/// Every transducer of the stream `bytes`, in stream order, in any type the
/// library reads, with an HFST3 header or without one.
pub fn read_transducers(bytes: &[u8]) -> Result<Vec<Transducer>, Error> {
    if bytes.is_empty() {
        return Err(stream::empty());
    }
    let mut cursor = Cursor::new(bytes);
    let mut stream = HfstInputStream::read_from(&mut cursor)?;
    let mut read = Vec::new();
    while stream.is_good() {
        read.push(Transducer {
            inner: held(stream.read()?),
        });
    }
    Ok(read)
}

impl Transducer {
    pub(crate) fn new(inner: AnyTransducer) -> Transducer {
        Transducer { inner: held(inner) }
    }

    pub fn format(&self) -> Format {
        match &self.inner {
            AnyTransducer::Tropical(_) => Format::OpenFstTropical,
            AnyTransducer::Foma(_) => Format::Foma,
            AnyTransducer::OlU(_) => Format::OptimizedLookupUnweighted,
            AnyTransducer::OlW(_) | AnyTransducer::Thfst(_) => Format::OptimizedLookupWeighted,
        }
    }

    /// The `name` property, or `""` without one.
    pub fn name(&self) -> String {
        self.inner.get_name()
    }

    /// The properties but `version`, `type` and `name`, in key order.
    pub fn properties(&self) -> Vec<(String, String)> {
        stream::shown(self.inner.get_properties())
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    /// A transducer of its own with the same contents and properties.
    pub fn copy(&self) -> Result<Transducer, Error> {
        Ok(Transducer::new(self.inner.copy()?))
    }

    // [spec:hfst:req:embed.transducer]
    /// This transducer in `format`, converted as `hfst fst2fst` converts it,
    /// without the name `fst2fst` gives what it writes. `quick` relaxes the
    /// optimized-lookup table packing.
    pub fn convert(&mut self, format: Format, quick: bool) -> Result<(), Error> {
        let ty = implementation_type(format);
        if self.inner.get_type() != ty {
            self.inner = held(self.inner.converted(ty, if quick { "quick" } else { "" })?);
        }
        Ok(())
    }

    // [spec:hfst:req:embed.transducer]
    // [spec:hfst:req:embed.writers]
    /// The bytes of a stream holding this transducer, written as `options`
    /// say.
    pub fn to_bytes(&self, options: &WriteOptions) -> Result<Vec<u8>, Error> {
        match options.format {
            None => {
                let ty = self.inner.get_type();
                stream_bytes(ty, options.header, |out| write_any(&self.inner, out))
            }
            Some(format) => {
                let ty = implementation_type(format);
                let mut converted = if self.inner.get_type() == ty {
                    self.inner.copy()?
                } else {
                    self.inner
                        .converted(ty, if options.quick { "quick" } else { "" })?
                };
                name_converted(&mut converted);
                stream_bytes(ty, options.header, |out| converted.write(out))
            }
        }
    }
}

/// Write `t` to `out` without changing it.
pub(crate) fn write_any(t: &AnyTransducer, out: &mut HfstOutputStream) -> hfst::error::Result<()> {
    match t {
        AnyTransducer::Tropical(t) => out.write(t).map(|_| ()),
        AnyTransducer::OlW(t) => out.write(t).map(|_| ()),
        AnyTransducer::OlU(t) => out.write(t).map(|_| ()),
        AnyTransducer::Foma(t) => out.write(t).map(|_| ()),
        AnyTransducer::Thfst(t) => out.write(t).map(|_| ()),
    }
}
