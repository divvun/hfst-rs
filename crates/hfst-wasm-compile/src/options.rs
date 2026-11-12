//! The options objects the wrapper passes a call, read a field at a time and
//! checked as the hfst tool checks the flags they stand for.
//!
//! An options object is copied out of JavaScript once, into [`Value`]s, and
//! read from there, so everything a call does with its options is plain Rust
//! that host tests run without a JavaScript engine.

use hfst_embed::Code;
use hfst_embed::format::Format;
use hfst_wasm_shared::GlueError;
use wasm_bindgen::{JsCast, JsValue};

fn invalid(message: String) -> GlueError {
    GlueError::new(Code::InvalidArgument, message)
}

/// A field of an options object, as the glue reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    /// A plain object's own fields, in their order.
    Record(Vec<(String, Value)>),
    /// Anything else, by what `typeof` calls it.
    Other(String),
}

impl Value {
    /// `value` copied out of JavaScript; `None` for `undefined`, which
    /// stands for a field not given.
    pub fn from_js(value: &JsValue) -> Option<Value> {
        if value.is_undefined() {
            return None;
        }
        Some(if value.is_null() {
            Value::Null
        } else if let Some(b) = value.as_bool() {
            Value::Bool(b)
        } else if let Some(n) = value.as_f64() {
            Value::Number(n)
        } else if let Some(text) = value.as_string() {
            Value::Text(text)
        } else if let Some(bytes) = value.dyn_ref::<js_sys::Uint8Array>() {
            Value::Bytes(bytes.to_vec())
        } else if let Some(list) = value.dyn_ref::<js_sys::Array>() {
            Value::List(
                list.iter()
                    .filter_map(|item| Value::from_js(&item))
                    .collect(),
            )
        } else if value.is_object() && !value.is_function() {
            Value::Record(entries(value))
        } else {
            Value::Other(value.js_typeof().as_string().unwrap_or_default())
        })
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Value {
        Value::Number(n)
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Value {
        Value::Text(text.to_owned())
    }
}

fn text_of(value: &Value) -> Option<String> {
    if let Value::Text(text) = value {
        Some(text.clone())
    } else {
        None
    }
}

/// The own fields of the object `value`, but those that are `undefined`.
fn entries(value: &JsValue) -> Vec<(String, Value)> {
    let Some(object) = value.dyn_ref::<js_sys::Object>() else {
        return Vec::new();
    };
    js_sys::Object::entries(object)
        .iter()
        .filter_map(|entry| {
            let entry = js_sys::Array::from(&entry);
            let value = Value::from_js(&entry.get(1))?;
            Some((entry.get(0).as_string().unwrap_or_default(), value))
        })
        .collect()
}

/// The options of one call. A field left out, or `undefined`, was not given.
#[derive(Debug)]
pub struct Options<'a> {
    call: &'a str,
    fields: Vec<(String, Value)>,
}

impl<'a> Options<'a> {
    /// The options object `options` of `call`. `undefined` and `null` give
    /// none.
    pub fn new(call: &'a str, options: &JsValue) -> Result<Self, GlueError> {
        if options.is_undefined() || options.is_null() {
            return Ok(Options::of(call, Vec::new()));
        }
        if !options.is_object() || options.is_function() || js_sys::Array::is_array(options) {
            return Err(invalid(format!("{call} takes its options as an object")));
        }
        Ok(Options::of(call, entries(options)))
    }

    pub fn of(call: &'a str, fields: Vec<(String, Value)>) -> Self {
        Options { call, fields }
    }

    /// Refuses an option `known` does not list, as the tool refuses a flag
    /// it does not know.
    pub fn only(&self, known: &[&str]) -> Result<(), GlueError> {
        let Some((key, _)) = self
            .fields
            .iter()
            .find(|(key, _)| !known.contains(&key.as_str()))
        else {
            return Ok(());
        };
        let options = if known.is_empty() {
            "it has none".to_owned()
        } else {
            format!("its options are {}", known.join(", "))
        };
        Err(invalid(format!(
            "{} has no option called {key}; {options}",
            self.call
        )))
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    fn wrong(&self, name: &str, expected: &str) -> GlueError {
        invalid(format!("{}'s option {name} is {expected}", self.call))
    }

    pub fn flag(&self, name: &str, default: bool) -> Result<bool, GlueError> {
        match self.get(name) {
            None => Ok(default),
            Some(Value::Bool(b)) => Ok(*b),
            Some(_) => Err(self.wrong(name, "a boolean")),
        }
    }

    /// A flag left out to keep a setting as it is.
    pub fn maybe_flag(&self, name: &str) -> Result<Option<bool>, GlueError> {
        self.get(name).map(|_| self.flag(name, false)).transpose()
    }

    pub fn text(&self, name: &str) -> Result<Option<String>, GlueError> {
        match self.get(name) {
            None => Ok(None),
            Some(Value::Text(text)) => Ok(Some(text.clone())),
            Some(_) => Err(self.wrong(name, "a string")),
        }
    }

    /// A file's text or bytes: a string, or a `Uint8Array`.
    pub fn contents(&self, name: &str) -> Result<Option<Vec<u8>>, GlueError> {
        match self.get(name) {
            None => Ok(None),
            Some(Value::Text(text)) => Ok(Some(text.clone().into_bytes())),
            Some(Value::Bytes(bytes)) => Ok(Some(bytes.clone())),
            Some(_) => Err(self.wrong(name, "a string or a Uint8Array")),
        }
    }

    pub fn texts(&self, name: &str) -> Result<Option<Vec<String>>, GlueError> {
        let wrong = || self.wrong(name, "an array of strings");
        match self.get(name) {
            None => Ok(None),
            Some(Value::List(items)) => items
                .iter()
                .map(|item| text_of(item).ok_or_else(wrong))
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            Some(_) => Err(wrong()),
        }
    }

    /// An object of strings, as pairs of each key and its value.
    pub fn record(&self, name: &str) -> Result<Option<Vec<(String, String)>>, GlueError> {
        let wrong = || self.wrong(name, "an object of strings");
        match self.get(name) {
            None => Ok(None),
            Some(Value::Record(fields)) => fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), text_of(value).ok_or_else(wrong)?)))
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            Some(_) => Err(wrong()),
        }
    }

    /// A number that `accept` takes, `expected` saying which.
    pub fn number(
        &self,
        name: &str,
        expected: &str,
        accept: impl Fn(f64) -> bool,
    ) -> Result<Option<f64>, GlueError> {
        match self.get(name) {
            None => Ok(None),
            Some(Value::Number(n)) if accept(*n) => Ok(Some(*n)),
            Some(_) => Err(self.wrong(name, expected)),
        }
    }

    /// A finite number, which the library holds as an f32 weight.
    pub fn weight(&self, name: &str) -> Result<Option<f32>, GlueError> {
        Ok(self
            .number(name, "a finite number", f64::is_finite)?
            .map(|n| n as f32))
    }

    /// A finite number of seconds, 0 or more, given in milliseconds as the
    /// package's time limits are.
    pub fn seconds(&self, name: &str) -> Result<Option<f64>, GlueError> {
        Ok(self
            .number(name, "a finite number of milliseconds, 0 or more", |n| {
                n.is_finite() && n >= 0.0
            })?
            .map(|ms| ms / 1000.0))
    }

    /// A whole number of at least `least`. A count past what `u64` holds is
    /// `u64::MAX`: every count it bounds is smaller.
    pub fn count(&self, name: &str, least: u64) -> Result<Option<u64>, GlueError> {
        let expected = format!("a whole number, {least} or more");
        Ok(self
            .number(name, &expected, |n| {
                n >= least as f64 && (n.fract() == 0.0 || n == f64::INFINITY)
            })?
            .map(|n| {
                if n >= u64::MAX as f64 {
                    u64::MAX
                } else {
                    n as u64
                }
            }))
    }

    pub fn usize(&self, name: &str, least: u64) -> Result<Option<usize>, GlueError> {
        Ok(self
            .count(name, least)?
            .map(|n| usize::try_from(n).unwrap_or(usize::MAX)))
    }

    /// A whole number the library takes as an `i32`, as the tool's flag
    /// does: a count past `i32::MAX` is `i32::MAX`.
    pub fn int(&self, name: &str, least: u64) -> Result<Option<i32>, GlueError> {
        Ok(self
            .count(name, least)?
            .map(|n| i32::try_from(n).unwrap_or(i32::MAX)))
    }

    /// One of the names `table` lists, as the value it stands for.
    pub fn choice<T: Copy>(&self, name: &str, table: &[(&str, T)]) -> Result<Option<T>, GlueError> {
        let Some(given) = self.text(name)? else {
            return Ok(None);
        };
        if let Some((_, value)) = table.iter().find(|(known, _)| *known == given) {
            return Ok(Some(*value));
        }
        let names: Vec<String> = table.iter().map(|(n, _)| format!("\"{n}\"")).collect();
        Err(self.wrong(
            name,
            &format!("one of {}, not \"{given}\"", names.join(", ")),
        ))
    }

    pub fn format(&self, allowed: &[Format], default: Format) -> Result<Format, GlueError> {
        let table: Vec<(&str, Format)> = allowed.iter().map(|f| (f.name(), *f)).collect();
        Ok(self.choice("format", &table)?.unwrap_or(default))
    }

    /// Refuses two flags that ask for opposite things.
    pub fn not_both(&self, first: &str, second: &str) -> Result<(), GlueError> {
        if self.flag(first, false)? && self.flag(second, false)? {
            return Err(invalid(format!(
                "{} takes {first} or {second}, not both",
                self.call
            )));
        }
        Ok(())
    }
}

pub fn format_named(call: &str, name: &str) -> Result<Format, GlueError> {
    Format::ALL
        .iter()
        .find(|f| f.name() == name)
        .copied()
        .ok_or_else(|| {
            let names: Vec<String> = Format::ALL
                .iter()
                .map(|f| format!("\"{}\"", f.name()))
                .collect();
            invalid(format!(
                "{call} takes a transducer type, one of {}, not \"{name}\"",
                names.join(", ")
            ))
        })
}

/// Options built in Rust, as a host test passes them.
#[cfg(test)]
pub fn given<'a>(call: &'a str, fields: &[(&str, Value)]) -> Options<'a> {
    let fields = fields
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect();
    Options::of(call, fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(fields: &[(&str, Value)]) -> Result<Options<'static>, GlueError> {
        let o = given("call", fields);
        o.only(&["a", "b"])?;
        Ok(o)
    }

    #[test]
    fn unknown_options_are_refused_by_name() {
        let error = options(&[("a", true.into()), ("c", true.into())]).expect_err("refused");
        assert_eq!(error.code(), "invalid-argument");
        assert_eq!(
            error.message(),
            "call has no option called c; its options are a, b"
        );
        let none = given("bare", &[("x", Value::Null)])
            .only(&[])
            .expect_err("refused");
        assert!(
            none.message().ends_with("it has none"),
            "{}",
            none.message()
        );
    }

    #[test]
    fn fields_are_read_as_their_type() {
        let o = options(&[("a", true.into()), ("b", "text".into())]).expect("known");
        assert!(o.flag("a", false).expect("a boolean"));
        assert!(!o.flag("missing", false).expect("the default"));
        assert_eq!(o.text("b").expect("a string").as_deref(), Some("text"));
        assert_eq!(
            o.flag("b", false).err().map(|e| e.code()),
            Some("invalid-argument")
        );
        assert_eq!(
            o.text("a").err().map(|e| e.message().to_owned()).as_deref(),
            Some("call's option a is a string")
        );
        assert_eq!(o.maybe_flag("missing").expect("left out"), None);
        assert_eq!(o.maybe_flag("a").expect("given"), Some(true));
    }

    #[test]
    fn numbers_are_checked_as_the_tools_check_them() {
        let o = options(&[("a", 3.0.into()), ("b", (-1.0).into())]).expect("known");
        assert_eq!(o.count("a", 0).expect("whole"), Some(3));
        assert!(o.count("b", 0).is_err(), "negative");
        assert_eq!(o.seconds("a").expect("ms"), Some(0.003));
        assert!(o.seconds("b").is_err(), "negative time");
        assert_eq!(o.weight("b").expect("a weight"), Some(-1.0));
        let o = options(&[("a", 1.5.into()), ("b", f64::INFINITY.into())]).expect("known");
        assert!(o.count("a", 0).is_err(), "not whole");
        assert_eq!(o.count("b", 0).expect("infinite"), Some(u64::MAX));
        assert_eq!(o.int("b", 0).expect("capped"), Some(i32::MAX));
        assert!(o.weight("b").is_err(), "not finite");
    }

    #[test]
    fn choices_name_what_they_take() {
        let o = options(&[("a", "up".into())]).expect("known");
        let error = o
            .choice("a", &[("input", 1), ("output", 2)])
            .expect_err("refused");
        assert_eq!(
            error.message(),
            "call's option a is one of \"input\", \"output\", not \"up\""
        );
        let o = options(&[("a", "output".into())]).expect("known");
        assert_eq!(
            o.choice("a", &[("input", 1), ("output", 2)])
                .expect("known"),
            Some(2)
        );
        assert_eq!(
            format_named("toBytes", "thfst").err().map(|e| e.code()),
            Some("invalid-argument")
        );
        assert_eq!(format_named("toBytes", "foma").ok(), Some(Format::Foma));
    }

    #[test]
    fn lists_records_and_contents_hold_their_parts() {
        let o = options(&[
            ("a", Value::List(vec!["x".into(), "y".into()])),
            ("b", Value::Record(vec![("k".to_owned(), "v".into())])),
        ])
        .expect("known");
        assert_eq!(
            o.texts("a").expect("strings"),
            Some(vec!["x".to_owned(), "y".to_owned()])
        );
        assert_eq!(
            o.record("b").expect("record"),
            Some(vec![("k".to_owned(), "v".to_owned())])
        );
        assert!(
            o.texts("b").is_err() && o.record("a").is_err(),
            "each is its own shape"
        );
        let o = options(&[("a", Value::Bytes(vec![1, 2])), ("b", "t".into())]).expect("known");
        assert_eq!(o.contents("a").expect("bytes"), Some(vec![1, 2]));
        assert_eq!(o.contents("b").expect("text"), Some(b"t".to_vec()));
        let both = options(&[("a", true.into()), ("b", true.into())]).expect("known");
        assert!(both.not_both("a", "b").is_err());
    }
}
