#![allow(dead_code)]

use iamf::bits::BitWriter;
use iamf::model::DescriptorSet;
use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, AudioElementParam, AudioFrame, BlockDurationFields, ChannelAudioLayerConfig,
    CodecConfig, DemixingInfoParameterData, IaSequenceHeader, Layout, LayoutWithLoudness, Loudness,
    LoudnessExtension, LpcmDecoderConfig, MixGainParamDefinition, MixGainParameterData,
    MixPresentation, Obu, ObuHeader, ObuType, OutputGain, ParamDefinition, ParameterBlock,
    ParameterData, ParameterSubblock, ReconGainElement, ReconGainInfoParameterData,
    RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
    TemporalDelimiter, write_param_definition,
};
use iamf::sequence::{ParsedSequence, SequenceObu, TemporalUnit, UnknownObu};
use proptest::prelude::*;

#[derive(Debug, Clone)]
pub struct CanonicalCase {
    pub descriptors: DescriptorSet,
    pub units: Vec<TemporalUnit>,
}

pub fn canonical_case_strategy() -> impl Strategy<Value = CanonicalCase> {
    (
        1_usize..=4,
        any::<bool>(),
        proptest::collection::vec(any::<u8>(), 0..=64),
        any::<i16>(),
    )
        .prop_map(|(unit_count, delimiters, payload, gain)| {
            rich_case(unit_count, delimiters, payload, gain)
        })
}

pub fn canonical_parsed_strategy() -> impl Strategy<Value = ParsedSequence> {
    (
        0_u8..=5,
        canonical_case_strategy(),
        proptest::collection::vec(any::<u8>(), 0..=64),
        0_usize..=32,
    )
        .prop_map(|(shape, case, unknown_payload, position)| match shape {
            0 => ParsedSequence::default(),
            1 => ParsedSequence::from_parts(&minimal_descriptors(), &[]).unwrap_or_default(),
            _ => {
                let mut parsed =
                    ParsedSequence::from_parts(&case.descriptors, &case.units).unwrap_or_default();
                if shape >= 3 {
                    insert_redundant_descriptor(&mut parsed);
                }
                if shape >= 4 {
                    let header = ObuHeader::new(ObuType::Reserved(27))
                        .with_redundant_copy(true)
                        .with_extension(vec![0x91, 0x00, 0xfe]);
                    let at = position % parsed.obus.len().saturating_add(1);
                    parsed.obus.insert(
                        at,
                        SequenceObu::Unknown(UnknownObu {
                            header,
                            payload: unknown_payload,
                        }),
                    );
                }
                parsed
            }
        })
}

pub fn minimal_descriptors() -> DescriptorSet {
    DescriptorSet::new(IaSequenceHeader::new(0, 0))
}

pub fn rich_case(
    unit_count: usize,
    delimiters: bool,
    frame_payload: Vec<u8>,
    gain: i16,
) -> CanonicalCase {
    let mut layer = ChannelAudioLayerConfig::new(LoudspeakerLayout::Stereo, 2, 0);
    layer.output_gain = Some(OutputGain {
        flags: 0x2a,
        reserved: 0x03,
        gain: -257,
    });
    layer.recon_gain_is_present = true;
    layer.reserved = 0x03;
    let mut element = AudioElement::channel_based(
        20,
        7,
        vec![0, 18],
        ScalableChannelLayoutConfig {
            reserved: 0x1d,
            layers: vec![layer],
        },
    );
    element.reserved = 0x1b;
    element.trailing = vec![0xa1, 0xb2];

    let mut demixing_definition = ParamDefinition::mode_1(10, 48_000);
    demixing_definition.reserved = 0x55;
    let mut recon_definition = ParamDefinition::mode_1(11, 48_000);
    recon_definition.reserved = 0x33;
    let mut raw_definition = ParamDefinition::mode_1(12, 48_000);
    raw_definition.reserved = 0x45;
    element.params = vec![
        AudioElementParam::Demixing {
            definition: demixing_definition,
            default_dmixp_mode: 5,
            default_reserved: 0x1b,
            default_w: 10,
            default_w_reserved: 0x0d,
        },
        AudioElementParam::ReconGain {
            definition: recon_definition,
        },
        AudioElementParam::Extension {
            param_definition_type: 7,
            bytes: definition_bytes(&raw_definition),
        },
    ];

    let mut codec = CodecConfig::lpcm(
        7,
        8,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::Reserved(0x7f),
            sample_size: 16,
            sample_rate: 48_000,
        },
    );
    codec.trailing = vec![0xc3, 0xd4];

    let mut element_gain = MixGainParamDefinition::mode_1(13, 48_000);
    element_gain.definition.reserved = 0x2d;
    element_gain.default_mix_gain = -32;
    let mut output_gain = MixGainParamDefinition::mode_1(14, 48_000);
    output_gain.definition.reserved = 0x35;
    output_gain.default_mix_gain = 64;
    let presentation = MixPresentation {
        mix_presentation_id: 30,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![b"round-trip".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 20,
                localized_element_annotations: vec![b"element".to_vec()],
                rendering_config: RenderingConfig {
                    headphones_rendering_mode: iamf::obu::HeadphonesRenderingMode::Reserved(3),
                    reserved: 0x15,
                    extension: vec![0xe1, 0xe2],
                },
                element_mix_gain: element_gain,
            }],
            output_mix_gain: output_gain,
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0x03,
                loudness: Loudness {
                    integrated: -6144,
                    digital_peak: -1536,
                    true_peak: Some(-1280),
                    anchored: None,
                    extension: Some(LoudnessExtension {
                        info_type_bits: 0x84,
                        bytes: vec![0xfa, 0xce],
                    }),
                },
            }],
        }],
        trailing: vec![0x71, 0x72],
    };

    let descriptors = DescriptorSet {
        sequence_header: IaSequenceHeader::new(0, 0),
        codec_configs: vec![codec],
        audio_elements: vec![element],
        mix_presentations: vec![presentation],
    };
    let units = (0..unit_count)
        .map(|index| rich_unit(index, delimiters, &frame_payload, gain))
        .collect();
    CanonicalCase { descriptors, units }
}

fn definition_bytes(definition: &ParamDefinition) -> Vec<u8> {
    let mut writer = BitWriter::new();
    assert!(write_param_definition(&mut writer, definition).is_ok());
    writer.finish().unwrap_or_default()
}

fn rich_unit(index: usize, delimiters: bool, frame_payload: &[u8], gain: i16) -> TemporalUnit {
    let mut recon_gains = [0_u8; 12];
    recon_gains[0] = 1;
    recon_gains[11] = 0xfe;
    let raw_a = frame_payload
        .get(..frame_payload.len().min(32))
        .unwrap_or_default();
    let raw_b = frame_payload
        .get(frame_payload.len().min(32)..)
        .unwrap_or_default();
    let mut blocks = vec![
        block(
            10,
            ParameterData::Demixing(DemixingInfoParameterData {
                dmixp_mode: 7,
                reserved: 0x1f,
            }),
        ),
        block(
            11,
            ParameterData::ReconGain(ReconGainInfoParameterData {
                layers: vec![Some(ReconGainElement {
                    recon_gain_flag: 0x1801,
                    recon_gain: recon_gains,
                })],
            }),
        ),
        ParameterBlock {
            parameter_id: 12,
            duration_fields: Some(BlockDurationFields {
                duration: 2,
                constant_subblock_duration: 1,
            }),
            subblocks: vec![raw_subblock(raw_a), raw_subblock(raw_b)],
        },
        ParameterBlock {
            parameter_id: 13,
            duration_fields: Some(BlockDurationFields {
                duration: 3,
                constant_subblock_duration: 1,
            }),
            subblocks: vec![
                ParameterSubblock {
                    subblock_duration: None,
                    data: ParameterData::MixGain(MixGainParameterData::Step {
                        start_point_value: gain,
                    }),
                },
                ParameterSubblock {
                    subblock_duration: None,
                    data: ParameterData::MixGain(MixGainParameterData::Linear {
                        start_point_value: gain,
                        end_point_value: gain.saturating_add(1),
                    }),
                },
                ParameterSubblock {
                    subblock_duration: None,
                    data: ParameterData::MixGain(MixGainParameterData::Bezier {
                        start_point_value: gain,
                        end_point_value: gain.saturating_add(1),
                        control_point_value: gain.saturating_sub(1),
                        control_point_relative_time: 0x80,
                    }),
                },
            ],
        },
        block(
            14,
            ParameterData::MixGain(MixGainParameterData::Step {
                start_point_value: gain,
            }),
        ),
    ]
    .into_iter()
    .map(|payload| Obu::new(ObuHeader::new(ObuType::ParameterBlock), payload))
    .collect::<Vec<_>>();
    if index == 0 {
        blocks[0].trailing = vec![0xde, 0xad];
    }
    TemporalUnit {
        temporal_delimiter: delimiters.then_some(TemporalDelimiter),
        parameter_blocks: blocks,
        audio_frames: vec![
            AudioFrame::new(0, frame_payload.to_vec()).into_obu(None),
            AudioFrame::new(18, frame_payload.iter().rev().copied().collect()).into_obu(None),
        ],
    }
}

fn block(parameter_id: u32, data: ParameterData) -> ParameterBlock {
    ParameterBlock {
        parameter_id,
        duration_fields: Some(BlockDurationFields {
            duration: 1,
            constant_subblock_duration: 1,
        }),
        subblocks: vec![ParameterSubblock {
            subblock_duration: None,
            data,
        }],
    }
}

fn raw_subblock(bytes: &[u8]) -> ParameterSubblock {
    ParameterSubblock {
        subblock_duration: None,
        data: ParameterData::Raw(bytes.to_vec()),
    }
}

fn insert_redundant_descriptor(sequence: &mut ParsedSequence) {
    if let Some((index, SequenceObu::AudioElement(obu))) = sequence
        .obus
        .iter()
        .enumerate()
        .find(|(_, obu)| matches!(obu, SequenceObu::AudioElement(_)))
    {
        let mut redundant = obu.clone();
        redundant.header = ObuHeader::new(ObuType::AudioElement).with_redundant_copy(true);
        sequence.obus.insert(
            index.saturating_add(1),
            SequenceObu::AudioElement(redundant),
        );
    }
}
