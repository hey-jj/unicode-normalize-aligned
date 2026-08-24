//! Throughput of the four forms on six corpora, against `unicode-normalization`
//! 0.1.25 (Unicode 17.0.0, no alignment).
//!
//! The gate runs first and panics when this crate falls below 0.90x the
//! baseline throughput on any corpus and form. Criterion reporting follows,
//! skipped when `THROUGHPUT_GATE_ONLY=1`.

// Dev-only target: never compiled at the MSRV floor, so post-1.63 APIs are fine.
#![allow(clippy::incompatible_msrv)]
use std::hint::black_box;
use std::time::Duration;

use criterion::Criterion;

#[path = "../tests/common/throughput_gate.rs"]
mod measure;

fn benches(c: &mut Criterion) {
    for (name, text) in measure::corpora() {
        let mut group = c.benchmark_group(name);
        group.measurement_time(Duration::from_millis(600));
        group.warm_up_time(Duration::from_millis(200));
        for form in measure::FORMS {
            group.bench_function(format!("{form}/aligned"), |b| {
                b.iter(|| measure::ours(form, black_box(&text)));
            });
            group.bench_function(format!("{form}/baseline"), |b| {
                b.iter(|| measure::baseline(form, black_box(&text)));
            });
        }
        group.finish();
    }
}

fn main() {
    measure::gate();
    if std::env::var_os("THROUGHPUT_GATE_ONLY").is_some() {
        return;
    }
    let mut criterion = Criterion::default().configure_from_args();
    benches(&mut criterion);
    criterion.final_summary();
}
