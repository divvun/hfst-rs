# tools/src/hfst-getopt.cc

> [spec:hfst:def:hfst-getopt.getopt-long-fn]
> int

> [spec:hfst:sem:hfst-getopt.getopt-long-fn]
> A minimal getopt_long replacement over the file-scope globals optarg, optopt,
> optind (init 1), and the two char* vectors free_arguments and other_arguments.
> One call returns the next option, or -1 when arguments are exhausted.
> Steps: (1) If optind > argc-1, the argument vector has been consumed: rewrite
> argv in place starting at index 1 with every other_arguments entry then every
> free_arguments entry, set optind to the count written + 1, and return -1.
> (2) Skip leading free (non-option) arguments: while argv[optind] does not start
> with '-', push it onto free_arguments and increment optind; if that exhausts the
> args (optind > argc-1), do the same argv-rewrite as step 1 and return -1.
> (3) Push argv[optind] onto other_arguments and strdup it into a working buffer
> arg (the copy may be mutated; it is intentionally leaked). Skip leading '-' in
> arg. If arg is now empty, set optopt = -2 and return '?'. Determine short_option
> = the char after the first surviving '-' is '\0' or '='. Scan arg for an '='; if
> found, replace it with '\0' (so the name compares cleanly) and set eq_used, with
> argptr pointing just past it. (4) Walk longopts until name == NULL: a match is
> strcmp(name, arg)==0, or short_option && val == (int)*arg. On match, increment
> optind, then: if has_arg==no_argument, warn to stderr if eq_used was given, and
> return val; if has_arg is required or optional and eq_used, set optarg =
> strdup(argptr), restore the '=' byte, and return val; if no '=' and the args are
> exhausted, return ':' with optopt=val for required (else optopt=NULL and return
> val); otherwise for required take argv[optind] as optarg (strdup + push to
> other_arguments + optind++) and return val, while optional sets optopt=NULL and
> returns val; an unexpected has_arg returns 0. (5) No match: optind++, optopt=-2
> (or (int)*arg if short_option), return '?'. longindex is unused.

> [spec:hfst:def:hfst-getopt.option]
> struct option {
>   const char *name;
>   int has_arg;
>   int *flag;
>   int val;
> }

