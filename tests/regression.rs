//! Named regressions for the round-2 HOLD items.

use std::fs;
use std::path::{Path, PathBuf};

use unicode_normalize_aligned::UnicodeNormalization;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn banned_needles() -> Vec<String> {
    [
        ["pre", "decessor"],
        ["in", "cumbent"],
        ["reb", "uild"],
        ["ref_", "normalize"],
        ["ora", "cle"],
        ["ORA", "CLE.md"],
        ["SPEC", ".md"],
        ["unicode-normalization-", "alignments"],
        ["cla", "ude"],
        ["cod", "ex"],
        ["gr", "ok"],
        ["fab", "le"],
    ]
    .into_iter()
    .map(|p| p.concat())
    .collect()
}

fn skip_dir(name: &str) -> bool {
    matches!(
        name,
        "target" | "fuzz" | "gen" | "scripts" | "tools" | ".github" | ".git"
    )
}

fn package_excluded(rel: &str) -> bool {
    rel == "tests/normalization_test.rs"
        || rel == "tests/alignment_invariants.rs"
        || rel == "tests/no_alloc.rs"
        || rel == "tests/regression.rs"
        || rel == "tests/common/ucd.rs"
        || (rel.starts_with("ucd/17.0.0/") && rel.ends_with(".txt"))
}

fn collect_shipped(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let child_rel = if rel.as_os_str().is_empty() {
            PathBuf::from(name.as_ref())
        } else {
            rel.join(name.as_ref())
        };
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            if skip_dir(name.as_ref()) {
                continue;
            }
            collect_shipped(&path, &child_rel, out);
        } else if ft.is_file() {
            let rel_str = child_rel.to_string_lossy().replace('\\', "/");
            if package_excluded(&rel_str) {
                continue;
            }
            out.push(child_rel);
        }
    }
}

/// Fails on the old module docs that framed the cases as copied work.
#[test]
fn version_skew_docs_describe_cases_directly() {
    let src = include_str!("version_skew.rs");
    let docs: String = src
        .lines()
        .take_while(|l| l.starts_with("//!") || l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !docs.contains("The 22 inputs the"),
        "module docs still open by referring to another crate's failures"
    );
    for needle in [
        ["in", "cumbent"].concat(),
        ["reb", "uild"].concat(),
        ["pre", "decessor"].concat(),
    ] {
        assert!(
            !docs.contains(&needle),
            "version_skew module docs still contain {needle:?}"
        );
    }
    assert!(
        docs.contains("Unicode 10.0") && docs.contains("unicode9_pairs"),
        "module docs must describe the 10.0-17.0 cases and unicode9_pairs"
    );
}

/// Fails when the vector file names the generator script or another crate.
#[test]
fn alignment_vectors_json_has_no_generator_source_field() {
    let text = include_str!("conformance-alignment-vectors.json");
    assert!(
        !text.contains(&["expected", "_source"].concat()),
        "expected_source must not ship"
    );
    assert!(
        !text.contains(&["ref_", "normalize"].concat()),
        "generator script name must not ship"
    );
    assert!(
        !text.contains(&["in", "cumbent"].concat()),
        "vector file still names another crate"
    );
}

/// Fails when cargo leaves target/ unignored.
#[test]
fn gitignore_ignores_cargo_target() {
    let text = fs::read_to_string(crate_root().join(".gitignore")).expect(".gitignore");
    assert!(
        text.lines().any(|l| l.trim() == "/target"),
        ".gitignore must ignore /target"
    );
}

/// Fails when crate rustdoc still says runs of more than 30 spill once.
#[test]
fn crate_docs_match_tested_spill_thresholds() {
    let lib = include_str!("../src/lib.rs");
    let docs: String = lib
        .lines()
        .take_while(|l| l.starts_with("//!") || l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !docs.contains("more than 30 combining characters"),
        "crate docs still overstate the spill as more than 30 combining characters once per run"
    );
    assert!(
        docs.contains("32") && docs.contains("34"),
        "crate docs must state the tested spill thresholds 32 (decompose) and 34 (compose)"
    );
}

/// Fails when final_summary runs on a freshly built Criterion.
#[test]
fn throughput_gate_reads_the_measured_criterion() {
    let src = include_str!("../benches/throughput.rs");
    assert!(
        src.contains("benches(&mut criterion);"),
        "benches must run on the Criterion that will be summarised"
    );
    assert!(
        src.contains("criterion.final_summary();"),
        "final_summary must run on the Criterion that measured the benches"
    );
    assert!(
        !src.contains("Criterion::default().configure_from_args().final_summary();"),
        "final_summary must not be called on a freshly built Criterion"
    );
}

/// Fails unless public iterator types implement Debug.
#[test]
fn decompositions_and_recompositions_are_debug() {
    let d = "e\u{301}".nfd();
    let r = "e\u{301}".nfc();
    let _ = format!("{d:?}{r:?}");
}

/// Fails if any file that cargo package would ship contains a banned token.
#[test]
fn shipped_files_contain_no_banned_residue() {
    let mut files = Vec::new();
    collect_shipped(&crate_root(), Path::new(""), &mut files);
    files.sort();
    let needles = banned_needles();
    let mut hits = Vec::new();
    for rel in &files {
        let name = rel
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let banned_names = [
            ["ora", "cle.md"].concat(),
            ["spec", ".md"].concat(),
            ["tests", ".md"].concat(),
        ];
        let ora_prefix = ["ora", "cle"].concat();
        if banned_names.iter().any(|n| n == &name) || name.starts_with(&ora_prefix) {
            hits.push(format!("{}: banned path name", rel.display()));
        }
        if rel == Path::new("src/tests.rs") {
            hits.push("src/tests.rs: rename to src/unit_tests.rs".into());
        }
        let path = crate_root().join(rel);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let lower = text.to_ascii_lowercase();
        for needle in &needles {
            if lower.contains(&needle.to_ascii_lowercase()) {
                hits.push(format!("{}: contains {needle:?}", rel.display()));
            }
        }
    }
    assert!(hits.is_empty(), "shipped residue:\n{}", hits.join("\n"));
    assert!(
        files
            .iter()
            .any(|p| p == Path::new("tests/conformance-alignment-vectors.json")),
        "conformance vectors must still ship"
    );
}
