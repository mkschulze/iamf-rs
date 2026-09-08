---
phase: 01-conformant-lpcm-bitstream
plan: 09
subsystem: planning-contract
tags: [iamf, conformance, requirements, traceability, reference-pins]

requires:
  - phase: 01-08
    provides: executed three-fixture conformance gate, exact reference results and approved D-01/corpus corrections
provides:
  - "Authoritative Phase 1 success criteria aligned with the executed sample-identity and structure-fixture split"
  - "Exact iamf-tools@v2.1.0 release and SHA identity for the IAMF-v1.1.0 strict-parser oracle"
  - "BITS-01, BITS-07 and CONF-11 clauses aligned with the evidence-backed implementation"
  - "An exact 63-ID proof preserving checked definitions and Complete traceability"
affects: [phase-01-verification, phase-02-parser, conformance-gate]

tech-stack:
  added: []
  patterns:
    - "Planning contracts follow accepted executable evidence without weakening observable gates"
    - "Reference tool versions identify releases; SPEC_VERSION identifies the wire-format version"

key-files:
  created:
    - .planning/phases/01-conformant-lpcm-bitstream/01-09-SUMMARY.md
  modified:
    - .planning/ROADMAP.md
    - .planning/REQUIREMENTS.md

key-decisions:
  - "Sample identity remains on the single-element 5.1 fixture; repeated-descriptor ordering is proved separately by the two-Audio-Element structure fixture"
  - "iamf-tools@v2.1.0 at 848c6ff4968ff8cc6f728259892ab4f90cb83256 is the pinned tool release implementing the four IAMF-v1.1.0 discriminators; SPEC_VERSION remains 1.1.0"
  - "Shipping bit I/O is hand-rolled and bitstream-io remains a dev-only differential oracle"
  - "The offline corpus is selected from committed libiamf fixtures through pinned git show; Bazel generates only the matching CONF-07 custom fixture"

patterns-established:
  - "Contract reconciliation: retain requirement IDs, completion state and oracle strength while replacing only disproven implementation premises"

requirements-completed:
  - BITS-01
  - BITS-07
  - CONF-02
  - CONF-03
  - CONF-04
  - CONF-05
  - CONF-06
  - CONF-07
  - CONF-11
  - DEC-04

metrics:
  duration: 4min
  completed: 2026-09-08

status: complete
---

# Phase 1 Plan 09: Reconcile the Phase 1 Contract Summary

The authoritative roadmap and requirement ledger now describe the already-proven
Phase 1 implementation exactly, without weakening sample identity, strict parsing,
byte identity, descriptor ordering, fixture provenance or reference pinning.

## Performance

- **Duration:** 4 min
- **Started:** 2026-09-08T22:10:40Z
- **Completed:** 2026-09-08T22:14:40Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Split ROADMAP criterion 2 cleanly between the non-silent, per-channel-distinguishable,
  positively trimmed 5.1 sample-identity fixture and the separate structure fixture
  with at least six OBUs and two Audio Elements.
- Replaced the nonexistent `iamf-tools` v1.x premise with the exact
  `iamf-tools@v2.1.0` pin while retaining IAMF `SPEC_VERSION = "1.1.0"`, strict-parser
  acceptance and the exact-or-exhaustively-ledgered byte-diff condition.
- Amended BITS-01/BITS-07 to specify the hand-rolled `BitCursor`/`BitWriter`, native
  located errors, shipping boundary and dev-only randomized `bitstream-io` oracle.
- Amended CONF-11 to specify the pinned `libiamf@v1.1.0` source corpus, D-14 cap,
  `git show` import and manifest, while keeping Bazel confined to CONF-07 generation.
- Proved all 63 Phase 1 IDs remain exact, unique, checked and uniquely traceable as
  `Complete`.

## Reconciled Clauses and Evidence

| Clause | Reconciled contract | Evidence source |
|---|---|---|
| ROADMAP criterion 2 | Sample identity and descriptor-order observability use separate fixtures | `tests/support/fixture.rs`, `tests/conformance.rs`, `CONFORMANCE-GATE.md` Experiments 2/3 |
| ROADMAP criterion 3 | The strict oracle is `iamf-tools@v2.1.0` at `848c6ff4968ff8cc6f728259892ab4f90cb83256` | `REFERENCES.md` version-number caveat and four discriminators; `CONFORMANCE-GATE.md` exit verdict |
| BITS-01 | Hand-rolled cursor/writer own byte offsets and native located failures | `src/bits/mod.rs`, D-01 in `01-CONTEXT.md`, `tests/bits_oracle.rs` |
| BITS-07 | Shipping bit primitives stay in `src/bits`; `bitstream-io` is dev-only | `Cargo.toml`, `tests/bits_oracle.rs`, linked dependency graph |
| CONF-11 | The capped offline corpus comes from pinned committed `libiamf` fixtures | `tests/fixtures/MANIFEST.md`, `01-02-SUMMARY.md`, reference-manifest tests |

## Focused Verification

- Exact Ruby contract check: **63 exact IDs**, one checked definition per ID, one
  unique `Complete` traceability row per ID.
- `cargo test --locked --test bits_oracle --test refcorpus --test reference_manifest
  --test golden --test fixture`: **49 passed, 0 failed** across five non-empty targets.
- `cargo tree -e normal,no-proc-macro`: linked graph is exactly `iamf` plus `thiserror`.
- `git diff --check -- .planning/ROADMAP.md .planning/REQUIREMENTS.md`: passed.
- Direct task diffs contain only the five intended clause descriptions; no IDs,
  statuses, mappings or historical plan records changed.

## Task Commits

1. **Task 1: Reconcile ROADMAP criteria 2 and 3** — `0148f14`
2. **Task 2: Amend BITS-01, BITS-07 and CONF-11** — `4f2a0bf`
3. **Task 3: Prove contract consistency** — verification-only; no file change or task commit

## Deviations from Plan

### Verification-tool workaround

**1. [Rule 3 - Blocking] Classified non-path references explicitly**

- **Found during:** Task 3
- **Issue:** `gsd-tools verify references` reported the literal verification command
  `git diff --check -- .planning/ROADMAP.md .planning/REQUIREMENTS.md` as a missing
  path and also reported this summary before it had been created.
- **Resolution:** Verified every real evidence path directly, ran the purported
  command itself successfully, and created the required summary. No product or
  planning contract was changed to accommodate the scanner limitation.
- **Files modified:** None beyond the planned summary.

**Total deviations:** 1 verification-tool workaround. **Impact:** none on the
contract or its evidence.

## Issues Encountered

None affecting the reconciled contract or its executable evidence.

## Next Phase Readiness

Plan 01-10 can now obtain exact-candidate cross-target CI evidence against a
contract whose stale prose gaps have been removed.

## Self-Check: PASSED

- `.planning/phases/01-conformant-lpcm-bitstream/01-09-SUMMARY.md` exists.
- Task commits `0148f14` and `4f2a0bf` exist in repository history.
- The 63-ID traceability proof and all 49 focused tests passed.
