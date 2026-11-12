//! `hfst predict`, `hfst predict-model` and `hfst predict-eval` run on a
//! small Greenlandic-like generator compiled in the test with `hfst lexc`
//! and `hfst fst2fst -w`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const LEXICON: &str = r#"
Multichar_Symbols
+N +V +Abs +Rel +Sg +Pl +Ind +3Sg +3Pl +NIQ +Der/vn +LI +Gram/IV

LEXICON Root
nuna:nuna N ;
nanoq:nanoq N ;
oqar:oqar V ;

LEXICON N
+N+Abs+Sg:0 End ;
+N+Abs+Pl:t End "weight: 1" ;
+N+Rel+Sg:p End "weight: 2" ;

LEXICON V
+V+Ind+3Sg:poq End ;
+V+Ind+3Pl:put End "weight: 1" ;
+Gram/IV+NIQ+Der/vn:niq N ;

LEXICON End
# ;
+LI:lu # ;
"#;

const GRAMMAR: &str = r#"
boundary = ["+Der/*"]
derivation = ["+NIQ"]
enclitic = ["+LI"]
hidden = ["+Gram/*"]

[[class]]
name = "noun"
patterns = ["+Der/vn", "+N"]

[[class]]
name = "verb"
patterns = ["+V"]
"#;

/// Analysed text in which nuna is mostly in the relative case. The generator
/// refuses the first reading of oqarpoq.
const TEXT: &str = "nunap\tnuna+N+Rel+Sg\t0.000000\n\n\
                    nunap\tnuna+N+Rel+Sg\t0.000000\n\n\
                    nuna\tnuna+N+Abs+Sg\t0.000000\n\n\
                    oqarpoq\tnuna+V+Ind+3Sg\t0.000000\n\
                    oqarpoq\toqar+V+Ind+3Sg\t0.000000\n\n\
                    qqq\tqqq+?\tinf\n\n\
                    nunap\tnuna+N+Rel+Sg\t0.000000\n\n";

fn run_as(program: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(program)
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
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for hfst")
}

fn hfst(args: &[&str], stdin: &str) -> Output {
    run_as(Path::new(env!("CARGO_BIN_EXE_hfst")), args, stdin)
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "hfst failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The generator and grammar files, in a directory that lives as long as
/// the fixture.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let fixture = Fixture { dir };
        std::fs::write(fixture.path("generator.lexc"), LEXICON).expect("write the lexicon");
        std::fs::write(fixture.path("grammar.toml"), GRAMMAR).expect("write the grammar");
        std::fs::write(fixture.path("text.txt"), TEXT).expect("write the text");
        stdout(&hfst(
            &[
                "lexc",
                "-f",
                "openfst-tropical",
                "-o",
                &fixture.arg("generator.hfst"),
                &fixture.arg("generator.lexc"),
            ],
            "",
        ));
        stdout(&hfst(
            &[
                "fst2fst",
                "-w",
                "-i",
                &fixture.arg("generator.hfst"),
                "-o",
                &fixture.arg("generator.hfstol"),
            ],
            "",
        ));
        fixture
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn arg(&self, name: &str) -> String {
        self.path(name).to_str().expect("a UTF-8 path").to_owned()
    }

    /// `hfst <tool>` over the generator and grammar, then `extra`.
    fn tool(&self, tool: &str, extra: &[&str], stdin: &str) -> Output {
        let generator = self.arg("generator.hfstol");
        let grammar = self.arg("grammar.toml");
        let mut args = vec![tool, "-g", &generator, "-G", &grammar];
        args.extend_from_slice(extra);
        hfst(&args, stdin)
    }

    fn build_model(&self) -> PathBuf {
        let model = self.arg("model.hfstpred");
        let text = self.arg("text.txt");
        stdout(&self.tool(
            "predict-model",
            &["-q", "--order", "2", "-o", &model, &text],
            "",
        ));
        self.path("model.hfstpred")
    }
}

fn unit_keys(tsv: &str, analysis: &str) -> Vec<String> {
    let header = format!("analysis\t{analysis}\t");
    tsv.split("\n\n")
        .find(|block| block.starts_with(&header))
        .unwrap_or_else(|| panic!("no block for {analysis} in {tsv}"))
        .lines()
        .filter_map(|line| line.strip_prefix("unit\t"))
        .map(|unit| unit.split('\t').next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn predict_writes_a_block_for_each_analysis() {
    let fixture = Fixture::new();
    let output = stdout(&fixture.tool("predict", &[], "nuna\nnuna+N+Abs+Sg\noqar+NIQ+Der/vn\n"));
    let blocks: Vec<&str> = output.split_terminator("\n\n").collect();
    assert_eq!(
        blocks[0],
        "analysis\tnuna\tcomplete=no\tstem-next=no\ttruncated=no\tunits=3\n\
         unit\t+N+Abs+Sg\tending\tnuna\t0\n\
         unit\t+N+Abs+Pl\tending\tnunat\t1\n\
         unit\t+N+Rel+Sg\tending\tnunap\t2"
    );
    assert_eq!(
        blocks[1],
        "analysis\tnuna+N+Abs+Sg\tcomplete=yes\tstem-next=no\ttruncated=no\tunits=1\n\
         unit\t+LI\tenclitic\t+LI\t0\n\
         form\tnuna\t0"
    );
    assert!(
        blocks[2].contains("unit\t+N+Abs+Pl\tending\toqarniqt\t1"),
        "{}",
        blocks[2]
    );
    assert_eq!(blocks.len(), 3);
}

#[test]
fn shown_cuts_the_units_but_not_their_total() {
    let fixture = Fixture::new();
    let output = stdout(&fixture.tool("predict", &["-n", "1"], "oqar\n"));
    assert!(
        output.starts_with("analysis\toqar\tcomplete=no\tstem-next=no\ttruncated=no\tunits=3\n")
    );
    assert_eq!(unit_keys(&output, "oqar"), ["+NIQ+Der/vn"]);
}

#[test]
fn json_writes_one_object_for_each_analysis() {
    let fixture = Fixture::new();
    let output = stdout(&fixture.tool("predict", &["--json"], "\nnuna+N+Abs+Sg\nnuna+V\n"));
    let objects: Vec<serde_json::Value> = output
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON object"))
        .collect();
    assert_eq!(objects.len(), 3);
    assert_eq!(objects[0]["analysis"], "");
    assert_eq!(objects[0]["stem_next"], true);
    assert_eq!(objects[1]["complete"], true);
    assert_eq!(objects[1]["units"][0]["key"], "+LI");
    assert_eq!(objects[1]["units"][0]["kind"], "enclitic");
    assert_eq!(objects[1]["forms"][0]["surface"], "nuna");
    assert!(objects[2]["error"].is_string());
}

#[test]
fn a_model_ranks_the_units() {
    let fixture = Fixture::new();
    let model = fixture.build_model();
    let model = model.to_str().expect("a UTF-8 path");
    let ranked = stdout(&fixture.tool("predict", &["--model", model], "nuna\n"));
    assert_eq!(
        unit_keys(&ranked, "nuna"),
        ["+N+Rel+Sg", "+N+Abs+Sg", "+N+Abs+Pl"]
    );
    let by_weight = stdout(&fixture.tool("predict", &[], "nuna\n"));
    assert_eq!(
        unit_keys(&by_weight, "nuna"),
        ["+N+Abs+Sg", "+N+Abs+Pl", "+N+Rel+Sg"]
    );
}

/// nuna was counted four times, three of them in the relative case, and
/// nanoq never, so after n the model ranks the complete word nunap first,
/// then nuna with what nunap leaves of it, then nanoq and its word unseen,
/// by the generator's weight.
// [spec:hfst:sem:prediction.whole-words/test]
#[test]
fn typed_letters_offer_stems_and_complete_words() {
    let fixture = Fixture::new();
    let model = fixture.build_model();
    let model = model.to_str().expect("a UTF-8 path");
    let output = stdout(&fixture.tool(
        "predict",
        &["--model", model, "--whole-words", "1"],
        "\tn\nnuna+N+Rel+Sg\n",
    ));
    let blocks: Vec<&str> = output.split_terminator("\n\n").collect();
    assert_eq!(
        blocks[0],
        "analysis\t\tcomplete=no\tstem-next=yes\ttruncated=no\tunits=2\ttyped=n\n\
         unit\tnuna+N+Rel+Sg\tword\tnunap\t2\n\
         unit\tnuna\tstem\tnuna\t0\n\
         unit\tnanoq\tstem\tnanoq\t0\n\
         unit\tnanoq+N+Rel+Sg\tword\tnanoqp\t2"
    );
    assert!(
        blocks[1].contains("complete=yes") && blocks[1].ends_with("form\tnunap\t2"),
        "{}",
        blocks[1]
    );
    let units = stdout(&fixture.tool("predict", &["--model", model], "\tn\n"));
    assert_eq!(unit_keys(&units, ""), ["nuna", "nanoq"]);
    let json = stdout(&fixture.tool(
        "predict",
        &["--json", "--whole-words", "1", "-n", "1"],
        "nuna\tn\n",
    ));
    let value: serde_json::Value = serde_json::from_str(json.trim()).expect("a JSON object");
    let refused = value["error"].as_str().unwrap_or_default();
    assert!(refused.contains("no stem can start"), "{value}");
}

#[test]
fn a_refused_analysis_leaves_the_rest_answered() {
    let fixture = Fixture::new();
    let output = stdout(&fixture.tool("predict", &[], "nuna+V+Ind+3Sg\nnanoq\n"));
    let (refused, rest) = output.split_once("\n\n").expect("two blocks");
    assert_eq!(
        refused,
        "error\tnuna+V+Ind+3Sg\tanalysis \"nuna+V+Ind+3Sg\": the generator does not \
         accept the unit \"+V+Ind+3Sg\" after \"nuna\""
    );
    assert!(rest.starts_with("analysis\tnanoq\t"));
}

#[test]
fn a_generator_not_in_optimized_lookup_is_refused() {
    let fixture = Fixture::new();
    let output = hfst(
        &[
            "predict",
            "-g",
            &fixture.arg("generator.hfst"),
            "-G",
            &fixture.arg("grammar.toml"),
        ],
        "nuna\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not optimized lookup"), "{stderr}");
}

#[test]
fn predict_model_reports_what_it_counted() {
    let fixture = Fixture::new();
    let model = fixture.arg("model.hfstpred");
    let output = fixture.tool("predict-model", &["--order", "2", "-o", &model], TEXT);
    stdout(&output);
    let report = String::from_utf8_lossy(&output.stderr);
    for line in [
        "tokens                             6",
        "readings                           6",
        "counted                            5",
        "refused by the generator           1",
        "tokens without a reading           1",
    ] {
        assert!(report.contains(line), "{line} in {report}");
    }
    let bytes = std::fs::read(&model).expect("the model is written");
    assert!(bytes.starts_with(b"HFSTPRED"));
}

#[test]
fn a_model_from_another_grammar_is_refused() {
    let fixture = Fixture::new();
    let model = fixture.build_model();
    std::fs::write(
        fixture.path("other.toml"),
        format!("{GRAMMAR}\n# changed\n"),
    )
    .expect("write the other grammar");
    let output = hfst(
        &[
            "predict",
            "-g",
            &fixture.arg("generator.hfstol"),
            "-G",
            &fixture.arg("other.toml"),
            "-m",
            model.to_str().expect("a UTF-8 path"),
        ],
        "nuna\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("hash"), "{stderr}");
}

#[test]
fn predict_eval_reports_variants_beside_recordings() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.path("recorded.tsv"),
        "nunap\t\tnunap\nnuna\t\tnunap\n",
    )
    .expect("write the recording");
    let recording = format!("speller={}", fixture.arg("recorded.tsv"));
    let text = TEXT.repeat(3);
    let table = stdout(&fixture.tool(
        "predict-eval",
        &[
            "--folds",
            "2",
            "--order",
            "1,2",
            "-n",
            "1",
            "--faces",
            "1+1",
            "--whole-words",
            "0,1",
            "--threads",
            "1",
            "--recording",
            &recording,
        ],
        &text,
    ));
    for name in [
        "o1 stupid-backoff n1 faces 1+1",
        "o2 stupid-backoff n1 faces 1+1",
        "o2 stupid-backoff n1 faces 1+1 words 1",
        "speller",
        "saved",
    ] {
        assert!(table.contains(name), "{name} in\n{table}");
    }
    assert!(
        table.lines().any(|line| line.starts_with("all ")),
        "{table}"
    );
}

#[test]
fn each_tool_answers_to_its_hfst_name() {
    let tools = hfst_cli::tools::TOOLS;
    let dir = tempfile::tempdir().expect("a temporary directory");
    for name in ["hfst-predict", "hfst-predict-model", "hfst-predict-eval"] {
        let (_, _, about) = tools
            .iter()
            .find(|(tool, _, _)| *tool == name)
            .unwrap_or_else(|| panic!("{name} is missing from TOOLS"));
        let link = dir.path().join(name);
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_hfst"), &link).expect("link the tool");
        let help = stdout(&run_as(&link, &["--help"], ""));
        assert!(help.contains(about), "{name}: {help}");
        assert!(
            help.contains(&format!("{name} [OPTIONS]")),
            "{name}: {help}"
        );
    }
}
