//! Interactive inspection of the top network: inspect-net and view.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-level-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-level-fn]
    fn print_level(&mut self, whole_path: &[u32], shortest_path: &[u32]) {
        print!("Level {}", whole_path.len() as i32);
        if shortest_path.len() < whole_path.len() {
            print!(" (= {})", shortest_path.len() as i32);
        }
        self.flush();
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.can-level-be-reached-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.can-level-be-reached-fn]
    fn can_level_be_reached(&mut self, level: i32, whole_path_length: usize) -> bool {
        // EOF is -1
        if level == -1 || level == 0 {
            println!("could not read level number (type '0' if you wish to exit program)");
            self.flush();
            return false;
        } else if level < 0 || level > whole_path_length as i32 {
            println!(
                "no such level: '{}' (current level is {})",
                level, whole_path_length as i32
            );
            self.flush();
            return false;
        }
        true
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.can-arc-be-followed-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.can-arc-be-followed-fn]
    fn can_transition_be_followed(&mut self, number: i32, number_of_transitions: u32) -> bool {
        // EOF is -1
        if number == -1 || number == 0 {
            println!("could not read arc number");
            self.flush();
            return false;
        } else if number < 1 || number > number_of_transitions as i32 {
            if number_of_transitions < 1 {
                println!("state has no arcs");
            } else {
                println!("arc number must be between 1 and {}", number_of_transitions);
            }
            self.flush();
            return false;
        }
        true
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-arcs-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-arcs-fn]
    fn print_transitions(
        &mut self,
        transitions: &HfstBasicTransitions,
        coder: &SymbolCoder,
    ) -> u32 {
        let mut first_loop = true;
        let mut arc_number: u32 = 1;
        for transition in transitions.iter() {
            if first_loop {
                print!("Arcs:");
                first_loop = false;
            } else {
                print!(", ");
            }
            self.flush();
            let isymbol = transition.get_input_symbol(coder);
            let osymbol = transition.get_output_symbol(coder);

            if isymbol == osymbol {
                print!(" {}. {}", arc_number, isymbol);
            } else {
                print!(" {}. {}:{}", arc_number, isymbol, osymbol);
            }
            self.flush();
            arc_number += 1;
        }
        println!();
        self.flush();
        arc_number - 1
    }

    // @brief View top network
    pub fn view_net(&mut self) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        let dotfilename = format!(
            "{}/hfst_view_dot_{}",
            std::env::temp_dir().to_string_lossy(),
            std::process::id()
        );
        let pngfilename = format!(
            "{}/hfst_view_png_{}",
            std::env::temp_dir().to_string_lossy(),
            std::process::id()
        );
        if self.verbose {
            debug!(
                "Writing net in dot format to temporary file '{}'.",
                dotfilename
            );
        }
        {
            let mut dotfile = match std::fs::File::create(&dotfilename) {
                Ok(f) => f,
                Err(_) => {
                    self.prompt();
                    return self;
                }
            };
            crate::hfst_print_dot::print_dot_os(&mut dotfile, self.net_mut(tmp));
        }
        if self.verbose {
            debug!("Wrote net, closing file and converting into png format.");
        }
        let cmd1 = format!("dot -Tpng {} > {} 2> /dev/null", dotfilename, pngfilename);
        if run_shell(&cmd1) != 0 {
            self.diag_error_with_notes(
                "could not render the network to png",
                &[String::from("'view' needs graphviz 'dot' on PATH")],
            );
            self.xfst_lesser_fail();
        }
        if self.verbose {
            debug!("Converted to png format, viewing the graph.");
        }
        let cmd2 = format!("/usr/bin/xdg-open {} 2> /dev/null &", pngfilename);
        if run_shell(&cmd2) != 0 {
            self.diag_error_with_notes(
                "could not open the rendered network",
                &[String::from("'view' needs 'xdg-open' on PATH")],
            );
            self.xfst_lesser_fail();
        }
        self.prompt();
        self
    }

    // @brief Interactive network traversal tool
    pub fn inspect_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(t) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        let net = HfstBasicTransducer::from_transducer(self.net(t));

        const INSPECT_NET_HELP_MSG: &str =
            "'N' transits arc N, '-N' returns to level N, '<' to previous level, '0' quits.\n";
        print!("{}", INSPECT_NET_HELP_MSG);

        // path of states visited, can contain loops
        let mut whole_path: Vec<u32> = Vec::new();
        // shortest path of states to current state, no loops
        let mut shortest_path: Vec<u32> = Vec::new();

        Self::append_state_to_paths(&mut whole_path, &mut shortest_path, 0);
        self.print_level(&whole_path, &shortest_path);

        if net.is_final_state(0) {
            print!(" (final)");
        }

        println!();

        // transitions of current state
        let mut transitions: HfstBasicTransitions = net.index(0)?.clone();
        // number of arcs in current state
        let mut number_of_arcs = self.print_transitions(&transitions, net.coder());

        // index after which the history added during inspect_net is ignored
        let ind = self.current_history_index();

        // the while loop begins, keep on reading from user
        while let Some(line) = self.read_prompted_line("") {
            // case (1): back to previous state
            if line == "<\n" || line == "<" {
                if whole_path.len() < 2 {
                    self.ignore_history_after_index(ind);
                    self.prompt();
                    return Ok(self);
                } else {
                    let __lvl = (whole_path.len() - 1) as u32;
                    if !Self::return_to_level(&mut whole_path, &mut shortest_path, __lvl) {
                        error!("FATAL ERROR: could not return to level '{}'", __lvl as i32);
                        self.ignore_history_after_index(ind);
                        self.prompt();
                        return Ok(self);
                    }
                }
            }
            // case (2): back to state number N
            else if line.as_bytes().first() == Some(&b'-') {
                let level = Self::parse_int(&line[1..]); // skip '-'
                if !self.can_level_be_reached(level, whole_path.len()) {
                    continue;
                } else if !Self::return_to_level(&mut whole_path, &mut shortest_path, level as u32)
                {
                    error!("FATAL ERROR: could not return to level '{}'", level);
                    self.ignore_history_after_index(ind);
                    self.prompt();
                    return Ok(self);
                }
            }
            // case (3): exit program
            else if line == "0\n" || line == "0" {
                self.ignore_history_after_index(ind);
                self.prompt();
                return Ok(self);
            }
            // case (4): follow arc
            else {
                let number = Self::parse_int(&line); // FIX: atoi is not portable
                if !self.can_transition_be_followed(number, number_of_arcs) {
                    continue;
                } else {
                    let tr = transitions[(number - 1) as usize].clone();
                    print!(
                        "  {}:{} --> ",
                        tr.get_input_symbol(net.coder()),
                        tr.get_output_symbol(net.coder())
                    );
                    Self::append_state_to_paths(
                        &mut whole_path,
                        &mut shortest_path,
                        tr.get_target_state(),
                    );
                }
            }

            // update transitions and number of arcs and print information about
            // current level
            transitions = net
                .index(*whole_path.last().expect("path is non-empty"))?
                .clone();
            self.print_level(&whole_path, &shortest_path);
            if net.is_final_state(*whole_path.last().expect("path is non-empty")) {
                print!(" (final)");
            }
            println!();
            number_of_arcs = self.print_transitions(&transitions, net.coder());
        } // end of while loop

        self.ignore_history_after_index(ind);
        self.prompt();
        Ok(self)
    }

    // For 'inspect_net': append state \a state to paths.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.append-state-to-paths-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.append-state-to-paths-fn]
    fn append_state_to_paths(whole_path: &mut Vec<u32>, shortest_path: &mut Vec<u32>, state: u32) {
        whole_path.push(state);
        let mut idx: Option<usize> = None;
        for (i, it) in shortest_path.iter().enumerate() {
            if *it == state {
                idx = Some(i);
                break;
            }
        }
        if let Some(i) = idx {
            shortest_path.truncate(i);
        }
        shortest_path.push(state);
    }

    // For 'inspect_net': return to level \a level.
    // Return whether the operation succeeded.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.return-to-level-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.return-to-level-fn]
    fn return_to_level(
        whole_path: &mut Vec<u32>,
        shortest_path: &mut Vec<u32>,
        level: u32,
    ) -> bool {
        if (whole_path.len() as u32) < level || level == 0 {
            return false;
        }

        whole_path.truncate(level as usize);
        let state = *whole_path
            .last()
            .expect("truncated to level >= 1, so non-empty");
        let mut idx: Option<usize> = None;
        for (i, it) in shortest_path.iter().enumerate() {
            if *it == state {
                idx = Some(i);
                break;
            }
        }
        if let Some(i) = idx {
            shortest_path.truncate(i);
        }
        shortest_path.push(state);
        true
    }

    // Leading-prefix integer parse (the C 'atoi' shape) used by 'inspect_net'
    // to parse user input.
    pub(super) fn parse_int(s: &str) -> i32 {
        crate::string_manipulation::parse_int_prefix(s.as_bytes(), 0).0
    }
}
