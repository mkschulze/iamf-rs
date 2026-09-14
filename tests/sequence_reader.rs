//! `SequenceReader` streams an IA Sequence one OBU at a time (quick 260914-hoa,
//! PARSE-01, SEQ-02).
//!
//! The reader is the primitive and `parse_sequence` is that reader collected,
//! so on every committed input the two must agree: the same OBU models in the
//! same order, or the same error kind at the same absolute input offset. The
//! tests below also pin what only a streaming reader can do: yield the valid
//! prefix before a later error, report the offset of the next OBU, and stay
//! fused once an error has been yielded.
//!
//! Anti-pattern guarded against: a differential that is green because it saw
//! nothing. The whole-corpus test asserts how many inputs, `.iamf` files and
//! OBUs it compared, and that the known error input `test_000129.iamf` was
//! among them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use iamf::obu::find_obu_boundaries;
use iamf::sequence::{SequenceObu, SequenceReader, parse_sequence};
use iamf::{Error, ErrorKind, Location};

const DELIMITERS_THEN_TRUNCATED: [u8; 6] = [0x20, 0x00, 0x20, 0x00, 0x20, 0x05];

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Recursively collect regular files under `dir`, sorted, keeping those for
/// which `keep` holds. A missing directory contributes nothing.
fn collect_files(dir: &Path, keep: &dyn Fn(&Path) -> bool) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    if !dir.is_dir() {
        return Ok(found);
    }
    let mut entries = fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<io::Result<Vec<PathBuf>>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            found.extend(collect_files(&path, keep)?);
        } else if path.is_file() && keep(&path) {
            found.push(path);
        }
    }
    Ok(found)
}

fn is_iamf(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "iamf")
}

fn any_file(_: &Path) -> bool {
    true
}

#[test]
fn reader_yields_each_obu_before_a_later_error() {
    let mut reader = SequenceReader::new(&DELIMITERS_THEN_TRUNCATED);
    assert!(matches!(
        reader.next(),
        Some(Ok(SequenceObu::TemporalDelimiter(_)))
    ));
    assert!(matches!(
        reader.next(),
        Some(Ok(SequenceObu::TemporalDelimiter(_)))
    ));
    let expected = || Error::new(ErrorKind::UnexpectedEndOfInput, Location::InputOffset(6));
    assert_eq!(reader.next(), Some(Err(expected())));
    assert_eq!(reader.next(), None);
    assert_eq!(reader.next(), None);
    assert_eq!(parse_sequence(&DELIMITERS_THEN_TRUNCATED), Err(expected()));
}

#[test]
fn reader_reports_the_position_of_the_next_obu() {
    let mut reader = SequenceReader::new(&DELIMITERS_THEN_TRUNCATED);
    assert_eq!(reader.byte_position(), 0);
    assert!(matches!(reader.next(), Some(Ok(_))));
    assert_eq!(reader.byte_position(), 2);
    assert!(matches!(reader.next(), Some(Ok(_))));
    assert_eq!(reader.byte_position(), 4);
    assert!(matches!(reader.next(), Some(Err(_))));
    assert_eq!(reader.byte_position(), 4);
    assert_eq!(reader.next(), None);
    assert_eq!(reader.byte_position(), 4);
}

#[test]
fn reader_streams_the_valid_prefix_of_test_000129() {
    let bytes = fs::read(repo_path("tests/fixtures/reference/test_000129.iamf"))
        .expect("test_000129.iamf is committed");
    let mut reader = SequenceReader::new(&bytes);
    let mut yielded = Vec::new();
    for _ in 0..3 {
        let item = reader.next().expect("an item").expect("an Ok item");
        yielded.push(item);
    }
    assert!(matches!(
        yielded.first(),
        Some(SequenceObu::IaSequenceHeader(_))
    ));
    assert!(matches!(yielded.get(1), Some(SequenceObu::CodecConfig(_))));
    assert!(matches!(yielded.get(2), Some(SequenceObu::AudioElement(_))));
    assert_eq!(
        reader.next(),
        Some(Err(Error::new(
            ErrorKind::UnexpectedEndOfInput,
            Location::InputOffset(53)
        )))
    );
    assert_eq!(reader.next(), None);
    assert_eq!(reader.byte_position(), 37);

    let prefix = parse_sequence(bytes.get(..37).expect("37-byte prefix"))
        .expect("the prefix before the failing OBU parses");
    assert_eq!(prefix.obus, yielded);
}

#[test]
fn reader_carries_parameter_definitions_across_obus() {
    let bytes = [
        0x10, 0x0a, 0x01, 0x00, 0x01, 0x00, 0x05, 0x01, 0x80, 0x00, 0x00,
        0x00, // Mix Presentation
        0x18, 0x06, 0x05, 0x01, 0x01, 0x00, 0x00, 0x00, // Parameter Block
    ];
    let obus = SequenceReader::new(&bytes)
        .collect::<Result<Vec<SequenceObu>, Error>>()
        .expect("both OBUs parse");
    assert_eq!(obus.len(), 2);
    assert!(matches!(
        obus.first(),
        Some(SequenceObu::MixPresentation(_))
    ));
    assert!(matches!(obus.get(1), Some(SequenceObu::ParameterBlock(_))));
}

#[test]
fn reader_on_empty_input_yields_nothing() {
    let mut reader = SequenceReader::new(&[]);
    assert_eq!(reader.next(), None);
    assert_eq!(reader.byte_position(), 0);
    let parsed = parse_sequence(&[]).expect("empty input is an empty sequence");
    assert_eq!(parsed.obus.len(), 0);
}

#[test]
fn reader_equals_parse_sequence_on_every_committed_input() {
    let fixtures = collect_files(&repo_path("tests/fixtures"), &is_iamf).expect("walk fixtures");
    let corpus = collect_files(&repo_path("fuzz/corpus/parse_sequence"), &any_file)
        .expect("walk fuzz corpus");
    let artifacts = collect_files(&repo_path("fuzz/artifacts/parse_sequence"), &any_file)
        .expect("walk fuzz artifacts");
    let iamf_files = fixtures.len();

    let mut inputs_seen: usize = 0;
    let mut obus_seen: usize = 0;
    let mut err_paths: Vec<PathBuf> = Vec::new();

    for path in fixtures.iter().chain(&corpus).chain(&artifacts) {
        let bytes = fs::read(path).expect("read input");
        let name = path.display();

        let streamed = SequenceReader::new(&bytes).collect::<Result<Vec<SequenceObu>, Error>>();
        let eager = parse_sequence(&bytes).map(|sequence| sequence.obus);
        assert_eq!(
            streamed, eager,
            "reader and parse_sequence differ on {name}"
        );

        // Drive a fresh reader by hand so the position after every Ok item
        // can be checked against the independent header-only boundary walk.
        let mut reader = SequenceReader::new(&bytes);
        let mut positions = vec![reader.byte_position()];
        let mut yielded = Vec::new();
        let mut failed = false;
        loop {
            match reader.next() {
                Some(Ok(obu)) => {
                    yielded.push(obu);
                    positions.push(reader.byte_position());
                }
                Some(Err(_)) => {
                    failed = true;
                    break;
                }
                None => break,
            }
        }

        obus_seen = obus_seen.saturating_add(yielded.len());
        inputs_seen = inputs_seen.saturating_add(1);

        if failed {
            let pos = usize::try_from(reader.byte_position()).expect("position fits usize");
            let prefix = parse_sequence(bytes.get(..pos).expect("position within input"))
                .expect("the prefix before the failing OBU parses");
            assert_eq!(prefix.obus, yielded, "prefix differs on {name}");
            err_paths.push(path.clone());
        } else {
            let positions = positions
                .iter()
                .map(|&p| usize::try_from(p).expect("position fits usize"))
                .collect::<Vec<usize>>();
            let boundaries = find_obu_boundaries(&bytes).expect("Ok input has boundaries");
            assert_eq!(positions, boundaries, "positions differ on {name}");
        }
    }

    eprintln!(
        "sequence_reader differential: inputs={inputs_seen} iamf_files={iamf_files} \
         obus={obus_seen} err_inputs={}",
        err_paths.len()
    );
    assert!(inputs_seen >= 44, "only {inputs_seen} inputs seen");
    assert!(iamf_files >= 40, "only {iamf_files} .iamf fixtures seen");
    assert!(obus_seen >= 5000, "only {obus_seen} OBUs seen");
    assert!(
        err_paths
            .iter()
            .any(|path| path.ends_with("tests/fixtures/reference/test_000129.iamf")),
        "test_000129.iamf was not among the error inputs: {err_paths:?}"
    );
}
