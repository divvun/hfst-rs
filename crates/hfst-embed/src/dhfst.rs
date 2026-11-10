//! DHFST files, divvunspell's speller formats, read from bytes: an acceptor
//! to look words up in, or an error model to describe.

use hfst::dhfst::DhfstReader;
pub use hfst::dhfst_acceptor::{AcceptorInfo, AcceptorReader};
use hfst::dhfst_header::{DhfstType, MAGIC, read_type};

use crate::Package;
use crate::error::{Code, Error};
use crate::stream;

const INVALID: Code = Code::InvalidFormat;

// [spec:hfst:req:javascript.dhfst+2]
// [spec:hfst:req:python.dhfst]
/// A DHFST file of either type, read and validated as divvunspell reads it.
pub enum Dhfst {
    /// Type 2. Its `lookup` answers as divvunspell's speller checks a word.
    Acceptor(Box<AcceptorReader<'static>>),
    /// Type 1.
    ErrorModel(ErrorModel),
}

/// What a DHFST error model's header and sections say. Nothing in the
/// packages searches an error model, so its bytes are not kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorModel {
    pub version: u8,
    pub state_count: u32,
    pub symbols: Vec<String>,
    pub meta: Option<String>,
}

// [spec:hfst:req:javascript.dhfst+2]
// [spec:hfst:req:javascript.load-bytes+2]
// [spec:hfst:req:python.dhfst]
// [spec:hfst:req:python.load]
/// The DHFST file in `bytes`, validated as divvunspell validates it on load.
pub fn read_dhfst(bytes: &[u8], package: &Package) -> Result<Dhfst, Error> {
    if !bytes.starts_with(MAGIC) {
        return Err(not_dhfst(bytes, package));
    }
    let invalid = |error: hfst::error::Error| Error::new(INVALID, error.to_string());
    match read_type(bytes).map_err(invalid)? {
        DhfstType::Acceptor => {
            let reader = AcceptorReader::parse_owned(bytes.to_vec()).map_err(invalid)?;
            Ok(Dhfst::Acceptor(Box::new(reader)))
        }
        DhfstType::ErrorModel => {
            let reader = DhfstReader::parse(bytes).map_err(invalid)?;
            Ok(Dhfst::ErrorModel(ErrorModel {
                version: reader.version(),
                state_count: reader.state_count(),
                symbols: reader.symbol_names().to_vec(),
                meta: reader.meta().map(str::to_owned),
            }))
        }
    }
}

/// The error for bytes that do not start as DHFST does. A transducer the
/// packages read is named, with the reader that reads it; a format they do
/// not read is turned down by name, as the transducer readers turn it down.
fn not_dhfst(bytes: &[u8], package: &Package) -> Error {
    let neither = || {
        Error::new(
            INVALID,
            "the bytes are not DHFST: they do not begin with \"DHFST\", nor with a transducer stream",
        )
    };
    if bytes.is_empty() {
        return Error::new(
            INVALID,
            "the bytes are empty, and a DHFST file begins with an eight-byte header",
        );
    }
    match stream::member(bytes, package) {
        Ok(member) if member.format.is_read() => Error::new(
            INVALID,
            format!(
                "the bytes are not DHFST: they hold an {} transducer, which {} reads",
                member.format.name(),
                package.read_transducers
            ),
        ),
        Ok(member) => stream::unsupported(
            &format!("a transducer in the {} format", member.format.name()),
            package,
        ),
        Err(error) if error.code() == INVALID => neither(),
        Err(error) => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst::dhfst_acceptor_writer::{AcceptorOptions, AcceptorSource};
    use hfst::dhfst_source::{SourceArc, SourceModel, SourceState};
    use hfst::dhfst_writer::WriteOptions;
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_transducer::HfstTransducer;

    /// An acceptor of `talo` (0.5) and `talon` (1.5), with a flag diacritic.
    fn acceptor() -> Vec<u8> {
        let att = "0\t1\t@P.CASE.NOM@\t@P.CASE.NOM@\t0\n1\t2\tt\tt\t0\n2\t3\ta\ta\t0\n\
                   3\t4\tl\tl\t0\n4\t5\to\to\t0.5\n5\t6\t@R.CASE.NOM@\t@R.CASE.NOM@\t0\n\
                   6\t0\n6\t7\tn\tn\t1\n7\t0\n";
        let mut lines = 0u32;
        let net =
            HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut lines, false)
                .expect("the AT&T text parses");
        let olw = HfstTransducer::<hfst_openfst::StdVectorFst>::new_from_basic(&net)
            .expect("a transducer")
            .to_ol(true, "")
            .expect("weighted optimized lookup");
        let source = AcceptorSource::new(&olw).expect("an acceptor source");
        let options = AcceptorOptions {
            threads: 1,
            source_name: "talo".into(),
        };
        hfst::dhfst_acceptor_writer::write(&source, &options)
            .expect("the acceptor writes")
            .bytes
    }

    /// An error model of two states that turns `a` into `b`.
    fn error_model() -> Vec<u8> {
        let symbols = ["@_EPSILON_SYMBOL_@", "a", "b"].map(String::from).to_vec();
        let arc = |input, output, target, weight| SourceArc {
            input,
            output,
            target,
            weight,
        };
        let states = vec![
            SourceState {
                final_weight: Some(0.0),
                arcs: vec![arc(1, 1, 0, 0.0), arc(1, 2, 1, 1.0)],
            },
            SourceState {
                final_weight: Some(0.0),
                arcs: vec![arc(1, 1, 1, 0.0)],
            },
        ];
        let model = SourceModel::new(symbols, states).expect("a model");
        let options = WriteOptions {
            threads: 1,
            source_name: "a-to-b".into(),
            ..WriteOptions::default()
        };
        hfst::dhfst_writer::write(&model, &options)
            .expect("the error model writes")
            .bytes
    }

    fn read(bytes: &[u8]) -> Result<Dhfst, Error> {
        read_dhfst(bytes, &crate::TEST_PACKAGE)
    }

    fn code(result: Result<Dhfst, Error>) -> &'static str {
        match result {
            Ok(Dhfst::Acceptor(_)) => "acceptor",
            Ok(Dhfst::ErrorModel(_)) => "error-model",
            Err(error) => error.code().name(),
        }
    }

    // [spec:hfst:req:javascript.dhfst+2/test]
    #[test]
    fn an_acceptor_loads_and_looks_words_up() {
        let Ok(Dhfst::Acceptor(acceptor)) = read(&acceptor()) else {
            panic!("the acceptor does not load as one");
        };
        assert_eq!(acceptor.version(), 1);
        assert_eq!(acceptor.symbol_names()[0], "@_EPSILON_SYMBOL_@");
        assert!(
            acceptor
                .meta()
                .is_some_and(|meta| meta.contains("\"talo\""))
        );
        assert_eq!(acceptor.lookup("talo"), Some(0.5));
        assert_eq!(acceptor.lookup("talon"), Some(1.5));
        assert_eq!(acceptor.lookup("tal"), None);
    }

    // [spec:hfst:req:javascript.dhfst+2/test]
    #[test]
    fn an_error_model_loads_with_its_metadata() {
        let Ok(Dhfst::ErrorModel(model)) = read(&error_model()) else {
            panic!("the error model does not load as one");
        };
        assert_eq!(model.version, 1);
        assert_eq!(model.state_count, 2);
        assert_eq!(model.symbols, ["@_EPSILON_SYMBOL_@", "a", "b"]);
        assert!(model.meta.is_some_and(|meta| meta.contains("\"a-to-b\"")));
    }

    // [spec:hfst:req:javascript.dhfst+2/test]
    // [spec:hfst:req:javascript.load-bytes+2/test]
    #[test]
    fn invalid_or_foreign_bytes_are_refused() {
        let acceptor = acceptor();
        let mut as_error_model = acceptor.clone();
        as_error_model[5] = 1;
        let mut unknown_type = acceptor.clone();
        unknown_type[5] = 9;
        let invalid = INVALID.name();
        let cases: [(Vec<u8>, &str); 9] = [
            (acceptor[..acceptor.len() - 9].to_vec(), invalid),
            (acceptor[..6].to_vec(), invalid),
            (as_error_model, invalid),
            (unknown_type, invalid),
            (error_model()[..40].to_vec(), invalid),
            (Vec::new(), invalid),
            (b"not a speller".to_vec(), invalid),
            (b"##foma-net 1.0##\n".to_vec(), "unsupported-format"),
            (vec![0x1f, 0x8b, 0x08, 0, 0], "unsupported-format"),
        ];
        for (bytes, expected) in cases {
            assert_eq!(code(read(&bytes)), expected, "{bytes:?}");
        }
    }
}
