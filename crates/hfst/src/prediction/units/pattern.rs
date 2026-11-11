use serde::Deserialize;

/// A unit grammar pattern. It matches a whole tag: `*` matches any run of
/// characters, the empty run included, and every other character matches
/// itself.
#[derive(Clone, Debug, Deserialize)]
#[serde(from = "String")]
pub(super) struct Pattern {
    source: String,
    /// The text between the stars. There is always one more piece than
    /// there are stars.
    pieces: Vec<String>,
}

impl From<String> for Pattern {
    fn from(source: String) -> Self {
        let pieces = source.split('*').map(str::to_owned).collect();
        Pattern { source, pieces }
    }
}

impl Pattern {
    pub(super) fn as_str(&self) -> &str {
        &self.source
    }

    pub(super) fn matches(&self, tag: &str) -> bool {
        let Some((first, after)) = self.pieces.split_first() else {
            return false;
        };
        let Some((last, middle)) = after.split_last() else {
            return tag == first;
        };
        let Some(mut rest) = tag.strip_prefix(first.as_str()) else {
            return false;
        };
        // Taking each middle piece at its leftmost place leaves the most room
        // for the pieces after it, so a match is never missed.
        for piece in middle {
            let Some(at) = rest.find(piece.as_str()) else {
                return false;
            };
            // `at` is a character boundary and the piece is whole UTF-8, so
            // the slice cannot split a character.
            rest = &rest[at + piece.len()..];
        }
        rest.ends_with(last.as_str())
    }
}

pub(super) fn any_matches(patterns: &[Pattern], tag: &str) -> bool {
    patterns.iter().any(|pattern| pattern.matches(tag))
}
