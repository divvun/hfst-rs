use std::collections::{BTreeMap, BTreeSet};

/// How the simulated user typed one word.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Typed {
    pub taps: usize,
    pub in_full: bool,
    /// A walk bound was reached while the word was built.
    pub truncated: bool,
    /// For each unit the user looked for, in order: whether it was the
    /// first suggestion, and whether it was among the N shown.
    pub looked_for: Vec<(bool, bool)>,
}

impl Typed {
    pub(super) fn in_full(characters: usize) -> Typed {
        Typed {
            taps: characters,
            in_full: true,
            ..Typed::default()
        }
    }
}

/// Counts over a set of words scored one way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub words: usize,
    pub characters: usize,
    pub taps: usize,
    /// Words the walk could not reach, or whose units were never offered,
    /// typed letter by letter.
    pub typed_in_full: usize,
    /// Words during which a walk bound was reached.
    pub truncated: usize,
    /// Units the user looked for. A word typed in full adds the units up to
    /// the one the user gave up on.
    pub units: usize,
    /// Units that were the first suggestion when the user looked for them.
    pub first: usize,
    /// Units that were among the N suggestions shown when the user looked
    /// for them.
    pub shown: usize,
}

impl Tally {
    fn word(characters: usize, typed: &Typed) -> Tally {
        Tally {
            words: 1,
            characters,
            taps: typed.taps,
            typed_in_full: usize::from(typed.in_full),
            truncated: usize::from(typed.truncated),
            units: typed.looked_for.len(),
            first: typed.looked_for.iter().filter(|(first, _)| *first).count(),
            shown: typed.looked_for.iter().filter(|(_, shown)| *shown).count(),
        }
    }

    /// One minus taps over characters; none when no character was typed.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    pub fn keystrokes_saved(&self) -> Option<f64> {
        (self.characters > 0).then(|| 1.0 - self.taps as f64 / self.characters as f64)
    }

    pub fn first_rate(&self) -> Option<f64> {
        (self.units > 0).then(|| self.first as f64 / self.units as f64)
    }

    pub fn shown_rate(&self) -> Option<f64> {
        (self.units > 0).then(|| self.shown as f64 / self.units as f64)
    }
}

impl std::ops::AddAssign for Tally {
    fn add_assign(&mut self, other: Tally) {
        self.words += other.words;
        self.characters += other.characters;
        self.taps += other.taps;
        self.typed_in_full += other.typed_in_full;
        self.truncated += other.truncated;
        self.units += other.units;
        self.first += other.first;
        self.shown += other.shown;
    }
}

/// The results of one variant, or of one recording from another system.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    /// False for a recording, whose suggestions are not units.
    pub ranks_units: bool,
    pub overall: Tally,
    /// By the number of units in the word's target; 0 for words without
    /// one.
    pub by_units: BTreeMap<usize, Tally>,
}

impl Row {
    pub(super) fn new(name: &str, ranks_units: bool) -> Row {
        Row {
            name: name.to_owned(),
            ranks_units,
            overall: Tally::default(),
            by_units: BTreeMap::new(),
        }
    }

    /// Add another row's counts, as of another part of the text.
    pub(super) fn absorb(&mut self, other: &Row) {
        self.overall += other.overall;
        for (&units, &tally) in &other.by_units {
            *self.by_units.entry(units).or_default() += tally;
        }
    }

    pub(super) fn add(&mut self, units: usize, characters: usize, typed: &Typed) {
        let tally = Tally::word(characters, typed);
        self.overall += tally;
        *self.by_units.entry(units).or_default() += tally;
    }
}

/// The rows of a run, all scored on the same text.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub rows: Vec<Row>,
}

/// The measures each row shows, by their column headings.
const MEASURES: [&str; 4] = ["saved", "first", "shown", "full"];
const CELL: usize = 7;
/// What a block of measures is at its narrowest.
const BLOCK: usize = MEASURES.len() * (CELL + 1) - 1;

impl Report {
    /// The rows side by side: a block of columns for each, and a line for
    /// all words and for each number of units in a word. `full` counts the
    /// words typed in full; a recording has no first or shown.
    // [spec:hfst:req:prediction-evaluation.report]
    pub fn table(&self) -> String {
        let widths: Vec<usize> = self
            .rows
            .iter()
            .map(|row| row.name.chars().count().max(BLOCK))
            .collect();
        let names = self
            .rows
            .iter()
            .map(|row| format!("{:<1$}", row.name, BLOCK));
        let headings = MEASURES
            .map(|measure| format!("{measure:>CELL$}"))
            .join(" ");
        let mut lines = vec![
            line(["", "", ""], &widths, names),
            line(
                ["units", "words", "chars"],
                &widths,
                self.rows.iter().map(|_| headings.clone()),
            ),
            self.group("all", &widths, |row| Some(row.overall)),
        ];
        let groups: BTreeSet<usize> = self
            .rows
            .iter()
            .flat_map(|row| row.by_units.keys().copied())
            .collect();
        for &units in groups.iter().filter(|&&units| units > 0) {
            lines.push(self.group(&units.to_string(), &widths, |row| {
                row.by_units.get(&units).copied()
            }));
        }
        if groups.contains(&0) {
            lines.push(self.group("no reading", &widths, |row| row.by_units.get(&0).copied()));
        }
        lines.concat()
    }

    /// One line of the table: the group's words and characters, which every
    /// row shares, then each row's measures.
    fn group(&self, name: &str, widths: &[usize], tally: impl Fn(&Row) -> Option<Tally>) -> String {
        let common = self.rows.iter().find_map(&tally).unwrap_or_default();
        let blocks = self.rows.iter().map(|row| {
            let tally = tally(row).unwrap_or_default();
            let (first, shown) = if row.ranks_units {
                (tally.first_rate(), tally.shown_rate())
            } else {
                (None, None)
            };
            [
                percent(tally.keystrokes_saved()),
                percent(first),
                percent(shown),
                tally.typed_in_full.to_string(),
            ]
            .map(|cell| format!("{cell:>CELL$}"))
            .join(" ")
        });
        let (words, characters) = (common.words.to_string(), common.characters.to_string());
        line([name, &words, &characters], widths, blocks)
    }
}

/// A line with three columns on the left and a block for each row, each
/// block right-aligned in its width, with no space at its end.
fn line(left: [&str; 3], widths: &[usize], blocks: impl Iterator<Item = String>) -> String {
    let [name, words, characters] = left;
    let mut line = format!("{name:<10} {words:>7} {characters:>8}");
    for (block, &width) in blocks.zip(widths) {
        line.push_str(&format!("   {block:>width$}"));
    }
    let mut line = line.trim_end().to_owned();
    line.push('\n');
    line
}

fn percent(rate: Option<f64>) -> String {
    rate.map_or_else(|| "-".to_owned(), |rate| format!("{:.1}%", rate * 100.0))
}
