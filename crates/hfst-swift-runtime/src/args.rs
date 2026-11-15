//! The numbers callers pass, checked before the library runs, as
//! `@divvun/hfst` checks them.

use crate::error::{GlueError, invalid};

// [spec:hfst:req:swift.errors]
/// A result limit, which `@divvun/hfst` takes as a whole number, 0 or more.
pub fn limit(limit: Option<i64>) -> Result<Option<usize>, GlueError> {
    match limit {
        None => Ok(None),
        Some(n) if n < 0 => Err(invalid(format!(
            "a lookup limit is a whole number, 0 or more, not {n}"
        ))),
        // A limit past what the library counts in is no limit, since nothing
        // finds that many results.
        Some(n) => Ok(Some(usize::try_from(n).unwrap_or(usize::MAX))),
    }
}

// [spec:hfst:req:swift.errors]
/// A time limit in seconds: finite, 0 or more, 0 or `None` for none.
pub fn seconds(what: &str, seconds: Option<f64>) -> Result<Option<f64>, GlueError> {
    match seconds {
        Some(s) if !(s.is_finite() && s >= 0.0) => Err(invalid(format!(
            "{what} is a finite number of seconds, 0 or more, not {s}"
        ))),
        given => Ok(given),
    }
}
