---
phase: quick-260913-js8
plan: 01
subsystem: parser-hardening
status: complete
tags: [guard-03, parse-01, fuzz-05, allocation, dos]
requires: []
provides:
  - "crate::bits::bounded_vec and MAX_PREALLOCATION_BYTES (64 KiB)"
  - "tests/allocation_bounds.rs hostile-count tests and source-scan reservation gate"
affects:
  - src/obu/mix_presentation.rs
  - src/obu/audio_element.rs
  - src/obu/param_definition.rs
  - src/obu/parameter_block.rs
  - src/obu/audio_frame.rs
tech-stack:
  added: []
  patterns:
    - "Every parse-path reservation goes through bits::bounded_vec; bytes_remaining() checks stay as the early typed error"
key-files:
  created:
    - tests/allocation_bounds.rs
  modified:
    - src/bits/reader.rs
    - src/bits/mod.rs
    - src/obu/mix_presentation.rs
    - src/obu/audio_element.rs
    - src/obu/param_definition.rs
    - src/obu/parameter_block.rs
    - src/obu/audio_frame.rs
    - .planning/codebase/CONCERNS.md
decisions:
  - "MAX_PREALLOCATION_BYTES = 1 << 16 caps any single parser reservation; larger collections grow by push"
  - "src/encoder.rs and src/packing.rs are excluded from the reservation scan: capacity comes from caller-owned collections"
requirements: [GUARD-03, PARSE-01, FUZZ-05]
metrics:
  duration: "~25 min"
  completed: 2026-09-13
actuals:
  tokens: 6700
  tasks: 3
  commits: 3
plan_head_before: 3db396c38380c163aba67ba035e1b38564690091
---

# Quick 260913-js8 Plan 01: Bounded count-field preallocation Summary

The parsers used to reserve capacity straight from a parsed count. All 12 of those reservations now go through one helper, `bounded_vec::<T>(count)`. It never reserves more than 64 KiB of elements, and a source scan fails the build if a parser reserves any other way.

## Tasks

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 (tracer) | Bounded helper wired through the Mix Presentation reader | 98a9902 | src/bits/reader.rs, src/bits/mod.rs, src/obu/mix_presentation.rs, tests/allocation_bounds.rs |
| 2 | Convert remaining 7 sites and add the source-scan gate | 1c5a5dd | src/obu/{audio_element,param_definition,parameter_block,audio_frame}.rs, tests/allocation_bounds.rs |
| 3 | Full gates and close the CONCERNS.md entry | 8b7657f | .planning/codebase/CONCERNS.md |

## What changed

- `src/bits/reader.rs` adds `MAX_PREALLOCATION_BYTES` (1 << 16) and a private `const fn bounded_capacity<T>`. That function returns `count`, or `MAX / size_of::<T>()` if that is smaller; for zero-sized types it returns `count`. The only parse-path reservation is in `pub(crate) fn bounded_vec<T>`. The module doc's "cap before reserve" rule now also names the helper. Inline unit tests prove the bound.
- `src/bits/mod.rs` re-exports `bounded_vec` as `pub(crate)`.
- All 12 sites now use `bounded_vec(n)`: 5 in mix_presentation, 3 in audio_element, 1 in param_definition, 2 in parameter_block and 1 in audio_frame. Every `bytes_remaining()` check, error kind, location and read order is unchanged.
- `tests/allocation_bounds.rs` adds 4 hostile-count tests that assert the exact kind and offset:
  - Mix Presentation: offset 3 (small buffer) and offset 5 (2 MiB buffer)
  - Audio Element: offset 10 (small buffer) and offset 12 (2 MiB buffer)

  It also adds a sorted `CARGO_MANIFEST_DIR` scan. The scan requires exactly one reservation token under `src/`, not counting `encoder.rs` and `packing.rs`.

## TDD evidence

**Task 1 RED** (`bash tools/cargo-test-tap.sh --locked --lib bits::reader`, before the helper existed):
```
error[E0432]: unresolved imports `super::MAX_PREALLOCATION_BYTES`, `super::bounded_capacity`, `super::bounded_vec`
error: could not compile `iamf` (lib test) due to 1 previous error
```
GREEN: 5 unit tests passed, and both Mix Presentation hostile tests passed.

**Task 2 RED** (`bash tools/cargo-test-tap.sh --locked --test allocation_bounds`, before conversion):
```
expected exactly one capacity reservation under src/ (inside bounded_vec in src/bits/reader.rs), found 8. Route each parse-path reservation through crate::bits::bounded_vec:
  src/bits/reader.rs:66
  src/obu/audio_element.rs:563
  src/obu/audio_element.rs:656
  src/obu/audio_element.rs:791
  src/obu/audio_frame.rs:349
  src/obu/param_definition.rs:300
  src/obu/parameter_block.rs:452
  src/obu/parameter_block.rs:735
test result: FAILED. 4 passed; 1 failed
```
GREEN: all 5 allocation_bounds tests passed. The structural grep found exactly 1 reservation, and there are 12 `bounded_vec(` call sites under `src/obu`.

## Verification results

- `cargo fmt --all -- --check`: passed
- `cargo clippy --locked --all-targets -- -D warnings`: passed
- `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`: passed
- `cargo test --locked --features fuzzing --test fuzz_regression`: 5 passed
- `cargo test --locked --release --test allocation_bounds`: 5 passed
- `cargo test --locked --no-fail-fast`: every target passed except **one failure in `tests/conformance.rs`**. The result there was 26 passed, 1 failed:
  ```
  test parallax_delivery_fixture_uses_the_offline_safe_reference_gates ... FAILED
  CONF-06 FAILED: decoder_main died on an absl CHECK abort, which produces no temporal-unit line at all.
  E ... obu_processor.cc:492] INVALID_ARGUMENT: Audio elements in a submix must have the same number of samples per frame.
  ```
  **This failure already existed.** It fails the same way at baseline `3db396c`, checked in a throwaway scratch worktree that has since been removed. It is about `iamf-tools` rejecting the encoder's Parallax delivery fixture, and it only runs when the local `iamf-tools` container is present. Reservation capacity never reaches output bytes, and the golden byte-identity tests pass (11/11). The failure is logged in `deferred-items.md` and was not fixed, because it is outside this task's scope.

## Deviations from Plan

**1. [Rule 3 - Blocking] Local mirror of ENTIRE_OBU_SIZE_MAX in the reader unit tests**
- **Found during:** Task 1
- **Issue:** `crate::obu::header` is a private module of `obu`, so `src/bits/reader.rs` cannot name `ENTIRE_OBU_SIZE_MAX`. Re-exporting it as `pub(crate)` would trigger an `unused_imports` warning in non-test builds, and `-D warnings` would fail.
- **Fix:** The test module declares `const ENTIRE_OBU_SIZE_MAX: usize = 1 << 21;` with a comment that points at the original. The `usize::MAX` case in the same test covers every count regardless.
- **Commit:** 98a9902

**2. Commits on `main`:** the orchestrator told me to run sequentially on `main` with no worktree, which matches the project's existing history. The executor's protected-branch halt was not applied.

## Known Stubs

None.

## Threat Flags

None. The change narrows the attack surface and adds no new surface. T-js8-01 to T-js8-03 are mitigated as planned. T-js8-04 is accepted and recorded as a residual in CONCERNS.md.

## Self-Check: PASSED

- FOUND: src/bits/reader.rs (`pub(crate) fn bounded_vec`), src/bits/mod.rs, tests/allocation_bounds.rs (204 lines)
- FOUND commits: 98a9902, 1c5a5dd, 8b7657f
