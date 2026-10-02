//! The test commands: equivalence, identity, boundedness, universality,
//! emptiness, overlap and sublanguage checks on the stack.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    fn print_bool(&self, value: bool) {
        println!("{}, (1 = TRUE, 0 = FALSE)", u8::from(value));
        self.flush();
    }

    /// Print a test's verdict, and fail when it is false under an
    /// assertion (the 'assert' prefix or the 'assert' variable).
    pub(super) fn report_test(&mut self, value: bool, assertion: bool) -> CmdResult {
        self.print_bool(value);
        if !value && (assertion || self.variables["assert"] == "ON") {
            return Err(CommandError::new("assertion failed: the test is false"));
        }
        self.prompt();
        Ok(())
    }

    // @brief Test top transducer in stack for equivalence
    pub fn test_eq(&mut self, assertion: bool) -> CmdResult {
        self.require_two()?;
        let first = self.stack[self.stack.len() - 1];
        let second = self.stack[self.stack.len() - 2];
        let result = self.net(first).compare(self.net(second), false)?;
        self.report_test(result, assertion)
    }

    // @brief Test top transducer in stack for identity
    pub fn test_id(&mut self, assertion: bool) -> CmdResult {
        let tmp = self.top()?;

        let mut tmp_input = HfstTransducer::new_copy(self.net(tmp))?;
        tmp_input.input_project()?;
        let mut tmp_output = HfstTransducer::new_copy(self.net(tmp))?;
        tmp_output.output_project()?;

        let result = tmp_input.compare(&tmp_output, false)?;
        self.report_test(result, assertion)
    }

    // @brief Test top transducer in stack for upper language boundedness
    pub fn test_upper_bounded(&mut self, assertion: bool) -> CmdResult {
        let temp = self.top()?;

        let mut tmp = HfstTransducer::new_copy(self.net(temp))?;
        tmp.output_project()?;
        tmp.remove_epsilons()?; // needed for testing cyclicity

        let result = !tmp.is_cyclic()?;
        self.report_test(result, assertion)
    }

    /// Whether one side of the top network is the universal language: its
    /// projection equals '?*'.
    pub fn test_uni(&mut self, level: Level, assertion: bool) -> CmdResult {
        let temp = self.top()?;
        let mut side = HfstTransducer::new_copy(self.net(temp))?;
        match level {
            Level::UPPER_LEVEL => side.input_project()?,
            Level::LOWER_LEVEL => side.output_project()?,
            Level::BOTH_LEVELS => unreachable!("universality is tested on one side"),
        };
        let mut universal = HfstTransducer::new_symbol(internal_identity)?;
        universal.repeat_star()?;
        let value = side.compare(&universal, false)?;
        self.report_test(value, assertion)
    }

    // @brief Test top transducer in stack for upper language universality
    pub fn test_upper_uni(&mut self, assertion: bool) -> CmdResult {
        self.test_uni(Level::UPPER_LEVEL, assertion)
    }

    // @brief Test top transducer in stack for lower language boundedness
    pub fn test_lower_bounded(&mut self, assertion: bool) -> CmdResult {
        let temp = self.top()?;

        let mut tmp = HfstTransducer::new_copy(self.net(temp))?;
        tmp.input_project()?;
        tmp.remove_epsilons()?; // needed for testing cyclicity

        let result = !tmp.is_cyclic()?;
        self.report_test(result, assertion)
    }

    // @brief Test top transducer in stack for lower language universality
    pub fn test_lower_uni(&mut self, assertion: bool) -> CmdResult {
        self.test_uni(Level::LOWER_LEVEL, assertion)
    }

    // @brief Test top transducer in stack for not emptiness
    pub fn test_nonnull(&mut self, assertion: bool) -> CmdResult {
        self.test_null(true, assertion)
    }

    // @brief Test top transducer in stack for emptiness
    // \a invert_test_result defines whether the result is inverted
    // (so that 'test_nonnull' can be implemented with the same function).
    pub fn test_null(&mut self, invert_test_result: bool, assertion: bool) -> CmdResult {
        let tmp = self.top()?;

        let empty: HfstTransducer<B> = HfstTransducer::new();
        let mut value = empty.compare(self.net(tmp), false)?;
        if invert_test_result {
            value = !value;
        }
        self.report_test(value, assertion)
    }

    // @brief Print the result of \a operation when applied to the whole stack.
    fn test_operation(&mut self, operation: TestOperation, assertion: bool) -> CmdResult {
        self.require_two()?;
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
                        return self.report_test(false, assertion);
                    }
                }
                TestOperation::TEST_SUBLANGUAGE_ => {
                    // [spec:hfst:def:xfst-compiler.hfst.xfst.intersection-fn]
                    // [spec:hfst:sem:xfst-compiler.hfst.xfst.intersection-fn]
                    let mut intersection = HfstTransducer::new_copy(&topmost_transducer)?;
                    intersection.intersect(&next_transducer, true)?;
                    if !intersection.compare(&topmost_transducer, true)? {
                        return self.report_test(false, assertion);
                    }
                    topmost_transducer = next_transducer;
                }
            }
        }
        self.report_test(true, assertion)
    }

    // @brief Test top transducer in stack for overlapping
    pub fn test_overlap(&mut self, assertion: bool) -> CmdResult {
        self.test_operation(TestOperation::TEST_OVERLAP_, assertion)
    }

    // @brief Test top transducer in stack for sublanguage
    pub fn test_sublanguage(&mut self, assertion: bool) -> CmdResult {
        self.test_operation(TestOperation::TEST_SUBLANGUAGE_, assertion)
    }

    pub fn test_infinitely_ambiguous(&mut self, assertion: bool) -> CmdResult {
        let tmp = self.top()?;
        let value = self.net(tmp).is_infinitely_ambiguous()?;
        self.report_test(value, assertion)
    }
}
