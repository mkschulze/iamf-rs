---
phase: quick-260913-th8
plan: 01
subsystem: encoder / mix-presentation validation
status: complete
tags: [iamf, mix-presentation, validation, encoder-builder, conformance]
requires: [260913-qk3]
provides:
  - ErrorKind::NoMixPresentation
  - ErrorKind::DuplicateMixPresentationAudioElement
  - MixPresentation::validate() num_sub_mixes / duplicate audio_element_id / reserved headphones_rendering_mode findings
affects: [src/encoder.rs validate_declarations, DescriptorSet::validate, ParsedSequence::validate, test_000124 semantic_sha256]
tech-stack:
  added: []
  patterns: [handle-lowered validation clone of presentation templates]
key-files:
  created: []
  modified:
    - src/error.rs
    - src/encoder.rs
    - src/obu/mix_presentation.rs
    - tests/encoder_builder.rs
    - tests/error_shape.rs
    - tests/encoder_streaming.rs
    - tests/sequence_parse.rs
    - tests/support/reference_expectations.rs
    - HANDOFF.md
decisions:
  - "build() validates a clone of each presentation template with audio_element_id lowered from the handles, so placeholder template ids cannot trigger presentation findings"
  - "Duplicate Audio Element references are rejected on handles with a dedicated kind after SubMixCountNotOne and ReservedHeadphonesRenderingMode"
  - "NoMixPresentation is checked last in validate_declarations so earlier declaration errors keep their kinds"
  - "The qk3 zero-Mix-Presentation compliance message is unchanged (D-04)"
metrics:
  duration: ~35 min
  completed: 2026-09-13
actuals:
  tokens: 7169
  tasks: 3
  commits: 3
plan_head_before: 36bf30ac2ae40aab45368d3eaa422cd0383dbfd8
---

# Phase quick-260913-th8 Plan 01: Reject empty Mix Presentation sets and duplicate element references Summary

`build()` now rejects an IA Sequence with no Mix Presentation (`NoMixPresentation`) and a Mix Presentation that lists one Audio Element handle twice (`DuplicateMixPresentationAudioElement`). It validates presentation templates on a clone whose ids come from the handles. `MixPresentation::validate()` reports `num_sub_mixes`, duplicate `audio_element_id` and reserved `headphones_rendering_mode` findings for foreign input, using the exact research E.2 text and order.

## Tasks

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Duplicate Audio Element: builder rejection, lowered clone, validate() finding (tracer) | 7683b70 | src/error.rs, src/encoder.rs, src/obu/mix_presentation.rs, tests/encoder_builder.rs, tests/sequence_parse.rs |
| 2 | Zero Mix Presentations rejected; rate-matched mono presentation in encoder_streaming | 74d6d9f | src/error.rs, src/encoder.rs, tests/encoder_builder.rs, tests/error_shape.rs, tests/encoder_streaming.rs |
| 3 | Sub-mix-count and reserved-headphones findings, test_000124 hash, HANDOFF.md | 6de3429 | src/obu/mix_presentation.rs, tests/sequence_parse.rs, tests/support/reference_expectations.rs, HANDOFF.md |

Deferred items were written to `260913-th8-deferred-items.md`. Per the orchestrator constraint that file is not committed here; it is left for the docs commit.

## RED evidence

- Task 1, `cargo test --locked --test encoder_builder build_rejects_the_same_audio_element`: this failed to compile on HEAD with `error[E0599]: no variant or associated item named DuplicateMixPresentationAudioElement found for enum iamf::ErrorKind`. The research probe had already shown the runtime behaviour on HEAD: `Ok(Base)`, wire ids `[0, 0]`. That confirms qk3 deferred item 2. In `sequence_parse`, `mix_presentation_validate_reports_each_later_duplicate_audio_element_id`, `mix_presentation_duplicate_scope_spans_sub_mixes` and `both_validators_carry_the_duplicate_audio_element_finding` all FAILED (`test result: FAILED. 3 passed; 3 failed`).
- Task 2, `cargo test --locked --test encoder_builder build_rejects_zero_mix_presentations`: this failed to compile with `error[E0599]: no variant or associated item named NoMixPresentation`. The research probe showed `Ok(Simple)` on HEAD, which confirms qk3 deferred item 1.
- Task 3, `cargo test --locked --test sequence_parse`: `test result: FAILED. 31 passed; 5 failed` (the five new zero/two sub-mix, headphones, order and both-validator tests).
- Task 1 tracer gate: `parse_reference` passed unchanged after Task 1 (`test result: ok. 6 passed`).

## Verification

- `cargo fmt --all -- --check`: clean
- `cargo clippy --locked --all-targets -- -D warnings`: clean
- `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`: clean
- `cargo test --locked`: 28 `test result: ok` lines, 0 failures
- `cargo test --locked --doc`: `test result: ok. 4 passed`
- `cargo test --locked --features fuzzing --test fuzz_regression`: `test result: ok. 5 passed`
- encoder_builder: `test result: ok. 64 passed; 0 failed`
- error_shape: `test result: ok. 17 passed; 0 failed`
- encoder_streaming: `test result: ok. 23 passed; 0 failed`
- sequence_parse: `test result: ok. 36 passed; 0 failed`
- parse_reference: `test result: ok. 6 passed; 0 failed`
- The plan's Task 3 `<verify>` command ran end to end and printed `VERIFY_PASS`.

### Reference hash

Before the literal was edited, `target/th8-parse-reference-before.log` failed only on `test_000124.iamf`, with computed `left: "795ef2885684954d4e5eda8e26c52e0a1328488844d4de54e82eb1e69c8a9e2f"`. That matches the pre-measured value exactly.

`test_000124.iamf semantic_sha256: 893a6103219259eb5863c01d649f97a8af298f0dcbda3e0514c5236264016ad6 -> 795ef2885684954d4e5eda8e26c52e0a1328488844d4de54e82eb1e69c8a9e2f`

This is the only expectation change: the diff against 36bf30a has exactly one `-` and one `+` line. The old/new hashes and the reason are in the body of commit 6de3429.

### Conformance

The docker probe (`docker image inspect iamf-tools:v2.1.0`, 60 s perl alarm) exited 0, so the image is present and no shim was used.

**Run A** (`IAMF_REF_DECODER` set, 1200 s alarm, `target/th8-conformance.log`): `test result: FAILED. 26 passed; 2 failed`, `conf_exit=101`, no `TIMED OUT`. The final `failures:` list:
- `the_flac_fixture_is_conformant`
- `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode`

Both are the known pre-existing failures from qk3 deferred item 5 (`iamfdec` built without FLAC/Opus). They did not pass. There were no other failures.

CONF lines from run A:
```
[expanded Lfe] CONF-05: iamfdec decoded 128 frames from a Base-Enhanced header; the Simple-header control decoded 0 frames (44 bytes)
[expanded Ch9_1_6] CONF-05: iamfdec decoded 128 frames from a Base-Enhanced header; the Simple-header control decoded 0 frames (44 bytes)
[parallax delivery] CONF-06: decoder_main reported "Decoded 1 temporal units." (exit Some(0), recorded but not the signal)
[phase1_structure_only] CONF-06: decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase1_endianness] CONF-05 (libiamf sample identity): 300 sample frames, 0 of 600 samples differ, limiter delta 0
[phase1_endianness] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase1_sample_identity] CONF-05 (libiamf sample identity): 300 sample frames, 0 of 1800 samples differ, limiter delta 0
[phase1_sample_identity] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
```

**Run B** (`IAMF_REF_DECODER` unset, 1200 s alarm, `target/th8-conformance-conf06.log`): `test result: ok. 28 passed; 0 failed`, `conf_exit=0`. With `iamfdec` absent, the CONF-05 clauses are SKIPPED (4x `SKIP CONF-05: IAMF_REF_DECODER is unset...`, plus `SKIPPED — no iamfdec` for phase1_endianness, phase1_sample_identity and phase3_flac). CONF-06 lines:
```
[phase1_endianness] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase1_structure_only] CONF-06: decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[parallax delivery] CONF-06: decoder_main reported "Decoded 1 temporal units." (exit Some(0), recorded but not the signal)
[phase1_sample_identity] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
```

### Boundary

`git diff --quiet 36bf30a -- src/bits src/model src/sequence.rs src/lib.rs src/fuzzing.rs src/packing.rs src/dump.rs DIFF-LEDGER.md tests/golden.rs tests/fixtures tests/conformance.rs tests/parallax_contract.rs tests/support/... fuzz/fuzz_targets` exited 0. `git diff --name-only 36bf30a -- src/obu` printed only `src/obu/mix_presentation.rs`. The golden fixtures and DIFF-LEDGER.md are unchanged. Foreign uncommitted changes were not touched or staged.

## Deviations from Plan

- **The deferred-items file is not committed.** The plan lists `260913-th8-deferred-items.md` in the Task 3 commit, but the orchestrator's constraints say not to commit the task's own docs artifacts, and the constraint takes precedence. The file is written and left for the orchestrator's docs commit.
- **The RED runs failed at compile time.** They could not fail at runtime, because the tests reference the new `ErrorKind` variants. The runtime behaviour on HEAD is taken from the research probe (`Ok(Base)` / `Ok(Simple)`).
- **The HANDOFF.md sentence names the two error kinds** (`NoMixPresentation`, `DuplicateMixPresentationAudioElement`), so that the plan's `grep -q 'NoMixPresentation' HANDOFF.md` verify passes.

Otherwise the plan was executed as written.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: src/error.rs `NoMixPresentation`, `DuplicateMixPresentationAudioElement`
- FOUND: src/encoder.rs `lowered.validate()`, `ErrorKind::NoMixPresentation`
- FOUND: src/obu/mix_presentation.rs `sub_mix.audio_element_id`, `IAMF_OBU.c:760-782`, `IAMF_decoder.c:1309-1315`
- FOUND: .planning/quick/260913-th8-reject-empty-mix-presentations-and-dupli/260913-th8-deferred-items.md
- FOUND commits: 7683b70, 74d6d9f, 6de3429
