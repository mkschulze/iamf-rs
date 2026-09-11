//! Public-boundary tests for the immutable high-level encoder configuration.

use iamf::encoder::EncoderBuilder;
use iamf::model::layout::{ExpandedLoudspeakerLayout, LoudspeakerLayout, SoundSystem};
use iamf::model::Profile;
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

#[test]
fn build_rejects_duplicate_authored_parameter_definition_ids() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = builder.add_audio_element(codec, stereo_element());
    let mut presentation = stereo_presentation();
    presentation
        .sub_mixes
        .first_mut()
        .expect("the stereo fixture has one sub-mix")
        .output_mix_gain
        .definition
        .parameter_id = 100;
    let _presentation = builder.add_mix_presentation(vec![element], presentation);

    let error = builder
        .build()
        .expect_err("static authoring cannot use one parameter id for two definitions");

    assert_eq!(error.kind(), &ErrorKind::DuplicateDeclaration);
    assert_eq!(error.at(), Location::Field("parameter_id"));
}

#[test]
fn unrelated_presentations_do_not_sum_their_element_or_channel_limits() {
    // Two independent 16-channel Presentations would exceed the sequence-wide
    // 28-channel cap if their elements were incorrectly unioned. Each
    // Presentation itself needs only Simple.
    let (_, manifest) = two_presentation_builder().build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Simple);
}

#[test]
fn shared_element_is_counted_in_each_presentation_that_references_it() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let shared = builder.add_audio_element(codec, stereo_element());
    let first = builder.add_audio_element(codec, stereo_element());
    let second = builder.add_audio_element(codec, stereo_element());

    let _first_presentation = builder.add_mix_presentation(
        vec![shared, first],
        presentation_for_elements(&[99, 100], 100, 110),
    );
    let _second_presentation = builder.add_mix_presentation(
        vec![shared, second],
        presentation_for_elements(&[99, 101], 200, 210),
    );

    let (_, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Base);
}

#[test]
fn largest_presentation_sets_the_sequence_profile() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = builder.add_audio_element(codec, stereo_element());
    let second = builder.add_audio_element(codec, stereo_element());
    let third = builder.add_audio_element(codec, stereo_element());
    let small = builder.add_audio_element(codec, stereo_element());

    let _large_presentation = builder.add_mix_presentation(
        vec![first, second, third],
        presentation_for_elements(&[99, 100, 101], 100, 110),
    );
    let _small_presentation =
        builder.add_mix_presentation(vec![small], presentation_for_elements(&[102], 200, 210));

    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
    assert_eq!(encoder.descriptors().sequence_header.primary_profile, 2);
    assert_eq!(encoder.descriptors().sequence_header.additional_profile, 2);
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
    presentation_for_elements(&[99], 100, 101)
}

fn two_presentation_builder() -> EncoderBuilder {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = builder.add_audio_element(
        codec,
        element_with_layout(LoudspeakerLayout::Expanded(
            ExpandedLoudspeakerLayout::Ch9_1_6,
        )),
    );
    let second = builder.add_audio_element(
        codec,
        element_with_layout(LoudspeakerLayout::Expanded(
            ExpandedLoudspeakerLayout::Ch9_1_6,
        )),
    );
    let _first_presentation =
        builder.add_mix_presentation(vec![first], presentation_for_elements(&[99], 100, 110));
    let _second_presentation =
        builder.add_mix_presentation(vec![second], presentation_for_elements(&[100], 200, 210));
    builder
}

fn element_with_layout(layout: LoudspeakerLayout) -> AudioElement {
    let channels = layout.channel_count().unwrap_or(0);
    AudioElement::channel_based(
        99,
        42,
        (0..channels).collect(),
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            layout,
            u8::try_from(channels).unwrap_or(0),
            1,
        )),
    )
}

fn presentation_for_elements(
    element_ids: &[u32],
    element_parameter_id: u32,
    output_parameter_id: u32,
) -> MixPresentation {
    MixPresentation {
        mix_presentation_id: 7,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![b"stereo".to_vec()],
        sub_mixes: vec![SubMix {
            elements: (element_parameter_id..)
                .zip(element_ids.iter())
                .map(
                    |(element_parameter_id, &audio_element_id)| SubMixAudioElement {
                        audio_element_id,
                        localized_element_annotations: vec![b"bed".to_vec()],
                        rendering_config: RenderingConfig::stereo(),
                        element_mix_gain: MixGainParamDefinition::mode_1(
                            element_parameter_id,
                            16_000,
                        ),
                    },
                )
                .collect(),
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
