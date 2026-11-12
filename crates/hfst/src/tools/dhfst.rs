//! `hfst fst2fst -f dhfst`: a transducer written as a DHFST error model or
//! acceptor, and the report `--verbose` prints of it.

use crate::dhfst_acceptor_writer::{AcceptorOptions, AcceptorSource, WrittenAcceptor};
use crate::dhfst_writer::{SourceModel, WriteOptions, Written};
use crate::hfst_data_types::ImplementationType;
use crate::hfst_transducer::{AnyTransducer, HfstTransducer};
use crate::transducer::{Transducer, WeightedTables};

/// What a DHFST file holds, as `--dhfst-type` names it. The type is never
/// guessed from the transducer.
// [spec:hfst:def:dhfst.fst2fst+2]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DhfstKind {
    /// `--dhfst-type errmodel`: an error model, type 1.
    ErrorModel,
    /// `--dhfst-type acceptor`: an acceptor, type 2.
    Acceptor,
}

/// The longest fallback chain an error model is written with unless told
/// otherwise.
pub const DEFAULT_FALLBACK_DEPTH: u32 = 4;

/// How a transducer is written as DHFST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DhfstOptions {
    pub kind: DhfstKind,
    /// An error model's longest fallback chain, `None` for no bound:
    /// `--max-fallback-depth` and `--unbounded-fallback`. An acceptor has
    /// no fallbacks.
    pub max_fallback_depth: Option<u32>,
    /// Relax the optimized-lookup table packing of the conversion: `--quick`.
    pub quick: bool,
    /// The name the meta section gives the source: the file name, without
    /// its directory, that the tool read it from.
    pub source_name: String,
}

impl DhfstOptions {
    pub fn new(kind: DhfstKind) -> Self {
        DhfstOptions {
            kind,
            max_fallback_depth: Some(DEFAULT_FALLBACK_DEPTH),
            quick: false,
            source_name: String::new(),
        }
    }
}

/// A DHFST file and the writer's report of it.
#[derive(Clone, Debug)]
pub struct DhfstFile {
    pub bytes: Vec<u8>,
    pub report: String,
}

type Weighted = HfstTransducer<Transducer<WeightedTables>>;

fn not_weighted() -> crate::error::Error {
    crate::err!(
        Hfst,
        "internal error: the conversion did not yield weighted optimized lookup"
    )
}

// [spec:hfst:sem:dhfst.fst2fst+2]
// [spec:hfst:req:embed.tools-in-library]
/// `t` as DHFST of the kind `options` ask for: converted to weighted
/// optimized lookup as `-f olw` converts it, then encoded and checked by the
/// DHFST writer, which gives no bytes unless the check passes.
pub fn write_dhfst(t: AnyTransducer, options: &DhfstOptions) -> crate::error::Result<DhfstFile> {
    let converted = t.convert(
        ImplementationType::HFST_OLW_TYPE,
        if options.quick { "quick" } else { "" },
    )?;
    let olw = match converted {
        AnyTransducer::OlW(t) => t,
        AnyTransducer::Tropical(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_) => {
            return Err(not_weighted());
        }
        #[cfg(feature = "foma")]
        AnyTransducer::Foma(_) => return Err(not_weighted()),
    };
    match options.kind {
        DhfstKind::ErrorModel => write_errmodel(olw, options),
        DhfstKind::Acceptor => write_acceptor(&olw, options),
    }
}

fn write_errmodel(olw: Weighted, options: &DhfstOptions) -> crate::error::Result<DhfstFile> {
    let model = SourceModel::from_olw(&olw)?;
    drop(olw);
    let written = crate::dhfst_writer::write(
        &model,
        &WriteOptions {
            max_fallback_depth: options.max_fallback_depth,
            threads: 0,
            source_name: options.source_name.clone(),
        },
    )?;
    let report = dhfst_report(&model, &written, options.max_fallback_depth);
    Ok(DhfstFile {
        bytes: written.bytes,
        report,
    })
}

fn write_acceptor(olw: &Weighted, options: &DhfstOptions) -> crate::error::Result<DhfstFile> {
    let source = AcceptorSource::new(olw)?;
    let written = crate::dhfst_acceptor_writer::write(
        &source,
        &AcceptorOptions {
            threads: 0,
            source_name: options.source_name.clone(),
        },
    )?;
    let report = acceptor_report(&written);
    Ok(DhfstFile {
        bytes: written.bytes,
        report,
    })
}

/// The DHFST acceptor writer's report, as `hfst fst2fst -v` prints it.
fn acceptor_report(w: &WrittenAcceptor) -> String {
    format!(
        "DHFST acceptor: {} states numbered below {}, {} arcs ({} free) in {} slots \
         ({} used, {:.1}%), {} list records, {} weights, {} free pairs, {}\n\
         checked: {} (state, symbol) answers read back from the written bytes, \
         all equal to the source ({} free-symbol quirks)\n\
         wrote {} bytes of DHFST\n",
        w.states,
        w.ids,
        w.arcs,
        w.free_arcs,
        w.slots,
        w.used_slots,
        100.0 * w.used_slots as f64 / w.slots.max(1) as f64,
        w.list_records,
        w.weights,
        w.free_pairs,
        if w.distances {
            "distances to a final state stored"
        } else {
            "every distance to a final state 0, none stored"
        },
        w.checked,
        w.free_symbol_quirks,
        w.bytes.len(),
    )
}

/// The DHFST error-model writer's report, as `hfst fst2fst -v` prints it.
fn dhfst_report(model: &SourceModel, written: &Written, depth: Option<u32>) -> String {
    let r = &written.report;
    let pc = |x: u64| 100.0 * x as f64 / r.source_arcs.max(1) as f64;
    let defaults: u64 = r.defaults.iter().sum();
    let entries = r.explicit_arcs + r.blockers + defaults;
    let b = &r.answered_by;
    format!(
        "DHFST source: {} states, {} arcs, {} symbols, {} duplicate arcs dropped\n\
         encoding: {entries} entries ({:.2}% of arcs) = explicit {} + blockers {} + defaults {defaults} \
         (identity {}, substitution {}, deletion {}, insertion {}); classes {}, class pairs {}\n\
         fallbacks: {} states; depth bound {}, longest chain {}; states per chain length {:?}\n\
         source arcs answered by: own explicit {:.2}%, own default identity {:.2}% \
         substitution {:.2}% deletion {:.2}% insertion {:.2}%, inherited explicit {:.2}%, \
         inherited default {:.2}%\n\
         checked: {} (state, pair) resolutions of the encoding; {} (state, input) queries, \
         {} arcs, read back from the written bytes: all equal to the source\n\
         wrote {} bytes of DHFST\n",
        model.states().len(),
        r.source_arcs,
        model.symbols().len(),
        model.duplicate_arcs(),
        pc(entries),
        r.explicit_arcs,
        r.blockers,
        r.defaults[0],
        r.defaults[1],
        r.defaults[2],
        r.defaults[3],
        r.classes,
        r.class_pairs,
        r.states_with_fallback,
        depth.map_or("none".to_string(), |d| d.to_string()),
        r.max_depth,
        r.depth_histogram,
        pc(b[0]),
        pc(b[1]),
        pc(b[2]),
        pc(b[3]),
        pc(b[4]),
        pc(b[5]),
        pc(b[6]),
        r.pairs_checked,
        r.queries_checked,
        r.arcs_checked,
        written.bytes.len(),
    )
}
