//! Port of `libhfst/src/FormatSpecifiers.h` — printf length-modifier strings.
//!
//! The non-MSVC/MinGW branch is taken (the platform target for this port). The
//! MSVC/MinGW branch used `"%Iu"`/`"%Id"`/`"%I64d"`.

pub const SIZE_T_SPECIFIER: &str = "%zu";
pub const SSIZE_T_SPECIFIER: &str = "%zd";
pub const PTRDIFF_T_SPECIFIER: &str = "%zd";
pub const LONG_LONG_SPECIFIER: &str = "%lld";
