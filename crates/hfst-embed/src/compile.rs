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
//! Natively every compile, xfst run and operation runs on a thread of its
//! own with the stack [`hfst::depth::COMPILE_STACK_SIZE`] gives, as the tools
//! do, so the library's depth limits hold whatever stack the caller is on: a
//! Python thread's, or a default Rust thread's. The thread is joined before
//! the call returns. On wasm32, which has no threads, they run where they are
//! called, under the lower limits [`hfst::depth`] sets for it.
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
pub use transducer::{Operand, Transducer, WriteOptions, read_thfst, read_transducers};
pub use writers::*;

// [spec:hfst:req:embed.compile-files]
/// Where a compile reads the files its source names and an xfst script
/// writes: the filesystem, files held in memory, or a caller's own.
pub use hfst::file_resolver::{FileResolver, FsResolver, MemoryFiles};

#[cfg(test)]
mod tests;
