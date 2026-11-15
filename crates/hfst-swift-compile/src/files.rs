//! Where a compile reads the files its source names and an xfst script
//! writes: the filesystem, files held in memory, or a caller's resolver.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use hfst_embed::compile::{FileResolver, FsResolver, MemoryFiles};

// [spec:hfst:req:swift.compile-files]
/// A caller's resolver: the file `name`'s bytes, or `None` for a file it
/// does not have. The library calls it from the thread the compile runs on.
#[uniffi::export(with_foreign)]
pub trait FileSource: Send + Sync {
    fn read(&self, name: String) -> Option<Vec<u8>>;
}

enum Kind {
    FileSystem,
    /// Files held in memory, where whatever the compile writes goes too,
    /// and behind them, the caller's resolver, asked once for each name the
    /// memory does not hold.
    Given {
        memory: MemoryFiles,
        source: Option<Arc<dyn FileSource>>,
    },
}

// [spec:hfst:req:swift.compile-files]
/// The files of one compile. Each is made for the call that uses it, so what
/// one compile writes is not another's.
#[derive(uniffi::Object)]
pub struct Files {
    kind: Kind,
}

#[uniffi::export]
impl Files {
    #[uniffi::constructor]
    pub fn file_system() -> Arc<Files> {
        Arc::new(Files {
            kind: Kind::FileSystem,
        })
    }

    #[uniffi::constructor]
    pub fn memory(files: HashMap<String, Vec<u8>>) -> Arc<Files> {
        let memory = MemoryFiles::default();
        for (name, contents) in files {
            memory.insert(name, contents);
        }
        Arc::new(Files {
            kind: Kind::Given {
                memory,
                source: None,
            },
        })
    }

    #[uniffi::constructor]
    pub fn resolver(source: Arc<dyn FileSource>) -> Arc<Files> {
        Arc::new(Files {
            kind: Kind::Given {
                memory: MemoryFiles::default(),
                source: Some(source),
            },
        })
    }
}

impl Files {
    /// The resolver the library reads and writes through.
    pub fn resolver_for(&self) -> Arc<dyn FileResolver> {
        match &self.kind {
            Kind::FileSystem => Arc::new(FsResolver),
            Kind::Given { memory, source } => Arc::new(Given {
                memory: memory.clone(),
                source: source.clone(),
            }),
        }
    }

    /// The contents of the file `name` the compile wrote, from memory or the
    /// filesystem, as it wrote it.
    pub fn written(&self, name: &str) -> io::Result<Vec<u8>> {
        match &self.kind {
            Kind::FileSystem => std::fs::read(name),
            Kind::Given { memory, .. } => memory
                .get(name)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, not_given(name))),
        }
    }
}

fn not_given(name: &str) -> String {
    format!("{name}: no such file among the files the compile was given")
}

/// Files held in memory, behind which a caller's resolver may stand. The
/// memory is a clone of the `Files`' own, which shares its contents, so
/// what the compile writes is there to read back.
struct Given {
    memory: MemoryFiles,
    source: Option<Arc<dyn FileSource>>,
}

impl Given {
    /// The file `name` from memory, or else from the caller's resolver, kept
    /// in memory so it is asked once.
    fn fetch(&self, name: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.memory.get(name) {
            return Some(bytes);
        }
        let bytes = self.source.as_ref()?.read(name.to_owned())?;
        self.memory.insert(name, bytes.clone());
        Some(bytes)
    }
}

impl FileResolver for Given {
    fn read(&self, name: &str) -> io::Result<Vec<u8>> {
        self.fetch(name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, not_given(name)))
    }

    fn create(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        self.memory.create(name)
    }

    fn append(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        // A file appended to that the resolver has starts from its bytes.
        let _ = self.fetch(name);
        self.memory.append(name)
    }
}
