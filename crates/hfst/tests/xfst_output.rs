// Command output goes to the writer the session is given, or to a buffer the
// caller takes, and interactive input comes from the reader it is given
// (docs/spec/javascript.md).
use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::rc::Rc;

use hfst::xfst_compiler::{Flow, XfstCompiler};
use hfst_openfst::StdVectorFst;

fn printed(script: &str) -> String {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse(script).expect("xfst script runs");
    String::from_utf8(c.take_output()).expect("xfst output is UTF-8")
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
#[test]
fn print_words_lands_in_the_buffer() {
    assert_eq!(printed("regex a:b c ;\nprint words\n"), "a:bc\n");
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
#[test]
fn apply_with_an_argument_prints_its_results() {
    assert_eq!(
        printed("regex a:b ;\napply down a\napply up b\napply down z\n"),
        "b\na\n???\n"
    );
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
#[test]
fn print_net_lands_in_the_buffer() {
    assert_eq!(
        printed("regex a ;\nprint net\n"),
        "Ss0:\ta -> fs1.\nfs1:\t(no arcs).\n"
    );
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
#[test]
fn help_text_lands_in_the_buffer() {
    assert_eq!(
        printed("help union\napropos union\n"),
        "no help found for 'union'\nnothing found for 'union'\n"
    );
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
#[test]
fn print_stack_reports_each_network() {
    assert_eq!(
        printed("regex a ;\nregex b c ;\nprint stack\n"),
        "0: 2 states, 1 arcs\n1: 3 states, 2 arcs\n"
    );
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// The prompt follows the output of the command it ends, as it does on a
// terminal.
#[test]
fn the_prompt_follows_each_command() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_verbosity(true).set_prompt_verbosity(true);
    c.parse("regex a ;\nprint words\necho hi\n")
        .expect("xfst script runs");
    let out = String::from_utf8(c.take_output()).expect("xfst output is UTF-8");
    assert_eq!(out, "2 states, 1 arcs\nhfst[1]: a\nhfst[1]: hi\nhfst[1]: ");
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// Taking the output empties the buffer, so each run reads only its own text.
#[test]
fn take_output_returns_each_run_once() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex a ;\nprint words\n").expect("first run");
    assert_eq!(c.take_output(), b"a\n");
    c.parse("regex b ;\nprint words\n").expect("second run");
    assert_eq!(c.take_output(), b"b\n");
    assert!(c.take_output().is_empty());
}

/// A writer the test can read back after the session has written to it.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// Once a writer is set the output goes there and nothing is buffered.
#[test]
fn a_supplied_writer_receives_the_output() {
    let sink = Shared::default();
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_output_stream(sink.clone());
    c.parse("regex a ;\nprint words\nshow print-space\n")
        .expect("xfst script runs");
    assert_eq!(
        String::from_utf8(sink.0.borrow().clone()).expect("xfst output is UTF-8"),
        "a\nvariable print-space = OFF\n"
    );
    assert!(c.take_output().is_empty());
}

/// A writer whose every write fails.
struct Closed;

impl Write for Closed {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("output closed"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("output closed"))
    }
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// A failed write fails the command that made it.
#[test]
fn a_failed_write_fails_the_command() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_output_stream(Closed);
    let src = "regex a ;\nprint words\nregex b ;\n";
    let err = c
        .parse(src)
        .expect_err("printing into a closed writer fails");
    let d = err.diagnostics.first().expect("the failure is described");
    assert_eq!(&src[d.primary_span().expect("a span")], "print words");
    assert!(d.message().contains("output closed"), "{}", d.message());
    assert_eq!(c.get_stack().len(), 1, "'regex b' ran after the failure");
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// The prompt is output too: a prompt that cannot be written fails the
// command it ends.
#[test]
fn a_failed_prompt_fails_the_command() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_output_stream(Closed);
    c.set_verbosity(true).set_prompt_verbosity(true);
    let err = c
        .parse("clear stack\n")
        .expect_err("prompting into a closed writer fails");
    assert!(
        err.diagnostics[0].message().contains("output closed"),
        "{}",
        err.diagnostics[0].message()
    );
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// 'inspect net' reads its moves from the session's reader.
#[test]
fn inspect_net_reads_the_supplied_input() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_input_stream(Cursor::new("1\n0\n"));
    let flow = c
        .parse("regex a ;\ninspect net\nregex b ;\n")
        .expect("xfst script runs");
    assert_eq!(flow, Flow::Continue);
    let out = String::from_utf8(c.take_output()).expect("xfst output is UTF-8");
    assert_eq!(
        out,
        "'N' transits arc N, '-N' returns to level N, '<' to previous level, '0' quits.\n\
         Level 1\nArcs: 1. a\n  a:a --> Level 2 (final)\n\n"
    );
    assert_eq!(
        c.get_stack().len(),
        2,
        "the script went on after 'inspect net'"
    );
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// 'apply' ending the script, with no words after it, reads them from the
// session's reader.
#[test]
fn apply_without_words_reads_the_supplied_input() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.set_input_stream(Cursor::new("b\nb\n"));
    c.parse("regex a:b ;\napply up").expect("xfst script runs");
    assert_eq!(c.take_output(), b"a\na\n");
}

// [spec:hfst:req:javascript.xfst-output-sink/test]
// With no reader supplied the input is empty, so nothing waits on a terminal.
#[test]
fn without_a_reader_interactive_input_is_empty() {
    let mut c = XfstCompiler::<StdVectorFst>::new();
    c.parse("regex a:b ;\ninspect net\napply up")
        .expect("xfst script runs");
    let out = String::from_utf8(c.take_output()).expect("xfst output is UTF-8");
    assert_eq!(
        out,
        "'N' transits arc N, '-N' returns to level N, '<' to previous level, '0' quits.\n\
         Level 1\nArcs: 1. a:b\n"
    );
}
