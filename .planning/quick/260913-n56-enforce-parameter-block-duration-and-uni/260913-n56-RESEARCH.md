# Quick 260913-n56: Enforce Parameter Block duration and uniform trimming in the encoder (Research)

**Researched:** 2026-09-13
**Domain:** IAMF v1.1.0 temporal rules (index.bs §4 common restrictions, OBU header trimming semantics), `EncodingWriter` preflight
**Confidence:** HIGH. Every rule is read at the pins, and every reference-behaviour claim marked
"executed" was run against the pinned `iamf-tools:v2.1.0` `decoder_main` (image `4a011a6d2b9e`) on files
this crate wrote from a scratch copy.

## Constraints (standing directive, CLAUDE.md)
- Derive rules from `iamf-tools@848c6ff4` (v2.1.0), `libiamf@f06e919e`, spec tag `v1.1.0`, and Eclipsa (read-only).
  Each added check gets a `// ref:` citation. Where the references disagree, `iamf-tools@v2.1.0` together
  with pinned `libiamf` decides, and the disagreement is recorded.
- No `unwrap`/`expect`/indexing/bare arithmetic in `src/`. No hashed containers (`Vec` + `contains`).
  `thiserror` kinds, `#[non_exhaustive]`, `size_of::<Error>() <= 32` (`src/error.rs:240`).
- Low-level writers must keep round-tripping foreign files byte-exactly. Golden fixtures and `DIFF-LEDGER.md`
  do not change.

## Summary
1. **iamf-tools is stricter than the spec on parameter rate.** The spec allows any `parameter_rate` that
   gives a whole, non-zero number of ticks per frame (index.bs:863-865), and its own example uses 480
   ticks at 24 kHz (:1916). Pinned `decoder_main` rejects **any mix-gain `parameter_rate` different from
   the output sample rate, even when the file has zero Parameter Blocks**. The executed error is
   `UNIMPLEMENTED: Parameter blocks that require resampling are not supported yet.`
   The spec-legal scaled case (24 kHz, duration 64 for a 128-sample frame) also fails, with
   `Expected timestamp != actual timestamp: (128 vs 64)`, because iamf-tools compares ticks with samples
   unscaled. Under the tie-break rule, `parameter_rate == output sample rate` is the rule. With that rule
   in place, "duration equals the frame duration" reduces to `duration == num_samples_per_frame`.
2. **Uniform trimming within a temporal unit is already guaranteed structurally.** `TemporalUnitInput`
   has one `trimming` shared by every frame (`src/encoder.rs:200-201`, applied at `:270`). The low-level
   `validate_temporal_unit` also rejects mixed trims (`src/obu/audio_frame.rs:351-355`). The **open gaps**
   are elsewhere:
   - trim *placement across* units: the start-trim chain (index.bs:541) and end trim only at the end (:542)
   - parameter-substream coverage: the same parameter ids in every unit (:1914), with no duplicates (:1918)
3. **`decoder_main` is blind to trim placement** (executed): mid-stream end trims, late start trims and
   frames after a full end trim all decode "4 temporal units". It **does** reject parameter gaps, late
   starts and duration mismatches from the second unit on. A one-unit file with a wrong duration passes
   (timestamps 0 == 0), so a single-TU conformance run cannot be the only guard.

**Primary recommendation:**
- In `EncoderBuilder::build()`, add two static rules: P1, rate equals the output sample rate; P2,
  mode-0 duration equals the frame size.
- In `EncodingWriter`, add per-unit rules: P3, mode-1 block duration equals the frame size; P5, no
  duplicate parameter id.
- Also add stateful rules (P4, T3, T4), with small state committed only after a successful sequence push.
- Use six new payload-free `ErrorKind`s (five if P5 is folded into the coverage kind).
- Defer the Opus `pre_skip` rule. It needs a decision, because the spec and iamf-tools disagree.

## A. Parameter Block duration

### Spec [VERIFIED: `git show v1.1.0:index.bs`]
- :1914 "Every [=Parameter Substream=] … SHALL have the same start timestamp as the [=Audio Substream=] which the [=Parameter Substream=] is applied to, and SHALL consist of the same number of [=Parameter Block OBU=]s."
- :1915 "Every [=Parameter Block OBU=] SHALL have the same duration as its corresponding [=Audio Frame OBU=] under the same sample rate." :1916 gives the example: 960 samples at 48 kHz means 960 units at 48000 Hz, or 480 units at 24000 Hz.
- :1917-1918 "the start timestamp of every [=Audio Frame OBU=] SHALL be the same as its corresponding [=Parameter Block OBU=], if present." / "There SHALL be no redundant [=Parameter Block OBU=]s."
- :863-865 "The parameter rate SHALL be a value such that the number of ticks per frame, computed as (parameter_rate × num_samples_per_frame / Audio Element sample rate), is a non-zero integer."
- **Mode.** The rule is about the effective block duration. In mode 0 the duration comes from the
  definition (:869, "None of the parameter blocks … SHALL specify these same fields"). In mode 1 each
  block carries it (:871). Both modes are therefore covered.
- **Demixing / recon gain** (:778-782, :788-792) have tighter rules: `parameter_rate` equals the element
  sample rate, mode 0, `duration` equals `num_samples_per_frame`, `num_subblocks` is 1, and
  `constant_subblock_duration` equals `duration`. **These are not reachable through the builder**, which
  rejects element params (`src/encoder.rs:962-971`, `UnsupportedParameterData`) and `recon_gain_is_present`
  (`:1192-1196`). Only mix gain is in scope. If the builder ever enables them, iamf-tools'
  `audio_element_generator.cc:148-155, 166-173` has the matching errors: "Demixing parameter duration= … is inconsistent with num_samples_per_frame=" and "Recon gain parameter duration= …".

### iamf-tools@v2.1.0 [VERIFIED: pinned tree]
- **Timing model** (`global_timing_module.cc`):
  - Audio substreams start at 0 and advance by `num_samples_per_frame` in samples
    (`obu_with_data_generator.cc:627-635`).
  - Parameter ids start at 0 and advance by the block duration in **ticks** (`:60-68`, `:162-177`).
  - `GetNextParameterBlockTimestamps` (`:121-131`) then runs `CompareTimestamps(input_start_timestamp, start_timestamp, …)`.
    Its text is `"In GetNextParameterBlockTimestamps() for param ID= <id>: Expected timestamp != actual timestamp: (<a> vs <b>)"` (`cli_util.cc:154-163`).
  - **There is no scaling**: `// TODO(b/283281856): Handle cases where parameter_rate and sample_rate differ.` (`global_timing_module.cc:45-46`).
- **Decoder:** `obu_processor.cc:176-189` passes the *global audio timestamp* (the common start of all
  substreams, `global_timing_module.cc:141-159`) as the expected start. Consequences:
  - a gap or a late-starting parameter substream fails
  - a wrong duration fails at the next block
- **Rate guard:** `rendering_mix_presentation_finalizer.cc:208-218`
  `if (mix_gain.parameter_rate_ != common_sample_rate) { return absl::UnimplementedError("Parameter blocks that require resampling are not supported yet."); }`
  - It sits before the block lookup, so it fires with no blocks present.
  - It is called for `element_mix_gain` (`:364`) and `output_mix_gain` (`:375`).
- **Encoder:**
  - `parameter_block_generator.cc:497-508` takes the duration from the definition (mode 0) or the
    metadata (mode 1), then runs the same timestamp compare.
  - `temporal_unit_view.cc:104-126` requires each block's start and end timestamps to equal the
    unit's audio start and end timestamps, and rejects duplicates: `"A temporal unit must not have multiple parameter blocks with the same parameter ID."`
  - `iamf_encoder.cc:458` comment: "Parameter blocks need to cover any delayed or trimmed frames."

### libiamf@f06e919e [VERIFIED: pinned tree; behaviour INFERRED]
- `iamf_database_parameter_add` (`IAMF_decoder.c:1131-1160`) queues segments and adds up `pi->duration`.
- Elapse is rescaled with `time_transform(duration, rate, param rate)` (`:1178`).
- There is no rejection path for a wrong duration or rate. It is permissive, as expected.

### Eclipsa [VERIFIED: working tree c964609, read-only]
- `MixPresentation.cpp:252-259` sets `set_parameter_rate(sampleRate)` and `set_param_definition_mode(true)`.
- No parameter-block metadata is written anywhere in `common/processors/file_output` (grep shows no
  `parameter_block_metadata`), so Eclipsa emits **zero Parameter Blocks** and relies on the default gains.
- This is consistent with P1. There is no disagreement.

### Executed probes (scratch crate → pinned `decoder_main`; LPCM 16-bit, 48 kHz, 128 samples per frame, stereo 1+1 coupled, 4 TUs)
| Case | Result |
|---|---|
| a: rate 48k, mode-1 duration 128 every TU | `Decoded 4 temporal units.` |
| b: rate 48k, duration 64 | `INVALID_ARGUMENT: In GetNextParameterBlockTimestamps() for param ID= 0: Expected timestamp != actual timestamp: (128 vs 64)` |
| c: rate 24k, duration 64 (spec-legal scaling) | same error `(128 vs 64)` |
| m: rate 24k, duration 128 | `UNIMPLEMENTED: Parameter blocks that require resampling are not supported yet.` |
| n: rate 24k, **no blocks** | `UNIMPLEMENTED: Parameter blocks that require resampling are not supported yet.` |
| d: blocks in TU 0, 2, 3 (gap) | `… Expected timestamp != actual timestamp: (256 vs 128)` |
| e: blocks from TU 1 (late start) | `… (128 vs 0)` |
| l: blocks in TU 0-2, none in TU 3 | `Decoded 4 temporal units.`, which the spec still forbids (:1914 "same number") |
| i: mode-0 definition duration 64 | `… (128 vs 64)` |
| j: mode-0 definition duration 128 | `Decoded 4 temporal units.` |
| f: rate 48k, no blocks | `Decoded 4 temporal units.` |

## B. Trimming

### Spec [VERIFIED: index.bs]
- :537 "the sum of num_samples_to_trim_at_start and num_samples_to_trim_at_end SHALL be less than or equal to … num_samples_per_frame". This is already enforced (`src/encoder.rs:453-466`).
- :541 "When num_samples_to_trim_at_start is non-zero, all Audio Frame OBUs with the same audio_substream_id, and preceding this OBU back until the Codec Config OBU defining this Audio Substream, SHALL have their num_samples_to_trim_at_start field equal to … num_samples_per_frame."
- :542 "When num_samples_to_trim_at_end is non-zero in an Audio Frame OBU, there SHALL be no subsequent Audio Frame OBU with the same audio_substream_id until a non-redundant Codec Config OBU …". The builder never repeats descriptors, so the end trim is terminal.
- :1913 "Every Audio Substream … SHALL have the same start timestamp, SHALL consist of the same number of Audio Frame OBUs, and SHALL have the same trimming information." This is structurally met: one trim per unit, and `MissingTemporalSubstream`/`DuplicateTemporalSubstream` checks (`src/encoder.rs:239-256`).
- :1819 (Opus) "Pre-skip SHALL be the same as the number of audio samples to be trimmed at the start of coded Audio Substreams."

### iamf-tools@v2.1.0 [VERIFIED]
- **Uniform per unit:** `temporal_unit_view.cc:147-155` (encoder). The texts are
  `"`num_samples_to_trim_at_end` must be the same for all audio frames"` / `"…_at_start…"`, and
  `:73-77` checks "cumulative trim is <= `num_samples_per_frame`".
- **Start trim after audio:** `obu_sequencer_base.cc:444-447` `"A unit has samples to trim at start, but the first untrimmed sample was already found."`
  - "Found" means `num_untrimmed_samples_ != 0` (`:570-577`).
- **Generation is structural:**
  - `audio_frame_generator.cc:362-373` `GetNumSamplesToTrimForFrame` takes `min(frame, remaining)`, so
    a start trim uses up whole frames and then the remainder.
  - End trim is only the final-frame padding (`:321-340`).
- **Opus:**
  - `opus_encoder.cc:132-135` checks that `pre_skip` equals the libopus lookahead.
  - `audio_frame_generator.cc:141-156` `"The encoder requires <n> samples trimmed at the start but only <m> were requested"`. This is **>=**, not ==.
  - **Disagreement with spec :1819 (==).**
- **Decoder:** no placement or uniformity check. The trim is applied per frame
  (`demixing_module.cc:574-577`, `renderer_utils.cc:70-80`).

### libiamf [VERIFIED; INFERRED behaviour]
- `iamf_decoder_internal_deliver` stores the trim only from substream index 0 of each element
  (`IAMF_decoder.c:3288-3298`), and applies it per frame (`:3778-3827`).
- It does not reject anything. Non-uniform trims would silently follow substream 0.

### Eclipsa [VERIFIED]
- `AudioElement.cpp:178-181` sets user trims to 0, with `…includes_padding(false)` and
  `…includes_codec_delay(false)`.
- `IAMFExportUtil.cpp:126-130` sets `automatically_override_codec_delay(true)` and `pre_skip(312)`.
- So iamf-tools inserts start trim equal to the codec delay, and end trim equal to the final padding.
- There are never mid-stream trims.

### Executed probes (no parameter blocks)
- g, trims `[None, end 100, None, start 48]`: `Decoded 4 temporal units.`
- h, `[start 48, start 48, …]`: `Decoded 4 temporal units.`
- o, `[None, end 128, None, None]`: `Decoded 4 temporal units.`
- k, valid `[start 128, start 28, None, end 50]` with blocks every TU: `Decoded 4 temporal units.`

**Conclusion:** trim placement is spec-only plus the iamf-tools *encoder*. The crate must enforce it,
because the gate cannot see it.

## C. Integration [VERIFIED: files read this session]

- `push_temporal_unit` (`src/encoder.rs:224-228`) calls `check_not_poisoned`, then
  `preflight(&self)`, then `sequence.push_temporal_unit`.
- `EncodingWriter` fields are `sequence`, `descriptors`, `generation` (`:206-210`). There is no
  cross-unit state today.
- `finish` (`:234-236`) delegates.
- Parameter blocks:
  - `preflight` `:273-301` resolves the handle, checks `parameter_id`, and runs
    `validate_submitted_parameter_block` (`:468-494`), which covers mode 1 only: a non-zero duration and
    non-empty, non-zero subblocks.
  - It then runs `write_parameter_block` into a scratch writer, which covers exact tiling
    (`SubblockDurationMismatch`, `src/obu/parameter_block.rs:589-615`).
  - There is **no** duplicate-id check and no duration-vs-frame check.
- Static: `validate_parameter_definition` (`:1233-1259`) checks rate != 0 (`invalid("parameter_rate")`)
  and mode-0 tiling. There is no rate or duration relation to the Codec Config.
- One Codec Config is guaranteed by `:846-851`, so `self.descriptors.codec_configs.first()` is *the*
  frame plan.
- Output sample rate:
  - LPCM uses `LpcmDecoderConfig.sample_rate` and FLAC uses `FlacDecoderConfig.sample_rate`
    (`src/obu/codec_config.rs:109, 125`).
  - Opus uses the constant 48 000. That is `OPUS_SAMPLE_RATE` (`codec_config.rs:52`, private), per
    iamf-tools `opus_decoder_config.h:80` `uint32_t GetOutputSampleRate() const { return 48000; }`.
  - Add a small private helper. Do not use `input_sample_rate`.
- `src/packing.rs` is a channel-to-substream plan only. It has no timing and is untouched.

### Where each rule goes
| # | Rule | Check point | Ref |
|---|---|---|---|
| P1 | every declared mix-gain `parameter_rate` == Codec Config output sample rate | `validate_declarations`, after `MultipleCodecConfigs` (`:846`) so that test keeps its kind | spec :863-865 (weaker); iamf-tools `rendering_mix_presentation_finalizer.cc GetParameterBlockLinearMixGainsPerTick`; executed m, n |
| P2 | mode-0 definition `duration` == `num_samples_per_frame` | same loop, after `validate_parameter_definition` so tiling errors keep precedence | :1915; `global_timing_module.cc GetNextParameterBlockTimestamps`; executed i/j |
| P3 | mode-1 block `duration` == `num_samples_per_frame` | `preflight`, after `write_parameter_block` validation (`:291-296`) | :1915; `obu_processor.cc GetAndStoreParameterBlockWithData`; `temporal_unit_view.cc ValidateAllParameterBlocksMatchStatistics`; executed b |
| P4 | the set of parameter ids with a block is identical in every unit (fixed by the first pushed unit) | `preflight` compares against state; the state is set after the first successful push | :1914 "same start timestamp … same number"; executed d, e (l is spec-only) |
| P5 | no two blocks with the same parameter id in one unit | `preflight`, parameter loop | :1918; `temporal_unit_view.cc:110-116` |
| T3 | `at_start > 0` only while every earlier unit had `at_start == num_samples_per_frame` | `preflight` against state `start_trim_open` | :541; `obu_sequencer_base.cc PushTemporalUnit` (:444-447) |
| T4 | no unit after a unit with `at_end > 0` | `preflight` against state `ended_by_end_trim` | :542; iamf-tools generator structural (`audio_frame_generator.cc:321-340`) |

**State and commit discipline.**
- Add to `EncodingWriter`:
  - `parameter_substreams: Option<Vec<u32>>`
  - `start_trim_open: bool` (initially `true`)
  - `end_trimmed: bool`
- `preflight` stays `&self` and returns the lowered unit **plus** the next state.
- Assign the state only after `self.sequence.push_temporal_unit(&unit)` returns `Ok`. A typed
  rejection then leaves both the sink and the state at the previous boundary, which keeps the
  guarantee in the existing doc comment at `:219-223`. After a sink error the writer is poisoned anyway.
- T3 transition: the next `start_trim_open = start_trim_open && at_start == num_samples_per_frame`,
  with `None` trim treated as 0.
- T1 (`:537`) already makes `at_start == nspf` imply `at_end == 0`, so T3 and T4 cannot conflict.
- **Place T3/T4 after the frames loop** so existing trim-size tests keep `TemporalUnitTrimMismatch`.
- Use only comparisons and `contains`; no arithmetic is needed.
- For P4, compare with a length check plus `contains` (P5 already rules out duplicates).

### Error variants (payload-free; unit variants cannot grow `Error` past 32 bytes, as measured in 260913-lta)
- `ParameterRateMismatch`: "a mix-gain parameter_rate differs from the Codec Config output sample rate". Location `Field("parameter_rate")`. Static list.
- `ParameterBlockDurationMismatch`: "a Parameter Block duration differs from its Audio Frame duration". Location `Field("duration_fields")` for P2 in build, `Field("parameter_block.duration")` for P3.
- `ParameterSubstreamCoverageMismatch`: "a temporal unit's Parameter Blocks do not cover the same parameters as the first temporal unit". Location `Field("parameter_blocks")`.
- `DuplicateTemporalParameterBlock`: "a temporal unit carries more than one Parameter Block for one parameter". Location `Field("parameter_blocks")`.
- `StartTrimAfterUntrimmedAudio`: "num_samples_to_trim_at_start follows audio that was not fully trimmed". Location `Field("trimming")`.
- `TemporalUnitAfterEndTrim`: "a temporal unit follows one that trimmed samples at its end". Location `Field("trimming")`.

Register the build kinds in `tests/error_shape.rs:167-181` and the temporal kinds in the temporal
list (`:185-195`). This gives six kinds; the planner may fold P5 into the coverage kind to get five.

### Tests that break or must be adjusted
- `tests/encoder_streaming.rs:408-446` `mode_1_submitted_blocks_require_non_empty_positive_exact_tiling`
  is a **positive** test with `duration: 10` and subblocks 4+6 on a 128-sample frame. Change it to
  `duration: 128` with, for example, 64+64. The test's intent (exact tiling accepted) is kept.
- `:302-344` (4+5 against 10) and `:346-406` (zero or empty) keep `SubblockDurationMismatch`, provided
  P3 runs *after* the existing tiling and write validation.
- `:93-139` trim-size tests are TU 0, so T3 and T4 do not fire. They are unchanged.
- `tests/encoder_builder.rs`:
  - All gains are 16 kHz on 16 kHz LPCM (`:22, 49-51, 326, 631-884`), so P1 passes.
  - The Opus 48 kHz case at `:236-251` still reports `MultipleCodecConfigs` if P1 is placed after that check.
  - `build_rejects_invalid_static_parameter_durations` (`:540-567`) stays rejected.
- `tests/parallax_contract.rs:238-278` uses 16 k/16 k with no blocks, so it is unchanged.
- The fixture `tests/support/parallax_contract.rs` uses 48 k LPCM with 48 k gains (`:108, 246-256`),
  block duration 128 on a 128-sample frame (`:273-286`), 1 TU and no trim. It is unchanged, and CONF-06
  is unchanged.
- `tests/golden.rs`, `tests/support/fixture.rs`, `test_000003` and `sequence_cases.rs` build
  `DescriptorSet`s or use the low-level writer, not `EncoderBuilder` (users are only
  `tests/{encoder_builder,encoder_streaming,parallax_contract,public_api}.rs` and the fixture).
  **No golden bytes and no `DIFF-LEDGER.md` change.**

### New tests (in `tests/encoder_streaming.rs`, reuse `stereo_builder_with_parameter` at `:517-560`; its rate and LPCM are both 16 kHz with 128 samples per frame)
- **Rejections.** For each, assert the kind and `bytes_written()` unchanged, and that a following
  *valid* unit is still accepted (state not committed):
  - P3: mode-1 block duration 64 with subblocks 32+32.
  - P4: block in TU 0, none in TU 1.
  - P4: none in TU 0, block in TU 1.
  - P5: the same handle twice in one unit.
  - T3: TU 0 `at_start 48`, then TU 1 `at_start 48`.
  - T3: TU 0 none, then TU 1 `at_start 1`.
  - T4: TU 0 `at_end 10`, then any TU 1.
- **Positives:**
  - 3 TUs with a block every unit (duration 128).
  - Trims `[start 128, start 28, None, end 50]`, which mirrors executed probe k.
  - The zero-block stream is unchanged.
- **Build** (`tests/encoder_builder.rs`):
  - P1: gain rate 48 000 on 16 kHz LPCM is rejected.
  - P1 Opus: an Opus config (960 samples) with 48 000 gains builds, and with 16 000 gains it is rejected.
  - P2: mode-0 duration 64 (constant 64) on a 128-sample frame is rejected.
  - P2: mode-0 duration 128, constant 128, builds.

## D. Low-level writers
- Keep all rules out of `SequenceWriter::push_temporal_unit` (`src/sequence.rs:816-853`), `write_sequence`
  and `write_parsed_sequence`. Foreign spec-legal files (for example a 24 k parameter rate, or libiamf-accepted
  streams) must keep round-tripping. This is the same conclusion as 260913-lta.
- The existing intra-unit trim equality check (`src/obu/audio_frame.rs:341-366`) is already there. It is
  spec-grounded and stays.
- **`validate()` findings:** a spec-grounded finding (P2/P3 duration vs frame, T3/T4 placement, P4
  coverage) would suit `parse_sequence(..).validate()` (`src/sequence.rs:195`). **P1 must not become a
  finding**, because it is an iamf-tools limitation and not a spec rule. Recommend deferring findings to a
  follow-up: they need temporal-unit grouping in `validate()`, which does not exist today.

## E. Deferred (with reason)
1. **Opus `pre_skip` vs total start trim.**
   - The spec (:1819) says ==. The iamf-tools encoder says >= (`audio_frame_generator.cc:141-156`).
   - The rule is stateful across the start-trim run and needs a `finish()` check for all-trimmed or empty streams.
   - It would break `tests/encoder_streaming.rs:141-185` (Opus `pre_skip` 312, no trim).
   - **Needs a user decision (== or >=).**
2. **`validate()` findings for P2–P5, T3 and T4** on parsed sequences (see D).
3. **Demixing and recon-gain duration rules** (:778-792). They are unreachable while the builder rejects
   those params.
4. **Discovered: stereo with 2 uncoupled substreams.**
   - The builder accepts `LoudspeakerLayout::Stereo` with `substream_count 2, coupled 0`, because
     `validate_element_topology` checks only channel-count equality (`src/encoder.rs:1198-1209`).
   - Pinned `decoder_main` rejects it: `INVALID_ARGUMENT: Coupled substream count different from the required number. In OBU: 0 vs expected: 1` (executed).
   - `tests/encoder_streaming.rs` stereo builders use this shape. This is out of scope and needs its own
     task (iamf-tools `obu_with_data_generator.cc ValidateSubstreamCounts`).

## Assumptions Log
| # | Claim | Risk if wrong |
|---|---|---|
| A1 | The tie-break rule applies to P1: the builder rejects spec-legal scaled parameter rates because pinned `decoder_main` cannot decode them (executed c, m, n). | If the user prefers the spec, replace P1 with the :863-865 integer-ticks rule and scale P2/P3 (`duration * sample_rate == parameter_rate * nspf`, with `checked_mul` in u64). Output would then fail CONF-06 whenever the rate differs. |
| A2 | The P4 "set fixed by the first unit" reading is exact: the spec needs equal counts and equal start, and one block per unit follows from P3. | Low. It matches executed d, e and the spec's "same number". |
| A3 | Behaviour notes on libiamf (permissive, substream-0 trims) are INFERRED from reading, not executed. | Informational only. |

## Environment
| Dependency | Available |
|---|---|
| Docker + `iamf-tools:v2.1.0` (`4a011a6d2b9e`) | ✓ (used) |
| Rust toolchain (repo pin 1.85.0) | ✓ |

Probe crate and outputs: `/private/tmp/claude-501/-Users-cell-local-iamf-rs/ffcacbe6-b061-4445-a28e-fe0219356ad0/scratchpad/probe/` (`src/main.rs`, `out/*.log`). The repo is unmodified.

## Security
ASVS V5 input validation. The new checks are typed rejections with no panics, no allocation beyond a
`Vec<u32>` of declared parameter ids, and no new dependencies.

## Sources
- Spec `docs/iamf` tag v1.1.0 `index.bs`: :537, :541-542, :778-792, :862-871, :1819, :1913-1919
- iamf-tools@848c6ff4:
  - `iamf/cli/global_timing_module.cc`, `cli_util.cc`, `obu_processor.cc`, `obu_with_data_generator.cc`
  - `temporal_unit_view.cc`, `obu_sequencer_base.cc`, `iamf_encoder.cc`, `rendering_mix_presentation_finalizer.cc`
  - `proto_conversion/proto_to_obu/{audio_frame_generator,parameter_block_generator,audio_element_generator}.cc`
  - `codec/opus_encoder.cc`, `obu/decoder_config/opus_decoder_config.h`
- libiamf@f06e919e `code/src/iamf_dec/IAMF_decoder.c`
- eclipsa-audio-plugin@c964609:
  - `common/data_structures/src/{MixPresentation,AudioElement}.cpp`
  - `common/processors/file_output/iamf_export_utils/IAMFExportUtil.cpp`
- This crate:
  - `src/encoder.rs`, `src/error.rs`, `src/sequence.rs`, `src/obu/audio_frame.rs`, `src/obu/codec_config.rs`, `src/packing.rs`
  - `tests/encoder_streaming.rs`, `tests/encoder_builder.rs`, `tests/parallax_contract.rs`, `tests/support/parallax_contract.rs`, `tests/error_shape.rs`

**Valid until:** as long as the pins are unchanged.
