//! Phase 2 sequence-parser context tests.

#[path = "support/test_000003.rs"]
mod support;

use iamf::bits::BitWriter;
use iamf::error::{ErrorKind, Location};
use iamf::model::DescriptorSet;
use iamf::model::layout::LoudspeakerLayout;
use iamf::obu::{
    AudioElementParam, AudioElementType, AudioFrame, BlockDurationFields, ChannelAudioLayerConfig,
    MixGainParameterData, Obu, ObuHeader, ObuType, ParamDefinition, ParamDefinitionRegistry,
    ParameterBlock, ParameterData, ParameterDataContext, ParameterSubblock, TemporalDelimiter,
    write_obu,
};
use iamf::sequence::{
    ParsedSequence, SequenceObu, UnknownObu, parse_sequence, write_parsed_sequence,
};
use support::published_descriptor_set;

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
fn an_extension_definition_registers_its_shared_prefix_and_reserved_context() {
    let mut descriptors = published_descriptor_set();
    // ParamDefinition { id: 7, rate: 1, mode: 1 }, followed by extension data.
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![AudioElementParam::Extension {
        param_definition_type: 9,
        bytes: vec![0x07, 0x01, 0x80, 0xaa],
    }];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the shared ParamDefinition prefix is present");
    let registered = registry.get(7).expect("extension parameter id 7");

    assert_eq!(registered.definition, ParamDefinition::mode_1(7, 1));
    assert_eq!(registered.context, ParameterDataContext::Reserved(9));
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
    let bytes = write_parsed_sequence(Vec::new(), sequence).expect("flat sequence writes");
    parse_sequence(&bytes).expect("written flat sequence parses")
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
    assert!(matches!(parsed.obus[0], SequenceObu::IaSequenceHeader(_)));
    assert!(matches!(parsed.obus[1], SequenceObu::CodecConfig(_)));
    assert!(matches!(parsed.obus[2], SequenceObu::AudioElement(_)));
    assert!(matches!(parsed.obus[3], SequenceObu::MixPresentation(_)));
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
fn a_parameter_block_without_a_governing_definition_is_a_located_error() {
    let sequence = ParsedSequence {
        obus: vec![SequenceObu::ParameterBlock(mode_1_block(100))],
    };
    let bytes = write_parsed_sequence(Vec::new(), &sequence)
        .expect_err("writer also requires explicit governing context");
    assert_eq!(bytes.kind(), &ErrorKind::NoGoverningParamDefinition);

    // Header type 3, payload size 1, parameter_id 100. The parser cannot know
    // the remainder's syntax without a definition.
    let err = parse_sequence(&[0x18, 0x01, 0x64]).expect_err("definition is absent");
    assert_eq!(err.kind(), &ErrorKind::NoGoverningParamDefinition);
    assert_eq!(err.at(), Location::InputOffset(2));
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
fn flat_validation_reports_local_then_duplicate_then_reference_findings_in_wire_order() {
    let mut invalid_header = support::published_sequence_header();
    invalid_header.payload.primary_profile = 9;
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

    assert!(messages[0].contains("primary_profile"), "{messages:?}");
    assert!(messages[1].contains("codec_config_id 7"), "{messages:?}");
    assert!(messages[2].contains("codec_config_id 999"), "{messages:?}");
    assert!(
        messages[3].contains("audio_substream_id 77"),
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
    let SequenceObu::Unknown(unknown) = &parsed.obus[0] else {
        panic!("reserved type dispatches as Unknown")
    };
    assert_eq!(unknown.header, header);
    assert_eq!(unknown.payload, vec![0xaa, 0xbb, 0xcc]);
    assert_eq!(write_parsed_sequence(Vec::new(), &parsed), Ok(bytes));
}
