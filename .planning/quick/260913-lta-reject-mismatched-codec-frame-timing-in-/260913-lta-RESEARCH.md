# Quick 260913-lta: Reject mismatched codec frame timing and repair the Parallax fixture (Research)

**Researched:** 2026-09-13
**Domain:** IAMF v1.1.0 cross-Codec-Config conformance rules, `EncoderBuilder::build()` validation
**Confidence:** HIGH. Every rule below was read in the pinned trees, and the fixture outcomes were
**executed** against the pinned `iamf-tools:v2.1.0` `decoder_main` container.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Governing principle:** be IAMF-conformant. Derive the rules from the reference implementations
  (`iamf-tools` and `eclipsa-audio-plugin`) and implement to match them.
- **Rules come from the references, not from guesses.** Every validation rule this task adds must cite
  its source as `// ref: iamf-tools@v2.1.0 <path> <function>`. Eclipsa confirms how a real authoring
  tool keeps its configurations within these rules.
- **Pinned tree only:** `iamf-tools` `848c6ff4968ff8cc6f728259892ab4f90cb83256` (v2.1.0). Never HEAD.
- If `iamf-tools` and Eclipsa disagree, `iamf-tools@v2.1.0` together with the pinned `libiamf` wins.
  Record the disagreement.
- **Validation scope (defaults accepted):**
  - The check applies per sub-mix, matching the reference decoder.
  - It also enforces the other cross-config rules the pinned references apply. Research decides which
    rules apply and at what scope.
- **Error surface (defaults accepted):**
  - Fail in `EncoderBuilder::build()`.
  - Use a new typed `ErrorKind` that names the conflicting values plus enough identity to locate the
    conflict.
  - Follow the `error.rs` conventions: `#[non_exhaustive]` and the size assertion.
- **Fixture repair (default, subject to research):**
  - Default: a common frame size of 960 for all four elements.
  - Keep the retained names `program stereo`, `flac archive`, `opus stream` and `ambisonics bed`, the
    4 Audio Elements, the filtering and determinism assertions, and the parameter block durations.
  - If the references require something else, adjust the fixture and record why.

### Claude's Discretion
- Whether `EncodingWriter` / preflight also needs a runtime guard.
- Test layout.
- Whether to open follow-ups for trimming position and for profile selection for expanded layouts.

### Deferred Ideas (OUT OF SCOPE)
- None listed. Do not touch `README.md`, the `docs/` clones, the golden fixtures or `DIFF-LEDGER.md`.
</user_constraints>

## Summary

The research overturns two default assumptions in CONTEXT.md. Both were checked by execution.

1. **The "all 960" fixture repair does not work.** The pinned finalizer rejects a sub-mix whose Codec
   Configs differ in *bit depth* as well as frame size. `iamf-tools` gives Opus a loudness bit depth of
   **32**, and the fixture's LPCM and FLAC are **16**. With everything at 960, `decoder_main` now fails
   with `UNIMPLEMENTED: This implementation does not support mixing Codec Config OBUs with different
   sample rates or bit-depths.` (probe output below). Separately, the committed FLAC packets are
   128-sample blocks, so FLAC at 960 would also need a regenerated immutable corpus.
2. **More than one Codec Config is non-conformant in every profile this crate emits.**
   - IAMF v1.1.0 §4 "Common restrictions … for all profiles" says: "There SHALL be only one unique
     [=Codec Config OBU=]."
   - The pinned `decoder_main` enforces this in practice. Even two Codec Configs with matching timing
     (LPCM and FLAC, both 16-bit at 128) fail with `INVALID_ARGUMENT: Invalid Output frame size, was
     this a trivial IA Sequence?`.
   - Eclipsa always emits exactly one Codec Config, with id 200.

The per-sub-mix frame-size, sample-rate and bit-depth rules are real. They are all **implied by** the
single-Codec-Config rule, because an Audio Element's timing comes only from its Codec Config.

**Primary recommendation:**
- In `validate_declarations()`, reject any builder that declares more than one Codec Config. Use a new
  compact `ErrorKind` at `Location::Field("codec_configs")`, with `// ref:` citations to the spec §4
  line and the three `iamf-tools` sites below.
- Repair the fixture so all four Audio Elements share the one LPCM Codec Config (16-bit LE, 48 kHz,
  128 samples per frame). This was executed: CONF-06 passes with `Decoded 1 temporal units.`

## Findings: the cross-Codec-Config rules at the pins

### R1: sub-mix frame size must be equal (decoder and encoder) [VERIFIED: pinned tree]
- **Where:** `iamf/cli/rendering_mix_presentation_finalizer.cc:98-126`,
  `GetCommonCodecConfigPropertiesFromAudioElementIds`. It collects
  `codec_config->GetNumSamplesPerFrame()` for each element in the sub-mix:
  `if (num_samples_per_frame.size() != 1) { return absl::InvalidArgumentError("Audio elements in a submix must have the same number of samples per " "frame."); }`
- **Scope: every sub-mix.** On the decoder side it covers only the *selected* mix presentation.
  - `ObuProcessor::ConfigureSimplifiedAudioProcessingPipeline` (`obu_processor.cc:750-778`) builds the
    finalizer from `{simplified_mix_presentation}` alone.
  - The mix is chosen by `FindMixPresentationAndLayout` (`obu_processor_utils.cc:31-90`). With no
    `--mix_id` and no layout, it takes `supported_mix_presentations.front()`.
- **Encoder side:** `iamf_encoder.cc:294-301` calls `RenderingMixPresentationFinalizer::Create(...,
  mix_presentation_obus)` with **all** mix presentations, so every sub-mix is checked.
- **Kind:** hard error (`InvalidArgument`). This is not a profile downgrade.

### R2: sub-mix output sample rate and bit depth must be equal (decoder and encoder) [VERIFIED: pinned tree]
- **Where:**
  - `rendering_mix_presentation_finalizer.cc:115-117` calls `GetCommonSampleRateAndBitDepth`
    (`cli_util.cc:192-222`). That function sets `requires_resampling = true` when there is more than
    one sample rate or bit depth.
  - Then `rendering_mix_presentation_finalizer.cc:561-569`:
    `if (requires_resampling) { … return absl::UnimplementedError("This implementation does not support mixing Codec Config OBUs with " "different sample rates or bit-depths."); }`
- **Bit depth per codec** (`CodecConfigObu::GetBitDepthToMeasureLoudness`):
  - Opus: `static constexpr uint8_t GetBitDepthToMeasureLoudness() { return 32; }` (`opus_decoder_config.h:99`)
  - LPCM: `bit_depth_to_measure_loudness = sample_size_;` (`lpcm_decoder_config.cc:122-126`)
  - FLAC: `bit_depth_to_measure_loudness = stream_info->bits_per_sample + 1;` (`flac_decoder_config.cc:276-283`)
  - **Consequence:** Opus can never share a sub-mix with 16- or 24-bit LPCM/FLAC under the pinned
    `iamf-tools`.
- **Output sample rate:** Opus is always 48 kHz (`codec_config.h:140-152` doc comment). LPCM and FLAC
  use the stream's own rate.
- **Order of checks:** R2 computes `requires_resampling` *before* R1 runs, but only acts on it after R1
  returns. That is why the current fixture reports R1 first.

### R3: all Codec Configs in the sequence must share rate, bit depth and frame size (encoder output only) [VERIFIED: pinned tree]
- **Where:** `obu_sequencer_base.cc:178-202`, `FillDescriptorStatistics`, called from
  `PushDescriptorObus` (`:390`) and the update path (`:501`). It runs over **all** `codec_config_obus`:
  - `"Codec Config OBUs with different bit-depths and/or sample " "rates are not in base-enhanced/base/simple profile; they are not " "allowed in ISOBMFF."` (Unimplemented)
  - `GetCommonSamplesPerFrame` (`cli_util.cc:224-240`): `"The encoder does not support Codec Config OBUs with a different " "number of samples per frame yet."` (Unknown)
- **Scope:** the whole IA sequence. `decoder_main` does not run this; it writes WAV and does not use
  the sequencer.

### R4: exactly one Codec Config per IA sequence (spec; decoder-enforced in practice) [VERIFIED: spec tag + pinned tree + executed]
- **Spec:** `docs/iamf` tag `v1.1.0` (`e1315500`), `index.bs:1908` "Common restrictions on the
  [=IA Sequence=] for all profiles specified in this version of the specification:" and
  `index.bs:1912` "- There SHALL be only one unique [=Codec Config OBU=]." These are the only profiles
  the crate writes: Simple, Base and Base-Enhanced (`src/model/profile.rs:136-174`).
- **iamf-tools@v2.1.0:**
  - `obu_processor.cc:243-256` `GetSampleRateAndFrameSize`: `if (output_codec_config_obus.size() != 1) { ABSL_LOG(WARNING) << "Expected exactly one codec config OBUs, but found " …; return; }`
  - `output_frame_size_` stays empty, so `ObuProcessor::GetOutputFrameSize` (`:505-510`) fails with
    `"Output frame size, was this a trivial IA Sequence?"`.
  - `iamf_decoder.cc:485` consumes it and `decoder_main` aborts.
  - Source comments at `rendering_mix_presentation_finalizer.cc:156-159` and `:562-565`: "As of IAMF
    v1.1.0, the spec forbids multiple Codec Config OBUs."
- **Contrast only, NOT pinned:** `iamf-tools` HEAD `profile_filter.cc:325-400`
  `FilterProfilesForCodecConfigRules` (Condition A–D) is **absent at 848c6ff4**. `git grep` at the pin
  returns nothing. HEAD's table gives Simple, Base and Base-Enhanced `max_codec_configs` = 1. Conditions
  C and D (LPCM pairing, equal frame size and rate) only matter for draft v2 Base-Advanced and higher.
  This agrees with R4; do not port it.
- **Kind:** spec SHALL. In practice it is a hard decoder failure, not a profile downgrade.

### Other profile rules at the pin (already handled) [VERIFIED: `profile_filter.cc:35-41, 219-236`]
`kBaseProfileMaxAudioElements = 2`, `kBaseEnhancedProfileMaxAudioElements = 28`,
`kBaseEnhancedProfileMaxChannels = 28`. A mix presentation with more than 1 sub-mix erases all three
profiles. The repaired fixture (4 elements, 2+2+2+4 = 10 channels, 1 sub-mix) is Base-Enhanced, which
the crate already selects.

### libiamf@f06e919e [VERIFIED: pinned tree; consequence INFERRED]
- It has **no explicit rejection** of mismatched frame sizes or sample rates.
  - `iamf_set_stream_info` (`code/src/iamf_dec/IAMF_decoder.c:2246-2273`) grows buffers to the largest
    `max_frame_size`.
  - Rate differences go to a Speex resampler (`:3614-3618`) or a `ia_logw("Difference rate …")`
    (`:3480-3484`).
- **Latent defect:** the growth loop is `for (int n = 0; i < DEC_BUF_CNT; ++n)` (`:2261`), which tests
  `i`, not `n`. `DEC_BUF_CNT` is `3` (`IAMF_decoder_private.h:47`).
  - [INFERRED] With unequal frame sizes, `n` runs past `decoder->buffers[3]` into out-of-bounds
    reallocs. This is one more reason the permissive oracle must not be the gate.

### Eclipsa [VERIFIED: working tree, read-only; the repository is not pinned]
- There is exactly one codec per export. `IAMFFileWriter.cpp:42-62` clears `codec_config_metadata` and
  writes one of LPCM, FLAC or Opus.
- Every config uses `set_codec_config_id(200)` (`IAMFExportUtil.cpp:59, 79, 120`), and every Audio
  Element uses `aeMD->set_codec_config_id(200);` (`common/data_structures/src/AudioElement.cpp:101`).
- Frame sizes:
  - LPCM and FLAC use `set_num_samples_per_frame(samplesPerBlock)` (`:62, :83`).
  - Opus is forced: `set_num_samples_per_frame(960);  // 20ms at 48kHz` (`:124`), with a 960-sample
    accumulator (`IAMFFileWriter.cpp:180-187`).
- **No disagreement with iamf-tools.**

### Codec frame-size constraints [VERIFIED: pinned tree]
- **Opus:** `iamf-tools` has no 120/240/480/960 allow-list. It checks non-zero and roll distance
  `kOpusAudioRollDividend / n` (`opus_decoder_config.cc:87-101`), which the crate already mirrors
  (CODEC-03).
- **FLAC:** `minimum_block_size` and `maximum_block_size` must equal `num_samples_per_frame`
  (`flac_decoder_config.cc:89-93`), with `kMinMinAndMaxBlockSize = 16` and `kMaxBitsPerSample = 31`
  (`flac_decoder_config.h:40, 52`).
- **Fixture corpora:**
  - The FLAC corpus is `block_size = 128` (`tests/fixtures/codecs/flac/MANIFEST.md`).
  - The Opus corpus is `frame_samples = 960` (`tests/fixtures/codecs/opus/MANIFEST.md`).
  - No common frame size exists without regenerating immutable packets, and R2 and R4 would still fail.

## Execution evidence (scratch copy of `src/` + `tests/`, pinned container `iamf-tools:v2.1.0` present)

| Variant | `decoder_main` result |
|---|---|
| Default idea: LPCM, Opus and ambisonics at 960, FLAC element moved to LPCM (2 Codec Configs) | `E… obu_processor.cc:492] UNIMPLEMENTED: This implementation does not support mixing Codec Config OBUs with different sample rates or bit-depths.` → `F… decoder_main.cc:239] Failed to decode: Failure: INVALID_ARGUMENT: Failed to create OBU processor.` |
| LPCM and FLAC both at 128 and 16-bit, Opus element moved to LPCM (2 Codec Configs, timing equal) | `W… obu_processor.cc:249] Expected exactly one codec config OBUs, but found 2` → `F… decoder_main.cc:262] Failed to setup after descriptors: Failure: INVALID_ARGUMENT: Invalid Output frame size, was this a trivial IA Sequence?. Expected to have a value.` |
| **Recommended:** 1 LPCM Codec Config for all 4 elements, frame 128 | `[parallax delivery] CONF-06: decoder_main reported "Decoded 1 temporal units." (exit Some(0)…)` `test parallax_delivery_fixture_uses_the_offline_safe_reference_gates ... ok` |

With the recommended variant, the other conformance tests also passed (`the_flac_fixture_is_conformant`,
`the_opus_fixture…`, `the_sample_identity…`, `the_endianness…`). The two `DIFF-LEDGER` tests failed in
the scratch copy only, because `DIFF-LEDGER.md` was not copied there. `tests/parallax_contract.rs`
failed exactly once, at the expected assertion:
`LPCM, FLAC, Opus declaration order  left: [0]  right: [0, 1, 2]`.

## Integration in this crate

### Where to add the check [VERIFIED: `src/encoder.rs` read this session]
- `build()` is `src/encoder.rs:688-803`. It calls `self.validate_declarations()?` at `:690`, before any
  id allocation.
- The per-config loop is `:817-828` (`for config in &self.codec_configs { … validate_findings(config.validate())?; }`).
- **Add the count check immediately before that loop** (or after it, so single-config errors keep their
  current precedence; see pitfall 3):
  `if self.codec_configs.len() > 1 { return Err(Error::new(ErrorKind::<New>, Location::Field("codec_configs"))); }`
- Cite with:
  `// ref: IAMF v1.1.0 §4 (index.bs:1912) "There SHALL be only one unique Codec Config OBU"`,
  `// ref: iamf-tools@v2.1.0 iamf/cli/obu_processor.cc GetSampleRateAndFrameSize`,
  `// ref: iamf-tools@v2.1.0 iamf/cli/rendering_mix_presentation_finalizer.cc GetCommonCodecConfigPropertiesFromAudioElementIds`,
  `// ref: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc FillDescriptorStatistics`.
- The comment should state that R1–R3 (sub-mix and sequence frame, rate and bit-depth equality) are
  implied, because Audio Element timing comes only from its Codec Config.
- **Runtime guard (discretion):** not needed. `Encoder` is constructed only inside `build()` (`:797`),
  and descriptors are frozen. Do **not** add the rule to the low-level `SequenceWriter`,
  `write_sequence` or `write_parsed_sequence` (`src/sequence.rs:594, 721, 1008`). Those paths must
  round-trip foreign files byte-exactly. A `DescriptorSet::validate()` finding (`src/model/mod.rs:122`)
  is a possible follow-up. Check `tests/descriptors.rs:1313`, which pushes a duplicate codec config,
  before adding one.

### Error shape: the locked "names both values" cannot fit the budget [VERIFIED: `src/error.rs:43-45, 232-240`; executed]
- `error.rs:240`: `const _: () = assert!(size_of::<Error>() <= 32);`. `Location` is 24 bytes.
- A scratch `rustc 1.85` probe of the same enum shapes printed:
  `loc 24 k1 12 e1 40 k2 6 e2 32 k3 8 e3 32`.
  - A kind carrying `{u32, u32}` makes `Error` **40 bytes**, which breaks the build.
  - `{u16, u16}` or a single `u32` fits, but frame sizes and rates are `u32`.
  - A `Box` payload is also 8 bytes plus a tag, and with other data-carrying variants there is no niche.
- **Recommendation:** a unit variant in the existing compact-kind style (`error.rs:126-161`), for example
  `#[error("an IA sequence permits only one Codec Config")] MultipleCodecConfigs`, at
  `Location::Field("codec_configs")`.
- If the user insists on carrying a value, the most that fits is one `u32`, such as the count as
  `{ count: u32 }`. Record the deviation from the CONTEXT default.
- Add it to the list in `tests/error_shape.rs:167-181` (`static_builder_errors_are_compact_typed_kinds`).

### Tests to add
- In `tests/encoder_builder.rs`, reuse `lpcm_config()` (`:236`), `stereo_element()`, `add_fresh_element()`
  and `presentation_for_elements()`:
  1. Two configs, LPCM at 128 and Opus at 960 (the review's case), each referenced by an element in
     one sub-mix, is rejected with the new kind at `Field("codec_configs")`.
  2. Two configs with *identical* timing (LPCM and LPCM, or LPCM and FLAC at 128) are still rejected.
     This matches the executed decoder_main failure.
  3. Two configs split across separate mix presentations are still rejected, because the scope is the
     whole sequence.
  4. Positive case: one config shared by 4 elements in one sub-mix builds, and `sequence_profile()` is
     `Profile::BaseEnhanced`.
- Since both R1 and R2 are implied, no separate sample-rate or bit-depth test is reachable through the
  builder. State this in the plan rather than writing unreachable checks.

## Fixture repair: exact edit list

`tests/support/parallax_contract.rs`:
- Delete `:108-109`, the `flac` and `opus` `add_codec_config` lines.
- `:118-119`: map `FlacStereo` and `OpusStereo` to `builder.add_audio_element(lpcm, stereo_element(20|30))`.
- Delete `:178-182` (the corpus comment and both `include_bytes!`).
  - This also removes the fixture's dependency on the immutable codec corpora.
- `:185-186` become `FrameInput::Lpcm(vec![0; 512])`. That is 128 samples × 2 channels × 2 bytes,
  identical to `:184`.
- **Unchanged and still correct:**
  - The Codec Config `lpcm(88, 128, {LittleEndian, 16, 48_000})` at `:99-107`.
  - Ambisonics `vec![0; 256]` at `:191` (128 × 1 × 2).
  - Parameter block `duration: 128` and subblocks `64 + 64` at `:278-293`. This equals the frame
    duration at `parameter_rate` 48 000, as spec §4 `index.bs:1915` requires.
  - Retained and excluded names, both presentations, and `stereo_element` labels 10/20/30.
- **Optional for honesty:** rename or document `CandidateKind::FlacStereo/OpusStereo`, since they now
  lower to LPCM. Keep the variants: they are constructed, so there is no dead-code lint. The
  candidate *names* are locked.

`tests/parallax_contract.rs:193-197`: `vec![0, 1, 2]` / `"LPCM, FLAC, Opus declaration order"` →
`vec![0]` with a message such as `"one Codec Config per IA sequence (IAMF v1.1.0 §4)"`. All other
assertions pass unchanged (executed): element ids `[0,1,2,3]`, presentations `[0,1]`, 1 parameter block.

`tests/conformance.rs:1729-1745`: no change. `"4 Audio Element(s)"` and the expected temporal units (1)
still hold (executed).

## Common Pitfalls
1. **Reading HEAD `profile_filter.cc`.** Condition A–D does not exist at 848c6ff4. Porting HEAD's
   Condition C/D would *allow* 2 Codec Configs, which pinned `decoder_main` rejects.
2. **Trusting `libiamf` acceptance.** It is permissive, and at `IAMF_decoder.c:2261` it is actively
   buggy for unequal frame sizes. `decoder_main` is the gate.
3. **Error precedence drift.** Existing tests expect `codec_id` and raw-config errors from single-config
   builders (`tests/encoder_builder.rs:249-266`). Those builders have one config, so the new check never
   fires there. Still, place it so any existing single-config test keeps its current error.
4. **Golden and ledger stability.** `tests/golden.rs`, `tests/support/fixture.rs` and `test_000003`
   build `DescriptorSet`s directly with `codec_configs: vec![config]` (one each) and do not use
   `EncoderBuilder`. `grep` shows builder users only in `tests/{encoder_builder,encoder_streaming,parallax_contract,public_api}.rs`
   and the fixture. All except the fixture use one config. No golden bytes or `DIFF-LEDGER.md` change.
5. **Clippy set.** No indexing or bare arithmetic is needed; `len() > 1` is a comparison. There is no
   `HashSet` for "distinct configs", which is one reason to use the simple count.
6. **Determinism.** The new check is order-independent. The fixture change removes two OBUs but stays
   deterministic, since the `first.bytes == second.bytes` assertion passed.

## Assumptions Log
| # | Claim | Risk if wrong |
|---|---|---|
| A1 | The user accepts superseding the CONTEXT defaults: a whole-sequence "one Codec Config" rule instead of a per-sub-mix frame check, and an all-LPCM fixture instead of all-960. The evidence is spec plus executed decoder failures, but it changes locked-default intent. | Plan needs a `checkpoint:decision` if this is not accepted. The fallback (R3 only: equal frame, rate and bit depth across configs) still cannot make any Opus + 16-bit multi-codec fixture pass `decoder_main`. |
| A2 | A unit `ErrorKind` (no values) is acceptable given the executed 40-byte probe. | Alternatively a single `u32` payload. Two values are impossible without changing the budget. |
| A3 | [INFERRED] The `libiamf` loop bug causes out-of-bounds access with unequal frame sizes. | Informational only; no plan dependency. |

## Open Questions
1. **Should Parallax still exercise FLAC and Opus deliveries?** Under v1.1.0, each codec must be its
   own IA sequence. Recommendation: out of scope here. A follow-up can add per-codec delivery builds
   (one Codec Config each) if the contract needs codec coverage.
2. **Follow-ups (discretion):**
   - Enforce spec §4 `index.bs:1915`: parameter block duration equals the frame duration at the same
     rate. `preflight` currently checks only non-zero.
   - The previously noted trimming-position and expanded-layout profile items.

## Environment Availability
| Dependency | Available | Version |
|---|---|---|
| Docker + `iamf-tools:v2.1.0` image | ✓ (`4a011a6d2b9e`, matches REFERENCES.md manifest-list prefix) | — |
| Rust toolchain | ✓ | rustc/cargo 1.85.0 |

## Security Domain
V5 Input Validation applies. This is a builder-side declaration check with a typed error and no panic
paths. Nothing else changes, and no new dependencies are added.

## Sources
- `iamf-tools@848c6ff4` (git show): `iamf/cli/rendering_mix_presentation_finalizer.cc`, `cli_util.cc`, `obu_sequencer_base.cc`, `obu_processor.cc`, `obu_processor_utils.cc`, `iamf_encoder.cc`, `profile_filter.cc`, `decoder_main.cc`, `iamf/obu/codec_config.{h,cc}`, `iamf/obu/decoder_config/{opus,lpcm,flac}_decoder_config.*`
- `iamf-tools@HEAD d13b8dd5` `iamf/cli/profile_filter.cc`, for contrast only
- IAMF spec `docs/iamf` tag `v1.1.0` `index.bs:1908-1916`
- `libiamf@f06e919e` `code/src/iamf_dec/IAMF_decoder.c`, `IAMF_decoder_private.h`
- `eclipsa-audio-plugin@c964609` `common/processors/file_output/iamf_export_utils/{IAMFExportUtil,IAMFFileWriter}.cpp`, `common/data_structures/src/AudioElement.cpp`
- This crate: `src/encoder.rs`, `src/error.rs`, `src/model/profile.rs`, `tests/support/parallax_contract.rs`, `tests/parallax_contract.rs`, `tests/conformance.rs`, `tests/encoder_builder.rs`, `tests/error_shape.rs`, `tests/fixtures/codecs/*/MANIFEST.md`

**Valid until:** as long as both pins are unchanged. A fact read at a pinned SHA does not expire.
