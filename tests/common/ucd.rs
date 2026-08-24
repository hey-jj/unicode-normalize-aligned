//! Readers for the pinned UCD files. Used only by tests that are excluded
//! from the published package, because the data files are excluded too.
#![allow(dead_code)]

/// One NormalizationTest line: line number, part, five columns.
pub struct TestLine {
    pub line: usize,
    pub part: u8,
    pub columns: [Vec<char>; 5],
}

fn ucd_path(name: &str) -> String {
    format!("{}/ucd/17.0.0/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub fn hex_seq(s: &str) -> Vec<char> {
    s.split_whitespace()
        .map(|h| {
            let cp = u32::from_str_radix(h, 16).unwrap_or_else(|_| panic!("bad hex {h:?}"));
            char::from_u32(cp).unwrap_or_else(|| panic!("bad code point {h:?}"))
        })
        .collect()
}

/// Parses every test line of NormalizationTest.txt.
pub fn test_lines() -> Vec<TestLine> {
    let text = std::fs::read_to_string(ucd_path("NormalizationTest.txt")).expect("read test file");
    let mut part = 0u8;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("@Part") {
            part = rest[..1].parse().expect("part number");
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut cols = line.split(';').map(hex_seq);
        let columns = [
            cols.next().expect("c1"),
            cols.next().expect("c2"),
            cols.next().expect("c3"),
            cols.next().expect("c4"),
            cols.next().expect("c5"),
        ];
        out.push(TestLine {
            line: i + 1,
            part,
            columns,
        });
    }
    assert_eq!(out.len(), 20_034, "NormalizationTest line count");
    out
}

/// The expected result column for a form applied to column `col` (0-based),
/// per the NormalizationTest header:
/// NFC gives c2 for c1..c3 and c4 for c4..c5. NFD gives c3 and c5.
/// NFKC always gives c4. NFKD always gives c5.
pub fn expected<'a>(line: &'a TestLine, form: &str, col: usize) -> &'a [char] {
    let c = &line.columns;
    match form {
        "NFC" => {
            if col < 3 {
                &c[1]
            } else {
                &c[3]
            }
        }
        "NFD" => {
            if col < 3 {
                &c[2]
            } else {
                &c[4]
            }
        }
        "NFKC" => &c[3],
        "NFKD" => &c[4],
        other => panic!("unknown form {other:?}"),
    }
}

/// Every assigned code point, from UnicodeData.txt including its ranged
/// entries, skipping surrogates.
pub fn assigned() -> Vec<char> {
    let text = std::fs::read_to_string(ucd_path("UnicodeData.txt")).expect("read UnicodeData");
    let mut out = Vec::new();
    let mut range_start: Option<u32> = None;
    for line in text.lines() {
        let mut f = line.split(';');
        let cp = u32::from_str_radix(f.next().expect("cp"), 16).expect("hex cp");
        let name = f.next().expect("name");
        if name.ends_with(", First>") {
            range_start = Some(cp);
            continue;
        }
        if name.ends_with(", Last>") {
            let start = range_start.take().expect("First before Last");
            out.extend((start..=cp).filter_map(char::from_u32));
            continue;
        }
        out.extend(char::from_u32(cp));
    }
    out
}
