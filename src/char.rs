//! Per-character queries backed by the same tables as the iterators.

use crate::props::{self, hangul, Props};

/// Canonical combining class (ccc) of the character. 0 for starters.
#[inline]
pub fn canonical_combining_class(c: char) -> u8 {
    Props::of(c).ccc()
}

/// Whether the character's General_Category is Mn, Mc or Me.
#[inline]
pub fn is_combining_mark(c: char) -> bool {
    Props::of(c).is_combining_mark()
}

/// Canonical composition of a pair. Returns the primary composite, or the
/// Hangul syllable the pair forms. Excluded composites and singleton
/// decompositions never compose, so they return `None`.
#[inline]
pub fn compose(a: char, b: char) -> Option<char> {
    hangul::compose(a, b).or_else(|| props::compose_pair(a, b))
}

/// Emits the full canonical decomposition of the character, one `char` at a
/// time. A character with no canonical mapping is emitted unchanged.
pub fn decompose_canonical<F: FnMut(char)>(c: char, mut emit: F) {
    if (c as u32) < 0x80 || !Props::of(c).nfd_no() {
        emit(c);
        return;
    }
    if let Some((l, v, t)) = hangul::decompose(c) {
        emit(l);
        emit(v);
        if let Some(t) = t {
            emit(t);
        }
        return;
    }
    match props::canonical(c) {
        Some(seq) => seq.iter().copied().for_each(emit),
        None => emit(c),
    }
}

/// Emits the full compatibility decomposition of the character, one `char` at
/// a time. A character with no mapping is emitted unchanged.
pub fn decompose_compatible<F: FnMut(char)>(c: char, mut emit: F) {
    let p = Props::of(c);
    if (c as u32) < 0x80 || !p.nfkd_no() {
        emit(c);
        return;
    }
    if let Some((l, v, t)) = hangul::decompose(c) {
        emit(l);
        emit(v);
        if let Some(t) = t {
            emit(t);
        }
        return;
    }
    let seq = if p.has_compat() {
        props::compat_distinct(c)
    } else {
        props::canonical(c)
    };
    match seq {
        Some(seq) => seq.iter().copied().for_each(emit),
        None => emit(c),
    }
}
