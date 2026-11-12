//! `hfst xfst`: the session a script runs in.

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::FromAnyTransducer;
use crate::xfst_compiler::{CmdResult, XfstCompiler};

/// `hfst xfst`'s options that change what a script does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XfstOptions {
    /// Print weights where words are printed: `--print-weight`.
    pub print_weight: bool,
    /// Read and write only in the working directory, and run no shell
    /// command: `--restricted-mode`.
    pub restricted_mode: bool,
    /// Print no command output: `--silent`.
    pub silent: bool,
}

impl XfstOptions {
    // [spec:hfst:req:embed.tools-in-library]
    /// A session set as `hfst xfst` sets one with these options, before it
    /// attaches its standard streams.
    pub fn session<B: AlgebraBackend + FromAnyTransducer>(&self) -> CmdResult<XfstCompiler<B>> {
        let mut session = XfstCompiler::<B>::new();
        session.set_verbosity(!self.silent);
        if self.print_weight {
            // The tool leaves prompts on after setting the variable, so a
            // script run with `--print-weight` prints them.
            session.set_prompt_verbosity(false);
            session.set("print-weight", "ON")?;
            session.set_prompt_verbosity(true);
        }
        if self.restricted_mode {
            session.set_restricted_mode(true);
        }
        Ok(session)
    }
}
