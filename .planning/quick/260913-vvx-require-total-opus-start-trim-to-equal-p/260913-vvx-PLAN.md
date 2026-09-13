---
phase: quick-260913-vvx
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/error.rs
  - src/encoder.rs
  - tests/encoder_streaming.rs
  - tests/error_shape.rs
  - .planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-deferred-items.md
autonomous: true
requirements: [API-06, API-08, CONF-06]

estimate:
  tokens: 85000
  raw_tokens: 85000
  tasks: 3
  confidence: low

must_haves:
  truths:
    - "Per the locked user decision (exactly equal): for an Opus Codec Config, EncodingWriter accepts a stream only when the sum of num_samples_to_trim_at_start over the leading start-trim run equals OpusDecoderConfig.pre_skip"
    - "A push whose running start-trim total would exceed pre_skip (in one unit or across fully trimmed units, including u32 overflow) fails with ErrorKind::OpusStartTrimExceedsPreSkip at Location::Field(\"trimming\")"
    - "A push that closes the start-trim run (at_start < num_samples_per_frame, including no trimming at all) with a total different from pre_skip fails with ErrorKind::OpusStartTrimShortOfPreSkip at Location::Field(\"trimming\")"
    - "finish() on an Opus writer whose start-trim run is still open with a total below pre_skip, including an empty stream with zero temporal units (assumption A1 accepted), fails with ErrorKind::OpusStartTrimShortOfPreSkip at Location::Field(\"trimming\"); a poisoned writer still reports its poisoned error first"
    - "A rejected push writes no bytes (bytes_written() unchanged) and commits no start_trim_total; the next valid unit is judged against the last successful push"
    - "Accepted: N=960 P=312 with 312 then untrimmed units; N=120 P=312 with 120, 120, 72 then untrimmed; N=P=120 with 120 then untrimmed, and 120 then finish(). Finished bytes parse with parse_sequence and carry the submitted trims"
    - "LPCM and FLAC writers are unaffected: untrimmed units and empty streams still finish"
    - "size_of::<Error>() stays <= 32. SequenceWriter, src/obu, tests/support, tests/fixtures, golden, DIFF-LEDGER.md and conformance code are unchanged, and the conformance suite with IAMF_REF_DECODER ends test result: ok with CONF-06 lines for phase3_flac and phase3_opus"
  artifacts:
    - path: "src/error.rs"
      provides: "payload-free ErrorKind::OpusStartTrimExceedsPreSkip and ErrorKind::OpusStartTrimShortOfPreSkip"
      contains: "OpusStartTrimShortOfPreSkip"
    - path: "src/encoder.rs"
      provides: "TemporalProgress.start_trim_total, opus_pre_skip(), the preflight equality check and the finish() open-run check"
      contains: "index.bs:1819"
    - path: "tests/encoder_streaming.rs"
      provides: "rejection, positive and codec-scope tests; the migrated Opus kind test trimming 312"
      contains: "OpusStartTrimExceedsPreSkip"
    - path: "tests/error_shape.rs"
      provides: "both new kinds in temporal_input_errors_are_compact_typed_kinds"
      contains: "OpusStartTrimExceedsPreSkip"
    - path: ".planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-deferred-items.md"
      provides: "deferred parse-side validate() finding with the three affected fixtures, decoder pre_skip evidence, reference disagreement, AAC-LC note"
  key_links:
    - from: "src/encoder.rs EncodingWriter::preflight"
      to: "TemporalProgress.start_trim_total"
      via: "new total computed into the returned TemporalProgress only; push_temporal_unit assigns self.progress after self.sequence.push_temporal_unit succeeds"
      pattern: "start_trim_total"
    - from: "src/encoder.rs EncodingWriter::finish"
      to: "ErrorKind::OpusStartTrimShortOfPreSkip"
      via: "check_not_poisoned first, then the open-run check, then self.sequence.finish()"
      pattern: "check_not_poisoned"
---

<objective>
Enforce IAMF v1.1.0 `index.bs:1819` in the high-level encoder: for an Opus Codec Config, the total number
of samples trimmed at the start of the coded Audio Substreams SHALL equal `pre_skip` (locked user decision
"ja, mit genau gleich umsetzen" — exactly equal, stricter than iamf-tools' `>=`).

Purpose: no pinned decoder reads `pre_skip` (research probes a-e), so a mismatch silently leaves priming
samples in, or removes real audio. `EncodingWriter` is the only guard.

Output: two payload-free `ErrorKind` variants; a `start_trim_total` in `TemporalProgress` checked in
`preflight` and `finish()`; migrated and new tests; a deferred-items record for the parse-side finding.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@.planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-CONTEXT.md
@.planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-RESEARCH.md

Interfaces the executor needs (verified this session; do not re-explore):

- `src/obu/header.rs`: `pub struct Trimming { pub at_end: u32, pub at_start: u32 }`;
  `pub enum TypeSpecific { Trimming(Option<Trimming>), Reserved }`; both re-exported from `iamf::obu`.
- `src/obu/codec_config.rs`: `CodecConfig { num_samples_per_frame: u32, decoder_config: DecoderConfig, .. }`;
  `pub const fn opus_config(&self) -> Option<&OpusDecoderConfig>`; `OpusDecoderConfig.pre_skip: u16`;
  `CodecConfig::opus(codec_config_id, num_samples_per_frame, sample_rate, pre_skip) -> Result<Self>`.
  `build()` rejects `pre_skip == 0` through `validate_findings`, so in the encoder path pre_skip >= 1.
- `src/encoder.rs`:
  - `TemporalProgress { parameter_ids: Option<Vec<u32>>, start_trim_open: bool, end_trimmed: bool }` with
    `const fn initial()` (around line 223-241).
  - `push_temporal_unit` runs `check_not_poisoned`, `preflight`, `self.sequence.push_temporal_unit(&unit)?`,
    then `self.progress = next` (around 260-266).
  - `pub fn finish(self) -> Result<W> { self.sequence.finish() }` (around 272-274).
  - In `preflight`, after the frames loop: `(at_start, at_end)` from `input.trimming`, T4 check
    (`TemporalUnitAfterEndTrim`), T3 check (`StartTrimAfterUntrimmedAudio`), then
    `let start_trim_open = self.progress.start_trim_open && Some(at_start) == self.frame_size();` and
    `let end_trimmed = at_end > 0;`, then `let parameter_definitions = ...` (around 322-339). The returned
    `TemporalProgress { parameter_ids, start_trim_open, end_trimmed }` is at the end (around 422-428).
  - `fn frame_size(&self) -> Option<u32>` reads `self.descriptors.codec_configs.first()` (around 433-438).
  - `fn temporal_input(kind: ErrorKind, field: &'static str) -> Error` (around 579).
  - `SequenceWriter::check_not_poisoned(&self) -> Result<()>` is `pub(crate)` in `src/sequence.rs:795`.
- `src/error.rs`: `#[non_exhaustive] pub enum ErrorKind`, last variants `StartTrimAfterUntrimmedAudio`,
  `TemporalUnitAfterEndTrim` (around 196-199); `const _: () = assert!(size_of::<Error>() <= 32);`.
- `tests/encoder_streaming.rs`: `mono_builder(config: CodecConfig) -> iamf::Result<(Encoder, (SubstreamHandle, FrameInput))>`
  (Opus frame input is `FrameInput::Opus(vec![0xf8])`, mix gains at 48 kHz, mode 1); `const fn start_trim(at_start: u32) -> Trimming`
  and `const fn end_trim(at_end: u32) -> Trimming` (around 678-690); `lpcm_flac_and_opus_inputs_follow_the_frozen_codec_kind`
  at 142-186 pushes one unit with `trimming: None` for all three codecs; the n56 trim tests at 563-640 show the
  assertion style (kind, `Location::Field("trimming")`, unchanged `bytes_written()`, a following valid push).
- `tests/error_shape.rs::temporal_input_errors_are_compact_typed_kinds` (around 193-215) lists temporal kinds.

Project constraints: Rust 1.85 toolchain (no let-chains; use nested `if let` or `match`), no
`unwrap`/`expect`/indexing/bare arithmetic in `src/` (clippy `indexing_slicing`, `arithmetic_side_effects`),
`thiserror` payload-free unit variants, `// ref:` citations on reference-derived rules.
</context>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: End-to-end Opus start-trim equality on the single-unit push path</name>
  <files>src/error.rs, src/encoder.rs, tests/error_shape.rs, tests/encoder_streaming.rs</files>
  <behavior>
    - N=960, P=312: one unit with trimming None fails OpusStartTrimShortOfPreSkip at Field("trimming"), bytes unchanged
    - N=960, P=312: one unit with start_trim(311) fails OpusStartTrimShortOfPreSkip, bytes unchanged
    - N=960, P=312: one unit with start_trim(313) fails OpusStartTrimExceedsPreSkip, bytes unchanged; the same writer then accepts start_trim(312) (a rejected unit commits no total)
    - N=960, P=312: start_trim(312), then None, then end_trim(100) is accepted, finish() succeeds, and parse_sequence yields three Audio Frames whose header.type_specific equals TypeSpecific::Trimming(Some(..)) with at_start 312, None (TypeSpecific::Trimming(None)), and at_end 100 respectively; before the end-trim unit, a push with start_trim(1) still fails StartTrimAfterUntrimmedAudio (n56 T3 keeps precedence once the run has closed)
    - The migrated kind test: LPCM and FLAC push trimming None, Opus pushes start_trim(312); all three finish and round-trip as before
    - Both new kinds appear in temporal_input_errors_are_compact_typed_kinds; size_of::<Error>() <= 32 still holds
  </behavior>
  <action>
    RED first: add the tests below to `tests/encoder_streaming.rs` and the two kinds to `tests/error_shape.rs`, run
    them and confirm they fail to compile or fail, then implement.

    `src/error.rs` (per the locked decision; payload-free, keeps the 32-byte assertion): append two unit variants
    directly after `TemporalUnitAfterEndTrim`:
    `OpusStartTrimExceedsPreSkip` with message "the total num_samples_to_trim_at_start exceeds the Opus pre_skip",
    and `OpusStartTrimShortOfPreSkip` with message "the start trim ended before reaching the Opus pre_skip".
    Give each a one-line doc comment only if neighbouring variants have them (match local style).

    `src/encoder.rs`:
    1. Add `start_trim_total: u32` to `TemporalProgress`, doc "Samples trimmed at the start across the leading
       start-trim run (tracked for Opus only; 48 kHz samples per index.bs:1825)", initialised to 0 in `initial()`.
    2. Add `fn opus_pre_skip(&self) -> Option<u16>` directly after `frame_size()`, doc "The frozen Opus pre_skip;
       `build()` guarantees exactly one Codec Config", implemented as `codec_configs.first()` then
       `and_then(CodecConfig::opus_config)` then map to `pre_skip`.
    3. In `preflight`, immediately after `let end_trimmed = at_end > 0;` and before `let parameter_definitions`,
       add the equality check with these comment citations (existing `// ref:` style):
       `// ref: IAMF v1.1.0 index.bs:1819 "Pre-skip SHALL be the same as the number of audio samples to be trimmed at the start of coded Audio Substreams"`,
       `// ref: IAMF v1.1.0 index.bs:541 (the start trim spans fully trimmed frames, then at most one remainder frame)`,
       `// ref: IAMF v1.1.0 index.bs:1825 (offsets are counted at 48 kHz)`,
       `// ref: iamf-tools@v2.1.0 iamf/cli/proto_conversion/proto_to_obu/audio_frame_generator.cc:141-156 ValidateUserStartTrimIncludesCodecDelay`,
       `// ref: eclipsa-audio-plugin@c9646092 common/processors/file_output/iamf_export_utils/IAMFExportUtil.cpp:124-130 (pre_skip 312, codec delay as the whole start trim)`,
       `// DISAGREEMENT: iamf-tools requires only trim >= codec delay; the spec requires equality (user decision 260913-vvx).`
       and a note that neither pinned decoder reads pre_skip (research 260913-vvx probes a-e), so this crate is the only guard.
       Logic: start from `self.progress.start_trim_total`. Only when `self.opus_pre_skip()` is `Some(pre_skip)` AND
       `self.progress.start_trim_open` is true: compute `checked_add(at_start)`; if the add overflows or the new total
       is greater than `u32::from(pre_skip)`, return `temporal_input(ErrorKind::OpusStartTrimExceedsPreSkip, "trimming")`;
       then, if the freshly computed `start_trim_open` is false (this unit closes the run) and the new total differs
       from `u32::from(pre_skip)`, return `temporal_input(ErrorKind::OpusStartTrimShortOfPreSkip, "trimming")`.
       When the run is already closed, T3 above already rejects any later non-zero start trim, so the total stays frozen.
       Use nested `if let`/`match` (Rust 1.85: no let-chains). No bare `+`.
    4. Put the computed `start_trim_total` into the returned `TemporalProgress` only. Do not touch `self.progress`
       in `preflight`; `push_temporal_unit` already commits `next` after a successful sink push (n56 convention).
    5. Update the doc comment on `TemporalUnitInput::trimming` to add: for an Opus Codec Config, the start trims of
       the leading run must add up to exactly the Codec Config's `pre_skip`. Extend the "Cross-unit rules" sentence
       in `push_temporal_unit`'s doc to include the Opus pre_skip total.

    Do NOT change `finish()` in this task (Task 2), `src/sequence.rs`, `src/obu`, or any fixture.

    `tests/error_shape.rs`: append `ErrorKind::OpusStartTrimExceedsPreSkip` and `ErrorKind::OpusStartTrimShortOfPreSkip`
    to the array in `temporal_input_errors_are_compact_typed_kinds`.

    `tests/encoder_streaming.rs`:
    - Migrate `lpcm_flac_and_opus_inputs_follow_the_frozen_codec_kind` (per orchestrator decision): pair each builder with
      its trim — LPCM `None`, FLAC `None`, Opus `Some(start_trim(312))` — and push that per iteration. Keep every existing
      assertion. Add a short comment citing index.bs:1819 for the Opus trim.
    - Add a local helper `fn opus_unit(handle: SubstreamHandle, trimming: Option<Trimming>) -> TemporalUnitInput`
      (frames `vec![(handle, FrameInput::Opus(vec![0xf8]))]`, no parameter blocks), and a helper
      `fn opus_writer(num_samples_per_frame: u32, pre_skip: u16)` that returns the started writer and the handle via
      `mono_builder(CodecConfig::opus(0, num_samples_per_frame, 48_000, pre_skip)?)` and `encoder.start(Vec::new())`.
      Import `TypeSpecific` from `iamf::obu` if needed.
    - Add tests named exactly: `opus_unit_without_a_start_trim_is_rejected`, `opus_start_trim_short_of_pre_skip_is_rejected`,
      `opus_start_trim_exceeding_pre_skip_in_one_unit_is_rejected`, `opus_start_trim_equal_to_pre_skip_round_trips`,
      implementing the behaviours above. Each rejection asserts kind, `Location::Field("trimming")`, and unchanged
      `bytes_written()`.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo test --locked --test encoder_streaming && cargo test --locked --test error_shape && for t in opus_unit_without_a_start_trim_is_rejected opus_start_trim_short_of_pre_skip_is_rejected opus_start_trim_exceeding_pre_skip_in_one_unit_is_rejected opus_start_trim_equal_to_pre_skip_round_trips; do grep -q "fn $t" tests/encoder_streaming.rs || exit 1; done && grep -q 'start_trim(312)' tests/encoder_streaming.rs && grep -q 'start_trim_total' src/encoder.rs && grep -q 'index.bs:1819' src/encoder.rs && grep -q 'index.bs:1825' src/encoder.rs && grep -q 'ValidateUserStartTrimIncludesCodecDelay' src/encoder.rs && grep -q 'IAMFExportUtil.cpp' src/encoder.rs && grep -q 'ErrorKind::OpusStartTrimShortOfPreSkip' tests/error_shape.rs && grep -q 'ErrorKind::OpusStartTrimExceedsPreSkip' tests/error_shape.rs && cargo test --locked --lib && cargo clippy --locked --all-targets -- -D warnings</automated>
  </verify>
  <done>Both kinds exist and are listed in error_shape. The single-unit Opus rejections and the exact-312 round trip pass, the migrated kind test trims 312 for Opus only, clippy is clean, and the change is committed by explicit path as `feat(quick-260913-vvx): require the Opus start trim of a temporal unit run to equal pre_skip`.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Multi-unit start-trim runs and the finish() open-run check</name>
  <files>src/encoder.rs, tests/encoder_streaming.rs</files>
  <behavior>
    - N=120, P=312: start_trim(120), start_trim(120) accepted; start_trim(120) fails OpusStartTrimExceedsPreSkip with bytes unchanged; then start_trim(72) accepted, None accepted, finish() succeeds
    - N=120, P=312: start_trim(120), start_trim(120), then start_trim(71) fails OpusStartTrimShortOfPreSkip with bytes unchanged; then start_trim(72) accepted
    - N=120, P=312: 120, 120, 72, None accepted; parse_sequence yields four Audio Frames whose trims are at_start 120, 120, 72 and TypeSpecific::Trimming(None)
    - N=P=120: start_trim(120) then None accepted and finish() succeeds; a second writer with only start_trim(120) then finish() succeeds
    - Opus N=960 P=312 with zero temporal units: finish() fails OpusStartTrimShortOfPreSkip at Field("trimming") (assumption A1 accepted)
    - N=120, P=312: start_trim(120), start_trim(120), then finish() fails OpusStartTrimShortOfPreSkip at Field("trimming")
    - LPCM (lpcm_config via mono_builder) and FLAC (CodecConfig::flac(0, 128, 16_000, 16)): an empty stream finishes, and a stream of one untrimmed unit finishes
    - partial_sink_failure_poisoned_high_level_writer still passes unchanged (poisoning keeps priority in finish)
  </behavior>
  <action>
    RED first: add the tests below and confirm the `finish()` ones fail (the push-time ones may already pass after
    Task 1; that is expected and they stay as regression coverage of cross-unit accumulation).

    `src/encoder.rs` `finish(self)` (per orchestrator decision: poison check first, then the open run):
    1. Call `self.sequence.check_not_poisoned()?` first so a poisoned writer keeps reporting its poisoned error.
    2. If `self.opus_pre_skip()` is `Some(pre_skip)`, `self.progress.start_trim_open` is true, and
       `self.progress.start_trim_total != u32::from(pre_skip)`, return
       `Err(temporal_input(ErrorKind::OpusStartTrimShortOfPreSkip, "trimming"))`. A closed run was already judged
       at push time, so only an open run is checked here. This rejects an empty Opus stream (total 0, pre_skip >= 1)
       and a stream whose every unit is fully trimmed but falls short; an all-trimmed stream whose total equals
       pre_skip finishes.
    3. Otherwise return `self.sequence.finish()`.
    Add a `// ref: IAMF v1.1.0 index.bs:1819` citation and a comment that the sink is dropped on this error, as it
    already is for sink and poisoned errors. Extend the `finish` doc comment: for an Opus Codec Config, finishing
    while the start-trim run is still open and short of `pre_skip` (including a stream with no temporal units)
    fails with `ErrorKind::OpusStartTrimShortOfPreSkip`. Use nested `if let` (no let-chains).

    `tests/encoder_streaming.rs` — add tests named exactly (reuse Task 1's `opus_unit`/`opus_writer` helpers and the
    existing `start_trim`/`end_trim`/`mono_builder`/`lpcm_config` helpers):
    `opus_start_trim_exceeding_pre_skip_across_units_is_rejected`,
    `opus_start_trim_run_closing_short_of_pre_skip_is_rejected`,
    `opus_start_trim_spanning_units_round_trips`,
    `opus_pre_skip_equal_to_the_frame_size_is_accepted`,
    `finish_rejects_an_empty_opus_stream`,
    `finish_rejects_an_open_opus_start_trim_run_short_of_pre_skip`,
    `lpcm_and_flac_streams_need_no_start_trim`.
    Every rejection asserts kind and `Location::Field("trimming")`; push rejections also assert unchanged
    `bytes_written()` and then push the valid unit the behaviour lists. `finish()` consumes the writer, so the
    finish-rejection tests assert only kind and location (use `expect_err` with a message, as existing tests do).
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo test --locked --test encoder_streaming && for t in opus_start_trim_exceeding_pre_skip_across_units_is_rejected opus_start_trim_run_closing_short_of_pre_skip_is_rejected opus_start_trim_spanning_units_round_trips opus_pre_skip_equal_to_the_frame_size_is_accepted finish_rejects_an_empty_opus_stream finish_rejects_an_open_opus_start_trim_run_short_of_pre_skip lpcm_and_flac_streams_need_no_start_trim partial_sink_failure_poisoned_high_level_writer; do grep -q "fn $t" tests/encoder_streaming.rs || exit 1; done && grep -q 'check_not_poisoned()?' src/encoder.rs && cargo test --locked --test parallax_contract && cargo test --locked --test encoder_builder && cargo clippy --locked --all-targets -- -D warnings</automated>
  </verify>
  <done>finish() rejects an open, short Opus run (including an empty stream) after the poison check; multi-unit accumulation, exact spanning runs, P == N and LPCM/FLAC scope are covered by named tests; the streaming, builder and Parallax-contract tests pass; clippy is clean; committed by explicit path as `feat(quick-260913-vvx): reject finishing an Opus stream whose start trim is still short of pre_skip`.</done>
</task>

<task type="auto">
  <name>Task 3: Record deferred items and run every release gate including conformance</name>
  <files>.planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-deferred-items.md</files>
  <action>
    Write `260913-vvx-deferred-items.md` in the style of `260913-n56-deferred-items.md` (heading, one sentence that
    nothing listed is implemented, then numbered sections each with Source / Current state / Why out of scope):
    1. **Parse-side `validate()` finding for Opus start trim vs `pre_skip`.** Source: index.bs:1819 and research E.
       Current state: only `EncodingWriter` enforces it. Why deferred: a finding would change `semantic_sha256`
       (`tests/parse_reference.rs:91-99` hashes the findings) of `iamf-tools/noise_1024samp_5p1_opus.iamf`
       (pre_skip 312, total start trim 0, end 584), `iamf-tools/noise_3s_stereo_opus.iamf` (pre_skip 120, total 0,
       end 96) and `iamf-tools/tones_100ms_3OA_stereo_opus.iamf` (pre_skip 312, total 0, end 648). None is marked
       `is_valid: false` upstream, and pinned `decoder_main` decodes them (probe a: 1336 frames = 1024 + 312 priming).
       `test_000059` (312/312, valid) and `test_000124` (312/312, `is_valid: false`) are equal and unaffected. The
       finding also needs temporal-unit grouping and substream-run tracking in `validate()`, which is n56 deferred item 2.
       These three files stay permanent fuzz seeds; replay is unaffected.
    2. **Neither pinned decoder reads `pre_skip`.** libiamf@f06e919e: the Opus vtable has no `.info`, so `ctx->delay`
       stays 0 (`IAMF_opus_decoder.c:140-146`, `IAMF_core_decoder.c:145,283-287`); `pre_skip` is only logged
       (`vlogging_tool_sr.c:461`). iamf-tools@848c6ff4 `opus_decoder.cc:41-49` checks only `output_gain` and
       `mapping_family`. Probe b-e: `test_000059.iamf` with `pre_skip` patched to 0/100/312/624 decodes to the same
       WAV sha256 `f556119f…ac13`. Consequence: conformance (CONF-05/CONF-06) cannot catch a regression of this rule;
       the `tests/encoder_streaming.rs` tests are the guard. An executed `EncodingWriter` Opus multi-unit CONF-06
       probe was therefore not run.
    3. **Reference disagreement.** iamf-tools@v2.1.0 `audio_frame_generator.cc:141-156` requires only `>=`; the spec
       and Eclipsa (`IAMFExportUtil.cpp:124-130`, `AudioElement.cpp:179-181`) give equality. Recorded as a
       DISAGREEMENT comment in `src/encoder.rs`; the crate follows the spec per the user decision.
    4. **AAC-LC and other codecs.** index.bs:1827-1842 has no pre-skip or trim-equality rule, and the crate has no
       AAC `DecoderConfig`; FLAC and LPCM have no `pre_skip`. Nothing to implement.
    5. **Empty Opus stream (assumption A1).** Accepted strict reading: an Opus writer finished with zero temporal
       units is rejected with `OpusStartTrimShortOfPreSkip`. If a descriptors-only Opus export is ever needed, revisit.
    Close with: this task resolves `260913-n56-deferred-items.md` item 1.

    Commit the deferred-items file by explicit path (`docs(quick-260913-vvx): record deferred Opus pre_skip items`).
    Never `git add -A`; the untracked `docs/` clones must stay untracked.

    Then run the gates, all from /Users/cell/local/iamf-rs, with logs under `target/`:
    1. `cargo fmt --all -- --check`
    2. `cargo clippy --locked --all-targets -- -D warnings`
    3. `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`
    4. `env -u IAMF_REF_DECODER cargo test --locked` to `target/vvx-offline.log`, appending `offline_exit=<code>`
    5. `cargo test --locked --doc`
    6. `cargo test --locked --features fuzzing --test fuzz_regression` (fuzz replay)
    Conformance, bounded (there is no `timeout` binary; perl is `/usr/bin/perl`). Wrapper W:
    `perl -e 'my $s=shift; my $p=fork; if(!$p){setpgrp(0,0); exec @ARGV; exit 127} $SIG{ALRM}=sub{kill "KILL", -$p; print STDERR "TIMED OUT after $s s\n"; exit 124}; alarm $s; waitpid($p,0); exit($? >> 8)' SECONDS CMD...`
    7. `cargo test --locked --test conformance --no-run`.
    8. Probe `docker image inspect iamf-tools:v2.1.0` under W with 60 s. If it does not exit 0, STOP and report:
       CONF-06 is a required gate here and must not be reported as passed when skipped.
    9. Under W with 1200 s:
       `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec cargo test --locked --test conformance -- --nocapture`,
       stdout+stderr to `target/vvx-conformance.log`, then append `conf_exit=<code>`.
    If fmt or clippy needs a fix, make a follow-up commit by explicit path `style(quick-260913-vvx): ...`; amend nothing.
    If any gate fails for a reason unrelated to this change, stop and report it with the log excerpt rather than
    editing fixtures, goldens, DIFF-LEDGER.md, reference expectations or conformance code.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && D=.planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-deferred-items.md && test -f $D && grep -q 'noise_1024samp_5p1_opus' $D && grep -q 'noise_3s_stereo_opus' $D && grep -q 'tones_100ms_3OA_stereo_opus' $D && grep -q 'semantic_sha256' $D && grep -q 'f556119f' $D && grep -q 'AAC' $D && cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && grep -q '^offline_exit=0$' target/vvx-offline.log && [ -z "$(grep '^test result: ' target/vvx-offline.log | grep -v '^test result: ok')" ] && cargo test --locked --doc && cargo test --locked --features fuzzing --test fuzz_regression && grep -q '^conf_exit=0$' target/vvx-conformance.log && grep -q '^test result: ok' target/vvx-conformance.log && ! grep -q 'TIMED OUT' target/vvx-conformance.log && grep -qF '[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported' target/vvx-conformance.log && grep -qF '[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported' target/vvx-conformance.log && [ -z "$(grep -F 'SKIP CONF-05' target/vvx-conformance.log | grep -vE 'for phase3_(flac|opus): ')" ] && git diff --quiet e486630 -- src/sequence.rs src/obu tests/support tests/fixtures tests/golden.rs tests/conformance.rs tests/parse_reference.rs DIFF-LEDGER.md tools fuzz && git diff --quiet HEAD -- src tests && git diff --name-only e486630 HEAD > target/vvx-committed.txt && [ -z "$(grep -vE '^(src/encoder\.rs|src/error\.rs|tests/encoder_streaming\.rs|tests/error_shape\.rs|\.planning/STATE\.md|\.planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/.*)$' target/vvx-committed.txt)" ]</automated>
  </verify>
  <done>The deferred-items file names the three hash-affected fixtures, the decoder pre_skip evidence, the reference disagreement, the AAC-LC note and A1. fmt, both clippy runs, offline `cargo test --locked`, doc tests and fuzz replay pass. The conformance run ends `test result: ok` with `conf_exit=0`, CONF-06 lines for phase3_flac and phase3_opus, and only the manifest-designed phase3 FLAC/Opus CONF-05 SKIP lines. Protected paths are unchanged since e486630, `src` and `tests` have no uncommitted changes, and only allowed paths were committed.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| caller -> EncodingWriter::push_temporal_unit / finish | Caller-supplied trimming values cross into the writer that emits output bytes |
| EncodingWriter -> reference decoders | Output is consumed by libiamf / iamf-tools, neither of which checks pre_skip against the trim |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-vvx-01 | Tampering (output integrity) | EncodingWriter preflight | medium | mitigate | Equality check of the leading-run start-trim total against pre_skip at push time (Task 1); silently leftover priming or over-trimmed audio can no longer be written |
| T-vvx-02 | Denial of Service | start_trim_total accumulation | medium | mitigate | `checked_add` mapped to `OpusStartTrimExceedsPreSkip`; no bare arithmetic (clippy `arithmetic_side_effects`), no panic on hostile u32 trims |
| T-vvx-03 | Tampering (state) | TemporalProgress commit | low | mitigate | New total is written only into the returned progress and committed after a successful sink push; tests assert a rejected unit leaves bytes and state unchanged |
| T-vvx-04 | Repudiation / silent acceptance | finish() | low | mitigate | Open-run check after the poison check rejects empty and short all-trimmed Opus streams (Task 2) |
| T-vvx-05 | Tampering (foreign file fidelity) | SequenceWriter / parse-side validate | low | accept | Low-level writers and validate() are deliberately unchanged so foreign files keep round-tripping byte-exactly; parse-side finding deferred with evidence (Task 3) |
</threat_model>

<verification>
- `cargo fmt --all -- --check`; `cargo clippy --locked --all-targets -- -D warnings` with and without `--features fuzzing`.
- `env -u IAMF_REF_DECODER cargo test --locked` (every binary ok), `cargo test --locked --doc`,
  `cargo test --locked --features fuzzing --test fuzz_regression`.
- Conformance with `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec`
  under a perl alarm ends `test result: ok`, with CONF-06 lines for phase3_flac and phase3_opus and only the
  manifest-designed FLAC/Opus CONF-05 SKIP lines.
- `git diff --quiet e486630 -- src/sequence.rs src/obu tests/support tests/fixtures tests/golden.rs tests/conformance.rs tests/parse_reference.rs DIFF-LEDGER.md tools fuzz`.
</verification>

<success_criteria>
- An Opus `EncodingWriter` stream is accepted only when its total start trim equals `pre_skip`; exceeding is
  rejected as soon as it happens, a short closing unit is rejected at push time, and a short open run (including an
  empty stream) is rejected at `finish()`.
- Rejected pushes write no bytes and commit no state; LPCM and FLAC behaviour is unchanged.
- `size_of::<Error>() <= 32`; no fixture, golden, DIFF-LEDGER, reference expectation, low-level writer or
  conformance code change.
- The deferred-items file records the parse-side finding (three named fixtures), decoder pre_skip evidence,
  the reference disagreement, AAC-LC and A1.
</success_criteria>

<output>
Create `.planning/quick/260913-vvx-require-total-opus-start-trim-to-equal-p/260913-vvx-SUMMARY.md` when done.
</output>
