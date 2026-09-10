//! Read-only checks of the committed fixed-block Verbatim corpus. This is
//! deliberately not a general FLAC decoder; Claxon lives in the excluded tool.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

use iamf::obu::Trimming;

#[path = "support/fixture.rs"]
mod fixture;

use fixture::{
    validate_opus_digest_keys, validate_opus_inventory, validate_opus_metadata,
    validate_opus_packet,
};

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs/flac")
}

#[allow(clippy::expect_used)] // Missing committed artifacts must fail loudly.
fn codec_artifact_sha256_inventory() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs");
    let mut inventory = Vec::new();
    for codec in std::fs::read_dir(root).expect("committed codec corpus directory") {
        let codec = codec.expect("codec corpus directory entry");
        if !codec.file_type().expect("codec corpus entry type").is_dir() {
            continue;
        }
        let codec_name = codec
            .file_name()
            .into_string()
            .expect("UTF-8 codec corpus directory name");
        for artifact in std::fs::read_dir(codec.path()).expect("committed codec directory") {
            let artifact = artifact.expect("codec artifact directory entry");
            if !artifact
                .file_type()
                .expect("codec artifact entry type")
                .is_file()
            {
                continue;
            }
            let name = artifact
                .file_name()
                .into_string()
                .expect("UTF-8 codec artifact filename");
            if name == "MANIFEST.md"
                || (name.starts_with("packet-") && name.ends_with(".bin"))
                || name.ends_with(".s16le")
            {
                let digest = format!(
                    "{:x}",
                    Sha256::digest(
                        std::fs::read(artifact.path()).expect("committed codec artifact")
                    )
                );
                inventory.push((format!("{codec_name}/{name}"), digest));
            }
        }
    }
    inventory.sort_by(|left, right| left.0.cmp(&right.0));
    inventory
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
    assert!(decoded
        .get(600..)
        .expect("84 padded stereo frames")
        .iter()
        .all(|sample| *sample == 0));
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
    assert!(expected
        .chunks_exact(2)
        .all(|pair| pair.first() != pair.last()));
    assert!(expected.iter().all(|sample| sample.unsigned_abs() <= 16384));
}

#[test]
fn flac_adapter_writes_exact_packets_and_only_final_end_trim() {
    use iamf::obu::{Trimming, TypeSpecific};
    use iamf::sequence::{parse_sequence, SequenceObu};
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
    let corpus = fixture::load_opus_corpus();
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
    assert!(validate_opus_inventory(
        &["packet-000.bin".to_owned()],
        &["packet-000.bin".to_owned(), "packet-001.bin".to_owned()]
    )
    .is_err());
    assert!(validate_opus_inventory(
        &[
            "packet-000.bin".to_owned(),
            "packet-001.bin".to_owned(),
            "unexpected.bin".to_owned()
        ],
        &packets
    )
    .is_err());
    assert!(validate_opus_packet(&[], 0).is_err());
    assert!(validate_opus_packet(b"OpusHead", 8).is_err());
    assert!(validate_opus_packet(b"OggS", 4).is_err());
    assert!(validate_opus_packet(b"raw", 4).is_err());
}

#[test]
fn write_sha256_inventory() {
    let inventory = codec_artifact_sha256_inventory();
    assert!(!inventory.is_empty(), "immutable artifact inventory");
    assert!(
        inventory
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if left.0 < right.0)),
        "artifact inventory is sorted by relative path"
    );
    assert_eq!(
        inventory
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        [
            "flac/MANIFEST.md",
            "flac/expected.s16le",
            "flac/packet-000.bin",
            "flac/packet-001.bin",
            "flac/packet-002.bin",
            "flac/source.s16le",
            "opus/MANIFEST.md",
            "opus/expected.s16le",
            "opus/packet-000.bin",
            "opus/packet-001.bin",
            "opus/source.s16le",
        ]
    );
    assert!(inventory.iter().all(|(_, digest)| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }));

    if let Some(destination) = std::env::var_os("CODEC_SHA256_INVENTORY") {
        let destination = PathBuf::from(destination);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).expect("inventory parent directory");
        }
        let mut output = String::new();
        for (path, digest) in &inventory {
            output.push_str(digest);
            output.push_str("  ");
            output.push_str(path);
            output.push('\n');
        }
        std::fs::write(destination, output).expect("write opt-in inventory");
    }
}
