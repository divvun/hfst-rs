//! What the `hfst` tools do to a transducer, for the packages: compile it,
//! transform it, inspect it and write it, over every type the library reads.
//!
//! [`Transducer`] holds a transducer of any type. Each operation the tools
//! offer is a method that changes it in place, each query a method that
//! returns what the tool reports as data, and each writer a method that
//! returns the bytes the tool writes. They call the library functions the
//! tools call, so for the same input and options a package gets what the tool
//! writes.
//!
//! None of this is linked without the `compile` feature.

mod compilers;
mod operations;
mod queries;
mod transducer;
mod writers;

pub use compilers::*;
pub use operations::*;
pub use queries::*;
pub use transducer::{Operand, Transducer, WriteOptions, read_transducers};
pub use writers::*;

#[cfg(test)]
mod tests;
