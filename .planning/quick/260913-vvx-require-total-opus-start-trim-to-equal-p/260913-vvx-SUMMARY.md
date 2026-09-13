---
phase: quick-260913-vvx
plan: 01
subsystem: encoder
status: complete
tags: [opus, pre_skip, trimming, encoder, validation]
requires: [260913-n56 T3/T4 trim state in EncodingWriter]
provides: [Opus start-trim total == pre_skip enforcement in EncodingWriter]
affects: [src/encoder.rs, src/error.rs]
tech-stack:
  added: []
  patterns: [cross-unit state committed only after a successful sink push]
key-files:
  created:
    - .planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-deferred-items.md
  modified:
    - src/error.rs
    - src/encoder.rs
    - tests/encoder_streaming.rs
    - tests/error_shape.rs
decisions:
  - "Opus start-trim total must equal pre_skip exactly (spec index.bs:1819), stricter than iamf-tools' >=; recorded as DISAGREEMENT"
  - "Overshoot is rejected at push time, a short closing unit at push time, and a short open run (including an empty stream) at finish() after the poison check"
  - "Parse-side validate() finding deferred: it would change semantic_sha256 of three valid iamf-tools Opus fixtures"
requirements: [API-06, API-08, CONF-06]
metrics:
  duration: ~25min
  completed: 2026-09-13
actuals:
  tokens: 5500
  tasks: 3
  commits: 2
plan_head_before: e4866302b7efc2943fdd4bcd901c1a96e47378ec
---

# Quick 260913-vvx Plan 01: Opus start trim equals pre_skip Summary

`EncodingWriter` now accepts an Opus stream only if the start trims of its leading run add up to exactly `pre_skip`. It uses two payload-free `ErrorKind`s: `OpusStartTrimExceedsPreSkip` and `OpusStartTrimShortOfPreSkip`. The push checks use a `checked_add` running total that is saved only after a successful push. `finish()` checks the same total for a run that is still open.

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | Opus start-trim equality on the single-unit push path (tracer) | 9a1c25b |
| 2 | Multi-unit runs and the finish() open-run check | d176856 |
| 3 | Deferred items + release gates | (deferred-items file left for the orchestrator's docs commit) |

## Verification

- `cargo fmt --all -- --check`: ok. `cargo clippy --locked --all-targets -- -D warnings`, with and without `--features fuzzing`: clean.
- `env -u IAMF_REF_DECODER cargo test --locked`: `offline_exit=0`, 28 `test result: ok` lines, none failing.
- Doc tests: `test result: ok. 4 passed`. Fuzz replay (`fuzz_regression`): `test result: ok. 5 passed`.
- `docker image inspect iamf-tools:v2.1.0`: exit 0.
- Conformance with `IAMF_REF_DECODER` (perl alarm, 1200 s): `conf_exit=0`, `test result: ok. 29 passed; 0 failed`
  - `[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)`
  - `[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 2 temporal units." (exit Some(0), recorded but not the signal)`
  - Only the two expected SKIP CONF-05 lines, for phase3_flac and phase3_opus (`dep_codecs_disabled=true`).
- Protected paths (`src/sequence.rs`, `src/obu`, `tests/support`, `tests/fixtures`, golden, conformance, parse_reference, `DIFF-LEDGER.md`, `tools`, `fuzz`) are unchanged since e486630.

## Deviations from Plan

- **Task 3 commit skipped:** the plan said to commit `260913-vvx-deferred-items.md`. The orchestrator's constraints forbid committing this task's planning artifacts, so the file stays untracked for the orchestrator's docs commit.
- **Protected-branch guard:** commits went to `main`, as the orchestrator directed (sequential mode, no worktree), and followed earlier quick tasks. The executor's generic protected-branch assertion would have halted there.

Otherwise the plan was executed as written.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: src/error.rs, src/encoder.rs, tests/encoder_streaming.rs, tests/error_shape.rs, 260913-vvx-deferred-items.md
- FOUND commits: 9a1c25b, d176856
