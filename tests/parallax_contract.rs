//! Public encoder contracts shaped like a filtered Parallax delivery.

#[path = "support/parallax_contract.rs"]
mod contract;

use iamf::encoder::{EncoderBuilder, FrameInput, TemporalUnitInput};
use iamf::model::layout::{AmbisonicsMonoConfig, LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, BlockDurationFields, CODEC_ID_AAC, CODEC_ID_FLAC, CODEC_ID_LPCM, CODEC_ID_OPUS,
    ChannelAudioLayerConfig, CodecConfig, Layout, LayoutWithLoudness, Loudness, LpcmDecoderConfig,
    MixGainParamDefinition, MixGainParameterData, MixPresentation, ParameterBlock, ParameterData,
    ParameterSubblock, RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix,
    SubMixAudioElement,
};
use iamf::sequence::{SequenceObu, parse_sequence};

#[test]
fn one_shared_element_is_referenced_by_two_presentations_but_emitted_once() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let shared = builder.add_audio_element(codec, stereo_element());
    let first = builder.add_mix_presentation(vec![shared], presentation(b"first", 10, 11));
    let second = builder.add_mix_presentation(vec![shared], presentation(b"second", 20, 21));

    let (encoder, manifest) = builder.build().expect("shared declaration is valid");
    assert_eq!(manifest.mix_presentation_id(first), Some(0));
    assert_eq!(manifest.mix_presentation_id(second), Some(1));
    let substreams = manifest
        .substreams(shared)
        .expect("shared element is present in the manifest");
    assert_eq!(substreams.len(), 1);
    let shared_substream = substreams
        .first()
        .copied()
        .expect("shared element has its declared substream");

    let mut writer = encoder
        .start(Vec::new())
        .expect("descriptor prologue writes");
    writer
        .push_temporal_unit(TemporalUnitInput {
            frames: vec![(shared_substream, FrameInput::Lpcm(vec![0, 0, 0, 0]))],
            parameter_blocks: Vec::new(),
            trimming: None,
        })
        .expect("one shared substream has one frame");
    let bytes = writer.finish().expect("sequence finishes");
    let parsed = parse_sequence(&bytes).expect("encoded sequence parses");

    let descriptors = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioElement(element) => Some(element.payload.audio_element_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(descriptors, vec![0]);

    let presentations = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::MixPresentation(presentation) => {
                let sub_mix = presentation.payload.sub_mixes.first()?;
                let element = sub_mix.elements.first()?;
                Some((
                    presentation.payload.mix_presentation_id,
                    presentation
                        .payload
                        .localized_presentation_annotations
                        .clone(),
                    element.audio_element_id,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        presentations,
        vec![
            (0, vec![b"first".to_vec()], 0),
            (1, vec![b"second".to_vec()], 0),
        ]
    );

    let frames = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioFrame(frame) => {
                Some((frame.payload.substream_id, frame.payload.payload.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(frames, vec![(0, vec![0, 0, 0, 0])]);
}

#[test]
fn mono_ambisonics_is_constructible_only_through_the_high_level_builder() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let substreams = (0..4).map(|_| builder.add_substream()).collect();
    let scene = builder.add_ambisonics_mono(
        codec,
        substreams,
        AmbisonicsMonoConfig {
            output_channel_count: 4,
            substream_count: 4,
            channel_mapping: vec![0, 1, 2, 3],
        },
    );
    builder.add_mix_presentation(vec![scene], presentation(b"hoa", 10, 11));

    let (encoder, _) = builder.build().expect("high-level mono scene is valid");
    let writer = encoder
        .start(Vec::new())
        .expect("descriptor prologue writes");
    let parsed = parse_sequence(&writer.finish().expect("descriptor sequence finishes"))
        .expect("descriptor sequence parses");

    let scenes = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioElement(element) => Some(&element.payload.audio_element_type),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(scenes.len(), 1);
    assert!(scenes.first().is_some_and(|scene| matches!(
        scene.scene_based_config(),
        Some(iamf::model::layout::AmbisonicsConfig::Mono(_))
    )));
}

#[test]
#[allow(clippy::indexing_slicing)]
fn filtered_delivery_is_deterministic_and_retains_only_public_contract_data() {
    let first = contract::build_delivery().expect("filtered public delivery builds");
    let second = contract::build_delivery().expect("equivalent filtered delivery builds");
    assert_eq!(
        first.deliveries, second.deliveries,
        "equivalent declarations are deterministic"
    );
    assert_eq!(
        first
            .deliveries
            .iter()
            .map(|delivery| delivery.name.clone())
            .collect::<Vec<_>>(),
        vec![
            b"Parallax primary".to_vec(),
            b"Parallax archive".to_vec(),
            b"Parallax stream".to_vec(),
            b"Parallax AAC-LC".to_vec(),
        ],
        "each incompatible codec family is emitted as its own IA Sequence"
    );
    assert_eq!(
        first.retained_names,
        vec![
            b"program stereo".to_vec(),
            b"flac archive".to_vec(),
            b"opus stream".to_vec(),
            b"aac-lc stream".to_vec(),
            b"ambisonics bed".to_vec(),
        ]
    );

    for excluded in &first.excluded_names {
        assert!(
            !first
                .deliveries
                .iter()
                .flat_map(|delivery| delivery.bytes.windows(excluded.len()))
                .any(|window| window == excluded),
            "filtered candidate {:?} must never reach encoder input or bytes",
            String::from_utf8_lossy(excluded)
        );
    }

    let declarations = first
        .deliveries
        .iter()
        .map(|delivery| {
            let parsed = parse_sequence(&delivery.bytes).expect("delivery bytes parse");
            let frame_sizes = parsed
                .obus
                .iter()
                .filter_map(|obu| match obu {
                    SequenceObu::CodecConfig(config) => Some(config.payload.num_samples_per_frame),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let codec_ids = parsed
                .obus
                .iter()
                .filter_map(|obu| match obu {
                    SequenceObu::CodecConfig(config) => Some(config.payload.codec_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let element_ids = parsed
                .obus
                .iter()
                .filter_map(|obu| match obu {
                    SequenceObu::AudioElement(element) => Some(element.payload.audio_element_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let presentation_ids = parsed
                .obus
                .iter()
                .filter_map(|obu| match obu {
                    SequenceObu::MixPresentation(presentation) => {
                        Some(presentation.payload.mix_presentation_id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let presentation_elements = parsed
                .obus
                .iter()
                .filter_map(|obu| match obu {
                    SequenceObu::MixPresentation(presentation) => Some(
                        presentation.payload.sub_mixes[0]
                            .elements
                            .iter()
                            .map(|element| element.audio_element_id)
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (
                codec_ids,
                frame_sizes,
                element_ids,
                presentation_ids,
                presentation_elements,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        declarations,
        vec![
            (
                vec![CODEC_ID_LPCM],
                vec![128],
                vec![0, 1],
                vec![0],
                vec![vec![0, 1]]
            ),
            (
                vec![CODEC_ID_FLAC],
                vec![128],
                vec![0],
                vec![0],
                vec![vec![0]]
            ),
            (
                vec![CODEC_ID_OPUS],
                vec![960],
                vec![0],
                vec![0],
                vec![vec![0]]
            ),
            (
                vec![CODEC_ID_AAC],
                vec![1024],
                vec![0],
                vec![0],
                vec![vec![0]]
            ),
        ],
        "each output carries one codec config and one compatible presentation"
    );

    let payloads = first
        .deliveries
        .iter()
        .map(|delivery| {
            parse_sequence(&delivery.bytes)
                .expect("delivery bytes parse")
                .obus
                .into_iter()
                .filter_map(|obu| match obu {
                    SequenceObu::AudioFrame(frame) => Some(frame.payload.payload),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        payloads[1],
        vec![include_bytes!("fixtures/codecs/flac/packet-000.bin").to_vec()],
        "archive retains the committed opaque FLAC packet"
    );
    assert_eq!(
        payloads[2],
        vec![include_bytes!("fixtures/codecs/opus/packet-000.bin").to_vec()],
        "stream retains the committed opaque Opus packet"
    );
    let pinned_aac_payload =
        parse_sequence(include_bytes!("fixtures/reference/test_000076_aac_lc.iamf"))
            .expect("the pinned AAC-LC source vector parses")
            .obus
            .into_iter()
            .find_map(|obu| match obu {
                SequenceObu::AudioFrame(frame) => Some(frame.payload.payload),
                _ => None,
            })
            .expect("the pinned AAC-LC source vector has an access unit");
    assert_eq!(
        payloads[3],
        vec![pinned_aac_payload],
        "AAC-LC retains the pinned external access unit"
    );
    let primary_frames = parse_sequence(&first.deliveries[0].bytes)
        .expect("primary delivery bytes parse")
        .obus
        .into_iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioFrame(frame) => {
                Some((frame.payload.substream_id, frame.payload.payload))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        primary_frames,
        vec![
            (0, vec![0; 512]),
            (1, vec![0; 256]),
            (2, vec![0; 256]),
            (3, vec![0; 256]),
            (4, vec![0; 256]),
        ],
        "primary retains its stereo frame followed by all ambisonics substreams"
    );

    let parameter_blocks = first
        .deliveries
        .iter()
        .map(|delivery| {
            parse_sequence(&delivery.bytes)
                .expect("delivery bytes parse")
                .obus
                .into_iter()
                .filter_map(|obu| match obu {
                    SequenceObu::ParameterBlock(block) => Some(block.payload),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        parameter_blocks,
        vec![
            vec![ParameterBlock {
                parameter_id: 0,
                duration_fields: Some(BlockDurationFields {
                    duration: 128,
                    constant_subblock_duration: 0,
                }),
                subblocks: vec![
                    ParameterSubblock {
                        subblock_duration: Some(64),
                        data: ParameterData::MixGain(MixGainParameterData::Linear {
                            start_point_value: -256,
                            end_point_value: 128,
                        }),
                    },
                    ParameterSubblock {
                        subblock_duration: Some(64),
                        data: ParameterData::MixGain(MixGainParameterData::Step {
                            start_point_value: 128,
                        }),
                    },
                ],
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ],
        "only primary carries its pre-decimated, governed gain block"
    );
}

fn lpcm_config() -> CodecConfig {
    CodecConfig::lpcm(
        42,
        1,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::LittleEndian,
            sample_size: 16,
            sample_rate: 16_000,
        },
    )
}

fn stereo_element() -> AudioElement {
    AudioElement::channel_based(
        99,
        42,
        vec![9],
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            LoudspeakerLayout::Stereo,
            1,
            1,
        )),
    )
}

fn presentation(
    annotation: &[u8],
    element_parameter_id: u32,
    output_parameter_id: u32,
) -> MixPresentation {
    MixPresentation {
        mix_presentation_id: 7,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![annotation.to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 99,
                localized_element_annotations: vec![b"shared bed".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: MixGainParamDefinition::mode_1(element_parameter_id, 16_000),
            }],
            output_mix_gain: MixGainParamDefinition::mode_1(output_parameter_id, 16_000),
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(0, 0),
            }],
        }],
        trailing: Vec::new(),
    }
}
