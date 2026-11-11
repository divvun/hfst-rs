//! Predictive text: a word is built a unit at a time, a stem, then
//! derivations, an ending and enclitics, from what the language's generator
//! accepts next.

pub mod entry;
pub mod model;
pub mod rank;
pub mod realise;
pub mod session;
pub mod units;
pub mod walk;
