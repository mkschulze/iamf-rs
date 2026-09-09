//! Field-level expectations for every committed foreign IAMF fixture.

#[path = "support/reference_expectations.rs"]
mod reference_expectations;

use std::path::{Path, PathBuf};

use iamf::error::Location;
use iamf::obu::{
    AudioElementParam, DecoderConfig, DurationFields, ParamDefinition, ParameterData,
    ReconGainElement, ReconGainInfoParameterData,
};
use iamf::sequence::{SequenceObu, parse_sequence, write_parsed_sequence};
use reference_expectations::{
    NEGATIVE_EXPECTATIONS, NegativeDisposition, POSITIVE_EXPECTATIONS, RAW_CODEC_EXPECTATIONS,
};
use sha2::{Digest, Sha256};

fn collect_iamf(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir).expect("reference fixture directory");
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_iamf(&path, base, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "iamf")
        {
            out.push(
                path.strip_prefix(base)
                    .expect("fixture below reference root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn expectation_inventory_is_a_bijection() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference");
    let mut actual = Vec::new();
    collect_iamf(&root, &root, &mut actual);
    actual.sort();

    let mut expected: Vec<String> = POSITIVE_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.path.to_owned())
        .collect();
    expected.extend(
        NEGATIVE_EXPECTATIONS
            .iter()
            .map(|expectation| expectation.path.to_owned()),
    );
    let original_len = expected.len();
    expected.sort();
    expected.dedup();

    assert_eq!(POSITIVE_EXPECTATIONS.len(), 37, "positive ledger count");
    assert_eq!(NEGATIVE_EXPECTATIONS.len(), 2, "negative ledger count");
    assert_eq!(original_len, 39, "37 positives plus two negatives");
    assert_eq!(expected.len(), original_len, "duplicate expectation path");
    assert_eq!(actual, expected, "missing or unexpected reference fixture");
    assert!(
        POSITIVE_EXPECTATIONS
            .iter()
            .all(|expectation| !expectation.path.is_empty()),
        "positive paths must not be empty"
    );
    assert!(
        NEGATIVE_EXPECTATIONS
            .iter()
            .all(|expectation| !expectation.path.is_empty())
    );
}

fn fixture_bytes(path: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/reference")
            .join(path),
    )
    .unwrap_or_else(|error| panic!("{path}: fixture read failed: {error}"))
}

fn semantic_sha256(sequence: &iamf::sequence::ParsedSequence) -> String {
    // Derived Debug includes every public modeled field, collection order,
    // header flag, trailing byte and raw payload. Findings are appended in
    // their returned order so validation is covered without sorting.
    let projection = format!("{sequence:#?}\nfindings={:#?}", sequence.validate());
    let mut hasher = Sha256::new();
    hasher.update(projection.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[test]
fn every_positive_matches_its_complete_modeled_field_ledger() {
    for expectation in POSITIVE_EXPECTATIONS {
        let bytes = fixture_bytes(expectation.path);
        let sequence = parse_sequence(&bytes)
            .unwrap_or_else(|error| panic!("{}: positive parse failed: {error}", expectation.path));
        assert_eq!(
            semantic_sha256(&sequence),
            expectation.semantic_sha256,
            "{}: complete ordered modeled-field projection differs",
            expectation.path
        );
        assert_eq!(
            write_parsed_sequence(Vec::new(), &sequence)
                .unwrap_or_else(|error| panic!("{}: flat write failed: {error}", expectation.path)),
            bytes,
            "{}: complete byte ownership or OBU position differs",
            expectation.path
        );
    }
}

#[test]
fn named_negative_dispositions_are_exact() {
    for expectation in NEGATIVE_EXPECTATIONS {
        let bytes = fixture_bytes(expectation.path);
        match expectation.disposition {
            NegativeDisposition::StructuralError { kind, offset } => {
                let error = parse_sequence(&bytes)
                    .expect_err("structural-negative fixture must not produce a model");
                assert_eq!(
                    format!("{:?}", error.kind()),
                    kind,
                    "{}: error kind",
                    expectation.path
                );
                assert_eq!(
                    error.at(),
                    Location::InputOffset(offset),
                    "{}: error offset",
                    expectation.path
                );
            }
            NegativeDisposition::ValidationFinding {
                zero_frame_size,
                field,
                message,
            } => {
                let sequence = parse_sequence(&bytes).unwrap_or_else(|error| {
                    panic!("{}: semantic negative parses: {error}", expectation.path)
                });
                let config = sequence
                    .obus
                    .iter()
                    .find_map(|obu| match obu {
                        SequenceObu::CodecConfig(obu) => Some(&obu.payload),
                        _ => None,
                    })
                    .expect("semantic negative carries a Codec Config");
                assert_eq!(
                    config.num_samples_per_frame, zero_frame_size,
                    "{}: stored frame size",
                    expectation.path
                );
                assert_eq!(
                    sequence.validate(),
                    vec![iamf::error::Finding {
                        at: Location::Field(field),
                        message: message.to_owned()
                    }],
                    "{}: exact ordered validation findings",
                    expectation.path
                );
            }
        }
    }
}

#[test]
fn every_non_lpcm_decoder_config_remains_raw_at_its_exact_boundary() {
    for expectation in RAW_CODEC_EXPECTATIONS {
        let bytes = fixture_bytes(expectation.path);
        let sequence = parse_sequence(&bytes).expect("raw-codec fixture parses");
        let config = sequence
            .obus
            .iter()
            .find_map(|obu| match obu {
                SequenceObu::CodecConfig(obu) => Some(&obu.payload),
                _ => None,
            })
            .expect("fixture carries a Codec Config");
        let DecoderConfig::Raw {
            codec_id,
            bytes: raw,
        } = &config.decoder_config
        else {
            panic!(
                "{}: non-LPCM config was interpreted early",
                expectation.path
            );
        };
        assert_eq!(
            config.codec_id, expectation.fourcc,
            "{}: FourCC",
            expectation.path
        );
        assert_eq!(
            *codec_id, expectation.fourcc,
            "{}: raw owner FourCC",
            expectation.path
        );
        assert_eq!(
            raw.len(),
            expectation.len,
            "{}: raw codec length",
            expectation.path
        );
        assert_eq!(
            sha256_hex(raw),
            expectation.sha256,
            "{}: raw codec digest",
            expectation.path
        );
        assert_eq!(
            bytes.get(expectation.offset..expectation.offset.saturating_add(expectation.len)),
            Some(raw.as_slice()),
            "{}: raw codec byte position",
            expectation.path
        );
    }
}

#[test]
fn test_000059_asserts_demixing_and_every_recon_gain_layer_flag_and_gain() {
    let sequence = parse_sequence(&fixture_bytes("test_000059.iamf")).expect("positive fixture");
    let element = sequence
        .obus
        .iter()
        .find_map(|obu| match obu {
            SequenceObu::AudioElement(obu) => Some(&obu.payload),
            _ => None,
        })
        .expect("fixture carries its Audio Element");
    let duration = || {
        Some(DurationFields {
            duration: 960,
            constant_subblock_duration: 960,
            subblock_durations: Vec::new(),
        })
    };
    assert_eq!(
        element.params,
        vec![
            AudioElementParam::Demixing {
                definition: ParamDefinition {
                    parameter_id: 998,
                    parameter_rate: 48_000,
                    reserved: 0,
                    duration_fields: duration(),
                },
                // The textproto's named DMIXP_MODE_2 has wire value 1.
                default_dmixp_mode: 1,
                default_reserved: 0,
                default_w: 0,
                default_w_reserved: 0,
            },
            AudioElementParam::ReconGain {
                definition: ParamDefinition {
                    parameter_id: 101,
                    parameter_rate: 48_000,
                    reserved: 0,
                    duration_fields: duration(),
                },
            },
        ],
        "test_000059.iamf: Demixing and Recon Gain definitions"
    );

    let blocks: Vec<_> = sequence
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::ParameterBlock(obu) if obu.payload.parameter_id == 101 => {
                Some(&obu.payload)
            }
            _ => None,
        })
        .collect();
    assert_eq!(blocks.len(), 26, "test_000059.iamf: Recon Gain block count");
    let mut gains = [0_u8; 12];
    for index in [0_usize, 2, 3, 4] {
        gains[index] = 255;
    }
    let expected = ParameterData::ReconGain(ReconGainInfoParameterData {
        layers: vec![
            None,
            Some(ReconGainElement {
                recon_gain_flag: 0x1d,
                recon_gain: gains,
            }),
        ],
    });
    for (index, block) in blocks.iter().enumerate() {
        assert_eq!(
            block.duration_fields, None,
            "test_000059.iamf: OBU Recon Gain {index} duration source"
        );
        assert_eq!(
            block.subblocks.len(),
            1,
            "test_000059.iamf: OBU Recon Gain {index} subblock count"
        );
        let subblock = block.subblocks.first().expect("one asserted subblock");
        assert_eq!(
            subblock.subblock_duration, None,
            "test_000059.iamf: OBU Recon Gain {index} subblock duration source"
        );
        assert_eq!(
            subblock.data, expected,
            "test_000059.iamf: OBU Recon Gain {index} layers/flag/gains"
        );
    }
}
