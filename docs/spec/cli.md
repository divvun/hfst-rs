# Command-line interface

Every hfst tool is a library module exposing `run(argv) -> i32`, reached
through the single `hfst` multiplexer binary.  The C++ suite expressed the
option handling as a per-tool `print_usage`/`parse_options` pair built out of
`#include`d getopt fragments, and the port reproduced that shape function for
function.  That shape is an implementation detail: this file states the
contract those functions have to satisfy, so it keeps holding when the
hand-rolled getopt scaffolding is replaced by a declarative parser and a
shared argument struct.

What is *not* boilerplate stays with the tool.  A rule under
`docs/spec/port/tools/src/hfst-<tool>.md` continues to own that tool's own
option vocabulary, validation, defaults and deliberate deviations; the rules
here own only what every tool shares.

## Dispatch

> [spec:hfst:req:cli.dispatch]
> The suite ships as one binary with two dispatch paths, and both must reach
> the same code with the same observable behaviour as the former standalone
> binaries.  Basename dispatch: when argv[0]'s file stem (with any `.exe`
> suffix stripped) names a tool, that tool runs with argv passed through
> completely unchanged.  Subcommand dispatch: `hfst <sub> [ARGS...]`, where
> `<sub>` is the tool name minus the `hfst-` prefix, runs the same tool with
> argv[0] rebuilt as `hfst <sub>` — so every program-name-derived message and
> usage line renders as invoked — and ARGS forwarded untouched; the outer
> parser must never interpret a tool's own flags, including `-h`/`--help`.
> The tool's return value is the process exit status.  The dispatch table must
> name every binary name the C++ suite installed for a tool this port
> implements, aliases and British spellings included: a missing name does not
> fail loudly, it silently resolves to whatever `hfst` binary sits further
> down PATH.  Each entry also carries the one-line summary shown both in the
> multiplexer's subcommand listing and in the tool's own usage.  The umbrella
> binary additionally answers `--version`/`-V` with the shared version banner
> under the program name `hfst`, provides `install-symlinks` to materialise
> the legacy names, and reports an unknown subcommand as an error.

## Shared options

> [spec:hfst:req:cli.common-options]
> Every tool accepts the common option set: `-d`/`--debug`, `-h`/`--help`,
> `-V`/`--version`, `-v`/`--verbose`, `-q`/`--quiet`, `-s`/`--silent`,
> `-o`/`--output=OUTFILE` and `--colour[=WHEN]` (spelled `--color` as well).
> `-v` turns verbosity on and silence off; `-q` and `-s` are the same option
> and turn silence on and verbosity off; the two states are mutually
> exclusive and whichever was given last wins.  `-o -` means standard
> output.  Whenever the tool's data output is
> standard output — because `-o -` was given, or because `-o` was omitted and
> the output defaulted to standard output — diagnostics, verbose lines and
> help must be written to standard error instead, so they cannot corrupt the
> data stream.  `--colour` takes `always`, `never` or `auto`; a bare
> `--colour` means `always`, an absent `--colour` means `auto`, `auto`
> decides by whether standard output is a terminal, and any other value is a
> fatal error naming the three legal values.  This state must be carried in
> a value threaded through the tool,
> never in process-wide mutable globals.

> [spec:hfst:req:cli.unary-options]
> A tool reading one transducer stream additionally accepts
> `-i`/`--input=INFILE`, where `-` means standard input.  After parsing, the
> input is resolved from what was given: with no `-i`, exactly one leftover
> positional argument is the input file (`-` again meaning standard input), no
> leftover positional means standard input, and more than one is the fatal
> error "no more than one transducer file may be given"; with `-i` given, any
> leftover positional is the fatal error "no more than one transducer filename
> may be given".

> [spec:hfst:req:cli.binary-options]
> A tool reading two transducer streams additionally accepts
> `-1`/`--input1=INFILE1`, `-2`/`--input2=INFILE2` and
> `-C`/`--do-not-convert`, where `-` again means standard input and `-C`
> forbids converting the operands into a common transducer type.  After
> parsing, the two inputs are resolved from whichever of `-1`/`-2` was given
> plus the leftover positionals: neither option and two positionals bind first
> and second in order; neither option and one positional binds it as the
> second input with the first read from standard input; one option and one
> positional binds the positional to the missing side; one option and no
> positional reads the missing side from standard input.  Both options with
> any positional left over, more positionals than the missing sides can
> absorb, no named input at all, and both options given with nothing to read
> from a file are each a fatal error.

## Argument parsing

> [spec:hfst:req:cli.arg-parse]
> Each tool accepts the common option set, the unary or binary set where it
> applies, and its own documented options, in any order, in short or long
> form, with a long option's argument accepted as `--option=VALUE`.  Before
> parsing, the whitespace-separated tokens of the `HFST_OPTIONS` environment
> variable are appended to the argument vector, so an option supplied there
> behaves as if it had been typed.  An unrecognised option, or a missing
> required option argument, must print the short-help pointer
> ("Try ``<program> --help'' for more information.") followed by the
> diagnostic naming the offending option, and exit 1; no work is done.  A
> tool's accepted option set and each option's effect on tool state is part of
> its contract however the parser is built — where those effects are more than
> setting a field (an argument vocabulary, a range check, a cross-option
> requirement, a post-parse default, or a deliberate deviation such as
> hfst-format and hfst-info accepting and ignoring options the shared error
> arm would reject) the tool's own rule pins them.

## Help and version

> [spec:hfst:req:cli.help]
> `-h`/`--help` must print the tool's usage and exit 0 without doing any
> work, writing to the tool's message stream — standard output unless the
> tool's data output has already been directed there.  The text must name the
> program as it was invoked (so a subcommand invocation renders as
> `hfst <sub>`), give the one-line summary the multiplexer's subcommand
> listing also shows, document every option the tool accepts including the
> shared ones, state how the standard streams are selected, and end with the
> bug-report address and the pointer to further documentation.  The exact
> layout of that text is not pinned by this rule.

> [spec:hfst:req:cli.version]
> `-V`/`--version` must print the shared version banner — the program-name
> line followed by the copyright and licence block, whose exact text the
> hfst-commandline print-version rule owns — and exit 0 without doing any
> work.  Every tool and the umbrella binary print the same banner,
> differing only in the program name, so the entry points cannot disagree
> about the version or the licence.

## Tool entry point

> [spec:hfst:req:cli.main]
> A tool is reachable only as `run(argv) -> i32`; there is no per-tool
> `main`.  `run` parses first, and a parse that finished the program — help,
> version, or a usage error — returns its exit code directly without opening
> any stream.  Otherwise `run` registers the program name, version and wiki
> name for diagnostics, opens the input stream(s) and the output stream from
> the filenames the parse resolved, reports what it is reading and writing
> when verbose, drives the operation one transducer at a time, closes the
> streams and returns the exit status.  The scaffolding shared by the
> one-input-stream and two-input-stream tool families is written once and
> parameterised by an operation descriptor rather than copied per tool.
