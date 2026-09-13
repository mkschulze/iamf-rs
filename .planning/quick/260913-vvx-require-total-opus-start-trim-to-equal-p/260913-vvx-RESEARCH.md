# Quick 260913-vvx: Require total Opus start trim to equal `pre_skip` (Research)

**Researched:** 2026-09-13
**Domain:** IAMF v1.1.0 Opus-specific constraints and the start-trim run in `EncodingWriter`
**Confidence:** HIGH (spec text, reference code and crate code were read this session; decoder behaviour was probed with the pinned `decoder_main`)

"VERIFIED" means read or executed this session. "INFERRED" means reasoned from code that was read but not executed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **User decision (2026-09-13): "ja, mit genau gleich umsetzen"** ("yes, implement with exactly equal").
  - The total number of samples trimmed at the start of an Opus IA sequence SHALL equal the Codec Config's `pre_skip`.
  - Source: IAMF v1.1.0 `index.bs:1819`, "Pre-skip SHALL be the same as the number of audio samples to be trimmed at the start of coded Audio Substreams".
  - This satisfies iamf-tools@v2.1.0 `audio_frame_generator.cc:141-156`, which only requires the trim to be at least `pre_skip`. The spec wins where it is stricter.
- **Scope:** Opus Codec Configs only. LPCM and FLAC are unaffected. AAC-LC analogue: record only.
- **Placement:** high-level encoder path (`EncodingWriter` / `finish()`). The low-level writers must keep round-tripping foreign files byte-exactly. A `validate()` finding is allowed only if a hash changes solely for pinned `is_valid: false` fixtures or with a full explanation; otherwise defer.
- **Errors:** payload-free `ErrorKind` variants, keeping the 32-byte assertion.

### Claude's Discretion
- When a mismatch is detected (first unit without a start trim, `finish()`, or both), and how to handle streams that are trimmed entirely or are empty.
- Test layout.

### Deferred Ideas (OUT OF SCOPE)
- None listed in CONTEXT.md. Gates: fmt, clippy (with and without `fuzzing`), `cargo test --locked`, fuzz replay, conformance. Golden fixtures and `DIFF-LEDGER.md` stay unchanged.
</user_constraints>

## Summary

1. **The rule is a whole-run total, and it spans units.** `index.bs:1819` compares `pre_skip` with the samples trimmed at the start of the coded substream. `:541` says that a start trim can only continue through frames that are fully trimmed. So the quantity is the sum of `num_samples_to_trim_at_start` over the leading run: zero or more units with `at_start == num_samples_per_frame`, followed by at most one closing unit with `at_start < N`. Offsets are in 48 kHz samples (`:1825`). `:1913` requires every substream to carry the same trims, and the builder writes one trim per unit, so one running total covers all substreams.
2. **No decoder ever reads `pre_skip`.** Neither libiamf@f06e919e nor iamf-tools `decoder_main` uses it. Both apply only the OBU trim fields. Probe (VERIFIED): `test_000059.iamf` with `pre_skip` patched to 0, 100, 312 or 624 gives the same WAV hash (`f556119f…`) in all four cases. The consequence of a mismatch is therefore silent: a trim smaller than the codec delay leaves priming samples in the output, and a larger one removes real audio. The crate is the only guard.
3. **No committed fixture bytes change.** `phase3_opus` trims 312 in unit 0 with `pre_skip = L = 312`. It is written by the low-level `SequenceWriter`, so the new rule does not reach it anyway. The only Opus `EncodingWriter` user is `tests/encoder_streaming.rs:155` (`pre_skip 312`, `trimming: None`). That test must change to trim 312.
4. **Defer the parse-side finding.** Three committed positive iamf-tools fixtures have a total start trim of 0 while `pre_skip` is 312 or 120 (measured). A finding would change their `semantic_sha256`. They have no `is_valid: false` marking, and `decoder_main` decodes them, so the CONTEXT rule says defer.

**Primary recommendation:** add `start_trim_total: u32` to `TemporalProgress`. Check it in `preflight` right after T3/T4, with two new payload-free kinds: `OpusStartTrimExceedsPreSkip`, raised as soon as the total goes over `pre_skip`, and `OpusStartTrimShortOfPreSkip`, raised when the run closes short. Add a matching check in `finish()` for runs that are still open, which covers empty streams and all-trimmed streams.

## A. Rule text (VERIFIED: `git -C docs/iamf show v1.1.0:index.bs`)

- `:1816-1821` Opus: "The [=DecoderConfig()=] class for OPUS conforms to [=ID Header=] with [=ChannelMappingFamily=] = 0 … - [=Pre-skip=] SHALL be the same as the number of audio samples to be trimmed at the start of coded [=Audio Substream=]s. - [=Output Gain=] SHALL NOT be used. In other words, it SHALL be set to 0 dB."
- `:1825` "The sample rate used for computing offsets SHALL be 48 kHz."
- `:646` `audio_roll_distance` "SHALL be set to \(-R\) when [=codec_id=] is set to <code>Opus</code>" (already derived by `CodecConfig::opus`).
- `:532` "If an [=Audio Frame OBU=] has its [=num_samples_to_trim_at_start=] field set to a non-zero value N, the decoder SHALL discard the first N audio samples."
- `:537` "the sum of [=num_samples_to_trim_at_start=] and [=num_samples_to_trim_at_end=] SHALL be less than or equal to … [=num_samples_per_frame=]".
- `:541` "When [=num_samples_to_trim_at_start=] is non-zero, all [=Audio Frame OBU=]s with the same [=audio_substream/audio_substream_id=], and preceding this OBU back until the [=Codec Config OBU=] …, SHALL have their [=num_samples_to_trim_at_start=] field equal to … [=num_samples_per_frame=]."
- `:408` "The presentation start time of an [=Audio Substream=] is the presentation start time of its first [=Audio Frame OBU=] which is not entirely trimmed."
- `:1913` "Every [=Audio Substream=] … SHALL have the same trimming information."

**Interpretation (INFERRED, direct from `:532` and `:541`):**
- The total start trim is `Σ at_start` over the leading run, per substream, in 48 kHz samples.
- `:537` caps each frame at N, so a `pre_skip > N` must be spread as `⌊P/N⌋` fully trimmed units plus one unit trimming `P mod N`.
- This is exactly iamf-tools' `min(num_samples_in_frame, remaining)` split (`audio_frame_generator.cc:363-372`).
- If `P mod N == 0` (e.g. `noise_3s_stereo_opus`: P = N = 120), the run ends with full units and the next unit carries `at_start == 0`.

**AAC-LC (record only):** `:1827-1842` has no pre-skip or trim-equality constraint, and the crate has no AAC `DecoderConfig`. FLAC (`:1844-1861`) and LPCM have none either. VERIFIED by reading those sections.

## B. References

| Source | Finding | Status |
|---|---|---|
| iamf-tools@848c6ff4 `audio_frame_generator.cc:141-156` `ValidateUserStartTrimIncludesCodecDelay` | Rejects only `user_samples_to_trim_at_start < encoder_required_samples_to_delay` ("The encoder requires … samples trimmed at the start but only … were requested"). This is **>=**. | VERIFIED |
| same, `:207-211, :733-747` | Total OBU start trim = user value (`includes_codec_delay`, proto default `true`, `audio_frame.proto:183`), or user value + codec delay (when `false`) | VERIFIED |
| `opus_encoder.cc:121-139` | `ValidateEqual(pre_skip_, lookahead, "Opus \`pre_skip\`")`: `pre_skip` must equal the libopus lookahead, not the trim | VERIFIED |
| `codec_config.cc:346-366` `SetCodecDelay` | Opus-only setter for `pre_skip_`; LPCM, FLAC and AAC "does not have a field for codec delay" | VERIFIED |
| `iamf/cli/codec/opus_decoder.cc` | Checks only `output_gain` and `mapping_family` (`:41-49`); never reads `pre_skip` | VERIFIED (grep of the whole pinned tree: `pre_skip` appears only in the encoder, the config I/O and the generator) |
| libiamf@f06e919e `IAMF_opus_decoder.c:140-146` | The Opus vtable has no `.info`, so `ctx->delay` stays 0 (`IAMF_core_decoder.c:145` `IAMF_MALLOCZ`, `:283-287`). Decoder `delay` is only non-zero for AAC (`aac_multistream_decoder.c:144`). `pre_skip` is only logged (`vlogging_tool_sr.c:461`). | VERIFIED by reading. INFERRED: an Opus mismatch gives leftover priming or over-trimming, never an error. |
| Eclipsa `IAMFExportUtil.cpp:124-130`, `AudioElement.cpp:179-181` | `pre_skip 312`, `automatically_override_codec_delay(true)`, user trim 0 with `includes_codec_delay(false)`. So OBU start trim = codec delay = `pre_skip`: **equal**. | VERIFIED |
| iamf-tools testdata (all `CODEC_ID_OPUS` textprotos) | Every valid vector has trim 312 with `pre_skip` 312 (`test_000035`: 120/120). `test_000014` (pre_skip 0, trim 0) is `is_valid: false`. | VERIFIED |

**Executed probe** (pinned `iamf-tools:v2.1.0` `decoder_main`, scratch copies under `scratchpad/vvx/probe_dm/`):

| Case | Input | Result |
|---|---|---|
| a | `iamf-tools/noise_1024samp_5p1_opus.iamf` (trim 0, pre_skip 312, end 584) | `Decoded 2 temporal units.`; WAV data = 1336 frames = 1024 + 312 priming |
| b, c, d, e | `test_000059.iamf` with `pre_skip` bytes 22-23 set to 100 / 624 / 312 (original) / 0 | Each gives `Decoded 26 temporal units.` and the identical sha256 `f556119f…ac13` |

## C. Crate integration (VERIFIED: `src/encoder.rs`, `src/error.rs`, `src/obu/codec_config.rs`)

- `TemporalProgress` (`src/encoder.rs:223-241`): `parameter_ids`, `start_trim_open` (initially `true`), `end_trimmed`. `push_temporal_unit` commits `next` only after `sequence.push_temporal_unit` succeeds (`:260-266`).
- `preflight` (`:276-430`): the frames loop runs `validate_temporal_trimming` (`:583-596`, `at_start + at_end <= N`, else `TemporalUnitTrimMismatch`). Then T4 (`:322-327`), T3 (`:328-333`), and `start_trim_open = progress.start_trim_open && Some(at_start) == self.frame_size()` (`:336`). Parameter checks follow.
- `finish(self)` (`:272-274`) only calls `self.sequence.finish()`, which checks poisoning and then flushes (`src/sequence.rs:908-917`).
- `frame_size()` (`:433-438`) reads `codec_configs.first()`. `build()` rejects more than one Codec Config (`MultipleCodecConfigs`, `:986-991`), so there is exactly one config and one `pre_skip`.
- `OpusDecoderConfig.pre_skip: u16` (`src/obu/codec_config.rs:135-144`). `build()` runs `validate_findings(config.validate())` (`:972`), and `validate` emits a finding for `pre_skip == 0` (`codec_config.rs:536-540`). So in the encoder path **P ≥ 1**, and a stream with no trim can never satisfy the rule.
- `ErrorKind` is `#[non_exhaustive]` (`src/error.rs:43-45`). The n56 kinds sit at `:196-199`. `const _: () = assert!(size_of::<Error>() <= 32);` (`:278`). Unit variants add no size.

**Design (recommended):**
1. Add `start_trim_total: u32` to `TemporalProgress` (initial 0). Add `fn opus_pre_skip(&self) -> Option<u16>`, a sibling of `frame_size()`, that matches `DecoderConfig::Opus(o)` on `codec_configs.first()`.
2. In `preflight`, after the T3 check and `start_trim_open` computation (keeping n56 ordering, so T1/T3/T4 tests keep their kinds), and before the parameter-block loop:
   ```rust
   // ref: IAMF v1.1.0 index.bs:1819 "Pre-skip SHALL be the same as the number of audio samples
   //      to be trimmed at the start of coded Audio Substreams"
   // ref: IAMF v1.1.0 index.bs:541 (the start trim spans fully trimmed frames, then one remainder)
   // ref: iamf-tools@v2.1.0 iamf/cli/proto_conversion/proto_to_obu/audio_frame_generator.cc ValidateUserStartTrimIncludesCodecDelay
   // DISAGREEMENT: iamf-tools requires only trim >= codec delay; the spec requires equality (user decision 260913-vvx).
   let mut start_trim_total = self.progress.start_trim_total;
   if let (Some(pre_skip), true) = (self.opus_pre_skip(), self.progress.start_trim_open) {
       start_trim_total = start_trim_total.checked_add(at_start)
           .filter(|total| *total <= u32::from(pre_skip))
           .ok_or_else(|| temporal_input(ErrorKind::OpusStartTrimExceedsPreSkip, "trimming"))?;
       if !start_trim_open && start_trim_total != u32::from(pre_skip) {
           return Err(temporal_input(ErrorKind::OpusStartTrimShortOfPreSkip, "trimming"));
       }
   }
   ```
   Commit `start_trim_total` in the returned `TemporalProgress`. Once the run has closed, T3 already rejects any later `at_start > 0`, so the total stays frozen.
3. `finish(self)`: call `self.sequence.check_not_poisoned()?` first, so poisoning keeps priority. Then, if the writer has an Opus config, `progress.start_trim_open` is still true, and `start_trim_total != pre_skip`, return `OpusStartTrimShortOfPreSkip` at `Field("trimming")`. Otherwise call `self.sequence.finish()`.
   - This covers the empty stream (total 0 < P ≥ 1) and a stream whose every unit is fully trimmed but falls short.
   - All-trimmed with total == P (e.g. P = N = 120, one unit) is accepted: equality holds.
   - The sink is dropped on this error, just as it already is for `SinkWrite`/poisoned errors.
4. Error kinds (add in `src/error.rs` after `TemporalUnitAfterEndTrim`):
   - `#[error("the total num_samples_to_trim_at_start exceeds the Opus pre_skip")] OpusStartTrimExceedsPreSkip`
   - `#[error("the start trim ended before reaching the Opus pre_skip")] OpusStartTrimShortOfPreSkip`
   - Add both to `tests/error_shape.rs::temporal_input_errors_are_compact_typed_kinds` (`:193-215`).
5. Update the doc comment on `TemporalUnitInput::trimming` (`:204-209`) and on `finish()` to state the Opus rule.

## D. Fixtures and tests (VERIFIED)

| Usage | Path | Start trim vs `pre_skip` | Impact |
|---|---|---|---|
| Streaming Opus kind test | `tests/encoder_streaming.rs:143-185` (`mono_builder(CodecConfig::opus(0, 960, 48_000, 312))`, `trimming: None` at `:170`) | 0 vs 312 | **Fails** under the new rule. The loop pushes all three codecs with one `trimming: None`. Migrate: Opus iteration uses `Some(Trimming { at_start: 312, at_end: 0 })`, LPCM and FLAC stay `None`. Test-only change, no fixture bytes. |
| `fixture::opus()` / `phase3_opus` | `tests/support/fixture.rs:1058-1099`, trims at `:530-543` | TU0 `at_start = lookahead` (MANIFEST `L = 312`) = `pre_skip` (`u16::try_from(corpus.lookahead)`, `:1064`); last unit `at_end = E = 1` | **Equal already.** Written through `SequenceWriter` (`fixture.rs:750`), not `EncodingWriter`, so it is untouched. CONF-06 is unaffected. |
| Builder-only Opus | `tests/encoder_builder.rs:712, 1212` | no temporal units | none |
| Proptest / fuzz Opus configs | `tests/support/sequence_cases.rs:84-87`, `tests/fuzz_regression.rs:153` | low-level models only | none |
| Parallax contract | `tests/support/parallax_contract.rs:99-120` | "Opus" candidate lowers onto LPCM | none |
| `tools/codec-fixtures` | produces packets and MANIFEST only | — | none |
| `descriptors.rs:524`, `error_shape.rs:218` | config construction only | — | none |

**No committed fixture bytes, hashes, goldens or `DIFF-LEDGER.md` change.** No stop condition applies.

## E. Parse-side `validate()` finding: defer

Measured with a scratch probe crate (path dependency, `parse_sequence`, summing leading `at_start` per substream):

| Fixture | pre_skip / N | Total start trim | Metadata | A finding would change the hash? |
|---|---|---|---|---|
| `test_000059.iamf` | 312 / 960 | 312 on all 4 substreams | `is_valid: true` | no (equal) |
| `test_000124.iamf` | 312 / 960 | 312 on both substreams | `is_valid: false` | no (equal) |
| `iamf-tools/noise_1024samp_5p1_opus.iamf` | 312 / 960 | **0** (only end 584) | no textproto; MANIFEST "valid iamf-tools-produced fixture" | **yes** |
| `iamf-tools/noise_3s_stereo_opus.iamf` | 120 / 120 | **0** (end 96 on unit 86) | same | **yes** |
| `iamf-tools/tones_100ms_3OA_stereo_opus.iamf` | 312 / 960 | **0** (end 648) | same | **yes** |

- `semantic_sha256` hashes `format!("{sequence:#?}\nfindings={:#?}", sequence.validate())` (`tests/parse_reference.rs:91-99`).
- A finding would therefore change three positive hashes whose fixtures are not marked `is_valid: false`, and `decoder_main` decodes them (probe a).
- The same three files are permanent fuzz seeds (`fuzz/corpus/parse_sequence/`). A finding does not affect replay, but it adds noise.
- **Recommendation: defer.** The finding also needs temporal-unit grouping and substream-run tracking inside `validate()`, which is the still-deferred n56 item 2.
- Record a deferred item: these three upstream files predate or bypass the codec-delay trim, and are spec-invalid under `:1819` but decoder-accepted.

## F. Recommendation

**Implementation:** as in C.1-C.5. No new dependencies (no package legitimacy audit needed).

**Tests** (in `tests/encoder_streaming.rs`, using `mono_builder(CodecConfig::opus(…))` and `FrameInput::Opus(vec![0xf8])`, each asserting kind, `Location::Field("trimming")`, and unchanged `bytes_written()` on rejection):

Rejections:
1. Missing trim: N = 960, P = 312, TU0 `None` → `OpusStartTrimShortOfPreSkip`.
2. Too short: TU0 `at_start 311` → `ShortOfPreSkip`.
3. Too long in one unit: TU0 `at_start 313` → `ExceedsPreSkip`.
4. Too long across units: N = 120, P = 312, TUs `120, 120`, then `120` → `ExceedsPreSkip` on the third.
5. Multi-unit short: N = 120, P = 312, TUs `120, 120`, then `71` → `ShortOfPreSkip`.
6. `finish()` checks:
   - an empty stream fails with `ShortOfPreSkip`;
   - an all-fully-trimmed short stream (N = 120, P = 312, TUs `120, 120`, then finish) fails the same way.
7. Rejected pushes do not advance the state: after rejection 3, pushing `at_start 312` succeeds.

Positives:
- Exactly 312 in one unit, followed by `None` units and an end trim, round-trips through `parse_sequence`.
- `pre_skip > N` spanning units: N = 120, P = 312 with `120, 120, 72`, then `None`.
- `P == N`: N = 120, P = 120 with `120`, then `None`. Also a single fully trimmed unit followed directly by `finish()`.
- LPCM and FLAC with no trim still finish (regression guard for the codec gate).

**Migration:** `lpcm_flac_and_opus_inputs_follow_the_frozen_codec_kind` gets a per-codec trim, and Opus trims 312.

**Deferred items:**
- (1) The parse-side finding (E), with the three affected fixtures named.
- (2) iamf-tools' `>=` vs spec `==`, as a DISAGREEMENT comment in code.
- (3) An executed `EncodingWriter` Opus multi-unit CONF-06 probe is optional. `decoder_main` ignores `pre_skip`, so it cannot catch a regression.

## Common Pitfalls

- **Checking per unit instead of per run:** rejects valid `pre_skip > N` streams. Accumulate over the run.
- **Checking only at `finish()`:** errors come late and the sink is lost after many units. Detect "exceeds" and "closed short" at push time, and keep `finish()` for open runs only.
- **Mutating progress before the push succeeds:** breaks the n56 convention. Compute the new total into `next` only.
- **Placing the check before the frames loop or before T3/T4:** existing tests would change kind, e.g. `trims_exceeding_the_frozen_frame_size…`, which expects `TemporalUnitTrimMismatch`.
- **`u32` overflow in the total:** use `checked_add` and map `None` to `ExceedsPreSkip` (`clippy::arithmetic_side_effects`).
- **Trusting `decoder_main`/libiamf to catch it:** both ignore `pre_skip` (probe b-e).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | An empty Opus stream (zero units) should be rejected at `finish()`, because a zero start trim is not equal to `pre_skip` ≥ 1 | C.3 | A caller who writes descriptors only gets an error. Alternative: exempt zero units. User may prefer the exemption. |
| A2 | An all-trimmed stream whose total equals `pre_skip` is accepted | C.3 | Low; equality literally holds |
| A3 | libiamf leftover-priming behaviour on mismatch is inferred from reading, not executed (host libiamf lacks Opus) | B | Informational only |
| A4 | Error kind names and messages are proposals | C.4 | Cosmetic |

## Open Questions

1. **Empty Opus stream at `finish()`: reject (recommended, strict reading) or exempt?** This is Claude's discretion per CONTEXT. The planner may proceed with reject and note it in SUMMARY.

## Security Domain

- The change applies to caller input that has already been validated structurally. It adds no parsing surface.
- ASVS V5 (input validation) applies: checked integer arithmetic, no panics, typed errors, and the sink stays at the previous unit boundary on rejection.
- No authentication, session, access-control or cryptography surface.

## Sources

- IAMF spec `v1.1.0:index.bs` lines 408, 532, 537, 541, 646, 1816-1825, 1827-1842, 1913
- iamf-tools@848c6ff4: `audio_frame_generator.cc:68,141-156,185-233,363-372,725-747`; `codec/opus_encoder.cc:121-139`; `codec/opus_decoder.cc:41-49`; `obu/codec_config.cc:346-366`; `cli/proto/audio_frame.proto:176-185`; `cli/testdata/*.textproto` (Opus set); `cli/testdata/iamf/README.md`
- libiamf@f06e919e: `IAMF_opus_decoder.c:140-146`; `IAMF_core_decoder.c:145,283-287`; `IAMF_decoder.c:2420-2455`; `aac/aac_multistream_decoder.c:144`
- Eclipsa@c9646092: `IAMFExportUtil.cpp:124-130`; `AudioElement.cpp:175-184`
- Crate: `src/encoder.rs:196-274,276-438,583-596,951-991`; `src/error.rs:43-45,196-199,278`; `src/obu/codec_config.rs:135-144,293-326,536-540`; `src/sequence.rs:908-917`; `tests/encoder_streaming.rs:143-185,560-668,735-777`; `tests/support/fixture.rs:419-575,750,1058-1099`; `tests/parse_reference.rs:91-99`; `tests/support/reference_expectations.rs:226-282`; `tests/error_shape.rs:193-215`; `tests/fixtures/codecs/opus/MANIFEST.md`
- Executed: scratch probe crate (trim totals) and pinned `iamf-tools:v2.1.0` `decoder_main` (probes a-e)

**Valid until:** stable (pinned refs); revisit if the builder ever admits multiple Codec Configs or redundant descriptors.
