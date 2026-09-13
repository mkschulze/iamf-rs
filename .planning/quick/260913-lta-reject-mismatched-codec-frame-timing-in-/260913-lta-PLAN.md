---
phase: quick-260913-lta
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/error.rs
  - src/encoder.rs
  - tests/encoder_builder.rs
  - tests/error_shape.rs
  - tests/support/parallax_contract.rs
  - tests/parallax_contract.rs
  - .planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md
autonomous: true
requirements: [API-01, API-09, API-10, CONF-06]

estimate:
  tokens: 70000
  raw_tokens: 70000
  tasks: 3
  confidence: low

must_haves:
  truths:
    - "EncoderBuilder::build() on a builder that declared an LPCM Codec Config (128 samples per frame) and an Opus Codec Config (960) used by elements in one sub-mix returns Err with kind ErrorKind::MultipleCodecConfigs at Location::Field(\"codec_configs\") (per D-LOCKED strict single Codec Config)"
    - "Two Codec Configs are rejected with the same error even when their timing is identical, when they feed different Mix Presentations, and when the second one is never referenced — the rule is whole-IA-sequence, per IAMF v1.1.0 index.bs:1912"
    - "One Codec Config shared by four Audio Elements in one sub-mix still builds, and the sequence profile is Profile::BaseEnhanced"
    - "parallax_contract::build_delivery() builds with exactly one Codec Config (wire id 0) feeding all four retained Audio Elements, and tests/parallax_contract.rs asserts codec ids == [0]"
    - "With the pinned iamf-tools:v2.1.0 container present, tests/conformance.rs::parallax_delivery_fixture_uses_the_offline_safe_reference_gates prints 'CONF-06: decoder_main reported' and passes; if the container is absent the SUMMARY says SKIPPED, never passed"
    - "size_of::<Error>() stays <= 32, golden byte-identity tests and DIFF-LEDGER.md are unchanged, and cargo clippy --all-targets -D warnings passes under the project lint set"
  artifacts:
    - path: "src/error.rs"
      provides: "unit variant ErrorKind::MultipleCodecConfigs"
      contains: "MultipleCodecConfigs"
    - path: "src/encoder.rs"
      provides: "count check in validate_declarations with spec and iamf-tools@v2.1.0 ref citations"
      contains: "index.bs:1912"
    - path: "tests/support/parallax_contract.rs"
      provides: "single-LPCM-Codec-Config Parallax delivery fixture"
    - path: ".planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md"
      provides: "recorded follow-ups (FLAC/Opus deliveries, parameter-block duration rule, trimming rule, low-level finding)"
  key_links:
    - from: "src/encoder.rs validate_declarations"
      to: "ErrorKind::MultipleCodecConfigs"
      via: "self.codec_configs.len() > 1 comparison after the per-config validation loop"
      pattern: "codec_configs.len\\(\\) > 1"
    - from: "tests/conformance.rs parallax_delivery_fixture_uses_the_offline_safe_reference_gates"
      to: "tests/support/parallax_contract.rs build_delivery"
      via: "decoder_main run in the pinned iamf-tools:v2.1.0 container"
      pattern: "build_delivery"
---

<objective>
Close review Consensus concern #2 and the CONF-06 failure deferred by quick 260913-js8.

`EncoderBuilder` accepts several Codec Configs whose timing disagrees. The Parallax delivery fixture
builds one such sequence: LPCM 128, FLAC 128 and Opus 960 in one sub-mix. The pinned `decoder_main`
rejects it. Research (executed against the pinned container) showed that the rule the references
actually apply is stronger than "equal frame size per sub-mix":

- IAMF v1.1.0 `index.bs:1912`, under the common restrictions for all profiles, says "There SHALL be
  only one unique Codec Config OBU".
- `iamf-tools@v2.1.0` `decoder_main` fails on any sequence with two Codec Configs, even when their
  timing is equal.

Per the LOCKED user decision (strict single Codec Config), this plan does three things:

1. `build()` rejects more than one Codec Config with a new payload-free `ErrorKind::MultipleCodecConfigs`
   at `Location::Field("codec_configs")`.
2. The Parallax fixture is repaired so all four retained Audio Elements share its one LPCM Codec Config.
3. The out-of-scope follow-ups are recorded in a deferred-items file.

Because an Audio Element's frame size, sample rate and bit depth come only from its Codec Config, the
per-sub-mix rules follow from the single-config rule and cannot be broken through the builder:
- frame size: `rendering_mix_presentation_finalizer.cc` `GetCommonCodecConfigPropertiesFromAudioElementIds`
- sample rate and bit depth: `cli_util.cc` `GetCommonSampleRateAndBitDepth`
- sequence-wide statistics: `obu_sequencer_base.cc` `FillDescriptorStatistics`

No separate timing, sample-rate or bit-depth check is written, because none would be reachable. This
is a planning decision, not an omission: the LOCKED block states "No separate timing check is needed".

Purpose: produce a Parallax delivery the pinned reference decoder accepts (Core Value / CONF-06), and
move the builder's static validation (API-01) onto the spec rule instead of a guess.

Output: new error kind, builder check with `// ref:` citations, 5 builder tests, repaired fixture,
updated contract assertion, deferred-items file.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@.planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-CONTEXT.md
@.planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-RESEARCH.md

<interfaces>
Extracted from the codebase. Use these directly; no further exploration is needed.

src/error.rs
- `#[non_exhaustive] pub enum ErrorKind` with compact unit variants for the builder, around lines
  132-145: `UnknownCodecConfigHandle`, `UnknownAudioElementHandle`, `UnknownSubstreamHandle`,
  `UnknownParameterHandle`, `InvalidDescriptorReference`, `DuplicateDeclaration`,
  `WireIdAllocationExhausted`. Each carries an `#[error("...")]` message.
- `pub struct Error { kind: ErrorKind, at: Location }`, built with `pub const fn new(kind: ErrorKind, at: Location) -> Self`.
- The size budget is `const _: () = assert!(size_of::<Error>() <= 32);` (line 240). A unit variant does not change the size.

src/encoder.rs
- `pub fn add_codec_config(&mut self, config: CodecConfig) -> CodecConfigHandle` (line 542). Its doc
  comment is a single line, "Declare a codec configuration."
- `pub fn build(mut self) -> Result<(Encoder, IdManifest)>` (line 688). It calls
  `self.resolve_parameter_references()?` then `self.validate_declarations()?`.
- `fn validate_declarations(&self) -> Result<()>` (line 805). The per-config loop is at lines 817-828:
  `for config in &self.codec_configs { ...codec_id match...; validate_findings(config.validate())?; }`.
  The audio-element loop follows at line 829.
- `fn invalid(field: &'static str) -> Error`, which gives `InvalidDescriptorReference` at `Field(field)` (line 1106).
- Existing ref style (`src/model/profile.rs:135`): `// ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc ProfileFilter::FilterProfilesForAudioElement`

tests/encoder_builder.rs helpers
- `lpcm_config()` (line 236) returns `CodecConfig::lpcm(42, 128, { LittleEndian, 16, 16_000 })`.
- `stereo_element()` (line 269) returns an `AudioElement`, id 99, one substream.
- `add_fresh_element(&mut builder, codec_handle, element) -> AudioElementHandle` (line 724).
- `presentation_for_elements(&[element ids], element_param_id, output_param_id) -> MixPresentation` (line 777).
- Pattern to copy: `largest_presentation_sets_the_sequence_profile` (line 215). It adds 3 fresh
  elements plus one more, and two presentations built with `presentation_for_elements(&[99,100,101],100,110)`
  and `(&[102],200,210)`.
- `CodecConfig::opus(id, 960, 48_000, 312)?` and `CodecConfig::flac(id, 128, rate, 16)?` return
  `iamf::Result<CodecConfig>`, as in `tests/encoder_streaming.rs:153-154`.
- Location assertion style: `assert_eq!(error.at(), Location::Field("parameter_id"));` (line 181).

tests/error_shape.rs
- `static_builder_errors_are_compact_typed_kinds` (line 167) holds the array of builder kinds that the
  new variant joins.

tests/support/parallax_contract.rs (the fixture; lines 98-195)
- Line 99: `let lpcm = builder.add_codec_config(CodecConfig::lpcm(88, 128, {LittleEndian, 16, 48_000}))`.
- Lines 108-109: the FLAC config (id 89) and the Opus config (id 90), bound to `flac` and `opus`.
- Lines 117-119: `CandidateKind::LpcmStereo`, `FlacStereo` and `OpusStereo` map to
  `builder.add_audio_element(lpcm|flac|opus, stereo_element(10|20|30))`.
- Lines 178-182: a corpus comment, then two committed packet includes bound to `flac_payload` and `opus_payload`.
- Lines 183-187: `frames = vec![(lpcm_stream, FrameInput::Lpcm(vec![0; 512])), (flac_stream, FrameInput::Flac(flac_payload)), (opus_stream, FrameInput::Opus(opus_payload))]`.
- Ambisonics frames use `FrameInput::Lpcm(vec![0; 256])`. The parameter block duration is 128 with subblocks 64+64.

tests/parallax_contract.rs lines 193-197: `assert_eq!(codec_ids, vec![0, 1, 2], "LPCM, FLAC, Opus declaration order");`

tests/conformance.rs lines 1729-1745: `parallax_delivery_fixture_uses_the_offline_safe_reference_gates`.
- If `iamf_tools_available()` is false it prints `SKIP parallax_delivery_fixture_uses_the_offline_safe_reference_gates: ...` and returns, so the test passes.
- On success it prints `[parallax delivery] CONF-06: <observed>`.
- No edit is needed in this file.
</interfaces>
</context>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1 (tracer): reject a second Codec Config end-to-end and repair the Parallax fixture so the pinned decoder accepts it</name>
  <files>src/error.rs, src/encoder.rs, tests/encoder_builder.rs, tests/support/parallax_contract.rs, tests/parallax_contract.rs</files>
  <behavior>
    - The review's case is rejected. A builder declares `lpcm_config()` (128 samples per frame) and `CodecConfig::opus(90, 960, 48_000, 312)?`. It adds one fresh `stereo_element()` on each handle and puts both elements in a single sub-mix via `presentation_for_elements(&[99, 100], 100, 110)`. `build()` returns an error whose `kind()` is `&ErrorKind::MultipleCodecConfigs` and whose `at()` is `Location::Field("codec_configs")`.
    - `parallax_contract::build_delivery()` succeeds. The parsed Codec Config ids are exactly `[0]`, the element ids stay `[0, 1, 2, 3]`, the presentations stay `[0, 1]`, and there is 1 parameter block.
    - The pinned `decoder_main` decodes the delivery (CONF-06), reporting `Decoded 1 temporal units.`
  </behavior>
  <action>
  RED first. Add the test `build_rejects_codec_configs_with_mismatched_frame_timing` to
  `tests/encoder_builder.rs`, exactly as in the first `<behavior>` bullet. Make the test fn return
  `iamf::Result<()>` so the Opus constructor can use `?`. Run it and confirm it fails: the variant
  does not exist yet, so it fails to compile.

  GREEN, in `src/error.rs`. Add a unit variant `MultipleCodecConfigs` to `ErrorKind`, placed directly
  after `WireIdAllocationExhausted`, with the message `#[error("an IA sequence permits only one Codec Config")]`.
  - It carries no payload, per the LOCKED decision. RESEARCH measured that a variant carrying two
    `u32` values makes `Error` 40 bytes and breaks the 32-byte const assertion.
  - Do not touch the size assertion.

  GREEN, in `src/encoder.rs` `validate_declarations`. Immediately after the closing brace of the
  per-config loop (`for config in &self.codec_configs`) and before the audio-element loop, add:
  if `self.codec_configs.len() > 1`, return `Err(Error::new(ErrorKind::MultipleCodecConfigs, Location::Field("codec_configs")))`.
  - Place it after the loop so that a malformed single config keeps reporting its own `codec_id` or
    `decoder_config` error first, as the existing tests at `tests/encoder_builder.rs:249-266` expect.
  - Above the check, write a short explanatory comment and these `// ref:` lines, verbatim in this form:
    `// ref: IAMF v1.1.0 index.bs:1912 "There SHALL be only one unique Codec Config OBU"`,
    `// ref: iamf-tools@v2.1.0 iamf/cli/obu_processor.cc GetSampleRateAndFrameSize`,
    `// ref: iamf-tools@v2.1.0 iamf/cli/rendering_mix_presentation_finalizer.cc GetCommonCodecConfigPropertiesFromAudioElementIds`,
    `// ref: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc FillDescriptorStatistics`.
  - The comment must say two things:
    - An Audio Element's frame size, sample rate and bit depth come only from its Codec Config, so
      this single check also enforces the reference's per-sub-mix frame-size rule and its
      sample-rate/bit-depth equality rule.
    - The check is whole-sequence, not per sub-mix, because `decoder_main` fails on two configs even
      with equal timing.
  - Use no indexing and no bare arithmetic: `len() > 1` is a comparison and is clippy-clean under
    `indexing_slicing` and `arithmetic_side_effects`.
  - Do NOT add this rule to `SequenceWriter`, `write_sequence`, `write_parsed_sequence` or
    `DescriptorSet::validate()`. Those low-level paths must round-trip foreign files byte-exactly.
    This is the discretion choice recorded in RESEARCH, and it is a deferred item.
  - No runtime guard in `EncodingWriter` either: `Encoder` is only constructed inside `build()`.

  Fixture repair in `tests/support/parallax_contract.rs`, per the LOCKED fixture decision. All four
  Audio Elements share the one LPCM Codec Config: id 88, 16-bit LE, 48 kHz, 128 samples per frame.
  - Delete the two `add_codec_config` declarations for the FLAC (id 89) and Opus (id 90) configs, and
    their `flac` / `opus` bindings.
  - Map `CandidateKind::FlacStereo` and `CandidateKind::OpusStereo` to `builder.add_audio_element(lpcm, stereo_element(20))`
    and `builder.add_audio_element(lpcm, stereo_element(30))`.
  - Delete the corpus comment and the two committed-packet include lines, together with the
    `flac_payload` / `opus_payload` bindings.
  - Change the `flac_stream` and `opus_stream` frame entries to `FrameInput::Lpcm(vec![0; 512])`
    (128 samples x 2 channels x 2 bytes, the same as the `lpcm_stream` entry).
  - Keep the frame order, the ambisonics `vec![0; 256]` frames, the parameter block (duration 128,
    subblocks 64+64), both presentations, and all candidate names. The names `flac archive` and
    `opus stream` are locked.
  - Keep the `CandidateKind` variants. They are still constructed, so there is no dead-code lint.
  - Add a comment above the single `add_codec_config`. It says that IAMF v1.1.0 permits one Codec
    Config per IA sequence, so the FLAC and Opus candidates lower onto the shared LPCM config here. It
    also says separate per-codec Parallax deliveries are recorded in `260913-lta-deferred-items.md`.
  - The comment text must not contain the FLAC/Opus constructor call syntax, and must not contain
    the include macro name.
  - Remove any import that becomes unused. `CodecConfig` is still used.

  In `tests/parallax_contract.rs` lines 193-197, change the expected value to `vec![0]` and the
  message to `"one Codec Config per IA sequence (IAMF v1.1.0 index.bs:1912)"`. Leave every other
  assertion unchanged.

  Do not edit `tests/conformance.rs`, the golden fixtures, `DIFF-LEDGER.md`, `README.md` or anything
  under `docs/`.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo test --locked --test encoder_builder build_rejects_codec_configs_with_mismatched_frame_timing && cargo test --locked --test parallax_contract && mkdir -p target && (cargo test --locked --test conformance parallax_delivery_fixture_uses_the_offline_safe_reference_gates -- --nocapture > target/lta-conf06.log 2>&1; true) && grep -q 'test parallax_delivery_fixture_uses_the_offline_safe_reference_gates ... ok' target/lta-conf06.log && grep -q 'CONF-06: decoder_main reported' target/lta-conf06.log && test "$(grep -v '^[[:space:]]*//' tests/support/parallax_contract.rs | grep -cE 'CodecConfig::(flac|opus)|include_bytes!')" -eq 0 && grep -q 'index.bs:1912' src/encoder.rs && grep -q 'obu_processor.cc GetSampleRateAndFrameSize' src/encoder.rs && grep -q 'rendering_mix_presentation_finalizer.cc GetCommonCodecConfigPropertiesFromAudioElementIds' src/encoder.rs && grep -q 'obu_sequencer_base.cc FillDescriptorStatistics' src/encoder.rs</automated>
  </verify>
  <done>
  - The mismatched-timing builder test passes, with the error kind and location asserted exactly.
  - `tests/parallax_contract.rs` passes with codec ids `[0]`.
  - `target/lta-conf06.log` shows `CONF-06: decoder_main reported` and the test `... ok`.
  - The fixture contains no FLAC/Opus Codec Config constructors and no packet includes.
  - All four `// ref:` citations are present.
  - If the log shows `SKIP parallax_delivery_fixture_uses_the_offline_safe_reference_gates` instead,
    the executor stops and reports CONF-06 as SKIPPED (container unavailable), never as passed.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: expand rejection coverage, prove the positive case, register the kind, document the rule</name>
  <files>tests/encoder_builder.rs, tests/error_shape.rs, src/encoder.rs</files>
  <behavior>
    - `build_rejects_two_codec_configs_with_identical_timing`. Two separate `lpcm_config()` declarations each get one fresh `stereo_element()`, and both elements sit in one sub-mix (`presentation_for_elements(&[99, 100], 100, 110)`). Result: `MultipleCodecConfigs` at `Field("codec_configs")`. This matches the executed `decoder_main` failure on two configs with equal timing.
    - `build_rejects_codec_configs_split_across_mix_presentations`. Config A's element is in presentation `(&[99], 100, 110)` and config B's element is in presentation `(&[100], 200, 210)`. Result: the same error, because the scope is the whole IA sequence.
    - `build_rejects_a_declared_but_unreferenced_second_codec_config`. One config feeds one element in one presentation (`(&[99], 100, 110)`), and a second `lpcm_config()` is declared but referenced by nothing. Result: the same error.
    - `one_codec_config_shared_by_four_elements_in_one_sub_mix_builds`. One `lpcm_config()` feeds four fresh `stereo_element()`s in one presentation (`(&[99, 100, 101, 102], 100, 110)`). `build()` returns Ok, `manifest.sequence_profile() == Profile::BaseEnhanced`, and `encoder.descriptors().codec_configs.len() == 1`.
    - `static_builder_errors_are_compact_typed_kinds` in `tests/error_shape.rs` includes `ErrorKind::MultipleCodecConfigs`, and the size assertion still holds.
  </behavior>
  <action>
  Add the four tests from `<behavior>` to `tests/encoder_builder.rs`.
  - Build each one with the existing helpers `lpcm_config`, `stereo_element`, `add_fresh_element` and
    `presentation_for_elements`, following the shape of `largest_presentation_sets_the_sequence_profile`.
  - Each rejection test asserts both `error.kind() == &ErrorKind::MultipleCodecConfigs` and
    `error.at() == Location::Field("codec_configs")`.
  - Tests may use `unwrap`/`expect`; the lint carve-out covers tests. Follow the style already used in
    that file.

  In `tests/error_shape.rs`, append `ErrorKind::MultipleCodecConfigs` to the kind array in
  `static_builder_errors_are_compact_typed_kinds`.

  In `src/encoder.rs`, extend the doc comments. Both edits document the rule from D-LOCKED strict
  single Codec Config:
  - On `add_codec_config`: an IA sequence carries exactly one Codec Config (IAMF v1.1.0
    `index.bs:1912`); every Audio Element must share it; `build()` rejects a builder that declared more
    than one with [`ErrorKind::MultipleCodecConfigs`]. `ErrorKind` is already imported there, so the
    intra-doc link resolves.
  - On `build`: its validation includes the single-Codec-Config rule.
  - No behaviour change in this task.

  Do not write any separate sample-rate or bit-depth test. With one Codec Config those rules cannot be
  reached through the builder (RESEARCH "Tests to add").
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo test --locked --test encoder_builder && cargo test --locked --test error_shape && test "$(grep -c 'ErrorKind::MultipleCodecConfigs' tests/encoder_builder.rs)" -ge 4 && grep -q 'ErrorKind::MultipleCodecConfigs' tests/error_shape.rs && grep -q 'one_codec_config_shared_by_four_elements_in_one_sub_mix_builds' tests/encoder_builder.rs && cargo test --locked --doc</automated>
  </verify>
  <done>
  - All 5 new builder tests pass: 4 from this task plus Task 1's.
  - `error_shape` passes with the new kind registered.
  - The doc comments state the rule, and doc tests pass.
  </done>
</task>

<task type="auto">
  <name>Task 3: record deferred follow-ups and run the full release gate</name>
  <files>.planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md</files>
  <action>
  Create `260913-lta-deferred-items.md` in the task directory, headed `# Deferred items — quick 260913-lta`.
  Record these follow-ups, and implement none of them. Each entry gives its source citation and why it
  is out of scope.
  - FLAC and Opus Parallax deliveries, each as its own IA sequence with one Codec Config.
    - Note that builder-level FLAC/Opus framing is still exercised by the single-config builders at
      `tests/encoder_streaming.rs:153-154`.
    - Note that API-09's contract-fixture codec coverage is what the follow-up restores.
  - Spec `index.bs:1915-1916`: every Parameter Block duration equals the Audio Frame duration.
    Temporal preflight currently checks only non-zero.
  - Spec `index.bs:1913`: every Audio Substream carries the same trimming information. This relates
    to the review's trim-position finding in `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md`.
  - A `DescriptorSet::validate()` finding for more than one Codec Config on the low-level model.
    Check `tests/descriptors.rs:1313` first, which pushes a duplicate config. Low-level writers must
    keep round-tripping foreign files byte-exactly.
  - The review's profile selection for expanded layouts. It is noted only, and unchanged here.
  - Informational, upstream: `libiamf@f06e919e` `code/src/iamf_dec/IAMF_decoder.c:2261`.
    - Its growth loop tests `i` instead of `n`. [INFERRED] With unequal frame sizes this can index
      past `DEC_BUF_CNT`.
    - This is one more reason `decoder_main`, not `libiamf`, is the gate.
  - A disagreement note: Eclipsa and `iamf-tools@v2.1.0` agree (Eclipsa always emits one Codec Config,
    id 200). Nothing to reconcile.
  - A resolution note: this task resolves the CONF-06 item deferred in
    `.planning/quick/260913-js8-fix-the-count-field-preallocation-memory/deferred-items.md`. Do not
    edit that file.

  Then run the full gate in the verify command below. If any existing test outside this task's files
  fails, check whether it declares more than one Codec Config through `EncoderBuilder`.
  - Planning grep found none outside the fixture: every builder in `tests/encoder_builder.rs`,
    `tests/encoder_streaming.rs`, `tests/parallax_contract.rs` and `src/encoder.rs` unit tests declares
    exactly one.
  - If one appears, collapse it to one config, or convert it into a rejection test, and record it in
    the SUMMARY.

  Confirm that `git status --porcelain` shows no change to `DIFF-LEDGER.md`, `tests/golden.rs` or
  `tests/fixtures/`.

  Commit with explicit paths only. Never stage `README.md` (an unrelated pre-existing user edit) or
  the untracked `docs/` clones.

  In the SUMMARY, quote the `[parallax delivery] CONF-06:` line from `target/lta-conf06.log`
  verbatim. If the container was unavailable, state SKIPPED.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && test -f .planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md && grep -q 'index.bs:1915' .planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md && grep -q 'index.bs:1913' .planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-deferred-items.md && cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && cargo test --locked && cargo test --locked --features fuzzing --test fuzz_regression && test -z "$(git status --porcelain -- DIFF-LEDGER.md tests/golden.rs tests/fixtures)"</automated>
  </verify>
  <done>
  - The deferred-items file exists with all entries.
  - `cargo fmt --check`, both clippy runs and the full `cargo test --locked` pass (golden,
    DIFF-LEDGER, parallax_contract and conformance included), as does the fuzz corpus replay.
  - No golden fixture or ledger changes.
  - The SUMMARY quotes the CONF-06 decoder_main line, or states SKIPPED honestly.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| caller declarations → `EncoderBuilder::build()` | Caller-authored static configuration is validated before an `Encoder` exists |
| crate output → external decoders (`decoder_main`, `libiamf`) | Emitted bytes must meet the spec rules the reference enforces; `libiamf` is permissive and has a latent OOB defect on unequal frame sizes |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-lta-01 | Tampering | `EncoderBuilder::build()` accepting a multi-Codec-Config declaration | medium | mitigate | `codec_configs.len() > 1` returns typed `MultipleCodecConfigs` before id allocation (Task 1). 4 rejection tests cover mismatched timing, identical timing, cross-presentation and unreferenced configs (Tasks 1-2) |
| T-lta-02 | Denial of Service | Downstream `libiamf` buffer growth loop (`IAMF_decoder.c:2261`) on unequal frame sizes | medium | mitigate | The builder can no longer emit unequal frame sizes, because one Codec Config fixes the timing. The upstream defect is recorded in the deferred items as informational |
| T-lta-03 | Denial of Service | New check panicking on hostile input | low | accept | A pure length comparison: no indexing, no arithmetic, no allocation. Enforced by clippy `indexing_slicing`/`arithmetic_side_effects` in the Task 3 gate |
| T-lta-04 | Information Disclosure | `Error` size or payload growth | low | accept | Unit variant; the `size_of::<Error>() <= 32` const assertion is unchanged and exercised in `tests/error_shape.rs` |
</threat_model>

<verification>
- `cargo test --locked --test encoder_builder` passes, with 5 new tests (4 rejections plus 1 positive).
- `cargo test --locked --test parallax_contract` asserts codec ids `[0]`.
- `cargo test --locked --test conformance parallax_delivery_fixture_uses_the_offline_safe_reference_gates -- --nocapture`
  prints `CONF-06: decoder_main reported`, or is reported as SKIPPED.
- The full gate passes: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`
  (with and without `--features fuzzing`), `cargo test --locked`, and the fuzz regression replay.
- Golden fixtures, `DIFF-LEDGER.md`, `README.md` and `docs/` are untouched.
</verification>

<success_criteria>
- A builder with more than one Codec Config fails `build()` with `ErrorKind::MultipleCodecConfigs` at
  `Field("codec_configs")`, and the check cites spec `index.bs:1912` plus three `iamf-tools@v2.1.0`
  sites.
- Matching timing (one shared config, four elements, one sub-mix) builds as Base-Enhanced.
- The Parallax delivery fixture passes CONF-06 against the pinned `decoder_main`, with evidence quoted.
- Deferred follow-ups are recorded in `260913-lta-deferred-items.md`, and none of them is implemented.
</success_criteria>

<output>
Create `.planning/quick/260913-lta-reject-mismatched-codec-frame-timing-in-/260913-lta-SUMMARY.md` when done.
</output>
