#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use arbitrary::Unstructured;
use iamf::fuzzing::sequence_from_fuzz_bytes;
use iamf::sequence::{SequenceObu, parse_sequence, write_parsed_sequence};
use sha2::{Digest, Sha256};

const PARSE_CORPUS: &str = "fuzz/corpus/parse_sequence";
const ROUNDTRIP_CORPUS: &str = "fuzz/corpus/obu_roundtrip";
const PARSE_ARTIFACTS: &str = "fuzz/artifacts/parse_sequence";
const ROUNDTRIP_ARTIFACTS: &str = "fuzz/artifacts/obu_roundtrip";

const PINNED_POSITIVE_FIXTURES: [(&str, &str); 4] = [
    (
        "noise_1024samp_5p1_opus.iamf",
        "2115fd08eee9791f1e35fe4a4bfb268ab02cc096ad9a5cc950aa911999fd1cae",
    ),
    (
        "noise_1024samp_stereo_flac.iamf",
        "4bcc7b0ba187da62915a209e1e8ebc02978b4d35cbd392a13d4a4ce9340a269a",
    ),
    (
        "noise_3s_stereo_opus.iamf",
        "a91b89d0691fa3e92e29ae2805019bb355081b9ae84cdd715269f4c5f99c5693",
    ),
    (
        "tones_100ms_3OA_stereo_opus.iamf",
        "d3d1405cd2ea4b93e6c7cabda110037983862f5b4755573f203b84b8b1d49cdf",
    ),
];

fn files_below(root: &Path, required: bool) -> Vec<PathBuf> {
    if !root.exists() {
        assert!(
            !required,
            "required replay root is missing: {}",
            root.display()
        );
        return Vec::new();
    }

    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| {
                panic!("cannot enumerate {}: {error}", directory.display())
            });
            let path = entry.path();
            let kind = entry
                .file_type()
                .unwrap_or_else(|error| panic!("cannot inspect {}: {error}", path.display()));
            assert!(
                !kind.is_symlink(),
                "replay input must be a copy: {}",
                path.display()
            );
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                files.push(path);
            } else {
                panic!("unsupported replay input: {}", path.display());
            }
        }
    }
    files.sort();
    if required {
        assert!(
            !files.is_empty(),
            "replay corpus is empty: {}",
            root.display()
        );
    }
    files
}

fn read_replay_input(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("cannot read replay input {}: {error}", path.display()));
    assert!(
        !bytes.is_empty(),
        "replay input is empty: {}",
        path.display()
    );
    bytes
}

fn replay_model(path: &Path, data: &[u8]) {
    let mut input = Unstructured::new(data);
    let sequence = sequence_from_fuzz_bytes(&mut input)
        .unwrap_or_else(|error| panic!("model seed {} is invalid: {error}", path.display()));
    let bytes = write_parsed_sequence(Vec::new(), &sequence)
        .unwrap_or_else(|error| panic!("model seed {} does not write: {error}", path.display()));
    let parsed = parse_sequence(&bytes)
        .unwrap_or_else(|error| panic!("model seed {} does not parse: {error}", path.display()));
    assert_eq!(
        parsed,
        sequence,
        "structural mismatch for {}",
        path.display()
    );
    assert_eq!(
        write_parsed_sequence(Vec::new(), &parsed).unwrap_or_else(|error| panic!(
            "model seed {} does not rewrite: {error}",
            path.display()
        )),
        bytes,
        "byte mismatch for {}",
        path.display()
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[test]
fn parse_replay_covers_every_seed_and_minimized_artifact() {
    let corpus = files_below(Path::new(PARSE_CORPUS), true);
    let artifacts = files_below(Path::new(PARSE_ARTIFACTS), false);
    for path in corpus.iter().chain(&artifacts) {
        let bytes = read_replay_input(path);
        let _ = parse_sequence(&bytes);
    }
}

#[test]
fn pinned_positive_parse_seeds_are_exact_fixture_copies() {
    let source = Path::new("tests/fixtures/reference/iamf-tools");
    let corpus = Path::new(PARSE_CORPUS);
    for (name, expected_digest) in PINNED_POSITIVE_FIXTURES {
        let original = read_replay_input(&source.join(name));
        let seed = read_replay_input(&corpus.join(name));
        assert_eq!(
            sha256_hex(&original),
            expected_digest,
            "fixture drift: {name}"
        );
        assert_eq!(sha256_hex(&seed), expected_digest, "seed drift: {name}");
        assert_eq!(seed, original, "seed is not byte-identical: {name}");
        parse_sequence(&seed)
            .unwrap_or_else(|error| panic!("positive seed {name} failed: {error}"));
    }
}

#[test]
fn roundtrip_replay_covers_every_seed_and_minimized_artifact() {
    let corpus = files_below(Path::new(ROUNDTRIP_CORPUS), true);
    let artifacts = files_below(Path::new(ROUNDTRIP_ARTIFACTS), false);
    for path in corpus.iter().chain(&artifacts) {
        let bytes = read_replay_input(path);
        replay_model(path, &bytes);
    }
}

#[test]
fn documented_roundtrip_seed_shapes_are_present() {
    let paths = files_below(Path::new(ROUNDTRIP_CORPUS), true);
    let names = paths
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>();
    for required in [
        "empty",
        "descriptors-only",
        "no-delimiter",
        "delimiter",
        "multi-substream-all-parameters",
        "redundant-descriptor",
        "reserved-trailing",
        "unknown-before",
        "unknown-between",
        "unknown-after",
        "bounded-raw-parameter-data",
    ] {
        assert!(
            names.contains(&required),
            "missing documented shape seed: {required}"
        );
    }

    for placement in ["unknown-before", "unknown-between", "unknown-after"] {
        let path = Path::new(ROUNDTRIP_CORPUS).join(placement);
        let bytes = read_replay_input(&path);
        let mut input = Unstructured::new(&bytes);
        let sequence = sequence_from_fuzz_bytes(&mut input)
            .unwrap_or_else(|error| panic!("{placement} is invalid: {error}"));
        let unknown = sequence
            .obus
            .iter()
            .position(|obu| matches!(obu, SequenceObu::Unknown(_)))
            .unwrap_or_else(|| panic!("{placement} has no unknown OBU"));
        match placement {
            "unknown-before" => assert_eq!(unknown, 0),
            "unknown-between" => assert!(unknown > 0 && unknown + 1 < sequence.obus.len()),
            "unknown-after" => assert_eq!(unknown + 1, sequence.obus.len()),
            _ => unreachable!(),
        }
    }
}
