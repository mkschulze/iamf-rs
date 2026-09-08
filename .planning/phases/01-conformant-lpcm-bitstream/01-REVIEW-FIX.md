---
phase: 01-conformant-lpcm-bitstream
fixed_at: 2026-09-08T19:45:46Z
review_path: .planning/phases/01-conformant-lpcm-bitstream/01-REVIEW.md
iteration: 2
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 1: Code Review Fix Report

**Fixed at:** 2026-09-08T19:45:46Z
**Source review:** `.planning/phases/01-conformant-lpcm-bitstream/01-REVIEW.md`
**Iteration:** 2

**Summary:**
- Findings in scope: 6
- Fixed: 6
- Skipped: 0

## Fixed Issues

### CR-01: Parameter Block serialization loses the definition type that determines wire syntax

**Files modified:** `src/obu/parameter_block.rs`, `src/sequence.rs`, `tests/temporal.rs`
**Commits:** 98d04e5, ab9e0e1
**Applied fix:** `write_parameter_block` requires the governing definition type, validates every data variant before output, and `SequenceWriter` retains each definition together with its kind. Iteration 2 canonicalizes the public enum by wire value so `Reserved(0..=2)` cannot bypass Mix Gain/Demixing/Recon Gain semantics; `Reserved(>=3)` retains raw extension round-trip.
**Status:** fixed; requires human verification of the context-propagation logic.

### CR-02: Parameter Block writer emits impossible duration/subblock combinations

**Files modified:** `src/obu/parameter_block.rs`, `tests/temporal.rs`
**Commit:** 9baa496
**Applied fix:** one preflight validator now enforces exact subblock counts, the mode-dependent duration-field policy, and checked explicit-duration sums before writing `parameter_id`.
**Status:** fixed; requires human verification of the duration/count logic.

### CR-03: Annotation-count mismatches are serialized as shifted Mix Presentation payloads

**Files modified:** `src/error.rs`, `src/obu/mix_presentation.rs`, `tests/descriptors.rs`
**Commit:** aa4a6c6
**Applied fix:** top-level and per-element annotation cardinalities are checked against `count_label` before any payload byte is emitted, with `AnnotationCountMismatch` as the typed error.
**Status:** fixed; requires human verification of the cardinality logic.

### CR-04: Generic OBU parsing does not enforce the 2 MiB ceiling

**Files modified:** `src/obu/boundaries.rs`, `src/obu/header.rs`, `tests/obu_header.rs`
**Commit:** 23a691b
**Applied fix:** writer sizing, generic parsing, and boundary walking share one measured-size validator. Generic parsing rejects oversized claims before after-size allocation while retaining legal non-minimal ULEB128 support.
**Status:** fixed; requires human verification of the measured-size boundary logic.

### WR-01: A failed streaming write leaves a reusable writer pointing at partial output

**Files modified:** `src/error.rs`, `src/sequence.rs`, `tests/sequence.rs`
**Commit:** d4467fb
**Applied fix:** every temporal OBU is preflighted before the first unit flush; partial sink progress is counted; sink or mid-unit failures poison the writer; and later pushes or finish return `SequenceWriterPoisoned`.
**Status:** fixed; requires human verification of the streaming state-machine logic.

### WR-02: CI executes mutable action tags despite the repository's pinning policy

**Files modified:** `.github/workflows/ci.yml`, `.github/workflows/reference.yml`
**Commit:** f14bca0
**Applied fix:** all action uses are pinned to full upstream commit SHAs with release tag comments (`checkout` v4.4.0, `upload-artifact` v4.6.2, and `install-action` v2.87.8).

## Verification

- `cargo test --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `bash tools/prove-guards.sh` — passed, 6/6 deliberate violations rejected.
- `cargo fmt --all -- --check` — reports the pre-existing repository-wide rustfmt drift already documented in `deferred-items.md`; no formatter rewrite was included in these finding-scoped commits.
- Behavioral findings followed red/green TDD with focused tests that observed the original failure before implementation.
- Iteration 2 observed the `Reserved(0..=2)` regression fail before the canonicalization fix, then pass in the targeted and full suites.
- The standing 01-05 TDD waiver was preserved; prior history was not rewritten.

---

_Fixed: 2026-09-08T19:45:46Z_
_Fixer: Codex (gsd-code-fixer)_
_Iteration: 2_
