//! Characters added in Unicode 10.0 through 17.0. 22 inputs, 54 form-results.
//! Output must equal `expected_pairs`. It must differ from `unicode9_pairs`,
//! the characters Unicode 9.0.0 tables emit for the same input.

mod common;

#[test]
fn skew_inputs_match_expected_and_differ_from_unicode9() {
    let mut inputs = 0;
    let mut form_results = 0;
    for v in common::load_vectors() {
        let mut any = false;
        for (form, r) in &v.forms {
            if !r.status.starts_with("unicode-9-wrong-chars") {
                continue;
            }
            any = true;
            form_results += 1;
            let got = common::normalize(form, &v.input);
            assert_eq!(got, r.expected_pairs, "{} {form}", v.label);
            assert_ne!(
                got, r.unicode9_pairs,
                "{} {form}: skew vector no longer differs from Unicode 9.0.0 tables",
                v.label
            );
        }
        if any {
            inputs += 1;
        }
    }
    assert_eq!(inputs, 22, "skew inputs");
    assert_eq!(form_results, 54, "skew form-results");
}

#[test]
fn no_alignment_anomalies_recorded() {
    for v in common::load_vectors() {
        for (_, r) in &v.forms {
            assert!(
                !r.status.starts_with("ANOMALY"),
                "{}: unexpected ANOMALY status",
                v.label
            );
        }
    }
}
