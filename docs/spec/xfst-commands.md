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
> format `write prolog` produces, onto the stack in file order. A file
> that does not parse as prolog fails with the line at fault.

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
> closest to WORD by edit distance, cheapest first, each with its cost. It
> stops after `med-limit` matches and does not search past cost
> `med-cutoff`.

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

## Tools

> [spec:hfst:req:xfst-cmd.no-dead-surface]
> A command-line option, trait method or code path that exists only to
> report that it is not implemented MUST be removed. This covers grep
> flags declared only to fail, trait methods whose body is
> `unimplemented!`, and branches for back-ends this build cannot contain.
> Comments that call working code unimplemented MUST be corrected.
