use super::input::{Token, tokens};

fn token<'t>(surface: &'t str, analyses: &[&'t str]) -> Token<'t> {
    Token {
        surface,
        analyses: analyses.to_vec(),
    }
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn blocks_of_lines_are_tokens() {
    let text = "nuna\tnuna+N+Abs+Sg\t0.000000\n\n\
                oqarpoq\toqar+V+Ind+3Sg\t1.5\noqarpoq\toqar+V+Gram/IV+Ind+3Sg\t2.0\n\n";
    assert_eq!(
        tokens(text).unwrap(),
        vec![
            token("nuna", &["nuna+N+Abs+Sg"]),
            token("oqarpoq", &["oqar+V+Ind+3Sg", "oqar+V+Gram/IV+Ind+3Sg"]),
        ]
    );
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn an_unknown_token_has_no_readings() {
    let text = "xyz\txyz+?\tinf\n\nnuna\tnuna+N+Abs+Sg\t0\n\n";
    assert_eq!(
        tokens(text).unwrap(),
        vec![token("xyz", &[]), token("nuna", &["nuna+N+Abs+Sg"])]
    );
}

#[test]
fn blank_lines_only_separate_blocks() {
    let text = "\n\nnuna\tnuna+N+Abs+Sg\t0\r\n\r\n\n\nsini\tsini+V+Ind+3Sg\t0";
    assert_eq!(
        tokens(text).unwrap(),
        vec![
            token("nuna", &["nuna+N+Abs+Sg"]),
            token("sini", &["sini+V+Ind+3Sg"])
        ]
    );
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn a_line_needs_three_fields() {
    let error = tokens("nuna\tnuna+N+Abs+Sg\t0\n\nsini\tsini+V\n").unwrap_err();
    assert!(error.to_string().contains("line 3"), "{error}");
    assert!(
        error.to_string().contains("2 tab-separated fields"),
        "{error}"
    );
}

#[test]
fn a_weight_must_be_a_number() {
    let error = tokens("nuna\tnuna+N+Abs+Sg\theavy\n").unwrap_err();
    assert!(error.to_string().contains("weight \"heavy\""), "{error}");
}

// [spec:hfst:req:prediction-model.input/test]
#[test]
fn a_block_of_two_surface_forms_is_refused() {
    let error = tokens("nuna\tnuna+N+Abs+Sg\t0\nsini\tsini+V+Ind+3Sg\t0\n").unwrap_err();
    assert!(error.to_string().contains("line 2"), "{error}");
    assert!(error.to_string().contains("blank line"), "{error}");
}
