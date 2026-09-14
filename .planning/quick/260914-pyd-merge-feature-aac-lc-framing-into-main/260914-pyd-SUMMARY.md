---
phase: quick-260914-pyd
plan: 01
subsystem: codec-framing
tags: [aac-lc, merge, encoder, obu, codec-config, references]

requires:
  - phase: quick-260913-vvx
    provides: "Opus pre_skip start-trim enforcement in tests/encoder_streaming.rs, whose (result, trimming) tuple shape this merge preserved"
provides:
  - "AAC-LC Audio Frame framing merged into main as a fourth pre-encoded codec alongside LPCM/FLAC/Opus"
  - "src/obu/codec_config.rs::aac_lc_output_sample_rate() — pub(crate) inverse lookup closing a non-exhaustive DecoderConfig match the merge exposed"
  - "REFERENCES.md documentation of the opt-in bundled FDK-AAC reference-tier archive"
affects: [encoder, obu-codec-config, tests-encoder-builder, tests-encoder-streaming, tests-round-trip, references]

actuals:
  tokens: 9500
  tasks: 3
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Framing-only codec boundary (established for FLAC/Opus) extended to AAC-LC: this crate accepts caller-supplied pre-encoded raw_data_block() access units, never encodes/decodes"

key-files:
  created: []
  modified:
    - src/encoder.rs
    - src/obu/codec_config.rs
    - src/obu/mod.rs
    - tests/encoder_builder.rs
    - tests/encoder_streaming.rs
    - tests/round_trip.rs
    - REFERENCES.md
    - .planning/PROJECT.md
    - HANDOFF.md

key-decisions:
  - "Real git merge --no-ff (not rebase, not squash) preserves the branch's own 8-commit history"
  - "Resolved exactly the 4 CONTEXT.md-predicted conflicts, each as specified; found and fixed one additional non-conflicting-but-non-exhaustive-match bug the merge exposed (see Deviations)"
  - "PROJECT.md's Key Decisions table did not get a new row — its convention is foundational 2026-09-07/08 scope/licence/architecture decisions; this routine feature merge does not fit that granularity"

patterns-established: []

requirements-completed: []

status: complete
completed: 2026-09-14
---

# Quick Task 260914-pyd: Merge feature/aac-lc-framing into main Summary

**Merged AAC-LC Audio Frame framing into main as a fourth pre-encoded codec (LPCM/FLAC/Opus/AAC-LC) via a real `git merge --no-ff`, resolving all 4 predicted conflicts plus one non-conflicting non-exhaustive-match bug the merge exposed in `output_sample_rate()`.**

## Performance

- **Tasks:** 3/3 completed
- **Files modified:** 24 (merge) + 2 (docs)
- **Commits:** 2 (merge commit, docs commit) — `.planning/STATE.md` updated but not committed, per plan

## Accomplishments

- `main` HEAD is now a real two-parent merge commit `474004a` combining `44221fe` (main) and `2674f1e` (`origin/feature/aac-lc-framing`)
- All 4 CONTEXT.md-predicted conflicts (`src/encoder.rs`, `tests/encoder_builder.rs`, `tests/encoder_streaming.rs`, `tests/round_trip.rs`) resolved exactly as specified; no conflict markers remain anywhere in the tree
- Fixed a real bug the merge exposed but did not conflict on textually: `src/encoder.rs`'s `output_sample_rate()` (added on `main` after the branch forked, for `ParameterRateMismatch` enforcement) was non-exhaustive over the new `DecoderConfig::AacLc` variant, which would have made `build()` reject every AAC-LC Audio Element with `InvalidDescriptorReference` at compile time this would not compile at all, so it was caught immediately by `cargo build`
- `REFERENCES.md`'s contamination-boundary section documents the opt-in bundled FDK-AAC reference-tier archive (`IAMF_REFERENCE_ENABLE_AAC=1`), following the existing "invoke, never read" pattern for Opus/FLAC
- `.planning/PROJECT.md` and `HANDOFF.md` both now describe AAC-LC framing as implemented and AAC-LC encode/decode as still out of scope, consistently worded and matching the FLAC/Opus precedent
- Full gate list (build/fmt/clippy ×2 feature sets, full offline test suite, fuzz replay, 9 prove-guards PASS, 1 float-census hit, `cargo tree` exactly `iamf thiserror`, `reference_expectations.rs` +4/-0, golden/fixtures_cap/citations) is green, re-run twice (once after Task 1's merge commit, once again after Task 3's docs commit)

## Task Commits

1. **Task 1: Merge origin/feature/aac-lc-framing into main, resolve the 4 conflicts, document the reference-tier AAC toggle** — `474004a` (merge; also carries Task 2's zero-amendment verification and the deviation fix below — see Deviations)
2. **Task 3: Update scope-boundary docs, record the quick task in STATE.md (not committed), re-run the full gate list** — `2ba6b33` (docs)

Task 2 produced no commit: every gate passed on the first run, so no amendment to Task 1's merge commit was needed for anything Task 2 itself discovered. (The deviation fix below was folded into the Task 1 commit directly, during Task 1's own Step 7 compile-clean requirement — not during Task 2.)

**Not committed by the executor (per plan):** `.planning/STATE.md` — edited with the new Quick Tasks Completed row and Session Continuity update, left for the orchestrator to commit.

## Files Created/Modified

- `src/encoder.rs` — merged `use crate::obu::{...}` import list (union of `HeadphonesRenderingMode` + `CODEC_ID_AAC`); the `FrameCodecMismatch` match auto-merged cleanly and was already exhaustive over `AacLc`; `output_sample_rate()` gained an `AacLc` arm (deviation fix, see below)
- `src/obu/codec_config.rs` — new `pub(crate) fn aac_lc_output_sample_rate(sampling_frequency_index: u8) -> Option<u32>`, an inverse lookup of the existing `AAC_LC_SAMPLE_RATES` table (deviation fix)
- `src/obu/mod.rs` — re-exports `aac_lc_output_sample_rate` as `pub(crate)`, following the existing `scalable_layout_finding` pattern (deviation fix)
- `tests/encoder_builder.rs` — merged import list: main's full kfs/m62 test-helper set plus `DecoderConfig` from the branch
- `tests/encoder_streaming.rs` — renamed `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`, kept main's `(result, trimming)` tuple shape and Opus `ref:` comment, added AAC-LC as a 5th case with `trimming = None`; `mono_builder`'s `(input, rate)` match gained `DecoderConfig::AacLc(_) => (FrameInput::AacLc(vec![0x21, 0x10, 0x04]), 48_000)`
- `tests/round_trip.rs` — took the branch's superset import list, minus one leftover unused `ParameterData` import (deviation fix, see below); `typed_aac_lc_codec_configs_round_trip` arrived as a clean branch addition
- `REFERENCES.md` — added a paragraph to the contamination-boundary section documenting the opt-in FDK-AAC reference-tier link
- `.planning/PROJECT.md` — reworded the AAC-LC Out-of-Scope entry: framing implemented, encode/decode still out of scope
- `HANDOFF.md` — updated the "three codecs shipped" item (4.1.3), the Scope **In** codec-framing line, and the Scope **Out** AAC-LC line, all to the same framing-implemented/decode-out-of-scope wording

## Decisions Made

- Kept the merge as a genuine `git merge --no-ff`, not a rebase/squash, per the plan and the user's own framing of this as "normal feature work"
- `PROJECT.md`'s Key Decisions table did not get a new row: its own convention is a small set of foundational 2026-09-07/08 scope/licence/architecture decisions, and this routine feature-branch merge does not fit that granularity — noted per the plan's own instruction to say so rather than force a row in

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1/Rule 3 - Bug / Blocking compile error] `output_sample_rate()` was non-exhaustive over the new `DecoderConfig::AacLc` variant**
- **Found during:** Task 1, Step 7 (`cargo build --locked --all-targets`)
- **Issue:** `src/encoder.rs`'s `output_sample_rate()` — added on `main` today (after the branch's 2026-09-12 fork point) to enforce `ParameterRateMismatch` — matched `Lpcm`/`Flac`/`Opus`/`Raw` but not `AacLc`. This hunk auto-merged with **no conflict markers** (git saw no textual overlap), so it was invisible to the 4-conflict diffstat CONTEXT.md predicted, and only surfaced as `error[E0004]: non-exhaustive patterns` on the very first `cargo build`. Left unfixed, this would have been a hard compile error, not a silent gap — but it is exactly the kind of match-arm-dropped-by-merge risk T-pyd-03 in the plan's own threat register calls out, just from a direction (main-only code added after the fork) the CONTEXT.md investigation didn't anticipate.
- **Fix:** Added `pub(crate) fn aac_lc_output_sample_rate(sampling_frequency_index: u8) -> Option<u32>` to `src/obu/codec_config.rs`, an inverse lookup of the existing `AAC_LC_SAMPLE_RATES` table using `.get()` (no indexing), mirroring the style of the neighboring `aac_lc_sampling_frequency_index` forward lookup. Re-exported it as `pub(crate)` from `src/obu/mod.rs` (matching the existing `scalable_layout_finding` pattern). Added `DecoderConfig::AacLc(aac_lc) => crate::obu::aac_lc_output_sample_rate(aac_lc.sampling_frequency_index)` as a new arm in `output_sample_rate()`, and dropped its now-unnecessary `const` qualifier (the lookup is a runtime `.get()` call, and the function has exactly one, non-const, call site).
- **Files modified:** `src/encoder.rs`, `src/obu/codec_config.rs`, `src/obu/mod.rs`
- **Verification:** `cargo build --locked --all-targets` clean; `cargo clippy --locked --all-targets -- -D warnings` (both feature sets) clean; `tests/encoder_builder.rs::build_rejects_aac_buffer_size_db_that_exceeds_its_24_bit_wire_field` and the full `EncoderBuilder::build()` path for AAC-LC exercised by the offline test suite (Task 2) all pass; `tests/citations.rs` still passes (the new function is not a `read_*`/`write_*` fn, so no citation is required, and it did not disturb the existing `// ref:` comments above `read_aac_lc_decoder_config`/`write_aac_lc_decoder_config`)
- **Committed in:** `474004a` (folded into Task 1's merge commit, per the plan's own instruction that a merge commit must build and lint clean on its own)

**2. [Rule 1 - Bug] Leftover unused `ParameterData` import in `tests/round_trip.rs`'s merged import list**
- **Found during:** Task 1, Step 7 (`cargo build --locked --all-targets`, warning; would have failed `cargo clippy -D warnings`)
- **Issue:** The plan's Step 4 called the branch's import-list side "a strict superset... take it as-is (or union manually; the result is identical either way)." That was true of the import list's *shape*, but not of its *usage*: the branch's own copy of `unknown_obu_and_raw_parameter_data_keep_exact_positions_and_offsets` used `ParameterData::Raw(...)` directly, while `main`'s own independent evolution of that same test (unrelated to this merge, from an earlier today quick task) had already switched to asserting through `SequenceObu::UngovernedParameterBlock`'s raw bytes instead. The merged file kept `main`'s version of the test body (clean auto-merge, no conflict) but the import brought in `ParameterData` anyway — unused.
- **Fix:** Removed `ParameterData` from the `use iamf::obu::{...}` list in `tests/round_trip.rs`.
- **Files modified:** `tests/round_trip.rs`
- **Verification:** `cargo build --locked --all-targets` produces zero warnings; `cargo clippy --locked --all-targets -- -D warnings` clean
- **Committed in:** `474004a` (folded into Task 1's merge commit)

---

**Total deviations:** 2 auto-fixed (1 Rule 1/3 blocking compile error, 1 Rule 1 lint-clean warning), both within Task 1's own Step 7 "fix anything that does not compile before proceeding" instruction — neither triggered a Task 2 `--amend` cycle, since both were caught and fixed before Task 1's own commit was made.
**Impact on plan:** Both fixes were necessary for the merge commit to build/lint clean, which is Task 1's own explicit success criterion. No scope creep — no behavior changed beyond making AAC-LC actually usable through the high-level builder (which the branch's own tests already assumed it was).

## Full Verification Gate Outcomes

Re-run twice: once after Task 1's merge commit (Task 2), once again after Task 3's docs commit (Task 3 Step 4). Both runs identical outcome:

| Gate | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo build --locked --all-targets` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo clippy --locked --all-targets --features fuzzing -- -D warnings` | pass |
| `cargo test --locked` (full offline suite, all binaries) | pass — 0 failed across every test binary |
| `cargo test --locked --features fuzzing --test fuzz_regression` | pass — 5/5 |
| `bash tools/prove-guards.sh` | 9/9 PASS |
| `bash tools/check-float-escape-census.sh` | exactly 1 hit (`src/model/loudness.rs:94`) |
| `cargo tree -e normal,no-proc-macro` | exactly `iamf thiserror` |
| `git diff --numstat 44221fe HEAD -- tests/support/reference_expectations.rs` | exactly `4  0` (4 insertions, 0 deletions — no pre-existing fixture hash changed) |
| `cargo test --locked --test golden` | pass — 11/11, including `the_regenerated_hash_matches_the_committed_hash` |
| `cargo test --locked --test fixtures_cap` | pass — 3/3 |
| `cargo test --locked --test citations` | pass — 3/3 |
| `src/error.rs` byte-identical to pre-merge `main` | confirmed (`git diff --quiet 44221fe HEAD -- src/error.rs`) |
| No `<<<<<<<`/`>>>>>>>` markers anywhere in `*.rs`/`*.md`/`*.sh`/`*.yml` | confirmed |
| No `*.proptest-regressions` files created | confirmed |

## Issues Encountered

None beyond the two deviations documented above — both were anticipated in kind (though not in exact location) by the plan's own Task 1 Step 7 instruction and threat T-pyd-03.

## Next Phase Readiness

- `main` now ships AAC-LC framing alongside LPCM/FLAC/Opus, with the high-level `EncoderBuilder` path fully usable for it (the deviation fix was required precisely to make that true)
- `origin/feature/aac-lc-framing` can now be deleted or left as a reference; it is fully merged
- `origin/wip/delivery-validator-conformance` remains untouched and unreferenced, as required
- **Follow-up needed:** see `260914-pyd-deferred-items.md` — `.planning/REQUIREMENTS.md`'s `DECO-03`, `API-06` and `API-09` text still describes AAC-LC as out of scope / lists only LPCM/FLAC/Opus, which this task's locked doc scope (`PROJECT.md` + `HANDOFF.md` only, per `260914-pyd-CONTEXT.md`) did not cover

## Self-Check: PASSED

All 9 modified/created source and doc files confirmed present on disk. Both commits (`474004a`
merge, `2ba6b33` docs) confirmed present in `git log --oneline --all`. This SUMMARY.md and
the accompanying deferred-items.md confirmed present.

---
*Quick task: 260914-pyd*
*Completed: 2026-09-14*
