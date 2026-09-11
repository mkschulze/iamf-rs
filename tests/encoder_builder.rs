//! Public-boundary tests for the immutable high-level encoder configuration.

use iamf::encoder::EncoderBuilder;
use iamf::model::Profile;
use iamf::model::layout::{ExpandedLoudspeakerLayout, LoudspeakerLayout, SoundSystem};
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
    let shared = add_fresh_element(&mut builder, codec, stereo_element());
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let second = add_fresh_element(&mut builder, codec, stereo_element());

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
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let second = add_fresh_element(&mut builder, codec, stereo_element());
    let third = add_fresh_element(&mut builder, codec, stereo_element());
    let small = add_fresh_element(&mut builder, codec, stereo_element());

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

#[test]
fn build_rejects_codec_id_payload_mismatch() {
    let mut builder = EncoderBuilder::new();
    let mut config = lpcm_config();
    config.codec_id = iamf::obu::CODEC_ID_FLAC;
    builder.add_codec_config(config);
    assert!(builder.build().is_err());
}

#[test]
fn build_rejects_raw_codec_configs_even_for_known_ids() {
    let mut builder = EncoderBuilder::new();
    let mut config = lpcm_config();
    config.decoder_config = iamf::obu::DecoderConfig::Raw {
        codec_id: iamf::obu::CODEC_ID_LPCM,
        bytes: vec![],
    };
    builder.add_codec_config(config);
    assert!(builder.build().is_err());
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

fn build_element(
    element: AudioElement,
) -> iamf::Result<(iamf::encoder::Encoder, iamf::encoder::IdManifest)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    builder.add_audio_element(codec, element);
    builder.build()
}

#[test]
fn build_rejects_scalable_channel_layers() {
    let element = AudioElement::channel_based(
        1,
        1,
        vec![0, 1],
        ScalableChannelLayoutConfig {
            reserved: 0,
            layers: vec![ChannelAudioLayerConfig::new(LoudspeakerLayout::Stereo, 1, 1); 2],
        },
    );
    assert!(build_element(element).is_err());
}

#[test]
fn build_rejects_incoherent_channel_topology() {
    for (layout, streams, coupled) in [
        (LoudspeakerLayout::Stereo, 1, 0),
        (LoudspeakerLayout::Ch5_1, 2, 4),
    ] {
        let element = AudioElement::channel_based(
            1,
            1,
            (0..u32::from(streams)).collect(),
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                layout, streams, coupled,
            )),
        );
        assert!(
            build_element(element).is_err(),
            "accepted {layout:?}, {streams}, {coupled}"
        );
    }
}

#[test]
fn build_rejects_projection_even_when_unreferenced() {
    let mut element = stereo_element();
    element.audio_element_type = iamf::obu::AudioElementType::scene_based_for_test(
        iamf::model::layout::AmbisonicsConfig::Projection(
            iamf::model::layout::AmbisonicsProjectionConfig {
                output_channel_count: 1,
                substream_count: 1,
                coupled_substream_count: 0,
                demixing_matrix: vec![1],
            },
        ),
    );
    assert!(build_element(element).is_err());
}

#[test]
fn build_rejects_reserved_layout_even_when_unreferenced() {
    for layout in [
        LoudspeakerLayout::Reserved(10),
        LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Reserved(13)),
    ] {
        let mut element = stereo_element();
        if let iamf::obu::AudioElementType::ChannelBased(config) = &mut element.audio_element_type {
            config
                .scalable_channel_layout
                .layers
                .first_mut()
                .unwrap()
                .loudspeaker_layout = layout;
        }
        let error = build_element(element).unwrap_err();
        assert_eq!(error.kind(), &ErrorKind::UnsupportedLayout);
    }
}

#[test]
fn build_rejects_invalid_ambisonics_mono_mapping() {
    let mut element = stereo_element();
    element.audio_element_type = iamf::obu::AudioElementType::scene_based_for_test(
        iamf::model::layout::AmbisonicsConfig::Mono(iamf::model::layout::AmbisonicsMonoConfig {
            output_channel_count: 4,
            substream_count: 1,
            channel_mapping: vec![0, 1, 255, 255],
        }),
    );
    assert!(build_element(element).is_err());
}

fn stereo_presentation() -> MixPresentation {
    presentation_for_elements(&[99], 100, 101)
}

fn build_presentation(
    presentation: MixPresentation,
) -> iamf::Result<(iamf::encoder::Encoder, iamf::encoder::IdManifest)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = builder.add_audio_element(codec, stereo_element());
    builder.add_mix_presentation(vec![element], presentation);
    builder.build()
}

#[test]
fn build_rejects_embedded_nul_in_labels() {
    let mut presentation = stereo_presentation();
    presentation.annotations_language = vec![b"e\0n".to_vec()];
    assert!(build_presentation(presentation).is_err());
}

#[test]
fn build_rejects_oversized_anchored_loudness() {
    let mut presentation = stereo_presentation();
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .layouts
        .first_mut()
        .unwrap()
        .loudness
        .anchored = Some(iamf::obu::AnchoredLoudness {
        anchor_elements: vec![
            iamf::obu::AnchorElement {
                anchor_element: 1,
                anchored_loudness: 0
            };
            256
        ],
    });
    assert!(build_presentation(presentation).is_err());
}

#[test]
fn build_rejects_loudness_extension_without_wire_gate() {
    let mut presentation = stereo_presentation();
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .layouts
        .first_mut()
        .unwrap()
        .loudness
        .extension = Some(iamf::obu::LoudnessExtension {
        info_type_bits: 0,
        bytes: vec![1],
    });
    assert!(build_presentation(presentation).is_err());
}

#[test]
fn build_rejects_reserved_presentation_layout() {
    let mut presentation = stereo_presentation();
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .layouts
        .push(LayoutWithLoudness {
            layout: Layout::SoundSystem(SoundSystem::Reserved(15)),
            reserved: 0,
            loudness: Loudness::new(0, 0),
        });
    assert_eq!(
        build_presentation(presentation).unwrap_err().kind(),
        &ErrorKind::UnsupportedLayout
    );
}

#[test]
fn build_rejects_zero_parameter_rate() {
    let mut presentation = stereo_presentation();
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .output_mix_gain
        .definition
        .parameter_rate = 0;
    assert!(build_presentation(presentation).is_err());
}

#[test]
fn build_rejects_invalid_static_parameter_durations() {
    for (duration, constant, subblocks) in [
        (0, 0, vec![]),
        (4, 0, vec![1, 2]),
        (4, 0, vec![0, 4]),
        (4, 2, vec![2, 2]),
        (4, 5, vec![]),
        (4, 0, vec![u32::MAX, 5]),
    ] {
        let mut presentation = stereo_presentation();
        presentation
            .sub_mixes
            .first_mut()
            .unwrap()
            .output_mix_gain
            .definition
            .duration_fields = Some(iamf::obu::DurationFields {
            duration,
            constant_subblock_duration: constant,
            subblock_durations: subblocks.clone(),
        });
        assert!(
            build_presentation(presentation).is_err(),
            "accepted {duration}, {constant}, {subblocks:?}"
        );
    }
}

#[test]
fn build_rejects_accidental_substream_collisions() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    builder.add_audio_element(codec, stereo_element());
    builder.add_audio_element(codec, stereo_element());
    assert_eq!(
        builder.build().unwrap_err().kind(),
        &ErrorKind::DuplicateDeclaration
    );
}

#[test]
fn all_substream_and_parameter_ids_are_allocated_and_manifested() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let mut template = stereo_element();
    template.audio_substream_ids = vec![900];
    let audio = builder.add_audio_element(codec, template);
    let mix = builder.add_mix_presentation(vec![audio], stereo_presentation());
    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(
        encoder
            .descriptors()
            .audio_elements
            .first()
            .unwrap()
            .audio_substream_ids,
        vec![0]
    );
    let stream = *manifest.substreams(audio).unwrap().first().unwrap();
    assert_eq!(manifest.substream_id(stream), Some(0));
    let params = manifest.parameters(mix).unwrap();
    assert_eq!(params.len(), 2);
    assert_eq!(manifest.parameter_id(*params.first().unwrap()), Some(0));
    assert_eq!(manifest.parameter_id(*params.last().unwrap()), Some(1));
    let submix = encoder
        .descriptors()
        .mix_presentations
        .first()
        .unwrap()
        .sub_mixes
        .first()
        .unwrap();
    assert_eq!(
        submix
            .elements
            .first()
            .unwrap()
            .element_mix_gain
            .definition
            .parameter_id,
        0
    );
    assert_eq!(submix.output_mix_gain.definition.parameter_id, 1);
}

#[test]
fn repeated_parameter_handle_is_rejected_even_when_substreams_are_shared() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let stream = builder.add_substream();
    let gain = builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(900, 16_000));
    let first = builder.add_audio_element_with_substreams(codec, vec![stream], stereo_element());
    let second = builder.add_audio_element_with_substreams(codec, vec![stream], stereo_element());
    builder.add_mix_presentation_with_parameters(
        vec![first],
        vec![gain, gain],
        stereo_presentation(),
    );
    builder.add_mix_presentation_with_parameters(
        vec![second],
        vec![gain, gain],
        stereo_presentation(),
    );
    assert_eq!(
        builder.build().unwrap_err().kind(),
        &ErrorKind::DuplicateDeclaration
    );
}

#[test]
fn explicit_substream_sharing_with_distinct_parameters_produces_valid_descriptors() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let stream = builder.add_substream();
    for _ in 0..2 {
        let audio =
            builder.add_audio_element_with_substreams(codec, vec![stream], stereo_element());
        let element_gain =
            builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(900, 16_000));
        let output_gain =
            builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(900, 16_000));
        builder.add_mix_presentation_with_parameters(
            vec![audio],
            vec![element_gain, output_gain],
            stereo_presentation(),
        );
    }
    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.substream_id(stream), Some(0));
    assert_eq!(encoder.descriptors().audio_elements.len(), 2);
    for element in &encoder.descriptors().audio_elements {
        assert_eq!(element.audio_substream_ids, vec![0]);
    }
    assert!(encoder.descriptors().validate().is_empty());
}

#[test]
fn foreign_substream_and_parameter_handles_are_rejected() {
    let mut source = EncoderBuilder::new();
    let foreign_stream = source.add_substream();
    let foreign_gain = source.add_mix_gain_parameter(MixGainParamDefinition::mode_1(0, 16_000));
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    builder.add_substream();
    builder.add_audio_element_with_substreams(codec, vec![foreign_stream], stereo_element());
    assert_eq!(
        builder.build().unwrap_err().kind(),
        &ErrorKind::UnknownSubstreamHandle
    );
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let audio = builder.add_audio_element(codec, stereo_element());
    builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(0, 16_000));
    builder.add_mix_presentation_with_parameters(
        vec![audio],
        vec![foreign_gain, foreign_gain],
        stereo_presentation(),
    );
    assert_eq!(
        builder.build().unwrap_err().kind(),
        &ErrorKind::UnknownParameterHandle
    );
}

#[test]
fn shared_substream_requires_matching_channel_width() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let stream = builder.add_substream();
    builder.add_audio_element_with_substreams(codec, vec![stream], stereo_element());
    let mono = AudioElement::channel_based(
        0,
        0,
        vec![0],
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            LoudspeakerLayout::Mono,
            1,
            0,
        )),
    );
    builder.add_audio_element_with_substreams(codec, vec![stream], mono);
    assert!(builder.build().is_err());
}

#[test]
fn builder_authors_ambisonics_mono_through_opaque_substreams() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let streams: Vec<_> = (0..4).map(|_| builder.add_substream()).collect();
    let scene = builder.add_ambisonics_mono(
        codec,
        streams,
        iamf::model::layout::AmbisonicsMonoConfig {
            output_channel_count: 4,
            substream_count: 4,
            channel_mapping: vec![0, 1, 2, 3],
        },
    );
    builder.add_mix_presentation(vec![scene], stereo_presentation());
    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Simple);
    assert_eq!(
        encoder
            .descriptors()
            .audio_elements
            .first()
            .unwrap()
            .audio_substream_ids,
        vec![0, 1, 2, 3]
    );
}

fn two_presentation_builder() -> EncoderBuilder {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(
        &mut builder,
        codec,
        element_with_layout(LoudspeakerLayout::Expanded(
            ExpandedLoudspeakerLayout::Ch9_1_6,
        )),
    );
    let second = add_fresh_element(
        &mut builder,
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

fn add_fresh_element(
    builder: &mut EncoderBuilder,
    codec: iamf::encoder::CodecConfigHandle,
    element: AudioElement,
) -> iamf::encoder::AudioElementHandle {
    let streams = element
        .audio_substream_ids
        .iter()
        .map(|_| builder.add_substream())
        .collect();
    builder.add_audio_element_with_substreams(codec, streams, element)
}

#[test]
fn high_level_rejects_deferred_audio_element_parameters() {
    let mut element = stereo_element();
    element.params.push(AudioElementParam::ReconGain {
        definition: iamf::obu::ParamDefinition::mode_1(0, 16_000),
    });
    assert_eq!(
        build_element(element).unwrap_err().kind(),
        &ErrorKind::UnsupportedParameterData
    );
}

#[test]
fn high_level_rejects_recon_gain_layer_gate() {
    let mut element = stereo_element();
    if let iamf::obu::AudioElementType::ChannelBased(config) = &mut element.audio_element_type {
        config
            .scalable_channel_layout
            .layers
            .first_mut()
            .unwrap()
            .recon_gain_is_present = true;
    }
    assert!(build_element(element).is_err());
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
            0,
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
