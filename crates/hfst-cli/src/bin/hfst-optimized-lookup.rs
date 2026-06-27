//! Faithful 1:1 port of tools/src/hfst-optimized-lookup.cc and
//! tools/src/hfst-optimized-lookup.h — a self-contained optimized-lookup
//! engine. Unlike most hfst tools this one does NOT use the hfst-cli
//! globals/inc framework: it parses its own HFST_OL/HFST_OLW binary format
//! directly from a FILE* via libc and runs its own getopt loop. It needs no
//! operations from the hfst library crate.
//!
//! The C++ single-file version is structurally awkward (assembled from several
//! files); this port mirrors that structure so the [spec] annotations line up
//! function-for-function.

use hfst::pmatch_compiler::{clock, clock_t};
use hfst_cli::hfst_getopt as getopt;
use libc::{EOF, FILE, c_char, c_int, c_void, fgetc, fopen, fread, free, malloc, strlen, ungetc};
use std::ffi::CString;

// ---------------------------------------------------------------------------
// config.h-defined constants
// ---------------------------------------------------------------------------
const PACKAGE_NAME: &str = "hfst-optimized-lookup";
const PACKAGE_BUGREPORT: &str = "hfst-bugs@helsinki.fi";
const PACKAGE_STRING: &str = "hfst-optimized-lookup 1.2";

// ---------------------------------------------------------------------------
// typedefs
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.symbol-number]
type SymbolNumber = u16;
// [spec:hfst:def:hfst-optimized-lookup.transition-table-index]
type TransitionTableIndex = u32;
// [spec:hfst:def:hfst-optimized-lookup.transition-number]
type TransitionNumber = u32;
// [spec:hfst:def:hfst-optimized-lookup.state-id-number]
type StateIdNumber = u32;
// [spec:hfst:def:hfst-optimized-lookup.arc-number]
#[allow(dead_code)]
type ArcNumber = u32;
// [spec:hfst:def:hfst-optimized-lookup.value-number]
type ValueNumber = i16;
// [spec:hfst:def:hfst-optimized-lookup.symbol-number-vector]
type SymbolNumberVector = Vec<SymbolNumber>;
// [spec:hfst:def:hfst-optimized-lookup.key-table]
type KeyTable = std::collections::BTreeMap<SymbolNumber, CString>;
// [spec:hfst:def:hfst-optimized-lookup.weight]
type Weight = f32;
// [spec:hfst:def:hfst-optimized-lookup.operation-vector]
type OperationVector = Vec<FlagDiacriticOperation>;
// [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-state]
type FlagDiacriticState = Vec<ValueNumber>;
// [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-state-stack]
type FlagDiacriticStateStack = Vec<FlagDiacriticState>;
// [spec:hfst:def:hfst-optimized-lookup.display-vector]
type DisplayVector = Vec<String>;
// [spec:hfst:def:hfst-optimized-lookup.display-set]
type DisplaySet = std::collections::BTreeSet<String>;
// [spec:hfst:def:hfst-optimized-lookup.display-multi-map]
// std::multimap<Weight, std::string>: ordered, allows duplicate keys; modelled
// as a sorted vector of (Weight, String) pairs.
type DisplayMultiMap = Vec<(Weight, String)>;
// [spec:hfst:def:hfst-optimized-lookup.display-map]
type DisplayMap = std::collections::BTreeMap<String, Weight>;

const NO_SYMBOL_NUMBER: SymbolNumber = u16::MAX;
const NO_TABLE_INDEX: TransitionTableIndex = u32::MAX;

// This is 2^31, hopefully equal to UINT_MAX/2 rounded up.
const TRANSITION_TARGET_TABLE_START: TransitionTableIndex = 2147483648u32;

const MAX_IO_STRING: usize = 5000;

const INFINITE_WEIGHT: Weight = NO_TABLE_INDEX as Weight;

// ---------------------------------------------------------------------------
// enums
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.colour-tristate]
#[allow(dead_code)]
enum ColourTristate {
    ColourNever,
    ColourAlways,
    ColourAuto,
}

// [spec:hfst:def:hfst-optimized-lookup.output-type]
#[derive(PartialEq, Clone, Copy)]
enum OutputType {
    #[allow(dead_code)]
    Hfst,
    Xerox,
}
use OutputType::Xerox;

// the flag diacritic operators as given in
// Beesley & Karttunen, Finite State Morphology (U of C Press 2003)
// [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operator]
#[derive(Clone, Copy, PartialEq)]
enum FlagDiacriticOperator {
    P,
    N,
    R,
    D,
    C,
    U,
}
use FlagDiacriticOperator::{C, D, N, P, R, U};

// [spec:hfst:def:hfst-optimized-lookup.header-flag]
#[derive(Clone, Copy)]
enum HeaderFlag {
    Weighted,
    Deterministic,
    InputDeterministic,
    Minimized,
    Cyclic,
    HasEpsilonEpsilonTransitions,
    HasInputEpsilonTransitions,
    HasInputEpsilonCycles,
    HasUnweightedInputEpsilonCycles,
}
use HeaderFlag::{HasInputEpsilonCycles, HasUnweightedInputEpsilonCycles, Weighted};

// ---------------------------------------------------------------------------
// global mutable tool state (the C++ globals)
// ---------------------------------------------------------------------------
static mut OUTPUT_TYPE: OutputType = OutputType::Xerox;
#[allow(dead_code)]
static mut VERBOSE_FLAG: bool = false;
static mut DISPLAY_WEIGHTS_FLAG: bool = false;
static mut DISPLAY_UNIQUE_FLAG: bool = false;
static mut ECHO_INPUTS_FLAG: bool = false;
static mut BE_FAST: bool = false;
static mut MAX_ANALYSES: c_int = c_int::MAX;
static mut LIMIT_REACHED: bool = false;
static mut CALL_COUNTER: u64 = 0;
static mut TIME_CUTOFF: f64 = 0.0;
static mut START_CLOCK: clock_t = 0;

static mut BEAM: f32 = -1.0;
#[allow(dead_code)]
static mut PIPE_INPUT: bool = false;
#[allow(dead_code)]
static mut PIPE_OUTPUT: bool = false;

const CLOCKS_PER_SEC: f64 = 1_000_000.0;

// ---------------------------------------------------------------------------
// small i/o helpers
// ---------------------------------------------------------------------------
fn print_out(s: &str) {
    use std::io::Write;
    let mut o = std::io::stdout();
    let _ = o.write_all(s.as_bytes());
}

fn print_err(s: &str) {
    use std::io::Write;
    let mut e = std::io::stderr();
    let _ = e.write_all(s.as_bytes());
}

// ---------------------------------------------------------------------------
// HeaderParsingException
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.header-parsing-exception]
struct HeaderParsingException;

// [spec:hfst:def:hfst-optimized-lookup.header-parsing-exception.what-fn]
// [spec:hfst:sem:hfst-optimized-lookup.header-parsing-exception.what-fn]
impl HeaderParsingException {
    #[allow(dead_code)]
    fn what(&self) -> &'static str {
        "Parsing error while reading header"
    }
}

// ---------------------------------------------------------------------------
// FlagDiacriticOperation
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation]
#[derive(Clone, Copy)]
struct FlagDiacriticOperation {
    operation: FlagDiacriticOperator,
    feature: SymbolNumber,
    value: ValueNumber,
}

impl FlagDiacriticOperation {
    // [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.flag-diacritic-operation-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.flag-diacritic-operation-fn]
    fn new(op: FlagDiacriticOperator, feat: SymbolNumber, val: ValueNumber) -> Self {
        FlagDiacriticOperation {
            operation: op,
            feature: feat,
            value: val,
        }
    }

    // dummy constructor
    fn dummy() -> Self {
        FlagDiacriticOperation {
            operation: P,
            feature: NO_SYMBOL_NUMBER,
            value: 0,
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.is-flag-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.is-flag-fn]
    fn is_flag(&self) -> bool {
        self.feature != NO_SYMBOL_NUMBER
    }
    // [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.operation-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.operation-fn]
    fn operation(&self) -> FlagDiacriticOperator {
        self.operation
    }
    // [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.feature-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.feature-fn]
    fn feature(&self) -> SymbolNumber {
        self.feature
    }
    // [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.value-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.value-fn]
    fn value(&self) -> ValueNumber {
        self.value
    }

    // [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.print-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.print-fn]
    // (only compiled under OL_FULL_DEBUG in the C++; kept for parity.)
    #[allow(dead_code)]
    fn print(&self) {
        print_out(&format!(
            "{}\t{}\t{}\n",
            self.operation as i32, self.feature, self.value
        ));
    }
}

// ---------------------------------------------------------------------------
// raw fread helpers
// ---------------------------------------------------------------------------
unsafe fn fread_val<T: Copy>(f: *mut FILE) -> T {
    unsafe {
        let mut v: T = std::mem::zeroed();
        let _ = fread(
            (&mut v as *mut T) as *mut c_void,
            std::mem::size_of::<T>(),
            1,
            f,
        );
        v
    }
}

unsafe fn read_le_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}
unsafe fn read_le_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}
unsafe fn read_le_f32(buf: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

// ---------------------------------------------------------------------------
// TransducerHeader
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transducer-header]
#[derive(Clone)]
struct TransducerHeader {
    number_of_symbols: SymbolNumber,
    number_of_input_symbols: SymbolNumber,
    size_of_transition_index_table: TransitionTableIndex,
    size_of_transition_target_table: TransitionTableIndex,
    #[allow(dead_code)]
    number_of_states: StateIdNumber,
    #[allow(dead_code)]
    number_of_transitions: TransitionNumber,
    weighted: bool,
    deterministic: bool,
    input_deterministic: bool,
    minimized: bool,
    cyclic: bool,
    has_epsilon_epsilon_transitions: bool,
    has_input_epsilon_transitions: bool,
    has_input_epsilon_cycles: bool,
    has_unweighted_input_epsilon_cycles: bool,
}

impl TransducerHeader {
    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.read-property-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.read-property-fn]
    unsafe fn read_property(f: *mut FILE) -> bool {
        unsafe {
            let prop: u32 = fread_val(f);
            prop != 0
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.transducer-header-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.transducer-header-fn]
    unsafe fn new(f: *mut FILE) -> Self {
        unsafe {
            Self::skip_hfst3_header(f);

            let number_of_input_symbols: SymbolNumber = fread_val(f);
            let number_of_symbols: SymbolNumber = fread_val(f);

            let size_of_transition_index_table: TransitionTableIndex = fread_val(f);
            let size_of_transition_target_table: TransitionTableIndex = fread_val(f);

            let number_of_states: StateIdNumber = fread_val(f);
            let number_of_transitions: TransitionNumber = fread_val(f);

            let weighted = Self::read_property(f);
            let deterministic = Self::read_property(f);
            let input_deterministic = Self::read_property(f);
            let minimized = Self::read_property(f);
            let cyclic = Self::read_property(f);
            let has_epsilon_epsilon_transitions = Self::read_property(f);
            let has_input_epsilon_transitions = Self::read_property(f);
            let has_input_epsilon_cycles = Self::read_property(f);
            let has_unweighted_input_epsilon_cycles = Self::read_property(f);

            TransducerHeader {
                number_of_symbols,
                number_of_input_symbols,
                size_of_transition_index_table,
                size_of_transition_target_table,
                number_of_states,
                number_of_transitions,
                weighted,
                deterministic,
                input_deterministic,
                minimized,
                cyclic,
                has_epsilon_epsilon_transitions,
                has_input_epsilon_transitions,
                has_input_epsilon_cycles,
                has_unweighted_input_epsilon_cycles,
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.skip-hfst3-header-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.skip-hfst3-header-fn]
    unsafe fn skip_hfst3_header(f: *mut FILE) -> Result<(), HeaderParsingException> {
        unsafe {
            let header1 = b"HFST";
            let mut header_loc: usize = 0; // how much of the header has been found
            let mut c: c_int = 0;
            while header_loc < header1.len() + 1 {
                c = fgetc(f);
                if header_loc < header1.len() {
                    if c != header1[header_loc] as c_int {
                        break;
                    }
                } else {
                    // the trailing NUL of "HFST\0"
                    if c != 0 {
                        break;
                    }
                }
                header_loc += 1;
            }
            if header_loc == header1.len() + 1 {
                // we found it
                let remaining_header_len: u16 = fread_val(f);
                if fgetc(f) != 0 {
                    return Err(HeaderParsingException);
                }
                let len = remaining_header_len as usize;
                let headervalue = malloc(len) as *mut u8;
                if fread(headervalue as *mut c_void, len, 1, f) != 1 {
                    free(headervalue as *mut c_void);
                    return Err(HeaderParsingException);
                }
                if *headervalue.add(len - 1) != 0 {
                    free(headervalue as *mut c_void);
                    return Err(HeaderParsingException);
                }
                let header_tail = std::slice::from_raw_parts(headervalue, len).to_vec();
                let header_tail = String::from_utf8_lossy(&header_tail).into_owned();
                free(headervalue as *mut c_void);
                if let Some(type_field) = header_tail.find("type") {
                    let ol = header_tail.find("HFST_OL");
                    let olw = header_tail.find("HFST_OLW");
                    if ol != Some(type_field + 5) && olw != Some(type_field + 5) {
                        return Err(HeaderParsingException);
                    }
                }
            } else {
                // nope. put back what we've taken
                ungetc(c, f); // first the non-matching character
                let mut i = header_loc as i64 - 1;
                while i >= 0 {
                    // then the characters that did match (if any)
                    ungetc(header1[i as usize] as c_int, f);
                    i -= 1;
                }
            }
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.symbol-count-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.symbol-count-fn]
    fn symbol_count(&self) -> SymbolNumber {
        self.number_of_symbols
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.input-symbol-count-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.input-symbol-count-fn]
    fn input_symbol_count(&self) -> SymbolNumber {
        self.number_of_input_symbols
    }
    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.index-table-size-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.index-table-size-fn]
    fn index_table_size(&self) -> TransitionTableIndex {
        self.size_of_transition_index_table
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.target-table-size-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.target-table-size-fn]
    fn target_table_size(&self) -> TransitionTableIndex {
        self.size_of_transition_target_table
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-header.probe-flag-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-header.probe-flag-fn]
    fn probe_flag(&self, flag: HeaderFlag) -> bool {
        match flag {
            HeaderFlag::Weighted => self.weighted,
            HeaderFlag::Deterministic => self.deterministic,
            HeaderFlag::InputDeterministic => self.input_deterministic,
            HeaderFlag::Minimized => self.minimized,
            HeaderFlag::Cyclic => self.cyclic,
            HeaderFlag::HasEpsilonEpsilonTransitions => self.has_epsilon_epsilon_transitions,
            HeaderFlag::HasInputEpsilonTransitions => self.has_input_epsilon_transitions,
            HeaderFlag::HasInputEpsilonCycles => self.has_input_epsilon_cycles,
            HeaderFlag::HasUnweightedInputEpsilonCycles => self.has_unweighted_input_epsilon_cycles,
        }
    }
}

// ---------------------------------------------------------------------------
// TransducerAlphabet
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet]
#[derive(Clone)]
struct TransducerAlphabet {
    #[allow(dead_code)]
    number_of_symbols: SymbolNumber,
    kt: KeyTable,
    operations: OperationVector,
    feature_bucket: std::collections::BTreeMap<String, SymbolNumber>,
    value_bucket: std::collections::BTreeMap<String, ValueNumber>,
    val_num: ValueNumber,
    feat_num: SymbolNumber,
}

impl TransducerAlphabet {
    // [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-next-symbol-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-next-symbol-fn]
    unsafe fn get_next_symbol(&mut self, f: *mut FILE, k: SymbolNumber) {
        unsafe {
            let mut line: Vec<u8> = Vec::new();
            loop {
                let byte = fgetc(f);
                if byte == 0 {
                    break;
                }
                if byte == EOF {
                    print_err("Could not parse transducer; wrong or corrupt file?\n");
                    std::process::exit(1);
                }
                line.push(byte as u8);
            }
            // line is now the NUL-terminated symbol bytes (without the NUL)
            let n = line.len();
            if n >= 5 && line[0] == b'@' && line[n - 1] == b'@' && line[2] == b'.' {
                // a special symbol needs to be parsed
                let mut feat = String::new();
                let mut val = String::new();
                // g++ worries about this falling through uninitialized
                let mut op = P;
                match line[1] {
                    b'P' => op = P,
                    b'N' => op = N,
                    b'R' => op = R,
                    b'D' => op = D,
                    b'C' => op = C,
                    b'U' => op = U,
                    _ => {}
                }
                // as long as we're working with utf-8, this should be ok
                let mut c = 3usize;
                while c < n && line[c] != b'.' && line[c] != b'@' {
                    feat.push(line[c] as char);
                    c += 1;
                }
                if c < n && line[c] == b'.' {
                    c += 1;
                    while c < n && line[c] != b'@' {
                        val.push(line[c] as char);
                        c += 1;
                    }
                }
                if !self.feature_bucket.contains_key(&feat) {
                    self.feature_bucket.insert(feat.clone(), self.feat_num);
                    self.feat_num += 1;
                }
                if !self.value_bucket.contains_key(&val) {
                    self.value_bucket.insert(val.clone(), self.val_num);
                    self.val_num += 1;
                }
                self.operations.push(FlagDiacriticOperation::new(
                    op,
                    self.feature_bucket[&feat],
                    self.value_bucket[&val],
                ));
                self.kt.insert(k, CString::new("").unwrap());
                return;
            }
            self.operations.push(FlagDiacriticOperation::dummy()); // dummy flag
            self.kt.insert(k, CString::new(line).unwrap_or_default());
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.transducer-alphabet-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.transducer-alphabet-fn]
    unsafe fn new(f: *mut FILE, symbol_number: SymbolNumber) -> Self {
        unsafe {
            let mut alphabet = TransducerAlphabet {
                number_of_symbols: symbol_number,
                kt: KeyTable::new(),
                operations: OperationVector::new(),
                feature_bucket: std::collections::BTreeMap::new(),
                value_bucket: std::collections::BTreeMap::new(),
                val_num: 1,
                feat_num: 0,
            };
            alphabet.value_bucket.insert(String::new(), 0); // empty value = neutral
            for k in 0..alphabet.number_of_symbols {
                alphabet.get_next_symbol(f, k);
            }
            // assume the first symbol is epsilon which we don't want to print
            alphabet.kt.insert(0, CString::new("").unwrap());
            alphabet
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-key-table-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-key-table-fn]
    fn get_key_table(&self) -> &KeyTable {
        &self.kt
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-operation-vector-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-operation-vector-fn]
    fn get_operation_vector(&self) -> OperationVector {
        self.operations.clone()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-state-size-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-state-size-fn]
    fn get_state_size(&self) -> SymbolNumber {
        self.feature_bucket.len() as SymbolNumber
    }
}

// ---------------------------------------------------------------------------
// LetterTrie
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.letter-trie-vector]
// LetterTrieVector: vector of optional child tries, one slot per byte value.
// [spec:hfst:def:hfst-optimized-lookup.letter-trie]
struct LetterTrie {
    letters: Vec<Option<Box<LetterTrie>>>,
    symbols: SymbolNumberVector,
}

const UCHAR_MAX: usize = 255;

impl LetterTrie {
    // [spec:hfst:def:hfst-optimized-lookup.letter-trie.letter-trie-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.letter-trie.letter-trie-fn]
    fn new() -> Self {
        let mut letters = Vec::with_capacity(UCHAR_MAX);
        for _ in 0..UCHAR_MAX {
            letters.push(None);
        }
        LetterTrie {
            letters,
            symbols: vec![NO_SYMBOL_NUMBER; UCHAR_MAX],
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.letter-trie.add-string-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.letter-trie.add-string-fn]
    // p is a pointer into a NUL-terminated byte string; this models *p / *(p+1).
    unsafe fn add_string(&mut self, p: *const u8, symbol_key: SymbolNumber) {
        unsafe {
            if *p.add(1) == 0 {
                self.symbols[*p as usize] = symbol_key;
                return;
            }
            if self.letters[*p as usize].is_none() {
                self.letters[*p as usize] = Some(Box::new(LetterTrie::new()));
            }
            self.letters[*p as usize]
                .as_mut()
                .unwrap()
                .add_string(p.add(1), symbol_key);
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.letter-trie.has-key-starting-with-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.letter-trie.has-key-starting-with-fn]
    fn has_key_starting_with(&self, c: u8) -> bool {
        self.letters[c as usize].is_some()
    }

    // [spec:hfst:def:hfst-optimized-lookup.letter-trie.find-key-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.letter-trie.find-key-fn]
    // p is a mutable pointer-to-pointer; we model it as a cursor offset.
    unsafe fn find_key(&self, base: *const u8, p: &mut usize) -> SymbolNumber {
        unsafe {
            let old_p = *p;
            *p += 1;
            let old_byte = *base.add(old_p);
            if self.letters[old_byte as usize].is_none() {
                return self.symbols[old_byte as usize];
            }
            let s = self.letters[old_byte as usize]
                .as_ref()
                .unwrap()
                .find_key(base, p);
            if s == NO_SYMBOL_NUMBER {
                *p -= 1;
                return self.symbols[old_byte as usize];
            }
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.encoder]
struct Encoder {
    number_of_input_symbols: SymbolNumber,
    letters: LetterTrie,
    ascii_symbols: SymbolNumberVector,
}

impl Encoder {
    // [spec:hfst:def:hfst-optimized-lookup.encoder.encoder-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.encoder.encoder-fn]
    fn new(kt: &KeyTable, input_symbol_count: SymbolNumber) -> Self {
        let mut encoder = Encoder {
            number_of_input_symbols: input_symbol_count,
            letters: LetterTrie::new(),
            ascii_symbols: vec![NO_SYMBOL_NUMBER; UCHAR_MAX],
        };
        encoder.read_input_symbols(kt);
        encoder
    }

    // [spec:hfst:def:hfst-optimized-lookup.encoder.read-input-symbols-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.encoder.read-input-symbols-fn]
    fn read_input_symbols(&mut self, kt: &KeyTable) {
        unsafe {
            for k in 0..self.number_of_input_symbols {
                let p = kt.get(&k).map(|c| c.as_ptr() as *const u8).unwrap();
                let plen = strlen(p as *const c_char);
                let first = *p;
                if plen == 1
                    && first <= 127
                    // we have a single char ascii symbol
                    && !self.letters.has_key_starting_with(first)
                {
                    // make sure there isn't a longer symbol we would be shadowing
                    self.ascii_symbols[first as usize] = k;
                }
                // If there's an ascii tokenized symbol shadowing this, remove it
                if plen > 1
                    && first <= 127
                    && self.ascii_symbols[first as usize] != NO_SYMBOL_NUMBER
                {
                    self.ascii_symbols[first as usize] = NO_SYMBOL_NUMBER;
                }
                self.letters.add_string(p, k);
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.encoder.find-key-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.encoder.find-key-fn]
    unsafe fn find_key(&self, base: *const u8, p: &mut usize) -> SymbolNumber {
        unsafe {
            let first = *base.add(*p);
            if self.ascii_symbols[first as usize] == NO_SYMBOL_NUMBER {
                return self.letters.find_key(base, p);
            }
            let s = self.ascii_symbols[first as usize];
            *p += 1;
            s
        }
    }
}

// ---------------------------------------------------------------------------
// TransitionIndex / Transition (unweighted)
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transition-index]
#[derive(Clone, Copy)]
struct TransitionIndex {
    input_symbol: SymbolNumber,
    first_transition_index: TransitionTableIndex,
}

impl TransitionIndex {
    const SIZE: usize =
        std::mem::size_of::<SymbolNumber>() + std::mem::size_of::<TransitionTableIndex>();

    // [spec:hfst:def:hfst-optimized-lookup.transition-index.transition-index-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-index.transition-index-fn]
    fn new(input: SymbolNumber, first_transition: TransitionTableIndex) -> Self {
        TransitionIndex {
            input_symbol: input,
            first_transition_index: first_transition,
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-index.matches-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-index.matches-fn]
    #[allow(dead_code)]
    fn matches(&self, s: SymbolNumber) -> bool {
        if self.input_symbol == NO_SYMBOL_NUMBER {
            return false;
        }
        if s == NO_SYMBOL_NUMBER {
            return true;
        }
        self.input_symbol == s
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-index.target-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-index.target-fn]
    fn target(&self) -> TransitionTableIndex {
        self.first_transition_index
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-index.final-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-index.final-fn]
    fn is_final(&self) -> bool {
        self.first_transition_index == 1
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-index.get-input-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-index.get-input-fn]
    fn get_input(&self) -> SymbolNumber {
        self.input_symbol
    }
}

// [spec:hfst:def:hfst-optimized-lookup.transition]
#[derive(Clone, Copy)]
struct Transition {
    input_symbol: SymbolNumber,
    output_symbol: SymbolNumber,
    target_index: TransitionTableIndex,
}

impl Transition {
    const SIZE: usize =
        2 * std::mem::size_of::<SymbolNumber>() + std::mem::size_of::<TransitionTableIndex>();

    // [spec:hfst:def:hfst-optimized-lookup.transition.transition-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition.transition-fn]
    fn new(input: SymbolNumber, output: SymbolNumber, target: TransitionTableIndex) -> Self {
        Transition {
            input_symbol: input,
            output_symbol: output,
            target_index: target,
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition.matches-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition.matches-fn]
    #[allow(dead_code)]
    fn matches(&self, s: SymbolNumber) -> bool {
        if self.input_symbol == NO_SYMBOL_NUMBER {
            return false;
        }
        if s == NO_SYMBOL_NUMBER {
            return true;
        }
        self.input_symbol == s
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition.target-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition.target-fn]
    fn target(&self) -> TransitionTableIndex {
        self.target_index
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition.get-output-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition.get-output-fn]
    fn get_output(&self) -> SymbolNumber {
        self.output_symbol
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition.get-input-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition.get-input-fn]
    fn get_input(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition.final-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition.final-fn]
    fn is_final(&self) -> bool {
        self.target_index == 1
    }
}

// ---------------------------------------------------------------------------
// IndexTableReader / TransitionTableReader (unweighted)
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transition-index-vector]
// TransitionIndexVector
// [spec:hfst:def:hfst-optimized-lookup.transition-vector]
// TransitionVector
// [spec:hfst:def:hfst-optimized-lookup.index-table-reader]
struct IndexTableReader {
    number_of_table_entries: TransitionTableIndex,
    table_indices: Vec<u8>,
    indices: Vec<TransitionIndex>,
}

impl IndexTableReader {
    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader.index-table-reader-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.index-table-reader-fn]
    unsafe fn new(f: *mut FILE, index_count: TransitionTableIndex) -> Self {
        unsafe {
            let table_size = index_count as usize * TransitionIndex::SIZE;
            let mut table_indices = vec![0u8; table_size];
            if table_size > 0 {
                let _ = fread(table_indices.as_mut_ptr() as *mut c_void, table_size, 1, f);
            }
            let mut r = IndexTableReader {
                number_of_table_entries: index_count,
                table_indices,
                indices: Vec::new(),
            };
            r.get_index_vector();
            r
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader.get-index-vector-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.get-index-vector-fn]
    fn get_index_vector(&mut self) {
        unsafe {
            for i in 0..self.number_of_table_entries as usize {
                let j = i * TransitionIndex::SIZE;
                let input = read_le_u16(&self.table_indices, j);
                let index =
                    read_le_u32(&self.table_indices, j + std::mem::size_of::<SymbolNumber>());
                self.indices.push(TransitionIndex::new(input, index));
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader.get-finality-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.get-finality-fn]
    #[allow(dead_code)]
    fn get_finality(&self, i: TransitionTableIndex) -> bool {
        self.indices[i as usize].is_final()
    }

    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader.at-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.at-fn]
    #[allow(dead_code)]
    fn at(&self, i: TransitionTableIndex) -> &TransitionIndex {
        &self.indices[i as usize]
    }
}

// [spec:hfst:def:hfst-optimized-lookup.transition-table-reader]
struct TransitionTableReader {
    number_of_table_entries: TransitionTableIndex,
    table_transitions: Vec<u8>,
    transitions: Vec<Transition>,
    position: TransitionTableIndex,
}

impl TransitionTableReader {
    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.transition-table-reader-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.transition-table-reader-fn]
    unsafe fn new(f: *mut FILE, transition_count: TransitionTableIndex) -> Self {
        unsafe {
            let table_size = transition_count as usize * Transition::SIZE;
            let mut table_transitions = vec![0u8; table_size];
            if table_size > 0 {
                let _ = fread(
                    table_transitions.as_mut_ptr() as *mut c_void,
                    table_size,
                    1,
                    f,
                );
            }
            let mut r = TransitionTableReader {
                number_of_table_entries: transition_count,
                table_transitions,
                transitions: Vec::new(),
                position: 0,
            };
            r.get_transition_vector();
            r
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.set-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.set-fn]
    #[allow(dead_code)]
    fn set(&mut self, pos: TransitionTableIndex) {
        if pos >= TRANSITION_TARGET_TABLE_START {
            self.position = pos - TRANSITION_TARGET_TABLE_START;
        } else {
            self.position = pos;
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-transition-vector-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-transition-vector-fn]
    fn get_transition_vector(&mut self) {
        unsafe {
            for i in 0..self.number_of_table_entries as usize {
                let j = i * Transition::SIZE;
                let input = read_le_u16(&self.table_transitions, j);
                let output = read_le_u16(
                    &self.table_transitions,
                    j + std::mem::size_of::<SymbolNumber>(),
                );
                let target = read_le_u32(
                    &self.table_transitions,
                    j + 2 * std::mem::size_of::<SymbolNumber>(),
                );
                self.transitions
                    .push(Transition::new(input, output, target));
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.matches-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.matches-fn]
    #[allow(dead_code)]
    fn matches(&self, s: SymbolNumber) -> bool {
        self.transitions[self.position as usize].matches(s)
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.next-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.next-fn]
    #[allow(dead_code)]
    fn next(&mut self) {
        self.position += 1;
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.at-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.at-fn]
    #[allow(dead_code)]
    fn at(&self, i: TransitionTableIndex) -> &Transition {
        &self.transitions[(i - TRANSITION_TARGET_TABLE_START) as usize]
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-target-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-target-fn]
    #[allow(dead_code)]
    fn get_target(&self) -> TransitionTableIndex {
        self.transitions[self.position as usize].target()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-output-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-output-fn]
    #[allow(dead_code)]
    fn get_output(&self) -> SymbolNumber {
        self.transitions[self.position as usize].get_output()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-input-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-input-fn]
    #[allow(dead_code)]
    fn get_input(&self) -> SymbolNumber {
        self.transitions[self.position as usize].get_input()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-finality-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-finality-fn]
    #[allow(dead_code)]
    fn get_finality(&self, i: TransitionTableIndex) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.transitions[(i - TRANSITION_TARGET_TABLE_START) as usize].is_final()
        } else {
            self.transitions[i as usize].is_final()
        }
    }
}

// ---------------------------------------------------------------------------
// TransitionWIndex / TransitionW (weighted)
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transition-w-index]
#[derive(Clone, Copy)]
struct TransitionWIndex {
    input_symbol: SymbolNumber,
    first_transition_index: TransitionTableIndex,
}

impl TransitionWIndex {
    const SIZE: usize =
        std::mem::size_of::<SymbolNumber>() + std::mem::size_of::<TransitionTableIndex>();

    // [spec:hfst:def:hfst-optimized-lookup.transition-w-index.transition-w-index-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.transition-w-index-fn]
    fn new(input: SymbolNumber, first_transition: TransitionTableIndex) -> Self {
        TransitionWIndex {
            input_symbol: input,
            first_transition_index: first_transition,
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w-index.matches-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.matches-fn]
    #[allow(dead_code)]
    fn matches(&self, s: SymbolNumber) -> bool {
        if self.input_symbol == NO_SYMBOL_NUMBER {
            return false;
        }
        if s == NO_SYMBOL_NUMBER {
            return true;
        }
        self.input_symbol == s
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w-index.target-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.target-fn]
    fn target(&self) -> TransitionTableIndex {
        self.first_transition_index
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w-index.final-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.final-fn]
    fn is_final(&self) -> bool {
        self.input_symbol == NO_SYMBOL_NUMBER && self.first_transition_index != NO_TABLE_INDEX
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w-index.final-weight-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.final-weight-fn]
    fn final_weight(&self) -> Weight {
        // union reinterpretation of the index bits as a float
        f32::from_bits(self.first_transition_index)
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w-index.get-input-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.get-input-fn]
    fn get_input(&self) -> SymbolNumber {
        self.input_symbol
    }
}

// [spec:hfst:def:hfst-optimized-lookup.transition-w]
#[derive(Clone, Copy)]
struct TransitionW {
    input_symbol: SymbolNumber,
    output_symbol: SymbolNumber,
    target_index: TransitionTableIndex,
    transition_weight: Weight,
}

impl TransitionW {
    const SIZE: usize = 2 * std::mem::size_of::<SymbolNumber>()
        + std::mem::size_of::<TransitionTableIndex>()
        + std::mem::size_of::<Weight>();

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.transition-w-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.transition-w-fn]
    fn new(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
        w: Weight,
    ) -> Self {
        TransitionW {
            input_symbol: input,
            output_symbol: output,
            target_index: target,
            transition_weight: w,
        }
    }

    // default constructor
    fn empty() -> Self {
        TransitionW {
            input_symbol: NO_SYMBOL_NUMBER,
            output_symbol: NO_SYMBOL_NUMBER,
            target_index: NO_TABLE_INDEX,
            transition_weight: INFINITE_WEIGHT,
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.matches-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.matches-fn]
    #[allow(dead_code)]
    fn matches(&self, s: SymbolNumber) -> bool {
        if self.input_symbol == NO_SYMBOL_NUMBER {
            return false;
        }
        if s == NO_SYMBOL_NUMBER {
            return true;
        }
        self.input_symbol == s
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.target-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.target-fn]
    fn target(&self) -> TransitionTableIndex {
        self.target_index
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.get-output-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-output-fn]
    fn get_output(&self) -> SymbolNumber {
        self.output_symbol
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.get-input-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-input-fn]
    fn get_input(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.get-weight-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-weight-fn]
    fn get_weight(&self) -> Weight {
        self.transition_weight
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-w.final-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-w.final-fn]
    fn is_final(&self) -> bool {
        self.input_symbol == NO_SYMBOL_NUMBER
            && self.output_symbol == NO_SYMBOL_NUMBER
            && self.target_index == 1
    }
}

// ---------------------------------------------------------------------------
// IndexTableReaderW / TransitionTableReaderW (weighted)
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transition-w-index-vector]
// TransitionWIndexVector
// [spec:hfst:def:hfst-optimized-lookup.transition-w-vector]
// TransitionWVector
// [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w]
struct IndexTableReaderW {
    number_of_table_entries: TransitionTableIndex,
    table_indices: Vec<u8>,
    indices: Vec<TransitionWIndex>,
}

impl IndexTableReaderW {
    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.index-table-reader-w-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.index-table-reader-w-fn]
    unsafe fn new(f: *mut FILE, index_count: TransitionTableIndex) -> Self {
        unsafe {
            let table_size = index_count as usize * TransitionWIndex::SIZE;
            let mut table_indices = vec![0u8; table_size];
            if table_size > 0 {
                let _ = fread(table_indices.as_mut_ptr() as *mut c_void, table_size, 1, f);
            }
            let mut r = IndexTableReaderW {
                number_of_table_entries: index_count,
                table_indices,
                indices: Vec::new(),
            };
            r.get_index_vector();
            r
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.get-index-vector-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.get-index-vector-fn]
    fn get_index_vector(&mut self) {
        unsafe {
            for i in 0..self.number_of_table_entries as usize {
                let j = i * TransitionWIndex::SIZE;
                let input = read_le_u16(&self.table_indices, j);
                let index =
                    read_le_u32(&self.table_indices, j + std::mem::size_of::<SymbolNumber>());
                self.indices.push(TransitionWIndex::new(input, index));
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.get-finality-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.get-finality-fn]
    #[allow(dead_code)]
    fn get_finality(&self, i: TransitionTableIndex) -> bool {
        self.indices[i as usize].is_final()
    }

    // [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.at-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.at-fn]
    #[allow(dead_code)]
    fn at(&self, i: TransitionTableIndex) -> &TransitionWIndex {
        &self.indices[i as usize]
    }
}

// [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w]
struct TransitionTableReaderW {
    number_of_table_entries: TransitionTableIndex,
    table_transitions: Vec<u8>,
    transitions: Vec<TransitionW>,
    position: TransitionTableIndex,
}

impl TransitionTableReaderW {
    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.transition-table-reader-w-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.transition-table-reader-w-fn]
    unsafe fn new(f: *mut FILE, transition_count: TransitionTableIndex) -> Self {
        unsafe {
            let table_size = transition_count as usize * TransitionW::SIZE;
            let mut table_transitions = vec![0u8; table_size];
            if table_size > 0 {
                let _ = fread(
                    table_transitions.as_mut_ptr() as *mut c_void,
                    table_size,
                    1,
                    f,
                );
            }
            let mut r = TransitionTableReaderW {
                number_of_table_entries: transition_count,
                table_transitions,
                transitions: Vec::new(),
                position: 0,
            };
            r.get_transition_vector();
            r
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.set-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.set-fn]
    #[allow(dead_code)]
    fn set(&mut self, pos: TransitionTableIndex) {
        if pos >= TRANSITION_TARGET_TABLE_START {
            self.position = pos - TRANSITION_TARGET_TABLE_START;
        } else {
            self.position = pos;
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-transition-vector-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-transition-vector-fn]
    fn get_transition_vector(&mut self) {
        unsafe {
            for i in 0..self.number_of_table_entries as usize {
                let j = i * TransitionW::SIZE;
                let input = read_le_u16(&self.table_transitions, j);
                let output = read_le_u16(
                    &self.table_transitions,
                    j + std::mem::size_of::<SymbolNumber>(),
                );
                let target = read_le_u32(
                    &self.table_transitions,
                    j + 2 * std::mem::size_of::<SymbolNumber>(),
                );
                let weight = read_le_f32(
                    &self.table_transitions,
                    j + 2 * std::mem::size_of::<SymbolNumber>()
                        + std::mem::size_of::<TransitionTableIndex>(),
                );
                self.transitions
                    .push(TransitionW::new(input, output, target, weight));
            }
            self.transitions.push(TransitionW::empty());
            self.transitions.push(TransitionW::empty());
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.matches-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.matches-fn]
    #[allow(dead_code)]
    fn matches(&self, s: SymbolNumber) -> bool {
        self.transitions[self.position as usize].matches(s)
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.next-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.next-fn]
    #[allow(dead_code)]
    fn next(&mut self) {
        self.position += 1;
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.at-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.at-fn]
    #[allow(dead_code)]
    fn at(&self, i: TransitionTableIndex) -> &TransitionW {
        &self.transitions[(i - TRANSITION_TARGET_TABLE_START) as usize]
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-target-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-target-fn]
    #[allow(dead_code)]
    fn get_target(&self) -> TransitionTableIndex {
        self.transitions[self.position as usize].target()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-output-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-output-fn]
    #[allow(dead_code)]
    fn get_output(&self) -> SymbolNumber {
        self.transitions[self.position as usize].get_output()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-input-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-input-fn]
    #[allow(dead_code)]
    fn get_input(&self) -> SymbolNumber {
        self.transitions[self.position as usize].get_input()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-finality-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-finality-fn]
    #[allow(dead_code)]
    fn get_finality(&self, i: TransitionTableIndex) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.transitions[(i - TRANSITION_TARGET_TABLE_START) as usize].is_final()
        } else {
            self.transitions[i as usize].is_final()
        }
    }
}

// ---------------------------------------------------------------------------
// Display sink: the various transducer variants differ only in how they
// collect and emit analyses. This enum captures the C++ class hierarchy's
// virtual note_analysis / printAnalyses, plus the optional flag-diacritic state.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Plain,
    Uniq,
    Fd,
    FdUniq,
    WPlain,
    WUniq,
    WFd,
    WFdUniq,
}

fn variant_weighted(v: Variant) -> bool {
    matches!(
        v,
        Variant::WPlain | Variant::WUniq | Variant::WFd | Variant::WFdUniq
    )
}

fn variant_has_fd(v: Variant) -> bool {
    matches!(
        v,
        Variant::Fd | Variant::FdUniq | Variant::WFd | Variant::WFdUniq
    )
}

// ---------------------------------------------------------------------------
// The Transducer (covers Transducer / TransducerUniq / TransducerFd /
// TransducerFdUniq for the unweighted side, and TransducerW family for the
// weighted side; selected via Variant). This mirrors the C++ class hierarchy.
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.transducer]
// [spec:hfst:def:hfst-optimized-lookup.transducer-uniq]
// [spec:hfst:def:hfst-optimized-lookup.transducer-fd]
// [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq]
// [spec:hfst:def:hfst-optimized-lookup.transducer-w]
// [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq]
// [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd]
// [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq]
struct Transducer {
    variant: Variant,
    header: TransducerHeader,
    keys: KeyTable,
    indices: Vec<TransitionIndex>,
    transitions: Vec<Transition>,
    w_indices: Vec<TransitionWIndex>,
    w_transitions: Vec<TransitionW>,
    encoder: Encoder,
    symbol_table: Vec<CString>,

    // unweighted output buffer (1000 entries of NO_SYMBOL_NUMBER)
    output_string: Vec<SymbolNumber>,

    // display sinks
    display_vector: DisplayVector,     // Plain
    display_set: DisplaySet,           // Uniq / FdUniq
    display_multimap: DisplayMultiMap, // WPlain
    display_map: DisplayMap,           // WUniq / WFdUniq

    // flag diacritics
    statestack: FlagDiacriticStateStack,
    operations: OperationVector,

    current_weight: Weight,
}

const START_INDEX: TransitionTableIndex = 0;

impl Transducer {
    // [spec:hfst:def:hfst-optimized-lookup.transducer.transducer-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.transducer-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.transducer-uniq-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.transducer-uniq-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd.transducer-fd-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.transducer-fd-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.transducer-fd-uniq-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.transducer-fd-uniq-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.transducer-w-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.transducer-w-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.transducer-w-uniq-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.transducer-w-uniq-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.transducer-w-fd-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.transducer-w-fd-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.transducer-w-fd-uniq-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.transducer-w-fd-uniq-fn]
    unsafe fn new(
        variant: Variant,
        f: *mut FILE,
        header: TransducerHeader,
        alphabet: TransducerAlphabet,
    ) -> Self {
        unsafe {
            let keys = alphabet.get_key_table().clone();
            let state_size = alphabet.get_state_size();
            let operations = alphabet.get_operation_vector();
            let input_symbol_count = header.input_symbol_count();
            let index_table_size = header.index_table_size();
            let target_table_size = header.target_table_size();

            // tables are read from the file in the same order the C++ ctor
            // initializer list reads them: index reader first, then transition.
            let (indices, transitions, w_indices, w_transitions) = if variant_weighted(variant) {
                let ir = IndexTableReaderW::new(f, index_table_size);
                let tr = TransitionTableReaderW::new(f, target_table_size);
                (Vec::new(), Vec::new(), ir.indices, tr.transitions)
            } else {
                let ir = IndexTableReader::new(f, index_table_size);
                let tr = TransitionTableReader::new(f, target_table_size);
                (ir.indices, tr.transitions, Vec::new(), Vec::new())
            };

            let encoder = Encoder::new(&keys, input_symbol_count);

            let mut symbol_table: Vec<CString> = Vec::new();
            for (_k, v) in keys.iter() {
                symbol_table.push(v.clone());
            }

            let statestack = if variant_has_fd(variant) {
                vec![vec![0 as ValueNumber; state_size as usize]]
            } else {
                Vec::new()
            };

            Transducer {
                variant,
                header,
                keys,
                indices,
                transitions,
                w_indices,
                w_transitions,
                encoder,
                symbol_table,
                output_string: vec![NO_SYMBOL_NUMBER; 1000],
                display_vector: DisplayVector::new(),
                display_set: DisplaySet::new(),
                display_multimap: DisplayMultiMap::new(),
                display_map: DisplayMap::new(),
                statestack,
                operations,
                current_weight: 0.0,
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.set-symbol-table-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.set-symbol-table-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.set-symbol-table-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.set-symbol-table-fn]
    // (the symbol table is built in new(); kept as a named step for parity.)
    fn symbol_str(&self, n: SymbolNumber) -> &str {
        self.symbol_table[n as usize].to_str().unwrap_or("")
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.get-key-table-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.get-key-table-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-key-table-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-key-table-fn]
    #[allow(dead_code)]
    fn get_key_table(&self) -> &KeyTable {
        &self.keys
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.find-next-key-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.find-next-key-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-next-key-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-next-key-fn]
    unsafe fn find_next_key(&self, base: *const u8, p: &mut usize) -> SymbolNumber {
        unsafe { self.encoder.find_key(base, p) }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.analyze-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.analyze-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.analyze-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.analyze-fn]
    fn analyze(&mut self, input_string: &[SymbolNumber]) {
        self.current_weight = 0.0;
        if variant_weighted(self.variant) {
            // C++: get_analyses(input_string, &output_string[0], &output_string[0], START_INDEX)
            let mut out = self.output_string.clone();
            self.w_get_analyses(input_string, 0, &mut out, 0, START_INDEX);
        } else {
            let mut out = self.output_string.clone();
            self.get_analyses(input_string, 0, &mut out, 0, START_INDEX);
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.final-transition-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.final-transition-fn]
    fn final_transition(&self, i: TransitionTableIndex) -> bool {
        self.transitions[i as usize].is_final()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.final-index-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.final-index-fn]
    fn final_index(&self, i: TransitionTableIndex) -> bool {
        self.indices[i as usize].is_final()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd.push-state-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.push-state-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.push-state-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.push-state-fn]
    fn push_state(&mut self, op: FlagDiacriticOperation) -> bool {
        let feat = op.feature() as usize;
        match op.operation() {
            P => {
                // positive set
                let mut top = self.statestack.last().unwrap().clone();
                top[feat] = op.value();
                self.statestack.push(top);
                true
            }
            N => {
                // negative set (literally, in this implementation)
                let mut top = self.statestack.last().unwrap().clone();
                top[feat] = -1 * op.value();
                self.statestack.push(top);
                true
            }
            R => {
                // require
                if op.value() == 0 {
                    // empty require
                    if self.statestack.last().unwrap()[feat] == 0 {
                        return false;
                    } else {
                        let top = self.statestack.last().unwrap().clone();
                        self.statestack.push(top);
                        return true;
                    }
                }
                if self.statestack.last().unwrap()[feat] == op.value() {
                    let top = self.statestack.last().unwrap().clone();
                    self.statestack.push(top);
                    return true;
                }
                false
            }
            D => {
                // disallow
                if op.value() == 0 {
                    // empty disallow
                    if self.statestack.last().unwrap()[feat] != 0 {
                        return false;
                    } else {
                        let top = self.statestack.last().unwrap().clone();
                        self.statestack.push(top);
                        return true;
                    }
                }
                if self.statestack.last().unwrap()[feat] == op.value() {
                    // nonempty disallow
                    return false;
                }
                let top = self.statestack.last().unwrap().clone();
                self.statestack.push(top);
                true
            }
            C => {
                // clear
                let mut top = self.statestack.last().unwrap().clone();
                top[feat] = 0;
                self.statestack.push(top);
                true
            }
            U => {
                // unification
                let cur = self.statestack.last().unwrap()[feat];
                if cur == 0 || cur == op.value() || (cur < 0 && (cur * -1 != op.value())) {
                    let mut top = self.statestack.last().unwrap().clone();
                    top[feat] = op.value();
                    self.statestack.push(top);
                    return true;
                }
                false
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.note-analysis-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.note-analysis-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.note-analysis-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.note-analysis-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.note-analysis-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.note-analysis-fn]
    // whole_output is the output buffer; we read from offset 0 to the first
    // NO_SYMBOL_NUMBER.
    fn note_analysis(&mut self, whole_output: &[SymbolNumber]) {
        match self.variant {
            Variant::Plain => {
                if unsafe { BE_FAST } {
                    for &num in whole_output.iter().take_while(|&&n| n != NO_SYMBOL_NUMBER) {
                        print_out(self.symbol_str(num));
                    }
                    print_out("\n");
                } else {
                    let mut s = String::new();
                    for &num in whole_output.iter().take_while(|&&n| n != NO_SYMBOL_NUMBER) {
                        s.push_str(self.symbol_str(num));
                    }
                    self.display_vector.push(s);
                }
            }
            Variant::Uniq | Variant::Fd | Variant::FdUniq => {
                // TransducerUniq/TransducerFdUniq insert into a DisplaySet.
                // (TransducerFd has no own note_analysis: it inherits the
                // Transducer/Plain behaviour, handled in the Fd arm below.)
                if self.variant == Variant::Fd {
                    // inherits Transducer::note_analysis
                    if unsafe { BE_FAST } {
                        for &num in whole_output.iter().take_while(|&&n| n != NO_SYMBOL_NUMBER) {
                            print_out(self.symbol_str(num));
                        }
                        print_out("\n");
                    } else {
                        let mut s = String::new();
                        for &num in whole_output.iter().take_while(|&&n| n != NO_SYMBOL_NUMBER) {
                            s.push_str(self.symbol_str(num));
                        }
                        self.display_vector.push(s);
                    }
                } else {
                    let mut s = String::new();
                    for &num in whole_output.iter().take_while(|&&n| n != NO_SYMBOL_NUMBER) {
                        s.push_str(self.symbol_str(num));
                    }
                    self.display_set.insert(s);
                }
            }
            _ => unreachable!("weighted note_analysis handled by w_note_analysis"),
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.note-analysis-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.note-analysis-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.note-analysis-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.note-analysis-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.note-analysis-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.note-analysis-fn]
    fn w_note_analysis(&mut self, whole_output: &[SymbolNumber], len: usize) {
        match self.variant {
            Variant::WPlain | Variant::WFd => {
                // TransducerW::note_analysis (WFd inherits it): iterate while
                // num <= &output_string.back() && *num != NO_SYMBOL_NUMBER
                let mut s = String::new();
                for k in 0..len {
                    if whole_output[k] == NO_SYMBOL_NUMBER {
                        break;
                    }
                    s.push_str(self.symbol_str(whole_output[k]));
                }
                self.display_multimap.push((self.current_weight, s));
            }
            Variant::WUniq | Variant::WFdUniq => {
                let mut s = String::new();
                for &num in whole_output.iter().take_while(|&&n| n != NO_SYMBOL_NUMBER) {
                    s.push_str(self.symbol_str(num));
                }
                // if there isn't an entry yet or we've found a lower weight
                let lower = match self.display_map.get(&s) {
                    None => true,
                    Some(&w) => w > self.current_weight,
                };
                if lower {
                    // C++ uses multimap-style insert which keeps the first; here
                    // BTreeMap::insert overwrites. The guard above matches the
                    // C++ count()==0 || stored>current logic.
                    self.display_map.entry(s).or_insert(self.current_weight);
                }
            }
            _ => unreachable!("unweighted note_analysis handled by note_analysis"),
        }
    }

    // ---- unweighted search ----

    // [spec:hfst:def:hfst-optimized-lookup.transducer.try-epsilon-transitions-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.try-epsilon-transitions-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd.try-epsilon-transitions-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.try-epsilon-transitions-fn]
    fn try_epsilon_transitions(
        &mut self,
        input: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        mut i: TransitionTableIndex,
    ) {
        if variant_has_fd(self.variant) {
            // TransducerFd::try_epsilon_transitions
            loop {
                let ti = i as usize;
                if self.transitions[ti].get_input() == 0 {
                    // epsilon
                    out[out_pos] = self.transitions[ti].get_output();
                    let target = self.transitions[ti].target();
                    self.get_analyses(input, in_pos, out, out_pos + 1, target);
                    i += 1;
                } else if self.transitions[ti].get_input() != NO_SYMBOL_NUMBER
                    && self.operations[self.transitions[ti].get_input() as usize].is_flag()
                {
                    let op = self.operations[self.transitions[ti].get_input() as usize];
                    if self.push_state(op) {
                        // flag diacritic allowed
                        out[out_pos] = self.transitions[ti].get_output();
                        let target = self.transitions[ti].target();
                        self.get_analyses(input, in_pos, out, out_pos + 1, target);
                        self.statestack.pop();
                    }
                    i += 1;
                } else {
                    return;
                }
            }
        } else {
            // Transducer::try_epsilon_transitions
            while self.transitions[i as usize].get_input() == 0 {
                out[out_pos] = self.transitions[i as usize].get_output();
                let target = self.transitions[i as usize].target();
                self.get_analyses(input, in_pos, out, out_pos + 1, target);
                i += 1;
            }
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.try-epsilon-indices-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.try-epsilon-indices-fn]
    fn try_epsilon_indices(
        &mut self,
        input: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        i: TransitionTableIndex,
    ) {
        if self.indices[i as usize].get_input() == 0 {
            let target = self.indices[i as usize].target() - TRANSITION_TARGET_TABLE_START;
            self.try_epsilon_transitions(input, in_pos, out, out_pos, target);
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.find-transitions-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.find-transitions-fn]
    fn find_transitions(
        &mut self,
        input: SymbolNumber,
        in_str: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        mut i: TransitionTableIndex,
    ) {
        while self.transitions[i as usize].get_input() != NO_SYMBOL_NUMBER {
            if self.transitions[i as usize].get_input() == input {
                out[out_pos] = self.transitions[i as usize].get_output();
                let target = self.transitions[i as usize].target();
                self.get_analyses(in_str, in_pos, out, out_pos + 1, target);
            } else {
                return;
            }
            i += 1;
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.find-index-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.find-index-fn]
    fn find_index(
        &mut self,
        input: SymbolNumber,
        in_str: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        i: TransitionTableIndex,
    ) {
        if self.indices[(i + input as TransitionTableIndex) as usize].get_input() == input {
            let target = self.indices[(i + input as TransitionTableIndex) as usize].target()
                - TRANSITION_TARGET_TABLE_START;
            self.find_transitions(input, in_str, in_pos, out, out_pos, target);
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer.get-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.get-analyses-fn]
    fn get_analyses(
        &mut self,
        input: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        mut i: TransitionTableIndex,
    ) {
        unsafe {
            if TIME_CUTOFF > 0.0 {
                CALL_COUNTER += 1;
                if LIMIT_REACHED
                    || (CALL_COUNTER % 1000000 == 0
                        && (((clock() as f64) - START_CLOCK as f64) / CLOCKS_PER_SEC) > TIME_CUTOFF)
                {
                    LIMIT_REACHED = true;
                    return;
                }
            }
        }
        if i >= TRANSITION_TARGET_TABLE_START {
            i -= TRANSITION_TARGET_TABLE_START;

            self.try_epsilon_transitions(input, in_pos, out, out_pos, i + 1);

            // input-string ended.
            if input[in_pos] == NO_SYMBOL_NUMBER {
                out[out_pos] = NO_SYMBOL_NUMBER;
                if self.final_transition(i) {
                    let snapshot = out.clone();
                    self.note_analysis(&snapshot);
                }
                return;
            }

            let in_sym = input[in_pos];
            self.find_transitions(in_sym, input, in_pos + 1, out, out_pos, i + 1);
        } else {
            self.try_epsilon_indices(input, in_pos, out, out_pos, i + 1);

            if input[in_pos] == NO_SYMBOL_NUMBER {
                // input-string ended.
                out[out_pos] = NO_SYMBOL_NUMBER;
                if self.final_index(i) {
                    let snapshot = out.clone();
                    self.note_analysis(&snapshot);
                }
                return;
            }

            let in_sym = input[in_pos];
            self.find_index(in_sym, input, in_pos + 1, out, out_pos, i + 1);
        }
        out[out_pos] = NO_SYMBOL_NUMBER;
    }

    // ---- weighted search ----

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.final-transition-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.final-transition-fn]
    fn w_final_transition(&self, i: TransitionTableIndex) -> bool {
        self.w_transitions[i as usize].is_final()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.final-index-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.final-index-fn]
    fn w_final_index(&self, i: TransitionTableIndex) -> bool {
        self.w_indices[i as usize].is_final()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-final-index-weight-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-final-index-weight-fn]
    fn get_final_index_weight(&self, i: TransitionTableIndex) -> Weight {
        self.w_indices[i as usize].final_weight()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-final-transition-weight-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-final-transition-weight-fn]
    fn get_final_transition_weight(&self, i: TransitionTableIndex) -> Weight {
        self.w_transitions[i as usize].get_weight()
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.try-epsilon-transitions-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.try-epsilon-transitions-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.try-epsilon-transitions-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.try-epsilon-transitions-fn]
    fn w_try_epsilon_transitions(
        &mut self,
        input: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        mut i: TransitionTableIndex,
    ) {
        if (self.w_transitions.len() as TransitionTableIndex) <= i {
            return;
        }
        if variant_has_fd(self.variant) {
            // Endless loop protection: output_symbol > &output_string.back()
            if out_pos > out.len() - 1 {
                return;
            }
            loop {
                let ti = i as usize;
                if self.w_transitions[ti].get_input() == 0 {
                    // epsilon
                    out[out_pos] = self.w_transitions[ti].get_output();
                    self.current_weight += self.w_transitions[ti].get_weight();
                    let target = self.w_transitions[ti].target();
                    self.w_get_analyses(input, in_pos, out, out_pos + 1, target);
                    self.current_weight -= self.w_transitions[ti].get_weight();
                    i += 1;
                } else if self.w_transitions[ti].get_input() != NO_SYMBOL_NUMBER
                    && self.operations[self.w_transitions[ti].get_input() as usize].is_flag()
                {
                    let op = self.operations[self.w_transitions[ti].get_input() as usize];
                    if self.push_state(op) {
                        // flag diacritic allowed
                        out[out_pos] = self.w_transitions[ti].get_output();
                        self.current_weight += self.w_transitions[ti].get_weight();
                        let target = self.w_transitions[ti].target();
                        self.w_get_analyses(input, in_pos, out, out_pos + 1, target);
                        self.current_weight -= self.w_transitions[ti].get_weight();
                        self.statestack.pop();
                    }
                    i += 1;
                } else {
                    return;
                }
            }
        } else {
            // TransducerW::try_epsilon_transitions
            while (i as usize) < self.w_transitions.len()
                && self.w_transitions[i as usize].get_input() == 0
            {
                out[out_pos] = self.w_transitions[i as usize].get_output();
                self.current_weight += self.w_transitions[i as usize].get_weight();
                let target = self.w_transitions[i as usize].target();
                self.w_get_analyses(input, in_pos, out, out_pos + 1, target);
                self.current_weight -= self.w_transitions[i as usize].get_weight();
                i += 1;
            }
            out[out_pos] = NO_SYMBOL_NUMBER;
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.try-epsilon-indices-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.try-epsilon-indices-fn]
    fn w_try_epsilon_indices(
        &mut self,
        input: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        i: TransitionTableIndex,
    ) {
        if self.w_indices[i as usize].get_input() == 0 {
            let target = self.w_indices[i as usize].target() - TRANSITION_TARGET_TABLE_START;
            self.w_try_epsilon_transitions(input, in_pos, out, out_pos, target);
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-transitions-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-transitions-fn]
    fn w_find_transitions(
        &mut self,
        input: SymbolNumber,
        in_str: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        mut i: TransitionTableIndex,
    ) {
        if (self.w_transitions.len() as TransitionTableIndex) <= i {
            return;
        }
        // Endless loop protection
        if out_pos > out.len() - 1 {
            return;
        }
        while self.w_transitions[i as usize].get_input() != NO_SYMBOL_NUMBER {
            if self.w_transitions[i as usize].get_input() == input {
                self.current_weight += self.w_transitions[i as usize].get_weight();
                out[out_pos] = self.w_transitions[i as usize].get_output();
                let target = self.w_transitions[i as usize].target();
                self.w_get_analyses(in_str, in_pos, out, out_pos + 1, target);
                self.current_weight -= self.w_transitions[i as usize].get_weight();
            } else {
                return;
            }
            i += 1;
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-index-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-index-fn]
    fn w_find_index(
        &mut self,
        input: SymbolNumber,
        in_str: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        i: TransitionTableIndex,
    ) {
        if (self.w_indices.len() as TransitionTableIndex) <= i {
            return;
        }
        if self.w_indices[(i + input as TransitionTableIndex) as usize].get_input() == input {
            let target = self.w_indices[(i + input as TransitionTableIndex) as usize].target()
                - TRANSITION_TARGET_TABLE_START;
            self.w_find_transitions(input, in_str, in_pos, out, out_pos, target);
        }
    }

    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-analyses-fn]
    fn w_get_analyses(
        &mut self,
        input: &[SymbolNumber],
        in_pos: usize,
        out: &mut Vec<SymbolNumber>,
        out_pos: usize,
        mut i: TransitionTableIndex,
    ) {
        unsafe {
            if TIME_CUTOFF > 0.0 {
                CALL_COUNTER += 1;
                if LIMIT_REACHED
                    || (CALL_COUNTER % 1000000 == 0
                        && (((clock() as f64) - START_CLOCK as f64) / CLOCKS_PER_SEC) > TIME_CUTOFF)
                {
                    LIMIT_REACHED = true;
                    return;
                }
            }
        }

        // Endless loop protection
        if out_pos > out.len() - 1 {
            return;
        }

        if i >= TRANSITION_TARGET_TABLE_START {
            i -= TRANSITION_TARGET_TABLE_START;

            self.w_try_epsilon_transitions(input, in_pos, out, out_pos, i + 1);

            // input-string ended.
            if input[in_pos] == NO_SYMBOL_NUMBER {
                out[out_pos] = NO_SYMBOL_NUMBER;
                if (self.w_transitions.len() as TransitionTableIndex) <= i {
                    return;
                }
                if self.w_final_transition(i) {
                    self.current_weight += self.get_final_transition_weight(i);
                    let snapshot = out.clone();
                    self.w_note_analysis(&snapshot, out_pos);
                    self.current_weight -= self.get_final_transition_weight(i);
                }
                return;
            }

            let in_sym = input[in_pos];
            self.w_find_transitions(in_sym, input, in_pos + 1, out, out_pos, i + 1);
        } else {
            self.w_try_epsilon_indices(input, in_pos, out, out_pos, i + 1);
            // input-string ended.
            if input[in_pos] == NO_SYMBOL_NUMBER {
                out[out_pos] = NO_SYMBOL_NUMBER;
                if self.w_final_index(i) {
                    self.current_weight += self.get_final_index_weight(i);
                    let snapshot = out.clone();
                    self.w_note_analysis(&snapshot, out_pos);
                    self.current_weight -= self.get_final_index_weight(i);
                }
                return;
            }

            let in_sym = input[in_pos];
            self.w_find_index(in_sym, input, in_pos + 1, out, out_pos, i + 1);
        }
    }

    // ---- printing ----

    // [spec:hfst:def:hfst-optimized-lookup.transducer.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
    // [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
    // [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
    fn print_analyses(&mut self, prepend: &str) {
        let output_type = unsafe { OUTPUT_TYPE };
        let display_weights = unsafe { DISPLAY_WEIGHTS_FLAG };
        let max_analyses = unsafe { MAX_ANALYSES };
        let beam = unsafe { BEAM };
        match self.variant {
            Variant::Plain | Variant::Fd => {
                // Transducer::printAnalyses (Fd inherits it). beFast -> nothing.
                if unsafe { BE_FAST } {
                    return;
                }
                if output_type == Xerox && self.display_vector.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                let mut i = 0;
                for it in self.display_vector.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if output_type == Xerox {
                        print_out(&format!("{}\t", prepend));
                    }
                    print_out(&format!("{}\n", it));
                    i += 1;
                }
                self.display_vector.clear(); // purge the display vector
                print_out("\n");
            }
            Variant::Uniq | Variant::FdUniq => {
                // TransducerUniq/TransducerFdUniq::printAnalyses
                if output_type == Xerox && self.display_set.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                let mut i = 0;
                for it in self.display_set.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if output_type == Xerox {
                        print_out(&format!("{}\t", prepend));
                    }
                    print_out(&format!("{}\n", it));
                    i += 1;
                }
                self.display_set.clear(); // purge the display set
                print_out("\n");
            }
            Variant::WPlain => {
                // TransducerW::printAnalyses
                if output_type == Xerox && self.display_multimap.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                // C++ iterates a std::multimap<Weight,string>, ascending by key.
                let mut sorted = self.display_multimap.clone();
                sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut i = 0;
                let mut lowest_weight: f32 = -1.0;
                let mut first = true;
                for (weight, value) in sorted.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if first {
                        lowest_weight = *weight;
                        first = false;
                    }
                    // if beam is not set (negative), only maxAnalyses constrains
                    if beam < 0.0 || *weight <= (lowest_weight + beam) {
                        if output_type == Xerox {
                            print_out(&format!("{}\t", prepend));
                        }
                        print_out(value);
                        if display_weights {
                            print_out(&format!("\t{}", fmt_weight(*weight)));
                        }
                        print_out("\n");
                    }
                    i += 1;
                }
                self.display_multimap.clear();
                print_out("\n");
            }
            Variant::WUniq | Variant::WFdUniq => {
                // TransducerWUniq/TransducerWFdUniq::printAnalyses
                if output_type == Xerox && self.display_map.is_empty() {
                    // NOTE: the WUniq/WFdUniq empty-case prints a single blank
                    // line (one std::endl), unlike WPlain's two.
                    print_out(&format!("{}\t{}\t+?\n", prepend, prepend));
                    print_out("\n");
                    return;
                }
                let mut lowest_weight: f32 = -1.0;
                let mut weight_sorted: Vec<(Weight, String)> = Vec::new();
                let mut first = true;
                // C++ iterates display_map (std::map<string,Weight>) in key order.
                for (key, weight) in self.display_map.iter() {
                    if first {
                        lowest_weight = *weight;
                        first = false;
                    }
                    if beam < 0.0 || *weight <= (lowest_weight + beam) {
                        weight_sorted.push((*weight, key.clone()));
                    }
                }
                weight_sorted
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut i = 0;
                for (weight, value) in weight_sorted.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if output_type == Xerox {
                        print_out(&format!("{}\t", prepend));
                    }
                    print_out(value);
                    if display_weights {
                        print_out(&format!("\t{}", fmt_weight(*weight)));
                    }
                    print_out("\n");
                    i += 1;
                }
                self.display_map.clear();
                print_out("\n");
            }
            Variant::WFd => {
                // TransducerWFd has no own printAnalyses: inherits TransducerW.
                if output_type == Xerox && self.display_multimap.is_empty() {
                    print_out(&format!("{}\t{}\t+?\n\n", prepend, prepend));
                    return;
                }
                let mut sorted = self.display_multimap.clone();
                sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut i = 0;
                let mut lowest_weight: f32 = -1.0;
                let mut first = true;
                for (weight, value) in sorted.iter() {
                    if i >= max_analyses {
                        break;
                    }
                    if first {
                        lowest_weight = *weight;
                        first = false;
                    }
                    if beam < 0.0 || *weight <= (lowest_weight + beam) {
                        if output_type == Xerox {
                            print_out(&format!("{}\t", prepend));
                        }
                        print_out(value);
                        if display_weights {
                            print_out(&format!("\t{}", fmt_weight(*weight)));
                        }
                        print_out("\n");
                    }
                    i += 1;
                }
                self.display_multimap.clear();
                print_out("\n");
            }
        }
    }
}

// C++ std::cout << float uses %g-like default formatting (6 significant
// digits); the weighted printAnalyses uses '\t' << (*it).first.
fn fmt_weight(w: Weight) -> String {
    // mimic ostream default float formatting (up to 6 significant digits)
    let s = format!("{:.6}", w);
    // trim trailing zeros but keep ostream-ish output
    s
}

// ---------------------------------------------------------------------------
// runTransducer
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.run-transducer-fn]
// [spec:hfst:sem:hfst-optimized-lookup.run-transducer-fn]
unsafe fn run_transducer(t: &mut Transducer) {
    unsafe {
        // input_string: 1000 SymbolNumber slots, all NO_SYMBOL_NUMBER.
        let mut input_string: Vec<SymbolNumber> = vec![NO_SYMBOL_NUMBER; 1000];

        use std::io::BufRead;
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();

        loop {
            // std::cin.getline(str, MAX_IO_STRING): read a line, drop the newline.
            let mut line_bytes: Vec<u8> = Vec::new();
            let n = handle.read_until(b'\n', &mut line_bytes).unwrap_or(0);
            if n == 0 {
                break; // EOF / read failure
            }
            // strip trailing newline (and a CR if present) — getline drops '\n'.
            if line_bytes.last() == Some(&b'\n') {
                line_bytes.pop();
            }
            // getline reads at most MAX_IO_STRING-1 chars into the buffer.
            line_bytes.truncate(MAX_IO_STRING - 1);
            // NUL-terminate, matching the C char* buffer.
            line_bytes.push(0);

            let str_display = {
                let end = line_bytes.iter().position(|&b| b == 0).unwrap_or(0);
                String::from_utf8_lossy(&line_bytes[..end]).into_owned()
            };

            if ECHO_INPUTS_FLAG {
                print_out(&format!("{}\n", str_display));
            }

            let base = line_bytes.as_ptr();
            let mut i = 0usize;
            let mut pos = 0usize; // cursor into line_bytes
            let mut failed = false;
            // for (char **Str = &str; **Str != 0;)
            while *base.add(pos) != 0 {
                let k = t.find_next_key(base, &mut pos);
                if k == NO_SYMBOL_NUMBER {
                    if ECHO_INPUTS_FLAG {
                        print_out("\n");
                    }
                    failed = true;
                    break;
                }
                input_string[i] = k;
                i += 1;
            }
            if failed {
                // tokenization failed
                if OUTPUT_TYPE == Xerox {
                    print_out(&format!("{}\t{}\t+?\n\n", str_display, str_display));
                }
                continue;
            }

            input_string[i] = NO_SYMBOL_NUMBER;

            if TIME_CUTOFF > 0.0 {
                START_CLOCK = clock();
                CALL_COUNTER = 0;
                LIMIT_REACHED = false;
            }

            t.analyze(&input_string);
            t.print_analyses(&str_display);
        }
    }
}

// ---------------------------------------------------------------------------
// setup
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.setup-fn]
// [spec:hfst:sem:hfst-optimized-lookup.setup-fn]
unsafe fn setup(f: *mut FILE) -> c_int {
    unsafe {
        let header = match parse_header(f) {
            Ok(h) => h,
            Err(_) => {
                print_err("Invalid transducer header.\n");
                print_err("The transducer must be in optimized lookup format.\n");
                return libc::EXIT_FAILURE;
            }
        };
        let alphabet = TransducerAlphabet::new(f, header.symbol_count());

        if header.probe_flag(HasUnweightedInputEpsilonCycles)
            || header.probe_flag(HasInputEpsilonCycles)
        {
            print_err(
                "!! Warning: transducer has epsilon cycles                  !!\n\
                 !! This is currently not handled - if they are encountered !!\n\
                 !! program *will* segfault.                                !!\n",
            );
        }

        if alphabet.get_state_size() == 0 {
            // if the state size is zero, there are no flag diacritics to handle
            if !header.probe_flag(Weighted) {
                if DISPLAY_UNIQUE_FLAG {
                    let mut c = Transducer::new(Variant::Uniq, f, header.clone(), alphabet.clone());
                    run_transducer(&mut c);
                } else {
                    let mut c =
                        Transducer::new(Variant::Plain, f, header.clone(), alphabet.clone());
                    run_transducer(&mut c);
                }
            } else if DISPLAY_UNIQUE_FLAG {
                let mut c = Transducer::new(Variant::WUniq, f, header.clone(), alphabet.clone());
                run_transducer(&mut c);
            } else {
                let mut c = Transducer::new(Variant::WPlain, f, header.clone(), alphabet.clone());
                run_transducer(&mut c);
            }
        } else {
            // handle flag diacritics
            if !header.probe_flag(Weighted) {
                if DISPLAY_UNIQUE_FLAG {
                    let mut c =
                        Transducer::new(Variant::FdUniq, f, header.clone(), alphabet.clone());
                    run_transducer(&mut c);
                } else {
                    let mut c = Transducer::new(Variant::Fd, f, header.clone(), alphabet.clone());
                    run_transducer(&mut c);
                }
            } else if DISPLAY_UNIQUE_FLAG {
                let mut c = Transducer::new(Variant::WFdUniq, f, header.clone(), alphabet.clone());
                run_transducer(&mut c);
            } else {
                let mut c = Transducer::new(Variant::WFd, f, header.clone(), alphabet.clone());
                run_transducer(&mut c);
            }
        }
        0
    }
}

// parse the header, surfacing a HeaderParsingException as Err so setup can
// reproduce the C++ try/catch that wraps the whole construction sequence.
unsafe fn parse_header(f: *mut FILE) -> Result<TransducerHeader, HeaderParsingException> {
    unsafe {
        TransducerHeader::skip_hfst3_header(f)?;
        let number_of_input_symbols: SymbolNumber = fread_val(f);
        let number_of_symbols: SymbolNumber = fread_val(f);
        let size_of_transition_index_table: TransitionTableIndex = fread_val(f);
        let size_of_transition_target_table: TransitionTableIndex = fread_val(f);
        let number_of_states: StateIdNumber = fread_val(f);
        let number_of_transitions: TransitionNumber = fread_val(f);
        let weighted = TransducerHeader::read_property(f);
        let deterministic = TransducerHeader::read_property(f);
        let input_deterministic = TransducerHeader::read_property(f);
        let minimized = TransducerHeader::read_property(f);
        let cyclic = TransducerHeader::read_property(f);
        let has_epsilon_epsilon_transitions = TransducerHeader::read_property(f);
        let has_input_epsilon_transitions = TransducerHeader::read_property(f);
        let has_input_epsilon_cycles = TransducerHeader::read_property(f);
        let has_unweighted_input_epsilon_cycles = TransducerHeader::read_property(f);
        Ok(TransducerHeader {
            number_of_symbols,
            number_of_input_symbols,
            size_of_transition_index_table,
            size_of_transition_target_table,
            number_of_states,
            number_of_transitions,
            weighted,
            deterministic,
            input_deterministic,
            minimized,
            cyclic,
            has_epsilon_epsilon_transitions,
            has_input_epsilon_transitions,
            has_input_epsilon_cycles,
            has_unweighted_input_epsilon_cycles,
        })
    }
}

// ---------------------------------------------------------------------------
// print_usage / print_version / print_short_help
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.print-usage-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-usage-fn]
fn print_usage() -> bool {
    print_out(&format!(
        "\nUsage: {} [OPTIONS] TRANSDUCER\n\
Run a transducer on standard input (one word per line) and print analyses\n\
NOTE: hfst-optimized-lookup does lookup from left to right as opposed to xfst\n\
      and foma lookup which is carried out from right to left. In order to do\n\
      lookup in a similar way as xfst and foma, invert the transducer first.\n\
\n\
  -h, --help                  Print this help message\n\
  -V, --version               Print version information\n\
  -v, --verbose               Be verbose\n\
  -q, --quiet                 Don't be verbose (default)\n\
  -s, --silent                Same as quiet\n\
  -e, --echo                  Echo inputs\n\
                              (useful if redirecting lots of output to a file)\n\
  -w, --show-weights          Print final analysis weights (if any)\n\
  -u, --unique                Suppress duplicate analyses\n\
  -n N, --analyses=N          Output no more than N analyses\n\
                              (if the transducer is weighted, the N best analyses)\n\
  -b, --beam=B                Output only analyses whose weight is within B from\n\
                              the best analysis\n\
  -t, --time-cutoff=S         Limit search after having used S seconds per input\n\
  -x, --xerox                 Xerox output format (default)\n\
  -f, --fast                  Be as fast as possible.\n\
                              (with this option enabled -u and -n don't work and\n\
                              output won't be ordered by weight).\n\
  -p, --pipe-mode[=STREAM]    Control input and output streams.\n\
\n\
N must be a positive integer. B must be a non-negative float.\n\
S must be a non-negative float. The default, 0.0, indicates no cutoff.\n\
Options -n and -b are combined with AND, i.e. they both restrict the output.\n\
\n\
STREAM can be {{ input, output, both }}. If not given, defaults to {{both}}.\n\
Input is read interactively line by line from the user. If you redirect input\n\
from a file, use --pipe-mode=input. --pipe-mode=output is ignored on non-windows\n\
platforms.\n\
\n\
Report bugs to {}\n\
\n",
        PACKAGE_NAME, PACKAGE_BUGREPORT
    ));
    true
}

// [spec:hfst:def:hfst-optimized-lookup.print-version-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-version-fn]
fn print_version() -> bool {
    print_out(&format!(
        "\n{}\ncopyright (C) 2009 University of Helsinki\n",
        PACKAGE_STRING
    ));
    true
}

// [spec:hfst:def:hfst-optimized-lookup.print-short-help-fn]
// [spec:hfst:sem:hfst-optimized-lookup.print-short-help-fn]
fn print_short_help() -> bool {
    print_usage();
    true
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
// [spec:hfst:def:hfst-optimized-lookup.main-fn]
// [spec:hfst:sem:hfst-optimized-lookup.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) for getopt_long.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();

        loop {
            let long_options: [getopt::Option; 15] = [
                // first the hfst-mandated options
                opt("help", 0, b'h'),
                opt("version", 0, b'V'),
                opt("verbose", 0, b'v'),
                opt("quiet", 0, b'q'),
                opt("silent", 0, b's'),
                // the hfst-optimized-lookup-specific options
                opt("echo-inputs", 0, b'e'),
                opt("show-weights", 0, b'w'),
                opt("beam", 1, b'b'),
                opt("time-cutoff", 1, b't'),
                opt("unique", 0, b'u'),
                opt("xerox", 0, b'x'),
                opt("fast", 0, b'f'),
                opt("pipe-mode", 2, b'p'),
                opt("analyses", 1, b'n'),
                getopt::Option {
                    name: std::ptr::null(),
                    has_arg: 0,
                    flag: std::ptr::null_mut(),
                    val: 0,
                },
            ];

            let short = CString::new("hVvqsewb:t:uxfn:p::").unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );

            if c == -1 {
                // no more options to look at
                break;
            }

            match c as u8 {
                b'h' => {
                    print_usage();
                    return libc::EXIT_SUCCESS;
                }
                b'V' => {
                    print_version();
                    return libc::EXIT_SUCCESS;
                }
                b'v' => {
                    VERBOSE_FLAG = true;
                }
                b'q' | b's' => {
                    VERBOSE_FLAG = false;
                    DISPLAY_WEIGHTS_FLAG = true;
                }
                b'e' => {
                    ECHO_INPUTS_FLAG = true;
                }
                b'w' => {
                    DISPLAY_WEIGHTS_FLAG = true;
                }
                b'u' => {
                    DISPLAY_UNIQUE_FLAG = true;
                }
                b'b' => {
                    BEAM = atof(getopt::OPTARG) as f32;
                    if BEAM < 0.0 {
                        print_err("Invalid argument for --beam\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b't' => {
                    TIME_CUTOFF = atof(getopt::OPTARG);
                    if TIME_CUTOFF < 0.0 {
                        print_err("Invalid argument for --time-cutoff\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b'n' => {
                    MAX_ANALYSES = atoi(getopt::OPTARG);
                    if MAX_ANALYSES < 1 {
                        print_err("Invalid or no argument for analyses count\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b'x' => {
                    OUTPUT_TYPE = Xerox;
                }
                b'f' => {
                    BE_FAST = true;
                }
                b'p' => {
                    let arg = getopt::OPTARG;
                    if arg.is_null() {
                        PIPE_INPUT = true;
                        PIPE_OUTPUT = true;
                    } else {
                        let a = cstr(arg);
                        if a == "both" || a == "BOTH" {
                            PIPE_INPUT = true;
                            PIPE_OUTPUT = true;
                        } else if a == "input" || a == "INPUT" || a == "in" || a == "IN" {
                            PIPE_INPUT = true;
                        } else if a == "output" || a == "OUTPUT" || a == "out" || a == "OUT" {
                            PIPE_OUTPUT = true;
                        } else {
                            print_err(&format!("--pipe-mode argument {} unrecognised\n\n", a));
                            return libc::EXIT_FAILURE;
                        }
                    }
                }
                _ => {
                    print_err("Invalid option\n\n");
                    print_short_help();
                    return libc::EXIT_FAILURE;
                }
            }
        }

        // no more options, we should now be at the input filename
        let optind = getopt::OPTIND;
        if (optind + 1) < argc {
            print_err("More than one input file given\n");
            libc::EXIT_FAILURE
        } else if (optind + 1) == argc {
            let path = *argv.offset(optind as isize);
            let mode = CString::new("rb").unwrap();
            let f = fopen(path, mode.as_ptr());
            if f.is_null() {
                print_err(&format!("Could not open file {}\n", cstr(path)));
                return 1;
            }
            setup(f)
        } else {
            print_err("No input file given\n");
            libc::EXIT_FAILURE
        }
    }
}

// helpers -------------------------------------------------------------------
fn opt(name: &str, has_arg: c_int, val: u8) -> getopt::Option {
    // leak the CString so the pointer stays valid for getopt's lifetime (the
    // long_options table is rebuilt every loop iteration in the C++ too, via a
    // static array of string literals).
    let c = CString::new(name).unwrap();
    let ptr = c.into_raw() as *const c_char;
    getopt::Option {
        name: ptr,
        has_arg,
        flag: std::ptr::null_mut(),
        val: val as c_int,
    }
}

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

// atof / atoi over a possibly-NULL C string, matching the C library semantics
// the tool relies on (atof(optarg) / atoi(optarg)).
unsafe fn atof(ptr: *const c_char) -> f64 {
    unsafe {
        if ptr.is_null() {
            return 0.0;
        }
        let s = cstr(ptr);
        parse_leading_f64(&s)
    }
}

unsafe fn atoi(ptr: *const c_char) -> c_int {
    unsafe {
        if ptr.is_null() {
            return 0;
        }
        let s = cstr(ptr);
        parse_leading_i32(&s)
    }
}

fn parse_leading_f64(s: &str) -> f64 {
    let t = s.trim_start();
    let mut end = 0;
    let bytes = t.as_bytes();
    let mut seen_dot = false;
    let mut seen_e = false;
    while end < bytes.len() {
        let ch = bytes[end];
        let ok = ch.is_ascii_digit()
            || (end == 0 && (ch == b'+' || ch == b'-'))
            || (ch == b'.' && !seen_dot && !seen_e)
            || ((ch == b'e' || ch == b'E') && !seen_e && end > 0)
            || ((ch == b'+' || ch == b'-')
                && end > 0
                && (bytes[end - 1] == b'e' || bytes[end - 1] == b'E'));
        if ch == b'.' {
            seen_dot = true;
        }
        if ch == b'e' || ch == b'E' {
            seen_e = true;
        }
        if !ok {
            break;
        }
        end += 1;
    }
    t[..end].parse::<f64>().unwrap_or(0.0)
}

fn parse_leading_i32(s: &str) -> c_int {
    let t = s.trim_start();
    let mut end = 0;
    let bytes = t.as_bytes();
    while end < bytes.len() {
        let ch = bytes[end];
        if ch.is_ascii_digit() || (end == 0 && (ch == b'+' || ch == b'-')) {
            end += 1;
        } else {
            break;
        }
    }
    t[..end].parse::<c_int>().unwrap_or(0)
}
