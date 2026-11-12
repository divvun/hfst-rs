//! The transducers of a call that takes several, as the wrapper names them.

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use hfst_embed::compile::Transducer as Machine;
use hfst_wasm_shared::GlueError;
use wasm_bindgen::prelude::*;

use crate::transducer::Transducer;

/// Transducers named for one call, in order: a lookup's stream, the rules
/// of a pair test or a compose-intersect. wasm-bindgen cannot lend a call
/// an array of objects, so the wrapper makes a list, pushes each transducer
/// and frees the list once the call returns. The list shares each machine
/// rather than copying it.
#[wasm_bindgen]
pub struct TransducerList {
    items: Vec<Rc<RefCell<Machine>>>,
}

#[wasm_bindgen]
impl TransducerList {
    #[wasm_bindgen(constructor)]
    #[expect(
        clippy::new_without_default,
        reason = "JavaScript makes a list with new, and Rust never makes one but in tests"
    )]
    pub fn new() -> TransducerList {
        TransducerList { items: Vec::new() }
    }

    pub fn push(&mut self, transducer: &Transducer) {
        self.items.push(Rc::clone(&transducer.machine));
    }
}

impl TransducerList {
    pub(crate) fn items(&self) -> &[Rc<RefCell<Machine>>] {
        &self.items
    }

    /// Every transducer of the list, each borrowed to read. A transducer
    /// named twice is borrowed twice, which reading allows.
    pub(crate) fn reading(&self) -> Result<Vec<Ref<'_, Machine>>, GlueError> {
        self.items
            .iter()
            .map(|machine| {
                machine.try_borrow().map_err(|_| {
                    GlueError::new(
                        hfst_embed::Code::InvalidArgument,
                        "a transducer of the list is in use by a call that has not returned",
                    )
                })
            })
            .collect()
    }
}
