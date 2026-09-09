---
phase: 02-parser-round-trip-and-fuzzing
plan: 03
subsystem: parser
tags: [iamf, sequence-parser, wire-order, validation, temporal-grouping]

requires:
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 01
    provides: Explicit ordered parameter-definition registry and contextual Parameter Block parsing
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 02
    provides: Syntax-preserving reserved fields and faithful descriptor models
provides:
  - Transactional whole-input parsing into an owned flat wire-order sequence
  - Faithful flat sequence writing with explicit Parameter Block registry context
  - Two-pass flat validation with first-wire binding and canonical temporal grouping
affects: [round-trip-properties, reference-corpus, fuzzing, parser-api]

tech-stack:
  added: []
  patterns: [cloned-cursor type inspection, central bounded OBU dispatch, flat two-pass validation]

key-files:
  created: []
  modified: [src/sequence.rs, src/error.rs, tests/sequence_parse.rs]

key-decisions:
  - "Sequence dispatch inspects type and payload base through a cloned cursor, then parses the real cursor through the central bounded OBU reader."
  - "Parsed known trailing bytes are normalized into Obu<T>::trailing while unknown payloads remain one indivisible byte vector."
  - "Validation and grouping scan the flat vector directly; they never reconstruct or sort a DescriptorSet."

patterns-established:
  - "Payload errors are translated once from bounded-reader coordinates to absolute input offsets."
  - "Flat registries and lookup vectors retain redundant copies and bind duplicates to the first wire occurrence."

requirements-completed: [PARSE-01]

duration: 15min
completed: 2026-09-09
---

# Phase 2 Plan 3: Transactional Flat Sequence Parser Summary

**A transactional whole-input parser and faithful flat writer now preserve every known, redundant, and reserved OBU in wire order, with deterministic validation and canonical temporal grouping.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-09-09T01:33:08Z
- **Completed:** 2026-09-09T01:48:28Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `ParsedSequence`, `SequenceObu`, `UnknownObu`, `parse_sequence`, and `write_parsed_sequence` for empty through fully mixed known/unknown sequences without synthesis or reordering.
- Routed all known parsing through the central bounded OBU readers, translated payload-relative failures to absolute offsets once, and required the explicit first-wire registry for Parameter Blocks.
- Added direct flat validation and delimiter/transition-based temporal-unit ranges without constructing or sorting `DescriptorSet`.
- Covered every known dispatch variant, reserved types 24 through 30, redundant descriptors, raw unknown placement, missing context, truncated suffixes, validation order, and canonical grouping.

## TDD Execution

- **RED:** Public parser/writer, validation, and grouping vectors failed to compile because the flat sequence API did not exist (`904a367`).
- **GREEN — Task 1:** Added transactional parse/write dispatch, explicit registry threading, absolute payload error translation, and central sequence-level trailing ownership (`3ddc4d1`).
- **GREEN — Task 2:** Added local/duplicate/reference validation passes and canonical grouping over the exact flat OBU vector (`5e99944`).
- **Guardrail regression:** The full suite exposed the missing pinned-reference citation on the new writer; adding it restored the citation gate (`56cc13a`).

## Task Commits

1. **RED: flat sequence parser vectors** - `904a367` (test)
2. **Task 1: complete wire-order sequence parsing and writing** - `3ddc4d1` (feat)
3. **Task 2: flat validation and canonical grouping** - `5e99944` (feat)
4. **Verification fix: pinned writer citation** - `56cc13a` (fix)

## Files Created/Modified

- `src/sequence.rs` - Flat sequence model, transactional dispatcher, faithful writer, validation passes, and grouping ranges.
- `src/error.rs` - Single-use payload-relative to absolute input-offset translation helper.
- `tests/sequence_parse.rs` - End-to-end flat parsing, fidelity, error, validation-order, and grouping coverage.

## Decisions Made

- Type inspection uses a cloned cursor only; the real cursor always traverses the central `read_obu_with` or `read_obu_with_header` framing path.
- The parse/write registry grows in descriptor wire order and is never used to emit or reorder descriptors.
- Known reader-specific trailing fields are moved into the central `Obu<T>::trailing` owner at the sequence boundary; raw codec/config syntax and unknown OBU payloads remain indivisible where their syntax requires it.
- Temporal delimiters are authoritative when present; delimiter-free boundaries arise only from repeated frame substream IDs or a Parameter Block after a frame.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added the required reference citation to the flat writer**
- **Found during:** Final full-suite verification
- **Issue:** The repository citation guard rejected `write_parsed_sequence` because it had no adjacent pinned-reference `// ref:` line.
- **Fix:** Cited `iamf-tools@v2.1.0`'s `ObuSequencerBase::PickAndPlace`, the same sequence-shape reference used by the existing one-shot writer.
- **Files modified:** `src/sequence.rs`
- **Verification:** `cargo test --locked --test citations`, the full root suite, and strict all-target Clippy pass.
- **Committed in:** `56cc13a`

---

**Total deviations:** 1 auto-fixed (1 blocking issue)
**Impact on plan:** The fix satisfies an inherited source-provenance guardrail and changes no behavior or scope.

## Issues Encountered

- The repository-wide formatter check reports pre-existing formatting differences outside the plan-owned files under the pinned toolchain. Only the three owned files were formatted, and `git diff --check` passes.
- The generic state helper misread the customized STATE frontmatter and attempted to change milestone `v1` to `v1.1` and mark the project complete. Those changes were rejected; the original milestone and verified Phase 1 context were preserved while applying only Plan 02-03 progress.
- `rust-toolchain.toml` remained unchanged; no restoration was necessary.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The stable flat parser surface is ready for structural/model byte round-trip properties in Plan 02-04.
- No blockers remain.

## Self-Check: PASSED

All three implementation/test files, this summary, and all four plan commits were verified on disk.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
