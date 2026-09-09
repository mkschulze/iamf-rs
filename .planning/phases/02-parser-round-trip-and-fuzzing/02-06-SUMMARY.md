---
phase: 02-parser-round-trip-and-fuzzing
plan: 06
subsystem: testing
tags: [cargo-fuzz, libfuzzer, arbitrary, parser, round-trip]

requires:
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 03
    provides: Transactional flat sequence parser and faithful writer
  - phase: 02-parser-round-trip-and-fuzzing
    plan: 05
    provides: Evidence-backed ungoverned Parameter Block behavior
provides:
  - Independent excluded cargo-fuzz workspace with scoped NCSA policy
  - Hostile-byte parse_sequence and structure-aware obu_roundtrip targets
  - Shared bounded canonical model generator with stable smoke replay
affects: [fuzz-corpus, nightly-fuzzing, parser-hardening, FUZZ-04, FUZZ-05]

tech-stack:
  added: [arbitrary 1.4.2, libfuzzer-sys 0.4.13, cargo-fuzz 0.13.2]
  patterns: [excluded fuzz workspace, feature-gated test API, bounded canonical generator]

key-files:
  created: [src/fuzzing.rs, fuzz/Cargo.toml, fuzz/Cargo.lock, fuzz/deny.toml, fuzz/fuzz_targets/parse_sequence.rs, fuzz/fuzz_targets/obu_roundtrip.rs, tests/fuzz_regression.rs]
  modified: [Cargo.toml, Cargo.lock, .gitignore, src/lib.rs]

key-decisions:
  - "The root fuzzing feature owns the optional arbitrary dependency; the excluded fuzz package enables it only through roundtrip-model."
  - "One bounded generator supplies both libFuzzer's structural oracle and the stable regression, with at most three temporal units and 32-byte generated frame payloads."

patterns-established:
  - "Fuzz-only licences and dependencies are governed by fuzz/deny.toml and fuzz/Cargo.lock, never the root policy or lock."
  - "Structural fuzz models are writer-valid by construction, then checked with derived model equality and exact rewritten-byte equality."

requirements-completed: [FUZZ-01, FUZZ-02, FUZZ-03]

duration: 16min
completed: 2026-09-09
---

# Phase 2 Plan 6: Isolated Parser Fuzz Harness Summary

**An excluded two-target cargo-fuzz workspace now exercises arbitrary parser bytes and bounded canonical sequence round trips without contaminating the production dependency or licence graph.**

## Performance

- **Duration:** 16 min
- **Started:** 2026-09-09T02:57:24Z
- **Completed:** 2026-09-09T03:13:38Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments

- Created an independent `fuzz/` workspace with its own lockfile and NCSA allowance while the root lock and default linked graph remain libFuzzer-free.
- Added exactly two targets: hostile-byte `parse_sequence` and feature-gated structural `obu_roundtrip` with derived model and byte equality oracles.
- Added one shared generator covering empty, descriptors-only, delimiter variants, all Parameter Data contexts, redundant descriptors, reserved/trailing values, and unknown placements under small allocation caps.
- Proved the real targets build under pinned nightly `2026-09-01`, while stable regression, strict Clippy, the full root suite, and both cargo-deny policies pass.

## TDD Execution

- **RED:** The feature-gated stable smoke test failed because `iamf::fuzzing` and `sequence_from_fuzz_bytes` did not exist (`c1e1498`).
- **GREEN:** Added the shared bounded generator and both fuzz targets; the smoke test then passed across all eight shape selectors (`d0b45aa`).

## Task Commits

1. **Task 1: isolate fuzz workspace and dependency policy** - `0e16858` (build)
2. **RED: bounded shared-generator regression** - `c1e1498` (test)
3. **Task 2: shared generator and exact two targets** - `d0b45aa` (feat)

## Files Created/Modified

- `Cargo.toml`, `Cargo.lock` - Explicit root workspace exclusion, optional feature-gated `arbitrary`, and stable test contract without `libfuzzer-sys`.
- `.gitignore` - Ignores only transient fuzz build output, leaving lockfiles, corpora, and minimized regressions trackable.
- `src/lib.rs`, `src/fuzzing.rs` - Doc-hidden feature-gated shared bounded canonical generator.
- `fuzz/Cargo.toml`, `fuzz/Cargo.lock`, `fuzz/deny.toml` - Independent exact two-bin package, resolved graph, and scoped NCSA policy.
- `fuzz/fuzz_targets/parse_sequence.rs` - Production parser invocation for every hostile input slice.
- `fuzz/fuzz_targets/obu_roundtrip.rs` - Writer-valid structural generation, parse equality, and rewritten-byte equality.
- `tests/fuzz_regression.rs` - Stable deterministic generator and round-trip smoke coverage.

## Decisions Made

- `parse_sequence` keeps root default features disabled; only `obu_roundtrip` requires the fuzzing model API through the fuzz package's `roundtrip-model` feature.
- The generator uses a fixed rich descriptor/model skeleton plus bounded byte-derived shape, unit, payload, gain, and insertion choices. This reaches every required canonical class without accepting arbitrary invalid structs or relaxing production bounds.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Completed fuzz-package licence and dependency metadata**
- **Found during:** Task 2 final cargo-deny verification
- **Issue:** The independent package initially omitted its own licence expression, and its path dependency had an implicit wildcard version; the copied deny policy correctly rejected both.
- **Fix:** Added `license = "MIT OR Apache-2.0"` and pinned the path dependency to root version `=0.1.0`.
- **Files modified:** `fuzz/Cargo.toml`
- **Verification:** `cargo deny --manifest-path fuzz/Cargo.toml --all-features check` passes all four checks.
- **Committed in:** `d0b45aa`

---

**Total deviations:** 1 auto-fixed (1 blocking manifest-policy issue)
**Impact on plan:** The fix tightened the independent workspace's reproducibility and licensing without changing target scope or the root graph.

## Issues Encountered

- The repository's pinned Rust 1.85 compiler was too old to compile cargo-fuzz 0.13.2's current locked tool dependencies. Installing the tool with the installed stable Rust 1.92 compiler succeeded; target builds still use the plan-pinned nightly `2026-09-01`.
- The pinned nightly was absent and was safely installed through rustup before running the required real builds.
- Formatting `src/lib.rs` traversed existing modules and created unrelated formatting drift; those five unowned changes were restored before final verification and commits.

## User Setup Required

None - the required cargo-fuzz tool and pinned nightly are installed locally.

## Next Phase Readiness

- Plan 02-07 can commit real iamf-tools corpus seeds and stable cross-target replay around these exact two targets.
- FUZZ-04 and FUZZ-05 remain intentionally pending for the corpus/CI plan.
- No blockers remain.

## Self-Check: PASSED

All implementation, manifest, lock, target, regression, and summary files exist; all three task commits resolve. The final pinned-nightly fuzz builds, full locked root suite, strict all-feature Clippy, both cargo-deny policies, default linked graph, lock isolation, exact bin count, diff check, and unchanged Rust toolchain pin pass.

---
*Phase: 02-parser-round-trip-and-fuzzing*
*Completed: 2026-09-09*
