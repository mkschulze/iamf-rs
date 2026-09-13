# Deferred items — quick 260913-n56

None of items 1-6 is implemented in this task. Each entry gives its source, the current state, and why
it is out of scope.

## 1. Opus `pre_skip` vs total start trim

- **Source:** IAMF v1.1.0 `index.bs:1819` ("Pre-skip SHALL be the same as the number of audio samples
  to be trimmed at the start of coded Audio Substreams") requires **equality**.
  `iamf-tools@v2.1.0` `iamf/cli/proto_conversion/proto_to_obu/audio_frame_generator.cc:141-156`
  requires only **>=** ("The encoder requires <n> samples trimmed at the start but only <m> were
  requested").
- **Current state:** the encoder does not relate `pre_skip` to start trims at all.
- **Why out of scope:** the references disagree and this needs a user decision (== or >=). The rule is
  stateful across the whole start-trim run and needs a `finish()` check for all-trimmed or empty
  streams. Enforcing it would break `tests/encoder_streaming.rs:141-185` (Opus `pre_skip` 312, no
  trim).

## 2. `validate()` findings for P2-P5, T3 and T4 on parsed sequences

- **Source:** research 260913-n56 section D.
- **Current state:** the rules live only in `EncoderBuilder::build()` and `EncodingWriter`.
  `parse_sequence(..).validate()` (`src/sequence.rs`) reports none of them.
- **Why out of scope:** findings need temporal-unit grouping inside `validate()`, which does not exist
  today. **P1 must never become a finding**: it is an iamf-tools limitation, not a spec rule.
  The low-level writers must keep round-tripping foreign spec-legal files byte-exactly.

## 3. Builder accepts Stereo with 2 uncoupled substreams

- **Source:** executed during research: pinned `decoder_main` rejects `LoudspeakerLayout::Stereo` with
  `substream_count 2, coupled 0`:
  `INVALID_ARGUMENT: Coupled substream count different from the required number. In OBU: 0 vs expected: 1`.
  See `iamf-tools@v2.1.0` `iamf/cli/obu_with_data_generator.cc` `ValidateSubstreamCounts`.
- **Current state:** `validate_element_topology` (`src/encoder.rs`) checks only channel-count
  equality. The stereo builders in `tests/encoder_streaming.rs` use this shape.
- **Why out of scope:** it is a separate topology rule and needs its own task (and a fixture change
  for the streaming tests).

## 4. Spec-legal scaled `parameter_rate` is unsupported (spec vs iamf-tools disagreement)

- **Source:** IAMF v1.1.0 `index.bs:863-865` allows any `parameter_rate` that yields a non-zero integer
  tick count per frame; `index.bs:1916` uses 480 ticks at 24 kHz as its example.
  `iamf-tools@v2.1.0` `iamf/cli/rendering_mix_presentation_finalizer.cc`
  `GetParameterBlockLinearMixGainsPerTick` rejects any rate other than the output sample rate, and
  `iamf/cli/global_timing_module.cc` `GetNextParameterBlockTimestamps` compares ticks with samples
  unscaled (TODO b/283281856). Executed research probes c, m and n all fail in pinned `decoder_main`,
  probe n even with zero Parameter Blocks.
- **Current state:** P1 in `build()` requires `parameter_rate == output sample rate`, per the standing
  tie-break rule (iamf-tools@v2.1.0 with pinned libiamf decides). Recorded in a DISAGREEMENT comment in
  `validate_declarations`.
- **If the spec is ever preferred:** replace P1 with the integer-ticks rule and scale P2/P3 as
  `duration * sample_rate == parameter_rate * num_samples_per_frame` using checked `u64`
  multiplication. Output would then fail CONF-06 whenever the rate differs.

## 5. Demixing and recon-gain duration rules

- **Source:** IAMF v1.1.0 `index.bs:778-782, 788-792` (rate equals element sample rate, mode 0,
  `duration == num_samples_per_frame`, one subblock). iamf-tools
  `audio_element_generator.cc:148-155, 166-173` has the matching errors.
- **Current state:** unreachable: the builder rejects audio element params (`UnsupportedParameterData`)
  and `recon_gain_is_present`.
- **Why out of scope:** nothing to enforce until the builder accepts those parameters.

## 6. Informational: CONF-06 is a single temporal unit

- **Finding:** a one-unit file with a wrong Parameter Block duration still passes `decoder_main`
  (timestamps 0 == 0), so CONF-06 alone would not catch a duration regression. The in-crate rules and
  their tests are the guard.
- **Possible follow-up:** a multi-unit `decoder_main` fixture.

## 7. Resolution note

- **Resolved:** this task resolves 260913-lta deferred items 2 (`index.bs:1915`, Parameter Block
  duration equals Audio Frame duration) and 3 (`index.bs:1913`, same trimming information). Uniform
  trimming within a unit was already structural (one `trimming` per `TemporalUnitInput`); placement
  across units (`index.bs:541-542`) is now enforced. The lta file is not edited.
