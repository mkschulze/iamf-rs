//! Read-only checks of the committed fixed-block Verbatim corpus. This is
//! deliberately not a general FLAC decoder; Claxon lives in the excluded tool.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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

fn validate_opus_metadata(fields: &BTreeMap<String, String>) -> Result<(), String> {
    for (key, expected) in [
        ("sample_rate", "48000"),
        ("channels", "2"),
        ("frame_samples", "960"),
        ("application", "audio"),
        ("bitrate", "128000"),
        ("vbr", "false"),
        ("complexity", "10"),
        ("force_channels", "stereo"),
        ("max_bandwidth", "fullband"),
        ("signal", "music"),
        ("dtx", "false"),
        ("inband_fec", "false"),
        ("packet_loss_perc", "0"),
        ("lsb_depth", "16"),
    ] {
        if fields.get(key).map(String::as_str) != Some(expected) {
            return Err(format!("Opus manifest metadata {key} = {expected}"));
        }
    }
    for key in ["sha256.source.s16le", "sha256.expected.s16le"] {
        if !fields.contains_key(key) {
            return Err(format!("Opus manifest field {key}"));
        }
    }
    Ok(())
}

fn validate_opus_inventory(actual: &[String], required: &[String]) -> Result<(), String> {
    if actual == required {
        Ok(())
    } else {
        Err("Opus corpus inventory mismatch".to_owned())
    }
}

fn validate_opus_packet(bytes: &[u8], expected_len: usize) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("Opus packet is empty".to_owned());
    }
    if bytes.windows(8).any(|window| window == b"OpusHead") {
        return Err("OpusHead marker in raw packet".to_owned());
    }
    if bytes.windows(4).any(|window| window == b"OggS") {
        return Err("OggS marker in raw packet".to_owned());
    }
    if bytes.len() != expected_len {
        return Err("Opus packet length mismatch".to_owned());
    }
    Ok(())
}

fn validate_opus_digest_keys(
    fields: &BTreeMap<String, String>,
    packet_names: &[String],
) -> Result<(), String> {
    let mut expected = BTreeSet::from([
        "sha256.source.s16le".to_owned(),
        "sha256.expected.s16le".to_owned(),
    ]);
    expected.extend(packet_names.iter().map(|name| format!("sha256.{name}")));
    let actual = fields
        .keys()
        .filter(|key| key.starts_with("sha256."))
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err("Opus manifest digest key set mismatch".to_owned())
    }
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
    validate_opus_metadata(&fields).expect("complete Opus manifest metadata");
    let number = |key: &str| -> usize {
        fields
            .get(key)
            .expect("Opus manifest field")
            .parse()
            .expect("numeric Opus manifest field")
    };
    let lookahead = number("L");
    let packets = number("P");
    let end_trim = number("E");
    let source_frames = number("S");
    assert!(lookahead > 0, "Opus lookahead must be nonzero");
    let total = source_frames
        .checked_add(lookahead)
        .expect("Opus source plus lookahead overflow");
    assert_eq!(packets, total.div_ceil(960));
    assert!(
        packets >= 2,
        "Opus corpus must contain at least two packets"
    );
    let padded = packets.checked_mul(960).expect("Opus padded length overflow");
    let expected_end_trim = padded
        .checked_sub(lookahead)
        .and_then(|value| value.checked_sub(source_frames))
        .expect("Opus trim arithmetic underflow");
    assert_eq!(end_trim, expected_end_trim);
    assert!(end_trim > 0 && end_trim < 960);
    assert_ne!(end_trim, lookahead);

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
    validate_opus_digest_keys(&fields, &canonical).expect("Opus manifest digest keys");
    let mut actual = std::fs::read_dir(opus_corpus())
        .expect("committed Opus corpus directory")
        .map(|entry| {
            entry
                .expect("Opus corpus directory entry")
                .file_name()
                .into_string()
                .expect("Opus corpus filenames must be UTF-8")
        })
        .filter(|name| name.starts_with("packet-") && name.ends_with(".bin"))
        .collect::<Vec<_>>();
    actual.sort();
    validate_opus_inventory(&actual, &canonical).expect("Opus packet file set");
    let mut inventory = std::fs::read_dir(opus_corpus())
        .expect("committed Opus corpus directory")
        .map(|entry| {
            entry
                .expect("Opus corpus directory entry")
                .file_name()
                .into_string()
                .expect("Opus corpus filenames must be UTF-8")
        })
        .collect::<Vec<_>>();
    inventory.sort();
    let mut required = vec!["MANIFEST.md".to_owned(), "expected.s16le".to_owned(), "source.s16le".to_owned()];
    required.extend(canonical.iter().cloned());
    required.sort();
    validate_opus_inventory(&inventory, &required).expect("Opus corpus inventory");
    let mut digest_names = vec!["source.s16le", "expected.s16le"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    digest_names.extend(canonical.iter().cloned());
    for name in digest_names {
        let digest_key = format!("sha256.{name}");
        let expected = fields
            .get(&digest_key)
            .expect("required Opus digest manifest field");
        let digest = format!("{:x}", Sha256::digest(opus_artifact(&name)));
        assert_eq!(&digest, expected, "{name}");
    }
    for name in &actual {
        let bytes = opus_artifact(name);
        let packet_len = fields
            .get(&format!("packet_len.{name}"))
            .expect("Opus packet length manifest field")
            .parse::<usize>()
            .expect("numeric Opus packet length");
        validate_opus_packet(&bytes, packet_len).expect("valid Opus packet");
    }
    let mut units = Vec::with_capacity(packets);
    for (index, name) in order.iter().enumerate() {
        let bytes = opus_artifact(name);
        let packet_len = fields
            .get(&format!("packet_len.{name}"))
            .expect("Opus packet length manifest field")
            .parse::<usize>()
            .expect("numeric Opus packet length");
        validate_opus_packet(&bytes, packet_len).expect("valid Opus packet");
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
        } else if index.checked_add(1).expect("Opus packet index overflow") == packets {
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
    let expected_samples = source_frames
        .checked_mul(2)
        .expect("Opus stereo sample length overflow");
    assert_eq!(expected.len(), expected_samples, "expected stereo frames");
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
        corpus.units.first().expect("Opus packets").trimming,
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

#[test]
fn opus_manifest_contract_rejects_missing_source_provenance_and_bad_metadata() {
    let mut fields = BTreeMap::from([
        ("sample_rate".to_owned(), "48000".to_owned()),
        ("channels".to_owned(), "2".to_owned()),
        ("frame_samples".to_owned(), "960".to_owned()),
        ("application".to_owned(), "audio".to_owned()),
        ("bitrate".to_owned(), "128000".to_owned()),
        ("vbr".to_owned(), "false".to_owned()),
        ("complexity".to_owned(), "10".to_owned()),
        ("force_channels".to_owned(), "stereo".to_owned()),
        ("max_bandwidth".to_owned(), "fullband".to_owned()),
        ("signal".to_owned(), "music".to_owned()),
        ("dtx".to_owned(), "false".to_owned()),
        ("inband_fec".to_owned(), "false".to_owned()),
        ("packet_loss_perc".to_owned(), "0".to_owned()),
        ("lsb_depth".to_owned(), "16".to_owned()),
        ("sha256.expected.s16le".to_owned(), "digest".to_owned()),
    ]);
    assert!(validate_opus_metadata(&fields).is_err());
    fields.insert("sha256.source.s16le".to_owned(), "digest".to_owned());
    for (key, expected) in [
        ("sample_rate", "48000"), ("channels", "2"), ("frame_samples", "960"),
        ("application", "audio"), ("bitrate", "128000"), ("vbr", "false"),
        ("complexity", "10"), ("force_channels", "stereo"),
        ("max_bandwidth", "fullband"), ("signal", "music"), ("dtx", "false"),
        ("inband_fec", "false"), ("packet_loss_perc", "0"), ("lsb_depth", "16"),
    ] {
        fields.insert(key.to_owned(), "wrong".to_owned());
        assert!(validate_opus_metadata(&fields).is_err(), "{key}");
        fields.insert(key.to_owned(), expected.to_owned());
    }
    fields.remove("sha256.expected.s16le");
    assert!(validate_opus_metadata(&fields).is_err());
    fields.insert("sha256.expected.s16le".to_owned(), "digest".to_owned());
    let packets = vec!["packet-000.bin".to_owned(), "packet-001.bin".to_owned()];
    assert!(validate_opus_digest_keys(&fields, &packets).is_err());
    fields.insert("sha256.packet-000.bin".to_owned(), "digest".to_owned());
    fields.insert("sha256.packet-001.bin".to_owned(), "digest".to_owned());
    fields.insert("sha256../escape".to_owned(), "digest".to_owned());
    assert!(validate_opus_digest_keys(&fields, &packets).is_err());
    assert!(validate_opus_inventory(&["packet-000.bin".to_owned()], &["packet-000.bin".to_owned(), "packet-001.bin".to_owned()]).is_err());
    assert!(validate_opus_inventory(&["packet-000.bin".to_owned(), "packet-001.bin".to_owned(), "unexpected.bin".to_owned()], &packets).is_err());
    assert!(validate_opus_packet(&[], 0).is_err());
    assert!(validate_opus_packet(b"OpusHead", 8).is_err());
    assert!(validate_opus_packet(b"OggS", 4).is_err());
    assert!(validate_opus_packet(b"raw", 4).is_err());
}
