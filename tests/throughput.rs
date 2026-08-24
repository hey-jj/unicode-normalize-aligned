//! Named 0.90x throughput assertion. Fails when this crate drops below the
//! floor against `unicode-normalization` 0.1.25 on any corpus and form.

// Dev-only target: never compiled at the MSRV floor, so post-1.63 APIs are fine.
#![allow(clippy::incompatible_msrv)]

#[path = "common/throughput_gate.rs"]
mod measure;

#[test]
fn throughput_at_least_0_90x_unicode_normalization() {
    // The 0.90x floor is calibrated for optimized builds and a debug
    // build lands just below it. Only the release run enforces the gate.
    if cfg!(debug_assertions) {
        eprintln!("skipping the throughput floor in a debug build");
        return;
    }
    measure::gate();
}
