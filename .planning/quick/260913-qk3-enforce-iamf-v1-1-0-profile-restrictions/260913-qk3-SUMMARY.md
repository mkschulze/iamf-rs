---
phase: quick-260913-qk3
plan: 01
subsystem: profile / encoder builder / validation
status: complete
tags: [iamf, profile, conformance, validation, spec-v1.1.0]
requires:
  - 260913-p28 (expanded-layout Base-Enhanced floor)
provides:
  - ErrorKind::SubMixCountNotOne, ErrorKind::ReservedHeadphonesRenderingMode
  - model::profile::select_sequence_profile, presentation_minimum_profile (pub)
  - model::profile::compliance_findings, sequence_limit_findings (pub(crate))
  - profile findings in DescriptorSet::validate() and ParsedSequence::validate()
affects:
  - EncoderBuilder::build() profile bytes for multi-presentation, multi-scene and unreferenced-element configurations
  - semantic_sha256 of reference fixtures test_000119/120/122/124/130
tech-stack:
  added: []
  patterns:
    - slice-based profile core shared by the builder and both validators
    - per-presentation floor maxed with a sequence-wide floor over unique audio_element_id
key-files:
  created:
    - .planning/quick/260913-qk3-enforce-iamf-v1-1-0-profile-restrictions/260913-qk3-deferred-items.md
  modified:
    - src/error.rs
    - src/encoder.rs
    - src/model/profile.rs
    - src/model/mod.rs
    - src/sequence.rs
    - tests/encoder_builder.rs
    - tests/error_shape.rs
    - tests/profile.rs
    - tests/sequence_parse.rs
    - tests/support/reference_expectations.rs
    - HANDOFF.md
    - .planning/REQUIREMENTS.md
decisions:
  - "build() rejects num_sub_mixes != 1 (SubMixCountNotOne) and any Reserved headphones_rendering_mode (ReservedHeadphonesRenderingMode) before presentation.validate() findings; the references are stricter than the spec's SHOULD"
  - "Profile scopes follow IAMF v1.1.0 (errata Simple/Base adopted): unique-element limits, Base scene-based/multi-layer limits and the BE 28-channel total are sequence-wide; Simple/Base channel limits and the BE 28-element limit are per Mix Presentation"
  - "Q1 = A: ParsedSequence::validate() also reports the sequence-limit finding; 5 reference hashes updated with provenance"
metrics:
  duration: 38min
  completed: 2026-09-13
  tasks: 3
  files: 12
estimate:
  tokens: 150000
  tasks: 3
actuals:
  tokens: 17400
  tasks: 3
  commits: 3
plan_head_before: 6622a09f0295734af93874b9e8e5760a4f5fbff7
---

# Quick 260913-qk3 Plan 01: Enforce IAMF v1.1.0 profile restrictions Summary

`build()` now rejects Mix Presentations libiamf cannot parse (sub-mix count other than 1, reserved headphones mode). Profile selection applies the spec's sequence-wide and per-presentation scopes through one slice-based core. Both validators report header profiles that no Mix Presentation complies with, or that the sequence's unique Audio Elements exceed.

## Tasks

| # | Task | Commit | Key files |
|---|------|--------|-----------|
| 1 | Reject sub-mix count != 1 and reserved headphones mode in build() (items 1, 2) | 48efbc0 | src/error.rs, src/encoder.rs, tests/encoder_builder.rs, tests/error_shape.rs, REQUIREMENTS.md (API-04) |
| 2 | Spec-scoped profile selection (items 3, 4) | dfe50b3 | src/model/profile.rs, src/encoder.rs, tests/profile.rs, tests/encoder_builder.rs, HANDOFF.md, REQUIREMENTS.md (API-03) |
| 3 | Profile findings in DescriptorSet/ParsedSequence validation, hash update (item 5) | 0bfdc70 | src/model/profile.rs, src/model/mod.rs, src/sequence.rs, tests/sequence_parse.rs, tests/support/reference_expectations.rs |

### TDD evidence

- **Task 1 RED:** after adding only the two error variants, `cargo test --locked --test encoder_builder build_re` gave `test result: FAILED. 29 passed; 3 failed`. The failures were `build_rejects_two_sub_mixes`, `build_rejects_zero_sub_mixes` and `build_rejects_reserved_headphones_rendering_modes`: `build()` returned Ok. This confirms deferred p28 items 1 and 2. They passed after the checks were added.
- **Task 2 RED:** 6 new or renamed builder tests failed on the Task 1 tree: `two_presentations_sum_channels_...`, `three_unique_elements_...`, `two_single_stereo_presentations_...`, both two-ambisonics tests and `an_unreferenced_second_element_...`. `a_shared_element_counts_once...` and `ambisonics_plus_stereo...` already passed, as expected. The direct tests/profile.rs tests needed the new pub functions, so they could not compile until GREEN.
- **Task 3:** the finding tests were written after the implementation, not RED first. The Step 0 projection probe ran before any src edit, with `QK3_PHASE=before changed=0`.

## Verification

Test results (`cargo test --locked`, full run, exit 0; IAMF_REF_DECODER unset):
- `tests/encoder_builder.rs: test result: ok. 58 passed; 0 failed`
- `tests/profile.rs: test result: ok. 39 passed; 0 failed`
- `tests/sequence_parse.rs: test result: ok. 27 passed; 0 failed`
- `tests/parse_reference.rs: test result: ok. 6 passed; 0 failed`
- `tests/conformance.rs: test result: ok. 28 passed; 0 failed` (CONF-05 clauses SKIP with the decoder unset)
- error_shape 17, golden 11, fixture 31: all ok. All 28 `test result:` lines in the log are ok.

Other gates:
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets -- -D warnings`: clean.
- `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`: clean.
- `cargo test --locked --doc`: `test result: ok. 4 passed`.
- `cargo test --locked --features fuzzing --test fuzz_regression`: `test result: ok. 5 passed`.

### Conformance with IAMF_REF_DECODER set (target/qk3-conformance.log)

The docker probe (`docker image inspect iamf-tools:v2.1.0`, 60 s perl alarm) exited 0, so no shim was used. The suite ran under the 1200 s alarm and did not time out. It exited 101 with **`test result: FAILED. 26 passed; 2 failed`**.

CONF-05 lines quoted from the log:
```
[expanded Lfe] CONF-05: iamfdec decoded 128 frames from a Base-Enhanced header; the Simple-header control decoded 0 frames (44 bytes)
[expanded Ch9_1_6] CONF-05: iamfdec decoded 128 frames from a Base-Enhanced header; the Simple-header control decoded 0 frames (44 bytes)
[phase1_sample_identity] CONF-05 (libiamf sample identity): 300 sample frames, 0 of 1800 samples differ, limiter delta 0
[phase1_endianness] CONF-05 (libiamf sample identity): 300 sample frames, 0 of 600 samples differ, limiter delta 0
```
CONF-06 lines from the same run:
```
[phase1_structure_only] CONF-06: decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase1_sample_identity] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[phase1_endianness] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
[parallax delivery] CONF-06: decoder_main reported "Decoded 1 temporal units." (exit Some(0), recorded but not the signal)
```

**The two failures are CONF-05 for `the_flac_fixture_is_conformant` and `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode`.** In both, `iamfdec` printed `errno: -6, fail to configure decoder.` and wrote a 44-byte WAV (`tests/conformance.rs:535`). **This predates the task.** An unmodified `git archive 6622a09` copy, given `.reference-manifest.json` and `.reference/`, fails both tests identically. The generated `phase3_flac.iamf` and `phase3_opus.iamf` are byte-identical before and after (`cmp`). Earlier quick tasks ran CONF-05 only filtered or with the decoder unset, so this had not surfaced. It is logged as deferred item 5. Because FLAC and Opus CONF-05 fail, they are **not** reported as passed.

A second run with IAMF_REF_DECODER unset (target/qk3-conformance-conf06.log, exit 0, `test result: ok. 28 passed`) collected the CONF-06 line the panicking test had skipped:
```
[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units." (exit Some(0), recorded but not the signal)
```

### Reference hash changes (Q1 = A)

Probe provenance: the before run had 0 mismatches. The after run changed exactly these 5, and each after-projection differs from the before-projection only by one appended Finding (diffed).

| Fixture | Old | New | Reason |
|---------|-----|-----|--------|
| test_000124.iamf | 153257eae43ba539d41c095cc684facb0edba6a70edb9e27c33836be118baedc | 893a6103219259eb5863c01d649f97a8af298f0dcbda3e0514c5236264016ad6 | F1: its one MP has 2 sub-mixes under a Base header; is_valid: false |
| test_000119.iamf | 13f77189d815adc5e0ce8327e171831702baf3cbb98be0ac9203e69419dcb355 | 1b21439e7ad58b67d7499af6265a1f8f172f9355b5caba7856bff94323c2f32a | F3: Simple header, 2 unique AEs (errata:1853); is_valid: false |
| test_000120.iamf | 1928ee543bbf14d8b8ed5c276a058c5679b2b82b6a82b476a35ec2e1edbf53a4 | 8602eda33e6259b59d6893e60ef04450c0938b957f084bd8c7fbe46621fd3659 | F3, same reason |
| test_000122.iamf | b324def10ef2e4ea0f6c68f3dd3b293d141aaed4c2305dbf7e76340ffa69353a | 9afccf111c59f50ca9e76a789ba7e4a82637487ba12fcdc989268b66cca4c696 | F3, same reason |
| test_000130.iamf | 219e6285ff7a47445f0007b01b2105dfba359eebcdc7dd054d1a58b2ab099d12 | 247bedac5d10092cb6d8881a2920973bc9f3bc7f779fa538f6bff5c4a39ec208 | F3, same reason |

No other expectation changed. The negative ValidationFinding test and the Opus/FLAC codec expectations pass unchanged.

### Boundary

`git diff --quiet 6622a09 -- src/obu src/bits src/lib.rs src/fuzzing.rs src/packing.rs src/dump.rs DIFF-LEDGER.md tests/golden.rs tests/fixtures tests/conformance.rs tests/parallax_contract.rs tests/support/fixture.rs tests/support/parallax_contract.rs tests/support/sequence_cases.rs tests/support/test_000003.rs fuzz/fuzz_targets` gave `boundary=0` (unchanged).

## Deviations from Plan

- **Branch guard:** the executor's pre-commit check reports `main` as protected. The orchestrator explicitly directed sequential commits on `main`, as earlier quick tasks did, so all three commits landed on `main`.
- **Conformance log grep:** the plan's automated verify expects `test result: ok` in target/qk3-conformance.log. That run honestly records `FAILED. 26 passed; 2 failed`, from the pre-existing FLAC/Opus CONF-05 failure above. The log was not edited, and the CONF-06-only rerun was written to a separate file.
- **Headphones location:** `Field("headphones_rendering_mode")` is used, as the plan says. Research had proposed `rendering_config.headphones_rendering_mode`.
- **Extra direct assertion:** `presentation_minimum_profile_rejects_what_no_profile_permits` also asserts that a compliant presentation gives `Simple`.
- **STATE.md / ROADMAP.md:** not updated by the executor. This is a quick task, and the orchestrator owns the docs commit and the quick-task table.

## Deferred Issues

See `260913-qk3-deferred-items.md`:
1. `build()` accepts zero Mix Presentations.
2. `build()` accepts a duplicate element in one sub-mix.
3. `MixPresentation::validate()` has no sub-mix or headphones findings.
4. The Binaural `decoder_main` abort.
5. The pre-existing CONF-05 FLAC/Opus `iamfdec` failure.

## Known Stubs

None.

## Threat Flags

None. The new code adds no endpoints or I/O. The foreign-input paths (`compliance_findings`, `sequence_limit_findings`) use no indexing and no bare arithmetic. Clippy passes with and without `--features fuzzing`, and so does the fuzz replay.

## Self-Check: PASSED
