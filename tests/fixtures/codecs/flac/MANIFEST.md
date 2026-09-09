# Fixed-block FLAC fixture

flacenc = 0.5.1
claxon = 0.4.3
opus = 0.4.0
sha2 = 0.10.9
sample_rate = 48000
channels = 2
bits_per_sample = 16
block_size = 128
S = 300
P = 0
E = 84
padded = 384
packet_order = packet-000.bin,packet-001.bin,packet-002.bin
stereo_coding = independent
subframe_coding = verbatim
flacenc_default_features = false
multithread = false

Source: deterministic interleaved L/R signed 16-bit little-endian PCM.
sample(channel, index) = ((channel * 104729 + index * 1299709) % 32769) - 16384.
S is source frames, P is priming, E is zero padding and final end trim;
S + P + E = 384 = 3 * 128. Source and expected PCM are identical, 300 stereo frames.
Encoder disables left/side, right/side, mid/side, constant, fixed and LPC subframes;
all other Encoder settings are flacenc 0.5.1 defaults (unused for Verbatim).
Each packet is serialized separately via Stream::frame(i), Frame::header and
BitRepr::write. No packet contains a stream marker or metadata.
Root checks are intentionally limited to this fixed Verbatim corpus, with CRCs,
boundaries and sample equality; they are not a general-purpose FLAC decoder.
The excluded verifier reconstructs a temporary stream in memory and uses Claxon
to decode all 384 frames before checking the final 84 zero frames and trimming.
Temporary STREAMINFO describes stereo and 384 samples; IAMF Codec Config uses
its canonical channel/unknown-length fields from CodecConfig::flac instead.

Run from the repository root (relative paths resolve there):

```sh
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact generate_flac_corpus
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact verify_flac_corpus
```

sha256.source.s16le = 059b72d5d22b70a90b6facfe69e315bbb1a804d2e82e11f268e97bf80644ede8
sha256.expected.s16le = 059b72d5d22b70a90b6facfe69e315bbb1a804d2e82e11f268e97bf80644ede8
sha256.packet-000.bin = a5f58f2819bad971f2c74bd4244daf0c1f948a4293abfa66e17b38b311072da1
sha256.packet-001.bin = c65b9933b88a16677901dbd4888b623d5ab109c5bb8415cadd44bcf94ba7532c
sha256.packet-002.bin = e63ddd09600f69a1e35ec0e4a71fe9d98c91918425edf57d3b59bff38e0302aa
