---
phase: quick-260914-0sp
plan: 01
subsystem: obu-header-parsing
status: complete
tags: [parser-hardening, obu, review-finding, conformance]
requires: []
provides:
  - "read_obu_header_parts reads trim/extension fields through an obu_size-bounded, non-advancing window"
  - "private read_fields_after_obu_size with spec/iamf-tools/libiamf ref + DISAGREEMENT comments"
affects: [src/obu/header.rs, src/error.rs, src/obu/mod.rs, tests/obu_header.rs]
tech-stack:
  added: []
  patterns: ["clone().sub_reader(min(obu_size, remaining)) window, then advance parent by measured count"]
key-files:
  created: []
  modified: [src/obu/header.rs, src/error.rs, src/obu/mod.rs, tests/obu_header.rs]
decisions:
  - "After-size overrun of obu_size is TruncatedObu at InputOffset(obu_start), reusing the kind/offset read_obu_with already reported; no new ErrorKind"
  - "Window is min(obu_size, bytes_remaining) so header-only reads without the payload keep working"
requirements: [OBU-02, OBU-06, OBU-07, PARSE-01]
metrics:
  duration: "~15 min"
  completed: 2026-09-14
commits: 2
plan_head_before: 2d61359ddc76590aa12e763f090dcbc78b735cdf
actuals:
  tokens: 4500
  tasks: 2
  commits: 2
---

# Phase quick-260914-0sp Plan 01: Bound OBU header after-size fields by obu_size Summary

The OBU header's trim pair and extension header are now read through a `min(obu_size, bytes_remaining)` window cut from a cursor clone. An overrun of `obu_size` is `TruncatedObu` at the OBU start on both the header-only and whole-OBU paths, so a 2-byte OBU can no longer allocate a 1 MiB extension (review [HIGH], `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md:56`).

## Tasks

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Read the OBU header after-size fields inside obu_size, RED first | 1b76415 | src/obu/header.rs, src/error.rs, tests/obu_header.rs |
| 2 | Mark the payload underflow check as defensive, run release gates | 9c6ea4b | src/obu/mod.rs |

## TDD Gate Compliance

- RED: `target/0sp-red.log` on the base code: the six overrun tests (a1, a2, b1, b2, b3, c) FAILED; d, e2, e3, tightened e1 and the `min()` clamp control were ok (28 passed, 6 failed).
- GREEN: `obu_header` 34/34 after the fix.
- RED and GREEN were committed together in one fix commit, as the plan specified (single-commit task).

## Gate Results

- `cargo fmt --all -- --check`: pass
- `cargo build --locked --all-targets`: pass
- `cargo clippy --locked --all-targets -- -D warnings`: pass. Also passes with `--features fuzzing`.
- `cargo test --locked`: exit 0. Every binary ok, 547 passed, 0 failed. This includes golden, parse_reference, citations and sequence_parse. The research baseline was 538, and this change adds 9 tests.
- `cargo test --locked --features fuzzing --test fuzz_regression`: 5 passed
- Protected paths unchanged since 2d61359: reference_expectations, fixtures, golden.rs, DIFF-LEDGER.md, HANDOFF.md, fuzz, tools, boundaries.rs, dump.rs, sequence.rs, bits.
- Committed src/tests scope is exactly the four planned files.
- No semantic_sha256, golden or fixture change.

## Deviations from Plan

### Gate-check discrepancy (not a code change)

**1. The float-escape census command returns 0, not 1.**
- **Found during:** Task 2, gate 7
- **Issue:** `rg 'allow.*disallowed_types' src/` returns no lines. rustfmt split the single sanctioned `#[allow(` attribute in `src/model/loudness.rs:94-95` across lines, so a single-line pattern cannot match it. The file hasn't changed since 2d61359 (last touched in 5c544cf), so this problem existed before this task.
- **Verification used instead:** `rg -U -c 'allow\(\s*clippy::disallowed_types' src/` gives exactly `src/model/loudness.rs:1`. The census still holds.
- **Not fixed:** out of scope. The census pattern is also documented in `clippy.toml`, CLAUDE.md and the plan verify, and they should be updated to a multiline form in a follow-up.

Otherwise the plan was executed as written.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: src/obu/header.rs (`fn read_fields_after_obu_size`, `with_input_base(after_size_start)`)
- FOUND: tests/obu_header.rs (`an_extension_length_inside_the_input_but_past_obu_size_is_refused`)
- FOUND: commit 1b76415, commit 9c6ea4b
