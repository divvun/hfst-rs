//! Options objects from the wrapper, read a field at a time.

use hfst_embed::Code;
use wasm_bindgen::prelude::*;

use crate::error::GlueError;

/// An options object from the wrapper. `undefined` and `null`, for the
/// object or any field, mean the option was not given.
pub struct Options<'a>(Option<&'a JsValue>);

impl<'a> Options<'a> {
    pub fn new(options: &'a JsValue) -> Result<Self, GlueError> {
        if options.is_undefined() || options.is_null() {
            return Ok(Options(None));
        }
        if !options.is_object() {
            return Err(GlueError::new(
                Code::InvalidArgument,
                "the options are an object",
            ));
        }
        Ok(Options(Some(options)))
    }

    pub fn get(&self, name: &str) -> Result<Option<JsValue>, GlueError> {
        let Some(options) = self.0 else {
            return Ok(None);
        };
        let value = js_sys::Reflect::get(options, &JsValue::from_str(name)).map_err(|_| {
            GlueError::new(
                Code::InvalidArgument,
                format!("the option {name} cannot be read"),
            )
        })?;
        Ok((!value.is_undefined() && !value.is_null()).then_some(value))
    }

    pub fn flag(&self, name: &str) -> Result<Option<bool>, GlueError> {
        self.get(name)?
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    GlueError::new(
                        Code::InvalidArgument,
                        format!("the option {name} is a boolean"),
                    )
                })
            })
            .transpose()
    }

    pub fn number(&self, name: &str) -> Result<Option<f64>, GlueError> {
        self.get(name)?
            .map(|value| {
                value.as_f64().ok_or_else(|| {
                    GlueError::new(
                        Code::InvalidArgument,
                        format!("the option {name} is a number"),
                    )
                })
            })
            .transpose()
    }
}
