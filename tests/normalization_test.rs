//! Full NormalizationTest 17.0.0 conformance: 20,034 lines, 5 columns,
//! 4 forms (400,680 assertions), the Part 1 implicit invariance clause, and
//! quick-check agreement on every column.

mod common;
#[path = "common/ucd.rs"]
mod ucd;

use std::collections::HashSet;

use unicode_normalize_aligned::{
    is_nfc_quick, is_nfd_quick, is_nfkc_quick, is_nfkd_quick, IsNormalized,
};

#[test]
fn conformance_400680_assertions() {
    let lines = ucd::test_lines();
    let mut assertions = 0u64;
    for line in &lines {
        for form in common::FORMS {
            for (col, input) in line.columns.iter().enumerate() {
                let got: Vec<char> = common::normalize(form, input)
                    .into_iter()
                    .map(|p| p.0)
                    .collect();
                let expected = ucd::expected(line, form, col);
                assert_eq!(
                    got,
                    expected,
                    "line {} {form} column {}",
                    line.line,
                    col + 1
                );
                assertions += 1;
            }
        }
    }
    assert_eq!(assertions, 400_680);
}

#[test]
fn part1_absent_code_points_are_invariant() {
    let lines = ucd::test_lines();
    let part1: HashSet<char> = lines
        .iter()
        .filter(|l| l.part == 1)
        .map(|l| {
            assert_eq!(l.columns[0].len(), 1, "Part 1 inputs are single chars");
            l.columns[0][0]
        })
        .collect();
    for c in ucd::assigned() {
        if part1.contains(&c) {
            continue;
        }
        for form in common::FORMS {
            let got = common::normalize(form, &[c]);
            assert_eq!(
                got,
                [(c, 0)],
                "U+{:04X} absent from Part 1 must be invariant under {form}",
                c as u32
            );
        }
    }
}

#[test]
fn quick_check_agrees_with_the_forms_on_every_column() {
    let lines = ucd::test_lines();
    for line in &lines {
        for form in common::FORMS {
            for (col, input) in line.columns.iter().enumerate() {
                let quick = match form {
                    "NFC" => is_nfc_quick(input.iter().copied()),
                    "NFD" => is_nfd_quick(input.iter().copied()),
                    "NFKC" => is_nfkc_quick(input.iter().copied()),
                    _ => is_nfkd_quick(input.iter().copied()),
                };
                let normalized: Vec<char> = common::normalize(form, input)
                    .into_iter()
                    .map(|p| p.0)
                    .collect();
                let unchanged = normalized == *input;
                if unchanged {
                    assert_ne!(
                        quick,
                        IsNormalized::No,
                        "line {} {form} column {}: No on unchanged text",
                        line.line,
                        col + 1
                    );
                } else {
                    assert_ne!(
                        quick,
                        IsNormalized::Yes,
                        "line {} {form} column {}: Yes on changed text",
                        line.line,
                        col + 1
                    );
                }
            }
        }
    }
}
