//! Public-boundary tests for the immutable high-level encoder configuration.

use iamf::encoder::EncoderBuilder;
use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, AudioElementParam, ChannelAudioLayerConfig, CodecConfig, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixPresentation,
    RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
};
use iamf::{ErrorKind, Location};

#[test]
fn build_returns_opaque_handles_and_a_complete_deterministic_manifest() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(CodecConfig::lpcm(
        42,
        128,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::LittleEndian,
            sample_size: 16,
            sample_rate: 16_000,
        },
    ));
    let bed = builder.add_audio_element(
        codec,
        AudioElement::channel_based(
            99,
            42,
            vec![0],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Stereo,
                1,
                1,
            )),
        ),
    );
    let mix = builder.add_mix_presentation(
        vec![bed],
        MixPresentation {
            mix_presentation_id: 7,
            annotations_language: vec![b"en".to_vec()],
            localized_presentation_annotations: vec![b"stereo".to_vec()],
            sub_mixes: vec![SubMix {
                elements: vec![SubMixAudioElement {
                    audio_element_id: 99,
                    localized_element_annotations: vec![b"bed".to_vec()],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: MixGainParamDefinition::mode_1(100, 16_000),
                }],
                output_mix_gain: MixGainParamDefinition::mode_1(101, 16_000),
                layouts: vec![LayoutWithLoudness {
                    layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                    reserved: 0,
                    loudness: Loudness::new(0, 0),
                }],
            }],
            trailing: Vec::new(),
        },
    );

    let (encoder, manifest) = builder.build().unwrap();

    assert_eq!(manifest.codec_config_id(codec), Some(0));
    assert_eq!(manifest.audio_element_id(bed), Some(0));
    assert_eq!(manifest.mix_presentation_id(mix), Some(0));
    assert_eq!(encoder.descriptors().mix_presentations.len(), 1);
}

#[test]
fn build_rejects_a_presentation_whose_declared_handles_do_not_match_its_elements() {
    let mut builder = EncoderBuilder::new();
    let _mix = builder.add_mix_presentation(
        Vec::new(),
        MixPresentation {
            mix_presentation_id: 7,
            annotations_language: vec![b"en".to_vec()],
            localized_presentation_annotations: vec![b"stereo".to_vec()],
            sub_mixes: vec![SubMix {
                elements: vec![SubMixAudioElement {
                    audio_element_id: 99,
                    localized_element_annotations: vec![b"bed".to_vec()],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: MixGainParamDefinition::mode_1(100, 16_000),
                }],
                output_mix_gain: MixGainParamDefinition::mode_1(101, 16_000),
                layouts: vec![LayoutWithLoudness {
                    layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                    reserved: 0,
                    loudness: Loudness::new(0, 0),
                }],
            }],
            trailing: Vec::new(),
        },
    );

    let error = builder
        .build()
        .expect_err("the presentation has an element with no matching handle");

    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(
        error.at(),
        Location::Field("mix_presentation.audio_elements")
    );
}

#[test]
fn build_rejects_a_codec_handle_from_another_builder_with_the_same_index() {
    let mut source = EncoderBuilder::new();
    let foreign_codec = source.add_codec_config(lpcm_config());

    let mut target = EncoderBuilder::new();
    let _local_codec = target.add_codec_config(lpcm_config());
    let _element = target.add_audio_element(foreign_codec, stereo_element());

    let error = target
        .build()
        .expect_err("a caller-local codec handle cannot name another builder's declaration");

    assert_eq!(error.kind(), &ErrorKind::UnknownCodecConfigHandle);
}

#[test]
fn build_rejects_an_audio_handle_from_another_builder_with_the_same_index() {
    let mut source = EncoderBuilder::new();
    let source_codec = source.add_codec_config(lpcm_config());
    let foreign_audio = source.add_audio_element(source_codec, stereo_element());

    let mut target = EncoderBuilder::new();
    let target_codec = target.add_codec_config(lpcm_config());
    let _local_audio = target.add_audio_element(target_codec, stereo_element());
    let _presentation = target.add_mix_presentation(vec![foreign_audio], stereo_presentation());

    let error = target
        .build()
        .expect_err("a caller-local audio handle cannot name another builder's declaration");

    assert_eq!(error.kind(), &ErrorKind::UnknownAudioElementHandle);
}

#[test]
fn build_rejects_a_malformed_extension_definition_before_accepting_the_encoder() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let mut element = stereo_element();
    element.params.push(AudioElementParam::Extension {
        param_definition_type: 3,
        bytes: Vec::new(),
    });
    let _element = builder.add_audio_element(codec, element);

    let error = builder
        .build()
        .expect_err("a malformed extension definition must fail the static gate");

    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert_eq!(error.at(), Location::InputOffset(0));
}

fn lpcm_config() -> CodecConfig {
    CodecConfig::lpcm(
        42,
        128,
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
        vec![0],
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            LoudspeakerLayout::Stereo,
            1,
            1,
        )),
    )
}

fn stereo_presentation() -> MixPresentation {
    MixPresentation {
        mix_presentation_id: 7,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![b"stereo".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 99,
                localized_element_annotations: vec![b"bed".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: MixGainParamDefinition::mode_1(100, 16_000),
            }],
            output_mix_gain: MixGainParamDefinition::mode_1(101, 16_000),
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(0, 0),
            }],
        }],
        trailing: Vec::new(),
    }
}
