---
phase: quick-260913-o3k
plan: 01
subsystem: encoder
status: complete
tags: [encoder, validation, conformance, layout, iamf-tools]
requires: []
provides:
  - LoudspeakerLayout::single_layer_substream_counts
  - ExpandedLoudspeakerLayout::single_layer_substream_counts
  - ErrorKind::CoupledSubstreamCountMismatch
affects: [EncoderBuilder::build, tests/encoder_streaming.rs helpers]
tech-stack:
  added: []
  patterns: [const-fn reference table cited to pinned iamf-tools symbols]
key-files:
  created:
    - .planning/quick/260913-o3k-reject-stereo-audio-elements-whose-subst/260913-o3k-deferred-items.md
  modified:
    - src/model/layout.rs
    - src/error.rs
    - src/encoder.rs
    - tests/encoder_builder.rs
    - tests/encoder_streaming.rs
    - tests/error_shape.rs
decisions:
  - "Single-layer channel-based elements must declare exactly the iamf-tools@v2.1.0 (substream_count, coupled_substream_count) for their layout; coupled count checked first (CoupledSubstreamCountMismatch), substream count second (ChannelCountMismatch)"
  - "The old coupled > substream / channel-sum clause is removed; the table test proves both equalities imply it"
metrics:
  duration: ~10 min
  completed: 2026-09-13
plan_head_before: acee797cf05a1bf2f752b41bdbdea4583c36f900
actuals:
  tokens: 10800
  tasks: 3
  commits: 2
---

# Quick 260913-o3k Plan 01: Reject channel elements with non-reference substream coupling Summary

`EncoderBuilder::build()` now rejects single-layer channel-based Audio Elements whose
`coupled_substream_count` or `substream_count` differs from the pinned iamf-tools@v2.1.0
`ValidateSubstreamCounts` table (e.g. Stereo 2/0, 5.1 6/0, 7.1.4 8/4), via a cited `const fn` table on
both layout enums and a new payload-free `ErrorKind::CoupledSubstreamCountMismatch`.

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (tracer, TDD) | Stereo 2/0 rejected at build(): table, error kind, topology check, helper migrations | bc1fd29 | src/model/layout.rs, src/error.rs, src/encoder.rs, tests/encoder_builder.rs, tests/encoder_streaming.rs |
| 2 (TDD) | Table-driven rejections, 23 positives, reserved None, error_shape registration | 1bc63b2 | tests/encoder_builder.rs, tests/error_shape.rs |
| 3 | Deferred-items record and full verification gate | none (docs artifact only; orchestrator commits it) | 260913-o3k-deferred-items.md |

TDD: RED observed for Task 1. `build_rejects_stereo_without_its_coupled_substream` first failed to
compile (no kind), then after adding the kind it panicked at `unwrap_err()` because Stereo 2/0 built Ok.
GREEN after the table and check landed. Task 2 tests passed immediately against Task 1 (as the plan
expected); no const-fn arm needed correcting.

## What changed

- `src/model/layout.rs`: `single_layer_substream_counts(self) -> Option<(u8, u8)>` on
  `LoudspeakerLayout` (cites `ValidateSubstreamCounts / CollectBaseChannelGroupLabels`) and on
  `ExpandedLoudspeakerLayout` (cites `ValidateSubstreamCounts /
  CollectChannelLayersAndLabelsForExpandedLoudspeakerLayout`). Per-arm comments name the coupled pairs
  and non-coupled channels. `Reserved` returns `None`; `Expanded` delegates.
- `src/error.rs`: `CoupledSubstreamCountMismatch` after `ChannelCountMismatch`. The 32-byte assertion
  is unchanged.
- `src/encoder.rs validate_element_topology`: let-else on the table (UnsupportedLayout if None), then
  coupled check, then substream check. Integer compares only.
- `tests/encoder_streaming.rs`: Stereo 1/1 single-handle helpers, 512 B `stereo_pcm_frame`, new
  256 B `mono_pcm_frame`, new `surround_builder` (5.1 4/2), `reverse_substream_order_builder` reworked
  to 5.1 4/2 returning `[SubstreamHandle; 4]` in declared order. A shared `lpcm_config()` helper replaces
  three copies of the same LPCM config. Every asserted kind, location and `bytes_written()` check kept.
- `tests/encoder_builder.rs`: `element_with_counts`, `build_counts`, and `element_with_layout` now
  derives counts from the table (9.1.6 becomes 9/7). `Profile::Simple` assertion untouched.

## Verification (run on this tree, 2026-09-13)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets -- -D warnings`: clean.
- `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`: clean.
- `cargo test --locked`: exit 0, 28 `test result:` lines, all ok. Named binaries:
  - `tests/encoder_builder.rs`: `test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
  - `tests/encoder_streaming.rs`: `test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
  - `tests/error_shape.rs`: `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
  - `tests/golden.rs`: `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
  - `tests/public_api.rs`: `test result: ok. 1 passed; ...`; `tests/citations.rs`: `test result: ok. 3 passed; ...`
  - `tests/round_trip.rs`: `test result: ok. 12 passed; ...`; `tests/parallax_contract.rs`: `test result: ok. 3 passed; ...`
- `cargo test --locked --doc`: `test result: ok. 4 passed; 0 failed; ...`
- `cargo test --locked --features fuzzing --test fuzz_regression`: `test result: ok. 5 passed; 0 failed; ...`
- Conformance, logged to `target/o3k-conformance.log` (`IAMF_TOOLS_IMAGE` and `IAMF_REF_DECODER` unset in
  the environment; local docker has `iamf-tools:v2.1.0 4a011a6d2b9e`, the pinned image):
  - `test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.49s`
  - **CONF-06 ran against decoder_main (not skipped):**
    - `[phase1_structure_only] CONF-06: decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)`
    - `[parallax delivery] CONF-06: decoder_main reported "Decoded 1 temporal units." (exit Some(0), recorded but not the signal)`
    - `[phase1_sample_identity] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)`
    - `[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)`
    - `[phase1_endianness] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)`
  - **CONF-05 (libiamf) was SKIPPED**, not passed: `SKIP CONF-05: IAMF_REF_DECODER is unset. ...` (4 lines)
    and `CONF-05 (libiamf sample identity): SKIPPED — no iamfdec` (3 fixtures). The three
    `probe_*` endianness probes also SKIPPED for the same reason.
- Boundary gate: `git diff --quiet acee797 -- src/obu src/sequence.rs src/fuzzing.rs src/packing.rs
  DIFF-LEDGER.md tests/golden.rs tests/fixtures tests/conformance.rs tests/support tests/sequence_parse.rs
  tests/descriptors.rs tests/profile.rs tests/parallax_contract.rs` returned exit 0 (unchanged).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 2 grep verify required fully qualified variant names**
- **Found during:** Task 2
- **Issue:** the positives table first used `use ExpandedLoudspeakerLayout as X;` aliases, so the plan's
  `grep -q 'ExpandedLoudspeakerLayout::Top6Ch'` / `StereoTpSi` checks failed.
- **Fix:** expanded the rows to fully qualified paths before committing.
- **Commit:** 1bc63b2

### Process notes

- The pre-commit protected-branch assertion reports `main` as protected
  (`git.base-branch --is-protected main` returned `true`). The orchestrator explicitly assigned this
  sequential, non-worktree run to `main`, matching prior quick-task commits (acee797 and earlier), so
  commits were made on `main` as instructed rather than halting.
- `tests/encoder_streaming.rs` gained a local `lpcm_config()` helper to de-duplicate the LPCM config
  across the rewritten builders; behaviour unchanged.
- Untracked `CLAUDE.md`, `fuzz/CLAUDE.md`, `tests/CLAUDE.md`, `tools/CLAUDE.md` and
  `tools/codec-fixtures/CLAUDE.md` appeared in `git status` during execution. They were not created by
  this plan and were not staged. `README.md` and `docs/` were left untouched.

## Deferred

Five items recorded in `260913-o3k-deferred-items.md` (expanded-layout profile, Binaural render abort,
scalable per-layer counts, parse-side `validate()` finding, projection coupling). 260913-n56 deferred
item 3 is resolved by this task.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: src/model/layout.rs, src/error.rs, src/encoder.rs, tests/encoder_builder.rs,
  tests/encoder_streaming.rs, tests/error_shape.rs, 260913-o3k-deferred-items.md, target/o3k-conformance.log
- FOUND commits: bc1fd29, 1bc63b2
