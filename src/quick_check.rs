//! UAX #15 section 9 quick checks and the authoritative `is_nfX` predicates.

use crate::props::{Props, QuickCheck};
use crate::UnicodeNormalization;

/// Answer of a quick check.
///
/// `Maybe` means the quick check cannot decide from per-character properties
/// alone. The `is_nfc`, `is_nfd`, `is_nfkc` and `is_nfkd` functions resolve it
/// by normalizing and comparing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNormalized {
    /// The text is in the normalization form.
    Yes,
    /// The text is not in the normalization form.
    No,
    /// Undecidable without normalizing.
    Maybe,
}

#[derive(Clone, Copy)]
enum Form {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

fn quick_check<I: Iterator<Item = char>>(s: I, form: Form) -> IsNormalized {
    let mut last_ccc = 0u8;
    let mut result = IsNormalized::Yes;
    for c in s {
        if (c as u32) < 0x80 {
            last_ccc = 0;
            continue;
        }
        let p = Props::of(c);
        let ccc = p.ccc();
        if ccc != 0 && last_ccc > ccc {
            return IsNormalized::No;
        }
        let qc = match form {
            Form::Nfc => p.nfc_qc(),
            Form::Nfkc => p.nfkc_qc(),
            Form::Nfd => {
                if p.nfd_no() {
                    QuickCheck::No
                } else {
                    QuickCheck::Yes
                }
            }
            Form::Nfkd => {
                if p.nfkd_no() {
                    QuickCheck::No
                } else {
                    QuickCheck::Yes
                }
            }
        };
        match qc {
            QuickCheck::No => return IsNormalized::No,
            QuickCheck::Maybe => result = IsNormalized::Maybe,
            QuickCheck::Yes => {}
        }
        last_ccc = ccc;
    }
    result
}

/// Quick check for NFC over a character iterator.
pub fn is_nfc_quick<I: Iterator<Item = char>>(s: I) -> IsNormalized {
    quick_check(s, Form::Nfc)
}

/// Quick check for NFD over a character iterator.
pub fn is_nfd_quick<I: Iterator<Item = char>>(s: I) -> IsNormalized {
    quick_check(s, Form::Nfd)
}

/// Quick check for NFKC over a character iterator.
pub fn is_nfkc_quick<I: Iterator<Item = char>>(s: I) -> IsNormalized {
    quick_check(s, Form::Nfkc)
}

/// Quick check for NFKD over a character iterator.
pub fn is_nfkd_quick<I: Iterator<Item = char>>(s: I) -> IsNormalized {
    quick_check(s, Form::Nfkd)
}

/// Whether `s` is in NFC. Resolves a `Maybe` quick check by normalizing and
/// comparing, without building a string.
pub fn is_nfc(s: &str) -> bool {
    match is_nfc_quick(s.chars()) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => s.nfc().map(|p| p.0).eq(s.chars()),
    }
}

/// Whether `s` is in NFD. Resolves a `Maybe` quick check by normalizing and
/// comparing, without building a string.
pub fn is_nfd(s: &str) -> bool {
    match is_nfd_quick(s.chars()) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => s.nfd().map(|p| p.0).eq(s.chars()),
    }
}

/// Whether `s` is in NFKC. Resolves a `Maybe` quick check by normalizing and
/// comparing, without building a string.
pub fn is_nfkc(s: &str) -> bool {
    match is_nfkc_quick(s.chars()) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => s.nfkc().map(|p| p.0).eq(s.chars()),
    }
}

/// Whether `s` is in NFKD. Resolves a `Maybe` quick check by normalizing and
/// comparing, without building a string.
pub fn is_nfkd(s: &str) -> bool {
    match is_nfkd_quick(s.chars()) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => s.nfkd().map(|p| p.0).eq(s.chars()),
    }
}
