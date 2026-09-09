---
phase: 02-parser-round-trip-and-fuzzing
plan: 07
subsystem: testing
tags: [cargo-fuzz, corpus, ci, cross-platform, regression]

requires:
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 06
    provides: Independent two-target fuzz workspace and shared bounded model generator
provides:
  - Byte-identical iamf-tools parser seeds with recorded provenance
  - Canonical model seeds covering every required Phase 2 shape
  - Stable exhaustive corpus replay in the four-target CI matrix
  - Separate scheduled bounded nightly fuzz discovery with artifact upload
affects: [parser-hardening, regression-testing, cross-platform-ci, FUZZ-04, FUZZ-05]

tech-stack:
  added: []
  patterns: [committed named corpus seeds, exhaustive stable replay, bounded scheduled fuzzing]

key-files:
  created: [fuzz/CORPUS.md, fuzz/corpus/parse_sequence/, fuzz/corpus/obu_roundtrip/, .github/workflows/fuzz.yml]
  modified: [tests/fuzz_regression.rs, .github/workflows/ci.yml]

key-decisions:
  - "Stable replay treats every corpus and artifact file as mandatory input: unreadable, empty, or skipped entries fail the test."
  - "Coverage discovery runs only on scheduled or manually dispatched Linux nightly jobs; pull-request CI remains stable-toolchain replay."

patterns-established:
  - "Real parser seeds are copied byte-for-byte and checked against their source fixture SHA-256 values."
  - "Every minimized failure is retained as corpus plus a focused named regression when its invariant is understood."

requirements-completed: [FUZZ-04, FUZZ-05]

duration: 12min
completed: 2026-09-09
---

# Phase 2 Plan 7: Permanent Fuzz Corpus and CI Summary

**Pinned real-world and structural seeds now replay exhaustively on stable Rust across all four target paths, while separate bounded nightly jobs continue coverage discovery.**

## Performance

- **Duration:** 12 min
- **Started:** 2026-09-09T03:23:44Z
- **Completed:** 2026-09-09T03:35:32Z
- **Tasks:** 2
- **Files modified:** 19

## Accomplishments

- Copied four pinned positive `iamf-tools` fixtures into the parser corpus and recorded source digests and provenance.
- Added eleven deterministic model seeds spanning empty and descriptor-only streams, delimiter variants, all Parameter Data contexts, redundant and reserved/trailing syntax, bounded raw Parameter Data, and unknown OBU placement.
- Made stable replay recursively enumerate both corpora and retained artifacts, reject empty or unreadable inputs, and run the exact production parser or shared structural round-trip oracle.
- Added the replay test to all four stable target paths and isolated two five-minute coverage fuzzers in a schedule/manual-only pinned-nightly workflow with failure artifacts.
- Completed fresh 10,000-run `parse_sequence` and `obu_roundtrip` fuzz sessions without a crash.

## TDD Execution

- **RED:** Exhaustive replay failed because the two permanent corpora and their required shape coverage did not yet exist (`0159143`).
- **GREEN:** Added the copied parser fixtures, deterministic model seeds, provenance map, and exhaustive replay (`0b62067`).
- **CI:** Wired stable four-target replay, dependency isolation checks, bounded nightly discovery, and artifact upload (`a6ccbdd`).
- **Hardening:** Scoped the existing deliberate test-harness panic diagnostics so the repository's explicit `-D clippy::panic` gate passes (`d719e82`).

## Task Commits

1. **RED: exhaustive corpus replay contract** - `0159143` (test)
2. **Task 1: seed permanent corpora** - `0b62067` (test)
3. **Task 2: replay corpus across four targets** - `a6ccbdd` (ci)
4. **Verification fix: strict replay-test lint scope** - `d719e82` (fix)

## Files Created/Modified

- `fuzz/CORPUS.md` - Seed provenance, required shape map, minimization and retention policy.
- `fuzz/corpus/parse_sequence/` - Four byte-identical positive `iamf-tools` fixtures.
- `fuzz/corpus/obu_roundtrip/` - Eleven named deterministic structural seeds.
- `tests/fuzz_regression.rs` - Exhaustive stable corpus/artifact enumeration and both production oracles.
- `.github/workflows/ci.yml` - Four-target stable replay and root/fuzz dependency-policy checks.
- `.github/workflows/fuzz.yml` - Scheduled/manual pinned-nightly bounded fuzzing and failure artifact upload.

## Decisions Made

- The committed corpus is deliberately small, named, and reviewable; coverage-generated transient expansions are not committed unless minimized and retained under the documented policy.
- Nightly/sanitizer coverage discovery does not run in the pull-request matrix. Stable replay is the permanent, deterministic cross-platform gate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Strict Clippy rejected deliberate test-harness panic diagnostics**
- **Found during:** Final verification
- **Issue:** `tests/fuzz_regression.rs` permitted `expect_used` but not the deliberate path-rich `panic!` diagnostics used by the exhaustive test harness, so explicit `-D clippy::panic` failed.
- **Fix:** Added the same test-file-scoped `clippy::panic` allowance already used by the reference corpus harness.
- **Files modified:** `tests/fuzz_regression.rs`
- **Verification:** Strict all-target/all-feature Clippy passes.
- **Committed in:** `d719e82`

---

**Total deviations:** 1 auto-fixed (1 verification lint mismatch)
**Impact on plan:** No production lint was relaxed; the allowance is scoped to the test harness and preserves descriptive corpus-path failures.

## Issues Encountered

- The executor completion handler was interrupted after all implementation commits existed. The plan artifacts were inspected directly, the full verification contract was rerun, and this summary and tracking update were completed from fresh evidence.
- The bounded fuzz runs generated transient corpus expansions locally. All 314 untracked hash-named discoveries were removed after the successful run, leaving only the curated committed seeds.

## User Setup Required

None.

## Next Phase Readiness

- All seven Phase 2 plans are implemented and ready for goal-level verification.
- FUZZ-04 and FUZZ-05 are complete; no implementation blocker remains.

## Self-Check: PASSED

All named seeds, source fixtures, workflows, tests, and task commits exist. Stable replay, the full locked suite, strict all-feature Clippy, both cargo-deny policies, workspace lock isolation, and both 10,000-run fuzz commands pass.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
