//! Every curated alignment vector, all four forms, exact `(char, tag)` match.

mod common;

#[test]
fn all_vectors_match_expected_pairs() {
    let vectors = common::load_vectors();
    assert_eq!(vectors.len(), 102, "vector count");
    let mut checked = 0;
    for v in &vectors {
        assert_eq!(v.input.len(), v.input_chars, "{}", v.label);
        for (form, r) in &v.forms {
            let got = common::normalize(form, &v.input);
            assert_eq!(got, r.expected_pairs, "{} {form}", v.label);
            checked += 1;
        }
    }
    assert_eq!(checked, 408, "form-results");
}

#[test]
fn matching_status_vectors_equal_unicode9_pairs() {
    for v in common::load_vectors() {
        for (form, r) in &v.forms {
            if r.status == "match" {
                let got = common::normalize(form, &v.input);
                assert_eq!(got, r.unicode9_pairs, "{} {form}", v.label);
            }
        }
    }
}
