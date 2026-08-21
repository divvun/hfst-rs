//! The GNU short-option spellings the C tools accept — clusters (`-wq` for
//! `-w -q`), attached arguments (`-n2`, `-Wall`), long names behind one dash
//! (`-quiet`), `--opt=val`, option/operand permutation, a lone `-` operand and
//! negative-looking argument values. The C tools got all of this from the
//! system `getopt_long` they linked against, so every Giella script and
//! hand-typed invocation depends on it.
//!
//! The ported getopt fallback is gone; the parser is clap 4 plus
//! [`hfst_cli::cli::normalize_argv`], the pre-pass that rewrites the two
//! spellings clap does not take (single-dash longs, attached optional-argument
//! values). The unit tests below pin the pre-pass; the end-to-end tests pin
//! the whole surface against the real binary, with expectations read off the
//! C++ 3.17.1 tools rather than from the GNU documentation.

use clap::{Arg, ArgAction, Command};
use hfst_cli::cli::normalize_argv;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command as Process, Stdio};

/// A Command mirroring hfst-optimized-lookup's option surface (the C short
/// string was `hVvqsewb:t:uxfn:p::`): no-argument, required-argument and
/// optional-argument letters together, plus the positional operand.
fn command() -> Command {
    Command::new("prog")
        .disable_help_flag(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-weights")
                .short('w')
                .long("show-weights")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("unique")
                .short('u')
                .long("unique")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("beam")
                .short('b')
                .long("beam")
                .allow_hyphen_values(true),
        )
        .arg(
            Arg::new("analyses")
                .short('n')
                .long("analyses")
                .allow_hyphen_values(true),
        )
        .arg(
            Arg::new("pipe-mode")
                .short('p')
                .long("pipe-mode")
                .num_args(0..=1)
                .require_equals(true)
                .default_missing_value("both"),
        )
        .arg(Arg::new("infile").num_args(0..))
}

fn norm(words: &[&str]) -> Vec<String> {
    let argv = words.iter().map(|w| (*w).to_string()).collect();
    normalize_argv(&command(), argv)
}

fn same(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_string()).collect()
}

// ---------------------------------------------------------------------------
// the argv pre-pass: the two spellings clap needs help with
// ---------------------------------------------------------------------------

/// A long option behind ONE dash grows its second dash, with an inline value
/// riding along; the old parser resolved these by scanning the long table.
#[test]
fn single_dash_long_names_grow_a_second_dash() {
    assert_eq!(
        norm(&["prog", "-quiet", "f.hfstol"]),
        same(&["prog", "--quiet", "f.hfstol"])
    );
    assert_eq!(
        norm(&["prog", "-show-weights"]),
        same(&["prog", "--show-weights"])
    );
    assert_eq!(norm(&["prog", "-beam=1.0"]), same(&["prog", "--beam=1.0"]));
}

/// The long-name lookup runs first, so a single-dash token that ALSO reads as
/// a cluster stays the long option: `-pipe-mode` must not be taken apart as
/// `-p` with the attached value "ipe-mode".
#[test]
fn long_names_outrank_the_cluster_scan() {
    assert_eq!(
        norm(&["prog", "-pipe-mode=input"]),
        same(&["prog", "--pipe-mode=input"])
    );
    assert_eq!(norm(&["prog", "-quiet"]), same(&["prog", "--quiet"]));
}

/// An optional-argument short with its value attached (`-pboth`) gets the '='
/// clap requires, through the long name; flags clustered ahead of it are split
/// off first, as getopt would have taken them apart.
#[test]
fn attached_optional_values_grow_an_equals() {
    assert_eq!(
        norm(&["prog", "-pboth", "f.hfstol"]),
        same(&["prog", "--pipe-mode=both", "f.hfstol"])
    );
    assert_eq!(
        norm(&["prog", "-wpboth"]),
        same(&["prog", "-w", "--pipe-mode=both"])
    );
    // GNU keeps an '=' verbatim inside an attached argument.
    assert_eq!(
        norm(&["prog", "-wpa=b"]),
        same(&["prog", "-w", "--pipe-mode=a=b"])
    );
}

/// A bare optional-argument short must stay bare — getopt's OPTIONAL_ARGUMENT
/// never reached out to the next argv word, and neither may the rewrite.
#[test]
fn bare_optional_shorts_are_untouched() {
    assert_eq!(
        norm(&["prog", "-p", "f.hfstol"]),
        same(&["prog", "-p", "f.hfstol"])
    );
    assert_eq!(
        norm(&["prog", "-wp", "f.hfstol"]),
        same(&["prog", "-wp", "f.hfstol"])
    );
}

/// Everything clap already handles passes through byte-identical: clusters,
/// attached required arguments, `--opt=val`, operands and a lone `-`.
#[test]
fn clap_native_spellings_pass_through() {
    for words in [
        &["prog", "-wq", "f.hfstol"][..],
        &["prog", "-n2", "f.hfstol"],
        &["prog", "-wn2", "f.hfstol"],
        &["prog", "-b1.0"],
        &["prog", "--pipe-mode=both"],
        &["prog", "f.hfstol", "-"],
    ] {
        assert_eq!(norm(words), same(words), "{words:?} must not be rewritten");
    }
}

/// `--` ends option parsing: nothing after it is rewritten, however much it
/// looks like an option.
#[test]
fn end_of_options_stops_the_rewrite() {
    assert_eq!(
        norm(&["prog", "-quiet", "--", "-quiet", "-pboth"]),
        same(&["prog", "--quiet", "--", "-quiet", "-pboth"])
    );
}

/// A letter the command never declared ends the cluster scan, so the token
/// reaches clap verbatim and clap reports the unknown option itself.
#[test]
fn unknown_letters_stop_the_cluster_scan() {
    assert_eq!(norm(&["prog", "-Zpboth"]), same(&["prog", "-Zpboth"]));
    // A required-argument letter also stops it: the rest of the token is that
    // option's value ('-wn2'), which clap glues on natively.
    assert_eq!(norm(&["prog", "-nboth"]), same(&["prog", "-nboth"]));
}

// ---------------------------------------------------------------------------
// end-to-end: the real binary, against the spellings the C++ tools accept
// ---------------------------------------------------------------------------

fn run_full(args: &[&str], stdin: &[u8]) -> (bool, String, String) {
    let mut child = Process::new(env!("CARGO_BIN_EXE_hfst"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn run(args: &[&str], stdin: &[u8]) -> (bool, String) {
    let (ok, stdout, _) = run_full(args, stdin);
    (ok, stdout)
}

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    path.to_str().expect("utf-8 path").to_string()
}

/// The original defect. `-wq` is byte-for-byte what the C++ tool prints for
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

/// The other originally-reported spelling, on a second tool — the behaviour
/// lives in the shared layer, so no tool needs its own repair.
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

/// `--opt=val` and the separate-word spelling are one invocation.
#[test]
fn long_options_take_an_equals_value() {
    let table = fixture("lookup.hfstol");
    let input = b"cat\n" as &[u8];

    let (ok, expected) = run(&["optimized-lookup", "-w", "-b", "1.0", &table], input);
    assert!(ok, "the separate-word spelling must work");

    let (ok, out) = run(&["optimized-lookup", "-w", "--beam=1.0", &table], input);
    assert!(ok, "hfst optimized-lookup --beam=1.0 exited non-zero");
    assert_eq!(out, expected, "--beam=1.0 must equal -b 1.0");
}

/// Options and operands permute freely: flags after the operand parse the
/// same as flags before it (getopt shuffled operands to the tail; clap
/// interleaves natively).
#[test]
fn operands_permute_with_the_options() {
    let table = fixture("lookup.hfstol");
    let input = b"cat\n" as &[u8];

    let (ok, expected) = run(&["optimized-lookup", "-wq", &table], input);
    assert!(ok, "the options-first spelling must work");

    let (ok, out) = run(&["optimized-lookup", &table, "-wq"], input);
    assert!(ok, "hfst optimized-lookup FILE -wq exited non-zero");
    assert_eq!(out, expected, "trailing flags must parse like leading ones");
}

/// A lone `-` is an operand naming the standard input, not an option.
#[test]
fn a_lone_dash_reads_the_standard_input() {
    let table = fixture("lookup.hfstol");
    let bytes = std::fs::read(&table).expect("read the fixture");

    let (ok, expected) = run(&["fst2strings", "-q", "-n", "2", &table], b"");
    assert!(ok, "the named-file spelling must work");

    let (ok, out) = run(&["fst2strings", "-q", "-n", "2", "-"], &bytes);
    assert!(ok, "hfst fst2strings - exited non-zero");
    assert_eq!(out, expected, "the - operand must read the same transducer");
}

/// A negative-looking word is still an argument: `-wb -1.0` gives the beam
/// "-1.0", which the tool then rejects as out of range — NOT as a stray
/// option. The diagnostic proves the value was consumed by `-b`.
#[test]
fn negative_looking_arguments_are_taken_verbatim() {
    let table = fixture("lookup.hfstol");
    let (ok, _, stderr) = run_full(&["optimized-lookup", "-wb", "-1.0", &table], b"cat\n");
    assert!(!ok, "a negative beam must be rejected");
    assert!(
        stderr.contains("Invalid argument for --beam"),
        "-1.0 must reach the beam validator, not the option parser: {stderr:?}"
    );
}
