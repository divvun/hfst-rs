---
id [dec:hfst:xfst-cancellation]
epitome "Ctrl-C in the xfst shell cancels the running command cooperatively: the host owns an interrupt flag set from SIGINT, a Copy handle to it rides in EngineConfig and the algorithm configs down to the long loops, which poll it every 1024 units of work and fail with an Interrupted error that the per-command undo turns back into the session as it was."
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
)
consequences {
    accepted (
        "The interrupt is a Copy handle, `Interrupt(Option<&'static AtomicBool>)`, with equality by address. EngineConfig carries it, and so do the configs handed to rustfst (DeterminizeConfig, the lazy compute and compose options, minimize, rm_epsilon, connect, shortest distance) and to the foma crate's algorithms."
        "Each long loop polls the flag with a relaxed load once every 1024 states, transitions or iterations, which is what bounds the time to stop. A tripped poll fails with a dedicated Interrupted error, distinct from the budget errors so no budget fallback swallows it. hfst maps it to ErrorKind::Interrupted and the xfst compiler to the command error 'interrupted', with a note that the session is as it was."
        "Loops in hfst itself poll too: lexc, twolc and pmatch compiles, the regex compiler between operations, path extraction and the lookups behind apply and print words."
        "Only an interactive session on a terminal installs the SIGINT handler, through signal-hook: `flag::register` sets the flag, and `flag::register_conditional_shutdown` makes a second Ctrl-C while the flag is still set end the process, as a way out of a loop that does not poll. The shell clears the flag before each command. Scripts, -p, -F and every other tool keep the default SIGINT, which ends the process."
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
handle to the session. The library only reads it. A borrowed `&'static` keeps the handle
Copy, so it travels inside EngineConfig and the rustfst configs with no signature churn.

The loops that read it are the ones that can run for long. In practice that is the lazy
expansion behind determinize and compose in rustfst, the minimization and epsilon removal
passes, and the compilers in hfst that drive many operations. Polling every 1024 units
keeps the cost to one relaxed load per 1024 iterations and keeps the time to stop small.
The determinize budget checks already sit at these points, so the interrupt check goes
next to them.

What a stopped command leaves behind is handled already. A cancelled command fails, and
the per-command undo restores the session, the same as for any failure at the prompt.
