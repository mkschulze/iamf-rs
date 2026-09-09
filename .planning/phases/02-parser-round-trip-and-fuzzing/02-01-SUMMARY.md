---
phase: 02-parser-round-trip-and-fuzzing
plan: 01
subsystem: parser
tags: [iamf, parameter-block, registry, demixing, recon-gain]

requires:
  - phase: 01-conformant-lpcm-bitstream
    provides: OBU read/write pairs, ordered descriptor model, and temporal-unit writer
provides:
  - Context-rich ordered ParamDefinitionRegistry with first-wire duplicate binding
  - Explicit registry dependency for Parameter Block parsing
  - Mix Gain, Demixing, Recon Gain, and bounded Reserved parameter-data round trips
affects: [02-sequence-parser, round-trip-properties, reference-corpus, fuzzing]

tech-stack:
  added: []
  patterns: [explicit descriptor context, ordered first-match registry, syntax-preserving semantic validation]

key-files:
  created: [tests/sequence_parse.rs]
  modified: [src/obu/param_definition.rs, src/obu/parameter_block.rs, src/obu/mod.rs, src/sequence.rs, src/model/mod.rs, tests/temporal.rs]

key-decisions:
  - "Parameter Block readers peek the parameter ID through a cloned cursor and require an explicit ParamDefinitionRegistry."
  - "The registry mirrors emitted Audio Element order but never drives output order; duplicate lookup binds to the first wire definition."
  - "Reserved demixing modes and high Recon Gain flag bits remain round-trippable and are reported through validation."

patterns-established:
  - "Registry context: definition type and owner-specific layer gates travel together."
  - "Known parameter syntax fails structurally; only explicit length-bounded extension syntax maps to Raw."

requirements-completed: [PARSE-02]

duration: 16min
completed: 2026-09-09
---

# Phase 2 Plan 1: Contextual Parameter Data Summary

**An ordered definition registry now supplies explicit descriptor context for complete Mix Gain, Demixing, Recon Gain, and bounded extension Parameter Block round trips.**

## Performance

- **Duration:** 16 min
- **Started:** 2026-09-09T00:54:34Z
- **Completed:** 2026-09-09T01:10:16Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added an ordered `ParamDefinitionRegistry` that retains duplicates, extracts extension definition prefixes, and records Recon Gain presence per channel layer.
- Refactored Parameter Block parsing to require registry context and added adjacent read/write support for all four Phase 2 parameter-data contexts.
- Added exact hand-computed Demixing and `test_000059` Recon Gain vectors, located context failures, semantic findings, and a guard preventing corrupt known syntax from becoming raw data.

## TDD Execution

- **RED (Task 1):** Registry API tests failed because the registry and context types did not exist.
- **GREEN (Task 1):** Added the registry, reused it in `SequenceWriter`, and extended descriptor validation for duplicate nested parameter IDs.
- **RED (Task 2):** Exact Demixing/Recon Gain vectors failed because those models and the mandatory registry parser signature did not exist.
- **GREEN (Task 2):** Added contextual parsing/writing, layer-gated Recon Gain bytes, bounded Reserved data, and validation findings.
- **Regression RED/GREEN:** An unsorted Audio Element test exposed input-order rather than emitted-wire-order binding; stable ID observation fixed it.

## Task Commits

Each TDD phase was committed atomically:

1. **Task 1 RED: registry context tests** - `d19b89b` (test)
2. **Task 1 GREEN: parameter definition registry** - `3c6a79e` (feat)
3. **Task 2 RED: contextual parameter-data vectors** - `d502180` (test)
4. **Task 2 GREEN: contextual parameter data** - `32c8b77` (feat)

## Files Created/Modified

- `src/obu/param_definition.rs` - Ordered registry and descriptor-owned parameter-data contexts.
- `src/obu/parameter_block.rs` - Demixing and Recon Gain models, registry-based reader, contextual writer, and semantic findings.
- `src/obu/mod.rs` - Public exports for the new registry and parameter-data types.
- `src/sequence.rs` - `SequenceWriter` registry reuse without changing output ordering.
- `src/model/mod.rs` - Duplicate nested parameter-ID findings.
- `tests/sequence_parse.rs` - Registry order, duplicate, extension-prefix, and layer-context coverage.
- `tests/temporal.rs` - Four-context round trips, `test_000059` bytes, and located corruption/context failures.

## Decisions Made

- Audio Elements are observed in the same stable ascending-ID order emitted by `write_descriptors`; registry iteration itself is never used to emit descriptors.
- Recon Gain retains all flag bits, reads and writes gain bytes only for bits 0 through 11, and reports higher bits semantically.
- Demixing stores the exact 3-bit mode and 5-bit reserved field without computing decoder coefficients.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Aligned registry binding with emitted Audio Element order**
- **Found during:** Task 2 verification
- **Issue:** The initial registry observed the model's Audio Element vector directly, while the descriptor writer emits Audio Elements in stable ascending-ID order; duplicates could therefore bind to a different definition than the wire carries first.
- **Fix:** Added a failing regression and made registry construction observe a stable ascending-ID view without mutating the model or using the registry for output order.
- **Files modified:** `src/obu/param_definition.rs`, `tests/sequence_parse.rs`
- **Verification:** The focused regression failed before the fix and passes with the complete plan suite.
- **Committed in:** `32c8b77`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** The fix is required for the plan's first-wire binding guarantee and adds no scope beyond registry correctness.

## Issues Encountered

- The pinned toolchain bootstrap temporarily added `rust-analyzer` to `rust-toolchain.toml`; the file was restored to its exact pre-plan content and was not committed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 02-02 can build sequence-level dispatch on the explicit registry-backed Parameter Block reader.
- No blockers remain.

## Self-Check: PASSED

All seven implementation/test files and all four TDD commits were verified on disk.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
