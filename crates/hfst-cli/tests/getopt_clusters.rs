//! Clustered short options (`-wq` for `-w -q`). The parser decided an option
//! was short only when the token held a single letter and never took a packed
//! token apart, so every cluster the C tools accept — via the system
//! `getopt_long` they link against — was rejected outright: `-wq` on
//! hfst-optimized-lookup, `-qn 2` on hfst-fst2strings, and any Giella script
//! or hand-typed invocation that packs its flags.
//!
//! The expectations below were each read off the C++ 3.17.1 tools rather than
//! from the GNU documentation.

use hfst_cli::hfst_getopt::{GetOpt, Getopt, NO_ARGUMENT, OPTIONAL_ARGUMENT, REQUIRED_ARGUMENT};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// hfst-optimized-lookup's table, whose C short string is `hVvqsewb:t:uxfn:p::`
/// — no-argument, required-argument and optional-argument letters together.
fn long_options() -> Vec<GetOpt> {
    fn opt(name: &'static str, has_arg: i32, val: u8) -> GetOpt {
        GetOpt {
            name,
            has_arg,
            val: val as i32,
        }
    }
    vec![
        opt("help", NO_ARGUMENT, b'h'),
        opt("verbose", NO_ARGUMENT, b'v'),
        opt("quiet", NO_ARGUMENT, b'q'),
        opt("show-weights", NO_ARGUMENT, b'w'),
        opt("unique", NO_ARGUMENT, b'u'),
        opt("beam", REQUIRED_ARGUMENT, b'b'),
        opt("analyses", REQUIRED_ARGUMENT, b'n'),
        opt("pipe-mode", OPTIONAL_ARGUMENT, b'p'),
    ]
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_string()).collect()
}

/// Everything one getopt loop yields: each `(returned code, optarg)` in order,
/// the permuted argv, and the `optind` left pointing at the first operand.
type Parse = (Vec<(i32, Option<String>)>, Vec<String>, usize);

/// Drive the parser to exhaustion, collecting `(returned code, optarg)`.
fn drive(words: &[&str]) -> Parse {
    let long = long_options();
    let mut args = argv(words);
    let mut opt = Getopt::new();
    let mut seen = Vec::new();
    loop {
        let c = opt.getopt_long(&mut args, &long);
        if c == -1 {
            break;
        }
        seen.push((c, opt.optarg_opt()));
        // A tool exits on '?' / ':'; stop so a broken parser cannot spin here.
        if c == b'?' as i32 && seen.len() > 8 {
            break;
        }
    }
    (seen, args, opt.optind)
}

fn codes(words: &[&str]) -> Vec<char> {
    drive(words)
        .0
        .iter()
        .map(|(c, _)| *c as u8 as char)
        .collect()
}

#[test]
fn clustered_no_argument_options_split_apart() {
    assert_eq!(codes(&["prog", "-wq", "f.hfstol"]), ['w', 'q']);
    assert_eq!(codes(&["prog", "-qw", "f.hfstol"]), ['q', 'w']);
    assert_eq!(codes(&["prog", "-wqu", "f.hfstol"]), ['w', 'q', 'u']);
    assert_eq!(codes(&["prog", "-uwq", "f.hfstol"]), ['u', 'w', 'q']);
}

/// `-wn2`: the first argument-taking letter swallows the rest of the token.
#[test]
fn required_argument_swallows_the_cluster_remainder() {
    let (seen, _, _) = drive(&["prog", "-wn2", "f.hfstol"]);
    assert_eq!(
        seen,
        vec![(b'w' as i32, None), (b'n' as i32, Some("2".to_string())),]
    );
}

/// `-wnq`: the remainder is the argument even when it spells another option.
/// The C++ tool answers "Invalid or no argument for analyses count" here.
#[test]
fn cluster_remainder_wins_over_later_letters() {
    let (seen, _, _) = drive(&["prog", "-wnq", "f.hfstol"]);
    assert_eq!(
        seen,
        vec![(b'w' as i32, None), (b'n' as i32, Some("q".into()))]
    );
}

/// `-wn 2`: nothing left in the token, so the next argv word is the argument.
#[test]
fn required_argument_reaches_the_next_word() {
    let (seen, args, optind) = drive(&["prog", "-wn", "2", "f.hfstol"]);
    assert_eq!(
        seen,
        vec![(b'w' as i32, None), (b'n' as i32, Some("2".into()))]
    );
    assert_eq!(args[optind], "f.hfstol", "the operand must survive");
}

/// A negative-looking word is still an argument: `-wb -1.0` gives the beam
/// "-1.0", which the C++ tool then rejects as out of range rather than as a
/// stray option.
#[test]
fn negative_looking_arguments_are_taken_verbatim() {
    let (seen, _, _) = drive(&["prog", "-wb", "-1.0", "f.hfstol"]);
    assert_eq!(
        seen,
        vec![(b'w' as i32, None), (b'b' as i32, Some("-1.0".into()))]
    );
}

/// An optional argument has to be attached: bare `-p` inside a cluster must
/// leave the operand that follows alone.
#[test]
fn optional_argument_never_eats_the_next_word() {
    let (seen, args, optind) = drive(&["prog", "-wp", "f.hfstol"]);
    assert_eq!(seen, vec![(b'w' as i32, None), (b'p' as i32, None)]);
    assert_eq!(args[optind], "f.hfstol");
}

#[test]
fn optional_argument_takes_an_attached_value() {
    let (seen, _, _) = drive(&["prog", "-wpboth", "f.hfstol"]);
    assert_eq!(
        seen,
        vec![(b'w' as i32, None), (b'p' as i32, Some("both".into()))]
    );
}

/// The C++ tools answer `-wZq` with "invalid option -- Z", naming the letter
/// and not the token. Scanning then resumes after it, as glibc does.
#[test]
fn an_unknown_letter_is_named_by_itself() {
    let long = long_options();
    let mut args = argv(&["prog", "-wZq", "f.hfstol"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'w' as i32);
    assert_eq!(opt.getopt_long(&mut args, &long), b'?' as i32);
    assert_eq!(opt.optopt, b'Z' as i32, "the offending letter, not -2");
    assert_eq!(opt.getopt_long(&mut args, &long), b'q' as i32);
}

#[test]
fn an_unknown_first_letter_is_named_too() {
    let long = long_options();
    let mut args = argv(&["prog", "-Zwq", "f.hfstol"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'?' as i32);
    assert_eq!(opt.optopt, b'Z' as i32);
}

/// The cluster's tail is not an argv element, so the end-of-argv test must not
/// reach it first: `prog -wq` has to yield both letters.
#[test]
fn a_trailing_cluster_is_scanned_to_the_end() {
    assert_eq!(codes(&["prog", "-wq"]), ['w', 'q']);
    assert_eq!(codes(&["prog", "f.hfstol", "-wq"]), ['w', 'q']);
}

/// `-wn` with nothing after it is the missing-argument return, exactly as a
/// lone `-n` at the end of argv already was.
#[test]
fn a_cluster_can_end_in_a_missing_argument() {
    let long = long_options();
    let mut args = argv(&["prog", "-wn"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'w' as i32);
    assert_eq!(opt.getopt_long(&mut args, &long), b':' as i32);
    assert_eq!(opt.optopt, b'n' as i32);
}

/// `-Wall`-style attached arguments predate this change and must be untouched;
/// GNU keeps an '=' verbatim in an attached argument.
#[test]
fn attached_arguments_are_unchanged() {
    let (seen, _, _) = drive(&["prog", "-n2", "f.hfstol"]);
    assert_eq!(seen, vec![(b'n' as i32, Some("2".into()))]);

    let (seen, _, _) = drive(&["prog", "-b1.0", "f.hfstol"]);
    assert_eq!(seen, vec![(b'b' as i32, Some("1.0".into()))]);

    let (seen, _, _) = drive(&["prog", "-pa=b", "f.hfstol"]);
    assert_eq!(seen, vec![(b'p' as i32, Some("a=b".into()))]);
}

/// This port accepts a long name behind one dash, and that lookup runs first,
/// so a name that also reads as a cluster is still the long option.
#[test]
fn long_names_outrank_the_cluster_scan() {
    let (seen, _, _) = drive(&["prog", "-quiet", "f.hfstol"]);
    assert_eq!(seen, vec![(b'q' as i32, None)]);

    let (seen, _, _) = drive(&["prog", "--beam=1.0", "f.hfstol"]);
    assert_eq!(seen, vec![(b'b' as i32, Some("1.0".into()))]);
}

/// Two dashes is a long option or nothing — never a cluster.
#[test]
fn a_double_dashed_token_is_never_a_cluster() {
    let long = long_options();
    let mut args = argv(&["prog", "--wq", "f.hfstol"]);
    let mut opt = Getopt::new();

    assert_eq!(opt.getopt_long(&mut args, &long), b'?' as i32);
    assert_eq!(opt.optopt, -2, "an unknown long option stays anonymous");
}

/// The specials the tools depend on: `--` ends option parsing and a lone `-`
/// is an operand (stdin), neither of which is a one-letter cluster.
#[test]
fn end_of_options_and_lone_dash_still_hold() {
    let (seen, args, optind) = drive(&["prog", "-wq", "--", "-not-an-option"]);
    assert_eq!(seen.len(), 2);
    assert_eq!(args[optind], "-not-an-option");

    let (seen, args, optind) = drive(&["prog", "-wq", "-"]);
    assert_eq!(seen.len(), 2);
    assert_eq!(args[optind], "-", "a lone dash is the stdin operand");
}

/// Operands still permute to the tail with `optind` on the first of them.
#[test]
fn operands_still_permute_behind_the_options() {
    let (seen, args, optind) = drive(&["prog", "one", "-wn", "2", "two"]);
    assert_eq!(seen.len(), 2);
    assert_eq!(&args[optind..], ["one", "two"]);
}

// ---------------------------------------------------------------------------
// end-to-end: the real binary, against the spellings the C++ tools accept
// ---------------------------------------------------------------------------

fn run(args: &[&str], stdin: &[u8]) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hfst");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hfst");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    path.to_str().expect("utf-8 path").to_string()
}

/// The reported defect. `-wq` is byte-for-byte what the C++ tool prints for
/// the same transducer, which is also what `-w -q` prints here.
#[test]
fn optimized_lookup_accepts_clustered_flags() {
    let table = fixture("lookup.hfstol");
    let input = b"cat\ndog\n" as &[u8];
    // Byte-for-byte the C++ 3.17.1 tool's stdout for this fixture.
    let expected = "cat\tcat\n\ndog\tdog\n\n";

    let (ok, separate) = run(&["optimized-lookup", "-w", "-q", &table], input);
    assert!(ok, "hfst optimized-lookup -w -q exited non-zero");
    assert_eq!(separate, expected);

    for spelling in [
        vec!["optimized-lookup", "-wq", &table],
        vec!["optimized-lookup", "-qw", &table],
        vec!["optimized-lookup", "-wqu", &table],
        vec!["optimized-lookup", "-uwq", &table],
    ] {
        let (ok, out) = run(&spelling, input);
        assert!(ok, "hfst {} exited non-zero", spelling[1]);
        assert_eq!(out, expected, "hfst {} produced other output", spelling[1]);
    }
}

/// An argument-taking letter packed into a cluster: `-wn2`, `-wn 2` and
/// `-w -n 2` are one invocation in three spellings.
#[test]
fn clustered_argument_options_agree_when_separated() {
    let table = fixture("lookup.hfstol");
    let input = b"cat\n" as &[u8];

    let (ok, expected) = run(&["optimized-lookup", "-w", "-n", "1", &table], input);
    assert!(ok, "the separated spelling must work");

    for spelling in [
        vec!["optimized-lookup", "-wn", "1", &table],
        vec!["optimized-lookup", "-wn1", &table],
        vec!["optimized-lookup", "-w", "-n1", &table],
    ] {
        let (ok, out) = run(&spelling, input);
        assert!(ok, "hfst {:?} exited non-zero", spelling);
        assert_eq!(out, expected, "hfst {:?} produced other output", spelling);
    }
}

/// The other reported spelling, on a second tool — the fix is in the shared
/// parser, so no tool needs its own repair.
#[test]
fn fst2strings_accepts_clustered_flags() {
    let table = fixture("lookup.hfstol");

    let (ok, expected) = run(&["fst2strings", "-q", "-n", "2", &table], b"");
    assert!(ok, "hfst fst2strings -q -n 2 exited non-zero");

    for spelling in [
        vec!["fst2strings", "-qn", "2", &table],
        vec!["fst2strings", "-qn2", &table],
    ] {
        let (ok, out) = run(&spelling, b"");
        assert!(ok, "hfst {:?} exited non-zero", spelling);
        assert_eq!(out, expected, "hfst {:?} produced other output", spelling);
    }
}

/// A cluster holding an unknown letter is still rejected.
#[test]
fn an_unknown_letter_still_fails_the_run() {
    let table = fixture("lookup.hfstol");
    let (ok, _) = run(&["optimized-lookup", "-wZq", &table], b"cat\n");
    assert!(!ok, "-wZq must fail on the unknown Z");
}
