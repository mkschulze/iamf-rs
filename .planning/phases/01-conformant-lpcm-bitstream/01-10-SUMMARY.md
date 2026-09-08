---
phase: 01-conformant-lpcm-bitstream
plan: 10
subsystem: conformance-ci
tags: [github-actions, byte-identity, cross-target, rosetta, golden-fixture]

requires:
  - phase: 01-09
    provides: reconciled Phase 1 ROADMAP and REQUIREMENTS contract
provides:
  - "Exact-candidate four-target CI proof for the committed golden fixture and same-process double encode"
  - "Windows LF-preserving checkout and executed x86_64 macOS tests under Rosetta 2"
  - "Durable run/job evidence plus a fresh 5/5 Phase 1 verification verdict"
affects: [phase-02-parser, release-gates, cross-platform-ci]

tech-stack:
  added: []
  patterns:
    - "Cross-target claims bind one terminal run to one exact candidate SHA"
    - "Retired native x86_64 macOS runners are replaced by executed Rosetta binaries, never compile-only evidence"
    - "Post-CI closure commits are constrained by an explicit documentation/state path allowlist"

key-files:
  created:
    - .planning/phases/01-conformant-lpcm-bitstream/01-10-SUMMARY.md
  modified:
    - .github/workflows/ci.yml
    - CONFORMANCE-GATE.md
    - .planning/phases/01-conformant-lpcm-bitstream/01-VERIFICATION.md

key-decisions:
  - "The terminal branch is normal-four-target: one exact-candidate run supplied four successful named golden steps, so no D-17 runner waiver is active"
  - "Windows preserves committed LF bytes before checkout, and x86_64-apple-darwin test binaries execute under Rosetta 2 on an arm64 runner"
  - "CI evidence remains bound to candidate 5fea02a00d23665353fed8e317d7cb7ab8325694; later closure commits may change only the plan's explicit documentation/state allowlist"

patterns-established:
  - "Durable CI evidence records candidate SHA, run, job, route, job conclusion, and named-step status/conclusion"

requirements-completed:
  - CONF-10
  - GUARD-09
  - GUARD-10
  - GUARD-12

metrics:
  duration: 34min
  completed: 2026-09-08

status: complete
---

# Phase 1 Plan 10: Close the Four-Target Byte-Identity Gate Summary

Exact candidate `5fea02a00d23665353fed8e317d7cb7ab8325694` passed one
complete GitHub Actions matrix whose four target jobs each executed and passed the
named golden-fixture and same-process double-encode tests.

## Performance

- **Duration:** 34 min
- **Started:** 2026-09-08T22:19:20Z
- **Completed:** 2026-09-08T22:53:08Z
- **Tasks:** 4
- **Files modified:** 3 plus this summary and final planning state

## Accomplishments

- Froze a locally green candidate, verified the worktree and publication ancestry,
  and crossed both remote-mutation boundaries only after explicit user authorization.
- Diagnosed the first exact-candidate run without borrowing stale jobs: Windows
  converted the committed golden dump to CRLF, while GitHub's native `macos-13`
  x86_64 runner was retired.
- Added workflow-only remediation. Windows disables `core.autocrlf` before checkout;
  macOS x86_64 builds and actually executes its target binaries under Rosetta 2.
- Obtained a fresh exact-SHA run with four successful named golden steps and a
  successful licence/guardrail job.
- Recorded durable evidence in `CONFORMANCE-GATE.md` and regenerated the Phase 1
  goal-backward report at `passed`, 5/5 truths and 63/63 requirements.

## Exact-Candidate CI Evidence

**Outcome:** `normal-four-target`

- **CI-tested candidate:** `5fea02a00d23665353fed8e317d7cb7ab8325694`
- **Run:** [34287100850](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850)
- **Run result:** `completed` / `success`
- **Completed:** 2026-09-08T22:42:13Z
- **Golden SHA-256:** `3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c`

| target | job | execution route | named golden step |
|---|---:|---|---|
| aarch64-apple-darwin | [102265091867](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265091867) | native arm64 macOS | completed / success |
| x86_64-apple-darwin | [102265092177](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092177) | x86_64 binaries under Rosetta 2 | completed / success |
| x86_64-pc-windows-msvc | [102265092145](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092145) | native Windows with LF-preserving checkout | completed / success |
| x86_64-unknown-linux-gnu | [102265092207](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092207) | native Linux x86_64 | completed / success |

The predecessor run
[34286069472](https://github.com/mkschulze/iamf-rs/actions/runs/34286069472)
belongs to candidate `61ba098413b48751838035c41a068b270a7a1ba0`. It supplied
diagnosis only. No successful job from it was combined with the final evidence.

## Closure Ancestry

- **CI-tested candidate:** `5fea02a00d23665353fed8e317d7cb7ab8325694`
- **Documentation evidence commit:** `4d81cd0d0b2754bdf87de6b7d4c77c5b7fbdf8c8`
- `git merge-base --is-ancestor 5fea02a… 4d81cd0…` passed.
- The candidate-to-evidence diff contains only `CONFORMANCE-GATE.md` and
  `.planning/phases/01-conformant-lpcm-bitstream/01-VERIFICATION.md`.
- After the final summary/state commit, the executor reruns the full committed and
  uncommitted descendant-path check. Closure is accepted only if every path remains
  inside the explicit plan 01-10 documentation/state allowlist.

## Verification

- `cargo test --locked --all-targets`: **PASS**, 285 tests.
- `cargo clippy --locked --all-targets -- -D warnings`: **PASS**.
- `cargo deny check`: **PASS**, advisories/bans/licences/sources.
- `bash tools/prove-guards.sh`: **PASS**, 6/6 canaries.
- Linked shipping graph: **PASS**, exactly `iamf` and `thiserror`.
- Pinned reference manifest: **PASS**, 4 tests.
- External-oracle conformance: **PASS**, 22 tests; 300/300 frames,
  0/1800 sample differences, strict parser acceptance, and zero-byte diff.
- Evidence-shape validator: **PASS**, one normal outcome and four unique successes.
- Independent run re-query: **PASS**, exact SHA and terminal success.
- Requirement ledger: **PASS**, 63 checked definitions and 63 unique Complete rows.
- Artifact/key-link audit: **PASS**, 56/56 unique artifacts and 36/36 links.
- `cargo fmt --all -- --check`: known non-blocking repository-wide drift, unchanged
  from the prior verification report.

## Task Commits

1. **Task 1: Freeze candidate and audit exact-SHA evidence** — verification-only;
   candidate `61ba098413b48751838035c41a068b270a7a1ba0`.
2. **Task 2: Publish authorized candidate** — external mutation only; no repository
   commit. Run 34286069472 reproduced the two infrastructure failures.
3. **Task 3: Diagnose and restore four executed target paths** — `5fea02a`.
4. **Task 4: Record proof and re-run goal-backward verification** — `4d81cd0`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Preserved committed LF bytes on Windows**

- **Found during:** Task 3, exact-candidate run 34286069472.
- **Issue:** Git checkout converted the committed golden dump to CRLF, so the
  byte-exact dump assertion failed before the named golden step.
- **Fix:** Set global `core.autocrlf=false` before checkout on Windows.
- **Files modified:** `.github/workflows/ci.yml`.
- **Verification:** Windows job 102265092145 passed the full suite and named golden
  step in run 34287100850.
- **Commit:** `5fea02a`.

**2. [Rule 3 - Blocking] Replaced the retired native x86_64 macOS runner with Rosetta execution**

- **Found during:** Task 3, exact-candidate run 34286069472.
- **Issue:** `macos-13`, GitHub's native x86_64 hosted image, is unavailable.
- **Fix:** Retained `x86_64-apple-darwin`, verified Rosetta 2, and configured Cargo
  to execute the produced x86_64 test binaries through `arch -x86_64`.
- **Files modified:** `.github/workflows/ci.yml`.
- **Verification:** Rosetta job 102265092177 passed the full suite and named golden
  step in run 34287100850.
- **Commit:** `5fea02a`.

**Total deviations:** 2 auto-fixed blocking CI infrastructure issues. **Impact:**
both preserve the original four executed target paths; neither changes Rust source,
tests, fixtures, assertions, dependencies or byte-identity semantics.

## Authentication and Authorization Gates

GitHub CLI authentication was already valid. Two authorization checkpoints were
honored: the user authorized publishing candidate `61ba098…`, then separately
authorized publishing only remediated candidate `5fea02a…`. Both used normal,
non-force pushes to `origin/main`; no other SHA or remote state was published.

## Issues Encountered

- GitHub annotates pinned checkout v4's Node.js 20 runtime as deprecated and forces
  Node.js 24. The action completed successfully; track as future maintenance.
- Repository-wide rustfmt drift remains a non-blocking, pre-existing cleanup item.
- Legal/patent sufficiency and contributor contamination attestation remain
  deliberately human-owned due diligence, not an implementation gap.

## Next Phase Readiness

Phase 1's executable gap is closed. The parser, round-trip and fuzzing work in Phase
2 can now build on a byte-identity baseline proven by both reference implementations
and one complete four-target exact-candidate matrix.

## Self-Check: PASSED

- This summary exists and passes `git diff --check`.
- Task commits `5fea02a` and `4d81cd0` exist in repository history.
- The final post-metadata candidate-descendant allowlist is run after the metadata
  commit and reported in the completion result.
