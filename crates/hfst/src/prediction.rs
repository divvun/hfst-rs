//! Predictive text: a word is built a unit at a time, a stem, then
//! derivations, an ending and enclitics, from what the language's generator
//! accepts next.

pub mod model;
pub mod rank;
pub mod units;
