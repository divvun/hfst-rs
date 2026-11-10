//! The numbers callers pass, checked as Python checks a typed parameter.

use pyo3::exceptions::{PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBool;

/// One argument or field, named for the errors it raises: `call` is how
/// Python names the call it went to, such as `lookup()` or
/// `TokeniseOptions()`. Each reader takes `None` for the value left out.
pub struct Value<'a, 'py> {
    pub call: &'a str,
    pub name: &'a str,
    pub value: &'a Bound<'py, PyAny>,
}

impl Value<'_, '_> {
    pub fn wrong_type(&self, expected: &str) -> PyErr {
        let given = self
            .value
            .get_type()
            .name()
            .map_or_else(|_| "object".to_owned(), |n| n.to_string());
        PyTypeError::new_err(format!(
            "{} argument '{}' must be {expected}, not {given}",
            self.call, self.name
        ))
    }

    fn wrong_value(&self, expected: &str) -> PyErr {
        PyValueError::new_err(format!(
            "{} argument '{}' must be {expected}, not {}",
            self.call, self.name, self.value
        ))
    }

    /// `True` or `False`. A truthy value of another type is refused, so a
    /// misplaced number cannot pass for a flag.
    pub fn flag(&self) -> PyResult<Option<bool>> {
        if self.value.is_none() {
            return Ok(None);
        }
        self.value
            .cast::<PyBool>()
            .map(|b| Some(b.is_true()))
            .map_err(|_| self.wrong_type("bool"))
    }

    /// A whole number of at least `least`. A number past `u64::MAX` is
    /// `u64::MAX`: every count it bounds is smaller. A bool is refused,
    /// although Python counts it as an int.
    pub fn count(&self, least: u64) -> PyResult<Option<u64>> {
        if self.value.is_none() {
            return Ok(None);
        }
        let expected = format!("int, {least} or more");
        if self.value.is_instance_of::<PyBool>() {
            return Err(self.wrong_type("int"));
        }
        match self.value.extract::<u64>() {
            Ok(n) if n >= least => Ok(Some(n)),
            Ok(_) => Err(self.wrong_value(&expected)),
            Err(e) if e.is_instance_of::<PyOverflowError>(self.value.py()) => {
                if self.value.lt(0)? {
                    Err(self.wrong_value(&expected))
                } else {
                    Ok(Some(u64::MAX))
                }
            }
            Err(_) => Err(self.wrong_type("int")),
        }
    }

    /// A number, int or float, that `accept` takes: `expected` says which.
    /// A bool is refused, although Python counts it as an int.
    pub fn number(&self, expected: &str, accept: impl Fn(f64) -> bool) -> PyResult<Option<f64>> {
        if self.value.is_none() {
            return Ok(None);
        }
        if self.value.is_instance_of::<PyBool>() {
            return Err(self.wrong_type("int or float"));
        }
        let number = self
            .value
            .extract::<f64>()
            .map_err(|_| self.wrong_type("int or float"))?;
        if accept(number) {
            Ok(Some(number))
        } else {
            Err(self.wrong_value(expected))
        }
    }
}

/// An optional argument, `None` when it was left out.
pub fn given<'a, 'py>(
    call: &'a str,
    name: &'a str,
    value: Option<&'a Bound<'py, PyAny>>,
) -> Option<Value<'a, 'py>> {
    value.map(|value| Value { call, name, value })
}

/// The `limit` of a lookup: a whole number of results, 0 or more.
pub fn limit(call: &str, value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<usize>> {
    let Some(value) = given(call, "limit", value) else {
        return Ok(None);
    };
    Ok(value
        .count(0)?
        .map(|n| usize::try_from(n).unwrap_or(usize::MAX)))
}

/// The `time_limit` of a lookup: a finite number of seconds, 0 or more.
pub fn time_limit(call: &str, value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<f64>> {
    let Some(value) = given(call, "time_limit", value) else {
        return Ok(None);
    };
    value.number("a finite number of seconds, 0 or more", |s| {
        s.is_finite() && s >= 0.0
    })
}
