# libhfst/src/parsers/commandline_src/CommandLine.cc, libhfst/src/parsers/commandline_src/CommandLine.h

> [spec:hfst:def:command-line.command-line]
> class CommandLine {
>   bool be_verbose;
>   bool be_quiet;
>   bool has_input_file;
>   std::string input_file_name;
>   std::istream * input_file;
>   bool has_output_file;
>   std::string output_file_name;
>   std::ostream * output_file;
>   ImplementationType format;
>   bool resolve_left_conflicts;
>   bool resolve_right_conflicts;
>   bool help;
>   bool version;
>   bool usage;
>   bool has_debug_file;
>   std::istream &set_input_file(void);
>   std::ostream &set_output_file(void);
> }

> [spec:hfst:def:command-line.command-line.command-line-fn]
> CommandLine::CommandLine(int argc,char * argv[])

> [spec:hfst:sem:command-line.command-line.command-line-fn]
> Constructor. Initializes all member fields to defaults: be_verbose=false,
> be_quiet=false, has_input_file=false, input_file=NULL, has_output_file=false,
> output_file=NULL, resolve_left_conflicts=false, resolve_right_conflicts=true,
> help=false, version=false, usage=false, has_debug_file=false. Then immediately
> calls parse_options(argc, argv) to process the command-line arguments and
> overwrite the relevant fields. The return value of parse_options is discarded.

> [spec:hfst:def:command-line.command-line.parse-options-fn]
> int CommandLine::parse_options(int argc, char** argv)

> [spec:hfst:sem:command-line.command-line.parse-options-fn]
> Parses argv with getopt_long and updates the object's fields. Begins with local
> defaults: resolve_left=false, resolve_right=true, verbose=false, silent=false,
> outfilename=NULL, outputNamed=false, inputNamed=false, isDebug=false,
> infilename=NULL, debug_file_name=NULL, form=TROPICAL_OPENFST_TYPE.
> Defines long options: --help(h, no arg), --version(V), --verbose(v), --quiet(q),
> --silent(s), --usage(u), --input(i, required arg), --output(o, required arg),
> --resolve-left(R), --dont-resolve-right(D), --debug_file(d, required arg),
> --format(f, required arg). The getopt short option string is ":hVvqsui:o:RDi:d:f:"
> (the leading ':' makes a missing required argument report as ':').
> Loops calling getopt_long until it returns -1, then breaks. Per option:
>   'h' sets help=true; 'V' sets version=true; 'u' sets usage=true; 'v' sets
>   verbose=true; both 'q' and 's' set silent=true; 'R' sets resolve_left=true;
>   'D' sets resolve_right=false; 'i' sets inputNamed=true and infilename =
>   hfst_strdup(optarg); 'd' sets isDebug=true and debug_file_name =
>   hfst_strdup(optarg); 'o' sets outputNamed=true and outfilename =
>   hfst_strdup(optarg).
>   'f' maps optarg string to a format: "tropical-weight" or "tropical" ->
>   TROPICAL_OPENFST_TYPE; then a separate if/else-if chain: "log" -> LOG_OPENFST_TYPE,
>   "tropical-openfst"/"openfst-tropical" -> TROPICAL_OPENFST_TYPE,
>   "log-weight"/"log-openfst"/"openfst-log" -> LOG_OPENFST_TYPE,
>   "openfst"/"weighted"/"weight" -> TROPICAL_OPENFST_TYPE, "sfst" -> SFST_TYPE,
>   "foma"/"unweighted" -> FOMA_TYPE; any other value prints
>   'Unknown format "<optarg>".Try running with option -h or --help.' to stderr and
>   calls exit(1). (Note the "tropical-weight"/"tropical" check is a standalone if,
>   so for those two strings the subsequent else-if chain still runs starting at "log".)
>   ':' prints 'Missing argument for -<optopt>. Try using --help.' to stderr and
>   exit(1). default prints 'Unknown commandline option: -<optopt>. Try using --help.'
>   to stderr and exit(1).
> After the loop, handles positional args: if not inputNamed, and (argc-optind)==1,
> sets inputNamed=true and infilename=hfst_strdup(argv[optind]); if (argc-optind)>1,
> prints "no more than one input rule file may be given" to stderr and exit(1).
> If inputNamed and (argc-optind)>0, prints the same message and exit(1).
> Then assigns to the object: be_verbose=verbose, be_quiet=silent,
> has_input_file=inputNamed, has_output_file=outputNamed,
> resolve_left_conflicts=resolve_left, resolve_right_conflicts=resolve_right;
> if has_input_file, input_file_name=infilename; if has_output_file,
> output_file_name=outfilename; format=form. (The help/usage/version fields set
> during the loop are left as-is; the lines copying them are commented out.)
> Frees infilename and outfilename. If isDebug, sets has_debug_file=true,
> has_input_file=true, and input_file_name=debug_file_name (overriding any input
> file). Returns EXIT_CONTINUE.

> [spec:hfst:def:command-line.command-line.print-help-fn]
> void CommandLine::print_help(void)

> [spec:hfst:sem:command-line.command-line.print-help-fn]
> Prints the full help text to stderr. First calls print_usage(). Then writes to
> stderr a description paragraph ("Read a twolc grammar, compile it and store it.
> If INFILE is missing, the grammar is read from STDIN. If there is no output file
> given using -o or --output, the compiled grammar is written to STDOUT.").
> Then a "Common options:" section listing -h/--help, -V/--version, -u/--usage,
> -v/--verbose, -q/--quiet, -s/--silent (alias of --quiet) with descriptions.
> Then an "Input/Output options:" section listing -i/--input=INFILE and
> -o/--output=OUTFILE. Then a "TwolC grammar options:" section listing
> -R/--resolve (Resolve left-arrow conflicts), -D/--dont-resolve-right (Don't
> resolve right-arrow conflicts), and -f/--format=FORMAT (Store result in format
> FORMAT). Then "Format may be one of openfst-log, openfst-tropical, foma or sfst."
> and finally "By default format is openfst-tropical. By default right arrow
> conflicts are resolved and left arrow conflicts are not resolved." Returns void.

> [spec:hfst:def:command-line.command-line.print-usage-fn]
> void CommandLine::print_usage(void)

> [spec:hfst:sem:command-line.command-line.print-usage-fn]
> Prints usage lines to stderr. Writes a blank line, then four usage forms, each
> using PROGRAM_NAME: "Usage: <PROGRAM_NAME> [OPTIONS...] INFILE",
> "Usage: <PROGRAM_NAME> [OPTIONS...] -i INFILE",
> "Usage: <PROGRAM_NAME> [OPTIONS...] --input=INFILE", and
> "Usage: cat INFILE | <PROGRAM_NAME> [OPTIONS...]". Then a note:
> "An input file has to be given either using the option -i or --input, as the
> last commandline argument or from STDIN." followed by a blank line. Returns void.

> [spec:hfst:def:command-line.command-line.print-version-fn]
> void CommandLine::print_version(void)

> [spec:hfst:sem:command-line.command-line.print-version-fn]
> Prints version/license info to stderr in the GNU --version style. Writes a blank
> line, then "<PROGRAM_NAME> <TOOL_VERSION> (<PACKAGE_STRING>)", then the shared
> copyright/licence block of
> [spec:hfst:sem:hfst-commandline.print-version-fn] verbatim, with a trailing
> blank line. Returns void.
>
> PORT DIVERGENCE (branding, licence, and version, deliberate): upstream printed
> its own copy of the banner — a 2010 Helsinki copyright, GPLv3, idiosyncratic
> line wrapping, and the literal integer 0 as the version. This port shares the
> one block (so the three former copies cannot drift apart again) and carries a
> real TOOL_VERSION. Rationale for the branding and licence change is recorded
> on [spec:hfst:sem:hfst-commandline.print-version-fn].

> [spec:hfst:def:command-line.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:command-line.main-fn]
> Test driver, compiled only when the TEST_COMMAND_LINE macro is defined.
> Constructs a CommandLine from (argc, argv), which parses the options, then writes
> the CommandLine to std::cout via its overloaded operator<< followed by a newline.
> The operator<< prints labeled fields (VERBOSE, QUIET, INFILE EXIST, INFILE,
> OUTFILE EXIST, OUTFILE, FORMAT, RESOLVE). main has no explicit return.

