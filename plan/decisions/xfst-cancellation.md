---
id [dec:hfst:xfst-cancellation]
epitome "Ctrl-C in the xfst shell cancels the running command cooperatively: the host owns an interrupt flag set from SIGINT, the session runs each command inside a scoped, thread-local interrupt in the rustfst fork, the long loops poll it every 1024 units of work and fail with Interrupted, and a set flag fails the command whatever it returned, so the per-command undo always puts the session back."
state @decided
category @executive
scope {
    elements ()
    rules ([spec:hfst:req:xfst-cmd.shell-cancel] [spec:hfst:req:xfst-cmd.failed-command-no-effect])
}
author "brendan@necessary.nu"
alternatives (
    {
        option "A process-wide static interrupt flag in the hfst library, read by every loop."
        rejected_because "The library holds no global mutable state ([dec:hfst:idiomatize-staging], idiom5), and two sessions in one process, such as a host running several compilers, would cancel each other."
    }
    {
        option "Run each command on a worker thread and abandon it on Ctrl-C."
        rejected_because "A Rust thread cannot be killed. The abandoned thread keeps burning CPU and memory, and it holds the session mutably, so the shell could not go on using the stack anyway."
    }
    {
        option "Fork a child process per command and kill it on Ctrl-C."
        rejected_because "Every result would have to be serialized back into the parent, large stacks would be copied on every fork, and it does not exist on Windows."
    }
    {
        option "Carry an Arc<AtomicBool> in EngineConfig."
        rejected_because "EngineConfig is Copy and is copied at about 30 call sites (`let cfg = self.engine_config;`) into the budget plumbing. A borrowed `&'static AtomicBool` shares the flag just as well and keeps Copy; the host leaks one flag for the life of the process."
    }
    {
        option "Thread a Copy handle through EngineConfig and every rustfst config (DeterminizeConfig, compose options, minimize, rm_epsilon) to the loops."
        rejected_because "The first version of this decision. Building it showed the regex, lexc and twolc compilers call rustfst from dozens of sites, many through EngineConfig::default() or no config at all, so threading would touch them all and still miss some. A scope set around the command reaches every rustfst call the command makes, by any path."
    }
)
consequences {
    accepted (
        "The session holds the flag as `Option<&'static AtomicBool>` (XfstCompiler::set_interrupt) and runs each command inside `rustfst::interrupt::scope(flag, ..)`, a thread-local that the call restores on the way out, even on panic. Outside a scope nothing polls. The hfst library still holds no global state: the scope is per thread and per call."
        "The loops poll with `rustfst::interrupt::check_every`, a relaxed load every 1024 steps, which is what bounds the time to stop: rustfst's LazyFst expansion (behind determinize, compose, closure, concat and lazy epsilon removal), the cyclic minimizer and static epsilon removal, and in hfst-openfst the compose materializer, its trim pass and the equivalence walk. A tripped poll fails with rustfst::interrupt::Interrupted."
        "After each command the session checks the flag itself: if it was set, the command fails as 'interrupted', with a note that the session is as it was, whatever it returned. A budget fallback that swallowed the error and went on cannot leave a half-done result standing. An interrupted command also stops the rest of its line or sourced script."
        "The foma backend polls the same flag: each command also runs inside foma's interrupt::scope (foma 0.5.2), whose subset construction, product loops and Hopcroft minimization poll every 1024 steps. foma's algorithms cannot fail, so an interrupted one returns the empty machine instead, which the backstop discards with the rest of the command's result."
        "Only an interactive session on a terminal installs the SIGINT handler, through signal-hook on Unix: `flag::register` sets the flag, and `flag::register_conditional_shutdown` makes a second Ctrl-C while the flag is still set end the process, as a way out of a loop that does not poll. The shell clears the flag before each line it runs. Scripts, -p, -F, every other tool and Windows keep the default SIGINT, which ends the process."
        "At a prompt the line editor holds the terminal in raw mode, so Ctrl-C arrives as a key and only clears the line; SIGINT is only raised while a command runs, in cooked mode."
        "A cancelled command is a failed command: the undo of [spec:hfst:req:xfst-cmd.failed-command-no-effect] puts the stack, definitions, lists and variables back."
    )
    deferred (
        "Interrupting a command from the JavaScript package: wasm has no signals, so it would need a host-driven flag set from another worker, which no caller asks for yet."
    )
}
edges {
    requires ([dec:hfst:idiomatize-staging])
}
codifies ()
establishes ()
---

## Rationale

The shell must survive Ctrl-C during a long `compose net` or `read lexc` (operator,
2026-11-03): stop the command, keep the stack. Nothing outside the running loop can stop
Rust code safely, so the loop has to stop itself. That makes cancellation cooperative,
and leaves three questions: where the flag lives, which loops read it, and what a stopped
command leaves behind.

The flag lives with the host. The CLI is the only part that knows it runs on a terminal
and owns signal handling, so it creates the flag, registers it for SIGINT and passes a
handle to the session. The library only reads it. The session makes it the interrupt of
a scope around each command, so every rustfst call the command makes sees it, however
deep and by whatever path, without any signature changing.

The loops that read it are the ones that can run for long. In practice that is the lazy
expansion behind determinize and compose in rustfst, the minimization and epsilon removal
passes, and the compilers in hfst that drive many operations. Polling every 1024 units
keeps the cost to one relaxed load per 1024 iterations and keeps the time to stop small.
The determinize budget checks already sit at these points, so the interrupt check goes
next to them.

What a stopped command leaves behind is handled already. A cancelled command fails, and
the per-command undo restores the session, the same as for any failure at the prompt.
