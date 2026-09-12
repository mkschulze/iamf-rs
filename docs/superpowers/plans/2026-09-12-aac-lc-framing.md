# AAC-LC IAMF Framing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add typed, normatively constrained AAC-LC IAMF codec configuration and externally framed AAC access-unit authoring.

**Architecture:** Extend the existing `DecoderConfig` family with a compact, lossless-within-the-fixed-profile `AacLcDecoderConfig`, whose 19-byte MPEG-4 `DecoderConfigDescriptor` is parsed and written by the Codec Config OBU layer. Extend the frozen high-level encoder with an AAC-LC frame input variant that accepts only the matching typed configuration; media remains opaque external codec output.

**Tech Stack:** Rust 1.85, existing `BitCursor`/`BitWriter`, existing integration-test suites, no new runtime dependencies.

**Spec:** `docs/superpowers/specs/2026-09-12-aac-lc-framing-design.md`

## Global Constraints

- Implement IAMF v1.1.0 §3.11.2 from the vendored local specification; do not follow draft-v2 `iamf-tools` APIs.
- Add no AAC encoder, decoder, ADTS parser, MP4 muxer, FFI, `unsafe`, or normal dependency.
- The supported constructor rates are exactly 96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000, and 7_350 Hz; reject the escape and reserved AAC indexes.
- The canonical authoring configuration uses `mp4a`, 1024 samples/frame, roll distance -1, `objectTypeIndication = 0x40`, `streamType = 0x05`, `upstream = 0`, `audioObjectType = 2`, `channelConfiguration = 2`, and all three constrained GA flags clear.
- Preserve unsupported, structurally short, or structurally noncanonical foreign `mp4a` configurations as `DecoderConfig::Raw`; write parsed typed values and trailing bytes faithfully.
- All new production behavior starts with a focused failing test, is verified red, then receives the minimal implementation and a green verification.

---

### Task 1: Typed AAC-LC codec configuration

**Files:**
- Modify: `src/obu/codec_config.rs`
- Modify: `src/obu/mod.rs`
- Modify: `src/dump.rs`
- Modify: `tests/descriptors.rs`
- Modify: `tests/round_trip.rs`

**Interfaces:**
- Produces `pub struct AacLcDecoderConfig`, `DecoderConfig::AacLc(AacLcDecoderConfig)`, `CodecConfig::aac_lc(codec_config_id: u32, sample_rate: u32) -> Result<CodecConfig>`, and `CodecConfig::aac_lc_config(&self) -> Option<&AacLcDecoderConfig>`.
- Produces typed `mp4a` parsing/writing in `read_codec_config` and `write_codec_config` for the canonical 19-byte descriptor.
- Consumes `BitCursor`, `BitWriter`, `ErrorKind::SampleRateNotSupportedByCodec`, and the existing finding-based `CodecConfig::validate()` contract.
- Enables Task 2 to match `DecoderConfig::AacLc(_)` without accepting raw `mp4a` configurations.

- [ ] **Step 1: Write the failing AAC-LC model tests before implementation**

  In `tests/descriptors.rs`, import `AacLcDecoderConfig` and add a test that constructs `CodecConfig::aac_lc(2, 48_000)`, wraps it in a Codec Config OBU, and asserts the full bytes below. Also assert the typed accessor exposes sampling-frequency index 3 and `validate()` has no findings.

  ```rust
  #[test]
  fn canonical_aac_lc_config_is_the_v1_1_descriptor() -> iamf::Result<()> {
      let config = CodecConfig::aac_lc(2, 48_000)?;
      assert_eq!(config.codec_id, *b"mp4a");
      assert_eq!(config.num_samples_per_frame, 1024);
      assert_eq!(config.audio_roll_distance, -1);
      assert_eq!(
          obu_bytes(
              &Obu::new(ObuHeader::new(ObuType::CodecConfig), config.clone()),
              write_codec_config,
          ),
          hex!("00 1c 02 6d 70 34 61 80 08 ff ff
                04 0d 40 15 00 00 00 00 00 00 00 00 00 00
                05 02 11 90"),
      );
      assert_eq!(config.aac_lc_config().expect("typed AAC-LC").sampling_frequency_index, 3);
      assert!(config.validate().is_empty());
      Ok(())
  }
  ```

  In the same change, add the short-recovery and semantic-validation cases below. The contradiction test must assert `DecoderConfig::AacLc`, retain each mutation after parsing, and check findings named `object_type_indication`, `stream_type`, `upstream`, `audio_object_type`, `channel_configuration`, `frame_length_flag`, `depends_on_core_coder`, `extension_flag`, `num_samples_per_frame`, and `audio_roll_distance`.

  ```rust
  #[test]
  fn short_aac_decoder_config_stays_raw_and_byte_exact() {
      let bytes = hex!("00 1b 02 6d 70 34 61 80 08 ff ff
                        04 0d 40 15 00 00 00 00 00 00 00 00 00 00
                        05 02 11");
      let mut reader = BitCursor::new(&bytes);
      let parsed = read_obu_with(&mut reader, read_codec_config).expect("short AAC parses");
      assert!(matches!(parsed.payload.decoder_config, DecoderConfig::Raw { .. }));
      assert_eq!(obu_bytes(&parsed, write_codec_config), bytes);
  }

  #[test]
  fn parsed_aac_lc_contradictions_are_preserved_and_diagnosed() {
      let contradictory = CodecConfig {
          codec_config_id: 2,
          codec_id: *b"mp4a",
          num_samples_per_frame: 960,
          audio_roll_distance: 0,
          decoder_config: DecoderConfig::AacLc(AacLcDecoderConfig {
              object_type_indication: 0x41, stream_type: 4, upstream: true,
              buffer_size_db: 0, max_bitrate: 0, avg_bitrate: 0,
              audio_object_type: 3, sampling_frequency_index: 3,
              channel_configuration: 1, frame_length_flag: true,
              depends_on_core_coder: true, extension_flag: true,
          }),
          trailing: Vec::new(),
      };
      let bytes = obu_bytes(&Obu::new(ObuHeader::new(ObuType::CodecConfig), contradictory), write_codec_config);
      let mut reader = BitCursor::new(&bytes);
      let parsed = read_obu_with(&mut reader, read_codec_config).expect("contradictory AAC parses");
      assert!(matches!(parsed.payload.decoder_config, DecoderConfig::AacLc(_)));
      let names: Vec<_> = parsed.payload.validate().iter().map(|finding| finding.at).collect();
      for field in [
          "object_type_indication", "stream_type", "upstream", "audio_object_type",
          "channel_configuration", "frame_length_flag", "depends_on_core_coder",
          "extension_flag", "num_samples_per_frame", "audio_roll_distance",
      ] {
          assert!(names.contains(&Location::Field(field)), "missing finding: {field}");
      }
  }

  #[test]
  fn typed_aac_lc_codec_configs_round_trip() -> iamf::Result<()> {
      let original = Obu::new(ObuHeader::new(ObuType::CodecConfig), CodecConfig::aac_lc(2, 48_000)?);
      let bytes = obu_bytes(&original, write_codec_config);
      let mut reader = BitCursor::new(&bytes);
      let parsed = read_obu_with(&mut reader, read_codec_config)?;
      assert!(matches!(parsed.payload.decoder_config, DecoderConfig::AacLc(_)));
      assert_eq!(obu_bytes(&parsed, write_codec_config), bytes);
      Ok(())
  }
  ```

- [ ] **Step 2: Run the new tests and verify the red state**

  Run: `cargo test --locked --test descriptors canonical_aac_lc_config_is_the_v1_1_descriptor`

  Expected: FAIL at compilation because `CodecConfig::aac_lc`, `aac_lc_config`, and `DecoderConfig::AacLc` do not exist.

  Run: `cargo test --locked --test round_trip typed_aac_lc_codec_configs_round_trip`

  Expected: FAIL because AAC-LC is not registered in the codec parser/writer.

- [ ] **Step 3: Implement the minimal typed descriptor model and codec-OBU integration**

  In `src/obu/codec_config.rs`:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct AacLcDecoderConfig {
      pub object_type_indication: u8,
      pub stream_type: u8,
      pub upstream: bool,
      pub buffer_size_db: u32,
      pub max_bitrate: u32,
      pub avg_bitrate: u32,
      pub audio_object_type: u8,
      pub sampling_frequency_index: u8,
      pub channel_configuration: u8,
      pub frame_length_flag: bool,
      pub depends_on_core_coder: bool,
      pub extension_flag: bool,
  }
  ```

  Add the 13-entry rate/index conversion helper, a 19-byte fixed canonical descriptor reader/writer, and `CodecConfig::aac_lc`. The canonical descriptor must be `04 0d 40 15 00 00 00 00 00 00 00 00 00 00 05 02` followed by the two-byte AAC AudioSpecificConfig. Parse `mp4a` as typed only when its descriptor tags and lengths are `0x04/13` and `0x05/2`; otherwise drain it as the existing raw variant. Match `AacLc` in all accessor, write, roll-distance, validation, and exhaustive-match branches. `validate()` must report wrong fixed semantic values and wrong 1024-frame/negative-one-roll IAMF fields without changing parsed data.

  In `src/obu/mod.rs`, re-export `AacLcDecoderConfig`. In `src/dump.rs`, print every public AAC-LC descriptor field using the existing field helper.

- [ ] **Step 4: Run the AAC-LC model tests and full descriptor suite**

  Run: `cargo test --locked --test descriptors canonical_aac_lc_config_is_the_v1_1_descriptor`

  Expected: PASS.

  Run: `cargo test --locked --test descriptors`

  Expected: PASS with existing descriptor coverage unchanged.

- [ ] **Step 5: Run Task 1 verification and commit**

  Run: `cargo fmt --check && cargo test --locked --test descriptors && cargo test --locked --test round_trip && cargo test --locked --test citations`

  Expected: PASS.

  ```bash
  git add src/obu/codec_config.rs src/obu/mod.rs src/dump.rs tests/descriptors.rs tests/round_trip.rs
  git commit -m "feat: add typed AAC-LC codec framing"
  ```

### Task 2: High-level AAC access-unit submission

**Files:**
- Modify: `src/encoder.rs`
- Modify: `tests/encoder_streaming.rs`
- Modify: `tests/encoder_builder.rs`
- Modify: `tests/public_api.rs`

**Interfaces:**
- Consumes `DecoderConfig::AacLc(_)` and `CodecConfig::aac_lc` from Task 1.
- Produces `FrameInput::AacLc(Vec<u8>)`, accepted only for typed AAC-LC configurations.
- Maintains the existing `ErrorKind::FrameCodecMismatch` and preflight-before-write behavior.

- [ ] **Step 1: Write a failing streaming test for the AAC-LC access-unit path**

  In `tests/encoder_streaming.rs`, add `mono_builder(CodecConfig::aac_lc(0, 48_000)?)` to the frozen-codec loop, rename it to `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`, and map that configuration to `FrameInput::AacLc(vec![0x21, 0x10, 0x04])`. Assert the opaque three-byte payload survives parse-back unchanged.

  Extend the mismatch test so an AAC-LC configuration submitted with `FrameInput::Opus(vec![0xf8])` returns `ErrorKind::FrameCodecMismatch` and leaves `bytes_written()` unchanged.

  In the same test-first change, extend `tests/public_api.rs` so its no-default-features consumer imports `iamf::obu::CodecConfig` and `iamf::encoder::FrameInput`, calls `CodecConfig::aac_lc(7, 48_000)`, and constructs `FrameInput::AacLc(vec![0x21])`.

- [ ] **Step 2: Run the streaming test and verify it fails because `FrameInput::AacLc` is absent**

  Run: `cargo test --locked --test encoder_streaming lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`

  Expected: FAIL at compilation because `FrameInput::AacLc` is not defined.

  Run: `cargo test --locked --no-default-features --test public_api`

  Expected: FAIL at compilation because `FrameInput::AacLc` is not defined.

- [ ] **Step 3: Implement only the matching and payload-lowering branch**

  In `src/encoder.rs`, add:

  ```rust
  /// One externally encoded AAC-LC raw_data_block access unit.
  AacLc(Vec<u8>),
  ```

  Add `(DecoderConfig::AacLc(_), FrameInput::AacLc(_)) => Ok(())` to `validate_frame`; add every AAC/non-AAC mismatch to the existing `FrameCodecMismatch` branch; and include `AacLc(payload)` in `frame_payload`. Do not inspect, decode, or syntactically validate the payload.

- [ ] **Step 4: Run the focused streaming tests and verify green**

  Run: `cargo test --locked --test encoder_streaming lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind wrong_codec_frame_writes_no_temporal_bytes`

  Expected: PASS.

- [ ] **Step 5: Run Task 2 verification and commit**

  Run: `cargo fmt --check && cargo test --locked --test encoder_streaming && cargo test --locked --test encoder_builder`

  Expected: PASS.

  ```bash
  git add src/encoder.rs tests/encoder_streaming.rs tests/encoder_builder.rs tests/public_api.rs
  git commit -m "feat: accept framed AAC-LC access units"
  ```

### Task 3: User documentation

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes the finished typed configuration and `FrameInput::AacLc` from Tasks 1–2.
- Produces accurate README wording without expanding this crate's codec implementation boundary.

- [ ] **Step 1: Document the precise consumer boundary**

  Update the README sentence “Frames are either LPCM bytes or already encoded FLAC/Opus access units” to include AAC-LC. State that AAC-LC input is one pre-encoded `raw_data_block()` per IAMF Audio Frame and that this crate neither encodes nor decodes AAC.

- [ ] **Step 2: Run final checks and commit**

  Run: `cargo fmt --check && cargo test --locked --no-default-features --test public_api && cargo test --locked && cargo clippy --locked --all-targets -- -D warnings`

  Expected: PASS with no new normal dependency.

  ```bash
  git add README.md
  git commit -m "docs: describe AAC-LC framing boundary"
  ```
