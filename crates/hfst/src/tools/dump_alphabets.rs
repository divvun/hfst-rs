//! `hfst dump-alphabets`: the alphabet a transducer declares and the symbols
//! its arcs use, as data and as the lists the tool prints.

use std::io::Write;

use crate::hfst_symbol_defs::StringSet;
use crate::hfst_transducer::AnyTransducer;

/// A transducer's alphabets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Alphabets {
    /// The alphabet it declares, in its header or its symbol table.
    pub declared: StringSet,
    /// The symbols its arcs use.
    pub seen: StringSet,
}

impl Alphabets {
    // [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
    // [spec:hfst:req:embed.tools-in-library]
    // [spec:hfst:req:embed.no-panic]
    pub fn of(t: &AnyTransducer) -> crate::error::Result<Alphabets> {
        Ok(Alphabets {
            declared: t.get_alphabet()?,
            seen: t.to_basic()?.symbols_used(),
        })
    }
}

// [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
// [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
/// Whether `s` is a symbol the VISL CG 3 formats list: longer than two bytes
/// and starting with `+`, a space or `@`.
pub fn is_multichar(s: &str) -> bool {
    s.len() > 2 && (s.starts_with('+') || s.starts_with(' ') || s.starts_with('@'))
}

/// How `hfst dump-alphabets --format` prints the symbols.
// [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphabetFormat {
    /// One symbol a line.
    Tsv,
    /// `LIST x = x ;` for each multicharacter symbol.
    Vislcg3List,
    /// A `STRICT-TAGS +=` list of the multicharacter symbols.
    Vislcg3Tags,
}

impl AlphabetFormat {
    /// Whether the format lists only the [`is_multichar`] symbols.
    pub fn only_multichars(self) -> bool {
        match self {
            AlphabetFormat::Tsv => false,
            AlphabetFormat::Vislcg3List | AlphabetFormat::Vislcg3Tags => true,
        }
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// What `hfst dump-alphabets` prints for one transducer: the declared
/// alphabet unless `!declared`, then the seen symbols unless `!seen`.
pub fn write_alphabets(
    out: &mut dyn Write,
    alphabets: &Alphabets,
    format: AlphabetFormat,
    (declared, seen): (bool, bool),
) -> std::io::Result<()> {
    match format {
        AlphabetFormat::Vislcg3Tags => {
            out.write_all(b"## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n## (some statistics here TODO)\nSTRICT-TAGS +=\n")?;
        }
        AlphabetFormat::Vislcg3List => {
            out.write_all(b"## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n## (some statistics here TODO)\n")?;
        }
        AlphabetFormat::Tsv => {}
    }
    let lists = [(declared, &alphabets.declared), (seen, &alphabets.seen)];
    for (printed, symbols) in lists {
        if !printed {
            continue;
        }
        for s in symbols.iter() {
            if format.only_multichars() && !is_multichar(s) {
                continue;
            }
            match format {
                AlphabetFormat::Tsv => writeln!(out, "{}", s)?,
                AlphabetFormat::Vislcg3Tags => writeln!(out, "\t{}", s)?,
                AlphabetFormat::Vislcg3List => writeln!(out, "LIST {} = {} ;", s, s)?,
            }
        }
    }
    Ok(())
}

/// What `hfst dump-alphabets` prints after the last transducer.
pub fn write_end(out: &mut dyn Write, format: AlphabetFormat) -> std::io::Result<()> {
    match format {
        AlphabetFormat::Vislcg3Tags => out.write_all(b"\t;\n"),
        AlphabetFormat::Tsv | AlphabetFormat::Vislcg3List => Ok(()),
    }
}
