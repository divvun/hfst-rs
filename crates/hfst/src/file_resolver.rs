//! Named files for the compilers. Every file a compile reads by name, and
//! every file an xfst command writes by name, goes through a [`FileResolver`]
//! the caller supplies. [`FsResolver`] is the default and uses the
//! filesystem; [`MemoryFiles`] keeps the files in memory, for a host that has
//! no filesystem.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::hfst_input_stream::HfstInputStream;

// [spec:hfst:req:javascript.named-file-resolver]
/// Where a compiler finds the files a source names, and where an xfst command
/// puts the files it writes.
///
/// The `Send + Sync` bound keeps a compiler that holds a resolver as `Send`
/// as it would be without one.
pub trait FileResolver: Send + Sync {
    /// The whole of the file `name`.
    fn read(&self, name: &str) -> io::Result<Vec<u8>>;

    /// The file `name` as UTF-8 text.
    fn read_to_string(&self, name: &str) -> io::Result<String> {
        String::from_utf8(self.read(name)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// A writer for the file `name`, which starts out empty.
    fn create(&self, name: &str) -> io::Result<Box<dyn io::Write>>;

    /// A writer that adds to the end of the file `name`, creating it when
    /// there is none.
    fn append(&self, name: &str) -> io::Result<Box<dyn io::Write>>;

    /// The filesystem path behind `name`, for a resolver that has one. Only
    /// a directory format such as .thfst needs it, since a directory has no
    /// bytes to [`read`](Self::read).
    fn local_path(&self, _name: &str) -> Option<PathBuf> {
        None
    }
}

/// The resolver a compiler starts with: the filesystem.
pub fn default_resolver() -> Arc<dyn FileResolver> {
    Arc::new(FsResolver)
}

/// Reads and writes the filesystem. A relative name is resolved against the
/// working directory.
#[derive(Clone, Copy, Debug, Default)]
pub struct FsResolver;

impl FileResolver for FsResolver {
    fn read(&self, name: &str) -> io::Result<Vec<u8>> {
        std::fs::read(name)
    }

    fn read_to_string(&self, name: &str) -> io::Result<String> {
        std::fs::read_to_string(name)
    }

    fn create(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        Ok(Box::new(std::fs::File::create(name)?))
    }

    fn append(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        let file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(name)?;
        Ok(Box::new(file))
    }

    fn local_path(&self, name: &str) -> Option<PathBuf> {
        Some(PathBuf::from(name))
    }
}

type FileMap = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

/// Files held in memory by name. Clones share the same files, so a caller
/// keeps one clone and hands another to a compiler. Bytes a writer writes
/// can be read back at once.
#[derive(Clone, Debug, Default)]
pub struct MemoryFiles {
    files: FileMap,
}

/// The map behind a lock. A writer that panicked mid-write leaves at worst a
/// file with part of its bytes, so a poisoned lock is still safe to use.
fn lock(files: &FileMap) -> MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
    files.lock().unwrap_or_else(PoisonError::into_inner)
}

impl MemoryFiles {
    /// Store `contents` as the file `name`, replacing any file of that name.
    pub fn insert(&self, name: impl Into<String>, contents: impl Into<Vec<u8>>) {
        lock(&self.files).insert(name.into(), contents.into());
    }

    /// The contents of the file `name`, if there is one.
    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        lock(&self.files).get(name).cloned()
    }
}

impl FileResolver for MemoryFiles {
    /// A name that only prefixes other names, as 'x.thfst' does
    /// 'x.thfst/index', is a directory.
    fn read(&self, name: &str) -> io::Result<Vec<u8>> {
        let files = lock(&self.files);
        if let Some(contents) = files.get(name) {
            return Ok(contents.clone());
        }
        let prefix = format!("{name}/");
        if files.keys().any(|k| k.starts_with(&prefix)) {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                "is a directory",
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "no such file"))
    }

    fn create(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        self.insert(name, Vec::new());
        Ok(Box::new(MemoryWriter {
            files: Arc::clone(&self.files),
            name: name.to_string(),
        }))
    }

    fn append(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        lock(&self.files).entry(name.to_string()).or_default();
        Ok(Box::new(MemoryWriter {
            files: Arc::clone(&self.files),
            name: name.to_string(),
        }))
    }
}

/// Adds every write to the end of one file in a [`MemoryFiles`].
struct MemoryWriter {
    files: FileMap,
    name: String,
}

impl io::Write for MemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        lock(&self.files)
            .entry(self.name.clone())
            .or_default()
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A transducer file a compiler opened by name: its bytes, or for a directory
/// format, the directory.
pub(crate) enum TransducerFile {
    Bytes(io::Cursor<Vec<u8>>),
    Directory(PathBuf),
}

// [spec:hfst:req:javascript.named-file-resolver]
/// The transducer file `name`. A directory is opened from its filesystem
/// path; a resolver without one cannot supply it, and the error says so.
pub(crate) fn open_transducer_file(
    files: &dyn FileResolver,
    name: &str,
) -> io::Result<TransducerFile> {
    if let Some(path) = files.local_path(name)
        && path.is_dir()
    {
        return Ok(TransducerFile::Directory(path));
    }
    match files.read(name) {
        Ok(bytes) => Ok(TransducerFile::Bytes(io::Cursor::new(bytes))),
        Err(e) if e.kind() == io::ErrorKind::IsADirectory => Err(io::Error::new(
            io::ErrorKind::IsADirectory,
            "it is a directory, and a directory format such as .thfst loads only \
             through a file resolver with filesystem paths",
        )),
        Err(e) => Err(e),
    }
}

impl TransducerFile {
    /// A stream over the transducers in this file.
    pub(crate) fn stream(&mut self) -> crate::error::Result<HfstInputStream<'_>> {
        match self {
            TransducerFile::Bytes(contents) => HfstInputStream::read_from(contents),
            TransducerFile::Directory(path) => {
                let Some(path) = path.to_str() else {
                    crate::bail!(
                        NotTransducerStream,
                        format!("'{}' is not a UTF-8 path", path.display())
                    );
                };
                HfstInputStream::new_filename(path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // [spec:hfst:req:javascript.named-file-resolver/test]
    // A created file starts empty, an appending writer adds to it, and each
    // write is readable at once.
    #[test]
    fn memory_writers_create_and_append() {
        let files = MemoryFiles::default();
        files.insert("a.txt", "old");
        let mut w = files.create("a.txt").expect("create");
        assert_eq!(files.get("a.txt").expect("created"), b"");
        w.write_all(b"one\n").expect("write");
        assert_eq!(files.read_to_string("a.txt").expect("read"), "one\n");
        let mut w = files.append("a.txt").expect("append");
        w.write_all(b"two\n").expect("write");
        assert_eq!(files.read_to_string("a.txt").expect("read"), "one\ntwo\n");
        let mut w = files.append("b.txt").expect("append to a new file");
        w.write_all(b"new").expect("write");
        assert_eq!(files.get("b.txt").expect("appended"), b"new");
    }

    // [spec:hfst:req:javascript.named-file-resolver/test]
    // A missing name and a name that only prefixes others fail differently,
    // and neither has a filesystem path.
    #[test]
    fn memory_names_missing_or_directories() {
        let files = MemoryFiles::default();
        files.insert("x.thfst/index", Vec::new());
        let missing = files.read("y.hfst").expect_err("missing");
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        let dir = files.read("x.thfst").expect_err("a directory");
        assert_eq!(dir.kind(), io::ErrorKind::IsADirectory);
        assert!(files.local_path("x.thfst").is_none());
        let Err(e) = open_transducer_file(&files, "x.thfst") else {
            panic!("a directory opened without a path");
        };
        assert!(e.to_string().contains("directory"), "{e}");
    }

    // [spec:hfst:req:javascript.named-file-resolver/test]
    // Text that is not UTF-8 is an error, not a lossy string.
    #[test]
    fn read_to_string_refuses_invalid_utf8() {
        let files = MemoryFiles::default();
        files.insert("bad.txt", vec![0xff, 0xfe]);
        let e = files.read_to_string("bad.txt").expect_err("invalid UTF-8");
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
    }

    // [spec:hfst:req:javascript.named-file-resolver/test]
    #[test]
    fn the_filesystem_resolver_has_paths() {
        assert_eq!(
            FsResolver.local_path("a/b.hfst"),
            Some(PathBuf::from("a/b.hfst"))
        );
    }
}
