# Changelog

## Changed

- Moved raw UCD 17.0.0 data and the table generator to the sibling `devkit/`
  directory. The published package no longer includes
  `ucd/LICENSE-UNICODE` or `ucd/17.0.0/MANIFEST.sha256`.

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
