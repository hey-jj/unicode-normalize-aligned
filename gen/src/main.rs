#![forbid(unsafe_code)]
//! Table generator. Reads the pinned UCD files, checks their hashes, asserts the
//! data invariants the runtime relies on, and writes `src/tables.rs`.
//!
//! Run from the workspace root: `cargo run -p gen --release`.
//! The output is byte-identical on every run. Every map is iterated in sorted
//! order and no timestamp is written.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const UNICODE_VERSION: (u8, u8, u8) = (17, 0, 0);
const INPUTS: [&str; 5] = [
    "UnicodeData.txt",
    "CompositionExclusions.txt",
    "DerivedNormalizationProps.txt",
    "DerivedAge.txt",
    "NormalizationTest.txt",
];

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

/// Packed property bits. Mirrors `src/props.rs`.
const CCC_MASK: u16 = 0x00FF;
const COMBINING_MARK: u16 = 1 << 8;
const NFD_NO: u16 = 1 << 9;
const HAS_COMPAT: u16 = 1 << 10;
const NFC_QC_SHIFT: u16 = 11;
const COMPOSES_FIRST: u16 = 1 << 13;

const QC_YES: u16 = 0;
const QC_NO: u16 = 1;
const QC_MAYBE: u16 = 2;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gen sits inside the workspace")
        .to_path_buf();
    let ucd = root.join("ucd").join("17.0.0");
    let hashes = verify_manifest(&ucd);

    let data = UnicodeData::parse(&read(&ucd, "UnicodeData.txt"));
    let exclusions = parse_exclusions(&read(&ucd, "CompositionExclusions.txt"));
    let props = DerivedProps::parse(&read(&ucd, "DerivedNormalizationProps.txt"));
    check_age(&read(&ucd, "DerivedAge.txt"));
    check_test_header(&read(&ucd, "NormalizationTest.txt"));

    let tables = Tables::build(&data, &exclusions, &props);
    let out = tables.render(&hashes);
    let path = root.join("src").join("tables.rs");
    fs::write(&path, out).expect("write src/tables.rs");
    println!("wrote {}", path.display());
    println!("static data: {} bytes", tables.byte_size());
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// Verifies every input against `MANIFEST.sha256` and returns the hashes in
/// `INPUTS` order.
fn verify_manifest(dir: &Path) -> Vec<(String, String)> {
    let manifest = read(dir, "MANIFEST.sha256");
    let mut listed = BTreeMap::new();
    for line in manifest.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else {
            panic!("malformed manifest line: {line:?}");
        };
        listed.insert(name.to_string(), hash.to_string());
    }
    let mut out = Vec::new();
    for name in INPUTS {
        let expected = listed
            .get(name)
            .unwrap_or_else(|| panic!("{name} missing from MANIFEST.sha256"));
        let bytes = fs::read(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let actual = hex(&sha256(&bytes));
        assert!(
            &actual == expected,
            "{name}: sha256 {actual} does not match manifest {expected}"
        );
        out.push((name.to_string(), actual));
    }
    out
}

fn check_age(text: &str) {
    let expect = format!(
        "# DerivedAge-{}.{}.{}.txt",
        UNICODE_VERSION.0, UNICODE_VERSION.1, UNICODE_VERSION.2
    );
    assert_eq!(
        text.lines().next(),
        Some(expect.as_str()),
        "DerivedAge header"
    );
    let mut newest = (0u32, 0u32);
    for line in text.lines() {
        let Some((body, _)) = line.split_once('#') else {
            continue;
        };
        let mut fields = body.split(';').map(str::trim);
        let (Some(_range), Some(age)) = (fields.next(), fields.next()) else {
            continue;
        };
        let (major, minor) = age.split_once('.').expect("age has a dot");
        let v = (major.parse().unwrap(), minor.parse().unwrap());
        newest = newest.max(v);
    }
    assert_eq!(
        newest,
        (u32::from(UNICODE_VERSION.0), u32::from(UNICODE_VERSION.1)),
        "newest Age in DerivedAge.txt must equal UNICODE_VERSION"
    );
}

fn check_test_header(text: &str) {
    let expect = format!(
        "# NormalizationTest-{}.{}.{}.txt",
        UNICODE_VERSION.0, UNICODE_VERSION.1, UNICODE_VERSION.2
    );
    assert_eq!(
        text.lines().next(),
        Some(expect.as_str()),
        "NormalizationTest header must name UNICODE_VERSION"
    );
}

fn parse_cp(s: &str) -> u32 {
    u32::from_str_radix(s.trim(), 16).unwrap_or_else(|_| panic!("bad code point {s:?}"))
}

fn parse_range(s: &str) -> (u32, u32) {
    match s.split_once("..") {
        Some((lo, hi)) => (parse_cp(lo), parse_cp(hi)),
        None => {
            let cp = parse_cp(s);
            (cp, cp)
        }
    }
}

struct UnicodeData {
    ccc: BTreeMap<u32, u8>,
    mark: BTreeSet<u32>,
    canon: BTreeMap<u32, Vec<u32>>,
    compat: BTreeMap<u32, Vec<u32>>,
}

impl UnicodeData {
    fn parse(text: &str) -> Self {
        let mut out = UnicodeData {
            ccc: BTreeMap::new(),
            mark: BTreeSet::new(),
            canon: BTreeMap::new(),
            compat: BTreeMap::new(),
        };
        let mut range_start = None;
        for line in text.lines() {
            let f: Vec<&str> = line.split(';').collect();
            assert!(f.len() >= 6, "short UnicodeData line: {line:?}");
            let cp = parse_cp(f[0]);
            let is_mark = matches!(f[2], "Mn" | "Mc" | "Me");
            if f[1].ends_with(", First>") {
                range_start = Some(cp);
                continue;
            }
            if f[1].ends_with(", Last>") {
                let start = range_start.take().expect("range Last without First");
                assert_eq!(f[3], "0", "ranged entries carry ccc 0");
                assert!(f[5].is_empty(), "ranged entries carry no mapping");
                if is_mark {
                    out.mark.extend(start..=cp);
                }
                continue;
            }
            let ccc: u8 = f[3].parse().expect("ccc is a number");
            if ccc != 0 {
                out.ccc.insert(cp, ccc);
            }
            if is_mark {
                out.mark.insert(cp);
            }
            if !f[5].is_empty() {
                let (tagged, body) = match f[5].strip_prefix('<') {
                    Some(rest) => (true, rest.split_once('>').expect("closing >").1),
                    None => (false, f[5]),
                };
                let cps: Vec<u32> = body.split_whitespace().map(parse_cp).collect();
                assert!(!cps.is_empty());
                if tagged {
                    out.compat.insert(cp, cps);
                } else {
                    out.canon.insert(cp, cps);
                }
            }
        }
        out
    }

    fn ccc(&self, cp: u32) -> u8 {
        self.ccc.get(&cp).copied().unwrap_or(0)
    }

    /// Full decomposition by fixed-point expansion. Hangul syllables are
    /// left alone here. The runtime decomposes them arithmetically.
    fn full(&self, cp: u32, compat: bool) -> Vec<u32> {
        if (S_BASE..S_BASE + S_COUNT).contains(&cp) {
            return vec![cp];
        }
        let mapping = if compat {
            self.compat.get(&cp).or_else(|| self.canon.get(&cp))
        } else {
            self.canon.get(&cp)
        };
        match mapping {
            None => vec![cp],
            Some(m) => m.iter().flat_map(|&d| self.full(d, compat)).collect(),
        }
    }
}

fn parse_exclusions(text: &str) -> BTreeSet<u32> {
    text.lines()
        .filter_map(|l| l.split('#').next())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(parse_cp)
        .collect()
}

#[derive(Default)]
struct DerivedProps {
    full_exclusion: BTreeSet<u32>,
    nfd_no: BTreeSet<u32>,
    nfkd_no: BTreeSet<u32>,
    nfc: BTreeMap<u32, u16>,
    nfkc: BTreeMap<u32, u16>,
}

impl DerivedProps {
    fn parse(text: &str) -> Self {
        let mut out = DerivedProps::default();
        for line in text.lines() {
            let body = line.split('#').next().unwrap_or("").trim();
            if body.is_empty() {
                continue;
            }
            let f: Vec<&str> = body.split(';').map(str::trim).collect();
            let (lo, hi) = parse_range(f[0]);
            let qc = |v: &str| match v {
                "N" => QC_NO,
                "M" => QC_MAYBE,
                other => panic!("unexpected quick-check value {other:?}"),
            };
            match f[1] {
                "Full_Composition_Exclusion" => out.full_exclusion.extend(lo..=hi),
                "NFD_QC" => {
                    assert_eq!(f[2], "N");
                    out.nfd_no.extend(lo..=hi);
                }
                "NFKD_QC" => {
                    assert_eq!(f[2], "N");
                    out.nfkd_no.extend(lo..=hi);
                }
                "NFC_QC" => {
                    for cp in lo..=hi {
                        out.nfc.insert(cp, qc(f[2]));
                    }
                }
                "NFKC_QC" => {
                    for cp in lo..=hi {
                        out.nfkc.insert(cp, qc(f[2]));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

struct Tables {
    ascii_first: [u64; 2],
    shift: u32,
    index: Vec<u16>,
    leaves: Vec<u8>,
    values: Vec<u16>,
    canon: Vec<(u32, u16, u8)>,
    compat: Vec<(u32, u16, u8)>,
    pool: Vec<u32>,
    compose: Vec<u64>,
}

impl Tables {
    fn build(data: &UnicodeData, exclusions: &BTreeSet<u32>, props: &DerivedProps) -> Self {
        // Counts that match UCD 17.0.0.
        assert_eq!(data.canon.len(), 2081, "canonical mappings");
        assert_eq!(data.compat.len(), 3833, "compatibility mappings");
        assert_eq!(data.ccc.len(), 968, "code points with nonzero ccc");
        assert_eq!(exclusions.len(), 81, "CompositionExclusions entries");
        assert_eq!(
            props.full_exclusion.len(),
            1120,
            "Full_Composition_Exclusion"
        );
        assert!(
            exclusions.is_subset(&props.full_exclusion),
            "CompositionExclusions must be folded into Full_Composition_Exclusion"
        );
        let longest = data
            .canon
            .values()
            .chain(data.compat.values())
            .map(Vec::len)
            .max()
            .unwrap();
        assert_eq!(longest, 18, "longest raw mapping");

        // Full decompositions and their invariants.
        let mut full_canon = BTreeMap::new();
        let mut full_compat = BTreeMap::new();
        let mut non_starter_first = BTreeSet::new();
        for &cp in data.canon.keys() {
            let d = data.full(cp, false);
            for &x in &d {
                assert!(
                    !data.canon.contains_key(&x),
                    "fixed point: {x:04X} in {cp:04X}"
                );
            }
            let mut last = 0u8;
            for &x in &d {
                let c = data.ccc(x);
                assert!(
                    c == 0 || last <= c,
                    "mapping {cp:04X} is not in canonical order"
                );
                last = c;
            }
            if data.ccc(d[0]) != 0 {
                non_starter_first.insert(cp);
            }
            full_canon.insert(cp, d);
        }
        let expected_nsf: BTreeSet<u32> = [0x0340, 0x0341, 0x0343, 0x0344, 0x0F73, 0x0F75, 0x0F81]
            .into_iter()
            .collect();
        assert_eq!(
            non_starter_first, expected_nsf,
            "non-starter-first mappings"
        );
        for &cp in data.canon.keys().chain(data.compat.keys()) {
            let k = data.full(cp, true);
            for &x in &k {
                assert!(
                    !data.canon.contains_key(&x) && !data.compat.contains_key(&x),
                    "fixed point: {x:04X} in compat {cp:04X}"
                );
            }
            let c = full_canon.get(&cp).cloned().unwrap_or_else(|| vec![cp]);
            if k != c {
                full_compat.insert(cp, k);
            }
        }
        for cp in S_BASE..S_BASE + S_COUNT {
            assert!(
                !full_canon.contains_key(&cp) && !full_compat.contains_key(&cp),
                "Hangul must stay out of the tables"
            );
        }

        // Primary composites.
        let mut compose_map = BTreeMap::new();
        for (&cp, m) in &data.canon {
            if m.len() == 2 && !props.full_exclusion.contains(&cp) {
                let key = (u64::from(m[0]) << 21) | u64::from(m[1]);
                assert!(compose_map.insert(key, cp).is_none(), "duplicate pair");
            }
        }
        assert_eq!(compose_map.len(), 961, "primary composite pairs");
        let mut composes_first: BTreeSet<u32> =
            compose_map.keys().map(|k| (k >> 21) as u32).collect();
        let mut composes_second: BTreeSet<u32> =
            compose_map.keys().map(|k| (k & 0x1F_FFFF) as u32).collect();
        composes_first.extend(L_BASE..L_BASE + L_COUNT);
        composes_first.extend((S_BASE..S_BASE + S_COUNT).step_by(T_COUNT as usize));
        composes_second.extend(V_BASE..V_BASE + V_COUNT);
        composes_second.extend(T_BASE + 1..T_BASE + T_COUNT);

        // Per code point packed value, and the equivalences the packing relies on.
        let hangul = S_BASE..S_BASE + S_COUNT;
        let mut packed = vec![0u16; 0x11_0000];
        for (cp, slot) in packed.iter_mut().enumerate() {
            let cp = cp as u32;
            let mut v = u16::from(data.ccc(cp)) & CCC_MASK;
            if data.mark.contains(&cp) {
                v |= COMBINING_MARK;
            }
            let nfd_no = full_canon.contains_key(&cp) || hangul.contains(&cp);
            assert_eq!(nfd_no, props.nfd_no.contains(&cp), "NFD_QC No at {cp:04X}");
            if nfd_no {
                v |= NFD_NO;
            }
            let has_compat = full_compat.contains_key(&cp);
            assert_eq!(
                nfd_no || has_compat,
                props.nfkd_no.contains(&cp),
                "NFKD_QC No at {cp:04X}"
            );
            if has_compat {
                v |= HAS_COMPAT;
            }
            let nfc = props.nfc.get(&cp).copied().unwrap_or(QC_YES);
            let nfkc = props.nfkc.get(&cp).copied().unwrap_or(QC_YES);
            let derived_nfkc = if nfc == QC_NO || has_compat {
                QC_NO
            } else {
                nfc
            };
            assert_eq!(nfkc, derived_nfkc, "NFKC_QC at {cp:04X}");
            assert!(
                !composes_second.contains(&cp) || nfc == QC_MAYBE,
                "every composes-as-second char is NFC_QC Maybe: {cp:04X}"
            );
            v |= nfc << NFC_QC_SHIFT;
            if composes_first.contains(&cp) {
                v |= COMPOSES_FIRST;
            }
            *slot = v;
        }
        let mut ascii_first = [0u64; 2];
        for cp in 0..0x80usize {
            assert_eq!(
                packed[cp] & !COMPOSES_FIRST,
                0,
                "ASCII carries no property except composes-as-first"
            );
            if packed[cp] & COMPOSES_FIRST != 0 {
                ascii_first[cp >> 6] |= 1 << (cp & 63);
            }
        }

        // Distinct values, then the two-level trie at the cheapest shift.
        let values: Vec<u16> = packed
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(values.len() <= 256, "leaf index must fit a byte");
        let value_index = |v: u16| values.binary_search(&v).unwrap() as u8;
        let mut best: Option<(usize, u32, Vec<u16>, Vec<u8>)> = None;
        for shift in 5..=8u32 {
            let block = 1usize << shift;
            let mut blocks: BTreeMap<Vec<u8>, u16> = BTreeMap::new();
            let mut leaves = Vec::new();
            let mut index = Vec::new();
            for chunk in packed.chunks(block) {
                let leaf: Vec<u8> = chunk.iter().map(|&v| value_index(v)).collect();
                let id = match blocks.get(&leaf) {
                    Some(&id) => id,
                    None => {
                        let id = u16::try_from(blocks.len()).expect("block id fits u16");
                        blocks.insert(leaf.clone(), id);
                        leaves.extend_from_slice(&leaf);
                        id
                    }
                };
                index.push(id);
            }
            let size = index.len() * 2 + leaves.len();
            if best.as_ref().is_none_or(|b| size < b.0) {
                best = Some((size, shift, index, leaves));
            }
        }
        let (_, shift, index, leaves) = best.unwrap();

        // Shared pool of full decompositions, longest first so shorter
        // sequences can reuse a window of a longer one.
        let mut sequences: Vec<&Vec<u32>> =
            full_canon.values().chain(full_compat.values()).collect();
        sequences.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
        sequences.dedup();
        let mut pool: Vec<u32> = Vec::new();
        let locate = |seq: &[u32], pool: &mut Vec<u32>| -> u16 {
            let found = pool.windows(seq.len()).position(|w| w == seq);
            let at = match found {
                Some(at) => at,
                None => {
                    pool.extend_from_slice(seq);
                    pool.len() - seq.len()
                }
            };
            u16::try_from(at).expect("pool offset fits u16")
        };
        for seq in &sequences {
            locate(seq, &mut pool);
        }
        let entry = |cp: u32, seq: &[u32], pool: &mut Vec<u32>| {
            let off = locate(seq, pool);
            (cp, off, u8::try_from(seq.len()).expect("length fits u8"))
        };
        let canon: Vec<_> = full_canon
            .iter()
            .map(|(&cp, s)| entry(cp, s, &mut pool))
            .collect();
        let compat: Vec<_> = full_compat
            .iter()
            .map(|(&cp, s)| entry(cp, s, &mut pool))
            .collect();
        for e in canon.iter().chain(&compat) {
            assert!(usize::from(e.1) + usize::from(e.2) <= pool.len());
        }
        let compose: Vec<u64> = compose_map
            .iter()
            .map(|(&k, &cp)| (k << 21) | u64::from(cp))
            .collect();
        assert!(compose.windows(2).all(|w| w[0] < w[1]));

        Tables {
            ascii_first,
            shift,
            index,
            leaves,
            values,
            canon,
            compat,
            pool,
            compose,
        }
    }

    fn byte_size(&self) -> usize {
        self.index.len() * 2
            + self.leaves.len()
            + self.values.len() * 2
            + (self.canon.len() + self.compat.len()) * 8
            + self.pool.len() * 4
            + self.compose.len() * 8
    }

    fn render(&self, hashes: &[(String, String)]) -> String {
        let mut s = String::new();
        writeln!(
            s,
            "// Machine-written file. Regenerate with `cargo run -p gen --release`."
        )
        .unwrap();
        writeln!(
            s,
            "// Source: Unicode Character Database {}.{}.{}.",
            UNICODE_VERSION.0, UNICODE_VERSION.1, UNICODE_VERSION.2
        )
        .unwrap();
        for (name, hash) in hashes {
            writeln!(s, "// sha256 {name}: {hash}").unwrap();
        }
        writeln!(s, "// Static data: {} bytes.", self.byte_size()).unwrap();
        writeln!(s).unwrap();
        writeln!(
            s,
            "/// Unicode version the tables were generated from.\npub const UNICODE_VERSION: (u8, u8, u8) = ({}, {}, {});",
            UNICODE_VERSION.0, UNICODE_VERSION.1, UNICODE_VERSION.2
        )
        .unwrap();
        writeln!(
            s,
            "\n/// ASCII characters that are the first of a primary composite pair, as a bitmap."
        )
        .unwrap();
        writeln!(
            s,
            "pub static ASCII_COMPOSES_FIRST: [u64; 2] = [0x{:016X}, 0x{:016X}];",
            self.ascii_first[0], self.ascii_first[1]
        )
        .unwrap();
        writeln!(
            s,
            "\n/// Low bits of a code point that select an entry inside a leaf block."
        )
        .unwrap();
        writeln!(s, "pub const PROPS_SHIFT: u32 = {};", self.shift).unwrap();
        writeln!(s, "\n/// Block id per `code_point >> PROPS_SHIFT`.").unwrap();
        write_slice(
            &mut s,
            "PROPS_INDEX",
            "u16",
            self.index.iter().map(|v| v.to_string()),
        );
        writeln!(
            s,
            "\n/// Deduplicated leaf blocks of `1 << PROPS_SHIFT` value indices."
        )
        .unwrap();
        write_slice(
            &mut s,
            "PROPS_LEAVES",
            "u8",
            self.leaves.iter().map(|v| v.to_string()),
        );
        writeln!(
            s,
            "\n/// Distinct packed property values. See `props.rs` for the bit layout."
        )
        .unwrap();
        write_slice(
            &mut s,
            "PROPS_VALUES",
            "u16",
            self.values.iter().map(|v| format!("0x{v:04X}")),
        );
        writeln!(
            s,
            "\n/// Full canonical decompositions: (code point, pool offset, length)."
        )
        .unwrap();
        write_slice(
            &mut s,
            "CANON",
            "(u32, u16, u8)",
            self.canon.iter().map(fmt_entry),
        );
        writeln!(
            s,
            "\n/// Full compatibility decompositions that differ from the canonical one."
        )
        .unwrap();
        write_slice(
            &mut s,
            "COMPAT",
            "(u32, u16, u8)",
            self.compat.iter().map(fmt_entry),
        );
        writeln!(s, "\n/// Shared decomposition pool.").unwrap();
        write_slice(
            &mut s,
            "POOL",
            "char",
            self.pool.iter().map(|&c| format!("'\\u{{{c:X}}}'")),
        );
        writeln!(
            s,
            "\n/// Primary composites packed as `(first << 42) | (second << 21) | composite`, sorted."
        )
        .unwrap();
        write_slice(
            &mut s,
            "COMPOSE",
            "u64",
            self.compose.iter().map(|v| format!("0x{v:016X}")),
        );
        s
    }
}

fn fmt_entry(e: &(u32, u16, u8)) -> String {
    format!("(0x{:X}, {}, {})", e.0, e.1, e.2)
}

fn write_slice(s: &mut String, name: &str, ty: &str, items: impl Iterator<Item = String>) {
    writeln!(s, "pub static {name}: &[{ty}] = &[").unwrap();
    let mut line = String::from("   ");
    for item in items {
        if line.len() + item.len() + 2 > 96 {
            writeln!(s, "{line}").unwrap();
            line = String::from("   ");
        }
        line.push(' ');
        line.push_str(&item);
        line.push(',');
    }
    if line.len() > 3 {
        writeln!(s, "{line}").unwrap();
    }
    writeln!(s, "];").unwrap();
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 as specified in FIPS 180-4.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_answers() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let long = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&sha256(&long)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
