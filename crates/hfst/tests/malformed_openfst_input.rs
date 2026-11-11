//! A tropical OpenFst stream that is corrupt, or AT&T or Prolog text that
//! names an empty symbol, fails to read with an error. rustfst panics on a
//! header string that is not UTF-8 and trusts every number it reads, so the
//! read checks them first.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The one-symbol language `zq` as an HFST3 tropical stream.
fn tropical_bytes() -> Vec<u8> {
    let t = XreCompiler::<StdVectorFst>::new()
        .compile("zq")
        .expect("the regex compiles");
    let buffer = Shared::default();
    let mut out = HfstOutputStream::from_writer(
        Box::new(buffer.clone()),
        ImplementationType::TROPICAL_OPENFST_TYPE,
        true,
    )
    .expect("a tropical stream");
    out.write(&t).expect("the transducer writes");
    out.close();
    buffer.0.take()
}

fn read_error(bytes: &[u8]) -> String {
    let mut cursor = std::io::Cursor::new(bytes.to_vec());
    let mut stream = HfstInputStream::read_from(&mut cursor).expect("an HFST3 stream");
    match stream.read() {
        Ok(_) => panic!("the corrupt transducer read"),
        Err(e) => e.to_string(),
    }
}

fn find(bytes: &[u8], piece: &[u8]) -> usize {
    bytes
        .windows(piece.len())
        .position(|w| w == piece)
        .expect("the piece is in the stream")
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn symbol_that_is_not_utf8_is_rejected() {
    let mut bytes = tropical_bytes();
    let at = find(&bytes, b"zq");
    bytes[at + 1] = 0xff;
    assert!(read_error(&bytes).contains("not UTF-8"));
}

// The symbol table's row count follows its magic number, its name and its
// next free key. rustfst reads a negative count as a huge one.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn negative_symbol_count_is_rejected() {
    let mut bytes = tropical_bytes();
    let table = find(&bytes, &2_125_658_996i32.to_le_bytes());
    let name_len = i32::from_le_bytes(bytes[table + 4..table + 8].try_into().expect("4 bytes"));
    let count = table + 8 + name_len as usize + 8;
    bytes[count..count + 8].copy_from_slice(&(-5i64).to_le_bytes());
    assert!(read_error(&bytes).contains("cut short"));
}

// rustfst sizes a symbol table by its largest key.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn huge_symbol_key_is_rejected() {
    let mut bytes = tropical_bytes();
    let key = find(&bytes, b"zq") + 2;
    bytes[key..key + 8].copy_from_slice(&(3_000_000_000i64).to_le_bytes());
    assert!(read_error(&bytes).contains("out of range"));
}

// An arc is (input i32, output i32, weight f32, target i32). Aim the one
// arc's target at a state the transducer does not have.
// [spec:hfst:req:embed.no-panic/test]
#[test]
fn arc_to_a_missing_state_is_rejected() {
    let mut bytes = tropical_bytes();
    let arc = (1..64i32)
        .find_map(|label| {
            let mut pattern = Vec::new();
            pattern.extend_from_slice(&label.to_le_bytes());
            pattern.extend_from_slice(&label.to_le_bytes());
            pattern.extend_from_slice(&0f32.to_le_bytes());
            pattern.extend_from_slice(&1i32.to_le_bytes());
            bytes.windows(16).position(|w| w == pattern.as_slice())
        })
        .expect("the zq arc");
    bytes[arc + 12..arc + 16].copy_from_slice(&40_000i32.to_le_bytes());
    assert!(read_error(&bytes).contains("state it does not hold"));
}

// [spec:hfst:req:embed.no-panic/test]
#[test]
fn text_arc_with_empty_symbol_is_an_error() {
    let mut line = 0u32;
    let att = "0\t1\ta\t\n1\n";
    let read = HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut line, true);
    assert!(read.is_err(), "an AT&T arc with an empty output read");

    let prolog = "network(net).\narc(net, 0, 1, \"a\":\"\").\nfinal(net, 1).\n";
    let read = HfstBasicTransducer::read_in_prolog_format(&mut prolog.as_bytes(), &mut line);
    let Err(e) = read else {
        panic!("a Prolog arc with an empty output read");
    };
    assert!(e.to_string().contains("empty symbol"), "{e}");
}
