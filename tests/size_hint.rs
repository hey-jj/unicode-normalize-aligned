//! Iterator `size_hint` lower bound never exceeds the remaining count.

use unicode_normalize_aligned::UnicodeNormalization;

fn assert_lower_never_exceeds_remaining<I>(mut it: I)
where
    I: Iterator + Clone,
{
    loop {
        let remaining = it.clone().count();
        let (lower, upper) = it.size_hint();
        assert!(
            lower <= remaining,
            "size_hint lower {lower} exceeds remaining {remaining}"
        );
        if let Some(upper) = upper {
            assert!(
                remaining <= upper,
                "size_hint upper {upper} is below remaining {remaining}"
            );
        }
        if it.next().is_none() {
            break;
        }
    }
}

#[test]
fn recompositions_size_hint_never_exceeds_actual() {
    // L+V+T jamo. NFC collapses the triple to one Hangul syllable.
    let jamo = "\u{1100}\u{1161}\u{11A8}";
    assert_lower_never_exceeds_remaining(jamo.nfc());
    assert_lower_never_exceeds_remaining(jamo.nfkc());

    // Precomposed Hangul syllable, NFC-stable.
    let hangul = "\u{AC01}";
    assert_lower_never_exceeds_remaining(hangul.nfc());
    assert_lower_never_exceeds_remaining(hangul.nfkc());

    // 300 input chars: 100 copies of the same jamo triple.
    let cycled = jamo.repeat(100);
    assert_eq!(cycled.chars().count(), 300);
    assert_lower_never_exceeds_remaining(cycled.nfc());
    assert_lower_never_exceeds_remaining(cycled.nfkc());
}

#[test]
fn decompositions_size_hint_saturates_on_unbounded_inner() {
    // `repeat` legally reports a `usize::MAX` lower bound. Adding the
    // buffered count must saturate instead of overflowing in debug.
    let mut it = core::iter::repeat('\u{FDFA}').nfkd();
    for _ in 0..40 {
        let (lower, upper) = it.size_hint();
        assert_eq!(lower, usize::MAX);
        assert_eq!(upper, None);
        assert!(it.next().is_some());
    }
}
