//! The running weight, RTN call stack and recursion budget a walk moves.

use super::*;

impl PmatchContainer {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-weight-fn]
    pub fn set_weight(&mut self, w: Weight) {
        self.running_weight = w;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.increment-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.increment-weight-fn]
    pub fn increment_weight(&mut self, w: Weight) {
        self.running_weight += w;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-weight-fn]
    pub fn get_weight(&self) -> Weight {
        self.running_weight
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.increase-stack-depth-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.increase-stack-depth-fn]
    pub fn increase_stack_depth(&mut self) {
        self.stack_depth += 1;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.decrease-stack-depth-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.decrease-stack-depth-fn]
    pub fn decrease_stack_depth(&mut self) -> crate::error::Result<()> {
        if self.stack_depth == 0 {
            crate::bail!(Hfst, "pmatch: negative stack depth");
        }
        self.stack_depth -= 1;
        Ok(())
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.push-rtn-call-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.push-rtn-call-fn]
    // C++ takes 'PmatchTransducer * caller'; we take the caller's owning symbol
    // plus a copy of the caller's frame (needed to resume it, see RtnStackFrame).
    pub fn push_rtn_call(
        &mut self,
        return_index: u32,
        caller: SymbolNumber,
        caller_frame: LocalVariables,
    ) {
        let new_top = RtnStackFrame {
            caller,
            caller_index: return_index,
            caller_frame,
        };
        if self.rtn_stacks.len() <= self.stack_depth as usize {
            self.rtn_stacks.push(vec![new_top]);
        } else {
            self.rtn_stacks[self.stack_depth as usize].push(new_top);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.rtn-stack-top-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.rtn-stack-top-fn]
    pub fn rtn_stack_top(&self) -> RtnStackFrame {
        self.rtn_stacks[self.stack_depth as usize]
            .last()
            .expect("rtn stack at this depth is non-empty")
            .clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-latest-rtn-caller-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-latest-rtn-caller-fn]
    // Returns the caller's owning symbol (see push_rtn_call).
    pub fn get_latest_rtn_caller(&self) -> SymbolNumber {
        self.rtn_stacks[(self.stack_depth - 1) as usize]
            .last()
            .expect("rtn stack at this depth is non-empty")
            .caller
    }

    // The caller's frame stored alongside get_latest_rtn_caller's symbol; used to
    // resume the suspended caller when an RTN returns to it. [hfst/hfst#354]
    pub fn get_latest_caller_frame(&self) -> LocalVariables {
        self.rtn_stacks[(self.stack_depth - 1) as usize]
            .last()
            .expect("rtn stack at this depth is non-empty")
            .caller_frame
            .clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.rtn-stack-pop-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.rtn-stack-pop-fn]
    pub fn rtn_stack_pop(&mut self) {
        self.rtn_stacks[self.stack_depth as usize].pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-stack-depth-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-stack-depth-fn]
    pub fn get_stack_depth(&self) -> u32 {
        self.stack_depth
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.try-recurse-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.try-recurse-fn]
    pub fn try_recurse(&mut self) -> bool {
        if self.recursion_depth_left > 0 {
            self.recursion_depth_left -= 1;
            true
        } else {
            false
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.unrecurse-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.unrecurse-fn]
    pub fn unrecurse(&mut self) {
        self.recursion_depth_left += 1;
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.reset-recursion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.reset-recursion-fn]
    pub fn reset_recursion(&mut self) {
        self.recursion_depth_left = self.props.max_recursion as u32;
    }
}
