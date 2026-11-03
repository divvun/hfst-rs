# XFST command contract

The XFST compiler came over from C++ with its control flow intact: a
command handler reported failure by setting a flag, the script driver
returned an integer, and a command with no implementation printed a
placeholder and carried on. Scripts built on that could not tell a
command that worked from one that did nothing. These rules state what
every command owes the script that runs it.

The per-function rules under `docs/spec/port/libhfst/src/parsers/` record
what the C++ code did. Where they disagree with this file, this file wins.

## Errors

> [spec:hfst:req:xfst-cmd.errors-are-values]
> Every command handler MUST return a `Result`. Running a script MUST
> return `Result<Flow>`, where `Flow` says whether the script ran to its end
> or asked to quit. A failed command's error MUST reach the driver as a
> value carrying the message, and the driver attaches the span of the
> command at fault. The driver MUST NOT keep a mutable fail flag or a quit
> flag, and MUST NOT report the outcome as an integer. `quit-on-fail`
> defaults to `ON`, so a script, an `-e` command or piped input stops at its
> first failing command and the tool exits nonzero. With `quit-on-fail`
> set to `OFF`, a failing command is reported and the script goes on. The
> interactive prompt always reports and goes on. Turning an error into a
> report happens in the driver, never in a handler.

> [spec:hfst:req:xfst-cmd.io-errors]
> A command that reads a file MUST fail, naming the path and the reason,
> when the file cannot be read. A command that writes a file MUST fail in
> the same way when the file cannot be created or written. Neither may
> treat a missing file as empty input, or skip the write in silence.

> [spec:hfst:req:xfst-cmd.failed-command-no-effect]
> When the session goes on after a failure, at the prompt or with
> `quit-on-fail` set to `OFF`, a command that fails MUST leave the stack,
> the definitions, the lists and the variables as they were before it ran.
> It MUST NOT drop the networks it had taken from the stack, and MUST NOT
> leave a network half changed. Output it printed before failing stays
> printed. `source` and an alias are the commands they run, and each of
> those is undone alone. When a failure ends the run, nothing is kept for
> undoing, so a script run pays no copies.

## No placeholders

> [spec:hfst:req:xfst-cmd.no-placeholders]
> A command MUST either do what its name says or fail. It MUST NOT print
> placeholder text such as "missing arc count", MUST NOT write placeholder
> text into an output file, and MUST NOT warn and then exit successfully
> having done nothing. A command the compiler does not support MUST fail
> with an error that names the command and says it is not supported.

## Scripts

> [spec:hfst:req:xfst-cmd.source]
> `source FILE` MUST run the commands in FILE as part of the current
> session: the stack, definitions, lists, aliases and variables are shared
> both ways. FILE is resolved against the working directory. Diagnostics
> raised inside FILE MUST name FILE and point into it. If a command in FILE
> fails, `source` fails with that error. A `quit` in FILE ends the whole
> session, not just FILE. Nesting deeper than 64 levels MUST fail, so a
> script that sources itself stops with an error rather than overflowing
> the stack.

## Reading and writing

> [spec:hfst:sem:xfst-cmd.read-word-lists]
> `read text` builds an automaton accepting each non-empty line of its
> input as one path, one symbol per character. `read spaced-text` reads one
> path per line with symbols separated by spaces. In both, a symbol written
> `a:b` is a pair. Both accept either a file name or inline text, and both
> MUST give the same network for the same text whichever way it arrives.

> [spec:hfst:sem:xfst-cmd.read-prolog]
> `read prolog FILE` pushes every network in FILE, written in the prolog
> format `write prolog` produces. `write prolog` writes the stack from the
> top down, so `read prolog` leaves the file's first network on top and
> restores the stack it came from. A file that does not parse as prolog
> fails with the line at fault.

> [spec:hfst:sem:xfst-cmd.write-word-lists]
> `write text` writes each string of the top network's upper side on its
> own line, symbols joined with nothing between them. `write spaced-text`
> writes each path on its own line, symbols separated by single spaces, and
> a symbol pair whose sides differ as `upper:lower`. In both, a `:`, space
> or backslash inside a symbol is escaped with a backslash, lines come in a
> stable order, and a cyclic network fails rather than being truncated.
> Reading `write spaced-text` output back with `read spaced-text` MUST give
> an equivalent network, and the same holds for `write text` and
> `read text` on an automaton whose symbols are single characters.

## Printing

> [spec:hfst:sem:xfst-cmd.print-counts]
> `print size` prints the top network's state and arc counts, and
> `print stack` prints them for every network on the stack, numbered from
> the bottom. `print arc-tally` prints the number of arcs in the top
> network. `print sigma-tally` prints, for each symbol of the top network's
> sigma, the number of arcs whose input or output label is that symbol, one
> `symbol: count` line per symbol in sigma order. `print flags` prints each
> flag diacritic in the top network's alphabet, one per line.
> `print props` prints each property of the top network as a
> `name: value` line. None of these prints a `?` in place of a number it
> could count.

## Network operations

> [spec:hfst:sem:xfst-cmd.sort]
> `sort net` sorts the arcs of every state of the top network by input
> label, then output label, then target. The network's language and
> weights do not change.

> [spec:hfst:sem:xfst-cmd.substring]
> `substring net` replaces the top network with one that accepts every
> substring of every path of the original, the empty string included:
> every state becomes both reachable from a new start and final.

> [spec:hfst:sem:xfst-cmd.ambiguity]
> `test functional` prints 1 when no input string of the top network maps
> to two different output strings, else 0. `test unambiguous` prints 1
> when no input string has two different paths, else 0. `extract
> ambiguous` replaces the top network with the part of it whose input
> strings have more than one path, and `extract unambiguous` with the part
> whose input strings have exactly one. `ambiguous upper` replaces the top
> network with the automaton of input strings that have more than one path.

> [spec:hfst:sem:xfst-cmd.apply-med]
> `apply med WORD` prints the strings of the top network's upper side
> closest to WORD by edit distance, cheapest first and equal costs in
> string order, each with its cost. It prints at most `med-limit` distinct
> strings and does not search past cost `med-cutoff`.

## Variables

> [spec:hfst:req:xfst-cmd.variables-take-effect]
> Every variable that `set` accepts MUST change what some command does. A
> variable with no effect MUST NOT be settable: `set` on it fails as it
> would on an unknown name, and `show` does not list it.

## Lists

> [spec:hfst:sem:xfst-cmd.list-range]
> `list NAME A-B` defines NAME as every Unicode scalar from A to B
> inclusive, where A and B are single characters and A is not after B.
> Any other range fails. A member that is a lone `-` is a symbol, not an
> empty range.

## Regular-expression string escapes

> [spec:hfst:sem:xfst-cmd.string-escapes]
> Inside a quoted XRE string, `\uXXXX` is the Unicode scalar with that
> four-digit hex value, `\xHH` the scalar with that two-digit hex value, and
> `\NNN` the scalar with that three-digit octal value. An escape that is
> malformed or names a surrogate or zero MUST fail the regex with the
> escape's span; it MUST NOT insert a NUL byte or drop characters.

## Replace-rule contexts

> [spec:hfst:sem:xfst-cmd.replace-context-symbol]
> In a replace rule's context, `?` and every expression built from it, such
> as `\[a]` or `[? - a]`, stand for exactly one symbol of the string. The
> string's edge is not a symbol, so `i -> u || ? _` leaves a word-initial
> `i` alone and `i -> u || _ ?` leaves a word-final one alone. Only `.#.`
> matches the edge.

## pmatch operators

> [spec:hfst:sem:xfst-cmd.pmatch-quotient-subtract]
> In pmatch, `A \\\ B` (left quotient) is the set of strings w such that
> some u in A makes u w a string of B. Upper subtraction `A .-u. B` removes
> from A the paths whose input side is in the input side of B, and lower
> subtraction `A .-l. B` does the same on the output side. None of these
> may yield an empty network in place of an error.

## The interactive shell

> [spec:hfst:req:xfst-cmd.help]
> `help` (or `describe`) with no argument MUST print one entry for every
> command the compiler accepts: its name, its arguments and what it does,
> with the other names it goes by on the next line. `help NAME` MUST print
> every entry with a name equal to NAME or starting with NAME and a space,
> so `help print` lists every `print` command. `apropos WORD` MUST print
> every entry whose names or description contain WORD as a whole word,
> ignoring case, and `apropos` alone prints the entry for `apropos`. Each
> name in the help MUST be a command the parser recognises, and a command
> that fails as not supported MUST NOT be listed. When nothing matches, the
> command says so and names what was asked for.

> [spec:hfst:req:xfst-cmd.interactive-prompt]
> The interactive shell MUST print exactly one prompt before each line it
> reads, and that prompt MUST reach the terminal before the shell waits for
> the line. This holds after a command that worked, after one that failed,
> after a blank or comment line, and on a continuation line.

> [spec:hfst:req:xfst-cmd.shell-in-library]
> The shell MUST be a library function: reading lines, deciding when a
> command is complete, prompting, running the command and reporting a
> failure all happen in the hfst library. Every line the shell reads,
> including the lines `apply`, `read text`, `read spaced-text` and `inspect
> net` read, MUST come through one line source the host supplies. The host
> supplies only the terminal: a plain reader or a line editor. A line that
> is not valid UTF-8 MUST be reported as such, and the session goes on.

> [spec:hfst:req:xfst-cmd.shell-complete-commands]
> A command typed at the prompt MUST mean what the same text means in a
> script. A command whose syntax ends in `;` (`regex`, `define` with a
> body, a `define` function, `list`) is complete at that `;`, outside
> brackets, quotes and comments. Until then, and after a line ending in
> `\`, the shell MUST read more lines under a continuation prompt that
> differs from the main one. `define NAME` with nothing after it is
> complete. A diagnostic MUST point into the command as it was typed, with
> its lines numbered from 1.

> [spec:hfst:req:xfst-cmd.shell-word-modes]
> `apply up`, `apply down`, `apply med`, `read text` and `read spaced-text`
> with nothing after them at the prompt MUST read their words from the
> lines that follow, each line under the command's own prompt, until end of
> input (Ctrl-D) or a `<ctrl-d>` line, and then return to the shell. `apply`
> answers each word as it is read. `read text` and `read spaced-text` build
> their network from all the lines once input ends.

> [spec:hfst:req:xfst-cmd.shell-line-editing]
> When standard input and output are terminals and `-r` is not given, the
> shell MUST read every line with a line editor: moving the cursor,
> deleting, and recalling earlier lines with up and down. The lines read in
> the word modes and in `inspect net` share this editor. History lasts for
> the session only. With `-r`, or when input is not a terminal, lines are
> read plainly. Ctrl-C at any prompt discards that line and any unfinished
> command, and prompts again. Ctrl-D at the main prompt ends the session;
> in a word mode it ends the mode.

> [spec:hfst:req:xfst-cmd.shell-completion]
> In the line editor, Tab at the start of a command MUST complete command
> names, taken from the table `help` reads. After a command that takes a
> file name, Tab completes file names. After a command that takes the name
> of a defined network (`push defined`, `substitute defined`, `undefine`,
> `print labels`, `write definition`), Tab completes defined names.

> [spec:hfst:req:xfst-cmd.shell-cancel]
> In an interactive session on a terminal, Ctrl-C while a command runs MUST
> stop the command in bounded time and report that it was interrupted. The
> stack, the definitions, the lists and the variables MUST be as they were
> before the command, and the session goes on. Elsewhere Ctrl-C ends the
> process.

> [spec:hfst:req:xfst-cmd.shell-cancel-foma]
> Under the foma backend (`-f foma`), Ctrl-C MUST stop a running command in
> bounded time just as under the default backend: each command runs inside
> foma's interrupt scope as well as rustfst's, so foma's long loops poll the
> same flag.

> [spec:hfst:req:xfst-cmd.shell-session-edges]
> On a terminal, the shell MUST print one line when it starts, naming the
> tool and saying how to get help and how to quit. Ending the session with
> Ctrl-D MUST leave the terminal at the start of a new line. A session that
> was interactive on a terminal MUST NOT print the invocation timing line
> on exit; every other run keeps it. `hfst xfst --help` MUST describe the
> line editor and `-r` as they are.

> [spec:hfst:sem:xfst-cmd.alias]
> `alias NAME` defines NAME as a sequence of commands, in the syntax Xerox
> xfst gives it, keeping the commands as they were written. Typing NAME
> where a command can stand, at the prompt or in a script, runs those
> commands as if they stood in its place. `print aliases` prints each alias
> and its commands.

> [spec:hfst:sem:xfst-cmd.terminal-output]
> `write dot`, `write prolog`, `write text` and `write spaced-text` with no
> file name print to the session output, and `> FILE` after any of them
> writes FILE. `print list NAME` prints the members of the one list NAME,
> and fails naming NAME when no such list exists.

> [spec:hfst:sem:xfst-cmd.shell-messages]
> `define` reports `Defined 'NAME'` for a network and `Defined function
> 'NAME(PARAMS)'` for a function, with its parameters as written. A
> diagnostic that quotes input MUST show a control character escaped, as
> `\u{1b}` for ESC, never raw. `lookup-optimize` and `remove-optimization`
> fail as not supported, since the stack never holds an optimized network;
> neither warns and succeeds.

## Tools

> [spec:hfst:req:xfst-cmd.no-dead-surface]
> A command-line option, trait method or code path that exists only to
> report that it is not implemented MUST be removed. This covers grep
> flags declared only to fail, trait methods whose body is
> `unimplemented!`, and branches for back-ends this build cannot contain.
> Comments that call working code unimplemented MUST be corrected.
