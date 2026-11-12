use std::collections::HashMap;

use super::ModelSettings;
use super::build::{Counts, ModelBuilder};
use super::format::{
    CLASS_RECORD, FACE_RECORD, FORM_RECORD, FORM_UNIT_RECORD, MAGIC, NO_CLASS, UNIT_RECORD,
    VERSION, ambiguity_code, backoff_code, header_len, run_record,
};
use crate::error::Result;

impl ModelBuilder<'_> {
    /// The model file of everything counted so far, cut to the size limit.
    // [spec:hfst:def:prediction-model.file+1]
    // [spec:hfst:req:prediction-model.word-forms]
    pub fn write(&self) -> Result<Vec<u8>> {
        let mut tables = Tables::new(&self.counts, self.settings.face_size);
        tables.fit(self.settings.order, self.settings.size_limit)?;
        tables.serialize(self.grammar.hash(), &self.settings)
    }
}

/// The tables of a model, numbered and sorted as the file stores them.
struct Tables<'c> {
    /// Sorted by their bytes. A unit's id is its index here.
    keys: Vec<&'c str>,
    counts: Vec<f64>,
    /// Each unit's index in `class_names`, or [`NO_CLASS`].
    classes: Vec<u32>,
    /// Sorted by their bytes.
    class_names: Vec<&'c str>,
    /// Each class's face, in the order of `class_names`.
    faces: Vec<Vec<u32>>,
    /// The runs of each length from 2 to the order, at index length - 2,
    /// sorted by their ids.
    runs: Vec<Vec<(Vec<u32>, f64)>>,
    /// Sorted by their bytes. A form's id is its index here.
    forms: Vec<&'c str>,
    form_counts: Vec<f64>,
    /// The unit ids of the reading each form was counted with most often.
    form_units: Vec<Vec<u32>>,
    /// The runs of forms of each length from 2 to the order, at index
    /// length - 2, sorted by their ids.
    form_runs: Vec<Vec<(Vec<u32>, f64)>>,
}

impl<'c> Tables<'c> {
    fn new(counts: &'c Counts, face_size: usize) -> Tables<'c> {
        let unit_order = sorted_by(counts.keys.len(), |i| counts.keys[i].as_str());
        let class_order = sorted_by(counts.class_names.len(), |i| counts.class_names[i].as_str());
        let unit_id = inverse(&unit_order);
        let class_id = inverse(&class_order);
        let classes = unit_order
            .iter()
            .map(|&old| {
                main_class(&counts.unit_classes[old], &counts.class_names)
                    .map_or(NO_CLASS, |class| class_id[class as usize])
            })
            .collect();
        let runs = renumbered(&counts.runs, &unit_id);
        let form_order = sorted_by(counts.forms.len(), |i| counts.forms[i].as_str());
        let form_id = inverse(&form_order);
        let form_units = form_order
            .iter()
            .map(|&old| main_reading(&counts.form_readings[old], &unit_id))
            .collect();
        let mut tables = Tables {
            keys: unit_order
                .iter()
                .map(|&i| counts.keys[i].as_str())
                .collect(),
            counts: unit_order.iter().map(|&i| counts.units[i]).collect(),
            classes,
            class_names: class_order
                .iter()
                .map(|&i| counts.class_names[i].as_str())
                .collect(),
            faces: Vec::new(),
            runs,
            forms: form_order
                .iter()
                .map(|&i| counts.forms[i].as_str())
                .collect(),
            form_counts: form_order.iter().map(|&i| counts.form_counts[i]).collect(),
            form_units,
            form_runs: renumbered(&counts.form_runs, &form_id),
        };
        tables.faces = tables.faces(face_size);
        tables
    }

    /// Each class's `face_size` most frequent units, most frequent first.
    /// Units counted equally keep the order of their keys.
    // [spec:hfst:req:prediction-model.faces]
    fn faces(&self, face_size: usize) -> Vec<Vec<u32>> {
        (0..self.class_names.len())
            .map(|class| {
                let mut members: Vec<u32> = (0..self.keys.len())
                    .filter(|&unit| self.classes[unit] as usize == class)
                    .map(|unit| u32::try_from(unit).expect("unit ids fit in u32"))
                    .collect();
                members.sort_by(|&a, &b| {
                    self.counts[b as usize]
                        .total_cmp(&self.counts[a as usize])
                        .then(a.cmp(&b))
                });
                members.truncate(face_size);
                members
            })
            .collect()
    }

    fn strings(&self) -> impl Iterator<Item = &&'c str> {
        self.keys.iter().chain(&self.class_names).chain(&self.forms)
    }

    fn strings_len(&self) -> usize {
        self.strings().map(|s| s.len()).sum()
    }

    /// The size of the file with no runs.
    fn floor(&self, order: usize) -> usize {
        let face_entries: usize = self.faces.iter().map(Vec::len).sum();
        let form_units: usize = self.form_units.iter().map(Vec::len).sum();
        header_len(order)
            + self.strings_len()
            + UNIT_RECORD * self.keys.len()
            + CLASS_RECORD * self.class_names.len()
            + FACE_RECORD * face_entries
            + FORM_RECORD * self.forms.len()
            + FORM_UNIT_RECORD * form_units
    }

    fn size(&self, order: usize) -> usize {
        let runs: usize = (2..=order)
            .map(|length| {
                run_record(length)
                    * (self.runs[length - 2].len() + self.form_runs[length - 2].len())
            })
            .sum();
        self.floor(order) + runs
    }

    /// Drop the least frequent runs, longest first, until the file fits in
    /// `limit` bytes. Runs of units and runs of forms of one length are
    /// dropped together, least frequent first. Of runs counted equally, a
    /// run of units goes before a run of forms, and of two runs of the same
    /// kind the one with the later key goes first.
    // [spec:hfst:req:prediction-model.budget]
    // [spec:hfst:req:prediction-model.word-forms]
    fn fit(&mut self, order: usize, limit: usize) -> Result<()> {
        let floor = self.floor(order);
        if floor > limit {
            crate::bail!(
                Hfst,
                format!(
                    "prediction model: its unit keys and word forms alone take {floor} bytes, {} bytes more than the size limit of {limit}",
                    floor - limit
                )
            );
        }
        let mut size = self.size(order);
        for length in (2..=order).rev() {
            if size <= limit {
                break;
            }
            let record = run_record(length);
            let units = &mut self.runs[length - 2];
            let forms = &mut self.form_runs[length - 2];
            let dropping = (size - limit)
                .div_ceil(record)
                .min(units.len() + forms.len());
            let run = |(form, at): (bool, usize)| if form { &forms[at] } else { &units[at] };
            let mut by_count: Vec<(bool, usize)> = (0..units.len())
                .map(|at| (false, at))
                .chain((0..forms.len()).map(|at| (true, at)))
                .collect();
            by_count.sort_by(|&a, &b| {
                run(a)
                    .1
                    .total_cmp(&run(b).1)
                    .then(a.0.cmp(&b.0))
                    .then_with(|| run(b).0.cmp(&run(a).0))
            });
            let dropped = &by_count[..dropping];
            let unit_dropped: Vec<usize> = dropped.iter().filter(|r| !r.0).map(|r| r.1).collect();
            let form_dropped: Vec<usize> = dropped.iter().filter(|r| r.0).map(|r| r.1).collect();
            drop_runs(units, &unit_dropped);
            drop_runs(forms, &form_dropped);
            size -= dropping * record;
        }
        Ok(())
    }

    // [spec:hfst:def:prediction-model.file+1]
    // [spec:hfst:syn:prediction-model.format]
    fn serialize(&self, hash: [u8; 32], settings: &ModelSettings) -> Result<Vec<u8>> {
        let order = settings.order;
        let mut out = Writer(Vec::with_capacity(self.size(order)));
        out.0.extend_from_slice(MAGIC);
        out.u32(VERSION);
        out.0.extend_from_slice(&hash);
        out.usize(order)?;
        out.u32(backoff_code(settings.backoff));
        out.u32(ambiguity_code(settings.ambiguity));
        out.0
            .extend_from_slice(&self.counts.iter().sum::<f64>().to_le_bytes());
        out.0
            .extend_from_slice(&self.form_counts.iter().sum::<f64>().to_le_bytes());
        self.directory(&mut out, order)?;
        for string in self.strings() {
            out.0.extend_from_slice(string.as_bytes());
        }
        let mut string_at = 0;
        for ((key, &count), &class) in self.keys.iter().zip(&self.counts).zip(&self.classes) {
            out.usize(string_at)?;
            out.usize(key.len())?;
            out.f32(count);
            out.u32(class);
            string_at += key.len();
        }
        let mut face_at = 0;
        for (name, face) in self.class_names.iter().zip(&self.faces) {
            out.usize(string_at)?;
            out.usize(name.len())?;
            out.usize(face_at)?;
            out.usize(face.len())?;
            string_at += name.len();
            face_at += face.len();
        }
        for &unit in self.faces.iter().flatten() {
            out.u32(unit);
        }
        let mut unit_at = 0;
        for ((form, &count), units) in self
            .forms
            .iter()
            .zip(&self.form_counts)
            .zip(&self.form_units)
        {
            out.usize(string_at)?;
            out.usize(form.len())?;
            out.f32(count);
            out.usize(unit_at)?;
            out.usize(units.len())?;
            string_at += form.len();
            unit_at += units.len();
        }
        for &unit in self.form_units.iter().flatten() {
            out.u32(unit);
        }
        for (ids, count) in self.runs.iter().chain(&self.form_runs).flatten() {
            for &id in ids {
                out.u32(id);
            }
            out.f32(*count);
        }
        Ok(out.0)
    }

    /// Each table's offset and number of records, as the module
    /// documentation lists them.
    fn directory(&self, out: &mut Writer, order: usize) -> Result<()> {
        let face_entries: usize = self.faces.iter().map(Vec::len).sum();
        let form_units: usize = self.form_units.iter().map(Vec::len).sum();
        let mut tables = vec![
            (self.strings_len(), 1),
            (self.keys.len(), UNIT_RECORD),
            (self.class_names.len(), CLASS_RECORD),
            (face_entries, FACE_RECORD),
            (self.forms.len(), FORM_RECORD),
            (form_units, FORM_UNIT_RECORD),
        ];
        for runs in [&self.runs, &self.form_runs] {
            for length in 2..=order {
                tables.push((runs[length - 2].len(), run_record(length)));
            }
        }
        let mut at = header_len(order);
        for (records, record) in tables {
            out.usize(at)?;
            out.usize(records)?;
            at += records * record;
        }
        Ok(())
    }
}

/// Each table of runs with its ids renumbered by `ids`, sorted by the new
/// ids.
fn renumbered(tables: &[HashMap<Vec<u32>, f64>], ids: &[u32]) -> Vec<Vec<(Vec<u32>, f64)>> {
    tables
        .iter()
        .map(|table| {
            let mut runs: Vec<(Vec<u32>, f64)> = table
                .iter()
                .map(|(run, &count)| (run.iter().map(|&id| ids[id as usize]).collect(), count))
                .collect();
            runs.sort_by(|a, b| a.0.cmp(&b.0));
            runs
        })
        .collect()
}

/// Keep the runs whose indices `dropped` does not hold.
fn drop_runs(runs: &mut Vec<(Vec<u32>, f64)>, dropped: &[usize]) {
    let mut gone = vec![false; runs.len()];
    for &at in dropped {
        gone[at] = true;
    }
    let mut index = 0;
    runs.retain(|_| {
        index += 1;
        !gone[index - 1]
    });
}

/// The units, renumbered by `unit_id`, of the reading a form was counted
/// with most often. Of readings counted equally, the one whose units come
/// first by their keys.
// [spec:hfst:req:prediction-model.word-forms]
fn main_reading(tally: &[(Vec<u32>, f64)], unit_id: &[u32]) -> Vec<u32> {
    let renumbered =
        |ids: &[u32]| -> Vec<u32> { ids.iter().map(|&id| unit_id[id as usize]).collect() };
    tally
        .iter()
        .map(|(ids, count)| (renumbered(ids), *count))
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(ids, _)| ids)
        .unwrap_or_default()
}

fn sorted_by<'k>(n: usize, key: impl Fn(usize) -> &'k str) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| key(a).as_bytes().cmp(key(b).as_bytes()));
    order
}

/// For each old index, its place in `order`.
fn inverse(order: &[usize]) -> Vec<u32> {
    let mut places = vec![0; order.len()];
    for (place, &old) in order.iter().enumerate() {
        places[old] = u32::try_from(place).expect("fewer than 2^32 entries");
    }
    places
}

/// The class a unit was counted under most, the first by name when two
/// tie.
fn main_class(tally: &[(u32, f64)], names: &[String]) -> Option<u32> {
    tally
        .iter()
        .max_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| names[b.0 as usize].cmp(&names[a.0 as usize]))
        })
        .map(|&(class, _)| class)
}

struct Writer(Vec<u8>);

impl Writer {
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn f32(&mut self, value: f64) {
        self.0.extend_from_slice(&(value as f32).to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<()> {
        let Ok(value) = u32::try_from(value) else {
            crate::bail!(
                Hfst,
                format!("prediction model: {value} does not fit in the file's 32-bit fields")
            );
        };
        self.u32(value);
        Ok(())
    }
}
