# tools/src/hfst-traverse.cc

> [spec:hfst:def:hfst-traverse.arclabel-completion-fn]
> static char**

> [spec:hfst:sem:hfst-traverse.arclabel-completion-fn]
> GNU-readline attempted-completion callback, compiled only when
> HAVE_DECL_RL_COMPLETION_MATCHES is set. Given the word 'text' and its
> 'start'/'end' offsets in the input line: if 'start' is 0 (the word is at the
> beginning of the line), return 'rl_completion_matches(text, arclabel_generator)'
> so readline offers arc labels as completions; otherwise return NULL (no
> completion). In the Rust port the foundation's readline is a plain
> getline with no readline backend, so — exactly as on a build without
> readline — this callback is not compiled in.

> [spec:hfst:def:hfst-traverse.arclabel-generator-fn]
> char*

> [spec:hfst:sem:hfst-traverse.arclabel-generator-fn]
> GNU-readline completion generator over the global '_rl_arcs' list of arc
> input symbols, compiled only when HAVE_DECL_RL_COMPLETION_MATCHES is set.
> Called repeatedly by readline for one completion request: when 'state' is 0
> (first call) it resets a static list index to 0 and records 'len' as the
> length of 'text'. On each call it scans forward from the current index and
> returns 'strdup' of the first '_rl_arcs' entry whose first 'len' characters
> equal 'text', advancing the index past it; when the list is exhausted it
> returns NULL. In the Rust port the foundation's readline has no readline
> backend, so this generator is not compiled in.

> [spec:hfst:def:hfst-traverse.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-traverse.main-fn]
> Program entry. Set the program name to argv[0] with version "0.1" and wiki
> name "HfstDeterminize" (carried verbatim from the source). Call
> parse_options; if it returns anything other than EXIT_CONTINUE, return that
> value. Otherwise close the input/output FILE buffers when they are real files
> (not stdin/stdout) since the tool works with streams, emit the verbose
> "Reading from <inputfilename>, writing to <outfilename>" message, then open
> the HfstInputStream — from 'inputfilename' when a file was given, else from
> stdin; if construction throws HfstException, error out with EXIT_FAILURE
> reporting "<inputfilename> is not a valid transducer file". Construct an
> HfstOutputStream of the input's type (to 'outfilename' or stdout) even though
> traversal never writes to it. Run process_stream on the input stream, free
> the input/output filename buffers, and return its result.

> [spec:hfst:def:hfst-traverse.main-loop-fn]
> int

> [spec:hfst:sem:hfst-traverse.main-loop-fn]
> Interactive traversal driver over an HfstBasicTransducer 'trans'. Print
> "Enter labels to seek all paths". Maintain a multimap of current paths keyed
> by the accumulated path string mapping to the end state, initialised with the
> single entry (empty string -> state 0). When readline completion is available,
> wire up arclabel_completion, bind Tab to complete, and suppress the append
> character; when history is available, enable history. Then loop forever:
>   1. For each current path, clear the '_rl_arcs' completion list and print
>      "On path `<pathstr>' are continuations:"; if the path's end state has no
>      outgoing transitions print "<Nothing, you've hit a dead end here>";
>      otherwise for each outgoing transition print "<input>\t<output>" and push
>      the input symbol onto '_rl_arcs'.
>   2. Read a line with hfst_readline("traverse> "). On NULL (EOF) return
>      EXIT_SUCCESS.
>   3. Build a new multimap: for every current path and every outgoing
>      transition whose input symbol equals the typed label, insert
>      ("<pathstr><input>:<output> " -> transition target state).
>   4. If the new multimap is empty: when the label is "quit" or empty print
>      "Use EOF (Ctrl-D or similar) to quit"; when it is "XYZZY" print
>      "Nothing happens"; in all of these cases also print "could not advance
>      with <label>". Otherwise replace the current paths with the new multimap.
>   5. Add the label to history when history is available, free the readline
>      buffer, and repeat.
> The C++ uses a multimap so several paths may share the same path-string; the
> Rust port reproduces that by keying a BTreeMap on (path-string, insertion
> counter), preserving ordered iteration and duplicate keys.

> [spec:hfst:def:hfst-traverse.parse-options-fn]
> int

> [spec:hfst:sem:hfst-traverse.parse-options-fn]
> Standard unary-tool option parsing. First call extend_options_getenv to splice
> in environment-provided options. Loop over getopt_long with the common+unary
> short option strings and a long-option table = common long options, unary long
> options, this tool's own {"cave", no_argument, 0, 'X'}, and the NULL
> terminator. Dispatch each returned option code through the common cases, then
> the unary cases, then the tool's own 'X' (which sets the global cave_mode flag
> to true), then the error/default case. Break out of the loop when getopt_long
> returns -1. Finally run the common and unary parameter checks and return
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-traverse.print-usage-fn]
> void

> [spec:hfst:sem:hfst-traverse.print-usage-fn]
> Print the --help text to message_out: a "Usage: <program_name> [OPTIONS...]
> [INFILE]" line followed by "Walk through the transducer arc by arc" and a
> blank line; then the common program options, the common unary program options,
> a blank line, the common unary parameter instructions, a blank line, the
> report-bugs notice, a blank line, and the more-info notice.

> [spec:hfst:def:hfst-traverse.process-stream-fn]
> int

> [spec:hfst:sem:hfst-traverse.process-stream-fn]
> Read transducers from 'instream' and traverse the first one. Loop while the
> stream is good: increment a transducer counter and read the next
> HfstTransducer. Take its name; if empty, fall back to 'inputfilename'.
> Build an HfstBasicTransducer 'walkable' from the transducer. If cave_mode is
> set, print the Colossal Cave "WELCOME TO ADVENTURE!!" prompt, read a line, and
> if the answer is "YES"/"yes" print the cave instructions block, then always
> print the "YOU ARE STANDING AT THE END OF A ROAD..." scene-setting block;
> otherwise print "Traversing automaton <name>". If 'walkable' has no states
> (begin()==end()), print "Nowhere to go" and return EXIT_SUCCESS. Otherwise
> return the result of main_loop(walkable) — i.e. only the first transducer is
> ever traversed. If the stream was not good at all, close it and return
> EXIT_SUCCESS.
