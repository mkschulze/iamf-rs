//! Explicit, excluded integration tests. Root tests only consume the results.
mod support;

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::error::Verify;
use std::path::Path;

const FILES: [&str; 5] = [
    "source.s16le",
    "expected.s16le",
    "packet-000.bin",
    "packet-001.bin",
    "packet-002.bin",
];

#[test]
#[ignore = "explicit codec fixture command only"]
fn generate_flac_corpus() {
    let dir = support::fixture_dir("CODEC_FIXTURE_OUTPUT");
    // Same documented Phase 1 signal, independently authored here: each
    // channel differs at every index and peak amplitude stays <= -6 dBFS.
    let samples: Vec<i32> = (0..300)
        .flat_map(|index| {
            (0..2).map(move |channel| {
                ((channel * 104_729_i64 + index * 1_299_709_i64) % 32_769 - 16_384) as i32
            })
        })
        .collect();
    let source: Vec<u8> = samples
        .iter()
        .flat_map(|sample| i16::try_from(*sample).unwrap().to_le_bytes())
        .collect();
    let mut padded = samples;
    padded.extend([0; 84 * 2]);
    assert_eq!(padded.len(), 384 * 2);

    // flacenc configuration types are non-exhaustive: customize defaults.
    let mut config = flacenc::config::Encoder::default();
    config.block_size = 128;
    config.multithread = false;
    config.stereo_coding.use_leftside = false;
    config.stereo_coding.use_rightside = false;
    config.stereo_coding.use_midside = false;
    config.subframe_coding.use_constant = false;
    config.subframe_coding.use_fixed = false;
    config.subframe_coding.use_lpc = false;
    let config = config
        .into_verified()
        .expect("valid fixed Verbatim encoder settings");
    let source_pcm = flacenc::source::MemSource::from_samples(&padded, 2, 16, 48_000);
    let stream = flacenc::encode_with_fixed_block_size(&config, source_pcm, 128).unwrap();
    assert_eq!(stream.frame_count(), 3);
    let packets: Vec<Vec<u8>> = (0..3)
        .map(|index| {
            let frame = stream.frame(index).unwrap();
            assert_eq!(frame.header().block_size(), 128);
            let mut sink = ByteSink::new();
            frame.write(&mut sink).unwrap();
            let packet = sink.into_inner();
            assert!(!packet.windows(4).any(|bytes| bytes == b"fLaC"));
            packet
        })
        .collect();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("source.s16le"), &source).unwrap();
    std::fs::write(dir.join("expected.s16le"), &source).unwrap();
    for (index, packet) in packets.iter().enumerate() {
        std::fs::write(dir.join(format!("packet-{index:03}.bin")), packet).unwrap();
    }
    let mut manifest = String::from(
        "# Fixed-block FLAC fixture\n\n\
flacenc = 0.5.1\nclaxon = 0.4.3\nopus = 0.4.0\nsha2 = 0.10.9\n\
sample_rate = 48000\nchannels = 2\nbits_per_sample = 16\nblock_size = 128\n\
S = 300\nP = 0\nE = 84\npadded = 384\n\
packet_order = packet-000.bin,packet-001.bin,packet-002.bin\n\
stereo_coding = independent\nsubframe_coding = verbatim\n\
flacenc_default_features = false\nmultithread = false\n\n\
Source: deterministic interleaved L/R signed 16-bit little-endian PCM.\n\
sample(channel, index) = ((channel * 104729 + index * 1299709) % 32769) - 16384.\n\
S is source frames, P is priming, E is zero padding and final end trim;\n\
S + P + E = 384 = 3 * 128. Source and expected PCM are identical, 300 stereo frames.\n\
Encoder disables left/side, right/side, mid/side, constant, fixed and LPC subframes;\n\
all other Encoder settings are flacenc 0.5.1 defaults (unused for Verbatim).\n\
Each packet is serialized separately via Stream::frame(i), Frame::header and\n\
BitRepr::write. No packet contains a stream marker or metadata.\n\
Root checks are intentionally limited to this fixed Verbatim corpus, with CRCs,\n\
boundaries and sample equality; they are not a general-purpose FLAC decoder.\n\
The excluded verifier reconstructs a temporary stream in memory and uses Claxon\n\
to decode all 384 frames before checking the final 84 zero frames and trimming.\n\
Temporary STREAMINFO describes stereo and 384 samples; IAMF Codec Config uses\n\
its canonical channel/unknown-length fields from CodecConfig::flac instead.\n\n\
Run from the repository root (relative paths resolve there):\n\n\
```sh\n\
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact generate_flac_corpus\n\
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact verify_flac_corpus\n\
```\n\n",
    );
    for name in FILES {
        manifest.push_str(&format!(
            "sha256.{name} = {}\n",
            support::digest(&std::fs::read(dir.join(name)).unwrap())
        ));
    }
    std::fs::write(dir.join("MANIFEST.md"), manifest).unwrap();
    verify(&dir);
}

#[test]
#[ignore = "explicit codec fixture command only"]
fn verify_flac_corpus() {
    verify(&support::fixture_dir("CODEC_FIXTURE_INPUT"));
}

fn verify(dir: &Path) {
    let fields = support::manifest(dir);
    for (key, value) in [
        ("flacenc", "0.5.1"),
        ("claxon", "0.4.3"),
        ("opus", "0.4.0"),
        ("sha2", "0.10.9"),
        ("S", "300"),
        ("P", "0"),
        ("E", "84"),
        ("padded", "384"),
        ("sample_rate", "48000"),
        ("channels", "2"),
        ("bits_per_sample", "16"),
        ("block_size", "128"),
        ("stereo_coding", "independent"),
        ("subframe_coding", "verbatim"),
        (
            "packet_order",
            "packet-000.bin,packet-001.bin,packet-002.bin",
        ),
    ] {
        assert_eq!(fields.get(key).map(String::as_str), Some(value), "{key}");
    }
    let mut names = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [
            "MANIFEST.md",
            "expected.s16le",
            "packet-000.bin",
            "packet-001.bin",
            "packet-002.bin",
            "source.s16le"
        ]
    );
    for name in FILES {
        assert_eq!(
            fields.get(&format!("sha256.{name}")),
            Some(&support::digest(&std::fs::read(dir.join(name)).unwrap()))
        );
    }
    let expected_bytes = std::fs::read(dir.join("expected.s16le")).unwrap();
    assert_eq!(
        expected_bytes,
        std::fs::read(dir.join("source.s16le")).unwrap()
    );
    let expected = support::pcm_s16le(&expected_bytes);
    assert_eq!(expected.len(), 600);

    // One last STREAMINFO metadata block: min/max block 128, unknown frame
    // sizes, 48 kHz / stereo / 16-bit / 384 samples, unchecked MD5 (all zero).
    let mut stream = b"fLaC\x80\x00\x00\x22\x00\x80\x00\x80".to_vec();
    stream.extend([0; 6]);
    stream.extend(((48_000_u64 << 44) | (1 << 41) | (15 << 36) | 384).to_be_bytes());
    stream.extend([0; 16]);
    assert_eq!(stream.len(), 42);
    for number in 0_u8..3 {
        let packet = std::fs::read(dir.join(format!("packet-{number:03}.bin"))).unwrap();
        assert_eq!(&packet[..5], &[0xff, 0xf8, 0x6a, 0x18, number]);
        assert_eq!(usize::from(packet[5]) + 1, 128);
        assert!(!packet.windows(4).any(|bytes| bytes == b"fLaC"));
        // Each packet independently decodes to exactly one block, no trailing
        // second frame; the complete stream below also checks global duration.
        let mut single = stream[..42].to_vec();
        single.extend(&packet);
        let mut reader = claxon::FlacReader::new(single.as_slice()).unwrap();
        let mut blocks = reader.blocks();
        let block = blocks.read_next_or_eof(Vec::new()).unwrap().unwrap();
        assert_eq!(block.duration(), 128);
        assert_eq!(block.channels(), 2);
        assert!(
            blocks
                .read_next_or_eof(block.into_buffer())
                .unwrap()
                .is_none()
        );
        stream.extend(packet);
    }
    let mut reader = claxon::FlacReader::new(stream.as_slice()).unwrap();
    assert_eq!(reader.streaminfo().sample_rate, 48_000);
    assert_eq!(reader.streaminfo().bits_per_sample, 16);
    assert_eq!(reader.streaminfo().channels, 2);
    let mut decoded = reader.samples().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(decoded.len(), 384 * 2);
    assert!(decoded[300 * 2..].iter().all(|sample| *sample == 0));
    decoded.truncate(decoded.len() - 84 * 2);
    assert_eq!(decoded.len(), 300 * 2);
    assert_eq!(decoded, expected);
}
