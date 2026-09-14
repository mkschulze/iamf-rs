# AAC-LC IAMF Framing Design

## Scope

Add AAC-LC to `iamf-rs` at the same typed IAMF wire-model and authoring
boundary as LPCM, FLAC, and Opus. The crate continues to accept already
encoded media payloads; it will not encode or decode AAC, parse ADTS, or mux
MP4.

The normative source is the vendored IAMF v1.1.0 specification,
`../iamf-decode-rs/docs/iamf/v1.1.0.html`, section 3.11.2.

## Public model

`AacLcDecoderConfig` becomes a public typed variant of `DecoderConfig`.
It represents the constrained values exposed by IAMF rather than a generic
MPEG-4 descriptor: the AAC sampling rate and the fixed 1024-sample AAC-LC
frame shape. Its constructor creates the exact MP4 `DecoderConfigDescriptor`
bytes mandated by the specification.

`CodecConfig::aac_lc(codec_config_id, sample_rate)` constructs a conformant
codec configuration with:

- `codec_id = "mp4a"`;
- `num_samples_per_frame = 1024`;
- `audio_roll_distance = -1`; and
- the canonical typed AAC-LC decoder configuration.

The supported sample rates are the 13 non-reserved rates represented by the
AAC `samplingFrequencyIndex` table: 96_000, 88_200, 64_000, 48_000, 44_100,
32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000, and 7_350 Hz. The
escape index (15) and reserved indexes (13–14) are not authored. Invalid rates
return the existing typed sample-rate error.

`FrameInput::AacLc(Vec<u8>)` accepts exactly one externally produced AAC
`raw_data_block()` for each audio frame. It is an opaque access unit: the wire
crate cannot inspect codec payload syntax and does not claim that it can.

## Decoder-config parsing and validation

The parser recognizes a complete AAC-LC descriptor as `DecoderConfig::AacLc`.
It preserves a short or structurally unrecognized `mp4a` decoder config as
`DecoderConfig::Raw`, matching existing FLAC and Opus recovery behavior.

For parsed typed values, validation reports each violated IAMF constraint
without normalizing it away:

- MP4 descriptor `objectTypeIndication = 0x40`, `streamType = 0x05`, and
  `upstream = 0`;
- AAC `audioObjectType = 2` and `channelConfiguration = 2`;
- GA-specific `frameLengthFlag = 0`, `dependsOnCoreCoder = 0`, and
  `extensionFlag = 0`;
- the configuration sample rate and IAMF `num_samples_per_frame = 1024`; and
- `audio_roll_distance = -1`.

The writer serializes typed parsed values byte-for-byte and preserves trailing
decoder-config bytes according to the existing OBU remainder rule.

## Encoder integration

The high-level builder admits only the typed AAC-LC configuration and requires
`FrameInput::AacLc` for substreams using it. A mismatched input variant is a
typed error before any temporal-unit bytes are emitted. Existing LPCM, FLAC,
and Opus behavior remains unchanged.

## Tests and evidence

Tests are added before production code and prove:

1. Canonical descriptor bytes and a codec-config OBU round-trip.
2. A canonical constructor derives the IAMF constants.
3. Each malformed descriptor field is retained by parsing and reported by
   validation, while short known data remains raw and byte-exact.
4. The streaming encoder accepts an AAC raw access unit and rejects cross-codec
   frame input before output mutation.
5. The public API remains usable with no default features.

An AAC bitstream fixture is not required for this change: byte-level descriptor
and access-unit framing are in scope, while AAC compression correctness belongs
to a codec implementation. A later conformance phase may add an externally
encoded AAC fixture and a pinned reference decoder oracle.
