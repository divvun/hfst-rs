//! Port of `libhfst/src/HfstEpsilonHandler.{h,cc}` — handles input-epsilon
//! cycles during lookup.

use crate::hfst_data_types::implementations::HfstState;

/// A class for handling input epsilon cycles in `lookup_fd`.
// [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler]
#[derive(Clone)]
pub struct HfstEpsilonHandler {
    // [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.hfst-state-vector]
    // the path of consecutive input epsilon transitions
    epsilon_path: HfstStateVector,
    // maximum number of consecutive epsilon cycles allowed
    max_cycles: usize,
    // number of cycles detected so far
    cycles: usize,
}

pub type HfstStateVector = Vec<HfstState>;

impl HfstEpsilonHandler {
    // [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.hfst-epsilon-handler-fn]
    // [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.hfst-epsilon-handler-fn]
    pub fn new(cutoff: usize) -> Self {
        HfstEpsilonHandler {
            epsilon_path: Vec::new(),
            max_cycles: cutoff,
            cycles: 0,
        }
    }

    // [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.push-back-fn]
    // [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.push-back-fn]
    pub fn push_back(&mut self, s: HfstState) {
        if !self.epsilon_path.is_empty() {
            if *self.epsilon_path.last().unwrap() != s {
                self.epsilon_path.push(s);
            }
        } else {
            self.epsilon_path.push(s);
        }
    }

    // [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.pop-back-fn]
    // [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.pop-back-fn]
    pub fn pop_back(&mut self) {
        if !self.epsilon_path.is_empty() {
            self.epsilon_path.pop();
        }
    }

    // [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.can-continue-fn]
    // [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.can-continue-fn]
    pub fn can_continue(&mut self, s: HfstState) -> bool {
        for i in 0..self.epsilon_path.len() {
            if self.epsilon_path[i] == s {
                // a cycle detected: erase the cycle (everything after the match)
                // and check whether the number of cycles is exceeded
                self.epsilon_path.truncate(i + 1);
                self.cycles += 1;
                if self.cycles > self.max_cycles {
                    return false;
                }
                return true;
            }
        }
        true // no cycle detected
    }
}
