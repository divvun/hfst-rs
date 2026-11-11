//! `hfst pmatch`: matching a pmatch container against input text, and the
//! pieces of input and the matches the tool reports.

use crate::pmatch::{Location, LocationVectorVector};

/// Splits input into the pieces `hfst pmatch` matches one at a time: each
/// line, or with `blankline_separated`, each run of lines up to an empty
/// one. A piece loses its final newline.
pub struct InputSplitter {
    blankline_separated: bool,
    pending: String,
}

fn without_final_newline(mut piece: String) -> String {
    if piece.ends_with('\n') {
        piece.pop();
    }
    piece
}

impl InputSplitter {
    pub fn new(blankline_separated: bool) -> Self {
        InputSplitter {
            blankline_separated,
            pending: String::new(),
        }
    }

    // [spec:hfst:def:hfst-pmatch.process-input-fn]
    // [spec:hfst:sem:hfst-pmatch.process-input-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Take the next line of input, newline included, and give back the
    /// piece it ends, if it ends one.
    pub fn push_line(&mut self, line: &str) -> Option<String> {
        if !self.blankline_separated {
            Some(without_final_newline(line.to_string()))
        } else if line.is_empty() || line.starts_with('\n') {
            Some(without_final_newline(std::mem::take(&mut self.pending)))
        } else {
            self.pending.push_str(line);
            None
        }
    }

    /// The last piece, when the input did not end it with an empty line.
    pub fn finish(self) -> Option<String> {
        (self.blankline_separated && !self.pending.is_empty())
            .then(|| without_final_newline(self.pending))
    }

    // [spec:hfst:req:embed.tools-in-library]
    /// The pieces of `text`, in order.
    pub fn split(text: &str, blankline_separated: bool) -> Vec<String> {
        let mut splitter = InputSplitter::new(blankline_separated);
        let mut pieces: Vec<String> = text
            .split_inclusive('\n')
            .filter_map(|line| splitter.push_line(line))
            .collect();
        pieces.extend(splitter.finish());
        pieces
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// The matches `hfst pmatch --locate` reports: the first location of each
/// vector that matched something.
pub fn reported_locations(locations: &LocationVectorVector) -> impl Iterator<Item = &Location> {
    locations
        .iter()
        .filter_map(|it| it.first())
        .filter(|first| first.output != "@_NONMATCHING_@")
}
