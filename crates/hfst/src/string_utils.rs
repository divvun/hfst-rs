//! Port of 'libhfst/src/string-utils.{h,cc}' — string manipulation utilities.

/// \brief Replace all occurrences of 'needle' in 'haystack' with 'replacement',
/// moving the cursor past the replacement each time (so if 'needle' occurs in
/// 'replacement', it won't be replaced).
///
/// Mirrors the C++ byte-offset 'find'/'replace' loop. The advance is by
/// 'replacement_len', computed on the input — preserving the original behaviour
/// (and its empty-'needle' non-termination) exactly.
pub fn replace_all<'a>(
    haystack: &'a mut String,
    needle: &str,
    replacement: &str,
) -> &'a mut String {
    let needle_len = needle.len();
    let replacement_len = replacement.len();
    let mut last_needle = haystack.find(needle);
    while let Some(pos) = last_needle {
        haystack.replace_range(pos..pos + needle_len, replacement);
        last_needle = haystack[pos + replacement_len..]
            .find(needle)
            .map(|i| i + pos + replacement_len);
    }
    haystack
}
