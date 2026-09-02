//! Unicode normalization (NFC, NFD, NFKC, NFKD) where every output character
//! carries an `isize` that aligns it to the input.
//!
//! ```
//! use unicode_normalize_aligned::UnicodeNormalization;
//!
//! // U+2126 OHM SIGN maps to U+03A9 GREEK CAPITAL LETTER OMEGA.
//! let pairs: Vec<(char, isize)> = "\u{2126} A\u{30A}".nfc().collect();
//! assert_eq!(pairs, [('\u{3A9}', 0), (' ', 0), ('\u{C5}', -1)]);
//! ```
//!
//! # The alignment tag
//!
//! Each output item is `(char, isize)`. The tag is a change count in chars,
//! consumed positionally in output order:
//!
//! * `0`: this character replaces exactly one input character.
//! * `+1`: this character is newly inserted and consumes no input.
//! * `-N`: this character replaces one input character and removes the `N`
//!   input characters after it.
//!
//! Three rules produce every tag. A character that fully decomposes to
//! `d1..dn` yields `d1` tagged `0` and `d2..dn` tagged `+1`. Canonical
//! reordering moves characters together with their tags. Composing two
//! characters with tags `a` and `b` yields one character tagged `a + b - 1`.
//! Everything else passes through tagged `0`.
//!
//! Two invariants follow for every input: the tags sum to the output length
//! minus the input length, and walking the output while consuming
//! `1 - min(tag, 0)` input positions per non-inserted character lands exactly
//! on the input end.
//!
//! # Scope
//!
//! Tables are generated from Unicode 17.0.0 and the crate passes the full
//! NormalizationTest for that version. The crate is `no_std`, has no runtime
//! dependencies, and normalizes any stream-safe text (UAX15-D4) without
//! allocating. A combining run of 32 or more characters spills the
//! decomposition buffer once, and a run of 33 or more allocates one
//! ordering scratch when it closes. NFC and NFKC spill a second
//! blocked-mark buffer from 34 combining characters. The first mark
//! composes with the starter, so that buffer holds one fewer character
//! than the run.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod buffer;
mod decompose;
mod props;
mod quick_check;
mod recompose;
// Machine-written file, kept byte-identical to the generator's output, so
// rustfmt must not rewrap it. The devkit regeneration check diffs the result.
#[rustfmt::skip]
mod tables;

pub mod char;

pub use crate::decompose::Decompositions;
pub use crate::quick_check::{
    is_nfc, is_nfc_quick, is_nfd, is_nfd_quick, is_nfkc, is_nfkc_quick, is_nfkd, is_nfkd_quick,
    IsNormalized,
};
pub use crate::recompose::Recompositions;
pub use crate::tables::UNICODE_VERSION;

use crate::decompose::Kind;

/// Iterator adapters for the four normalization forms.
///
/// Implemented for `&str` and for every `Iterator<Item = char>`. Every
/// adapter yields `(char, isize)` pairs. See the crate docs for the tag
/// contract.
pub trait UnicodeNormalization<I: Iterator<Item = char>> {
    /// Canonical decomposition with canonical ordering.
    fn nfd(self) -> Decompositions<I>;

    /// Compatibility decomposition with canonical ordering.
    fn nfkd(self) -> Decompositions<I>;

    /// Canonical decomposition followed by canonical composition.
    fn nfc(self) -> Recompositions<I>;

    /// Compatibility decomposition followed by canonical composition.
    fn nfkc(self) -> Recompositions<I>;
}

impl<'a> UnicodeNormalization<core::str::Chars<'a>> for &'a str {
    fn nfd(self) -> Decompositions<core::str::Chars<'a>> {
        Decompositions::new(self.chars(), Kind::Canonical)
    }

    fn nfkd(self) -> Decompositions<core::str::Chars<'a>> {
        Decompositions::new(self.chars(), Kind::Compatible)
    }

    fn nfc(self) -> Recompositions<core::str::Chars<'a>> {
        Recompositions::new(self.nfd())
    }

    fn nfkc(self) -> Recompositions<core::str::Chars<'a>> {
        Recompositions::new(self.nfkd())
    }
}

impl<I: Iterator<Item = char>> UnicodeNormalization<I> for I {
    fn nfd(self) -> Decompositions<I> {
        Decompositions::new(self, Kind::Canonical)
    }

    fn nfkd(self) -> Decompositions<I> {
        Decompositions::new(self, Kind::Compatible)
    }

    fn nfc(self) -> Recompositions<I> {
        Recompositions::new(self.nfd())
    }

    fn nfkc(self) -> Recompositions<I> {
        Recompositions::new(self.nfkd())
    }
}
