---
phase: quick-260913-lta
plan: 01
subsystem: encoder
status: complete
tags: [encoder, validation, conformance, codec-config, conf-06]
requires: []
provides:
  - "ErrorKind::MultipleCodecConfigs (payload-free unit variant)"
  - "EncoderBuilder::build() single-Codec-Config rule with spec and iamf-tools@v2.1.0 citations"
  - "Parallax delivery fixture on one shared LPCM Codec Config, accepted by the pinned decoder_main"
affects: [tests/conformance.rs CONF-06, tests/parallax_contract.rs]
tech-stack:
  added: []
  patterns: ["whole-sequence count check placed after per-config validation to preserve error precedence"]
key-files:
  created:
    - .planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md
  modified:
    - src/error.rs
    - src/encoder.rs
    - tests/encoder_builder.rs
    - tests/error_shape.rs
    - tests/support/parallax_contract.rs
    - tests/parallax_contract.rs
decisions:
  - "EncoderBuilder::build() rejects more than one Codec Config (IAMF v1.1.0 index.bs:1912) with ErrorKind::MultipleCodecConfigs at Field(\"codec_configs\"). No separate frame-size/sample-rate/bit-depth check, because none is reachable."
  - "The rule is not added to SequenceWriter, write_sequence, write_parsed_sequence or DescriptorSet::validate(), so low-level byte-exact round-trips of foreign files keep working"
  - "The Parallax fixture lowers the FLAC and Opus candidates onto the shared LPCM config. Per-codec deliveries are deferred."
metrics:
  duration: "6 min"
  completed: 2026-09-13
actuals:
  tokens: 2800
  tasks: 3
  commits: 2
plan_head_before: 1671f693335876d8129c0cba7392ef466dc2a70c
requirements: [API-01, API-09, API-10, CONF-06]
---

# Quick 260913-lta Plan 01: Reject multiple Codec Configs and repair the Parallax delivery Summary

`EncoderBuilder::build()` now enforces IAMF v1.1.0's "only one unique Codec Config OBU" rule and
returns a payload-free `ErrorKind::MultipleCodecConfigs` when it is broken. The Parallax delivery
fixture now uses a single LPCM Codec Config, and the pinned `iamf-tools:v2.1.0` `decoder_main`
decodes it (CONF-06 ran and passed).

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (tracer) | Reject a second Codec Config; repair the fixture | de829cf | src/error.rs, src/encoder.rs, tests/encoder_builder.rs, tests/support/parallax_contract.rs, tests/parallax_contract.rs |
| 2 | Rejection coverage, positive case, kind registration, doc comments | e77d4e6 | tests/encoder_builder.rs, tests/error_shape.rs, src/encoder.rs |
| 3 | Deferred items and the full release gate | (no code commit; docs artifact left for the orchestrator) | 260913-lta-deferred-items.md |

## What was built

- **`src/error.rs`:** a unit variant `MultipleCodecConfigs`, message "an IA sequence permits only one
  Codec Config". It has no payload, so `size_of::<Error>() <= 32` still holds.
- **`src/encoder.rs` `validate_declarations`:** a `self.codec_configs.len() > 1` check, placed after
  the per-config loop and before the audio-element loop.
  - It carries the four `// ref:` lines: `index.bs:1912`, `obu_processor.cc GetSampleRateAndFrameSize`,
    `rendering_mix_presentation_finalizer.cc GetCommonCodecConfigPropertiesFromAudioElementIds`, and
    `obu_sequencer_base.cc FillDescriptorStatistics`.
  - Its comment explains that the rule is whole-sequence and that it implies the per-sub-mix rules.
  - The doc comments on `add_codec_config` and `build` state the rule.
- **Five new builder tests:**
  - mismatched timing (LPCM 128 + Opus 960)
  - identical timing
  - configs split across mix presentations
  - a declared but unreferenced second config
  - one config shared by four elements, which builds as `Profile::BaseEnhanced` with 1 Codec Config
- **`tests/error_shape.rs`:** registers the new kind.
- **Fixture:**
  - The FLAC (89) and Opus (90) Codec Configs and both `include_bytes!` packet loads are removed.
  - The `FlacStereo` and `OpusStereo` candidates use the LPCM config, with `FrameInput::Lpcm(vec![0; 512])`
    frames.
  - Candidate names, presentations, ambisonics frames and the parameter block are unchanged.
- **Contract test:** now expects codec ids `[0]`.

## Verification (results as run)

- **CONF-06 ran against the pinned `decoder_main`. It was not skipped.** Verbatim from
  `target/lta-conf06.log`:
  - `[parallax delivery] CONF-06: decoder_main reported "Decoded 1 temporal units." (exit Some(0), recorded but not the signal)`
  - `test parallax_delivery_fixture_uses_the_offline_safe_reference_gates ... ok`
- `cargo test --locked --test encoder_builder`: 37 passed.
- `cargo test --locked --test error_shape`: 17 passed.
- `cargo test --locked --test parallax_contract`: 3 passed.
- `cargo test --locked --doc`: 4 passed.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --locked --all-targets -- -D warnings`: clean, with and without `--features fuzzing`.
- `cargo test --locked` (full suite): exit 0, and every test binary reported `ok` with 0 failed. This
  includes golden, DIFF-LEDGER, conformance (27 passed) and parallax_contract.
- `cargo test --locked --features fuzzing --test fuzz_regression`: 5 passed.
- `git status --porcelain -- DIFF-LEDGER.md tests/golden.rs tests/fixtures`: empty. Golden bytes and
  the ledger are unchanged.
- Existing builders with more than one Codec Config outside the fixture: none found. The full suite
  passed without migrating any.

## Deviations from Plan

- **TDD note for Task 2:** its four new tests passed on first run, because Task 1's implementation
  already satisfies them. They are characterization/coverage tests of that behaviour, and no
  implementation change was needed. This follows the plan's own sequencing.
- **Commits on `main`:** the orchestrator named `main` as the working branch for this sequential
  quick task, which matches the repo's existing quick-task commit history. Both commits landed there
  as instructed.
- **Pre-existing rustdoc error found:** `RUSTDOCFLAGS="-D warnings" cargo doc` fails on an unresolved
  `ParamDefinition` link at `src/sequence.rs:669`. That file was not touched and the check is not in
  the plan's gate. It is recorded in deferred-items.md (item 9) and not fixed.

Otherwise the plan executed as written.

## Known Stubs

None.

## Threat Flags

None. There is no new surface; the check is a pure length comparison (T-lta-01 mitigated, T-lta-03
and T-lta-04 accepted as planned).

## Self-Check: PASSED

- FOUND: src/error.rs, src/encoder.rs, tests/encoder_builder.rs, tests/error_shape.rs,
  tests/support/parallax_contract.rs, tests/parallax_contract.rs, 260913-lta-deferred-items.md
- FOUND commits: de829cf, e77d4e6
