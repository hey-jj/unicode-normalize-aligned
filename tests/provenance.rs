//! The tables must come from the pinned UCD 17.0.0 files: the version
//! constant matches, and the hashes recorded in the generated header equal
//! the manifest. CI additionally regenerates the tables and diffs.

use unicode_normalize_aligned::UNICODE_VERSION;

#[test]
fn unicode_version_is_17_0_0() {
    assert_eq!(UNICODE_VERSION, (17, 0, 0));
}

#[test]
fn tables_header_hashes_equal_the_manifest() {
    let tables = include_str!("../src/tables.rs");
    let manifest = include_str!("../ucd/17.0.0/MANIFEST.sha256");
    let mut checked = 0;
    for line in manifest.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next().expect("manifest hash");
        let name = parts.next().expect("manifest name");
        let header = format!("// sha256 {name}: {hash}");
        assert!(
            tables.lines().take(10).any(|l| l == header),
            "tables.rs header lacks {header:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 5, "manifest entries");
}
