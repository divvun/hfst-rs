//! Oqaasileriffik's Greenlandic prediction, emulated. Its patch to
//! hfst-optimized-lookup, on hfst's predict branch, reads each of a list of
//! morpheme names where the input runs out, closes over epsilon and flag
//! arcs, reads each of five Der tags, and prints every pair it can read.
//! This reads a generator's optimized-lookup tables the same way, with code
//! of its own, so the walk can be checked against it.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Mutex;

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_flag_diacritics::{FdOperation, FdState, FdValue};
use hfst::lexc::LexcCompiler;
use hfst::prediction::units::{Unit, UnitGrammar};
use hfst::prediction::walk::{WalkLimits, Walker, Word};
use hfst::transducer::{
    NO_SYMBOL_NUMBER, SymbolNumber, TRANSITION_TARGET_TABLE_START, Transducer,
    TransducerTablesInterface, TransitionTableIndex, indexes_transition_table,
};
use hfst_openfst::StdVectorFst;

/// The Der tags the patched lookup tries after a morpheme name.
pub const TINO_DERS: &[&str] = &["+Der/nn", "+Der/nv", "+Der/vn", "+Der/vv", "+Der/xx"];

/// The morpheme names the patched lookup tries, as its dms_txt lists them.
pub const TINO_MORPHEMES: &str = "
+A +AJAAQ +ALAAQ +ALAAR +ALLAK +ALLAP +ALUK +ALUP +AQ +AQQIK +AR +ARAQ +ARSAAR
+ARSUK +ARSUP +ATAAR +ATSIAQ +GAJUK +GAJUP +GAJUUQ +GALLAR +GALUAQ +GALUAR
+GALUTTUAR +GAR +GASAAQ +GASAAR +GASAP +GASUAR +GASUGE +GE +GI +GIAQ +GIAQAR +GIAR
+GIARTUAAR +GIARTUR +GIIAAQ +GIIAAR +GIIAAT +GIIAK +GIILIP +GIIP +GIIT +GIK +GIP
+GISSAAR +GUMA +GUMAAR +GUMAATAAR +GUMALLIR +GUMINAALLI +GUMINAR +GUNAR +GUNNAIR
+GUNNAR +GUP +GUSUP +GUTE +HTR +IAR +ILATSI +ILIQI +INNAQ +INNAR +INNARIAA +IP +IR
+IRLUIR +IRNIAQ +IRNIAR +IRSI +IRSIR +IRUTE +ISAP +ISUA +ISUR +IT +JA +JAALLU +JAAR
+JURAR +JURTUR +KAA +KAALLAP +KAJAAQ +KAJAAR +KANNIQ +KANNIR +KAR +KASAK +KASIK
+KASIP +KASSAK +KASSAP +KATAAR +KATAK +KATAP +KATTAR +KILLI +KILLIUR +KIP +KISAAR
+KISAR +KKAAQ +KKAAR +KKAJAAQ +KKATAAQ +KKULUK +KKUMINAR +KKUP +KKUQ +KKUT +KKUUR
+KKUUTAAQ +KKUUTAAR +KU +KUJUK +KUJUP +KUJUUQ +KUJUUR +KULA +KULAAR +KULAK +KULLAK
+KULLAP +KULUK +KULUP +KULUUQ +KULUUR +KUQ +KUSUUR +KUTSUUR +LAAQ +LAAR +LI +LIAQ
+LIAR +LIARE +LII +LIK +LIKKAAQ +LIKKIRSAAR +LIP +LIR +LIRI +LIRNGUSAATE +LIRSAAR
+LIRSUR +LIRTUR +LISAAR +LISAQ +LISAR +LIUR +LIUTE +LIVIK +LLAARE +LLAATAA +LLAMMAK
+LLAP +LLAQQIP +LLAR +LLARIP +LLATSIAANNAQ +LLATSIAR +LLATTAA +LLATTAAR +LLATUAR
+LLIQ +LLIR +LLIUR +LLUAR +LLUATAAR +LLUINNAQ +LLUINNAR +LUP +LUSSINNAR +LUUMI +LUUR
+LUUSAAR +MAAR +MI +MIIP +MINAATSIAQ +MINIQ +MINIR +MIP +MISAAR +MIU +MIUQ +MMIP
+MMIRSUR +NAAR +NAQ +NAR +NASURE +NAVIAR +NAVIIR +NAVIIRSAAR +NGA +NGAAQ +NGAAR
+NGAATSIAQ +NGAJAK +NGAJAP +NGAJASSAA +NGIR +NGUSAQ +NGUSAR +NGUSIR +NIAA +NIAALUP
+NIALUP +NIAQ +NIAR +NIARSARE +NIARSUAAR +NIINNAR +NIKU +NIKUQ +NIKUU +NIP +NIQ
+NIQAR +NIQQISAATE +NIQ_AJUR +NIR +NIRAR +NIRLIUR +NIRLUP +NIRPAA +NIRPAAQ
+NIRPAARPAA +NIRSAQ +NIRTUUQ +NIRU +NIRUJUSSUAQ +NIRUMAAQ +NIRUMAAR +NISAQ +NIT
+NIUTAA +NIUTE +NNAAQ +NNAP +NNGA +NNGIR +NNGIRSAA +NNGIRSAAR +NNGISUNGAR +NNGIT
+NNGU +NNGUAKKULUP +NNGUALLAARE +NNGUAQ +NNGUAR +NNGUARAARSUK +NNGUASIK +NNGUATSIAR
+NNGUJUUR +NNGUR +NNGUUJUNNGUAQ +NNI +PAAQ +PAJAAR +PAJUK +PAJUP +PAK +PALAAQ
+PALAAR +PALLAK +PALLAP +PALUK +PALUP +PASIP +PIAQ +PIAR +PILUK +PILUP +PILUUR +QAR
+QATE +QATESIRE +QATIGIIP +QATIGIIT +QATTAAR +QE +QINA +QQA +QQAAQ +QQAAR +QQAJAA
+QQAJAR +QQAMMI +QQAMMIQ +QQAMMIR +QQAR +QQATAR +QQINNAAQ +QQINNAAR +QQIP +QQISSAAQ
+QQISSAAR +QQU +QQURTU +QQURTUSI +QQURTUUQ +QQUSAAR +QQUTE +QQUUR +R +RAAR +RAARSUK
+RAJUK +RAK +RALAAQ +RALAK +RAR +RARE +RATAANNAA +RATAR +RIAA +RIAANNAA +RIAAT +RIAQ
+RIAR +RIATAAR +RIIR +RIP +RLAAQ +RRAK +RRATE +RSUR +RUJUK +RUJUP +RUJUUR +RULLIR
+RULUP +RULUUR +RUR +RUSAAR +RUSAQ +RUTTUR +RUUSAAR +RUUSAQ +SAAR +SAR +SARE +SI
+SIAAQ +SIAQ +SII +SIMA +SIMAAR +SINNAA +SINNAANNGUR +SINNAAQ +SINNAR +SIP +SIQ +SIR
+SIRE +SIRIAR +SIRIIP +SIUR +SIUT +SSA +SSAALIQI +SSAAR +SSAARUTE +SSAASUA +SSAMAAQ
+SSAMAAR +SSAMAARTIP +SSAQ +SSAQQIP +SSI +SSIAQ +SSIARE +SSIP +SSUSIQ +SUAQ +SUAR
+SUATAAR +SUB +SUGE +SUNGAR +SUNNI +SUNNIP +SUNNIT +SUR +SURE +T +TAAQ +TAAR +TALLAP
+TAQ +TAR +TARE +TARIAANNAA +TARIAARUTE +TARIAQ +TARIAQAR +TASSAA +TE +TIGE +TIP
+TIR +TITIR +TSAALI +TSAALIUR +TSAP +TSIALAK +TSIAQ +TSIAR +TSIR +TTUR +TU +TUAQ
+TUAR +TUGAQ +TUJAAR +TUKASSAA +TUQ +TUQAQ +TUR +TURSUU +TURUJUSSUANNGUR +TURUJUSSUU
+TUSI +TUSSAA +TUSSANNGUR +TUU +TUUQ +TUUR +TUUSAAR +TUUSI +U +UMA +UMI +UMISAAR
+UMMI +UMMIR +UNIQ +UR +USAAQ +USAAR +USAP +USAQ +USAR +USIAQ +USIQ +USSUR +UTE
+UTIGE +VALLAAQ +VALLAAR +VIK +VIP +VISSUR +VVAARIK +VVAARIP
";

/// A morpheme name and the Der tag after it.
pub type Pair = (String, String);

/// A configuration with its output left out: a state and its flag values,
/// by their place in a [`FlagTable`].
pub type Point = (TransitionTableIndex, usize);

/// One bit for each morpheme name the emulation tries.
type Names = [u64; 8];

/// Flag value vectors, each kept once and known by its place, and what each
/// flag diacritic makes of each. A word reaches tens of thousands of
/// configurations, so they are not copied from point to point.
struct FlagTable {
    neutral: FdState<SymbolNumber>,
    values: Vec<Vec<FdValue>>,
    places: HashMap<Vec<FdValue>, usize>,
    after: HashMap<(usize, SymbolNumber), Option<usize>>,
}

impl FlagTable {
    fn new(neutral: FdState<SymbolNumber>) -> Self {
        FlagTable {
            neutral,
            values: Vec::new(),
            places: HashMap::new(),
            after: HashMap::new(),
        }
    }

    fn place(&mut self, values: &[FdValue]) -> usize {
        if let Some(&at) = self.places.get(values) {
            return at;
        }
        let at = self.values.len();
        self.values.push(values.to_vec());
        self.places.insert(values.to_vec(), at);
        at
    }

    /// The flags after the flag diacritic `symbol`, whose operation is
    /// `operation`, or none when it fails, as lookup decides it.
    fn after(
        &mut self,
        flags: usize,
        symbol: SymbolNumber,
        operation: &FdOperation,
    ) -> Option<usize> {
        if let Some(&known) = self.after.get(&(flags, symbol)) {
            return known;
        }
        let mut state = self.neutral.clone();
        state.assign_values(&self.values[flags]);
        let after = state
            .apply_operation(operation)
            .then(|| self.place(state.get_values()));
        self.after.insert((flags, symbol), after);
        after
    }
}

/// The patched lookup's prediction, emulated over a generator's tables as
/// hfst-optimized-lookup reads them: from where the input ran out, read a
/// morpheme name, close over epsilon and flag arcs, and read a Der tag.
pub struct Tino<'a, T: TransducerTablesInterface> {
    generator: &'a Transducer<T>,
    flags: RefCell<FlagTable>,
    numbers: HashMap<String, SymbolNumber>,
    pub morphemes: Vec<(SymbolNumber, &'static str)>,
    /// Where each morpheme name is in `morphemes`.
    names: HashMap<SymbolNumber, usize>,
    pub ders: Vec<(SymbolNumber, &'static str)>,
    /// The states from which a Der tag can be read over epsilon and flag
    /// arcs, whatever the flags.
    der_reaching: HashSet<TransitionTableIndex>,
    /// What the start state reaches without reading a symbol.
    start: HashSet<Point>,
}

impl<'a, T: TransducerTablesInterface> Tino<'a, T> {
    /// Only the names the generator's analysis side has are tried.
    pub fn new(generator: &'a Transducer<T>) -> Self {
        let numbers = input_symbols(generator);
        let known = |names: &mut dyn Iterator<Item = &'static str>| {
            names
                .filter_map(|name| Some((*numbers.get(name)?, name)))
                .collect::<Vec<_>>()
        };
        let morphemes = known(&mut TINO_MORPHEMES.split_whitespace());
        assert!(morphemes.len() <= 64 * Names::default().len());
        let names = morphemes
            .iter()
            .enumerate()
            .map(|(at, &(symbol, _))| (symbol, at))
            .collect();
        let ders = known(&mut TINO_DERS.iter().copied());
        let neutral = FdState::new(generator.get_fd_table());
        let mut flags = FlagTable::new(neutral.clone());
        let unset = flags.place(neutral.get_values());
        let mut tino = Tino {
            generator,
            flags: RefCell::new(flags),
            numbers,
            morphemes,
            names,
            ders,
            der_reaching: HashSet::new(),
            start: HashSet::new(),
        };
        tino.der_reaching = tino.der_reaching();
        tino.start = tino.close(vec![(0, unset)]);
        tino
    }

    /// The states from which a Der tag can be read over epsilon and flag
    /// arcs, flags not applied, found backwards from the states that read
    /// one. Every state is the start state or the target of an arc.
    fn der_reaching(&self) -> HashSet<TransitionTableIndex> {
        let t = self.generator;
        let mut states = HashSet::from([0]);
        for arc in 0..t.get_header().target_table_size() {
            if t.get_transition_input(arc) != NO_SYMBOL_NUMBER {
                states.insert(t.get_transition_target(arc));
            }
        }
        let epsilon = |symbol: SymbolNumber| symbol == 0 || self.is_flag(symbol);
        let mut sources: HashMap<TransitionTableIndex, Vec<TransitionTableIndex>> = HashMap::new();
        let mut agenda = Vec::new();
        for &state in &states {
            for arc in self.arcs(state, 0, &epsilon) {
                sources
                    .entry(t.get_transition_target(arc))
                    .or_default()
                    .push(state);
            }
            if self.ders.iter().any(|&(der, _)| self.reads(state, der)) {
                agenda.push(state);
            }
        }
        let mut reaching = HashSet::new();
        while let Some(state) = agenda.pop() {
            if reaching.insert(state) {
                agenda.extend(sources.get(&state).into_iter().flatten());
            }
        }
        reaching
    }

    fn is_flag(&self, symbol: SymbolNumber) -> bool {
        symbol != NO_SYMBOL_NUMBER
            && self
                .generator
                .get_alphabet()
                .get_operation(symbol)
                .is_some()
    }

    /// The arcs from `state` whose input `wanted` takes. A state kept in the
    /// transition table alone lists its arcs after its own entry. An index
    /// state keeps the arcs for a symbol under the symbol's slot, and its
    /// epsilon and flag arcs under slot 0.
    fn arcs(
        &self,
        state: TransitionTableIndex,
        slot: SymbolNumber,
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> Vec<TransitionTableIndex> {
        let t = self.generator;
        let mut arcs = Vec::new();
        if indexes_transition_table(state) {
            let mut arc = state - TRANSITION_TARGET_TABLE_START + 1;
            while t.get_transition_input(arc) != NO_SYMBOL_NUMBER {
                if wanted(t.get_transition_input(arc)) {
                    arcs.push(arc);
                }
                arc += 1;
            }
            return arcs;
        }
        let index = state + 1 + TransitionTableIndex::from(slot);
        if t.get_index_input(index) != slot {
            return arcs;
        }
        let Some(mut arc) = t
            .get_index_target(index)
            .checked_sub(TRANSITION_TARGET_TABLE_START)
        else {
            return arcs;
        };
        while wanted(t.get_transition_input(arc)) {
            arcs.push(arc);
            arc += 1;
        }
        arcs
    }

    /// Where the epsilon and flag arcs from `point` lead, a flag arc taken
    /// only when its operation holds.
    fn epsilon_step(&self, (state, flags): Point) -> Vec<Point> {
        let t = self.generator;
        let epsilon = |symbol: SymbolNumber| symbol == 0 || self.is_flag(symbol);
        self.arcs(state, 0, &epsilon)
            .into_iter()
            .filter_map(|arc| {
                let input = t.get_transition_input(arc);
                let after = match t.get_alphabet().get_operation(input) {
                    None => flags,
                    Some(operation) => self.flags.borrow_mut().after(flags, input, operation)?,
                };
                Some((t.get_transition_target(arc), after))
            })
            .collect()
    }

    /// Every point `points` reach over epsilon and flag arcs.
    fn close(&self, points: Vec<Point>) -> HashSet<Point> {
        let mut seen = HashSet::new();
        let mut agenda = points;
        while let Some(point) = agenda.pop() {
            if seen.insert(point) {
                agenda.extend(self.epsilon_step(point));
            }
        }
        seen
    }

    /// Where the arcs reading `symbol` from `points` lead.
    fn read(&self, points: &HashSet<Point>, symbol: SymbolNumber) -> Vec<Point> {
        let t = self.generator;
        let mut reached = Vec::new();
        for &(state, flags) in points {
            for arc in self.arcs(state, symbol, &|input| input == symbol) {
                reached.push((t.get_transition_target(arc), flags));
            }
        }
        reached
    }

    fn reads(&self, state: TransitionTableIndex, symbol: SymbolNumber) -> bool {
        !self
            .arcs(state, symbol, &|input| input == symbol)
            .is_empty()
    }

    /// The points lookup reaches over an analysis prefix from the start
    /// state, as the patched lookup reads what was typed.
    pub fn spell(&self, symbols: &[String]) -> HashSet<Point> {
        let mut points = self.start.clone();
        for name in symbols {
            let Some(&symbol) = self.numbers.get(name) else {
                return HashSet::new();
            };
            points = self.close(self.read(&points, symbol));
        }
        points
    }

    /// The word's configurations with their outputs left out.
    pub fn points<W: TransducerTablesInterface>(
        &self,
        walker: &Walker<'_, W>,
        word: &Word,
    ) -> HashSet<Point> {
        let mut flags = self.flags.borrow_mut();
        walker
            .configurations(word)
            .iter()
            .map(|config| (config.state(), flags.place(config.flag_values())))
            .collect()
    }

    /// Where reading each morpheme name from `points` leads, in the order
    /// of `morphemes`. Each point is visited once: an index state is probed
    /// at each name's slot, and a state kept in the transition table is
    /// scanned once.
    fn after_names(&self, points: &HashSet<Point>) -> Vec<Vec<Point>> {
        let t = self.generator;
        let named = |input: SymbolNumber| self.names.contains_key(&input);
        let mut reached = vec![Vec::new(); self.morphemes.len()];
        for &(state, flags) in points {
            let arcs = if indexes_transition_table(state) {
                self.arcs(state, 0, &named)
            } else {
                self.morphemes
                    .iter()
                    .filter(|&&(m, _)| {
                        t.get_index_input(state + 1 + TransitionTableIndex::from(m)) == m
                    })
                    .flat_map(|&(m, _)| self.arcs(state, m, &|input| input == m))
                    .collect()
            };
            for arc in arcs {
                let at = self.names[&t.get_transition_input(arc)];
                reached[at].push((t.get_transition_target(arc), flags));
            }
        }
        reached
    }

    /// Every point from which a Der tag may be read after a morpheme name
    /// and the closure after it, with the names that reach it. The names
    /// are carried over one closure together, since a closure after a name
    /// can hold a million points, and only states a Der tag can follow are
    /// walked.
    fn carry(&self, points: &HashSet<Point>) -> HashMap<Point, Names> {
        let mut carried: HashMap<Point, Names> = HashMap::new();
        let mut agenda = Vec::new();
        let wanted = |point: &Point| self.der_reaching.contains(&point.0);
        for (at, reached) in self.after_names(points).into_iter().enumerate() {
            for point in reached.into_iter().filter(wanted) {
                let names = carried.entry(point).or_default();
                let bit = 1u64 << (at % 64);
                if names[at / 64] & bit == 0 {
                    names[at / 64] |= bit;
                    agenda.push(point);
                }
            }
        }
        while let Some(point) = agenda.pop() {
            let names = carried[&point];
            for next in self.epsilon_step(point).into_iter().filter(wanted) {
                let known = carried.entry(next).or_default();
                let merged: Names = std::array::from_fn(|i| known[i] | names[i]);
                if merged != *known {
                    *known = merged;
                    agenda.push(next);
                }
            }
        }
        carried
    }

    /// Every (morpheme name, Der tag) pair the patched lookup prints from
    /// `points`.
    pub fn pairs(&self, points: &HashSet<Point>) -> BTreeSet<Pair> {
        let mut found = BTreeSet::new();
        for (&(state, _), names) in &self.carry(points) {
            for &(der, der_name) in &self.ders {
                if !self.reads(state, der) {
                    continue;
                }
                for (at, &(_, name)) in self.morphemes.iter().enumerate() {
                    if names[at / 64] & (1u64 << (at % 64)) != 0 {
                        found.insert((name.to_owned(), der_name.to_owned()));
                    }
                }
            }
        }
        found
    }

    fn is_morpheme(&self, name: &str) -> bool {
        self.morphemes.iter().any(|&(_, known)| known == name)
    }

    pub fn is_der(&self, name: &str) -> bool {
        self.ders.iter().any(|&(_, known)| known == name)
    }

    /// The (morpheme name, Der tag) pair a boundary unit holds, when it
    /// holds one of the names the patched lookup tries.
    pub fn pair_of(&self, unit: &Unit) -> Option<Pair> {
        let der = unit.symbols.last()?;
        let morpheme = unit.symbols.iter().find(|s| self.is_morpheme(s))?;
        Some((morpheme.to_string(), der.to_string()))
    }
}

pub fn input_symbols<T: TransducerTablesInterface>(
    generator: &Transducer<T>,
) -> HashMap<String, SymbolNumber> {
    let count = generator.get_header().input_symbol_count();
    let table = generator.get_symbol_table();
    (1..count)
        .filter_map(|symbol| Some((table.get(symbol as usize)?.to_string(), symbol)))
        .collect()
}

/// Greenlandic in small. After the stem, +NIQ reaches its Der tag over a
/// flag that holds and +SSAQ over one that fails; +KIP reads its Der tag at
/// once; a hidden tag comes before +TAQ and between +LI and its Der tag.
const SMALL: &str = r#"
Multichar_Symbols
+V +N +Abs +Sg +Ind +3Sg +NIQ +KIP +TAQ +LI +SSAQ +Der/vn +Der/nv +Der/vv +Gram/IV
@U.cls.v@ @U.cls.n@

LEXICON Root
@U.cls.v@ Stem ;

LEXICON Stem
oqar Verb ;

LEXICON Verb
+V+Ind+3Sg:poq # ;
+NIQ:niq Nominal ;
+KIP+Der/nv:kip Verb ;
+Gram/IV+TAQ+Der/vn:taq Noun ;
+LI+Gram/IV+Der/vv:li Verb ;
+SSAQ:ssaq Clash ;

LEXICON Nominal
@U.cls.v@ VerbToNoun ;

LEXICON Clash
@U.cls.n@ VerbToNoun ;

LEXICON VerbToNoun
+Der/vn Noun ;

LEXICON Noun
+N+Abs+Sg:0 # ;
"#;

const SMALL_GRAMMAR: &str = r#"
boundary = ["+Der/*"]
derivation = ["+NIQ", "+KIP", "+TAQ", "+LI", "+SSAQ"]
hidden = ["+Gram/*"]
"#;

pub fn pairs(list: &[(&str, &str)]) -> BTreeSet<Pair> {
    list.iter()
        .map(|&(m, d)| (m.to_owned(), d.to_owned()))
        .collect()
}

/// Compiling writes the tropical backend's process-wide symbol table, so
/// tests that share a process take turns.
static COMPILING: Mutex<()> = Mutex::new(());

pub fn small() -> (Transducer, UnitGrammar) {
    let _turn = COMPILING.lock().unwrap_or_else(|e| e.into_inner());
    let compiled = LexcCompiler::<StdVectorFst>::new()
        .compile(SMALL)
        .expect("the lexicon compiles");
    let generator =
        ConversionFunctions::hfst_transducer_to_hfst_ol(&compiled).expect("the lexicon converts");
    let grammar = UnitGrammar::from_toml(SMALL_GRAMMAR.as_bytes()).expect("the grammar reads");
    (generator, grammar)
}

pub fn after_stem<T: TransducerTablesInterface>(walker: &Walker<'_, T>) -> Word {
    let start = walker.start();
    let entered = walker.entry(&start, "oqar", None).expect("a stem starts");
    let stem = entered
        .candidates
        .iter()
        .find(|c| c.key.as_str() == "oqar")
        .expect("the stem is offered");
    start.pick(stem)
}

#[test]
fn emulation_reads_a_name_then_a_der_tag() {
    let (generator, grammar) = small();
    let walker = Walker::new(&generator, &grammar, WalkLimits::default()).expect("it fits");
    let tino = Tino::new(&generator);
    assert_eq!(tino.ders.len(), 3);
    assert_eq!(tino.morphemes.len(), 5);
    let word = after_stem(&walker);
    let expected = pairs(&[("+KIP", "+Der/nv"), ("+NIQ", "+Der/vn")]);
    assert_eq!(tino.pairs(&tino.points(&walker, &word)), expected);
    let spelled: Vec<String> = "oqar".chars().map(String::from).collect();
    assert_eq!(tino.pairs(&tino.spell(&spelled)), expected);
}
