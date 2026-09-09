//! Read-only checks of the committed fixed-block Verbatim corpus. This is
//! deliberately not a general FLAC decoder; Claxon lives in the excluded tool.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[path = "support/fixture.rs"]
mod fixture;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs/flac")
}

#[allow(clippy::expect_used)] // Missing committed artifacts must fail loudly.
fn artifact(name: &str) -> Vec<u8> {
    std::fs::read(corpus().join(name)).expect("committed FLAC artifact must exist")
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
