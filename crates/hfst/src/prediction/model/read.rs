use std::cmp::Ordering;

use super::format::{
    AMBIGUITY_AT, BACKOFF_AT, CLASS_RECORD, DIRECTORY_AT, DIRECTORY_ENTRY, FACE_RECORD,
    FIXED_TABLES, HASH_AT, MAGIC, NO_CLASS, ORDER_AT, TOTAL_AT, UNIT_RECORD, VERSION, VERSION_AT,
    ambiguity_from_code, backoff_from_code, hex, le_f64, le_u32, run_record,
};
use super::{Ambiguity, Backoff};
use crate::error::Result;
use crate::prediction::units::{UnitGrammar, UnitKey};

/// What stupid backoff multiplies a score by for each unit it drops from the
/// context.
const BACKOFF_FACTOR: f64 = 0.4;

/// A table's place in the file. [`Model::from_bytes`] checks that every
/// record lies inside the file, so reading one cannot go out of bounds.
#[derive(Clone, Copy, Debug)]
struct Table {
    at: usize,
    records: usize,
    record: usize,
}

impl Table {
    fn record<'a>(&self, bytes: &'a [u8], index: usize) -> &'a [u8] {
        let start = self.at + index * self.record;
        &bytes[start..start + self.record]
    }
}

/// A model file, read where it lies. Only the header is read when it is
/// loaded; each count is found by binary search over the file's bytes. A
/// file damaged past its header gives wrong answers, never a panic.
// [spec:hfst:def:prediction-model.file]
#[derive(Clone, Debug)]
pub struct Model<'a> {
    bytes: &'a [u8],
    order: usize,
    backoff: Backoff,
    ambiguity: Ambiguity,
    total: f64,
    strings: Table,
    units: Table,
    classes: Table,
    faces: Table,
    /// One for each run length from 2 to the order.
    runs: Vec<Table>,
}

fn invalid(cause: String) -> crate::error::Error {
    crate::err!(InvalidModelFile, format!("prediction model: {cause}"))
}

impl<'a> Model<'a> {
    /// Read a model's header and check it was built with `grammar`.
    // [spec:hfst:syn:prediction-model.format]
    // [spec:hfst:req:prediction-model.same-grammar]
    pub fn from_bytes(bytes: &'a [u8], grammar: &UnitGrammar) -> Result<Model<'a>> {
        if bytes.len() < DIRECTORY_AT || &bytes[..MAGIC.len()] != MAGIC {
            return Err(invalid("the file does not start with HFSTPRED".to_owned()));
        }
        let version = le_u32(bytes, VERSION_AT);
        if version != VERSION {
            return Err(invalid(format!(
                "format version {version}; this reader reads version {VERSION}"
            )));
        }
        let built_with = &bytes[HASH_AT..HASH_AT + 32];
        if built_with != grammar.hash() {
            return Err(invalid(format!(
                "built with the unit grammar whose hash is {}, but loaded with the grammar whose hash is {}",
                hex(built_with),
                hex(&grammar.hash())
            )));
        }
        let order = le_u32(bytes, ORDER_AT) as usize;
        if order == 0 {
            return Err(invalid("order 0".to_owned()));
        }
        let backoff_code = le_u32(bytes, BACKOFF_AT);
        let backoff = backoff_from_code(backoff_code)
            .ok_or_else(|| invalid(format!("backoff method {backoff_code} is not 0 or 1")))?;
        let ambiguity_code = le_u32(bytes, AMBIGUITY_AT);
        let ambiguity = ambiguity_from_code(ambiguity_code)
            .ok_or_else(|| invalid(format!("ambiguity rule {ambiguity_code} is not 0 or 1")))?;
        let mut tables = directory(bytes, order)?;
        let runs = tables.split_off(FIXED_TABLES);
        let [strings, units, classes, faces] = tables[..] else {
            return Err(invalid("the directory is short".to_owned()));
        };
        Ok(Model {
            bytes,
            order,
            backoff,
            ambiguity,
            total: le_f64(bytes, TOTAL_AT),
            strings,
            units,
            classes,
            faces,
            runs,
        })
    }

    pub fn order(&self) -> usize {
        self.order
    }

    pub fn backoff(&self) -> Backoff {
        self.backoff
    }

    pub fn ambiguity(&self) -> Ambiguity {
        self.ambiguity
    }

    /// How many times the run was counted, 0 when never or when it is longer
    /// than the order. The empty run's count is that of all units.
    pub fn count(&self, run: &[UnitKey]) -> f64 {
        let ids: Option<Vec<u32>> = run.iter().map(|key| self.unit_id(key)).collect();
        ids.map_or(0.0, |ids| self.run_count(&ids))
    }

    /// The face of a class: its most frequent unit keys, most frequent
    /// first. Empty for a class the model has no unit of.
    // [spec:hfst:req:prediction-model.faces]
    pub fn face(&self, class: &str) -> Vec<UnitKey> {
        let found = self.search(self.classes, |record| {
            self.string(record)
                .map_or(Ordering::Less, |name| name.cmp(class.as_bytes()))
        });
        let Some(index) = found else {
            return Vec::new();
        };
        let record = self.classes.record(self.bytes, index);
        let start = le_u32(record, 8) as usize;
        let end = start.saturating_add(le_u32(record, 12) as usize);
        (start..end.min(self.faces.records))
            .filter_map(|entry| self.key(le_u32(self.faces.record(self.bytes, entry), 0)))
            .collect()
    }

    /// The class the key was counted under most, if any.
    pub fn class_of(&self, key: &UnitKey) -> Option<&'a str> {
        let unit = self.unit_id(key)? as usize;
        let class = le_u32(self.units.record(self.bytes, unit), 12);
        if class == NO_CLASS || class as usize >= self.classes.records {
            return None;
        }
        let name = self.string(self.classes.record(self.bytes, class as usize))?;
        std::str::from_utf8(name).ok()
    }

    /// The key's score after the context, or `None` when the model never saw
    /// the key. Only the last order - 1 units of the context are read. With
    /// drop-oldest backoff this is the count after the longest end of the
    /// context after which the key was seen; [`Model::rank`] instead picks
    /// one context length for all its candidates.
    // [spec:hfst:sem:prediction-model.order]
    pub fn score(&self, context: &[UnitKey], key: &UnitKey) -> Option<f64> {
        let key = self.unit_id(key)?;
        let context = self.context(context);
        let score = match self.backoff {
            Backoff::DropOldest => (0..=context.len())
                .map(|drop| self.count_after(&context[drop..], key))
                .find(|&count| count > 0.0)
                .unwrap_or(0.0),
            Backoff::StupidBackoff => self.stupid_backoff(&context, key),
        };
        (score > 0.0).then_some(score)
    }

    /// The count after the context over the context's count, or 0.4 times
    /// the score after the context less its oldest unit when the key was not
    /// seen after it; with no context left, the key's share of all units.
    // [spec:hfst:sem:prediction-model.order]
    fn stupid_backoff(&self, context: &[Option<u32>], key: u32) -> f64 {
        let mut factor = 1.0;
        for drop in 0..context.len() {
            let context = &context[drop..];
            let seen = self.count_after(context, key);
            let of = self.context_count(context);
            if seen > 0.0 && of > 0.0 {
                return factor * seen / of;
            }
            factor *= BACKOFF_FACTOR;
        }
        if self.total > 0.0 {
            factor * self.run_count(&[key]) / self.total
        } else {
            0.0
        }
    }

    /// The ids of the last order - 1 units of the context, `None` for a
    /// unit the model never saw.
    pub(crate) fn context(&self, context: &[UnitKey]) -> Vec<Option<u32>> {
        let keep = context.len().min(self.order - 1);
        context[context.len() - keep..]
            .iter()
            .map(|key| self.unit_id(key))
            .collect()
    }

    /// How many times the key was seen right after the context.
    pub(crate) fn count_after(&self, context: &[Option<u32>], key: u32) -> f64 {
        let ids: Option<Vec<u32>> = context
            .iter()
            .copied()
            .chain(std::iter::once(Some(key)))
            .collect();
        ids.map_or(0.0, |ids| self.run_count(&ids))
    }

    pub(crate) fn context_count(&self, context: &[Option<u32>]) -> f64 {
        let ids: Option<Vec<u32>> = context.iter().copied().collect();
        ids.map_or(0.0, |ids| self.run_count(&ids))
    }

    pub(crate) fn unit_id(&self, key: &UnitKey) -> Option<u32> {
        let key = key.as_str().as_bytes();
        let index = self.search(self.units, |record| {
            self.string(record)
                .map_or(Ordering::Less, |found| found.cmp(key))
        })?;
        u32::try_from(index).ok()
    }

    fn key(&self, id: u32) -> Option<UnitKey> {
        let id = id as usize;
        if id >= self.units.records {
            return None;
        }
        let bytes = self.string(self.units.record(self.bytes, id))?;
        std::str::from_utf8(bytes).ok().map(UnitKey::from)
    }

    fn run_count(&self, ids: &[u32]) -> f64 {
        match ids {
            [] => self.total,
            [unit] => {
                let unit = *unit as usize;
                if unit >= self.units.records {
                    return 0.0;
                }
                f64::from(f32::from_bits(le_u32(
                    self.units.record(self.bytes, unit),
                    8,
                )))
            }
            [_, _, ..] => {
                let Some(&table) = self.runs.get(ids.len() - 2) else {
                    return 0.0;
                };
                let found = self.search(table, |record| {
                    (0..ids.len())
                        .map(|i| le_u32(record, 4 * i))
                        .cmp(ids.iter().copied())
                });
                found.map_or(0.0, |index| {
                    f64::from(f32::from_bits(le_u32(
                        table.record(self.bytes, index),
                        4 * ids.len(),
                    )))
                })
            }
        }
    }

    /// The string a unit or class record's first two fields locate.
    fn string(&self, record: &[u8]) -> Option<&'a [u8]> {
        let offset = le_u32(record, 0) as usize;
        let length = le_u32(record, 4) as usize;
        let end = offset.checked_add(length)?;
        if end > self.strings.records {
            return None;
        }
        self.bytes
            .get(self.strings.at + offset..self.strings.at + end)
    }

    /// Binary search over a table's records. `compare` orders a record
    /// against the one sought.
    // [spec:hfst:syn:prediction-model.format]
    fn search(&self, table: Table, compare: impl Fn(&'a [u8]) -> Ordering) -> Option<usize> {
        let (mut low, mut high) = (0, table.records);
        while low < high {
            let middle = low + (high - low) / 2;
            match compare(table.record(self.bytes, middle)) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Some(middle),
            }
        }
        None
    }
}

/// The tables the directory locates, each checked to lie inside the file.
fn directory(bytes: &[u8], order: usize) -> Result<Vec<Table>> {
    let too_long = || {
        invalid(format!(
            "the directory of a model of order {order} runs past the end of the file"
        ))
    };
    let entries = (order - 1).checked_add(FIXED_TABLES).ok_or_else(too_long)?;
    let entries_end = entries
        .checked_mul(DIRECTORY_ENTRY)
        .and_then(|length| length.checked_add(DIRECTORY_AT))
        .filter(|&end| end <= bytes.len())
        .ok_or_else(too_long)?;
    let record_sizes = [1, UNIT_RECORD, CLASS_RECORD, FACE_RECORD]
        .into_iter()
        .chain((2..=order).map(run_record));
    let mut tables = Vec::with_capacity(entries);
    for (index, record) in record_sizes.enumerate() {
        let entry = DIRECTORY_AT + index * DIRECTORY_ENTRY;
        let at = le_u32(bytes, entry) as usize;
        let records = le_u32(bytes, entry + 4) as usize;
        let end = records
            .checked_mul(record)
            .and_then(|length| length.checked_add(at));
        if at < entries_end || end.is_none_or(|end| end > bytes.len()) {
            return Err(invalid(format!(
                "table {index} of the directory lies outside the file"
            )));
        }
        tables.push(Table {
            at,
            records,
            record,
        });
    }
    Ok(tables)
}
