//! Public encoder contracts shaped like a filtered Parallax delivery.

#[path = "support/parallax_contract.rs"]
mod contract;

use iamf::encoder::{EncoderBuilder, FrameInput, TemporalUnitInput};
use iamf::model::layout::{AmbisonicsMonoConfig, LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, ChannelAudioLayerConfig, CodecConfig, Layout, LayoutWithLoudness, Loudness,
    LpcmDecoderConfig, MixGainParamDefinition, MixPresentation, RenderingConfig, SampleFormatFlags,
    ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
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
        first.bytes, second.bytes,
        "equivalent declarations are deterministic"
    );
    assert_eq!(
        first.retained_names,
        vec![
            b"program stereo".to_vec(),
            b"flac archive".to_vec(),
            b"opus stream".to_vec(),
            b"ambisonics bed".to_vec(),
        ]
    );

    for excluded in &first.excluded_names {
        assert!(
            !first
                .bytes
                .windows(excluded.len())
                .any(|window| window == excluded),
            "filtered candidate {:?} must never reach encoder input or bytes",
            String::from_utf8_lossy(excluded)
        );
    }

    let parsed = parse_sequence(&first.bytes).expect("delivery bytes parse");
    let codec_ids = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::CodecConfig(config) => Some(config.payload.codec_config_id),
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
    assert_eq!(
        codec_ids,
        vec![0, 1, 2],
        "LPCM, FLAC, Opus declaration order"
    );
    assert_eq!(
        element_ids,
        vec![0, 1, 2, 3],
        "channel then scene descriptor order"
    );
    assert_eq!(
        presentation_ids,
        vec![0, 1],
        "ordered primary and alternate presentations"
    );

    let presentations = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::MixPresentation(presentation) => Some(presentation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        presentations[0].payload.sub_mixes[0].elements[0].audio_element_id,
        0
    );
    assert_eq!(
        presentations[1].payload.sub_mixes[0].elements[0].audio_element_id,
        0
    );

    let parameter_blocks = parsed
        .obus
        .iter()
        .filter(|obu| matches!(obu, SequenceObu::ParameterBlock(_)))
        .count();
    assert_eq!(
        parameter_blocks, 1,
        "pre-decimated supplied parameter block persists"
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
