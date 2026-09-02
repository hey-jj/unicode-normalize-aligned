# unicode-normalize-aligned

Unicode normalization (NFC, NFD, NFKC, NFKD) where every output character
carries an `isize` that aligns it to the input. Built on Unicode 17.0.0 data.

```rust
use unicode_normalize_aligned::UnicodeNormalization;

let pairs: Vec<(char, isize)> = "A\u{30A}".nfc().collect();
// U+0041 U+030A composes to U+00C5, which replaces two input chars.
assert_eq!(pairs, [('\u{C5}', -1)]);
```

## The alignment tag

Each output item is `(char, isize)`. The tag is a change count in chars,
consumed positionally in output order:

| tag | meaning for this output char |
|---|---|
| `0` | replaces exactly one input char |
| `+1` | newly inserted, consumes no input |
| `-N` | replaces one input char and removes the `N` input chars after it |

Three rules produce every tag. A character that fully decomposes to
`d1..dn` yields `d1` tagged `0` and the rest tagged `+1`. Canonical
reordering moves characters together with their tags. Composing two
characters with tags `a` and `b` yields one character tagged `a + b - 1`.

Two invariants hold for every input and every form, and the test suite
checks them across all of NormalizationTest: the tags sum to the output
length minus the input length, and walking the output while consuming
`1 - min(tag, 0)` input positions per non-inserted character lands exactly
on the input end.

The tag contract is the per-character change count HuggingFace tokenizers
consumes. Adopting the crate is one dependency line and one `use` path.
Characters added to Unicode after 9.0.0 normalize under the current tables.

## API

- `UnicodeNormalization` trait on `&str` and on any `Iterator<Item = char>`:
  `nfd()`, `nfkd()`, `nfc()`, `nfkc()`, each yielding `(char, isize)`.
- Quick checks `is_nfc_quick`, `is_nfd_quick`, `is_nfkc_quick`,
  `is_nfkd_quick` returning `Yes`, `No` or `Maybe`, and `is_nfc`, `is_nfd`,
  `is_nfkc`, `is_nfkd`, which resolve `Maybe` by normalizing and comparing.
- `char::canonical_combining_class`, `char::is_combining_mark`,
  `char::compose`, `char::decompose_canonical`, `char::decompose_compatible`.

## What the test suite pins

- `devkit/check/tests/normalization_test.rs` runs all 400,680 assertions of
  NormalizationTest 17.0.0, plus the invariance clause for every assigned
  code point absent from its Part 1.
- `tests/alignment_vectors.rs` matches 102 curated inputs across all four
  forms, 408 `(char, isize)` sequences, character for character and tag
  for tag.
- `devkit/check/tests/no_alloc.rs` counts heap allocations with a wrapping allocator:
  zero on every NormalizationTest input and on any stream-safe text
  (UAX15-D4), one per spilled buffer on longer combining runs.
- `benches/throughput.rs` fails CI when this crate drops below 0.90x the
  throughput of `unicode-normalization` 0.1.25 on any of six corpora:
  ASCII, accented Latin, decomposed Vietnamese, Hangul jamo, pointed
  Arabic, emoji ZWJ.
- Table generation uses the generator and raw UCD 17.0.0 data in the sibling
  `devkit/` directory. From `devkit/`, run `cargo run -p gen --release` to
  regenerate `src/tables.rs`. The published package contains the generated
  tables.
- `fuzz/` feeds arbitrary input through every form, quick check and `char`
  query, asserting idempotence, the cross-form equalities and the
  alignment invariants on each run.

The library is `no_std` with `alloc`, has zero runtime dependencies, and
compiles under `#![forbid(unsafe_code)]`. MSRV is 1.63: the library builds
and its doctests pass there. Development dependencies may need a newer
toolchain.

## License

The Rust source code is licensed under MIT OR Apache-2.0, at your option.
The generated tables in `src/tables.rs` are derived from Unicode Character
Database 17.0.0 and are covered by the Unicode License v3 (Unicode-3.0),
reproduced in `LICENSE-UNICODE`.
