//! Public-boundary tests for the immutable high-level encoder configuration.

use iamf::encoder::EncoderBuilder;
use iamf::model::Profile;
use iamf::model::layout::{ExpandedLoudspeakerLayout, LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AnchorElement, AnchoredLoudness, AudioElement, AudioElementParam, ChannelAudioLayerConfig,
    CodecConfig, HeadphonesRenderingMode, Layout, LayoutWithLoudness, Loudness, LpcmDecoderConfig,
    MixGainParamDefinition, MixPresentation, RenderingConfig, SampleFormatFlags,
    ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
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
fn build_rejects_audio_element_params_even_when_an_extension_definition_is_opaque() {
    // Opaque extension definitions are no longer decoded (quick 260914-5c5),
    // so the params gate is what rejects them.
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
        .expect_err("the builder rejects Audio Element params");

    assert_eq!(error.kind(), &ErrorKind::UnsupportedParameterData);
    assert_eq!(error.at(), Location::Field("audio_element_params"));
}

#[test]
fn build_rejects_257_audio_element_params_through_the_element_finding() {
    // build() never accepts Audio Element params, but the element's own
    // num_parameters finding (quick 260914-kfs) runs first at
    // `validate_findings(declaration.element.validate())`, the same precedence
    // as the existing param_definition_type 0 finding.
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let mut element = stereo_element();
    element.params = core::iter::repeat_with(|| AudioElementParam::Extension {
        param_definition_type: 3,
        bytes: Vec::new(),
    })
    .take(257)
    .collect();
    let _element = builder.add_audio_element(codec, element);

    let error = builder
        .build()
        .expect_err("the builder rejects more than 256 Audio Element params");

    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(error.at(), Location::Field("descriptors"));
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

// ref: IAMF v1.1.0 index.bs:1278, :1920; libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:760-782
#[test]
fn build_rejects_two_sub_mixes() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let second = add_fresh_element(&mut builder, codec, stereo_element());
    let mut presentation = presentation_for_elements(&[99], 100, 110);
    let second_sub_mix = presentation_for_elements(&[100], 200, 210)
        .sub_mixes
        .into_iter()
        .next()
        .expect("the helper presentation has one sub-mix");
    presentation.sub_mixes.push(second_sub_mix);
    builder.add_mix_presentation(vec![first, second], presentation);

    let error = builder
        .build()
        .expect_err("libiamf fails the Mix Presentation parse on two sub-mixes");

    assert_eq!(error.kind(), &ErrorKind::SubMixCountNotOne);
    assert_eq!(error.at(), Location::Field("num_sub_mixes"));
}

// ref: IAMF v1.1.0 index.bs:1929; iamf-tools@v2.1.0 iamf/cli/obu_processor.cc:531-533
#[test]
fn build_rejects_zero_mix_presentations() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    add_fresh_element(&mut builder, codec, stereo_element());

    let error = builder
        .build()
        .expect_err("an IA sequence without a Mix Presentation never configures a decoder");

    assert_eq!(error.kind(), &ErrorKind::NoMixPresentation);
    assert_eq!(error.at(), Location::Field("mix_presentations"));
}

// ref: IAMF v1.1.0 index.bs:1278 (num_sub_mixes SHALL NOT be 0)
#[test]
fn build_rejects_zero_sub_mixes() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    add_fresh_element(&mut builder, codec, stereo_element());
    let mut presentation = stereo_presentation();
    presentation.sub_mixes.clear();
    builder.add_mix_presentation(vec![], presentation);

    let error = builder
        .build()
        .expect_err("a Mix Presentation without a sub-mix is not conformant");

    assert_eq!(error.kind(), &ErrorKind::SubMixCountNotOne);
    assert_eq!(error.at(), Location::Field("num_sub_mixes"));
}

// ref: IAMF v1.1.0 index.bs:1337-1341; libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:1309-1315
#[test]
fn build_rejects_reserved_headphones_rendering_modes() {
    for mode in [2, 3] {
        let mut presentation = stereo_presentation();
        presentation
            .sub_mixes
            .first_mut()
            .and_then(|sub_mix| sub_mix.elements.first_mut())
            .expect("the stereo presentation references one element")
            .rendering_config
            .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(mode);

        let error = build_presentation(presentation)
            .expect_err("a reserved headphones_rendering_mode drops the Mix Presentation");

        assert_eq!(error.kind(), &ErrorKind::ReservedHeadphonesRenderingMode);
        assert_eq!(error.at(), Location::Field("headphones_rendering_mode"));
    }
}

#[test]
fn build_accepts_binaural_headphones_rendering_mode() {
    let mut presentation = stereo_presentation();
    presentation
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .expect("the stereo presentation references one element")
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Binaural;

    let (_, manifest) = build_presentation(presentation).unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Simple);
}

// ref: IAMF v1.1.0 index.bs:1280 (no duplicate audio_element_id within one Mix Presentation)
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:41-54 ValidateUniqueAudioElementIds
#[test]
fn build_rejects_the_same_audio_element_twice_in_one_presentation() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    builder.add_mix_presentation(
        vec![element, element],
        presentation_for_elements(&[99, 100], 100, 110),
    );

    let error = builder
        .build()
        .expect_err("iamf-tools rejects a duplicate audio_element_id on read and write");

    assert_eq!(
        error.kind(),
        &ErrorKind::DuplicateMixPresentationAudioElement
    );
    assert_eq!(
        error.at(),
        Location::Field("mix_presentation.audio_elements")
    );
}

#[test]
fn build_rejects_the_same_audio_element_across_two_sub_mixes_as_sub_mix_count_first() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    let mut presentation = presentation_for_elements(&[99], 100, 110);
    let mut second_sub_mix = presentation_for_elements(&[100], 200, 210)
        .sub_mixes
        .into_iter()
        .next()
        .expect("the helper presentation has one sub-mix");
    second_sub_mix
        .elements
        .first_mut()
        .expect("the helper sub-mix references one element")
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(2);
    presentation.sub_mixes.push(second_sub_mix);
    builder.add_mix_presentation(vec![element, element], presentation);

    let error = builder
        .build()
        .expect_err("two sub-mixes are rejected before any other presentation rule");

    assert_eq!(error.kind(), &ErrorKind::SubMixCountNotOne);
    assert_eq!(error.at(), Location::Field("num_sub_mixes"));
}

#[test]
fn reserved_headphones_mode_wins_over_a_duplicate_handle() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    let mut presentation = presentation_for_elements(&[99, 100], 100, 110);
    presentation
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .expect("the helper presentation references two elements")
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(3);
    builder.add_mix_presentation(vec![element, element], presentation);

    let error = builder
        .build()
        .expect_err("a reserved headphones mode is rejected before a duplicate handle");

    assert_eq!(error.kind(), &ErrorKind::ReservedHeadphonesRenderingMode);
    assert_eq!(error.at(), Location::Field("headphones_rendering_mode"));
}

#[test]
fn a_duplicate_handle_wins_over_a_presentation_finding() {
    let without_layouts = || {
        let mut presentation = presentation_for_elements(&[99, 100], 100, 110);
        presentation
            .sub_mixes
            .first_mut()
            .expect("the helper presentation has one sub-mix")
            .layouts
            .clear();
        presentation
    };

    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    builder.add_mix_presentation(vec![element, element], without_layouts());
    let error = builder
        .build()
        .expect_err("the dedicated duplicate kind is reported before generic findings");
    assert_eq!(
        error.kind(),
        &ErrorKind::DuplicateMixPresentationAudioElement
    );
    assert_eq!(
        error.at(),
        Location::Field("mix_presentation.audio_elements")
    );

    // Control: distinct handles reach the generic presentation finding.
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let second = add_fresh_element(&mut builder, codec, stereo_element());
    builder.add_mix_presentation(vec![first, second], without_layouts());
    let error = builder
        .build()
        .expect_err("a sub-mix without a stereo layout is a presentation finding");
    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(error.at(), Location::Field("descriptors"));
}

fn presentation_with_languages(languages: &[&[u8]]) -> MixPresentation {
    let mut presentation = presentation_for_elements(&[0], 100, 110);
    presentation.annotations_language =
        languages.iter().map(|language| language.to_vec()).collect();
    presentation.localized_presentation_annotations = vec![b"stereo".to_vec(); languages.len()];
    for sub_mix in &mut presentation.sub_mixes {
        for element in &mut sub_mix.elements {
            element.localized_element_annotations = vec![b"bed".to_vec(); languages.len()];
        }
    }
    presentation
}

fn set_anchors(presentation: &mut MixPresentation, anchors: &[u8]) {
    for sub_mix in &mut presentation.sub_mixes {
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

fn build_single(
    presentation: MixPresentation,
) -> iamf::Result<(iamf::encoder::Encoder, iamf::encoder::IdManifest)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    builder.add_mix_presentation(vec![element], presentation);
    builder.build()
}

#[test]
fn build_rejects_duplicate_annotations_languages() {
    let error = build_single(presentation_with_languages(&[b"en", b"en"]))
        .expect_err("a repeated annotations_language is rejected");
    assert_eq!(error.kind(), &ErrorKind::DuplicateAnnotationsLanguage);
    assert_eq!(error.at(), Location::Field("annotations_language"));
}

#[test]
fn build_rejects_annotations_languages_differing_only_in_ascii_case() {
    let error = build_single(presentation_with_languages(&[b"en-US", b"en-us"]))
        .expect_err("BCP-47 tags are case-insensitive");
    assert_eq!(error.kind(), &ErrorKind::DuplicateAnnotationsLanguage);
    assert_eq!(error.at(), Location::Field("annotations_language"));
}

#[test]
fn distinct_annotations_languages_build() {
    assert!(build_single(presentation_with_languages(&[b"en", b"es"])).is_ok());
}

#[test]
fn build_rejects_duplicate_anchor_elements() {
    for anchors in [[1, 1], [0, 0]] {
        let mut presentation = presentation_for_elements(&[0], 100, 110);
        set_anchors(&mut presentation, &anchors);
        let error = build_single(presentation)
            .expect_err("a repeated anchor_element, Unknown included, is rejected");
        assert_eq!(error.kind(), &ErrorKind::DuplicateAnchorElement);
        assert_eq!(
            error.at(),
            Location::Field("anchored_loudness.anchor_element")
        );
    }
}

#[test]
fn distinct_anchor_elements_build() {
    let mut presentation = presentation_for_elements(&[0], 100, 110);
    set_anchors(&mut presentation, &[1, 2]);
    assert!(build_single(presentation).is_ok());
}

#[test]
fn a_duplicate_handle_wins_over_a_duplicate_annotations_language() {
    let mut presentation = presentation_for_elements(&[0, 0], 100, 110);
    presentation.annotations_language = vec![b"en".to_vec(); 2];
    presentation.localized_presentation_annotations = vec![b"stereo".to_vec(); 2];
    for sub_mix in &mut presentation.sub_mixes {
        for element in &mut sub_mix.elements {
            element.localized_element_annotations = vec![b"bed".to_vec(); 2];
        }
    }
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    builder.add_mix_presentation(vec![element, element], presentation);

    let error = builder
        .build()
        .expect_err("a duplicate handle is reported before a duplicate language");
    assert_eq!(
        error.kind(),
        &ErrorKind::DuplicateMixPresentationAudioElement
    );
    assert_eq!(
        error.at(),
        Location::Field("mix_presentation.audio_elements")
    );
}

#[test]
fn a_duplicate_annotations_language_wins_over_a_duplicate_anchor_and_a_presentation_finding() {
    let mut presentation = presentation_with_languages(&[b"en", b"en"]);
    set_anchors(&mut presentation, &[1, 1]);
    presentation.localized_presentation_annotations.pop();

    let error = build_single(presentation)
        .expect_err("a duplicate language is reported before anchors and findings");
    assert_eq!(error.kind(), &ErrorKind::DuplicateAnnotationsLanguage);
    assert_eq!(error.at(), Location::Field("annotations_language"));
}

// Quick 260914-m62: build() refuses a tag that is not RFC 5646 section 2.1 well-formed.
#[test]
fn build_rejects_annotations_languages_that_are_not_well_formed_bcp47() {
    let languages: [&[u8]; 4] = [b"en_US", b"", b"en-", b"e\0n"];
    for language in languages {
        let error = build_single(presentation_with_languages(&[language]))
            .expect_err("a malformed annotations_language is rejected");
        assert_eq!(
            error.kind(),
            &ErrorKind::AnnotationsLanguageNotWellFormed,
            "tag {:?}",
            String::from_utf8_lossy(language)
        );
        assert_eq!(error.at(), Location::Field("annotations_language"));
    }
}

// Quick 260914-m62: the malformed-tag kind precedes anchors and generic presentation findings.
#[test]
fn a_malformed_annotations_language_wins_over_a_duplicate_anchor_and_a_presentation_finding() {
    let mut presentation = presentation_with_languages(&[b"en_US"]);
    set_anchors(&mut presentation, &[1, 1]);
    presentation.localized_presentation_annotations.pop();

    let error = build_single(presentation)
        .expect_err("a malformed language is reported before anchors and findings");
    assert_eq!(error.kind(), &ErrorKind::AnnotationsLanguageNotWellFormed);
    assert_eq!(error.at(), Location::Field("annotations_language"));
}

// Quick 260914-m62: the duplicate check runs first, so a repeated malformed tag stays a duplicate.
#[test]
fn a_duplicate_annotations_language_wins_over_a_malformed_one() {
    let error = build_single(presentation_with_languages(&[b"en_US", b"en_US"]))
        .expect_err("a repeated malformed language is rejected");
    assert_eq!(error.kind(), &ErrorKind::DuplicateAnnotationsLanguage);
    assert_eq!(error.at(), Location::Field("annotations_language"));
}

// Quick 260914-m62: grandfathered, private-use and three-digit-region tags build.
#[test]
fn well_formed_bcp47_annotations_languages_build() {
    assert!(
        build_single(presentation_with_languages(&[
            b"en",
            b"zh-Hant-TW",
            b"x-private",
            b"i-klingon",
            b"es-419",
        ]))
        .is_ok()
    );
}

#[test]
fn a_duplicate_anchor_element_wins_over_a_presentation_finding() {
    let without_stereo = |anchors: &[u8]| {
        let mut presentation = presentation_for_elements(&[0], 100, 110);
        for sub_mix in &mut presentation.sub_mixes {
            for layout in &mut sub_mix.layouts {
                layout.layout = Layout::SoundSystem(SoundSystem::B0_5_0);
            }
        }
        set_anchors(&mut presentation, anchors);
        presentation
    };

    let error = build_single(without_stereo(&[2, 2]))
        .expect_err("a duplicate anchor is reported before the missing stereo layout");
    assert_eq!(error.kind(), &ErrorKind::DuplicateAnchorElement);
    assert_eq!(
        error.at(),
        Location::Field("anchored_loudness.anchor_element")
    );

    // Control: distinct anchors reach the generic presentation finding.
    let error = build_single(without_stereo(&[1, 2]))
        .expect_err("a sub-mix without a stereo layout is a presentation finding");
    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(error.at(), Location::Field("descriptors"));
}

#[test]
fn distinct_elements_with_equal_template_ids_build() {
    // Templates carry placeholder ids; build() lowers them from the handles,
    // so equal template ids on distinct handles are not duplicates.
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let second = add_fresh_element(&mut builder, codec, stereo_element());
    builder.add_mix_presentation(
        vec![first, second],
        presentation_for_elements(&[0, 0], 100, 110),
    );

    let (_, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Base);
}

#[test]
fn two_presentations_sum_channels_against_the_sequence_wide_base_enhanced_ceiling() {
    // Two 16-channel TOA elements, one per Presentation, total 32 channels
    // across the IA Sequence. Base-Enhanced permits at most 28 channels in
    // total across all Audio Elements in the IA Sequence, so no profile
    // permits the configuration.
    // ref: IAMF v1.1.0 index.bs:1953
    let error = two_presentation_builder()
        .build()
        .expect_err("32 channels across the sequence exceed every profile");
    assert_eq!(error.kind(), &ErrorKind::ChannelCountExceedsProfile);
    assert_eq!(error.at(), Location::Field("loudspeaker_layout"));
}

#[test]
fn three_unique_elements_across_presentations_select_base_enhanced() {
    // Three unique Audio Elements in the IA Sequence exceed Base's two, even
    // though each Presentation references only two.
    // ref: IAMF v1.0.0-errata index.bs:1866 (adopted by v1.1.0 index.bs:1943)
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
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
}

#[test]
fn a_shared_element_counts_once_toward_the_sequence_unique_element_limit() {
    // Three references, two unique Audio Elements: Base.
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let shared = add_fresh_element(&mut builder, codec, stereo_element());
    let other = add_fresh_element(&mut builder, codec, stereo_element());

    let _first_presentation = builder.add_mix_presentation(
        vec![shared, other],
        presentation_for_elements(&[99, 100], 100, 110),
    );
    let _second_presentation =
        builder.add_mix_presentation(vec![shared], presentation_for_elements(&[99], 200, 210));

    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Base);
    assert_eq!(encoder.descriptors().sequence_header.primary_profile, 1);
}

#[test]
fn two_single_stereo_presentations_write_base_profile_bytes() {
    // ref: IAMF v1.0.0-errata index.bs:1853 (Simple: only one unique Audio Element OBU in the IA Sequence)
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let second = add_fresh_element(&mut builder, codec, stereo_element());
    let _first_presentation =
        builder.add_mix_presentation(vec![first], presentation_for_elements(&[99], 100, 110));
    let _second_presentation =
        builder.add_mix_presentation(vec![second], presentation_for_elements(&[100], 200, 210));

    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Base);
    assert_eq!(encoder.descriptors().sequence_header.primary_profile, 1);
    assert_eq!(encoder.descriptors().sequence_header.additional_profile, 1);
}

#[test]
fn two_ambisonics_elements_in_separate_presentations_select_base_enhanced() {
    // ref: IAMF v1.0.0-errata index.bs:1868 (Base: at most one Scene-based Audio Element)
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_foa_element(&mut builder, codec);
    let second = add_foa_element(&mut builder, codec);
    let _first_presentation =
        builder.add_mix_presentation(vec![first], presentation_for_elements(&[99], 100, 110));
    let _second_presentation =
        builder.add_mix_presentation(vec![second], presentation_for_elements(&[100], 200, 210));

    let (_, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
}

#[test]
fn two_ambisonics_elements_in_one_presentation_select_base_enhanced() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_foa_element(&mut builder, codec);
    let second = add_foa_element(&mut builder, codec);
    let _presentation = builder.add_mix_presentation(
        vec![first, second],
        presentation_for_elements(&[99, 100], 100, 110),
    );

    let (_, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
}

#[test]
fn ambisonics_plus_stereo_in_one_presentation_selects_base() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let scene = add_foa_element(&mut builder, codec);
    let bed = add_fresh_element(&mut builder, codec, stereo_element());
    let _presentation = builder.add_mix_presentation(
        vec![scene, bed],
        presentation_for_elements(&[99, 100], 100, 110),
    );

    let (_, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::Base);
}

#[test]
fn an_unreferenced_second_element_raises_simple_to_base() {
    // Unique Audio Element OBUs are counted in the IA Sequence, referenced or not.
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, codec, stereo_element());
    let _unreferenced = add_fresh_element(&mut builder, codec, stereo_element());
    let _presentation =
        builder.add_mix_presentation(vec![first], presentation_for_elements(&[99], 100, 110));

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

#[test]
fn a_lone_expanded_lfe_element_writes_base_enhanced_profile_bytes() {
    // One element, one channel: the counts alone would say Simple. But
    // loudspeaker_layout = 15 is reserved to Simple and Base parsers, and pinned
    // libiamf drops such an element under a Simple header and decodes 0 frames
    // (iamf-tools profile_filter.cc:165-171, libiamf IAMF_decoder.c:643-648).
    let (encoder, manifest) = build_counts(
        LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe),
        1,
        0,
    )
    .expect("a lone expanded LFE element with its reference counts builds");
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
    assert_eq!(encoder.descriptors().sequence_header.primary_profile, 2);
    assert_eq!(encoder.descriptors().sequence_header.additional_profile, 2);
}

#[test]
fn every_named_expanded_layout_builds_a_base_enhanced_header() {
    // Literal rows: every named expanded layout alone writes 2/2, and every
    // standard layout alone stays Simple 0/0.
    for (layout, profile, wire) in [
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoS),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoSs),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoRs),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoTf),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoTb),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Top4Ch),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch3_0),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoF),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoSi),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoTpSi),
            Profile::BaseEnhanced,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Top6Ch),
            Profile::BaseEnhanced,
            2,
        ),
        (LoudspeakerLayout::Mono, Profile::Simple, 0),
        (LoudspeakerLayout::Stereo, Profile::Simple, 0),
        (LoudspeakerLayout::Ch5_1, Profile::Simple, 0),
        (LoudspeakerLayout::Ch5_1_2, Profile::Simple, 0),
        (LoudspeakerLayout::Ch5_1_4, Profile::Simple, 0),
        (LoudspeakerLayout::Ch7_1, Profile::Simple, 0),
        (LoudspeakerLayout::Ch7_1_2, Profile::Simple, 0),
        (LoudspeakerLayout::Ch7_1_4, Profile::Simple, 0),
        (LoudspeakerLayout::Ch3_1_2, Profile::Simple, 0),
        (LoudspeakerLayout::Binaural, Profile::Simple, 0),
    ] {
        let (substream_count, coupled_substream_count) = layout
            .single_layer_substream_counts()
            .expect("every named layout has reference substream counts");
        let (encoder, manifest) = build_counts(layout, substream_count, coupled_substream_count)
            .unwrap_or_else(|error| panic!("{layout:?} did not build: {error:?}"));
        assert_eq!(manifest.sequence_profile(), profile, "{layout:?}");
        let header = &encoder.descriptors().sequence_header;
        assert_eq!(header.primary_profile, wire, "{layout:?} primary_profile");
        assert_eq!(
            header.additional_profile, wire,
            "{layout:?} additional_profile"
        );
    }
}

#[test]
fn a_stereo_presentation_and_an_expanded_presentation_make_a_base_enhanced_sequence() {
    // The stereo presentation alone needs Simple; the expanded one needs
    // Base-Enhanced. The highest presentation sets the sequence header.
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let stereo = add_fresh_element(&mut builder, codec, stereo_element());
    let lfe = add_fresh_element(
        &mut builder,
        codec,
        element_with_layout(LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe)),
    );
    let _stereo_presentation =
        builder.add_mix_presentation(vec![stereo], presentation_for_elements(&[99], 100, 110));
    let _lfe_presentation =
        builder.add_mix_presentation(vec![lfe], presentation_for_elements(&[100], 200, 210));

    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
    assert_eq!(encoder.descriptors().sequence_header.primary_profile, 2);
    assert_eq!(encoder.descriptors().sequence_header.additional_profile, 2);
}

#[test]
fn build_rejects_codec_configs_with_mismatched_frame_timing() -> iamf::Result<()> {
    let mut builder = EncoderBuilder::new();
    let lpcm = builder.add_codec_config(lpcm_config());
    let opus = builder.add_codec_config(CodecConfig::opus(90, 960, 48_000, 312)?);
    let first = add_fresh_element(&mut builder, lpcm, stereo_element());
    let second = add_fresh_element(&mut builder, opus, stereo_element());
    let _presentation = builder.add_mix_presentation(
        vec![first, second],
        presentation_for_elements(&[99, 100], 100, 110),
    );

    let error = builder.build().unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::MultipleCodecConfigs);
    assert_eq!(error.at(), Location::Field("codec_configs"));
    Ok(())
}

#[test]
fn build_rejects_two_codec_configs_with_identical_timing() {
    let mut builder = EncoderBuilder::new();
    let first_codec = builder.add_codec_config(lpcm_config());
    let second_codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, first_codec, stereo_element());
    let second = add_fresh_element(&mut builder, second_codec, stereo_element());
    let _presentation = builder.add_mix_presentation(
        vec![first, second],
        presentation_for_elements(&[99, 100], 100, 110),
    );

    let error = builder.build().unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::MultipleCodecConfigs);
    assert_eq!(error.at(), Location::Field("codec_configs"));
}

#[test]
fn build_rejects_codec_configs_split_across_mix_presentations() {
    let mut builder = EncoderBuilder::new();
    let first_codec = builder.add_codec_config(lpcm_config());
    let second_codec = builder.add_codec_config(lpcm_config());
    let first = add_fresh_element(&mut builder, first_codec, stereo_element());
    let second = add_fresh_element(&mut builder, second_codec, stereo_element());
    let _first_presentation =
        builder.add_mix_presentation(vec![first], presentation_for_elements(&[99], 100, 110));
    let _second_presentation =
        builder.add_mix_presentation(vec![second], presentation_for_elements(&[100], 200, 210));

    let error = builder.build().unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::MultipleCodecConfigs);
    assert_eq!(error.at(), Location::Field("codec_configs"));
}

#[test]
fn build_rejects_a_declared_but_unreferenced_second_codec_config() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let _unreferenced = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(&mut builder, codec, stereo_element());
    let _presentation =
        builder.add_mix_presentation(vec![element], presentation_for_elements(&[99], 100, 110));

    let error = builder.build().unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::MultipleCodecConfigs);
    assert_eq!(error.at(), Location::Field("codec_configs"));
}

#[test]
fn one_codec_config_shared_by_four_elements_in_one_sub_mix_builds() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let elements = (0..4)
        .map(|_| add_fresh_element(&mut builder, codec, stereo_element()))
        .collect();
    let _presentation = builder.add_mix_presentation(
        elements,
        presentation_for_elements(&[99, 100, 101, 102], 100, 110),
    );

    let (encoder, manifest) = builder.build().unwrap();
    assert_eq!(manifest.sequence_profile(), Profile::BaseEnhanced);
    assert_eq!(encoder.descriptors().codec_configs.len(), 1);
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
fn build_rejects_substream_topologies_the_reference_rejects() {
    // Every row is a shape pinned iamf-tools@v2.1.0 decoder_main rejected (research 260913-o3k E),
    // plus the correct-coupled / wrong-substream case the reference checks second.
    let coupled = (
        ErrorKind::CoupledSubstreamCountMismatch,
        "coupled_substream_count",
    );
    let substreams = (ErrorKind::ChannelCountMismatch, "substream_count");
    for (layout, substream_count, coupled_substream_count, (kind, field)) in [
        (LoudspeakerLayout::Stereo, 2, 0, coupled.clone()),
        (LoudspeakerLayout::Binaural, 2, 0, coupled.clone()),
        (LoudspeakerLayout::Ch5_1, 6, 0, coupled.clone()),
        (LoudspeakerLayout::Ch5_1, 5, 1, coupled.clone()),
        (LoudspeakerLayout::Ch7_1_4, 8, 4, coupled.clone()),
        (LoudspeakerLayout::Ch3_1_2, 6, 0, coupled.clone()),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoS),
            2,
            0,
            coupled.clone(),
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch3_0),
            3,
            0,
            coupled.clone(),
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6),
            16,
            0,
            coupled.clone(),
        ),
        (LoudspeakerLayout::Ch5_1, 3, 2, substreams),
    ] {
        let row = format!("{layout:?} {substream_count}/{coupled_substream_count}");
        let error = build_counts(layout, substream_count, coupled_substream_count)
            .err()
            .unwrap_or_else(|| panic!("{row} built, but the reference rejects it"));
        assert_eq!(error.kind(), &kind, "{row}");
        assert_eq!(error.at(), Location::Field(field), "{row}");
    }
}

#[test]
fn build_accepts_the_reference_substream_counts_for_every_named_layout() {
    for (layout, substream_count, coupled_substream_count) in [
        (LoudspeakerLayout::Mono, 1, 0),
        (LoudspeakerLayout::Stereo, 1, 1),
        (LoudspeakerLayout::Ch5_1, 4, 2),
        (LoudspeakerLayout::Ch5_1_2, 5, 3),
        (LoudspeakerLayout::Ch5_1_4, 6, 4),
        (LoudspeakerLayout::Ch7_1, 5, 3),
        (LoudspeakerLayout::Ch7_1_2, 6, 4),
        (LoudspeakerLayout::Ch7_1_4, 7, 5),
        (LoudspeakerLayout::Ch3_1_2, 4, 2),
        (LoudspeakerLayout::Binaural, 1, 1),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe),
            1,
            0,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoS),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoSs),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoRs),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoTf),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoTb),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Top4Ch),
            2,
            2,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch3_0),
            2,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6),
            9,
            7,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoF),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoSi),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::StereoTpSi),
            1,
            1,
        ),
        (
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Top6Ch),
            3,
            3,
        ),
    ] {
        assert_eq!(
            layout.single_layer_substream_counts(),
            Some((substream_count, coupled_substream_count)),
            "{layout:?}"
        );
        assert_eq!(
            layout.channel_count(),
            Some(u32::from(substream_count).saturating_add(u32::from(coupled_substream_count))),
            "{layout:?}"
        );
        if let Err(error) = build_counts(layout, substream_count, coupled_substream_count) {
            panic!("{layout:?} {substream_count}/{coupled_substream_count} rejected: {error:?}");
        }
    }
}

#[test]
fn reserved_layouts_have_no_single_layer_substream_counts() {
    for layout in [
        LoudspeakerLayout::Reserved(10),
        LoudspeakerLayout::Reserved(14),
        LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Reserved(13)),
    ] {
        assert_eq!(layout.single_layer_substream_counts(), None, "{layout:?}");
    }
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
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let stream = builder.add_substream();
    builder.add_ambisonics_mono(
        codec,
        vec![stream],
        iamf::model::layout::AmbisonicsMonoConfig {
            output_channel_count: 4,
            substream_count: 1,
            channel_mapping: vec![0, 1, 255, 255],
        },
    );
    assert!(builder.build().is_err());
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
fn build_rejects_mix_gain_rate_differing_from_the_output_sample_rate() {
    let mut presentation = presentation_for_elements(&[99], 100, 110);
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .output_mix_gain
        .definition
        .parameter_rate = 48_000;

    let error = build_presentation(presentation).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::ParameterRateMismatch);
    assert_eq!(error.at(), Location::Field("parameter_rate"));
}

#[test]
fn opus_mix_gains_must_use_the_48k_output_rate() -> iamf::Result<()> {
    let build_opus = |rate: u32| -> iamf::Result<()> {
        let mut builder = EncoderBuilder::new();
        let opus = builder.add_codec_config(CodecConfig::opus(90, 960, 48_000, 312)?);
        let element = add_fresh_element(&mut builder, opus, stereo_element());
        let mut presentation = presentation_for_elements(&[99], 100, 110);
        let sub_mix = presentation.sub_mixes.first_mut().unwrap();
        sub_mix.output_mix_gain.definition.parameter_rate = rate;
        sub_mix
            .elements
            .first_mut()
            .unwrap()
            .element_mix_gain
            .definition
            .parameter_rate = rate;
        builder.add_mix_presentation(vec![element], presentation);
        builder.build().map(|_| ())
    };

    build_opus(48_000)?;
    let error = build_opus(16_000).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::ParameterRateMismatch);
    assert_eq!(error.at(), Location::Field("parameter_rate"));
    Ok(())
}

#[test]
fn build_rejects_mode_0_duration_differing_from_the_frame_size() {
    let mut presentation = presentation_for_elements(&[99], 100, 110);
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .output_mix_gain
        .definition
        .duration_fields = Some(iamf::obu::DurationFields {
        duration: 64,
        constant_subblock_duration: 64,
        subblock_durations: vec![],
    });

    let error = build_presentation(presentation).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::ParameterBlockDurationMismatch);
    assert_eq!(error.at(), Location::Field("duration_fields"));
}

#[test]
fn mode_0_duration_matching_the_frame_size_builds() -> iamf::Result<()> {
    let mut presentation = presentation_for_elements(&[99], 100, 110);
    presentation
        .sub_mixes
        .first_mut()
        .unwrap()
        .output_mix_gain
        .definition
        .duration_fields = Some(iamf::obu::DurationFields {
        duration: 128,
        constant_subblock_duration: 128,
        subblock_durations: vec![],
    });

    build_presentation(presentation)?;
    Ok(())
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
fn build_rejects_an_unused_explicit_parameter_handle() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let audio = builder.add_audio_element(codec, stereo_element());
    builder.add_mix_presentation(vec![audio], stereo_presentation());
    builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(900, 16_000));

    let error = builder
        .build()
        .expect_err("unused parameter declarations have no emitted wire definition");
    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(error.at(), Location::Field("unused_parameter"));
}

#[test]
fn build_rejects_an_unused_explicit_substream_handle() {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let audio = builder.add_audio_element(codec, stereo_element());
    builder.add_mix_presentation(vec![audio], stereo_presentation());
    builder.add_substream();

    let error = builder
        .build()
        .expect_err("unused substreams have no emitted audio element reference");
    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(error.at(), Location::Field("unused_substream"));
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
    let first = add_toa_element(&mut builder, codec);
    let second = add_toa_element(&mut builder, codec);
    let _first_presentation =
        builder.add_mix_presentation(vec![first], presentation_for_elements(&[99], 100, 110));
    let _second_presentation =
        builder.add_mix_presentation(vec![second], presentation_for_elements(&[100], 200, 210));
    builder
}

/// A 16-channel third-order Ambisonics mono element on fresh substreams.
fn add_toa_element(
    builder: &mut EncoderBuilder,
    codec: iamf::encoder::CodecConfigHandle,
) -> iamf::encoder::AudioElementHandle {
    let streams = (0..16).map(|_| builder.add_substream()).collect();
    builder.add_ambisonics_mono(
        codec,
        streams,
        iamf::model::layout::AmbisonicsMonoConfig {
            output_channel_count: 16,
            substream_count: 16,
            channel_mapping: (0..16).collect(),
        },
    )
}

/// A 4-channel first-order Ambisonics mono element on fresh substreams.
fn add_foa_element(
    builder: &mut EncoderBuilder,
    codec: iamf::encoder::CodecConfigHandle,
) -> iamf::encoder::AudioElementHandle {
    let streams = (0..4).map(|_| builder.add_substream()).collect();
    builder.add_ambisonics_mono(
        codec,
        streams,
        iamf::model::layout::AmbisonicsMonoConfig {
            output_channel_count: 4,
            substream_count: 4,
            channel_mapping: (0..4).collect(),
        },
    )
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

fn element_with_counts(
    layout: LoudspeakerLayout,
    substream_count: u8,
    coupled_substream_count: u8,
) -> AudioElement {
    AudioElement::channel_based(
        99,
        42,
        (0..u32::from(substream_count)).collect(),
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            layout,
            substream_count,
            coupled_substream_count,
        )),
    )
}

fn build_counts(
    layout: LoudspeakerLayout,
    substream_count: u8,
    coupled_substream_count: u8,
) -> iamf::Result<(iamf::encoder::Encoder, iamf::encoder::IdManifest)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(lpcm_config());
    let element = add_fresh_element(
        &mut builder,
        codec,
        element_with_counts(layout, substream_count, coupled_substream_count),
    );
    builder.add_mix_presentation(vec![element], presentation_for_elements(&[99], 100, 101));
    builder.build()
}

#[test]
fn build_rejects_stereo_without_its_coupled_substream() {
    let error = build_counts(LoudspeakerLayout::Stereo, 2, 0).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::CoupledSubstreamCountMismatch);
    assert_eq!(error.at(), Location::Field("coupled_substream_count"));
    assert!(build_counts(LoudspeakerLayout::Stereo, 1, 1).is_ok());
}

fn element_with_layout(layout: LoudspeakerLayout) -> AudioElement {
    let (substream_count, coupled_substream_count) =
        layout.single_layer_substream_counts().unwrap_or((0, 0));
    element_with_counts(layout, substream_count, coupled_substream_count)
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
