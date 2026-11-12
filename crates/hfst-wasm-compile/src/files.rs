//! The files a compile reads and an xfst script writes: those the caller
//! gave in the compile's options, and none other.
//!
//! The wrapper hands the files over as name and content pairs, which it
//! makes of a record or a `Map`, or as the caller's function. A function is
//! asked for each name the compile reads that it has not given yet, while
//! the compile runs: on wasm32 a compile runs where it is called, so the
//! function is called on the caller's own thread. The resolver a compiler
//! holds must be `Send` and `Sync`, which a JavaScript value is not, so the
//! function waits in a thread-local slot for the length of the compile.

use std::cell::RefCell;
use std::io;

use hfst_embed::Code;
use hfst_embed::compile::{FileResolver, MemoryFiles};
use hfst_wasm_shared::GlueError;
use wasm_bindgen::{JsCast, JsValue};

use crate::failure::Failure;

/// The caller's files function while a compile that was given one runs, and
/// what it threw, if anything.
struct Asked {
    function: js_sys::Function,
    thrown: Option<JsValue>,
}

thread_local! {
    static ASKED: RefCell<Option<Asked>> = const { RefCell::new(None) };
}

/// The files of one compile. Each file it reads comes from the pairs it was
/// given or from the caller's function, and each file it writes lands here,
/// where the compile reads it back and its result finds it.
#[derive(Clone)]
pub struct Files {
    memory: MemoryFiles,
    asks: bool,
}

/// A file's content, given as text or bytes.
fn file_bytes(value: &JsValue) -> Option<Vec<u8>> {
    match value.as_string() {
        Some(text) => Some(text.into_bytes()),
        None => value
            .dyn_ref::<js_sys::Uint8Array>()
            .map(js_sys::Uint8Array::to_vec),
    }
}

fn not_given(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{name}: no such file among the files the compile was given"),
    )
}

// [spec:hfst:req:javascript.compile-files]
/// Asks the caller's function for `name`: `None` when it gives `undefined`.
/// What it throws, or a value that is not a file, is kept for the compile to
/// throw once it is over, and fails this read. The slot is not held while
/// the function runs, so a function that calls into the module again finds
/// it free.
fn ask(name: &str) -> io::Result<Option<Vec<u8>>> {
    let function = ASKED.with(|slot| {
        let slot = slot.borrow();
        slot.as_ref()
            .map(|asked| asked.thrown.is_none().then(|| asked.function.clone()))
    });
    let function = match function {
        None => return Ok(None),
        Some(None) => return Err(io::Error::other("the files function threw")),
        Some(Some(function)) => function,
    };
    let keep = |thrown: JsValue| {
        ASKED.with(|slot| {
            if let Some(asked) = slot.borrow_mut().as_mut() {
                asked.thrown.get_or_insert(thrown);
            }
        });
    };
    let given = match function.call1(&JsValue::UNDEFINED, &JsValue::from_str(name)) {
        Ok(given) => given,
        Err(thrown) => {
            keep(thrown);
            return Err(io::Error::other("the files function threw"));
        }
    };
    if given.is_undefined() {
        return Ok(None);
    }
    match file_bytes(&given) {
        Some(bytes) => Ok(Some(bytes)),
        None => {
            let message = format!(
                "the files function gave {} for {name}, not a string, a Uint8Array or undefined",
                given.js_typeof().as_string().unwrap_or_default()
            );
            keep(GlueError::new(Code::InvalidArgument, message).into());
            Err(io::Error::other("the files function gave no file"))
        }
    }
}

impl Files {
    // [spec:hfst:req:javascript.compile-files]
    /// The files `given`: `undefined` for none, an array of name and content
    /// pairs, or the caller's function.
    pub fn new(given: &JsValue) -> Result<Files, GlueError> {
        if given.is_undefined() {
            return Ok(Files::of(Vec::new()));
        }
        if given.is_function() {
            return Ok(Files {
                memory: MemoryFiles::default(),
                asks: true,
            });
        }
        let invalid = || {
            GlueError::new(
                Code::InvalidArgument,
                "the files are pairs of a name and a string or a Uint8Array",
            )
        };
        let pairs = given
            .dyn_ref::<js_sys::Array>()
            .ok_or_else(invalid)?
            .iter()
            .map(|pair| {
                let pair = pair.dyn_into::<js_sys::Array>().map_err(|_| invalid())?;
                let name = pair.get(0).as_string().ok_or_else(invalid)?;
                Ok((name, file_bytes(&pair.get(1)).ok_or_else(invalid)?))
            })
            .collect::<Result<Vec<_>, GlueError>>()?;
        Ok(Files::of(pairs))
    }

    /// The files `pairs`, each a name and its content, and no others.
    pub fn of(pairs: Vec<(String, Vec<u8>)>) -> Files {
        let memory = MemoryFiles::default();
        for (name, bytes) in pairs {
            memory.insert(name, bytes);
        }
        Files {
            memory,
            asks: false,
        }
    }

    /// Runs `compile` with the caller's function, when the files are one,
    /// where reads can ask it. What it threw is thrown in place of the
    /// compile's own failure.
    pub fn asking<T>(
        &self,
        given: &JsValue,
        compile: impl FnOnce() -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let Some(function) = given.dyn_ref::<js_sys::Function>().filter(|_| self.asks) else {
            return compile();
        };
        let outer = ASKED.with(|slot| {
            slot.borrow_mut().replace(Asked {
                function: function.clone(),
                thrown: None,
            })
        });
        let compiled = compile();
        let asked = ASKED.with(|slot| std::mem::replace(&mut *slot.borrow_mut(), outer));
        match asked.and_then(|asked| asked.thrown) {
            Some(thrown) => Err(Failure::Thrown(thrown)),
            None => compiled,
        }
    }
}

impl FileResolver for Files {
    fn read(&self, name: &str) -> io::Result<Vec<u8>> {
        match self.memory.read(name) {
            Ok(bytes) => return Ok(bytes),
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            Err(_) => {}
        }
        if self.asks
            && let Some(bytes) = ask(name)?
        {
            self.memory.insert(name, bytes.clone());
            return Ok(bytes);
        }
        Err(not_given(name))
    }

    fn create(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        self.memory.create(name)
    }

    fn append(&self, name: &str) -> io::Result<Box<dyn io::Write>> {
        if self.memory.get(name).is_none()
            && self.asks
            && let Some(bytes) = ask(name)?
        {
            self.memory.insert(name, bytes);
        }
        self.memory.append(name)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;

    use super::*;

    fn files() -> Files {
        Files::of(vec![("words.txt".to_owned(), b"cat\n".to_vec())])
    }

    // [spec:hfst:req:javascript.compile-files/test]
    #[test]
    fn reads_only_the_given_files() {
        let files = files();
        assert_eq!(files.read("words.txt").expect("given"), b"cat\n");
        let missing = files.read("other.txt").expect_err("not given");
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        assert!(missing.to_string().contains("other.txt"), "{missing}");
        assert!(
            Files::of(Vec::new()).read("words.txt").is_err(),
            "no files, no filesystem"
        );
    }

    // [spec:hfst:req:javascript.compile-files/test]
    #[test]
    fn written_files_read_back() {
        let files = files();
        let resolver: Arc<dyn FileResolver> = Arc::new(files.clone());
        let mut out = resolver.create("out.txt").expect("created");
        out.write_all(b"one\n").expect("written");
        drop(out);
        let mut more = resolver.append("out.txt").expect("appended");
        more.write_all(b"two\n").expect("written");
        drop(more);
        assert_eq!(resolver.read("out.txt").expect("read back"), b"one\ntwo\n");
        assert_eq!(files.read("out.txt").expect("found"), b"one\ntwo\n");
        let mut fresh = resolver.append("words.txt").expect("appended");
        fresh.write_all(b"dog\n").expect("written");
        drop(fresh);
        assert_eq!(files.read("words.txt").expect("found"), b"cat\ndog\n");
    }
}
