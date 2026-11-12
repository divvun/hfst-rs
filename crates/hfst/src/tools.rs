//! What the `hfst` tools decide, as library functions. Each tool is a process
//! around these: it parses its arguments, opens its files and streams, calls
//! the function, and prints what comes back. A program that calls the same
//! function with the same input gets what the tool writes.
// [spec:hfst:def:embed.tools]
// [spec:hfst:req:embed.tools-in-library]

#[cfg(feature = "bhfst")]
pub mod bhfst;
pub mod check_alpha;
pub mod compare;
pub mod compose_intersect;
pub mod conversion;
pub mod dhfst;
pub mod dump_alphabets;
pub mod format;
pub mod fst2strings;
pub mod fst2txt;
pub mod grep;
pub mod guess;
pub mod kill_paths;
pub mod lexc;
pub mod lookup;
pub mod metadata;
pub mod operations;
pub mod optimized_lookup;
pub mod pair_test;
pub mod pmatch;
pub mod regexp2fst;
pub mod reweight;
pub mod strings2fst;
pub mod substitute;
pub mod summarize;

#[cfg(all(test, feature = "bhfst"))]
mod bhfst_tests;
#[cfg(test)]
mod compiler_tests;
#[cfg(test)]
mod operation_tests;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod writer_tests;
