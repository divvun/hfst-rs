use super::{SymbolClass, Unit, UnitGrammar, UnitKind};
use crate::error::Result;
use crate::hfst_data_types::Symbol;

impl UnitGrammar {
    /// Cut one complete analysis, already split into symbols, into units by
    /// its tags alone. This is how the model builder reads analysed text;
    /// the walk cuts where the generator's configurations are final instead.
    /// So the ending here is the whole run of tags before the first
    /// enclitic, not each shorter ending a walk would also offer, and no
    /// filter is ever found, since an analysis does not spell one.
    ///
    /// The analysis is refused when it holds a flag diacritic or a reserved
    /// symbol, has a tag before its first stem that matches no prefix
    /// pattern, has no stem, or has a letter after its ending.
    pub fn segment<S: AsRef<str>>(&self, analysis: &[S]) -> Result<Vec<Unit>> {
        let mut cutter = Cutter::new(self, analysis)?;
        cutter.prefixes()?;
        cutter.stem()?;
        cutter.after_stem()?;
        Ok(cutter.units)
    }
}

/// An analysis being cut, with the units cut so far and where the next one
/// starts.
struct Cutter<'g> {
    grammar: &'g UnitGrammar,
    symbols: Vec<Symbol>,
    classes: Vec<SymbolClass>,
    at: usize,
    units: Vec<Unit>,
}

impl<'g> Cutter<'g> {
    fn new<S: AsRef<str>>(grammar: &'g UnitGrammar, analysis: &[S]) -> Result<Self> {
        let symbols: Vec<Symbol> = analysis.iter().map(|s| Symbol::new(s.as_ref())).collect();
        let mut classes = Vec::with_capacity(symbols.len());
        for (at, symbol) in symbols.iter().enumerate() {
            let class = grammar.classify(symbol);
            match class {
                SymbolClass::Letter | SymbolClass::Tag => classes.push(class),
                SymbolClass::Flag | SymbolClass::Reserved => crate::bail!(
                    FaultyStringInput,
                    format!(
                        "{}: symbol {at}, {symbol:?}, is neither a letter nor a tag",
                        spell(&symbols)
                    )
                ),
            }
        }
        Ok(Cutter {
            grammar,
            symbols,
            classes,
            at: 0,
            units: Vec::new(),
        })
    }

    fn refuse<T>(&self, cause: String) -> Result<T> {
        Err(crate::err!(
            FaultyStringInput,
            format!("{}: {cause}", spell(&self.symbols))
        ))
    }

    fn take(&mut self, kind: UnitKind, end: usize) {
        let symbols = self.symbols[self.at..end].to_vec();
        self.units.push(Unit { kind, symbols });
        self.at = end;
    }

    /// The first place from where the next unit starts that holds a symbol
    /// of `class`, or the end of the analysis.
    fn next(&self, class: SymbolClass) -> usize {
        (self.at..self.symbols.len())
            .find(|&i| self.classes[i] == class)
            .unwrap_or(self.symbols.len())
    }

    // [spec:hfst:sem:prediction-units.prefix]
    fn prefixes(&mut self) -> Result<()> {
        while self.classes.get(self.at) == Some(&SymbolClass::Tag) {
            let tag = &self.symbols[self.at];
            if !self.grammar.is_prefix(tag) {
                return self.refuse(format!(
                    "tag {tag:?} comes before the first stem and matches no prefix pattern"
                ));
            }
            self.take(UnitKind::Prefix, self.at + 1);
        }
        Ok(())
    }

    // [spec:hfst:sem:prediction-units.stem]
    fn stem(&mut self) -> Result<()> {
        let end = self.next(SymbolClass::Tag);
        if end == self.at {
            return self.refuse(format!("no stem starts at symbol {}", self.at));
        }
        self.take(UnitKind::Stem, end);
        Ok(())
    }

    /// After a stem or a boundary unit, a boundary tag before the next letter
    /// ends a boundary unit, and letters after it start a stem. With no
    /// boundary tag left, the rest is an ending and enclitics.
    // [spec:hfst:sem:prediction-units.boundary+3]
    fn after_stem(&mut self) -> Result<()> {
        loop {
            let letter = self.next(SymbolClass::Letter);
            let Some(boundary) =
                (self.at..letter).find(|&i| self.grammar.is_boundary(&self.symbols[i]))
            else {
                return self.ending_and_enclitics();
            };
            self.one_derivation(boundary + 1)?;
            self.take(UnitKind::Boundary, boundary + 1);
            if self.classes.get(self.at) == Some(&SymbolClass::Letter) {
                self.stem()?;
            }
        }
    }

    /// A derivation tag in an ending is refused, since no boundary tag ends
    /// the run. An enclitic may begin with a tag that also names a
    /// derivation, as Greenlandic's `+LI` does, but holds no other.
    // [spec:hfst:sem:prediction-units.ending+1]
    // [spec:hfst:sem:prediction-units.enclitic]
    fn ending_and_enclitics(&mut self) -> Result<()> {
        let letter = self.next(SymbolClass::Letter);
        if let Some(symbol) = self.symbols.get(letter) {
            return self.refuse(format!(
                "letter {symbol:?} at symbol {letter} follows an ending with no boundary tag"
            ));
        }
        let first_enclitic = self.next_enclitic(self.at);
        if let Some(at) = (self.at..first_enclitic).find(|&i| self.is_derivation(i)) {
            return self.refuse(format!(
                "derivation tag {:?} at symbol {at} is in a run that no boundary tag ends",
                self.symbols[at]
            ));
        }
        if first_enclitic > self.at {
            self.take(UnitKind::Ending, first_enclitic);
        }
        while self.at < self.symbols.len() {
            let end = self.next_enclitic(self.at + 1);
            if let Some(at) = (self.at + 1..end).find(|&i| self.is_derivation(i)) {
                return self.refuse(format!(
                    "derivation tag {:?} at symbol {at} is inside an enclitic",
                    self.symbols[at]
                ));
            }
            self.take(UnitKind::Enclitic, end);
        }
        Ok(())
    }

    fn is_derivation(&self, at: usize) -> bool {
        self.grammar.is_derivation(&self.symbols[at])
    }

    /// Refuse a second derivation tag in the run from here to `end`.
    // [spec:hfst:sem:prediction-units.boundary+3]
    fn one_derivation(&self, end: usize) -> Result<()> {
        let mut derivations = (self.at..end).filter(|&at| self.is_derivation(at));
        if let (Some(first), Some(second)) = (derivations.next(), derivations.next()) {
            return self.refuse(format!(
                "derivation tag {:?} at symbol {second} follows {:?} at symbol {first} in one unit",
                self.symbols[second], self.symbols[first]
            ));
        }
        Ok(())
    }

    fn next_enclitic(&self, from: usize) -> usize {
        (from..self.symbols.len())
            .find(|&i| self.grammar.is_enclitic(&self.symbols[i]))
            .unwrap_or(self.symbols.len())
    }
}

fn spell(symbols: &[Symbol]) -> String {
    symbols.iter().map(Symbol::as_str).collect()
}
