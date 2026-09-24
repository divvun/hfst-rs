//! Unary stream transforms: the tools that read one transducer stream,
//! apply a single algebraic or alphabet operation, and write the result.
//!
//! Contains one child module per tool:
//! - `affix_guessify`
//! - `determinize`
//! - `eliminate_flags`
//! - `insert_freely`
//! - `invert`
//! - `kill_paths`
//! - `minimize`
//! - `multiply`
//! - `preprocess_for_optimized_lookup_format`
//! - `project`
//! - `prune_alphabet`
//! - `push_labels`
//! - `push_weights`
//! - `realign`
//! - `remove_epsilons`
//! - `repeat`
//! - `reverse`

pub mod affix_guessify;
pub mod determinize;
pub mod eliminate_flags;
pub mod insert_freely;
pub mod invert;
pub mod kill_paths;
pub mod minimize;
pub mod multiply;
pub mod preprocess_for_optimized_lookup_format;
pub mod project;
pub mod prune_alphabet;
pub mod push_labels;
pub mod push_weights;
pub mod realign;
pub mod remove_epsilons;
pub mod repeat;
pub mod reverse;
