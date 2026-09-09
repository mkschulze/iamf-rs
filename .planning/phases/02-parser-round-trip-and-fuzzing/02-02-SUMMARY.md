---
phase: 02-parser-round-trip-and-fuzzing
plan: 02
subsystem: parser
tags: [iamf, reserved-bits, round-trip, validation, deterministic-dump]

requires:
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 01
    provides: Context-rich parameter-definition registry and contextual parameter data
provides:
  - Width-accurate ownership and faithful writing for all nine known reserved bit groups
  - Permissive non-zero reserved-value parsing with separate semantic findings
  - Explicitly migrated encoder, fixture, profile, conformance, and registry constructors
  - Annotated dump visibility for every newly owned reserved value
affects: [02-sequence-parser, round-trip-properties, reference-corpus, fuzzing]

tech-stack:
  added: []
  patterns: [syntax-preserving reserved fields, semantic validation separate from parsing, explicit-zero constructors]

key-files:
  created: []
  modified: [src/obu/param_definition.rs, src/obu/audio_element.rs, src/obu/mix_presentation.rs, src/dump.rs, tests/descriptors.rs, tests/support/fixture.rs]

key-decisions:
  - "Each reserved group is stored at its nearest exact wire structure and emitted unchanged."
  - "Non-zero reserved syntax remains structurally permissive and is diagnosed only through validate()."
  - "Encoder and fixture constructors explicitly choose zero; fidelity vectors alone choose non-zero values."

patterns-established:
  - "Reserved ownership: one width-accurate u8 field per independently encoded reserved group."
  - "Dump transparency: preserved syntax fields appear in deterministic annotated output, including zero values."

requirements-completed: []

duration: 14min
completed: 2026-09-09
---

# Phase 2 Plan 2: Reserved Field Fidelity Summary

**All nine formerly discarded reserved bit groups now survive parse/write byte round trips, remain semantically diagnosable, and are visible in deterministic dumps.**

## Performance

- **Duration:** 14 min
- **Started:** 2026-09-09T01:15:27Z
- **Completed:** 2026-09-09T01:29:02Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments

- Added exact-width owners for the shared parameter-definition, Audio Element, two Demixing-default, scalable-layout, channel-layer, Output Gain, Rendering Config, and conditional-width layout reserved groups.
- Added hand-computed non-zero vectors proving readers remain permissive, writers reproduce the original bytes, and validation reports every group independently.
- Migrated all live struct literals and constructors to deliberate zero values without changing the committed IAMF golden bytes or SHA-256.
- Extended the annotated dump and its committed golden text to expose every owned reserved value.

## TDD Execution

- **RED (Task 1):** Exact Audio Element and Mix Presentation vectors failed to compile because none of the nine reserved owners existed.
- **GREEN (Task 1):** Added stored fields, faithful read/write paths, zero constructors, and validation findings; 49 descriptor and 34 temporal tests passed.
- **RED (Task 2):** The annotated-dump test failed on the first absent reserved label and printed no owned values.
- **GREEN (Task 2):** Migrated the complete literal census and dump paths; the full suite, golden reproduction, and strict Clippy passed.

## Task Commits

1. **Task 1 RED: reserved-field fidelity vectors** - `8764910` (test)
2. **Task 1 GREEN: preserve reserved fields** - `dc937a6` (feat)
3. **Task 2 RED: reserved dump visibility** - `862b442` (test)
4. **Task 2 GREEN: migrate literals and dump paths** - `415dc8b` (refactor)

## Files Created/Modified

- `src/obu/param_definition.rs` - Stores, writes, and validates the shared seven-bit reserved field.
- `src/obu/audio_element.rs` - Owns the Audio Element, Demixing-default, scalable-layout, layer, and Output Gain fields.
- `src/obu/mix_presentation.rs` - Owns Rendering Config and conditional two-/six-bit layout fields.
- `src/dump.rs` - Prints every owned reserved value in deterministic field output.
- `tests/descriptors.rs` - Hand-computed non-zero vectors, byte-fidelity assertions, and dump visibility coverage.
- `tests/temporal.rs` - Explicit zero for the mode-0 parameter-definition construction path.
- `tests/profile.rs`, `tests/conformance.rs`, `tests/sequence_parse.rs` - Explicitly migrated profile, conformance, and registry construction paths.
- `tests/support/fixture.rs`, `tests/support/test_000003.rs` - Explicit zero values in reusable encoder fixtures.
- `tests/fixtures/golden/phase1_sample_identity.dump.txt` - Reviewable reserved-field annotations with unchanged IAMF bytes.
- `tests/fixture.rs` - Aligns validation assertions with Plan 02-01's intentional shared parameter-ID findings.

## Decisions Made

- Used separate `u8` fields instead of combining reserved groups, because each field has an independent wire position and validation location.
- Kept write-time width enforcement in `BitWriter::write_unsigned`; model validation reports non-zero reserved syntax without structurally rejecting foreign input.
- Printed zero-valued reserved fields too, so the dump has a stable schema and future changes remain reviewable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated stale clean-validation fixture assertions from Plan 02-01**
- **Found during:** Task 1 focused verification and Task 2 full-suite verification
- **Issue:** Plan 02-01 intentionally added findings for duplicate nested parameter IDs, but older golden/fixture tests still required an empty finding list for fixtures that deliberately share parameter ID 100.
- **Fix:** Updated the assertions to require only the intentional first-wire-binding findings while continuing to reject every unexpected finding.
- **Files modified:** `tests/descriptors.rs`, `tests/fixture.rs`
- **Verification:** Full `cargo test --locked` and strict all-target Clippy pass.
- **Committed in:** `dc937a6`, `415dc8b`

---

**Total deviations:** 1 auto-fixed (1 blocking issue)
**Impact on plan:** The correction restored consistency with Plan 02-01 semantics and introduced no production behavior or scope beyond verification accuracy.

## Issues Encountered

- The pinned toolchain did not modify `rust-toolchain.toml`; no restoration was necessary.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Reserved-field normalization no longer blocks sequence byte-fidelity or PARSE-04 work.
- Plan 02-03 can build sequence parsing over syntax-preserving descriptor models.

## Self-Check: PASSED

All thirteen modified implementation/test artifacts and all four TDD commits were verified on disk. Full tests, golden reproduction, strict Clippy, both censuses, and `git diff --check` pass.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
