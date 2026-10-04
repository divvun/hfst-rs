//! Compare two transducers word by word as weighted relations: for every
//! input word, the set of (output, minimum weight) pairs each one gives.
//!
//! Used to check `hfst regexp2fst --replace-pass` against the classic
//! `?* P ?*` compile of the same replace rule. Words come one per line on
//! standard input; each character is one symbol, or with `--spaced` each
//! space-separated token is.
//!
//! ```text
//! cargo run --release -p hfst --example replace_pass_check -- \
//!     classic.hfst pass.hfst [--spaced] [--cap N] < words.txt
//! ```
//!
//! A word whose search exceeds `--cap` live configurations (state, output
//! so far) in either transducer is skipped and counted, not compared.

use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};

const EPS: u32 = 0;
const IDENT: u32 = 1;
const UNK: u32 = 2;

/// Symbol strings shared by both transducers, so outputs compare by id.
struct Symbols {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl Symbols {
    fn new() -> Self {
        let mut s = Symbols {
            ids: HashMap::new(),
            names: Vec::new(),
        };
        for name in [internal_epsilon, internal_identity, internal_unknown] {
            s.id(name);
        }
        s
    }

    fn id(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        id
    }
}

struct Arc {
    input: u32,
    output: u32,
    weight: f32,
    target: u32,
}

struct Machine {
    arcs: Vec<Vec<Arc>>,
    finals: Vec<Option<f32>>,
    alphabet: Vec<bool>,
}

impl Machine {
    fn load(path: &str, symbols: &mut Symbols) -> hfst::error::Result<Self> {
        let mut stream = HfstInputStream::new_filename(path)?;
        let graph = stream.read()?.to_basic()?;
        let coder = graph.coder();
        let mut arcs = Vec::new();
        let mut finals = Vec::new();
        for (s, transitions) in graph.state_vector.iter().enumerate() {
            let mut out = Vec::new();
            for t in transitions {
                out.push(Arc {
                    input: symbols.id(t.get_input_symbol(coder).as_str()),
                    output: symbols.id(t.get_output_symbol(coder).as_str()),
                    weight: t.get_weight(),
                    target: t.get_target_state(),
                });
            }
            arcs.push(out);
            finals.push(graph.get_final_weight(s as u32).ok());
        }
        let mut alphabet = Vec::new();
        for name in graph.get_alphabet() {
            let id = symbols.id(name.as_str()) as usize;
            if alphabet.len() <= id {
                alphabet.resize(id + 1, false);
            }
            alphabet[id] = true;
        }
        Ok(Machine {
            arcs,
            finals,
            alphabet,
        })
    }

    fn knows(&self, symbol: u32) -> bool {
        self.alphabet.get(symbol as usize).copied().unwrap_or(false)
    }
}

/// Outputs as a trie: node 0 is the empty output.
struct Outputs {
    parent: Vec<(u32, u32)>,
    child: HashMap<(u32, u32), u32>,
}

impl Outputs {
    fn new() -> Self {
        Outputs {
            parent: vec![(0, EPS)],
            child: HashMap::new(),
        }
    }

    fn extend(&mut self, node: u32, symbol: u32) -> u32 {
        if symbol == EPS {
            return node;
        }
        if let Some(&c) = self.child.get(&(node, symbol)) {
            return c;
        }
        let c = self.parent.len() as u32;
        self.parent.push((node, symbol));
        self.child.insert((node, symbol), c);
        c
    }

    fn string(&self, mut node: u32, symbols: &Symbols) -> String {
        let mut parts = Vec::new();
        while node != 0 {
            let (p, s) = self.parent[node as usize];
            parts.push(symbols.names[s as usize].as_str());
            node = p;
        }
        parts.reverse();
        parts.concat()
    }
}

type Configs = HashMap<(u32, u32), f32>;

/// Relax `config` into `into`; true when it improved.
fn relax(into: &mut Configs, config: (u32, u32), weight: f32) -> bool {
    match into.get(&config) {
        Some(&w) if w <= weight => false,
        _ => {
            into.insert(config, weight);
            true
        }
    }
}

/// Every (output, minimum weight) for `word`, or `None` past `cap`.
fn outputs_of(
    m: &Machine,
    word: &[u32],
    cap: usize,
    trie: &mut Outputs,
) -> Option<BTreeMap<u32, f32>> {
    let mut configs: Configs = HashMap::new();
    configs.insert((0, 0), 0.0);
    let mut created = 1usize;
    for step in 0..=word.len() {
        // Epsilon closure.
        let mut work: Vec<(u32, u32)> = configs.keys().copied().collect();
        while let Some((s, node)) = work.pop() {
            let w = configs[&(s, node)];
            for a in &m.arcs[s as usize] {
                if a.input != EPS {
                    continue;
                }
                let out = if a.output == EPS {
                    node
                } else {
                    trie.extend(node, a.output)
                };
                if relax(&mut configs, (a.target, out), w + a.weight) {
                    work.push((a.target, out));
                    created += 1;
                    if created > cap {
                        return None;
                    }
                }
            }
        }
        if step == word.len() {
            break;
        }
        let c = word[step];
        let unknown = !m.knows(c);
        let mut next: Configs = HashMap::new();
        for (&(s, node), &w) in &configs {
            for a in &m.arcs[s as usize] {
                let output = if a.input == c {
                    a.output
                } else if unknown && a.input == IDENT {
                    c
                } else if unknown && a.input == UNK {
                    a.output
                } else {
                    continue;
                };
                let out = trie.extend(node, output);
                if relax(&mut next, (a.target, out), w + a.weight) {
                    created += 1;
                    if created > cap {
                        return None;
                    }
                }
            }
        }
        configs = next;
    }
    let mut result = BTreeMap::new();
    for (&(s, node), &w) in &configs {
        if let Some(f) = m.finals[s as usize] {
            let total = w + f;
            let e = result.entry(node).or_insert(f32::INFINITY);
            if total < *e {
                *e = total;
            }
        }
    }
    Some(result)
}

fn same_weight(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-3 * (1.0 + a.abs().max(b.abs()))
}

fn main() -> hfst::error::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();
    let mut spaced = false;
    let mut cap = 300_000usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--spaced" => spaced = true,
            "--cap" => {
                i += 1;
                cap = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| hfst::err!(Hfst, "--cap needs a number"))?;
            }
            other => files.push(other.to_string()),
        }
        i += 1;
    }
    let [classic, pass] = files.as_slice() else {
        hfst::bail!(
            Hfst,
            "usage: replace_pass_check CLASSIC PASS [--spaced] [--cap N] < words"
        );
    };
    let mut symbols = Symbols::new();
    let a = Machine::load(classic, &mut symbols)?;
    let b = Machine::load(pass, &mut symbols)?;
    let (mut words, mut skipped, mut outputs, mut differing) = (0usize, 0usize, 0usize, 0usize);
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| hfst::err!(Hfst, format!("reading words: {e}")))?;
        let word: Vec<u32> = if spaced {
            line.split_whitespace().map(|t| symbols.id(t)).collect()
        } else {
            line.chars().map(|c| symbols.id(&c.to_string())).collect()
        };
        if word.is_empty() {
            continue;
        }
        words += 1;
        // One output trie per word, so memory follows the largest word, not
        // the sum of all of them.
        let mut trie = Outputs::new();
        let (Some(x), Some(y)) = (
            outputs_of(&a, &word, cap, &mut trie),
            outputs_of(&b, &word, cap, &mut trie),
        ) else {
            skipped += 1;
            continue;
        };
        outputs += x.len();
        let agree = x.len() == y.len()
            && x.iter()
                .all(|(k, &w)| y.get(k).is_some_and(|&v| same_weight(w, v)));
        if !agree {
            differing += 1;
            if differing <= 10 {
                println!("DIFF {line:?}");
                for (k, w) in &x {
                    if !y.get(k).is_some_and(|&v| same_weight(*w, v)) {
                        println!(
                            "  classic {:?} {w} pass {:?}",
                            trie.string(*k, &symbols),
                            y.get(k)
                        );
                    }
                }
                for (k, w) in &y {
                    if !x.contains_key(k) {
                        println!("  pass-only {:?} {w}", trie.string(*k, &symbols));
                    }
                }
            }
        }
    }
    println!(
        "words {words} compared {} skipped {skipped} outputs {outputs} differing {differing}",
        words - skipped
    );
    Ok(())
}
