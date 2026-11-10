//! The pyo3 glue behind `divvun-hfst`.
//!
//! maturin builds this crate as the extension module `divvun_hfst._divvun_hfst`.
//! The `divvun_hfst` package in `python/divvun_hfst/` re-exports it and is the
//! public API.

mod args;
mod call;
mod dhfst;
mod error;
mod log;
mod optimised;
mod tokenise;
mod transducer;

// [spec:hfst:req:python.parity]
// Every reader and every call reaches hfst-embed, the code @divvun/hfst runs
// too, with the caller's arguments turned into its types and its results
// turned back, and nothing else between: the results are the library's.
// python/tests/test_parity.py checks them against the native library's on
// the npm package's parity fixtures.

/// What the package calls itself and its readers, for the errors hfst-embed
/// raises that send a caller to another reader.
const PACKAGE: hfst_embed::Package = hfst_embed::Package {
    name: "divvun-hfst",
    read_transducers: "read_transducers",
    read_optimised_transducer: "read_optimised_transducer",
    read_dhfst: "read_dhfst",
};

/// Divvun HFST for Python: reads optimized-lookup transducers, pmatch
/// tokenisers and DHFST files, and looks words up, tokenises text and checks
/// words against DHFST acceptors with them.
#[pyo3::pymodule(name = "_divvun_hfst")]
mod module {
    use pyo3::prelude::*;
    use pyo3::types::PyTuple;

    #[pymodule_export]
    use crate::dhfst::{AcceptorInfo, DhfstAcceptor, DhfstErrorModel, load_dhfst, read_dhfst};
    #[pymodule_export]
    use crate::error::{FormatError, HfstError};
    #[pymodule_export]
    use crate::optimised::{
        OptimisedTransducer, load_optimised_transducer, read_optimised_transducer,
    };
    #[pymodule_export]
    use crate::tokenise::{
        Reading, Token, Tokeniser, check_format_options, check_tokenise_options, load_tokeniser,
        read_tokeniser,
    };
    #[pymodule_export]
    use crate::transducer::{
        LookupResult, Transducer, load_transducer, load_transducers, read_transducer,
        read_transducers,
    };

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("__version__", env!("CARGO_PKG_VERSION"))?;
        // Each `typing.Literal` is made from the table the binding checks its
        // values against, so the two cannot disagree.
        let literal = m.py().import("typing")?.getattr("Literal")?;
        let add = |name: &str, values: Vec<&'static str>| -> PyResult<()> {
            let values = PyTuple::new(m.py(), values)?;
            m.add(name, literal.get_item(values)?)
        };
        add(
            "HfstErrorCode",
            hfst_embed::Code::ALL
                .iter()
                .map(|code| code.name())
                .collect(),
        )?;
        add(
            "TransducerType",
            hfst_embed::format::Format::ALL
                .iter()
                .filter(|format| format.is_read())
                .map(|format| format.name())
                .collect(),
        )?;
        add(
            "TokeniseFormat",
            hfst_embed::tokenise::FORMATS
                .iter()
                .map(|(name, _)| *name)
                .collect(),
        )
    }
}
