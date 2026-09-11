#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/sequence_cases.rs"]
mod sequence_cases;

use hex_literal::hex;
use iamf::bits::{BitCursor, BitWriter};
use iamf::error::ErrorKind;
use iamf::obu::{
    AudioElementParam, Obu, ObuHeader, ObuType, ParameterData, find_obu_boundaries,
    read_codec_config, read_obu_with, write_codec_config, write_obu, write_obu_with,
};
use iamf::sequence::{
    ParsedSequence, SequenceObu, UnknownObu, parse_sequence, write_parsed_sequence, write_sequence,
};
use proptest::prelude::*;
use sequence_cases::{
    CanonicalCase, canonical_case_strategy, canonical_parsed_strategy, flac_codec_config_strategy,
    opus_codec_config_strategy,
};

// Enrich the existing canonical shapes locally: their original LPCM strategies
// and fixed byte-offset sentinels remain unchanged for existing consumers.
fn canonical_codec_case_strategy() -> impl Strategy<Value = CanonicalCase> {
    (
        canonical_case_strategy(),
        prop_oneof![flac_codec_config_strategy(), opus_codec_config_strategy()],
        proptest::collection::vec(any::<u8>(), 1..=16),
    )
        .prop_map(|(mut case, mut codec, trailing)| {
            codec.codec_config_id = 7;
            codec.trailing = trailing;
            case.descriptors.codec_configs = vec![codec];
            case
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn canonical_codec_sequences_round_trip(case in canonical_codec_case_strategy()) {
        for codec in &case.descriptors.codec_configs {
            prop_assert!(codec.flac_config().is_some() || codec.opus_config().is_some());
            prop_assert!(!codec.trailing.is_empty());
            prop_assert!(codec.validate().is_empty());
        }
        let expected = ParsedSequence::from_parts(&case.descriptors, &case.units)
            .expect("codec-enriched canonical parts flatten");
        let bytes = write_sequence(Vec::new(), &case.descriptors, case.units)
            .expect("codec-enriched streaming sequence writes");
        let parsed = parse_sequence(&bytes).expect("typed codec sequence parses");
        prop_assert_eq!(&parsed, &expected);
        prop_assert_eq!(write_parsed_sequence(Vec::new(), &parsed).unwrap(), bytes);
    }

    #[test]
    fn model_round_trip(sequence in canonical_parsed_strategy()) {
        let bytes = write_parsed_sequence(Vec::new(), &sequence).expect("canonical model writes");
        let parsed = parse_sequence(&bytes).expect("own bytes parse");

        prop_assert_eq!(parsed, sequence);
    }

    #[test]
    fn parsed_writer_bytes_round_trip(sequence in canonical_parsed_strategy()) {
        let bytes = write_parsed_sequence(Vec::new(), &sequence).expect("canonical model writes");
        let reparsed = parse_sequence(&bytes).expect("own bytes parse");
        let rewritten = write_parsed_sequence(Vec::new(), &reparsed).expect("parsed model writes");

        prop_assert_eq!(rewritten, bytes);
    }

    #[test]
    fn streaming_writer_bytes_round_trip(case in canonical_case_strategy()) {
        let bytes = write_sequence(Vec::new(), &case.descriptors, case.units)
            .expect("canonical model writes through the streaming primitive");
        let reparsed = parse_sequence(&bytes).expect("streaming-writer bytes parse");
        let rewritten = write_parsed_sequence(Vec::new(), &reparsed).expect("parsed model writes");

        prop_assert_eq!(rewritten, bytes);
    }

    #[test]
    fn typed_flac_codec_configs_round_trip(codec in flac_codec_config_strategy()) {
        let obu = Obu::new(ObuHeader::new(ObuType::CodecConfig), codec.clone());
        let mut writer = BitWriter::new();
        write_obu_with(&mut writer, &obu, write_codec_config).expect("valid FLAC writes");
        let bytes = writer.finish().expect("FLAC OBU is byte-aligned");

        let mut reader = BitCursor::new(&bytes);
        let parsed = read_obu_with(&mut reader, read_codec_config).expect("own FLAC parses");
        prop_assert_eq!(&parsed.payload, &codec);

        let mut rewritten = BitWriter::new();
        write_obu_with(&mut rewritten, &parsed, write_codec_config).expect("parsed FLAC writes");
        prop_assert_eq!(rewritten.finish().expect("rewritten OBU is aligned"), bytes);
    }

    #[test]
    fn typed_opus_codec_configs_round_trip(codec in opus_codec_config_strategy()) {
        let obu = Obu::new(ObuHeader::new(ObuType::CodecConfig), codec.clone());
        let mut writer = BitWriter::new();
        write_obu_with(&mut writer, &obu, write_codec_config).expect("valid Opus writes");
        let bytes = writer.finish().expect("Opus OBU is byte-aligned");

        let mut reader = BitCursor::new(&bytes);
        let parsed = read_obu_with(&mut reader, read_codec_config).expect("own Opus parses");
        prop_assert_eq!(&parsed.payload, &codec);

        let mut rewritten = BitWriter::new();
        write_obu_with(&mut rewritten, &parsed, write_codec_config).expect("parsed Opus writes");
        prop_assert_eq!(rewritten.finish().expect("rewritten OBU is aligned"), bytes);
    }
}

#[test]
fn foreign_typed_opus_config_round_trips_without_normalization() {
    let foreign = hex!(
        "00 14 02 4f 70 75 73 c0 07 00 00
         00 01 00 00 00 00 ac 44 00 01 01"
    );
    let mut reader = BitCursor::new(&foreign);
    let parsed = read_obu_with(&mut reader, read_codec_config)
        .expect("complete foreign Opus syntax is preserved as typed data");

    assert!(parsed.payload.opus_config().is_some());
    assert!(!parsed.payload.validate().is_empty());

    let mut writer = BitWriter::new();
    write_obu_with(&mut writer, &parsed, write_codec_config).expect("foreign Opus writes");
    assert_eq!(
        writer.finish().expect("foreign Opus OBU is aligned"),
        foreign
    );
}

#[test]
fn from_parts_is_an_ordering_adapter() {
    let case = sequence_cases::rich_case(2, true, vec![1, 2, 3], -7);
    let parsed = ParsedSequence::from_parts(&case.descriptors, &case.units)
        .expect("canonical parts have a flat ordering");

    assert!(!parsed.obus.is_empty());
}

#[test]
fn foreign_non_minimal_obu_size_is_the_documented_width_caveat() {
    // A zero-payload Temporal Delimiter whose legal `obu_size = 0` occupies
    // two bytes instead of the crate writer's minimal one-byte ULEB128.
    let foreign = [0x20, 0x80, 0x00];
    let parsed = parse_sequence(&foreign).expect("fixed-width foreign size is legal IAMF");
    let canonical = write_parsed_sequence(Vec::new(), &parsed).expect("parsed sequence writes");

    assert_eq!(canonical, [0x20, 0x00]);
    assert_ne!(canonical, foreign);
    assert_eq!(parse_sequence(&canonical), Ok(parsed));
}

#[test]
fn test_000015_preserves_its_raw_ungoverned_parameter_block_in_place() {
    let bytes = include_bytes!("fixtures/reference/test_000015.iamf");
    let parsed = parse_sequence(bytes).expect("valid pinned fixture parses");
    let raw_index = parsed
        .obus
        .iter()
        .position(|obu| matches!(obu, SequenceObu::UngovernedParameterBlock(_)))
        .expect("fixture carries its explicit ungoverned block");

    assert_eq!(
        raw_index, 4,
        "raw block remains immediately after descriptors"
    );
    assert_eq!(
        write_parsed_sequence(Vec::new(), &parsed).expect("flat fixture writes"),
        bytes
    );
}

#[test]
fn unknown_obu_and_raw_parameter_data_keep_exact_positions_and_offsets() {
    let case = sequence_cases::rich_case(1, false, vec![0x44, 0x55], -7);
    let mut expected = ParsedSequence::from_parts(&case.descriptors, &case.units)
        .expect("the explicit sentinel is canonical");
    expected.obus.insert(
        9,
        SequenceObu::Unknown(UnknownObu {
            header: ObuHeader::new(ObuType::Reserved(27)).with_extension(vec![0x91, 0x00, 0xfe]),
            payload: vec![0xde, 0xad, 0xbe],
        }),
    );

    let bytes = write_parsed_sequence(Vec::new(), &expected).expect("sentinel writes");
    let parsed = parse_sequence(&bytes).expect("sentinel parses");
    assert_eq!(
        parsed, expected,
        "derived equality includes every byte owner"
    );

    assert!(matches!(
        parsed.obus.first(),
        Some(SequenceObu::IaSequenceHeader(_))
    ));
    assert!(matches!(
        parsed.obus.get(1),
        Some(SequenceObu::CodecConfig(_))
    ));
    assert!(matches!(
        parsed.obus.get(2),
        Some(SequenceObu::AudioElement(_))
    ));
    assert!(matches!(
        parsed.obus.get(3),
        Some(SequenceObu::MixPresentation(_))
    ));
    assert!(matches!(
        parsed.obus.get(6),
        Some(SequenceObu::ParameterBlock(_))
    ));
    assert!(matches!(parsed.obus.get(9), Some(SequenceObu::Unknown(_))));
    assert!(matches!(
        parsed.obus.get(10),
        Some(SequenceObu::AudioFrame(_))
    ));
    assert!(matches!(
        parsed.obus.get(11),
        Some(SequenceObu::AudioFrame(_))
    ));

    let Some(SequenceObu::AudioElement(element)) = parsed.obus.get(2) else {
        panic!("index 2 is the known Audio Element sentinel");
    };
    let Some(SequenceObu::CodecConfig(codec)) = parsed.obus.get(1) else {
        panic!("index 1 is the known Codec Config sentinel");
    };
    assert_eq!(codec.trailing, [0xc3, 0xd4]);
    assert_eq!(element.trailing, [0xa1, 0xb2]);
    let Some(SequenceObu::MixPresentation(presentation)) = parsed.obus.get(3) else {
        panic!("index 3 is the known Mix Presentation sentinel");
    };
    assert_eq!(presentation.trailing, [0x71, 0x72]);
    let Some(AudioElementParam::Extension {
        param_definition_type,
        bytes: definition_bytes,
    }) = element.payload.params.get(2)
    else {
        panic!("third definition is the raw extension definition");
    };
    assert_eq!(*param_definition_type, 7);
    assert_eq!(definition_bytes, &[0x0c, 0x80, 0xf7, 0x02, 0xc5]);

    let Some(SequenceObu::ParameterBlock(raw_block)) = parsed.obus.get(6) else {
        panic!("index 6 is the raw Parameter Block sentinel");
    };
    assert_eq!(raw_block.payload.subblocks.len(), 2);
    assert_eq!(
        raw_block
            .payload
            .subblocks
            .first()
            .map(|subblock| &subblock.data),
        Some(&ParameterData::Raw(vec![0x44, 0x55]))
    );
    assert_eq!(
        raw_block
            .payload
            .subblocks
            .get(1)
            .map(|subblock| &subblock.data),
        Some(&ParameterData::Raw(Vec::new()))
    );
    let Some(SequenceObu::ParameterBlock(known_trailing)) = parsed.obus.get(4) else {
        panic!("index 4 is the known trailing-byte sentinel");
    };
    assert_eq!(known_trailing.trailing, [0xde, 0xad]);

    let Some(SequenceObu::Unknown(unknown)) = parsed.obus.get(9) else {
        panic!("index 9 is the unknown OBU sentinel");
    };
    assert_eq!(
        unknown.header.extension.as_deref(),
        Some(&[0x91, 0x00, 0xfe][..])
    );
    assert_eq!(unknown.payload, [0xde, 0xad, 0xbe]);

    let boundaries = find_obu_boundaries(&bytes).expect("all sentinel boundaries are valid");
    assert_eq!(
        boundaries,
        vec![0, 8, 26, 65, 126, 134, 143, 152, 173, 181, 190, 194, 199]
    );
    assert_eq!(
        bytes.get(181..190),
        Some(&[0xd9, 0x07, 0x03, 0x91, 0x00, 0xfe, 0xde, 0xad, 0xbe][..])
    );
}

#[test]
fn truncation_and_corrupt_known_syntax_never_become_unknown_obus() {
    let mut truncated = BitWriter::new();
    write_obu(
        &mut truncated,
        &ObuHeader::new(ObuType::IaSequenceHeader),
        &[0x69, 0x61],
    )
    .expect("malformed known payload still has valid framing");
    let truncated = truncated.finish().expect("whole-byte vector");

    let error = parse_sequence(&truncated).expect_err("known sequence header is truncated");
    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert!(
        parse_sequence(&[0x20, 0x02, 0xaa]).is_err(),
        "truncated OBU fails"
    );
}
