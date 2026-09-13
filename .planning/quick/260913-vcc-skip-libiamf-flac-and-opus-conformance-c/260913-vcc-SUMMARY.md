---
phase: quick-260913-vcc
plan: 01
subsystem: conformance-gate
tags: [conformance, libiamf, flac, opus, ci, reference-manifest]
status: complete
requires:
  - tools/build-reference.sh manifest stamp (dep_codecs_disabled, host_triple)
provides:
  - manifest-driven CONF-05 skip for the FLAC/Opus fixtures on hosts without those codecs
  - reference-workflow guard that makes that skip impossible in CI
affects:
  - tests/conformance.rs
  - .github/workflows/reference.yml
  - CONFORMANCE-GATE.md
tech-stack:
  added: []
  patterns:
    - line-scan manifest reader (no JSON dependency), the same approach as tests/reference_manifest.rs
    - exhaustive FixtureCodec match with no wildcard arm, so a new codec has to make an explicit choice
key-files:
  created: []
  modified:
    - tests/conformance.rs
    - .github/workflows/reference.yml
    - CONFORMANCE-GATE.md
decisions:
  - "The FLAC/Opus CONF-05 skip is decided only by .reference-manifest.json dep_codecs_disabled. It never looks at iamfdec output, because iamfdec exits 0 on failure"
  - "An absent or malformed flag is an error. true on x86_64-unknown-linux-gnu is also an error. None of these skip"
  - "The reference workflow fails before any cargo test unless dep_codecs_disabled is literally false. This is recorded as a skip, not a D-17 waiver"
metrics:
  duration: "~4 min measured from ledger (wall clock after plan reading)"
  completed: 2026-09-13
  tasks: 2
  files: 3
actuals:
  tokens: 3650
  tasks: 2
  commits: 3
plan_head_before: 9c905ce306dd0cc58481fd9714408a09d44d2c5d
---

# Quick 260913-vcc: Skip libiamf CONF-05 for FLAC and Opus when the reference lacks those codecs

The FLAC/Opus CONF-05 clause now skips with a printed reason, but only when `.reference-manifest.json` records `dep_codecs_disabled: true` on a host other than x86_64 Linux. Every other clause still runs. A new CI guard step fails the reference job unless the flag is `false`.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 9f76fdd | fix(quick-260913-vcc): skip libiamf CONF-05 for FLAC/Opus when the reference lacks those codecs |
| 2 | 01e1885 | ci(quick-260913-vcc): fail the reference job if its libiamf build lacks FLAC/Opus |
| 2 | 98fd653 | docs(quick-260913-vcc): record the host-dependent FLAC/Opus CONF-05 skip |

## What changed

- `tests/conformance.rs`:
  - `manifest_raw_value` reads one key per line. A `:` check rejects the `dep_codecs_disabled_note` decoy.
  - `libiamf_flac_opus_skip_reason(fixture_name, manifest) -> Result<Option<String>, String>` makes the decision.
  - `assert_conformant`'s CONF-05 `Some(decoder)` arm first matches exhaustively on `spec.codec`. `Lpcm { .. }` gives `None` and never reads the manifest. `Flac | Opus` reads the manifest and calls the helper. The existing round-trip body is unchanged (confirmed with `git diff -w`) and now sits in the `else` branch. CONF-06 still runs after a skip.
  - New offline test `conf05_flac_opus_skip_is_decided_by_the_manifest` covers all 8 behaviours: darwin/true skip, darwin/false, linux/false, absent, malformed `"yes"`, linux/true, missing host_triple, and the decoy.
- `.github/workflows/reference.yml`: new step "Assert the reference decoder has FLAC and Opus (CONF-05 may not skip here)". It is step index 5, after build (2) and before "Assert the manifest matches REFERENCES.md" (6).
- `CONFORMANCE-GATE.md`: new `### Host-dependent skip: CONF-05 for FLAC and Opus (not a waiver)` subsection, placed before `## Cross-target byte-identity evidence`.

## Verification evidence

TDD RED: before the helper existed, the test failed to compile with 8 errors of `error[E0425]: cannot find function libiamf_flac_opus_skip_reason`. GREEN: `test result: ok. 1 passed`.

Full conformance run with `IAMF_REF_DECODER` set (`target/vcc-conformance.log`):

The two SKIP lines:
```
SKIP CONF-05 (libiamf FLAC/Opus) for phase3_flac: reference decoder built without FLAC/Opus codecs (dep_codecs_disabled=true in .reference-manifest.json; only x86_64 Linux reference hosts ship them — tools/build-reference.sh)
SKIP CONF-05 (libiamf FLAC/Opus) for phase3_opus: reference decoder built without FLAC/Opus codecs (dep_codecs_disabled=true in .reference-manifest.json; only x86_64 Linux reference hosts ship them — tools/build-reference.sh)
```
CONF-06 lines for FLAC/Opus:
```
[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 2 temporal units." (exit Some(0), recorded but not the signal)
```
LPCM CONF-05 sample-identity lines (the clause still runs for LPCM):
```
[phase1_endianness] CONF-05 (libiamf sample identity): 300 sample frames, 0 of 600 samples differ, limiter delta 0
[phase1_sample_identity] CONF-05 (libiamf sample identity): 300 sample frames, 0 of 1800 samples differ, limiter delta 0
```
Final line: `test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.26s`, `conf_exit=0`.

Tracer run (`target/vcc-tracer.log`): `test result: ok. 3 passed; 0 failed; ...; 26 filtered out`, `conf_exit=0`. Both phase3 fixtures still print clause 0, CONF-02, CONF-03, CONF-04 and CONF-06.

Offline `env -u IAMF_REF_DECODER cargo test --locked` (`target/vcc-offline.log`): exit 0, and every `test result:` line is `ok` (28 binaries).

fmt check and `clippy --all-targets -D warnings`, with and without `--features fuzzing`, are clean.

CI guard, with the step extracted from the YAML via ruby and run against derived manifests (`target/vcc-ci-guard.log`): `false_exit=0`, `true_exit=1`, `absent_exit=1`. For absent it reports `dep_codecs_disabled=None`.

Negative proof, flag flipped to `false` (`target/vcc-flip.log`): the run printed `errno: -6, fail to configure decoder.` and `test result: FAILED. 0 passed; 1 failed`, with `flip_exit=101`. This failure is the expected result, not a regression: with `false`, the real CONF-05 clause runs and fails on this host's codec-less iamfdec.

Manifest restore: `shasum -a 256 -c target/vcc-manifest.sha256` returned `.reference-manifest.json: OK` (sha256 `52b638b7bed6c72db9c881b7c4e3cbe596addf01b84c4b4dc79871b0e54e382b`). `"dep_codecs_disabled": true` is back in place.

Boundaries: `git diff --quiet 9c905ce -- src tests/support tests/fixtures tests/reference_manifest.rs tests/golden.rs DIFF-LEDGER.md tools` passes. The six foreign-modified files and the untracked files were not staged or edited.

## Deviations from Plan

**1. [Rule 3 - Blocking] The Opus conformance test now prints its GateReport**
- **Found during:** Task 1 tracer run.
- **Issue:** `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode` never called `report.print`, and that was already true at 9c905ce. As a result the required `[phase3_opus] clause 0 ...` and `[phase3_opus] CONF-06 ...` lines could not appear, and the plan's verify could not pass.
- **Fix:** added one `report.print(fixture.name);` after the test's existing CONF-03 assertion. No assertion changed.
- **Files modified:** tests/conformance.rs
- **Commit:** 9f76fdd

**2. [Process note] Committed on `main`**
`git.base-branch --is-protected main` returns `true`. The commits still went to `main` because the orchestrator named `main` as the sequential working branch, `.planning/config.json` has `git.branching_strategy: "none"`, and every earlier quick task (th8, qk3) committed there. None of this was a drift recovery, and no ref was rewritten.

STATE.md and ROADMAP.md were left untouched, as instructed. The orchestrator owns the docs commit.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: tests/conformance.rs (contains `libiamf_flac_opus_skip_reason`)
- FOUND: .github/workflows/reference.yml (contains `dep_codecs_disabled`)
- FOUND: CONFORMANCE-GATE.md (contains `dep_codecs_disabled`)
- FOUND commits: 9f76fdd, 01e1885, 98fd653 (`git rev-list --count 9c905ce..HEAD` = 3)
