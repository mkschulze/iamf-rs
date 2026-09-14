---
created: 2026-09-14T00:00:00.000Z
title: Run the full libiamf v1.1.0 vector corpus and consolidate duplicated test scaffolding
area: testing
severity: minor
files:
  - tests/conformance.rs
  - tests/support/
  - tests/encoder_builder.rs
  - tests/encoder_streaming.rs
  - tests/profile.rs
  - tools/build-reference.sh
---

## Problem

**Vector coverage.** The pinned reference (`.reference/libiamf`, `f06e919`, built by
`tools/build-reference.sh`) ships 221 test vectors: `test_NNNNNN.iamf`, a `.textproto` per vector
(`is_valid` true for 193, false for 28; `is_valid_to_decode`; `primary_tested_spec_sections`),
`_f.mp4`/`_s.mp4`, `_decoded_substream_x.wav`, `_rendered_*.wav`, and `tests/coverage.csv` mapping
~200 spec fields to vectors. The suite exercises essentially `test_000003` plus its own generated
fixtures. The corpus is largely unused.

**Test duplication** (read-only audit, 2026-09-14; counts measured with grep/wc, savings estimated
from sampling). `tests/**/*.rs` is 20,187 lines, 469 tests. Reducible without losing coverage:
~1,000–1,400 lines (5–7%). The suite is not bloated; most bulk is legitimate (`conformance.rs`
harness, `support/fixture.rs` generator, per-field `descriptors.rs`, citation/golden/manifest
checks).

- Helpers redefined across files: `lpcm_config` ×4, `stereo_element` ×3, `presentation` ×3,
  `sha256_hex` ×3 (`golden.rs:82`, `parse_reference.rs:101`, `fuzz_regression.rs:176`),
  `rust_files_under` ×3, `repo_root` ×2, `sha_on_line_naming` ×2, `PartialThenFail` ×2
  (`encoder_streaming.rs:225`, `sequence.rs:259`).
- Error-assert boilerplate: 113 `kind()` + 84 `at()` asserts; 52 `EncoderBuilder::new()` and 49
  `add_codec_config(lpcm_config())` in `encoder_builder.rs`; 22 repeated `bytes_written()` checks in
  `encoder_streaming.rs`.
- Near-duplicate clusters: `obu_header.rs:361-453` (6 truncation tests), `profile.rs:199-484`
  (12 tier tests), `encoder_builder.rs:564-737` (9 profile-selection tests),
  `encoder_streaming.rs:686-857` (10 Opus pre-skip tests).
- Builder-side presentation rules (`encoder_builder.rs:187-548`) re-test what the validators already
  test in `sequence_parse.rs:795-1131`.

## Solution

1. **Vector harness** (test-only, corpus fetched at the pinned revision, never vendored — ~2.6 GB):
   - parse all 221 `.iamf`; accept/reject must agree with `is_valid`/`is_valid_to_decode`, with any
     stricter-than-libiamf rejection recorded as a `DIFF-LEDGER.md` entry (libiamf is permissive —
     see `CONFORMANCE-GATE.md`);
   - parse → serialise round-trip byte-identical for every accepted vector;
   - **encoder conformance:** build each `.textproto` configuration through the encoder API and compare
     the output byte-for-byte with the `.iamf` (explicit, documented skip list for configurations the
     API cannot express yet);
   - report per-spec-field coverage from `coverage.csv`.
   Hand-written strict negative tests stay; happy-path tests fully subsumed by the corpus may be removed.
2. **Consolidate scaffolding:** `tests/support/builders.rs` with the shared helpers above plus
   `assert_error(result, kind, at)` / `assert_rejected(writer, unit, kind)` (~500 lines).
3. **Table-drive** profile tiers (`profile.rs` + `encoder_builder.rs:564-737`, with a reason column,
   ~250 lines) and the builder-side presentation rules (keep precedence tests, ~150–200 lines).

Related: `baylesj/iamf-rs` as an independent differential oracle — Parallax
`.planning/todos/pending/2026-09-14-baylesj-iamf-rs-as-differential-oracle.md`.
