//! Every form, every quick check and every `char` query over arbitrary
//! input, with the idempotence and alignment properties asserted inline.
#![no_main]

use libfuzzer_sys::fuzz_target;
use unicode_normalize_aligned::{
    char, is_nfc, is_nfc_quick, is_nfd, is_nfd_quick, is_nfkc, is_nfkc_quick, is_nfkd,
    is_nfkd_quick, UnicodeNormalization,
};

fn check_invariants(input_chars: usize, pairs: &[(char, isize)]) {
    let sum: isize = pairs.iter().map(|p| p.1).sum();
    assert_eq!(sum, pairs.len() as isize - input_chars as isize, "tag sum");
    assert!(pairs.iter().all(|p| p.1 <= 1), "tag above +1");
    if let Some(first) = pairs.first() {
        assert_ne!(first.1, 1, "leading +1");
    }
    let mut cursor = 0usize;
    for &(_, tag) in pairs {
        if tag != 1 {
            cursor += (1 - tag) as usize;
        }
    }
    assert_eq!(cursor, input_chars, "positional consumption");
}

fn text(pairs: &[(char, isize)]) -> String {
    pairs.iter().map(|p| p.0).collect()
}

fn check(s: &str) {
    let n = s.chars().count();
    let nfc = s.nfc().collect::<Vec<_>>();
    let nfd = s.nfd().collect::<Vec<_>>();
    let nfkc = s.nfkc().collect::<Vec<_>>();
    let nfkd = s.nfkd().collect::<Vec<_>>();
    for pairs in [&nfc, &nfd, &nfkc, &nfkd] {
        check_invariants(n, pairs);
    }

    let (nfc, nfd, nfkc, nfkd) = (text(&nfc), text(&nfd), text(&nfkc), text(&nfkd));
    assert_eq!(text(&nfc.nfc().collect::<Vec<_>>()), nfc, "nfc idempotent");
    assert_eq!(text(&nfd.nfd().collect::<Vec<_>>()), nfd, "nfd idempotent");
    assert_eq!(text(&nfkc.nfkc().collect::<Vec<_>>()), nfkc, "nfkc idempotent");
    assert_eq!(text(&nfkd.nfkd().collect::<Vec<_>>()), nfkd, "nfkd idempotent");
    assert_eq!(text(&nfc.nfd().collect::<Vec<_>>()), nfd, "nfd of nfc");
    assert_eq!(text(&nfkc.nfkd().collect::<Vec<_>>()), nfkd, "nfkd of nfkc");

    assert!(is_nfc(&nfc) && is_nfd(&nfd) && is_nfkc(&nfkc) && is_nfkd(&nfkd));
    let _ = is_nfc_quick(s.chars());
    let _ = is_nfd_quick(s.chars());
    let _ = is_nfkc_quick(s.chars());
    let _ = is_nfkd_quick(s.chars());
    let _ = is_nfc(s);
    let _ = is_nfd(s);
    let _ = is_nfkc(s);
    let _ = is_nfkd(s);

    for c in s.chars() {
        let _ = char::canonical_combining_class(c);
        let _ = char::is_combining_mark(c);
        char::decompose_canonical(c, |_| {});
        char::decompose_compatible(c, |_| {});
    }
    let mut prev = None;
    for c in s.chars() {
        if let Some(p) = prev {
            let _ = char::compose(p, c);
        }
        prev = Some(c);
    }
}

fuzz_target!(|data: &[u8]| {
    check(&String::from_utf8_lossy(data));
    if let Ok(s) = core::str::from_utf8(data) {
        check(s);
    }
});
