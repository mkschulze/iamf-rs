---
phase: 02-parser-round-trip-and-fuzzing
plan: 04
subsystem: testing
tags: [iamf, proptest, round-trip, byte-fidelity, unknown-obu]

requires:
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 03
    provides: Transactional flat sequence parsing, faithful writing, and canonical grouping
provides:
  - Derived-equality model round trips over bounded canonical sequence shapes
  - Byte-identical re-emission for both crate writer paths
  - Exact unknown OBU and raw parameter-data position and offset evidence
  - Explicit foreign non-minimal ULEB128 width caveat
affects: [reference-corpus, fuzzing, parser-api]

tech-stack:
  added: []
  patterns: [bounded compositional proptest strategies, ordering-only model flattening, exact offset sentinels]

key-files:
  created: [tests/round_trip.rs, tests/support/sequence_cases.rs]
  modified: [src/sequence.rs]

key-decisions:
  - "ParsedSequence::from_parts mirrors canonical writer order without serializing and centralizes known payload trailing bytes into Obu::trailing for derived equality."
  - "Own-output byte identity is promised for both sequence writers; foreign non-minimal obu_size widths remain the explicit syntax-only caveat."
  - "Unknown fidelity is asserted by variant index, raw bytes, and the complete absolute boundary vector, not by presence alone."

patterns-established:
  - "Canonical property inputs keep collections at 0..=4 and byte payloads at 0..=64 while deliberately including every Phase 2 parameter-data context and reserved-field family."
  - "Wire-position sentinels assert both semantic list positions and absolute byte offsets before and after opaque data."

requirements-completed: [PARSE-03, PARSE-04, PARSE-05, PARSE-06]

duration: 13min
completed: 2026-09-09
---

# Phase 2 Plan 4: Round-Trip and Unknown-Data Fidelity Summary

**Bounded canonical properties now prove derived-equality model round trips and both own-byte directions, with exact opaque-data offsets and an explicit foreign-width boundary.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-09-09T01:53:40Z
- **Completed:** 2026-09-09T02:06:40Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added 256-case structural properties spanning empty, descriptors-only, delimiter and delimiter-free temporal units, multi-substream frames, redundant descriptors, non-zero reserved syntax, all four Parameter Data contexts, known trailing bytes, and unknown OBU placement.
- Proved byte-identical parse/re-emission for both `write_parsed_sequence` and streaming `write_sequence` output while documenting and testing legal foreign non-minimal `obu_size` canonicalization.
- Locked a rich unknown-data sentinel by derived equality, exact OBU indices, raw extension-definition and multi-subblock Parameter Data bytes, known trailing bytes, and every absolute OBU boundary.
- Added negative evidence that truncated framing and corrupt known syntax return errors rather than being recovered as unknown/raw data.

## TDD Execution

- **RED:** The canonical model property failed to compile because `ParsedSequence::from_parts` did not exist (`c806a04`).
- **GREEN — Task 1:** Added the ordering-only adapter and passed the 256-case derived-equality property (`54de1b1`).
- **Task 2:** Added both byte-direction properties, foreign-width regression, exact opaque-position sentinel, corruption negatives, and public fidelity rustdoc (`6e0e476`).
- **Verification fix:** Replaced test indexing and modulo with bounded access required by the repository's strict parser lint set (`c12e22b`).

## Task Commits

1. **RED: bounded model round-trip properties** - `c806a04` (test)
2. **Task 1: structural model round trips** - `54de1b1` (test)
3. **Task 2: byte and unknown-data fidelity** - `6e0e476` (test)
4. **Verification fix: strict round-trip test lints** - `c12e22b` (fix)

## Files Created/Modified

- `src/sequence.rs` - Ordering-only `ParsedSequence::from_parts` adapter and precise own-output/foreign-width API documentation.
- `tests/support/sequence_cases.rs` - Bounded canonical strategies covering Phase 2 model shapes, reserved syntax, and all Parameter Data contexts.
- `tests/round_trip.rs` - Structural and byte properties, foreign-width caveat, exact unknown/raw placement sentinel, and negative corruption vectors.

## Decisions Made

- `from_parts` validates that descriptor-owned Parameter Block context can be constructed, but emits no bytes; Codec Config and Audio Element ordering exactly mirrors the canonical descriptor writer.
- Known payload-level trailing fields are moved into the common `Obu::trailing` owner during flattening, matching `parse_sequence` and making unmodified derived `PartialEq` the round-trip oracle.
- Foreign fixed-width ULEB128 is accepted on input but not represented as model state; re-emission uses minimal width, while every payload and opaque semantic position remains exact.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Made property and sentinel tests satisfy strict parser lints**
- **Found during:** Final strict Clippy verification
- **Issue:** New tests used direct indexing and `%`, rejected by `clippy::indexing_slicing` and `clippy::arithmetic_side_effects` under `-D warnings`.
- **Fix:** Replaced indexing with `first`/`get`/`first_mut` and used checked remainder for generated insertion positions.
- **Files modified:** `tests/round_trip.rs`, `tests/support/sequence_cases.rs`
- **Verification:** The 256-case round-trip target and strict all-target Clippy both pass.
- **Committed in:** `c12e22b`

---

**Total deviations:** 1 auto-fixed (1 blocking issue)
**Impact on plan:** The fix only aligns new tests with existing hostile-input guardrails; behavior and scope are unchanged.

## Issues Encountered

- The pinned `cargo fmt` command formatted unrelated files despite explicit path arguments. Those automatic changes were immediately restored; only plan-owned files were retained.
- Proptest reports that source-parallel failure persistence cannot locate a crate root for this integration test; it does not affect case execution or results.
- `rust-toolchain.toml` remained unchanged; no restoration was necessary.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Round-trip invariants are executable and ready to back the reference-corpus semantic assertions in Plan 02-05 and structured fuzz targets in Plan 02-06.
- No blockers remain.

## Self-Check: PASSED

All three implementation/test files and all four plan commits were verified on disk. The 256-case target, full root suite, strict all-target Clippy, `git diff --check`, and unchanged toolchain check pass.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
