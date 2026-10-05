//! The eight bytes every DHFST file starts with: `DHFST`, a type byte, a
//! version byte and a reserved zero byte. Authored greenfield against
//! `docs/spec/port/back-ends/dhfst/dhfst.md`, after divvunspell's
//! `TransducerFormat::detect`. The type says what the file holds; this crate
//! reads and writes only error models, type 1. The rest of an error model is
//! [`crate::dhfst`].

use crate::dhfst::corrupt;

/// The first five bytes of a DHFST file.
// [spec:hfst:def:dhfst.header+1]
pub const MAGIC: &[u8; 5] = b"DHFST";
/// Bytes every DHFST file starts with: the magic, the type, the version and
/// a reserved byte.
pub const PREFIX_LEN: usize = 8;

/// What a DHFST file holds, as byte 5 of its header says.
// [spec:hfst:def:dhfst.header+1]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DhfstType {
    /// An error model, type 1.
    ErrorModel,
    /// An acceptor, type 2.
    Acceptor,
}

impl DhfstType {
    /// The type a header's type byte names, or `None` for a reserved value.
    pub fn from_byte(byte: u8) -> Option<DhfstType> {
        match byte {
            1 => Some(DhfstType::ErrorModel),
            2 => Some(DhfstType::Acceptor),
            _ => None,
        }
    }

    /// The header's type byte for this type.
    pub fn byte(self) -> u8 {
        match self {
            DhfstType::ErrorModel => 1,
            DhfstType::Acceptor => 2,
        }
    }

    /// The format version of this type that this reader reads and the
    /// writer writes.
    pub fn version(self) -> u8 {
        match self {
            DhfstType::ErrorModel => 1,
            DhfstType::Acceptor => 1,
        }
    }

    /// The type's name with its article, as in "an error model".
    pub fn with_article(self) -> &'static str {
        match self {
            DhfstType::ErrorModel => "an error model",
            DhfstType::Acceptor => "an acceptor",
        }
    }

    /// The eight bytes every file of this type starts with.
    pub fn prefix(self) -> [u8; PREFIX_LEN] {
        let mut prefix = [0u8; PREFIX_LEN];
        prefix[..MAGIC.len()].copy_from_slice(MAGIC);
        prefix[5] = self.byte();
        prefix[6] = self.version();
        prefix
    }
}

impl std::fmt::Display for DhfstType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DhfstType::ErrorModel => "error model",
            DhfstType::Acceptor => "acceptor",
        })
    }
}

/// Whether `bytes` start the way a DHFST file does, whatever its type and
/// version.
// [spec:hfst:sem:dhfst.header+1]
pub fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// The type a DHFST file's first eight bytes declare. A type this reader does
/// not know is refused by its number, and so is a version of a known type
/// other than the one it reads, and a reserved byte that is not zero.
// [spec:hfst:sem:dhfst.header+1]
pub fn read_type(b: &[u8]) -> crate::error::Result<DhfstType> {
    if !has_magic(b) {
        return Err(corrupt("it does not start with \"DHFST\""));
    }
    let Some(&[type_byte, version, reserved]) = b.get(MAGIC.len()..PREFIX_LEN) else {
        return Err(corrupt(
            "the header is truncated before its type and version",
        ));
    };
    let Some(kind) = DhfstType::from_byte(type_byte) else {
        return Err(corrupt(format!(
            "DHFST type {type_byte} is not a type this reader knows; \
             type 1 is an error model and type 2 an acceptor"
        )));
    };
    if version != kind.version() {
        return Err(corrupt(format!(
            "DHFST {kind} version {version}; this reader reads version {}",
            kind.version()
        )));
    }
    if reserved != 0 {
        return Err(corrupt(format!(
            "header byte 7 is {reserved}; it is reserved and must be 0"
        )));
    }
    Ok(kind)
}

/// The refusal of a DHFST file of type `found` where `wanted` is needed.
pub fn wrong_type(found: DhfstType, wanted: DhfstType) -> crate::error::Error {
    crate::err!(
        Hfst,
        format!(
            "this DHFST file is {} (type {}); {} is type {}",
            found.with_article(),
            found.byte(),
            wanted.with_article(),
            wanted.byte()
        )
    )
}
