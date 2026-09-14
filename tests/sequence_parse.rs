//! Phase 2 sequence-parser context tests.

#[path = "support/test_000003.rs"]
mod support;

use hex_literal::hex;
use iamf::bits::BitWriter;
use iamf::error::{ErrorKind, Finding, Location};
use iamf::model::DescriptorSet;
use iamf::model::layout::{ExpandedLoudspeakerLayout, LoudspeakerLayout};
use iamf::obu::{
    AnchorElement, AnchoredLoudness, AudioElement, AudioElementParam, AudioElementType, AudioFrame,
    BlockDurationFields, ChannelAudioLayerConfig, HeadphonesRenderingMode, IaSequenceHeader,
    MixGainParameterData, MixPresentation, Obu, ObuHeader, ObuType, ParamDefinition,
    ParamDefinitionRegistry, ParameterBlock, ParameterData, ParameterDataContext,
    ParameterSubblock, SubMixAudioElement, TemporalDelimiter, write_obu,
};
use iamf::sequence::{
    ParsedSequence, SequenceObu, UngovernedParameterBlock, UnknownObu, parse_sequence,
    write_parsed_sequence,
};
use support::published_descriptor_set;

// These literals independently fix both codec prefix lengths and their common
// nine-byte Codec Config header (after the two-byte OBU header).
const FLAC_OBU: &[u8] = &hex!(
    "00 2f 01 66 4c 61 43 80 01 00 00
     80 00 00 22 00 80 00 80 00 00 00 00 00 00
     0b b8 02 f0 00 00 00 00
     00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00"
);
const OPUS_OBU: &[u8] = &hex!(
    "00 14 02 4f 70 75 73 c0 07 ff fc
     01 02 01 38 00 00 bb 80 00 00 00"
);

#[allow(clippy::expect_used, clippy::panic)] // Hand-derived bounded test vectors.
fn assert_every_codec_prefix_is_bounded(obu: &[u8], codec_id: [u8; 4], prefix_len: usize) {
    use iamf::obu::DecoderConfig;

    let prologue = hex!("f8 06 69 61 6d 66 00 00");
    // More than a whole codec prefix follows the OBU, so an unbounded reader
    // would incorrectly steal the delimiter/frame bytes and claim typed data.
    let mut suffix = vec![0x20, 0x00, 0x30, 0x40];
    suffix.extend_from_slice(&[0xa5; 64]);
    for length in 0..=prefix_len {
        let end = 11_usize.saturating_add(length);
        let mut bounded = obu.get(..end).expect("fixed prefix slice").to_vec();
        *bounded.get_mut(1).expect("one-byte OBU size") =
            u8::try_from(9_usize.saturating_add(length)).expect("small payload");
        let bytes = [prologue.as_slice(), &bounded, &suffix].concat();
        let parsed = parse_sequence(&bytes).expect("complete enclosing OBU parses at every length");
        assert_eq!(parsed.obus.len(), 4, "length {length}");
        let Some(SequenceObu::CodecConfig(codec)) = parsed.obus.get(1) else {
            panic!("bounded known codec must keep its sequence position");
        };
        assert_eq!(codec.payload.codec_id, codec_id);
        assert!(codec.trailing.is_empty());
        assert!(codec.payload.trailing.is_empty());
        if length < prefix_len {
            assert_eq!(
                codec.payload.decoder_config,
                DecoderConfig::Raw {
                    codec_id,
                    bytes: obu.get(11..end).expect("raw prefix bytes").to_vec(),
                }
            );
        } else if codec_id == *b"fLaC" {
            assert!(codec.payload.flac_config().is_some());
        } else {
            assert!(codec.payload.opus_config().is_some());
        }
        assert!(matches!(
            parsed.obus.get(2),
            Some(SequenceObu::TemporalDelimiter(_))
        ));
        let Some(SequenceObu::AudioFrame(frame)) = parsed.obus.get(3) else {
            panic!("following frame remains independently framed");
        };
        assert_eq!(frame.payload.payload, [0xa5; 64]);
        assert_eq!(write_parsed_sequence(Vec::new(), &parsed), Ok(bytes));

        if length < prefix_len {
            // Keep the original declared size: an actual truncated OBU must
            // fail transactionally at the absolute payload start (8 + 2).
            let truncated = [prologue.as_slice(), obu.get(..end).expect("truncated OBU")].concat();
            let error = parse_sequence(&truncated).expect_err("enclosing OBU is incomplete");
            assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
            assert_eq!(error.at(), Location::InputOffset(10));
        }
    }
}

#[test]
fn every_flac_prefix_length_in_a_sequence_is_raw_until_38_and_truncation_is_positioned() {
    assert_every_codec_prefix_is_bounded(FLAC_OBU, *b"fLaC", 38);
}

#[test]
fn every_opus_prefix_length_in_a_sequence_is_raw_until_11_and_truncation_is_positioned() {
    assert_every_codec_prefix_is_bounded(OPUS_OBU, *b"Opus", 11);
}

fn definition(parameter_id: u32) -> ParamDefinition {
    ParamDefinition::mode_1(parameter_id, 48_000)
}

#[test]
fn an_empty_registry_has_no_parameter_definition() {
    let registry = ParamDefinitionRegistry::default();

    assert!(registry.get(7).is_none());
    assert!(registry.is_empty());
}

#[test]
fn duplicate_parameter_ids_are_retained_and_lookup_binds_to_the_first() {
    let mut registry = ParamDefinitionRegistry::default();
    registry.register(definition(7), ParameterDataContext::MixGain);
    registry.register(definition(7), ParameterDataContext::Demixing);

    assert_eq!(registry.len(), 2, "duplicates remain observable");
    assert_eq!(
        registry.get(7).map(|entry| &entry.context),
        Some(&ParameterDataContext::MixGain),
        "lookup follows the first definition in wire order"
    );
}

#[test]
fn descriptor_observation_preserves_nested_wire_order() {
    let mut descriptors = published_descriptor_set();
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![
        AudioElementParam::Demixing {
            definition: definition(11),
            default_dmixp_mode: 0,
            default_reserved: 0,
            default_w: 0,
            default_w_reserved: 0,
        },
        AudioElementParam::ReconGain {
            definition: definition(12),
        },
    ];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the descriptor definitions are structurally readable");
    let ids: Vec<u32> = registry
        .entries()
        .iter()
        .map(|entry| entry.definition.parameter_id)
        .collect();

    assert_eq!(ids, vec![11, 12, 100, 100]);
}

#[test]
fn registry_lookup_follows_the_audio_element_order_emitted_by_the_writer() {
    let mut later = support::published_audio_element().payload;
    later.audio_element_id = 20;
    later.params = vec![AudioElementParam::Demixing {
        definition: definition(55),
        default_dmixp_mode: 0,
        default_reserved: 0,
        default_w: 0,
        default_w_reserved: 0,
    }];
    let mut earlier = later.clone();
    earlier.audio_element_id = 10;
    earlier.params = vec![AudioElementParam::ReconGain {
        definition: definition(55),
    }];
    let mut descriptors = published_descriptor_set();
    descriptors.audio_elements = vec![later, earlier];
    descriptors.mix_presentations.clear();

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the definitions are structurally readable");

    assert_eq!(
        registry.get(55).map(|entry| &entry.context),
        Some(&ParameterDataContext::ReconGain {
            recon_gain_is_present: vec![false],
        }),
        "Audio Element 10 is emitted before Audio Element 20"
    );
}

#[test]
fn a_parameter_block_whose_id_only_appears_inside_extension_bytes_is_ungoverned() {
    let mut descriptors = published_descriptor_set();
    // Opaque bytes 07 01 80 aa would decode as ParamDefinition { id: 7,
    // rate: 1, mode: 1 } + aa, but param_definition_type 9 is not recognised,
    // so parsers ignore the bytes (index.bs:772, :796).
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![AudioElementParam::Extension {
        param_definition_type: 9,
        bytes: vec![0x07, 0x01, 0x80, 0xaa],
    }];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("opaque extension bytes are never decoded");
    assert!(
        registry.get(7).is_none(),
        "an extension definition never governs a Parameter Block"
    );

    let mut model =
        ParsedSequence::from_parts(&descriptors, &[]).expect("descriptor set is orderable");
    // id 7 (07), duration 1 (01), constant_subblock_duration 1 (01), then one
    // opaque subblock: size 1 (01) and byte aa.
    model.obus.push(SequenceObu::UngovernedParameterBlock(
        UngovernedParameterBlock {
            header: ObuHeader::new(ObuType::ParameterBlock),
            payload: vec![0x07, 0x01, 0x01, 0x01, 0xaa],
        },
    ));

    let bytes =
        write_parsed_sequence(Vec::new(), &model).expect("an ungoverned block writes verbatim");
    let parsed = parse_sequence(&bytes).expect("the written sequence parses");
    assert_eq!(parsed, model);
    assert!(matches!(
        parsed.obus.last(),
        Some(SequenceObu::UngovernedParameterBlock(_))
    ));
    assert_eq!(write_parsed_sequence(Vec::new(), &parsed), Ok(bytes));
}

#[test]
fn opaque_extension_definitions_without_a_param_definition_prefix_round_trip() {
    let mut descriptors = published_descriptor_set();
    // Empty bytes, a lone continuation byte (80) and a truncated prefix
    // (05 01: id 5, rate 1, no mode byte). None is a ParamDefinition prefix,
    // and none has to be one.
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![
        AudioElementParam::Extension {
            param_definition_type: 7,
            bytes: Vec::new(),
        },
        AudioElementParam::Extension {
            param_definition_type: 3,
            bytes: vec![0x80],
        },
        AudioElementParam::Extension {
            param_definition_type: 9,
            bytes: vec![0x05, 0x01],
        },
    ];

    let model = ParsedSequence::from_parts(&descriptors, &[])
        .expect("opaque extension definitions are not decoded");
    let bytes = write_parsed_sequence(Vec::new(), &model).expect("opaque extensions write");
    assert_eq!(parse_sequence(&bytes), Ok(model));
}

#[test]
fn extension_bytes_that_decode_to_a_mix_gain_id_do_not_shadow_it() {
    let mut descriptors = published_descriptor_set();
    // 64 01 80 would decode as ParamDefinition { id: 100, rate: 1, mode: 1 },
    // the id of both published Mix Gain definitions. It must not bind block 100.
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![AudioElementParam::Extension {
        param_definition_type: 7,
        bytes: vec![0x64, 0x01, 0x80],
    }];

    let mut model =
        ParsedSequence::from_parts(&descriptors, &[]).expect("descriptor set is orderable");
    model.obus.push(SequenceObu::ParameterBlock(Obu::new(
        ObuHeader::new(ObuType::ParameterBlock),
        ParameterBlock {
            parameter_id: 100,
            duration_fields: Some(BlockDurationFields {
                duration: 1,
                constant_subblock_duration: 1,
            }),
            subblocks: vec![ParameterSubblock {
                subblock_duration: None,
                data: ParameterData::MixGain(MixGainParameterData::Step {
                    start_point_value: 256,
                }),
            }],
        },
    )));

    let bytes =
        write_parsed_sequence(Vec::new(), &model).expect("block 100 is governed by Mix Gain");
    let parsed = parse_sequence(&bytes).expect("the written sequence parses");
    assert_eq!(parsed, model);
    let Some(SequenceObu::ParameterBlock(block)) = parsed.obus.last() else {
        panic!("block 100 stays a governed Parameter Block");
    };
    assert!(matches!(
        block
            .payload
            .subblocks
            .first()
            .map(|subblock| &subblock.data),
        Some(ParameterData::MixGain(_))
    ));
}

#[test]
fn descriptor_validation_keeps_nested_duplicate_findings_beside_an_opaque_extension() {
    let mut descriptors = published_descriptor_set();
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![AudioElementParam::Extension {
        param_definition_type: 7,
        bytes: Vec::new(),
    }];

    let findings = descriptors.validate();

    assert!(
        findings.iter().any(|finding| {
            finding.at == Location::Field("parameter_id")
                && finding
                    .message
                    .contains("parameter_id 100 appears at nested definition indices 0 and 1")
        }),
        "{findings:?}"
    );
}

#[test]
fn recon_gain_context_has_exactly_one_presence_flag_per_channel_layer() {
    let mut descriptors = published_descriptor_set();
    let element = descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element");
    element.params = vec![AudioElementParam::ReconGain {
        definition: definition(12),
    }];
    let AudioElementType::ChannelBased(config) = &mut element.audio_element_type else {
        unreachable!("published fixture is channel based");
    };
    config.scalable_channel_layout.layers = vec![
        ChannelAudioLayerConfig::new(LoudspeakerLayout::Stereo, 1, 1),
        ChannelAudioLayerConfig {
            recon_gain_is_present: true,
            reserved: 0,
            ..ChannelAudioLayerConfig::new(LoudspeakerLayout::Ch5_1, 3, 1)
        },
    ];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the descriptor definitions are structurally readable");

    assert_eq!(
        registry.get(12).map(|entry| &entry.context),
        Some(&ParameterDataContext::ReconGain {
            recon_gain_is_present: vec![false, true],
        })
    );
}

#[test]
fn descriptor_validation_reports_duplicate_nested_parameter_ids() {
    let descriptors: DescriptorSet = published_descriptor_set();

    assert!(descriptors.validate().iter().any(|finding| {
        finding.at == iamf::error::Location::Field("parameter_id")
            && finding.message.contains("parameter_id 100")
    }));
}

fn parsed_round_trip(sequence: &ParsedSequence) -> ParsedSequence {
    let written = write_parsed_sequence(Vec::new(), sequence);
    assert!(written.is_ok(), "flat sequence writes: {written:?}");
    let bytes = written.unwrap_or_default();
    let parsed = parse_sequence(&bytes);
    assert!(parsed.is_ok(), "written flat sequence parses: {parsed:?}");
    parsed.unwrap_or_default()
}

fn mode_1_block(parameter_id: u32) -> Obu<ParameterBlock> {
    Obu::new(
        ObuHeader::new(ObuType::ParameterBlock),
        ParameterBlock {
            parameter_id,
            duration_fields: Some(BlockDurationFields {
                duration: 1,
                constant_subblock_duration: 1,
            }),
            subblocks: vec![ParameterSubblock {
                subblock_duration: None,
                data: ParameterData::MixGain(MixGainParameterData::Step {
                    start_point_value: 0,
                }),
            }],
        },
    )
}

#[test]
fn empty_and_descriptor_only_inputs_parse_without_synthesis() {
    assert_eq!(parse_sequence(&[]), Ok(ParsedSequence { obus: Vec::new() }));

    let bytes = support::descriptor_bytes(&published_descriptor_set());
    let parsed = parse_sequence(&bytes).expect("descriptor prologue parses");
    assert_eq!(parsed.obus.len(), 4);
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
    assert!(parsed.temporal_unit_ranges().is_empty());
    assert_eq!(write_parsed_sequence(Vec::new(), &parsed), Ok(bytes));
}

#[test]
fn every_known_variant_and_all_reserved_types_keep_flat_wire_order() {
    let descriptors = published_descriptor_set();
    let mut obus = vec![
        SequenceObu::IaSequenceHeader(support::published_sequence_header()),
        SequenceObu::CodecConfig(support::published_codec_config()),
        SequenceObu::AudioElement(support::published_audio_element()),
        SequenceObu::MixPresentation(support::published_mix_presentation()),
        SequenceObu::ParameterBlock(mode_1_block(100)),
        SequenceObu::TemporalDelimiter(Obu::new(
            ObuHeader::new(ObuType::TemporalDelimiter),
            TemporalDelimiter,
        )),
        SequenceObu::AudioFrame(AudioFrame::new(0, vec![0xaa]).into_obu(None)),
    ];
    for obu_type in 24_u8..=30 {
        obus.push(SequenceObu::Unknown(UnknownObu {
            header: ObuHeader::new(ObuType::Reserved(obu_type))
                .with_extension(vec![obu_type, 0xee]),
            payload: vec![0xf0, obu_type],
        }));
    }
    let sequence = ParsedSequence { obus };

    let reparsed = parsed_round_trip(&sequence);

    assert_eq!(reparsed, sequence);
    assert_eq!(
        reparsed
            .obus
            .iter()
            .filter_map(|obu| match obu {
                SequenceObu::Unknown(unknown) => Some(unknown.header.obu_type.value()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        (24_u8..=30).collect::<Vec<_>>()
    );
    assert_eq!(
        ParamDefinitionRegistry::from_descriptors(&descriptors)
            .expect("fixture definitions")
            .get(100)
            .map(|entry| &entry.context),
        Some(&ParameterDataContext::MixGain)
    );
}

#[test]
fn redundant_descriptors_and_unknown_obus_remain_at_their_exact_positions() {
    let mut redundant = support::published_codec_config();
    redundant.header = redundant.header.with_redundant_copy(true);
    redundant.trailing = vec![0xde, 0xad];
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::Unknown(UnknownObu {
                header: ObuHeader::new(ObuType::Reserved(24)),
                payload: vec![1, 2, 3],
            }),
            SequenceObu::CodecConfig(redundant),
        ],
    };

    assert_eq!(parsed_round_trip(&sequence), sequence);
}

#[test]
fn a_bounded_parameter_block_without_governing_context_is_preserved_and_diagnosed() {
    // Header type 3, payload size 4, parameter_id 101, followed by syntax that
    // cannot be interpreted without the missing definition.
    let bytes = [0x18, 0x04, 0x65, 0x00, 0xbb, 0xcc];
    let parsed = parse_sequence(&bytes).expect("bounded ungoverned block parses as raw");

    assert_eq!(
        parsed.obus,
        vec![SequenceObu::UngovernedParameterBlock(
            UngovernedParameterBlock {
                header: ObuHeader::new(ObuType::ParameterBlock),
                payload: vec![0x65, 0x00, 0xbb, 0xcc],
            }
        )]
    );
    assert_eq!(
        parsed.validate(),
        vec![iamf::error::Finding {
            at: Location::Field("parameter_id"),
            message: "parameter block references parameter_id 101, which no definition in this sequence carries".to_owned(),
        }]
    );
    assert_eq!(
        write_parsed_sequence(Vec::new(), &parsed),
        Ok(bytes.to_vec())
    );
}

#[test]
fn malformed_governed_parameter_block_remains_a_structural_error() {
    let mut bytes = support::descriptor_bytes(&published_descriptor_set());
    let payload_offset = u64::try_from(bytes.len()).unwrap_or(0).saturating_add(2);
    // parameter_id 100 is governed by the published Mix Presentation. Its
    // mode-1 duration starts an unterminated ULEB128 and must not become raw.
    bytes.extend_from_slice(&[0x18, 0x02, 0x64, 0x80]);

    let error = parse_sequence(&bytes).expect_err("governed syntax is truncated");
    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert_eq!(
        error.at(),
        Location::InputOffset(payload_offset.saturating_add(2))
    );
}

#[test]
fn a_valid_prefix_plus_truncated_suffix_returns_no_partial_model_and_absolute_offset() {
    let mut bytes = support::descriptor_bytes(&published_descriptor_set());
    let truncated_obu_start = bytes.len();
    // Codec Config: complete header and size, but only one of two payload bytes.
    bytes.extend_from_slice(&[0x00, 0x02, 0x01]);

    let err = parse_sequence(&bytes).expect_err("the entire parse is transactional");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnexpectedEndOfInput | ErrorKind::TruncatedObu
    ));
    assert!(
        matches!(err.at(), Location::InputOffset(at) if at >= u64::try_from(truncated_obu_start).unwrap_or(0)),
        "error is absolute: {err}"
    );
}

#[test]
fn flat_validation_reports_local_then_duplicate_then_reference_then_profile_findings_in_wire_order()
{
    let mut invalid_header = support::published_sequence_header();
    invalid_header.payload.ia_code = 0;
    let mut first_config = support::published_codec_config();
    first_config.payload.codec_config_id = 7;
    let mut duplicate_config = first_config.clone();
    duplicate_config.header = duplicate_config.header.with_redundant_copy(true);
    let mut missing_element = support::published_audio_element();
    missing_element.payload.codec_config_id = 999;
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::IaSequenceHeader(invalid_header),
            SequenceObu::CodecConfig(first_config),
            SequenceObu::CodecConfig(duplicate_config),
            SequenceObu::AudioElement(missing_element),
            SequenceObu::AudioFrame(AudioFrame::new(77, vec![]).into_obu(None)),
        ],
    };

    let messages: Vec<String> = sequence
        .validate()
        .into_iter()
        .map(|finding| finding.message)
        .collect();

    assert_eq!(messages.len(), 5, "{messages:?}");
    assert!(
        messages.first().is_some_and(|m| m.contains("ia_code")),
        "{messages:?}"
    );
    assert!(
        messages
            .get(1)
            .is_some_and(|m| m.contains("codec_config_id 7")),
        "{messages:?}"
    );
    assert!(
        messages
            .get(2)
            .is_some_and(|m| m.contains("codec_config_id 999")),
        "{messages:?}"
    );
    assert!(
        messages
            .get(3)
            .is_some_and(|m| m.contains("audio_substream_id 77")),
        "{messages:?}"
    );
    // The sequence has no Mix Presentation, so none complies with its
    // primary_profile 0 (IAMF v1.1.0 index.bs:1929).
    assert_eq!(
        messages.get(4).map(String::as_str),
        Some("no Mix Presentation complies with primary_profile 0 (IAMF v1.1.0 index.bs:1929)"),
        "{messages:?}"
    );
}

#[test]
fn canonical_grouping_uses_only_delimiters_and_wire_distinguishable_transitions() {
    let descriptor = SequenceObu::CodecConfig(support::published_codec_config());
    let delimiter = || {
        SequenceObu::TemporalDelimiter(Obu::new(
            ObuHeader::new(ObuType::TemporalDelimiter),
            TemporalDelimiter,
        ))
    };
    let frame = |id| SequenceObu::AudioFrame(AudioFrame::new(id, vec![]).into_obu(None));
    let block = || SequenceObu::ParameterBlock(mode_1_block(100));

    let delimited = ParsedSequence {
        obus: vec![
            descriptor.clone(),
            delimiter(),
            frame(0),
            frame(1),
            delimiter(),
            frame(0),
        ],
    };
    assert_eq!(delimited.temporal_unit_ranges(), vec![1..4, 4..6]);

    let delimiter_free = ParsedSequence {
        obus: vec![descriptor, block(), frame(0), frame(1), block(), frame(0)],
    };
    assert_eq!(delimiter_free.temporal_unit_ranges(), vec![1..4, 4..6]);

    let repeated_substream = ParsedSequence {
        obus: vec![frame(0), frame(1), frame(0)],
    };
    assert_eq!(repeated_substream.temporal_unit_ranges(), vec![0..2, 2..3]);

    let unknown_between_frames = ParsedSequence {
        obus: vec![
            frame(0),
            SequenceObu::Unknown(UnknownObu {
                header: ObuHeader::new(ObuType::Reserved(30)),
                payload: vec![0xff],
            }),
            frame(1),
        ],
    };
    assert_eq!(unknown_between_frames.temporal_unit_ranges(), vec![0..3]);
}

#[test]
fn raw_unknown_payload_is_indivisible_and_preserves_header_flags() {
    let header = ObuHeader::new(ObuType::Reserved(29))
        .with_redundant_copy(true)
        .with_extension(vec![0x11, 0x22]);
    let mut writer = BitWriter::new();
    write_obu(&mut writer, &header, &[0xaa, 0xbb, 0xcc]).expect("unknown OBU writes");
    let bytes = writer.finish().expect("byte aligned");

    let parsed = parse_sequence(&bytes).expect("unknown OBU parses");
    let unknown = match parsed.obus.first() {
        Some(SequenceObu::Unknown(unknown)) => unknown,
        _ => panic!("reserved type dispatches as Unknown"),
    };
    assert_eq!(unknown.header, header);
    assert_eq!(unknown.payload, vec![0xaa, 0xbb, 0xcc]);
    assert_eq!(write_parsed_sequence(Vec::new(), &parsed), Ok(bytes));
}

// ---------------------------------------------------------------------------
// Profile compliance findings (quick 260913-qk3, item 5)
// ---------------------------------------------------------------------------

fn f1(primary: u8) -> Finding {
    Finding {
        at: Location::Field("primary_profile"),
        message: format!(
            "no Mix Presentation complies with primary_profile {primary} (IAMF v1.1.0 index.bs:1929)"
        ),
    }
}

fn f2(additional: u8) -> Finding {
    Finding {
        at: Location::Field("additional_profile"),
        message: format!(
            "no Mix Presentation complies with additional_profile {additional} (IAMF v1.1.0 index.bs:596)"
        ),
    }
}

fn f3(unique: usize, needed: u8, primary: u8) -> Finding {
    Finding {
        at: Location::Field("primary_profile"),
        message: format!(
            "the IA Sequence's {unique} unique Audio Elements need profile {needed} or higher, \
             above primary_profile {primary} (IAMF v1.0.0-errata index.bs:1852-1873, adopted by \
             v1.1.0 index.bs:1936, :1943)"
        ),
    }
}

/// Only the profile findings F1-F4, in their returned order.
fn profile_findings(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| {
            finding.message.contains("complies with")
                || finding.message.contains("unique Audio Elements")
        })
        .collect()
}

/// A sub-mix element reference to `audio_element_id`, cloned from MP 42's.
fn reference_to(audio_element_id: u32) -> Option<SubMixAudioElement> {
    let mut reference = support::published_mix_presentation()
        .payload
        .sub_mixes
        .first()?
        .elements
        .first()?
        .clone();
    reference.audio_element_id = audio_element_id;
    Some(reference)
}

/// Element 300's clone, with id 301.
fn element_301() -> AudioElement {
    let mut element = support::published_audio_element().payload;
    element.audio_element_id = 301;
    element
}

/// MP 42 retargeted as `mix_presentation_id` referencing only `audio_element_id`.
fn presentation_for(mix_presentation_id: u32, audio_element_id: u32) -> MixPresentation {
    let mut presentation = support::published_mix_presentation().payload;
    presentation.mix_presentation_id = mix_presentation_id;
    for sub_mix in &mut presentation.sub_mixes {
        for element in &mut sub_mix.elements {
            element.audio_element_id = audio_element_id;
        }
    }
    presentation
}

#[test]
fn descriptor_set_profile_findings_follow_the_header_profile() {
    let set = published_descriptor_set();
    assert_eq!(profile_findings(set.validate()), vec![]);

    // Two elements in MP 42 under a Simple header: MP 42 needs Base, and the
    // sequence carries two unique Audio Elements.
    let mut two = published_descriptor_set();
    two.audio_elements.push(element_301());
    two.mix_presentations
        .first_mut()
        .and_then(|presentation| presentation.sub_mixes.first_mut())
        .unwrap()
        .elements
        .push(reference_to(301).unwrap());
    assert_eq!(profile_findings(two.validate()), vec![f1(0), f3(2, 1, 0)]);
    two.sequence_header = IaSequenceHeader::new(1, 1);
    assert_eq!(profile_findings(two.validate()), vec![]);

    // Zero Mix Presentations never comply.
    let mut none = published_descriptor_set();
    none.mix_presentations.clear();
    assert_eq!(profile_findings(none.validate()), vec![f1(0)]);
}

#[test]
fn descriptor_set_reports_sub_mix_and_headphones_non_compliance() {
    let mut two_sub_mixes = published_descriptor_set();
    let presentation = two_sub_mixes.mix_presentations.first_mut().unwrap();
    let extra = presentation.sub_mixes.first().unwrap().clone();
    presentation.sub_mixes.push(extra);
    assert_eq!(profile_findings(two_sub_mixes.validate()), vec![f1(0)]);

    // One complying presentation alongside is enough.
    two_sub_mixes
        .mix_presentations
        .push(presentation_for(43, 300));
    assert_eq!(profile_findings(two_sub_mixes.validate()), vec![]);

    let mut reserved = published_descriptor_set();
    reserved
        .mix_presentations
        .first_mut()
        .and_then(|presentation| presentation.sub_mixes.first_mut())
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .unwrap()
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(2);
    assert_eq!(profile_findings(reserved.validate()), vec![f1(0)]);
}

#[test]
fn descriptor_set_counts_unique_elements_across_presentations() {
    let mut set = published_descriptor_set();
    set.audio_elements.push(element_301());
    set.mix_presentations.push(presentation_for(43, 301));
    assert_eq!(profile_findings(set.validate()), vec![f3(2, 1, 0)]);
}

#[test]
fn descriptor_set_reports_an_expanded_element_under_simple_and_base() {
    let mut set = published_descriptor_set();
    let element = set.audio_elements.first_mut().unwrap();
    let AudioElementType::ChannelBased(config) = &mut element.audio_element_type else {
        panic!("the published element is channel-based");
    };
    config
        .scalable_channel_layout
        .layers
        .first_mut()
        .unwrap()
        .loudspeaker_layout = LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe);
    assert_eq!(profile_findings(set.validate()), vec![f1(0), f3(1, 2, 0)]);

    set.sequence_header = IaSequenceHeader::new(2, 1);
    assert_eq!(profile_findings(set.validate()), vec![f2(1)]);
}

#[test]
fn a_reserved_primary_profile_adds_no_profile_finding() {
    let mut set = published_descriptor_set();
    set.audio_elements.push(element_301());
    set.mix_presentations.clear();
    set.sequence_header = IaSequenceHeader::new(3, 3);
    assert_eq!(profile_findings(set.validate()), vec![]);
}

/// Only the th8 Mix Presentation findings, in their returned order.
fn th8_findings(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| {
            matches!(
                finding.at,
                Location::Field(
                    "num_sub_mixes" | "sub_mix.audio_element_id" | "headphones_rendering_mode"
                )
            )
        })
        .collect()
}

/// The duplicate `audio_element_id` finding.
fn dup(mix_presentation_id: u32, audio_element_id: u32) -> Finding {
    Finding {
        at: Location::Field("sub_mix.audio_element_id"),
        message: format!(
            "mix presentation {mix_presentation_id} references audio_element_id \
             {audio_element_id} more than once; there SHALL be no duplicate audio_element_id \
             within one Mix Presentation (IAMF v1.1.0 index.bs:1280)"
        ),
    }
}

/// The zero `num_sub_mixes` finding.
fn sub0() -> Finding {
    Finding {
        at: Location::Field("num_sub_mixes"),
        message: "num_sub_mixes is 0; it SHALL NOT be 0 (IAMF v1.1.0 index.bs:1278)".to_owned(),
    }
}

/// The `num_sub_mixes` > 1 finding.
fn subn(count: usize) -> Finding {
    Finding {
        at: Location::Field("num_sub_mixes"),
        message: format!(
            "num_sub_mixes is {count}; it SHOULD be 1 and parsers SHOULD ignore a Mix \
             Presentation with more (IAMF v1.1.0 index.bs:1920); libiamf fails its parse"
        ),
    }
}

/// The reserved `headphones_rendering_mode` finding.
fn hp(audio_element_id: u32, raw: u8) -> Finding {
    Finding {
        at: Location::Field("headphones_rendering_mode"),
        message: format!(
            "audio element {audio_element_id} carries reserved headphones_rendering_mode {raw}; \
             parsers SHALL ignore this Mix Presentation (IAMF v1.1.0 index.bs:1341)"
        ),
    }
}

/// MP 42 with its sub-mix cloned once (the test_000124 shape).
fn presentation_with_two_sub_mixes() -> Option<MixPresentation> {
    let mut presentation = support::published_mix_presentation().payload;
    let extra = presentation.sub_mixes.first()?.clone();
    presentation.sub_mixes.push(extra);
    Some(presentation)
}

#[test]
fn published_mix_presentation_has_no_findings() {
    assert_eq!(
        support::published_mix_presentation().payload.validate(),
        vec![]
    );
}

#[test]
fn mix_presentation_validate_reports_zero_sub_mixes() {
    let mut presentation = support::published_mix_presentation().payload;
    presentation.sub_mixes.clear();
    assert_eq!(th8_findings(presentation.validate()), vec![sub0()]);
}

#[test]
fn mix_presentation_validate_reports_sub_mix_count_before_duplicates() {
    let presentation = presentation_with_two_sub_mixes().unwrap();
    assert_eq!(
        th8_findings(presentation.validate()),
        vec![subn(2), dup(42, 300)]
    );
}

#[test]
fn mix_presentation_validate_reports_reserved_headphones_modes() {
    for raw in [2, 3] {
        let mut presentation = support::published_mix_presentation().payload;
        presentation
            .sub_mixes
            .first_mut()
            .and_then(|sub_mix| sub_mix.elements.first_mut())
            .unwrap()
            .rendering_config
            .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(raw);
        assert_eq!(th8_findings(presentation.validate()), vec![hp(300, raw)]);
    }

    let mut binaural = support::published_mix_presentation().payload;
    binaural
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .unwrap()
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Binaural;
    assert_eq!(th8_findings(binaural.validate()), vec![]);
}

#[test]
fn mix_presentation_findings_follow_the_research_order() {
    let mut presentation = presentation_with_two_sub_mixes().unwrap();
    presentation
        .sub_mixes
        .last_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .unwrap()
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(3);
    assert_eq!(
        th8_findings(presentation.validate()),
        vec![subn(2), dup(42, 300), hp(300, 3)]
    );
}

#[test]
fn both_validators_carry_the_sub_mix_and_headphones_findings() {
    let mut set = published_descriptor_set();
    set.mix_presentations = vec![presentation_with_two_sub_mixes().unwrap()];
    assert_eq!(th8_findings(set.validate()), vec![subn(2), dup(42, 300)]);

    let mut header = support::published_sequence_header();
    header.payload = IaSequenceHeader::new(1, 1);
    let mut presentation = support::published_mix_presentation();
    presentation.payload = presentation_with_two_sub_mixes().unwrap();
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::IaSequenceHeader(header),
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::AudioElement(support::published_audio_element()),
            SequenceObu::MixPresentation(presentation),
        ],
    };
    assert_eq!(
        th8_findings(sequence.validate()),
        vec![subn(2), dup(42, 300)]
    );
}

/// MP 42 whose first sub-mix references `ids`, in order.
fn presentation_referencing(ids: &[u32]) -> Option<MixPresentation> {
    let mut presentation = support::published_mix_presentation().payload;
    let sub_mix = presentation.sub_mixes.first_mut()?;
    sub_mix.elements = ids
        .iter()
        .map(|&id| reference_to(id))
        .collect::<Option<Vec<_>>>()?;
    Some(presentation)
}

#[test]
fn mix_presentation_validate_reports_each_later_duplicate_audio_element_id() {
    let mut pushed = support::published_mix_presentation().payload;
    pushed
        .sub_mixes
        .first_mut()
        .unwrap()
        .elements
        .push(reference_to(300).unwrap());
    assert_eq!(th8_findings(pushed.validate()), vec![dup(42, 300)]);

    let repeated = presentation_referencing(&[300, 301, 300, 300]).unwrap();
    assert_eq!(
        th8_findings(repeated.validate()),
        vec![dup(42, 300), dup(42, 300)]
    );

    let distinct = presentation_referencing(&[300, 301]).unwrap();
    assert_eq!(th8_findings(distinct.validate()), vec![]);
}

#[test]
fn mix_presentation_duplicate_scope_spans_sub_mixes() {
    // Presentation scope, as iamf-tools reads IAMF v1.1.0 index.bs:1280.
    let mut presentation = support::published_mix_presentation().payload;
    let extra = presentation.sub_mixes.first().unwrap().clone();
    presentation.sub_mixes.push(extra);
    let duplicates: Vec<Finding> = presentation
        .validate()
        .into_iter()
        .filter(|finding| finding.at == Location::Field("sub_mix.audio_element_id"))
        .collect();
    assert_eq!(duplicates, vec![dup(42, 300)]);
}

#[test]
fn both_validators_carry_the_duplicate_audio_element_finding() {
    let mut set = published_descriptor_set();
    set.mix_presentations
        .first_mut()
        .and_then(|presentation| presentation.sub_mixes.first_mut())
        .unwrap()
        .elements
        .push(reference_to(300).unwrap());
    assert_eq!(th8_findings(set.validate()), vec![dup(42, 300)]);

    let mut presentation = support::published_mix_presentation();
    presentation
        .payload
        .sub_mixes
        .first_mut()
        .unwrap()
        .elements
        .push(reference_to(300).unwrap());
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::IaSequenceHeader(support::published_sequence_header()),
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::AudioElement(support::published_audio_element()),
            SequenceObu::MixPresentation(presentation),
        ],
    };
    assert_eq!(th8_findings(sequence.validate()), vec![dup(42, 300)]);
}

fn published_parsed_sequence() -> Vec<SequenceObu> {
    vec![
        SequenceObu::IaSequenceHeader(support::published_sequence_header()),
        SequenceObu::CodecConfig(support::published_codec_config()),
        SequenceObu::AudioElement(support::published_audio_element()),
        SequenceObu::MixPresentation(support::published_mix_presentation()),
    ]
}

#[test]
fn parsed_sequence_profile_findings_skip_redundant_copies() {
    let sequence = ParsedSequence {
        obus: published_parsed_sequence(),
    };
    assert_eq!(profile_findings(sequence.validate()), vec![]);

    let mut element_copy = support::published_audio_element();
    element_copy.header = element_copy.header.with_redundant_copy(true);
    let mut presentation_copy = support::published_mix_presentation();
    presentation_copy.header = presentation_copy.header.with_redundant_copy(true);
    let mut obus = published_parsed_sequence();
    obus.push(SequenceObu::AudioElement(element_copy));
    obus.push(SequenceObu::MixPresentation(presentation_copy));
    let sequence = ParsedSequence { obus };
    assert_eq!(profile_findings(sequence.validate()), vec![]);
}

#[test]
fn parsed_sequence_reports_two_sub_mixes_under_a_base_header() {
    // The test_000124 shape: one Mix Presentation with two sub-mixes.
    let mut header = support::published_sequence_header();
    header.payload = IaSequenceHeader::new(1, 1);
    let mut presentation = support::published_mix_presentation();
    let extra = presentation.payload.sub_mixes.first().unwrap().clone();
    presentation.payload.sub_mixes.push(extra);
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::IaSequenceHeader(header),
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::AudioElement(support::published_audio_element()),
            SequenceObu::MixPresentation(presentation),
        ],
    };
    assert_eq!(profile_findings(sequence.validate()), vec![f1(1)]);
}

#[test]
fn parsed_sequence_reports_the_sequence_wide_unique_element_limit() {
    // qk3 Q1 = A: the parse-side validator reports the sequence-limit finding.
    let mut obus = published_parsed_sequence();
    obus.push(SequenceObu::AudioElement(Obu::new(
        ObuHeader::new(ObuType::AudioElement),
        element_301(),
    )));
    obus.push(SequenceObu::MixPresentation(Obu::new(
        ObuHeader::new(ObuType::MixPresentation),
        presentation_for(43, 301),
    )));
    let sequence = ParsedSequence { obus };
    assert_eq!(profile_findings(sequence.validate()), vec![f3(2, 1, 0)]);
}

#[test]
fn parsed_sequence_without_a_header_gains_no_profile_finding() {
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::AudioElement(support::published_audio_element()),
        ],
    };
    assert_eq!(profile_findings(sequence.validate()), vec![]);
}

/// Only the wlv findings (languages, anchors, scalable layouts), in their returned order.
fn wlv_findings(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| {
            matches!(
                finding.at,
                Location::Field(
                    "annotations_language"
                        | "anchored_loudness.anchor_element"
                        | "sub_mix.num_layouts"
                )
            )
        })
        .collect()
}

/// The duplicate `annotations_language` finding.
fn lang(mix_presentation_id: u32, language: &str) -> Finding {
    Finding {
        at: Location::Field("annotations_language"),
        message: format!(
            "mix presentation {mix_presentation_id} lists annotations_language {language:?} more \
             than once; the same language SHALL NOT be duplicated (IAMF v1.1.0 index.bs:1273)"
        ),
    }
}

/// The duplicate `anchor_element` finding.
fn anchor(value: u8) -> Finding {
    Finding {
        at: Location::Field("anchored_loudness.anchor_element"),
        message: format!(
            "anchor_element {value} appears more than once in one loudness_info; there SHALL be \
             no duplicate anchor_element within one LoudnessInfo() (IAMF v1.1.0 index.bs:1485)"
        ),
    }
}

/// MP 42 with `languages` (annotation counts matched) and, if given, `anchors` on every layout.
fn presentation_with(languages: &[&[u8]], anchors: Option<&[u8]>) -> MixPresentation {
    let mut presentation = support::published_mix_presentation().payload;
    presentation.annotations_language =
        languages.iter().map(|language| language.to_vec()).collect();
    presentation.localized_presentation_annotations = vec![b"mix".to_vec(); languages.len()];
    for sub_mix in &mut presentation.sub_mixes {
        for element in &mut sub_mix.elements {
            element.localized_element_annotations = vec![b"element".to_vec(); languages.len()];
        }
        if let Some(anchors) = anchors {
            for layout in &mut sub_mix.layouts {
                layout.loudness.anchored = Some(AnchoredLoudness {
                    anchor_elements: anchors
                        .iter()
                        .map(|&anchor_element| AnchorElement {
                            anchor_element,
                            anchored_loudness: 0,
                        })
                        .collect(),
                });
            }
        }
    }
    presentation
}

#[test]
fn mix_presentation_validate_reports_each_later_case_insensitive_duplicate_language() {
    let presentation = presentation_with(&[b"en-us", b"EN-US", b"en-us"], None);
    assert_eq!(
        wlv_findings(presentation.validate()),
        vec![lang(42, "EN-US"), lang(42, "en-us")]
    );
    let distinct = presentation_with(&[b"en-us", b"es-mx"], None);
    assert_eq!(wlv_findings(distinct.validate()), vec![]);
}

#[test]
fn mix_presentation_validate_reports_duplicate_anchor_elements_per_loudness_info() {
    let repeated = presentation_with(&[b"en-us"], Some(&[1, 2, 1]));
    assert_eq!(wlv_findings(repeated.validate()), vec![anchor(1)]);
    let unknown = presentation_with(&[b"en-us"], Some(&[0, 0]));
    assert_eq!(wlv_findings(unknown.validate()), vec![anchor(0)]);
    for distinct in [[1, 2], [3, 4]] {
        let presentation = presentation_with(&[b"en-us"], Some(&distinct));
        assert_eq!(wlv_findings(presentation.validate()), vec![]);
    }
}

#[test]
fn mix_presentation_language_findings_precede_anchor_findings() {
    let presentation = presentation_with(&[b"en-us", b"EN-US"], Some(&[1, 1]));
    assert_eq!(
        wlv_findings(presentation.validate()),
        vec![lang(42, "EN-US"), anchor(1)]
    );
}

#[test]
fn both_validators_carry_the_language_and_anchor_findings() {
    let expected = vec![lang(42, "EN-US"), anchor(1)];

    let mut set = published_descriptor_set();
    set.mix_presentations = vec![presentation_with(&[b"en-us", b"EN-US"], Some(&[1, 1]))];
    assert_eq!(wlv_findings(set.validate()), expected);

    let mut presentation = support::published_mix_presentation();
    presentation.payload = presentation_with(&[b"en-us", b"EN-US"], Some(&[1, 1]));
    let sequence = ParsedSequence {
        obus: vec![
            SequenceObu::IaSequenceHeader(support::published_sequence_header()),
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::AudioElement(support::published_audio_element()),
            SequenceObu::MixPresentation(presentation),
        ],
    };
    assert_eq!(wlv_findings(sequence.validate()), expected);
}

/// The single-scalable-channel-element `num_layouts` finding.
fn scalable(
    mix_presentation_id: u32,
    audio_element_id: u32,
    num_layers: usize,
    num_layouts: usize,
) -> Finding {
    Finding {
        at: Location::Field("sub_mix.num_layouts"),
        message: format!(
            "mix presentation {mix_presentation_id} has a sub-mix whose only audio element \
             {audio_element_id} is scalable with num_layers {num_layers} but num_layouts is \
             {num_layouts}; num_layouts SHALL be >= num_layers unless the highest loudness_layout \
             is the layout the sub-mix was authored on (IAMF v1.1.0 index.bs:1305-1309)"
        ),
    }
}

/// Clones a channel-based element's first layer, making it scalable.
fn with_second_layer(element: &mut AudioElement) {
    if let AudioElementType::ChannelBased(config) = &mut element.audio_element_type {
        let layers = &mut config.scalable_channel_layout.layers;
        if let Some(layer) = layers.first().cloned() {
            layers.push(layer);
        }
    }
}

/// The published sequence with a two-layer element 300 and `presentation` as its Mix Presentation.
fn scalable_sequence(presentation: MixPresentation) -> ParsedSequence {
    let mut element = support::published_audio_element();
    with_second_layer(&mut element.payload);
    let mut mix = support::published_mix_presentation();
    mix.payload = presentation;
    ParsedSequence {
        obus: vec![
            SequenceObu::IaSequenceHeader(support::published_sequence_header()),
            SequenceObu::CodecConfig(support::published_codec_config()),
            SequenceObu::AudioElement(element),
            SequenceObu::MixPresentation(mix),
        ],
    }
}

/// The published descriptor set with a two-layer element 300 and `presentation`.
fn scalable_set(presentation: MixPresentation) -> DescriptorSet {
    let mut set = published_descriptor_set();
    for element in &mut set.audio_elements {
        with_second_layer(element);
    }
    set.mix_presentations = vec![presentation];
    set
}

#[test]
fn both_validators_report_num_layouts_below_num_layers_for_one_scalable_element() {
    let presentation = support::published_mix_presentation().payload;
    let expected = vec![scalable(42, 300, 2, 1)];
    assert_eq!(
        wlv_findings(scalable_set(presentation.clone()).validate()),
        expected
    );
    assert_eq!(
        wlv_findings(scalable_sequence(presentation).validate()),
        expected
    );

    assert_eq!(wlv_findings(published_descriptor_set().validate()), vec![]);
    let published = ParsedSequence {
        obus: published_parsed_sequence(),
    };
    assert_eq!(wlv_findings(published.validate()), vec![]);
}

#[test]
fn num_layouts_equal_to_num_layers_has_no_scalable_finding() {
    let mut presentation = support::published_mix_presentation().payload;
    for sub_mix in &mut presentation.sub_mixes {
        let extra = sub_mix.layouts.first().cloned().unwrap();
        sub_mix.layouts.push(extra);
    }
    assert_eq!(
        wlv_findings(scalable_set(presentation.clone()).validate()),
        vec![]
    );
    assert_eq!(
        wlv_findings(scalable_sequence(presentation).validate()),
        vec![]
    );
}

#[test]
fn a_sub_mix_with_two_elements_has_no_scalable_finding() {
    let mut presentation = support::published_mix_presentation().payload;
    for sub_mix in &mut presentation.sub_mixes {
        let extra = sub_mix.elements.first().cloned().unwrap();
        sub_mix.elements.push(extra);
    }
    assert_eq!(
        wlv_findings(scalable_set(presentation.clone()).validate()),
        vec![]
    );
    assert_eq!(
        wlv_findings(scalable_sequence(presentation).validate()),
        vec![]
    );
}

#[test]
fn an_unresolved_scalable_element_reference_has_no_scalable_finding() {
    let mut presentation = support::published_mix_presentation().payload;
    presentation
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .unwrap()
        .audio_element_id = 999;
    let unresolved = |findings: &[Finding]| {
        findings
            .iter()
            .any(|finding| finding.at == Location::Field("audio_element_id"))
    };

    let set_findings = scalable_set(presentation.clone()).validate();
    assert!(unresolved(&set_findings));
    assert_eq!(wlv_findings(set_findings), vec![]);

    let sequence_findings = scalable_sequence(presentation).validate();
    assert!(unresolved(&sequence_findings));
    assert_eq!(wlv_findings(sequence_findings), vec![]);
}

fn published_set_with_extension_params(count: usize) -> DescriptorSet {
    let mut set = published_descriptor_set();
    if let Some(element) = set.audio_elements.first_mut() {
        element.params = core::iter::repeat_with(|| AudioElementParam::Extension {
            param_definition_type: 3,
            bytes: Vec::new(),
        })
        .take(count)
        .collect();
    }
    set
}

fn num_parameters_findings(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| finding.at == Location::Field("num_parameters"))
        .collect()
}

fn audio_element_payloads(parsed: &ParsedSequence) -> Vec<&AudioElement> {
    parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioElement(obu) => Some(&obu.payload),
            _ => None,
        })
        .collect()
}

// User decision 2026-09-14 (quick 260914-kfs): read and write stay unrestricted,
// validate() reports the iamf-tools@v2.1.0 kMaxNumParameters refusal.
// Hand decode: num_parameters 257 is uleb128 `81 02`; each param is `03 00`
// (param_definition_type 3, param_definition_size 0).
#[test]
fn audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit() {
    let expected = Finding {
        at: Location::Field("num_parameters"),
        message: "audio element 300 has 257 parameters; iamf-tools@v2.1.0 refuses an Audio \
                  Element with more than 256 (kMaxNumParameters) on read and write, although \
                  IAMF v1.1.0 requires parsers to support any num_parameters"
            .to_owned(),
    };
    let set = published_set_with_extension_params(257);

    let element = set.audio_elements.first().unwrap();
    assert_eq!(
        num_parameters_findings(element.validate()),
        vec![expected.clone()]
    );

    let set_findings = set.validate();
    assert_eq!(
        set_findings.len(),
        published_descriptor_set().validate().len() + 1
    );
    assert_eq!(
        num_parameters_findings(set_findings),
        vec![expected.clone()]
    );

    let bytes = support::descriptor_bytes(&set);
    let parsed = parse_sequence(&bytes).expect("any num_parameters parses (decision 1)");
    let elements = audio_element_payloads(&parsed);
    assert_eq!(elements.len(), 1);
    assert_eq!(elements.first().unwrap().params.len(), 257);
    assert_eq!(write_parsed_sequence(Vec::new(), &parsed), Ok(bytes));

    assert_eq!(num_parameters_findings(parsed.validate()), vec![expected]);
}

// User decision 2026-09-14 (quick 260914-kfs): the boundary is iamf-tools'
// `num_parameters > kMaxNumParameters`, so exactly 256 carries no finding.
#[test]
fn audio_element_with_256_params_has_no_num_parameters_finding() {
    let set = published_set_with_extension_params(256);
    assert_eq!(set.validate(), published_descriptor_set().validate());

    let bytes = support::descriptor_bytes(&set);
    let parsed = parse_sequence(&bytes).expect("256 params parse");
    let elements = audio_element_payloads(&parsed);
    assert_eq!(elements.len(), 1);
    assert_eq!(elements.first().unwrap().params.len(), 256);
    assert_eq!(num_parameters_findings(parsed.validate()), vec![]);
}
