---
phase: quick-260913-wlv
plan: 01
subsystem: mix-presentation-validation
status: complete
tags: [iamf, mix-presentation, loudness, validation, build]
requires: [260913-th8]
provides:
  - ErrorKind::DuplicateAnnotationsLanguage
  - ErrorKind::DuplicateAnchorElement
  - MixPresentation::validate language and anchor findings
  - pub(crate) scalable_layout_finding in both cross-OBU validators
affects: [EncoderBuilder::build, DescriptorSet::validate, ParsedSequence::validate, reference expectations]
tech-stack:
  added: []
  patterns: [enumerate/take(position)/any duplicate scan, shared pub(crate) finding helper]
key-files:
  created:
    - .planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-deferred-items.md
  modified:
    - src/error.rs
    - src/encoder.rs
    - src/obu/mix_presentation.rs
    - src/obu/mod.rs
    - src/model/mod.rs
    - src/sequence.rs
    - tests/error_shape.rs
    - tests/encoder_builder.rs
    - tests/sequence_parse.rs
    - tests/parse_reference.rs
    - tests/support/reference_expectations.rs
    - HANDOFF.md
decisions:
  - "annotations_language equality is ASCII-case-insensitive (RFC 5646 2.1.1), stricter than iamf-tools' exact bytes"
  - "anchor_element duplicates compare raw u8, Unknown 0 and reserved values included, never folded"
  - "num_layouts >= num_layers for a single scalable channel element is a finding only, never a build() or writer rejection"
metrics:
  duration: ~35min
  completed: 2026-09-14
actuals:
  tokens: 8714
  tasks: 3
  commits: 3
plan_head_before: 3347a7dc8d89c7a1383341e34cf443293405836d
---

# Quick 260913-wlv Plan 01: Unique annotations languages, unique anchor elements, scalable num_layouts finding Summary

`build()` now rejects a Mix Presentation that repeats an `annotations_language` (ASCII case folded) or repeats a raw `anchor_element` within one loudness info. It does so with two payload-free kinds, checked after `DuplicateMixPresentationAudioElement`. `MixPresentation::validate()` reports both duplicates. Both cross-OBU validators now report `num_layouts < num_layers` for a sub-mix whose only element is a scalable channel element. Exactly one reference hash changed: `test_000063`.

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | build() rejection of duplicate annotations languages and anchor elements (tracer) | b03597b |
| 2 | Language and anchor findings in MixPresentation::validate(), test_000063 hash | 3d6e3cb |
| 3 | Scalable num_layouts finding in both validators, deferred items, gates | 2d2c150 |

Tracer gate (Task 1): verify re-ran green end-to-end before expansion.

## Reference expectation change

`test_000063.iamf semantic_sha256: 793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24 -> a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409`

- Before the update, `every_positive_matches_its_complete_modeled_field_ledger` computed exactly `a2774dd6…9465409` for test_000063.
- After the update, all positives and negatives passed, which confirms no other fixture changed.
- `git diff -U0 3347a7d -- tests/support/reference_expectations.rs` shows only the old/new line pair.
- The provenance test `test_000063_reports_its_duplicate_anchor_element_and_test_000062_does_not` passes. test_000059, the only `num_layers = 2` fixture, has `num_layouts = 2` and no finding.

## Verification

- `cargo fmt --all -- --check`: ok.
- `cargo clippy --locked --all-targets -- -D warnings`, with and without `--features fuzzing`: clean.
- `cargo test --locked`: `full_exit=0`, 28 `test result: ok`, 0 non-ok.
- `cargo test --locked --doc`: ok (4). `--features fuzzing --test fuzz_regression`: ok (5).
- `docker image inspect iamf-tools:v2.1.0`: exit 0.
- Conformance with `IAMF_REF_DECODER` (perl alarm, 1200 s): `test result: ok. 29 passed; 0 failed`, `conf_exit=0`, no TIMED OUT.
  - `[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 3 temporal units."`
  - `[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported "Decoded 2 temporal units."`
  - Only SKIP lines: `SKIP CONF-05 (libiamf FLAC/Opus) for phase3_flac: …` and `… for phase3_opus: …` (dep_codecs_disabled=true).
- Protected paths are unchanged since 3347a7d: DIFF-LEDGER.md, tests/fixtures, golden, conformance, parallax_contract, support fixtures, tools, fuzz.

## Deviations from Plan

- **Docs commit not made by the executor.** The plan's Task 3 says to commit `260913-wlv-deferred-items.md` as `docs(quick-260913-wlv): …`. The orchestrator constraints say not to commit this task's planning artifacts, so the file was written but left uncommitted for the orchestrator's docs commit. STATE.md and ROADMAP.md were also left to the orchestrator.
- **Extra control assertion.** `mix_presentation_validate_reports_each_later_case_insensitive_duplicate_language` also asserts that the distinct `["en-us","es-mx"]` gives no findings.

Otherwise the plan was executed as written. The `src/` changes follow the research scratch diff, plus the plan-specified doc-comment and comment wording.

## Known Stubs

None.

## Self-Check: PASSED

- Commits b03597b, 3d6e3cb and 2d2c150 are on `main`, and `git rev-list --count 3347a7d..HEAD` = 3.
- `260913-wlv-deferred-items.md` exists.
- `git diff --quiet HEAD -- src tests HANDOFF.md` passes.
