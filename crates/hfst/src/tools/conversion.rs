//! The conversions the tools make between transducer types: to the type of
//! the stream a transducer is written to, and of two transducers to one type
//! before an operation combines them.

use crate::backend::AlgebraBackend;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer, is_safe_conversion};
use crate::tools::format::format_description;
use crate::tools::metadata::{hfst_set_formula_unary, hfst_set_name_unary};

/// Which of two transducers converts to the other's type, without loss
/// unless the variant says otherwise, before an operation combines them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionNote {
    SecondToFirst(ImplementationType),
    FirstToSecond(ImplementationType),
    /// Neither converts without loss; the second converts to the first's.
    SecondToFirstLossy(ImplementationType),
}

// [spec:hfst:def:hfst-commandline.conversion-type-fn]
// [spec:hfst:sem:hfst-commandline.conversion-type-fn]
// [spec:hfst:req:embed.tools-in-library]
/// The conversion two transducers of types `first` and `second` need, or
/// `None` when they have one type.
pub fn conversion_needed(
    first: ImplementationType,
    second: ImplementationType,
) -> Option<ConversionNote> {
    if first == second {
        None
    } else if is_safe_conversion(second, first) {
        Some(ConversionNote::SecondToFirst(first))
    } else if is_safe_conversion(first, second) {
        Some(ConversionNote::FirstToSecond(second))
    } else {
        Some(ConversionNote::SecondToFirstLossy(first))
    }
}

impl ConversionNote {
    /// The type both transducers end up with.
    pub fn target(self) -> ImplementationType {
        match self {
            ConversionNote::SecondToFirst(ty)
            | ConversionNote::FirstToSecond(ty)
            | ConversionNote::SecondToFirstLossy(ty) => ty,
        }
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// How a binary tool says which input's type its output takes.
    pub fn output_choice(self) -> &'static str {
        match self {
            ConversionNote::SecondToFirst(_) => "using former type as output",
            ConversionNote::FirstToSecond(_) => "using latter type as output",
            ConversionNote::SecondToFirstLossy(_) => {
                "using former type as output, loss of information is possible"
            }
        }
    }

    // [spec:hfst:def:hfst-commandline.convert-transducers-fn]
    // [spec:hfst:sem:hfst-commandline.convert-transducers-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Convert `first` and `second` as this note says.
    pub fn apply(
        self,
        first: AnyTransducer,
        second: AnyTransducer,
    ) -> crate::error::Result<(AnyTransducer, AnyTransducer)> {
        Ok(match self {
            ConversionNote::SecondToFirst(ty) | ConversionNote::SecondToFirstLossy(ty) => {
                (first, second.convert(ty, "")?)
            }
            ConversionNote::FirstToSecond(ty) => (first.convert(ty, "")?, second),
        })
    }
}

/// The warning a tool prints before it converts, newline included.
impl std::fmt::Display for ConversionNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionNote::SecondToFirst(ty) | ConversionNote::FirstToSecond(ty) => writeln!(
                f,
                "transducers have different types, converting to format {}",
                format_description(*ty)
            ),
            ConversionNote::SecondToFirstLossy(ty) => writeln!(
                f,
                "transducers have different types, converting to format {}, loss of information is possible",
                format_description(*ty)
            ),
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// Convert `first` and `second` to one type, as the binary tools do before
/// they combine two transducers.
pub fn convert_to_common_type(
    first: AnyTransducer,
    second: AnyTransducer,
) -> crate::error::Result<(AnyTransducer, AnyTransducer)> {
    match conversion_needed(first.get_type(), second.get_type()) {
        Some(note) => note.apply(first, second),
        None => Ok((first, second)),
    }
}

impl HfstOutputStream {
    // [spec:hfst:req:embed.tools-in-library]
    /// Write `t`, converted first to the type this stream was opened with
    /// when that is optimized lookup or foma, as a compiler tool writes its
    /// result.
    pub fn write_converted<B: AlgebraBackend>(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<()> {
        match self.get_type() {
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                let weighted = self.get_type() == ImplementationType::HFST_OLW_TYPE;
                self.write(&t.to_ol(weighted, "")?)?;
            }
            #[cfg(feature = "foma")]
            ImplementationType::FOMA_TYPE => {
                self.write(&t.to_foma()?)?;
            }
            _ => {
                self.write(t)?;
            }
        }
        Ok(())
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// `t` converted to `ty` as `hfst fst2fst` converts it, and named as it
/// names what it writes. `quick` relaxes the optimized-lookup table packing,
/// as `--quick` does.
pub fn fst2fst(
    t: AnyTransducer,
    ty: ImplementationType,
    quick: bool,
) -> crate::error::Result<AnyTransducer> {
    let mut converted = t.convert(ty, if quick { "quick" } else { "" })?;
    name_converted(&mut converted);
    Ok(converted)
}

// [spec:hfst:req:embed.tools-in-library]
/// Name `t` as `hfst fst2fst` names what it converted.
pub fn name_converted(t: &mut AnyTransducer) {
    hfst_set_name_unary(t, "convert");
    hfst_set_formula_unary(t, "Id");
}
