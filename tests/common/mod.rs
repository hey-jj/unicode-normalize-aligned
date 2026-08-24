//! Helpers shared by the integration tests.
#![allow(dead_code)]
// A test helper that holds either iterator. The size gap does not matter here.
#![allow(clippy::large_enum_variant)]

use unicode_normalize_aligned::UnicodeNormalization;

pub const FORMS: [&str; 4] = ["NFC", "NFD", "NFKC", "NFKD"];

/// Parses "0041 030A" into chars.
pub fn hex_seq(s: &str) -> Vec<char> {
    s.split_whitespace()
        .map(|h| {
            let cp = u32::from_str_radix(h, 16).unwrap_or_else(|_| panic!("bad hex {h:?}"));
            char::from_u32(cp).unwrap_or_else(|| panic!("bad code point {h:?}"))
        })
        .collect()
}

/// Iterator over one normalization form applied to chars.
pub enum FormIter<I: Iterator<Item = char>> {
    D(unicode_normalize_aligned::Decompositions<I>),
    R(unicode_normalize_aligned::Recompositions<I>),
}

impl<I: Iterator<Item = char>> Iterator for FormIter<I> {
    type Item = (char, isize);

    fn next(&mut self) -> Option<(char, isize)> {
        match self {
            FormIter::D(it) => it.next(),
            FormIter::R(it) => it.next(),
        }
    }
}

/// Builds the form iterator without collecting.
pub fn normalize_iter<'a>(
    form: &str,
    input: &'a [char],
) -> FormIter<std::iter::Copied<std::slice::Iter<'a, char>>> {
    let it = input.iter().copied();
    match form {
        "NFC" => FormIter::R(it.nfc()),
        "NFD" => FormIter::D(it.nfd()),
        "NFKC" => FormIter::R(it.nfkc()),
        "NFKD" => FormIter::D(it.nfkd()),
        other => panic!("unknown form {other:?}"),
    }
}

/// Runs one normalization form over chars and collects the `(char, isize)` pairs.
pub fn normalize(form: &str, input: &[char]) -> Vec<(char, isize)> {
    let it = input.iter().copied();
    match form {
        "NFC" => it.nfc().collect(),
        "NFD" => it.nfd().collect(),
        "NFKC" => it.nfkc().collect(),
        "NFKD" => it.nfkd().collect(),
        other => panic!("unknown form {other:?}"),
    }
}

/// One form result from the alignment vector file.
pub struct FormResult {
    pub expected_pairs: Vec<(char, isize)>,
    pub unicode9_pairs: Vec<(char, isize)>,
    pub status: String,
}

/// One curated vector.
pub struct Vector {
    pub label: String,
    pub input: Vec<char>,
    pub input_chars: usize,
    pub forms: Vec<(String, FormResult)>,
}

fn pairs(v: &serde_json::Value) -> Vec<(char, isize)> {
    v.as_array()
        .expect("pair list")
        .iter()
        .map(|p| {
            let p = p.as_array().expect("pair");
            let c = hex_seq(p[0].as_str().expect("hex char"));
            assert_eq!(c.len(), 1);
            (c[0], p[1].as_i64().expect("tag") as isize)
        })
        .collect()
}

/// Loads `tests/conformance-alignment-vectors.json`.
pub fn load_vectors() -> Vec<Vector> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance-alignment-vectors.json"
    );
    let text = std::fs::read_to_string(path).expect("read conformance-alignment-vectors.json");
    let root: serde_json::Value = serde_json::from_str(&text).expect("parse vectors");
    assert_eq!(root["schema"], "conformance-alignment-vectors/1");
    assert_eq!(root["unicode_version"], "17.0.0");
    root["vectors"]
        .as_array()
        .expect("vectors array")
        .iter()
        .map(|v| Vector {
            label: v["label"].as_str().expect("label").to_string(),
            input: hex_seq(v["input"].as_str().expect("input")),
            input_chars: v["input_chars"].as_u64().expect("input_chars") as usize,
            forms: FORMS
                .iter()
                .map(|&form| {
                    let f = &v["forms"][form];
                    (
                        form.to_string(),
                        FormResult {
                            expected_pairs: pairs(&f["expected_pairs"]),
                            unicode9_pairs: pairs(&f["unicode9_pairs"]),
                            status: f["status"].as_str().expect("status").to_string(),
                        },
                    )
                })
                .collect(),
        })
        .collect()
}
