use hfst::convert_transducer_format::FormatCoder;

fn main() {
    // The format coder is now an owned value (no process-global). The three
    // special symbols are pre-seeded at 0/1/2.
    let mut coder = FormatCoder::new();
    assert_eq!(coder.get_string(0), "@_EPSILON_SYMBOL_@");
    assert_eq!(coder.get_string(1), "@_UNKNOWN_SYMBOL_@");
    assert_eq!(coder.get_string(2), "@_IDENTITY_SYMBOL_@");
    assert_eq!(coder.get_number("@_EPSILON_SYMBOL_@"), 0);
    assert_eq!(coder.get_number("@_IDENTITY_SYMBOL_@"), 2);

    // An out-of-range number gives the empty string.
    assert_eq!(coder.get_string(9999), "");

    // A fresh symbol gets the next free index and round-trips.
    let n = coder.get_number("cat");
    assert_eq!(n, 3);
    assert_eq!(coder.get_string(3), "cat");
    // Looking it up again is stable (no new index).
    assert_eq!(coder.get_number("cat"), 3);

    let m = coder.get_number("dog");
    assert_eq!(m, 4);

    // get_harmonization_vector: known symbols map to their numbers; "" -> 0.
    let coding = vec![
        "cat".to_string(),
        "".to_string(),
        "dog".to_string(),
        "fish".to_string(),
    ];
    let hv = coder.get_harmonization_vector(&coding);
    assert_eq!(hv, vec![3, 0, 4, 5]); // "fish" is freshly assigned index 5
    assert_eq!(coder.get_string(5), "fish");

    println!("convert coding OK");
}
