# tools/src/hfst-info.cc

> [spec:hfst:def:hfst-info.main-fn]
> int main (int argc, char * argv[])

> [spec:hfst:sem:hfst-info.main-fn]
> Entry point of the hfst-info tool. It does not read or write any
> transducer stream; it only reports or tests the library's compiled-in
> version and feature set.
> Steps:
> 1. (On Windows only) set stdin to binary mode.
> 2. Call hfst_set_program_name(argv[0], "0.1", "HfstInfo") to set the
>    program name, tool version and wiki name.
> 3. Call parse_options(argc, argv) (its EXIT_CONTINUE return is ignored;
>    -h/-V exit inside it via process exit).
> 4. If min_version != -1: verbose-print "Requiring current version
>    <HFST_LONGVERSION> to be greater than <min_version>\n", and if
>    HFST_LONGVERSION < min_version call error(EXIT_FAILURE, 0, "Version
>    requirements not met").
> 5. If exact_version != -1: verbose-print "Requiring current version
>    <HFST_LONGVERSION> to be exactly <exact_version>\n", and if
>    HFST_LONGVERSION != exact_version call error(EXIT_FAILURE, 0,
>    "Version requirements not met").
> 6. If max_version != -1: verbose-print "Requiring current version
>    <HFST_LONGVERSION> to be greater than <max_version>\n", and if
>    HFST_LONGVERSION < max_version call error(EXIT_FAILURE, 0, "Version
>    requirements not met"). (The message and the comparison reuse the
>    "greater than" / "<" form of the min-version case verbatim.)
> 7. For each required feature string f (iterated in sorted std::set
>    order):
>    - sfst/SFST/HAVE_SFST: verbose-print "Requiring SFST support from
>      library"; in a build without SFST, error that SFST support is not
>      present (or, with lean SFST, that it is present only in limited
>      form). In the reference build (HAVE_SFST defined) no error fires.
>    - foma/FOMA/HAVE_FOMA: verbose-print "Requiring foma support from
>      library"; guarded by "#if HAVE_FOMA", so when foma is compiled in
>      it calls error(EXIT_FAILURE, 0, "Required foma support not
>      present") (the source guard is the unnegated macro).
>    - xfsm/XFSM/HAVE_XFSM: verbose-print "Requiring xfsm support from
>      library"; guarded by "#if HAVE_XFSM" calling error "Required xfsm
>      support not present". With xfsm absent no error fires.
>    - openfst/OPENFST/HAVE_OPENFST: verbose-print "Requiring OpenFst
>      support from library"; guarded by "#if HAVE_OPENFST" calling error
>      "Required OpenFst support not present" (the source guard is the
>      unnegated macro).
>    - icu/USE_ICU_UNICODE: verbose-print "Requiring Unicode parsed by
>      ICU"; no error.
>    - any other token: error(EXIT_FAILURE, 0, "Required <f> support is
>      unrecognised and therefore assumed to be missing").
> 8. Verbose-print the known data block: "HFST info version: <tool
>    version>\nHFST packaging: <PACKAGE_STRING>\nHFST version:
>    <PACKAGE_VERSION>\nHFST long version: <HFST_LONGVERSION>\n".
> 9. Verbose-print one "<backend> supported" line per backend compiled in:
>    "OpenFst supported", then "SFST supported" (or "SFST limitedly
>    supported" for lean SFST), "foma supported", "xfsm supported" as the
>    respective macros dictate, then always "Unicode support: ICU\n".
> 10. Return EXIT_SUCCESS.
> Because the no-test default in parse_options forces verbose mode, an
> invocation with no test options prints the full known-data and
> supported-backends report; with verbose off (some test selected) only
> the failing tests produce output via error.

> [spec:hfst:def:hfst-info.parse-options-fn]
> int

> [spec:hfst:sem:hfst-info.parse-options-fn]
> Parse command-line options for hfst-info.
> 1. Call extend_options_getenv(&argc, &argv) to splice any HFST_OPTIONS
>    environment arguments in.
> 2. Loop calling getopt_long with the long-option table
>    HFST_GETOPT_COMMON_LONG plus {"atleast-version",'a'},
>    {"exact-version",'e'}, {"max-version",'m'},
>    {"require-feature",'f'} (each required_argument), and the short
>    string HFST_GETOPT_COMMON_SHORT followed by "a:e:f:m:". Break the
>    loop when getopt_long returns -1.
> 3. Switch on the returned option character; only the following cases act
>    (every other accepted option, i.e. the common -v/-q/-s/-d/-o/--colour,
>    falls through and does nothing):
>    - 'a': min_version = parse_version_string(optarg).
>    - 'e': exact_version = parse_version_string(optarg).
>    - 'm': max_version = parse_version_string(optarg).
>    - 'f': insert optarg into the required_features set.
>    - 'h': call print_usage(); return EXIT_SUCCESS.
>    - 'V': call print_version(); return EXIT_SUCCESS.
> 4. After the loop, if min_version, max_version and exact_version are all
>    -1, required_features is empty, and verbose is false, then set verbose
>    = true and verbose-print "No tests selected; printing known data\n"
>    (so the default run reports everything).
> 5. Return EXIT_CONTINUE.

> [spec:hfst:def:hfst-info.parse-version-string-fn]
> static

> [spec:hfst:sem:hfst-info.parse-version-string-fn]
> Parse a version vector of one to three full-stop-separated runs of
> decimal digits into a single long, packing each component into a
> 10000-radix field.
> 1. strtoul the leading digits as 'major'. If the remainder (endptr) is
>    the empty string, return major * 10000 * 10000. If the remainder does
>    not begin with '.', call error(EXIT_FAILURE, 0, "cannot parse version
>    string from <remainder>"). Otherwise advance past the '.'.
> 2. strtoul the next digits as 'minor'. If the remainder is empty, return
>    major*10000*10000 + minor*10000. If it does not begin with '.', error
>    as above. Otherwise advance past the '.'.
> 3. strtoul the next digits as 'patch'. If the remainder is empty, return
>    major*10000*10000 + minor*10000 + patch. Otherwise error as above.
> 4. (Unreachable) return -1.
> The packing means version a.b.c maps to a*100000000 + b*10000 + c, which
> is the same scheme as the HFST_LONGVERSION constant the tool compares
> against. strtoul follows libc semantics: it consumes the maximal leading
> run of digits and yields 0 with the whole input as remainder when no
> digit is present.
