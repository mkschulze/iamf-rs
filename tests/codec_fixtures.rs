//! Read-only checks of the committed fixed-block Verbatim corpus. This is
//! deliberately not a general FLAC decoder; Claxon lives in the excluded tool.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

use iamf::obu::Trimming;

#[path = "support/fixture.rs"]
mod fixture;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs/flac")
}

fn opus_corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs/opus")
}

#[allow(clippy::expect_used)] // Missing committed artifacts must fail loudly.
fn artifact(name: &str) -> Vec<u8> {
    std::fs::read(corpus().join(name)).expect("committed FLAC artifact must exist")
}

#[allow(clippy::expect_used)] // Missing committed artifacts must fail loudly.
fn opus_artifact(name: &str) -> Vec<u8> {
    std::fs::read(opus_corpus().join(name)).expect("committed Opus artifact must exist")
}

#[allow(clippy::expect_used)] // Committed test data must be readable.
fn manifest() -> BTreeMap<String, String> {
    let text = String::from_utf8(artifact("MANIFEST.md")).expect("UTF-8 manifest");
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(" = ") {
            assert!(fields.insert(key.to_owned(), value.to_owned()).is_none());
        }
    }
    fields
}

#[allow(clippy::expect_used)] // chunks_exact guarantees this fixed conversion.
fn pcm(bytes: &[u8], big_endian: bool) -> Vec<i32> {
    assert_eq!(bytes.len().checked_rem(2), Some(0));
    bytes
        .chunks_exact(2)
        .map(|pair| {
            let pair: [u8; 2] = pair.try_into().expect("two bytes");
            i32::from(if big_endian {
                i16::from_be_bytes(pair)
            } else {
                i16::from_le_bytes(pair)
            })
        })
        .collect()
}

struct OpusCorpus {
    lookahead: usize,
    packets: usize,
    end_trim: usize,
    source_frames: usize,
    units: Vec<fixture::EncodedTemporalUnit>,
    expected: Vec<i32>,
}

#[allow(clippy::expect_used)] // A malformed committed corpus is a hard test failure.
fn load_opus_corpus() -> OpusCorpus {
    let text = String::from_utf8(opus_artifact("MANIFEST.md")).expect("UTF-8 Opus manifest");
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(" = ") {
            assert!(fields.insert(key.to_owned(), value.to_owned()).is_none());
        }
    }
    let number = |key: &str| -> usize {
        fields
            .get(key)
            .unwrap_or_else(|| panic!("Opus manifest field {key}"))
            .parse()
            .unwrap_or_else(|_| panic!("numeric Opus manifest field {key}"))
    };
    let lookahead = number("L");
    let packets = number("P");
    let end_trim = number("E");
    let source_frames = number("S");
    assert!(lookahead > 0, "Opus lookahead must be nonzero");
    assert_eq!(packets, (source_frames + lookahead).div_ceil(960));
    assert!(
        packets >= 2,
        "Opus corpus must contain at least two packets"
    );
    assert_eq!(end_trim, packets * 960 - lookahead - source_frames);
    assert!(end_trim > 0 && end_trim < 960);
    assert_ne!(end_trim, lookahead);

    for (key, expected) in fields.iter().filter(|(key, _)| key.starts_with("sha256.")) {
        let name = key.strip_prefix("sha256.").expect("digest key prefix");
        let digest = format!("{:x}", Sha256::digest(opus_artifact(name)));
        assert_eq!(&digest, expected, "{name}");
    }

    let order = fields
        .get("packet_order")
        .expect("Opus packet order")
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    let canonical = (0..packets)
        .map(|index| format!("packet-{index:03}.bin"))
        .collect::<Vec<_>>();
    assert_eq!(order, canonical.iter().map(String::as_str).collect::<Vec<_>>());
    let mut actual = std::fs::read_dir(opus_corpus())
        .expect("committed Opus corpus directory")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            (name.starts_with("packet-") && name.ends_with(".bin")).then_some(name)
        })
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(actual, canonical, "Opus packet file set");
    for name in &actual {
        let bytes = opus_artifact(name);
        assert!(!bytes.windows(8).any(|window| window == b"OpusHead"), "{name}");
    }
    let mut units = Vec::with_capacity(packets);
    for (index, name) in order.iter().enumerate() {
        let bytes = opus_artifact(name);
        assert!(!bytes.is_empty(), "Opus packet {name} must be nonempty");
        assert!(!bytes.windows(8).any(|window| window == b"OpusHead"));
        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            fields.get(&format!("sha256.{name}")),
            Some(&digest),
            "{name}"
        );
        let trimming = if index == 0 {
            Some(Trimming {
                at_start: lookahead as u32,
                at_end: 0,
            })
        } else if index + 1 == packets {
            Some(Trimming {
                at_start: 0,
                at_end: end_trim as u32,
            })
        } else {
            None
        };
        units.push(fixture::EncodedTemporalUnit {
            trimming,
            substream_payloads: vec![bytes],
        });
    }
    let expected_bytes = opus_artifact("expected.s16le");
    let digest = format!("{:x}", Sha256::digest(&expected_bytes));
    assert_eq!(fields.get("sha256.expected.s16le"), Some(&digest));
    let expected = pcm(&expected_bytes, false);
    assert_eq!(expected.len(), source_frames * 2, "expected stereo frames");
    OpusCorpus {
        lookahead,
        packets,
        end_trim,
        source_frames,
        units,
        expected,
    }
}

fn crc(bytes: &[u8], width: u32, polynomial: u16) -> u16 {
    let mut value = 0_u16;
    let high = 1_u16.wrapping_shl(width.saturating_sub(1));
    let mask = u16::MAX.wrapping_shr(16_u32.saturating_sub(width));
    for byte in bytes {
        value ^= u16::from(*byte).wrapping_shl(width.saturating_sub(8));
        for _ in 0..8 {
            value = if value & high != 0 {
                value.wrapping_shl(1) ^ polynomial
            } else {
                value.wrapping_shl(1)
            } & mask;
        }
    }
    value
}

/// Only fixed-block, 48 kHz, independent stereo, 16-bit Verbatim is accepted.
/// Exact lengths prohibit extra frames, metadata, and unconsumed trailing data.
#[allow(clippy::expect_used)] // A malformed committed packet is a hard test failure.
fn decode_committed_packet(bytes: &[u8], number: u8) -> Vec<i32> {
    assert!(!bytes.windows(4).any(|window| window == b"fLaC"));
    assert_eq!(bytes.len(), 523);
    assert_eq!(
        bytes.get(..5),
        Some([0xff, 0xf8, 0x6a, 0x18, number].as_slice())
    );
    let block_size = usize::from(*bytes.get(5).expect("block-size byte")).saturating_add(1);
    assert_eq!(block_size, 128);
    assert_eq!(crc(bytes.get(..7).expect("header and CRC-8"), 8, 0x07), 0);
    assert_eq!(crc(bytes, 16, 0x8005), 0);
    assert_eq!(
        bytes.get(7),
        Some(&0x02),
        "left Verbatim subframe, no wasted bits"
    );
    assert_eq!(
        bytes.get(264),
        Some(&0x02),
        "right Verbatim subframe, no wasted bits"
    );
    let left = pcm(bytes.get(8..264).expect("128 left samples"), true);
    let right = pcm(bytes.get(265..521).expect("128 right samples"), true);
    left.into_iter()
        .zip(right)
        .flat_map(|(l, r)| [l, r])
        .collect()
}

#[test]
fn flac_manifest_authenticates_exactly_three_packets_and_both_pcm_files() {
    let fields = manifest();
    for (key, value) in [
        ("flacenc", "0.5.1"),
        ("claxon", "0.4.3"),
        ("sha2", "0.10.9"),
        ("sample_rate", "48000"),
        ("channels", "2"),
        ("bits_per_sample", "16"),
        ("block_size", "128"),
        ("S", "300"),
        ("P", "0"),
        ("E", "84"),
        ("padded", "384"),
        (
            "packet_order",
            "packet-000.bin,packet-001.bin,packet-002.bin",
        ),
        ("stereo_coding", "independent"),
        ("subframe_coding", "verbatim"),
    ] {
        assert_eq!(fields.get(key).map(String::as_str), Some(value), "{key}");
    }
    let mut names = std::fs::read_dir(corpus())
        .expect("committed corpus directory")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 filename")
        })
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
    for name in names.iter().filter(|name| name.as_str() != "MANIFEST.md") {
        let digest = format!("{:x}", Sha256::digest(artifact(name)));
        assert_eq!(
            fields.get(&format!("sha256.{name}")),
            Some(&digest),
            "{name}"
        );
    }
}

#[test]
fn flac_three_headers_and_samples_prove_384_frames_then_trim_84_to_exact_300() {
    let mut decoded = Vec::new();
    for number in 0_u8..3 {
        decoded.extend(decode_committed_packet(
            &artifact(&format!("packet-{number:03}.bin")),
            number,
        ));
    }
    assert_eq!(decoded.len(), 768, "384 stereo frames before trimming");
    assert!(
        decoded
            .get(600..)
            .expect("84 padded stereo frames")
            .iter()
            .all(|sample| *sample == 0)
    );
    let fields = manifest();
    let end: usize = fields
        .get("E")
        .expect("end trim")
        .parse()
        .expect("numeric trim");
    assert_eq!(end, 84);
    decoded.truncate(
        decoded
            .len()
            .checked_sub(end.saturating_mul(2))
            .expect("bounded trim"),
    );
    assert_eq!(decoded.len(), 600);
    let expected = pcm(&artifact("expected.s16le"), false);
    assert_eq!(expected.len(), 600);
    assert_eq!(decoded, expected);
    assert_eq!(artifact("source.s16le"), artifact("expected.s16le"));
    assert!(
        expected
            .chunks_exact(2)
            .all(|pair| pair.first() != pair.last())
    );
    assert!(expected.iter().all(|sample| sample.unsigned_abs() <= 16384));
}

#[test]
fn flac_adapter_writes_exact_packets_and_only_final_end_trim() {
    use iamf::obu::{Trimming, TypeSpecific};
    use iamf::sequence::{SequenceObu, parse_sequence};
    let fixture = fixture::flac();
    assert_eq!(
        fixture.spec().expect("FLAC spec").codec,
        fixture::FixtureCodec::Flac
    );
    assert_eq!(fixture.sample_frames(), 300);
    assert_eq!(
        fixture.single_pcm(),
        pcm(&artifact("expected.s16le"), false)
    );
    let bytes = fixture.encode().expect("opaque FLAC encode");
    let frames = parse_sequence(&bytes)
        .expect("IAMF sequence")
        .obus
        .into_iter()
        .filter_map(|obu| {
            if let SequenceObu::AudioFrame(frame) = obu {
                Some(frame)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 3);
    for (number, frame) in frames.iter().enumerate() {
        assert_eq!(
            frame.payload.payload,
            artifact(&format!("packet-{number:03}.bin"))
        );
        assert_eq!(
            frame.header.type_specific,
            TypeSpecific::Trimming((number == 2).then_some(Trimming {
                at_end: 84,
                at_start: 0
            }))
        );
    }
}

#[test]
fn opus_manifest_authenticates_arithmetic_packets_trims_and_exact_stereo_output() {
    let corpus = load_opus_corpus();
    assert_eq!(corpus.units.len(), corpus.packets);
    assert_eq!(
        corpus.units[0].trimming,
        Some(Trimming {
            at_start: corpus.lookahead as u32,
            at_end: 0
        })
    );
    assert_eq!(
        corpus.units.last().and_then(|unit| unit.trimming),
        Some(Trimming {
            at_start: 0,
            at_end: corpus.end_trim as u32
        })
    );
    for unit in corpus
        .units
        .iter()
        .skip(1)
        .take(corpus.packets.saturating_sub(2))
    {
        assert!(
            unit.trimming.is_none()
                || unit.trimming
                    == Some(Trimming {
                        at_start: 0,
                        at_end: 0
                    })
        );
    }
    assert_eq!(corpus.expected.len(), corpus.source_frames * 2);
}
