# Exact raw Opus fixture

opus = 0.4.0
opusic-sys = 0.7.5
libopus = libopus 1.6.1
sha2 = 0.10.9
sample_rate = 48000
channels = 2
frame_samples = 960
application = audio
bitrate = 128000
vbr = false
complexity = 10
force_channels = stereo
max_bandwidth = fullband
signal = music
dtx = false
inband_fec = false
packet_loss_perc = 0
lsb_depth = 16
L = 312
S = 1607
P = 2
E = 1
padded = 1920
packet_order = packet-000.bin,packet-001.bin
Source: deterministic interleaved L/R signed 16-bit little-endian PCM, zero-padded after S.
sample(channel, index) = ((channel * 104729 + index * 1299709) % 24577) - 12288.
All source samples have absolute value <= 12288 (-8.5 dBFS), below the limiter threshold.
The encoder is configured before querying L. S is selected after that query as
two 960-frame packets minus L and one trailing padding frame; P is ceil((S + L)/960), and E is one.
Each packet is raw Opus data only: no Ogg container and no OpusHead.
expected.s16le is independently decoded by a fresh decoder from all P packets,
then trimmed by L leading and E trailing stereo frames; source.s16le is provenance only.
libiamf-expected.s16le is the exact 16-bit PCM decoded from the same IAMF fixture
by pinned libiamf v1.1.0 (f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63) in the
reference workflow.  It differs from expected.s16le only at interleaved samples
887 (1433 -> 1432), 1642 (-5421 -> -5422), 2798 (-1473 -> -1474), and
3008 (-7284 -> -7285), all one-LSB rounding differences.  The reference gate
compares against this pinned libiamf oracle exactly; the excluded fixture tool
continues to authenticate expected.s16le independently.

Run from the repository root (relative paths resolve there):

```sh
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact generate_opus_corpus
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact verify_opus_corpus
```

sha256.source.s16le = 6a77c6d63d50050a3ba0d94fb929186dc65cb3a546b5d31bd79dfd6c501de48f
sha256.expected.s16le = a8e6e9ab27992ecfa3bcb2de1095634041640ff0efd74534970b2a05ffabaa2e
sha256.libiamf-expected.s16le = 9b3c06e94ed47cc5a99fe0cd18bc63d841f772ba0308950c1ecba0b5db2891ae
packet_len.packet-000.bin = 320
sha256.packet-000.bin = 6be750d461fbf35144932c497581e521c5cf7b0b0dcd1d045c042616ffcecd59
packet_len.packet-001.bin = 320
sha256.packet-001.bin = 5d45a91fd85cdefe94afd8a0c25ae8e0622c5ad54fa21d21b08bd5d50a7be0b7
