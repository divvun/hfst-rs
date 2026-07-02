//! Shared line-oriented I/O helpers used by the text-format readers.

use std::io::BufRead;

/// Read one line from `reader`: everything up to and including the next
/// newline. The trailing newline (if any) is kept, matching C `fgets`.
/// Returns `None` at EOF, when no bytes at all could be read. Invalid UTF-8
/// is replaced lossily.
pub fn read_line_lossy(reader: &mut dyn BufRead) -> Option<String> {
    let mut buf: Vec<u8> = Vec::new();
    match reader.read_until(b'\n', &mut buf) {
        Ok(0) => None,
        // An I/O error mid-line yields the bytes read so far, as the original
        // C `fgets` loop did.
        Ok(_) | Err(_) => {
            if buf.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(&buf).into_owned())
            }
        }
    }
}
