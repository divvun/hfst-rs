//! DHFST files, divvunspell's speller formats, read from bytes: an acceptor
//! to look words up in, or an error model to describe.

use std::sync::Arc;

use hfst_embed::dhfst::{AcceptorReader, Dhfst as Read, read_dhfst as read};

use crate::PACKAGE;
use crate::error::GlueError;

// [spec:hfst:req:swift.dhfst]
/// How a DHFST acceptor stores its arcs, as `dhfst-tools` reports it.
#[derive(uniffi::Record)]
pub struct AcceptorInfo {
    pub ids: u32,
    pub slots: u32,
    pub used_slots: u32,
    pub check_bytes: u64,
    pub label_max: u32,
    pub record_bytes: u64,
    pub list_records: u32,
    pub arcs: u64,
    pub free_arcs: u64,
    pub free_pairs: u32,
    pub weights: u32,
    pub finals: u32,
    pub distances: bool,
}

// [spec:hfst:req:swift.dhfst]
// [spec:hfst:req:swift.threads]
/// A DHFST acceptor, read and validated as divvunspell reads it. It never
/// changes, so threads may look words up in it at once.
#[derive(uniffi::Object)]
pub struct DhfstAcceptor {
    reader: Box<AcceptorReader<'static>>,
}

#[uniffi::export]
impl DhfstAcceptor {
    pub fn version(&self) -> u8 {
        self.reader.version()
    }

    pub fn symbols(&self) -> Vec<String> {
        self.reader.symbol_names().to_vec()
    }

    pub fn meta(&self) -> Option<String> {
        self.reader.meta().map(str::to_owned)
    }

    pub fn info(&self) -> AcceptorInfo {
        let info = self.reader.info();
        AcceptorInfo {
            ids: info.ids,
            slots: info.slots,
            used_slots: info.used_slots,
            check_bytes: info.check_bytes as u64,
            label_max: info.label_max,
            record_bytes: info.record_bytes as u64,
            list_records: info.list_records,
            arcs: info.arcs,
            free_arcs: info.free_arcs,
            free_pairs: info.free_pairs,
            weights: info.weights,
            finals: info.finals,
            distances: info.distances,
        }
    }

    // [spec:hfst:req:swift.dhfst]
    /// The least weight of a path that accepts `word`, as `@divvun/hfst`'s
    /// acceptor lookup gives it.
    pub fn lookup(&self, word: String) -> Option<f32> {
        self.reader.lookup(&word)
    }
}

// [spec:hfst:req:swift.dhfst]
/// What a DHFST error model's header and sections say.
#[derive(uniffi::Record)]
pub struct DhfstErrorModel {
    pub version: u8,
    pub state_count: u32,
    pub symbols: Vec<String>,
    pub meta: Option<String>,
}

// [spec:hfst:req:swift.dhfst]
#[derive(uniffi::Enum)]
pub enum Dhfst {
    Acceptor { acceptor: Arc<DhfstAcceptor> },
    ErrorModel { model: DhfstErrorModel },
}

// [spec:hfst:req:swift.load]
// [spec:hfst:req:swift.dhfst]
/// The DHFST file in `bytes`, as `@divvun/hfst`'s `readDhfst` reads it.
#[uniffi::export]
pub fn read_dhfst(bytes: Vec<u8>) -> Result<Dhfst, GlueError> {
    Ok(match read(&bytes, &PACKAGE)? {
        Read::Acceptor(reader) => Dhfst::Acceptor {
            acceptor: Arc::new(DhfstAcceptor { reader }),
        },
        Read::ErrorModel(model) => Dhfst::ErrorModel {
            model: DhfstErrorModel {
                version: model.version,
                state_count: model.state_count,
                symbols: model.symbols,
                meta: model.meta,
            },
        },
    })
}
