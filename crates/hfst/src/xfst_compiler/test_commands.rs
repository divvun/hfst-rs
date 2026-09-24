//! The test commands: equivalence, identity, boundedness, universality,
//! emptiness, overlap and sublanguage checks on the stack.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    fn print_bool(&mut self, value: bool) -> &mut Self {
        let printval = if value { 1 } else { 0 };
        println!("{}, (1 = TRUE, 0 = FALSE)", printval);
        self.flush();
        self
    }

    // @brief Test top transducer in stack for equivalence
    // @todo tests are not implemented
    pub fn test_eq(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        if self.stack.len() < 2 {
            self.diag_warning("not enough networks on the stack: this operation needs two");
            self.xfst_lesser_fail();
            return Ok(self);
        }
        let first = *self.stack.last().expect("stack has >= 2, checked above");
        self.stack.pop();
        let second = *self
            .stack
            .last()
            .expect("stack still non-empty after one pop");
        self.stack.pop();
        let result = self.net(first).compare(self.net(second), false)?;
        self.print_bool(result);
        self.stack.push(second);
        self.stack.push(first);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        Ok(self)
    }

    // @brief Test top transducer in stack for functionality
    // @todo tests are not implemented
    pub fn test_funct(&mut self, assertion: bool) -> &mut Self {
        let _ = assertion;
        self.diag_warning("test funct is not implemented; no verdict was produced");
        self.prompt();
        self
    }

    // @brief Test top transducer in stack for identity
    // @todo tests are not implemented
    pub fn test_id(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            return Ok(self);
        };

        let mut tmp_input = HfstTransducer::new_copy(self.net(tmp))?;
        tmp_input.input_project()?;
        let mut tmp_output = HfstTransducer::new_copy(self.net(tmp))?;
        tmp_output.output_project()?;

        let result = tmp_input.compare(&tmp_output, false)?;
        self.print_bool(result);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for upper language boundedness
    // @todo tests are not implemented
    pub fn test_upper_bounded(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(temp) = self.top() else {
            return Ok(self);
        };

        let mut tmp = HfstTransducer::new_copy(self.net(temp))?;
        tmp.output_project()?;
        tmp.remove_epsilons()?; // needed for testing cyclicity

        let result = !tmp.is_cyclic()?;
        self.print_bool(result);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    pub fn test_uni(&mut self, level: Level, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(temp) = self.top() else {
            return Ok(self);
        };

        let mut tmp = HfstTransducer::new_copy(self.net(temp))?;
        tmp.input_project()?;
        let id = HfstTransducer::new_symbol(internal_identity)?;
        let mut value = false;

        if level == Level::UPPER_LEVEL {
            value = id.compare(&tmp, false)?;
        } else if level == Level::LOWER_LEVEL {
            value = !id.compare(&tmp, false)?;
        } else {
            error!("ERROR: argument given to function 'test_uni' not recognized");
        }
        self.print_bool(value);
        // MAYBE_ASSERT(assertion, value)
        if !value
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for upper language universality
    // @todo tests are not implemented
    pub fn test_upper_uni(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_uni(Level::UPPER_LEVEL, assertion)
    }

    // @brief Test top transducer in stack for lower language boundedness
    // @todo tests are not implemented
    pub fn test_lower_bounded(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(temp) = self.top() else {
            return Ok(self);
        };

        let mut tmp = HfstTransducer::new_copy(self.net(temp))?;
        tmp.input_project()?;
        tmp.remove_epsilons()?; // needed for testing cyclicity

        let result = !tmp.is_cyclic()?;
        self.print_bool(result);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for lower language universality
    // @todo tests are not implemented
    pub fn test_lower_uni(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_uni(Level::LOWER_LEVEL, assertion)
    }

    // @brief Test top transducer in stack for not emptiness
    // @todo tests are not implemented
    pub fn test_nonnull(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_null(true, assertion)
    }

    // @brief Test top transducer in stack for emptiness
    // \a invert_test_result defines whether the result is inverted
    // (so that 'test_nonnull' can be implemented with the same function).
    // @todo tests are not implemented
    pub fn test_null(
        &mut self,
        invert_test_result: bool,
        assertion: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            return Ok(self);
        };

        let empty: HfstTransducer<B> = HfstTransducer::new();
        let mut value = empty.compare(self.net(tmp), false)?;
        if invert_test_result {
            value = !value;
        }
        self.print_bool(value);

        // MAYBE_ASSERT(assertion, value)
        if !value
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Print the result of \a operation when applied to the whole stack.
    fn test_operation(
        &mut self,
        operation: TestOperation,
        assertion: bool,
    ) -> crate::error::Result<&mut Self> {
        if self.stack.len() < 2 {
            self.diag_warning("not enough networks on the stack: this operation needs two");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        // [spec:hfst:def:xfst-compiler.hfst.xfst.copied-stack-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.copied-stack-fn]
        let mut copied_stack: Vec<NetId> = self.stack.clone();

        let topmost = copied_stack
            .pop()
            .expect("stack has at least 2 networks (checked above)");
        let mut topmost_transducer = HfstTransducer::new_copy(self.net(topmost))?;

        let empty: HfstTransducer<B> = HfstTransducer::new();

        while let Some(next) = copied_stack.pop() {
            let next_transducer = HfstTransducer::new_copy(self.net(next))?;

            match operation {
                TestOperation::TEST_OVERLAP_ => {
                    topmost_transducer.intersect(&next_transducer, true)?;
                    if topmost_transducer.compare(&empty, true)? {
                        self.print_bool(false);
                        // MAYBE_ASSERT(assertion, false)
                        let value = false;
                        if !value
                            && ((self.variables["assert"] == "ON" || assertion)
                                && (self.variables["quit-on-fail"] == "ON"))
                        {
                            self.fail_flag = true;
                        }
                        self.prompt();
                        return Ok(self);
                    }
                }
                TestOperation::TEST_SUBLANGUAGE_ => {
                    // [spec:hfst:def:xfst-compiler.hfst.xfst.intersection-fn]
                    // [spec:hfst:sem:xfst-compiler.hfst.xfst.intersection-fn]
                    let mut intersection = HfstTransducer::new_copy(&topmost_transducer)?;
                    intersection.intersect(&next_transducer, true)?;
                    if !intersection.compare(&topmost_transducer, true)? {
                        self.print_bool(false);
                        // MAYBE_ASSERT(assertion, false)
                        let value = false;
                        if !value
                            && ((self.variables["assert"] == "ON" || assertion)
                                && (self.variables["quit-on-fail"] == "ON"))
                        {
                            self.fail_flag = true;
                        }
                        self.prompt();
                        return Ok(self);
                    }
                    topmost_transducer = next_transducer;
                }
            }
        }
        self.print_bool(true);
        // MAYBE_ASSERT(assertion, true)
        let value = true;
        if !value
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for overlapping
    // @todo tests are not implemented
    pub fn test_overlap(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_operation(TestOperation::TEST_OVERLAP_, assertion)
    }

    // @brief Test top transducer in stack for sublanguage
    // @todo tests are not implemented
    pub fn test_sublanguage(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_operation(TestOperation::TEST_SUBLANGUAGE_, assertion)
    }

    // @brief Test top transducer in stack for unambiguity
    // @todo tests are not implemented
    pub fn test_unambiguous(&mut self, assertion: bool) -> &mut Self {
        let _ = assertion;
        self.diag_warning("test unambiguous is not implemented; no verdict was produced");
        self.prompt();
        self
    }

    pub fn test_infinitely_ambiguous(
        &mut self,
        assertion: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            return Ok(self);
        };
        let value = self.net(tmp).is_infinitely_ambiguous()?;
        self.print_bool(value);
        // MAYBE_ASSERT(assertion, value)
        if !value
            && ((self.variables["assert"] == "ON" || assertion)
                && (self.variables["quit-on-fail"] == "ON"))
        {
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }
}
