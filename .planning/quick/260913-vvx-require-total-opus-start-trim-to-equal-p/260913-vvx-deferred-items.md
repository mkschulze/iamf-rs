# Deferred items — quick 260913-vvx

None of items 1-5 is implemented in this task. Each entry gives its source, the current state, and why
it is out of scope.

## 1. Parse-side `validate()` finding for Opus start trim vs `pre_skip`

- **Source:** IAMF v1.1.0 `index.bs:1819` ("Pre-skip SHALL be the same as the number of audio samples
  to be trimmed at the start of coded Audio Substreams"); research 260913-vvx section E.
- **Current state:** only `EncodingWriter` (`push_temporal_unit` preflight and `finish()`) enforces it.
  `parse_sequence(..).validate()` and `DescriptorSet::validate()` report nothing.
- **Why out of scope:** a finding would change the `semantic_sha256` (`tests/parse_reference.rs:91-99`
  hashes the findings) of three committed reference files:
  - `iamf-tools/noise_1024samp_5p1_opus.iamf`: `pre_skip` 312, total start trim 0, end trim 584
  - `iamf-tools/noise_3s_stereo_opus.iamf`: `pre_skip` 120, total start trim 0, end trim 96
  - `iamf-tools/tones_100ms_3OA_stereo_opus.iamf`: `pre_skip` 312, total start trim 0, end trim 648

  None is marked `is_valid: false` upstream, and pinned `decoder_main` decodes them (probe a: 1336
  frames = 1024 + 312 priming). `test_000059` (312/312, valid) and `test_000124` (312/312,
  `is_valid: false`) are equal and unaffected. The finding also needs temporal-unit grouping and
  substream-run tracking in `validate()`, which is n56 deferred item 2. The three files stay permanent
  fuzz seeds; replay is unaffected.

## 2. Neither pinned decoder reads `pre_skip`

- **Source:** research 260913-vvx probes a-e.
- **Current state:** libiamf@f06e919e: the Opus vtable has no `.info`, so `ctx->delay` stays 0
  (`IAMF_opus_decoder.c:140-146`, `IAMF_core_decoder.c:145,283-287`); `pre_skip` is only logged
  (`vlogging_tool_sr.c:461`). iamf-tools@848c6ff4 `opus_decoder.cc:41-49` checks only `output_gain`
  and `mapping_family`. Probes b-e: `test_000059.iamf` with `pre_skip` patched to 0/100/312/624 decodes
  to the same WAV sha256 `f556119f…ac13`.
- **Why out of scope:** conformance (CONF-05/CONF-06) cannot catch a regression of this rule. The
  `tests/encoder_streaming.rs` tests are the guard, so no `EncodingWriter` Opus multi-unit CONF-06 probe
  was run.

## 3. Reference disagreement

- **Source:** iamf-tools@v2.1.0 `iamf/cli/proto_conversion/proto_to_obu/audio_frame_generator.cc:141-156`
  requires only `>=`. The spec and Eclipsa (`IAMFExportUtil.cpp:124-130`, `AudioElement.cpp:179-181`)
  require equality.
- **Current state:** recorded as a DISAGREEMENT comment in `src/encoder.rs`. The crate follows the spec,
  per the user decision "ja, mit genau gleich umsetzen" ("yes, implement with exactly equal").
- **Why out of scope:** nothing further to implement.

## 4. AAC-LC and other codecs

- **Source:** IAMF v1.1.0 `index.bs:1827-1842`.
- **Current state:** AAC-LC has no pre-skip or trim-equality rule, and the crate has no AAC
  `DecoderConfig`. FLAC and LPCM have no `pre_skip`.
- **Why out of scope:** nothing to implement.

## 5. Empty Opus stream (assumption A1)

- **Source:** research 260913-vvx assumption A1.
- **Current state:** the strict reading is accepted. An Opus writer finished with zero temporal units
  is rejected with `ErrorKind::OpusStartTrimShortOfPreSkip`.
- **Why out of scope:** revisit only if a descriptors-only Opus export is ever needed.

This task resolves `260913-n56-deferred-items.md` item 1.
