---
created: 2026-09-14T00:00:00.000Z
updated: 2026-09-25T00:00:00.000Z
title: Remaining reference-vector work after the disposition harness, plus test-scaffolding consolidation
area: testing
severity: minor
files:
  - tests/refvectors.rs
  - tests/support/vector_ledger.rs
  - tests/support/
  - tests/encoder_builder.rs
  - tests/encoder_streaming.rs
  - tests/profile.rs
  - src/sequence.rs
---

## What already landed — quick task `260924-tvf` (2026-09-25)

This todo was written on 2026-09-14 as one item covering a vector harness *and* test consolidation.
The harness half is now partly built; the rest is below. Do not re-plan the parts marked done.

Commits `39f00fc`, `ab8fe28`, `e9921e2`, docs `f149c3d`. Artefacts:
`.planning/quick/260924-tvf-libiamf-vector-harness-run-the-full-pinn/`.

- **Done:** `tests/refvectors.rs` — one `walk()` at two roots: the vendored corpus
  (`tests/fixtures/reference`, always on, all four targets) and the fetched corpus
  (`.reference/libiamf/tests`, gated, prints `SKIP` offline). It joins each `.iamf` to its paired
  upstream `.textproto`, scans `is_valid` / `is_valid_to_decode`, classifies into a four-cell matrix,
  asserts the OBU boundary walk ends on the file length, and classifies parse→write as
  identical / canonicalized / diverged.
- **Done:** `tests/support/vector_ledger.rs` — 35 committed disposition rows plus
  `STRICTER_THAN_REFERENCE`, asserted as an exact bijection with what the walker produces, so a
  disposition drift is a reviewable red diff.
- **Done:** the `CONFORMANCE-GATE.md` section recording the reverse direction (where this crate is
  stricter than the permissive `libiamf`), and the `.github/workflows/reference.yml` step that fails
  the Linux job if the corpus layer skips there.
- **Measured at that commit:** over the vendored 34 pairs, `is_valid: true` → 2 clean / 20 findings /
  **0 rejected**; `is_valid: false` → 0 clean / 11 findings / 1 rejected. The RED cell is empty, so
  `STRICTER_THAN_REFERENCE` is a measured-empty slice, not a placeholder. All 34 parsed files
  round-trip byte-identical.
- **Key correction to the original text below:** the premise "the suite exercises essentially
  `test_000003`" was wrong. `tests/parse_reference.rs:45-80` already asserted an exact bijection over
  38 positive + 2 negative foreign `.iamf`, each pinned by a `semantic_sha256`. What was genuinely
  missing — and is now present — is that **nothing read the upstream `is_valid` declaration at all**.

## Remaining track 1 — encoder conformance from each `.textproto` (phase-sized, NOT a quick task)

Build each vector's configuration through the encoder API and compare the output byte-for-byte with
the `.iamf`, with a documented skip list. Why this is not a quick task:

- It needs a hand-rolled reader for the **old** `UserMetadata` proto dialect — libiamf's textprotos
  still carry `count_label`, `num_substreams`, `num_layers`, `num_sub_mixes`, `num_audio_elements`,
  `num_layouts`, `param_definition_size` (`tests/fixtures/MANIFEST.md:65-71`), so they must not be fed
  to `encoder_main`.
- No textproto crate is on `deny.toml`'s allow-list, and the linked graph must stay `iamf` +
  `thiserror`.
- The existing `textproto_for` writer (`tests/conformance.rs:2698-2735`) handles exactly one Codec
  Config, one Audio Element and one Mix Presentation, LPCM only.
- Every non-LPCM vector is **permanently** unencodeable here: this crate frames pre-encoded access
  units and never encodes them.

Route through `/gsd-phase` (new phase) or `/gsd-quick --discuss --research`, not a bare quick task.

## Remaining track 2 — the `coverage.csv` per-spec-field report (quick task)

Report per-spec-field coverage from the pinned `tests/coverage.csv` (~200 spec fields mapped to
vectors). Do this only now that the ledger exists, so the report has a committed floor to assert
against instead of being a printout nobody reads.

## Remaining track 3 — test-scaffolding consolidation (quick task, on its own)

**Never in the same commit as a harness that adds test surface** — a diff that both ADDS and MOVES
test surface makes a regression ambiguous between "the new harness found a real bug" and "the refactor
dropped an assertion".

Re-measured 2026-09-24 (the tree grew ~9% since this todo was written; recount before planning —
`260924-tvf` added ~1 000 more test lines on top):

- 22 017 lines / 609 `#[test]` across `tests/**/*.rs` (the original text below said 20 187 / 469).
- `.kind()` asserts: 182 (was 113). `bytes_written()`: 47 (was 22).
- **Only five are true mechanical duplicates**: `sha256_hex` ×3, `rust_files_under` ×3, `repo_root` ×2,
  `sha_on_line_naming` ×2, `PartialThenFail` ×2. Start there.
- `lpcm_config` / `stereo_element` / `presentation` are **not** true duplicates — their signatures
  differ, so unifying them is a design exercise over ~100 call sites, not a move.
- **Budget for the clippy carve-out boundary:** `clippy.toml`'s unwrap/expect/panic allowance applies
  inside `#[test]` bodies **only**. A panicking assertion moved into a shared helper trips
  `-D warnings`. Helpers must be total. Do not paper over it with a file-level `#[allow]` — the escape
  census counts those.
- Guards are safe from test-side refactoring: `tests/citations.rs:37` walks `src/` only, and
  `tools/prove-guards.sh:67-77` copies `src/` only. **One trap:** do not add an unconditional
  `[[test]]` entry to `Cargo.toml` — the proof crate has no `tests/` dir and runs
  `cargo clippy --all-targets`, so an unconditional entry breaks all 9 guard cases.

## Remaining track 4 — the duplicate-`parameter_id` finding (needs a `src/` slice)

Discovered while measuring: one finding is present on **31 of 35** vendored vectors —
`parameter_id N appears at OBU indices i and i` (`src/sequence.rs:498`). It is factually correct: the
reference vectors deliberately give `element_mix_gain` and `output_mix_gain` the **same**
`parameter_id` inside one Mix Presentation (`test_000003.textproto:104` and `:114` are both
`parameter_id: 100`), and first-in-wire-order binding is the documented Phase 02 decision.

The problem is signal value: a `Finding` that fires on the overwhelming majority of canonical,
upstream-valid files is not useful to Parallax. Deciding this means a `src/` change **and** a change
to the Parallax-facing error surface, so it needs its own slice with the consumer contract in view.
Recorded in `CONFORMANCE-GATE.md` under the section `260924-tvf` added.

## Remaining track 5 — tighten `MIN_CORPUS_VECTORS` (trivial, blocked on one CI run)

`tests/refvectors.rs` carries a conservative floor of 200. Tighten it to the count the first green
Linux `reference` run actually prints, citing that run. The floor exists to catch a half-materialised
checkout, not to pin the upstream census. `MANIFEST.md:343` records 221 `.iamf` + 221 `.textproto` at
the pinned tag.

## Original problem statement (2026-09-14, kept for provenance)

**Vector coverage.** The pinned reference (`.reference/libiamf`, `f06e919`, built by
`tools/build-reference.sh`) ships 221 test vectors: `test_NNNNNN.iamf`, a `.textproto` per vector
(`is_valid` true for 193, false for 28; `is_valid_to_decode`; `primary_tested_spec_sections`),
`_f.mp4`/`_s.mp4`, `_decoded_substream_x.wav`, `_rendered_*.wav`, and `tests/coverage.csv` mapping
~200 spec fields to vectors. *(Superseded in part: see "What already landed" above — the corpus is
now walked, and the claim that the suite ran only `test_000003` was wrong.)*

**Test duplication** (read-only audit, 2026-09-14). `tests/**/*.rs` was 20 187 lines, 469 tests.
Reducible without losing coverage: ~1 000–1 400 lines (5–7%). The suite is not bloated; most bulk is
legitimate (`conformance.rs` harness, `support/fixture.rs` generator, per-field `descriptors.rs`,
citation/golden/manifest checks). *(Counts superseded — see track 3.)*

Related: `baylesj/iamf-rs` as an independent differential oracle — Parallax
`.planning/todos/pending/2026-09-14-baylesj-iamf-rs-as-differential-oracle.md`.
