---
phase: 02-parser-round-trip-and-fuzzing
plan: 05
subsystem: testing
tags: [iamf, reference-corpus, parser, parameter-block, provenance]

requires:
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 03
    provides: Transactional flat sequence parser and validation interface
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 04
    provides: Flat byte-preserving writer and exact opaque-data positioning
provides:
  - Evidence-backed bijection over all 39 vendored IAMF files
  - Complete ordered modeled-field projections for 37 positive fixtures
  - Exact structural and semantic dispositions for two negative fixtures
  - Raw ungoverned Parameter Block preservation with validation diagnostics
affects: [fuzzing, parser-api, reference-corpus, PARSE-07]

tech-stack:
  added: []
  patterns: [independent fixture ledgers, bounded raw sequence variants, exact negative dispositions]

key-files:
  created: [.planning/phases/02-parser-round-trip-and-fuzzing/02-05-SUMMARY.md]
  modified: [src/sequence.rs, tests/parse_reference.rs, tests/support/reference_expectations.rs, tests/sequence_parse.rs, tests/round_trip.rs, tests/fixtures/MANIFEST.md, tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md, .planning/phases/02-parser-round-trip-and-fuzzing/02-CONTEXT.md, .planning/phases/02-parser-round-trip-and-fuzzing/02-05-PLAN.md, .planning/ROADMAP.md]

key-decisions:
  - "A bounded Parameter Block with no registry definition is preserved as an explicit raw ungoverned sequence variant; governed malformed syntax remains a structural error."
  - "The pinned corpus is classified as 37 positive and two negative fixtures: test_000129 is structurally truncated at offset 53, while the zero-frame fixture is a semantic validation negative."
  - "PARSE-07 applies parse success to valid iamf-tools fixtures without weakening exact dispositions for fixtures explicitly marked invalid upstream."

patterns-established:
  - "Corpus expectations are a filesystem bijection, with positives and named structural/semantic negative dispositions stored in separate arrays."
  - "Non-LPCM codec configs remain Raw and are asserted by FourCC, byte position, length, and SHA-256."

requirements-completed: [PARSE-07]

duration: 19min
completed: 2026-09-09
---

# Phase 2 Plan 5: Reference Corpus Semantics Summary

**All 39 vendored IAMF files now have evidence-backed dispositions: 37 positives with complete ordered modeled-field and byte-ownership assertions, plus exact structural and semantic negatives.**

## Performance

- **Duration:** 19 min
- **Started:** 2026-09-09T02:32:12Z
- **Completed:** 2026-09-09T02:50:41Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- Hand-decoded all four unpaired iamf-tools binaries against pinned blobs, recording complete OBU boundaries, semantic fields, raw codec offsets/digests, temporal grouping, and findings.
- Inventoried every vendored `.iamf` exactly once as 37 positive fixtures, one structural negative (`test_000129`), and one semantic negative (zero frame size).
- Added an explicit `UngovernedParameterBlock` sequence variant so valid `test_000015` preserves its bounded type-3 header, raw payload, and exact position; flat validation reports the missing definition and the flat writer reproduces the file byte-for-byte.
- Proved governed malformed Parameter Block syntax never falls back to raw and still returns its exact structural error.
- Asserted complete ordered model projections and exact byte ownership for all positives, explicit raw codec FourCC/position/length/SHA-256, and all 26 `test_000059` Recon Gain blocks including both layers, flag `0x1d`, and every gain byte.
- Corrected context, plan, manifest, provenance, and roadmap wording to the pinned evidence without weakening PARSE-07.

## TDD Execution

- **Task 1:** Committed the independent four-file hand-decode before Rust expectation constants (`08482dd`).
- **Task 2:** Committed the initial filesystem expectation inventory (`7efea81`), then corrected its classification under the checkpoint ruling in Task 3.
- **RED:** Focused sequence and round-trip tests failed because the ungoverned variant did not exist (`448fee3`).
- **GREEN:** Added bounded raw preservation, validation, and flat writing while leaving `read_parameter_block(&ParamDefinitionRegistry)` and governed failure behavior intact (`f7f6f6c`).
- **Task 3:** Added the 37+2 ledger, complete modeled-field projections, raw-codec boundaries, exact negative dispositions, and exhaustive Recon Gain assertions (`975f153`).
- **Verification fix:** Scoped test-harness panic allowances and replaced direct gain indexing for strict Clippy (`59e2133`).

## Task Commits

1. **Task 1: hand-decode unpaired references** - `08482dd` (docs)
2. **Task 2: inventory reference expectations** - `7efea81` (test)
3. **RED: define ungoverned Parameter Block contract** - `448fee3` (test)
4. **GREEN: preserve ungoverned Parameter Blocks** - `f7f6f6c` (feat)
5. **Task 3: assert reference corpus semantics** - `975f153` (test)
6. **Formatting: canonicalize grouping arm** - `78c3d67` (style)
7. **Evidence ruling documentation** - `627a200` (docs)
8. **Verification fix: strict corpus test lints** - `59e2133` (fix)

## Files Created/Modified

- `src/sequence.rs` - Explicit raw ungoverned Parameter Block variant, validation finding, temporal grouping, parser dispatch, and faithful flat writing.
- `tests/sequence_parse.rs` - Focused raw-preservation/validation regression and governed-corruption structural regression.
- `tests/round_trip.rs` - Pinned `test_000015` position and byte-identity regression.
- `tests/support/reference_expectations.rs` - 37 positive semantic ledgers, two negative dispositions, and raw-codec boundary/digest expectations.
- `tests/parse_reference.rs` - Corpus bijection, complete ordered semantic projections, byte ownership, exact negatives, raw codecs, and `test_000059` assertions.
- `tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md` - Pinned byte-level truth for the four unpaired files and `test_000129` classification evidence.
- `tests/fixtures/MANIFEST.md` - Evidence-backed structural-negative description for `test_000129`.
- `.planning/phases/02-parser-round-trip-and-fuzzing/02-CONTEXT.md` - Revised missing-context decision, explicitly excluding governed corruption fallback.
- `.planning/phases/02-parser-round-trip-and-fuzzing/02-05-PLAN.md` - Corrected 37+2 counts and two-negative verification contract.
- `.planning/ROADMAP.md` - Clarified criterion 2 to valid generated fixtures while retaining exact invalid dispositions.

## Decisions Made

- Missing descriptor context and corrupt governed syntax are different parser states. Only the former can be preserved opaquely because the common OBU framing already supplies a safe byte boundary.
- `test_000129` follows its paired pinned metadata (`is_valid: false`, `invalidates_bitstream: true`), not permissive parse-success assumptions; its exact error is `UnexpectedEndOfInput` at input offset 53.
- The semantic-zero-frame fixture remains separately valuable because it parses completely before `validate()` reports `num_samples_per_frame is 0, outside 1..=96000`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Corrected the original 38-positive/one-negative corpus classification**
- **Found during:** Task 2 decision checkpoint
- **Issue:** The original plan counted `test_000129` as positive even though its paired pinned textproto explicitly marks it invalid and injects structurally truncated syntax.
- **Fix:** Reclassified it as a structural negative at offset 53 and corrected all plan, context, roadmap, manifest, and provenance wording to 37+2.
- **Files modified:** `tests/support/reference_expectations.rs`, `tests/parse_reference.rs`, `tests/fixtures/MANIFEST.md`, `tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md`, `.planning/phases/02-parser-round-trip-and-fuzzing/02-05-PLAN.md`, `.planning/ROADMAP.md`
- **Verification:** Corpus bijection, exact negative test, manifest/cap/pin suites, and full root suite pass.
- **Committed in:** `975f153`, `627a200`

**2. [Rule 3 - Blocking] Satisfied strict corpus-test Clippy lints**
- **Found during:** Final strict all-target Clippy verification
- **Issue:** The new integration test used panic-style assertions without the repository's normal test-only allowances and directly indexed the fixed Recon Gain array.
- **Fix:** Scoped the allowances to the integration test and changed the gain setup to checked indexing.
- **Files modified:** `tests/parse_reference.rs`
- **Verification:** `cargo clippy --locked --all-targets --all-features -- -D warnings` passes.
- **Committed in:** `59e2133`

---

**Total deviations:** 2 auto-fixed (1 corpus classification bug, 1 blocking lint issue)
**Impact on plan:** The evidence correction strengthens the plan's validity contract; no positive fixture was excluded, and no malformed governed syntax was made permissive.

## Issues Encountered

- The first strict Clippy run exposed test-harness lint violations; the narrow fix above passed on the next run.
- `rust-toolchain.toml` remained unchanged and the active toolchain stayed pinned to Rust 1.85.0.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The complete positive corpus can seed Phase 02 fuzzing while both negative dispositions remain ordinary stable regressions.
- The raw ungoverned Parameter Block variant is covered by focused parser and writer tests and is ready for hostile-byte fuzzing.
- No blockers remain for Plan 02-06.

## Self-Check: PASSED

All key implementation, test, provenance, and summary files exist; all eight
plan commits resolve. The full locked root suite, strict all-target Clippy,
37+2 corpus suite, fixture cap/manifest/pin gates, provenance gate,
`git diff --check`, and unchanged Rust 1.85.0 toolchain check pass.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
