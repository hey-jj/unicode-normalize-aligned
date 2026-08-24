//! Tag invariants over every NormalizationTest input column and form:
//! the tags sum to the length change, no tag exceeds +1, positional
//! consumption lands exactly on the input end, and the first output char
//! never carries +1.

mod common;
#[path = "common/ucd.rs"]
mod ucd;

#[test]
fn invariants_hold_on_every_test_input() {
    let lines = ucd::test_lines();
    let mut checked = 0u64;
    for line in &lines {
        for form in common::FORMS {
            for input in &line.columns {
                let out = common::normalize(form, input);
                let sum: isize = out.iter().map(|p| p.1).sum();
                let diff = out.len() as isize - input.len() as isize;
                assert_eq!(sum, diff, "line {} {form}: tag sum", line.line);
                assert!(
                    out.iter().all(|p| p.1 <= 1),
                    "line {} {form}: tag above +1",
                    line.line
                );
                if let Some(first) = out.first() {
                    assert_ne!(first.1, 1, "line {} {form}: leading +1", line.line);
                }
                let mut cursor = 0usize;
                for &(_, tag) in &out {
                    if tag == 1 {
                        continue;
                    }
                    cursor += (1 - tag) as usize;
                }
                assert_eq!(
                    cursor,
                    input.len(),
                    "line {} {form}: consumption",
                    line.line
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 400_680);
}
