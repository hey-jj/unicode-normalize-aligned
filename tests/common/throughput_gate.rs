//! Shared 0.90x throughput comparison against `unicode-normalization` 0.1.25.

#![allow(clippy::incompatible_msrv)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use unicode_normalization::UnicodeNormalization as Baseline;
use unicode_normalize_aligned::UnicodeNormalization as Aligned;

pub const GATE_RATIO: f64 = 0.90;
pub const FORMS: [&str; 4] = ["nfc", "nfd", "nfkc", "nfkd"];

pub fn corpora() -> Vec<(&'static str, String)> {
    let ascii =
        "The quick brown fox jumps over the lazy dog; 1234567890 times it went. ".repeat(220);
    let latin1 =
        "Élève déjà çà et là, garçon naïf: mère, père, fenêtre. Grüße aus Köln, schöne Bäume. "
            .repeat(160);
    let vietnamese = "Tiếng Việt là ngôn ngữ của người Việt và là quốc ngữ. ".repeat(240);
    let vietnamese_nfd: String = Aligned::nfd(vietnamese.as_str()).map(|p| p.0).collect();
    let korean = "한국어의 자모 분해 형태를 시험한다 다람쥐 헌 쳇바퀴에 타고파. ".repeat(220);
    let jamo: String = Aligned::nfd(korean.as_str()).map(|p| p.0).collect();
    let arabic = "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0627}\u{0644}\u{0644}\u{0651}\u{0647}\u{0650} \u{0627}\u{0644}\u{0631}\u{0651}\u{064E}\u{062D}\u{0652}\u{0645}\u{064E}\u{0646}\u{0650} \u{0627}\u{0644}\u{0631}\u{0651}\u{064E}\u{062D}\u{0650}\u{064A}\u{0645}\u{0650} "
        .repeat(260);
    let emoji = "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466} \u{1F3F4}\u{200D}\u{2620}\u{FE0F} \u{1F468}\u{1F3FD}\u{200D}\u{1F692} \u{1F1FA}\u{1F1F8} "
        .repeat(300);
    vec![
        ("ascii", ascii),
        ("latin1", latin1),
        ("vietnamese-nfd", vietnamese_nfd),
        ("hangul-jamo", jamo),
        ("arabic-pointed", arabic),
        ("emoji-zwj", emoji),
    ]
}

pub fn ours(form: &str, text: &str) -> u64 {
    let mut sum = 0u64;
    match form {
        "nfc" => {
            for (c, t) in Aligned::nfc(text) {
                sum = sum.wrapping_add(c as u64).wrapping_add(t as u64);
            }
        }
        "nfd" => {
            for (c, t) in Aligned::nfd(text) {
                sum = sum.wrapping_add(c as u64).wrapping_add(t as u64);
            }
        }
        "nfkc" => {
            for (c, t) in Aligned::nfkc(text) {
                sum = sum.wrapping_add(c as u64).wrapping_add(t as u64);
            }
        }
        _ => {
            for (c, t) in Aligned::nfkd(text) {
                sum = sum.wrapping_add(c as u64).wrapping_add(t as u64);
            }
        }
    }
    sum
}

pub fn baseline(form: &str, text: &str) -> u64 {
    let mut sum = 0u64;
    match form {
        "nfc" => {
            for c in Baseline::nfc(text) {
                sum = sum.wrapping_add(c as u64);
            }
        }
        "nfd" => {
            for c in Baseline::nfd(text) {
                sum = sum.wrapping_add(c as u64);
            }
        }
        "nfkc" => {
            for c in Baseline::nfkc(text) {
                sum = sum.wrapping_add(c as u64);
            }
        }
        _ => {
            for c in Baseline::nfkd(text) {
                sum = sum.wrapping_add(c as u64);
            }
        }
    }
    sum
}

/// Best (smallest) wall time of `samples` runs of `f`.
fn best_of(samples: u32, mut f: impl FnMut() -> u64) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..samples {
        let start = Instant::now();
        black_box(f());
        best = best.min(start.elapsed());
    }
    best
}

/// Panics when this crate falls below `GATE_RATIO` times baseline throughput
/// on any corpus and form.
pub fn gate() {
    let mut worst: Option<(String, f64)> = None;
    for (name, text) in corpora() {
        for form in FORMS {
            // Warm both paths, then interleave best-of samples so a
            // frequency shift hits both sides alike.
            black_box(ours(form, &text));
            black_box(baseline(form, &text));
            let mut our_time = Duration::MAX;
            let mut base_time = Duration::MAX;
            for _ in 0..15 {
                our_time = our_time.min(best_of(3, || ours(form, black_box(&text))));
                base_time = base_time.min(best_of(3, || baseline(form, black_box(&text))));
            }
            let ratio = base_time.as_secs_f64() / our_time.as_secs_f64();
            println!("gate {name}/{form}: {ratio:.3}x baseline");
            assert!(
                ratio >= GATE_RATIO,
                "{name}/{form}: {ratio:.3}x is below the {GATE_RATIO}x floor"
            );
            if worst.as_ref().map_or(true, |w| ratio < w.1) {
                worst = Some((format!("{name}/{form}"), ratio));
            }
        }
    }
    if let Some((name, ratio)) = worst {
        println!("gate worst case {name}: {ratio:.3}x baseline");
    }
}
