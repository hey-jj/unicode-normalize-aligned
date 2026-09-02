# Changelog

## 0.1.1 - 2026-09-02

- Moved raw UCD 17.0.0 data and the table generator to the sibling `devkit/` directory, out of the published package.
- Added `LICENSE-UNICODE` at the crate root and corrected `license` to `(MIT OR Apache-2.0) AND Unicode-3.0`.

## 0.1.0 - 2026-08-21

First release.

- NFC, NFD, NFKC and NFKD iterators yielding `(char, isize)` alignment
  pairs, on `&str` and on any `Iterator<Item = char>`.
- Quick checks and full `is_nfX` predicates for all four forms.
- `char` module: combining class, combining-mark test, pairwise composition,
  canonical and compatibility decomposition.
- Tables generated from Unicode 17.0.0 UCD files, pinned by sha256.
  Generated static data measures 116,650 bytes.
- Full NormalizationTest 17.0.0 conformance (400,680 assertions), alignment
  vector and invariant suites, allocation budget test, throughput gate
  against `unicode-normalization` 0.1.25, and a fuzz target.
- `no_std` with `alloc`, zero runtime dependencies, `forbid(unsafe_code)`,
  MSRV 1.63.
