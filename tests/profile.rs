//! PROF-01, PROF-02 and PROF-03 — the profile enum, minimum-profile selection
//! at every limit with one step either side, and the Q7.8 loudness helper.
//!
//! The limits are `profile_filter.cc`'s, quoted in `src/model/profile.rs`
//! beside the constants:
//!
//! | Profile       | max Audio Elements | max channels |
//! |---------------|--------------------|--------------|
//! | Simple        | 1                  | 16           |
//! | Base          | 2                  | 18           |
//! | Base-Enhanced | 28                 | 28           |
//!
//! A Mix Presentation with an element whose first layer is an expanded
//! loudspeaker layout needs Base-Enhanced regardless of the counts.
//!
//! Scopes follow IAMF v1.1.0, which adopts the v1.0.0-errata Simple and Base
//! sections (quick 260913-qk3):
//!
//! - R1/R3: the Simple and Base unique-element limits (1, 2) span the IA Sequence.
//! - R4/R5: Base's "at most one scene-based" and "at most one channel-based
//!   element with `num_layers > 1`" span the IA Sequence.
//! - R2/R6: the Simple and Base channel limits (16, 18) are per Mix Presentation.
//! - R7: the Base-Enhanced 28-element limit is per Mix Presentation.
//! - R8: the Base-Enhanced 28-channel limit spans the IA Sequence.
//!
//! The per-presentation channel tests below still pass one Mix Presentation's
//! elements to `select_minimum_profile`, which applies both scopes to that set.

use std::fs;
use std::path::{Path, PathBuf};

use iamf::encoder::EncoderBuilder;
use iamf::error::ErrorKind;
use iamf::model::layout::{
    AmbisonicsMonoConfig, ExpandedLoudspeakerLayout, LoudspeakerLayout, SoundSystem,
};
use iamf::model::profile::{
    BASE_ENHANCED_MAX_AUDIO_ELEMENTS, BASE_ENHANCED_MAX_CHANNELS, BASE_MAX_AUDIO_ELEMENTS,
    BASE_MAX_CHANNELS, SIMPLE_MAX_AUDIO_ELEMENTS, SIMPLE_MAX_CHANNELS,
    presentation_minimum_profile, select_sequence_profile,
};
use iamf::model::{Profile, Q7_8, lufs_to_q7_8, select_minimum_profile};
use iamf::obu::{
    AudioElement, ChannelAudioLayerConfig, CodecConfig, HeadphonesRenderingMode, IaSequenceHeader,
    Layout, LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition,
    MixPresentation, RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix,
    SubMixAudioElement,
};

// ---------------------------------------------------------------------------
// Builders. Not `#[test]` bodies, so GUARD-04's in-tests carve-out does not
// reach them and they are written without a panic path.
// ---------------------------------------------------------------------------

/// A channel-based Audio Element whose single layer carries `layout`.
fn element(id: u32, layout: LoudspeakerLayout) -> AudioElement {
    let channels = layout.channel_count().unwrap_or(0);
    let substreams: Vec<u32> = (0..channels).collect();
    AudioElement::channel_based(
        id,
        200,
        substreams,
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(layout, 1, 0)),
    )
}

/// `n` distinct stereo Audio Elements — the cheapest way to vary the element
/// count without also varying the channel count faster than the limits.
fn stereo_elements(n: u32) -> Vec<AudioElement> {
    (0..n)
        .map(|i| element(i, LoudspeakerLayout::Stereo))
        .collect()
}

/// A scene-based Ambisonics mono element carrying `order_channels` channels.
///
/// The scene-based constructor is `pub(crate)`, so the public builder is the
/// only route to one from outside the crate: build a one-element sequence and
/// take its Audio Element back out. Valid sizes are 4 (FOA), 9 (SOA) and 16
/// (TOA).
fn ambisonics_element(order_channels: u8) -> iamf::Result<AudioElement> {
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
    let streams = (0..order_channels)
        .map(|_| builder.add_substream())
        .collect();
    let scene = builder.add_ambisonics_mono(
        codec,
        streams,
        AmbisonicsMonoConfig {
            output_channel_count: order_channels,
            substream_count: order_channels,
            channel_mapping: (0..order_channels).collect(),
        },
    );
    let _presentation = builder.add_mix_presentation(
        vec![scene],
        MixPresentation {
            mix_presentation_id: 7,
            annotations_language: vec![b"en".to_vec()],
            localized_presentation_annotations: vec![b"scene".to_vec()],
            sub_mixes: vec![SubMix {
                elements: vec![SubMixAudioElement {
                    audio_element_id: 0,
                    localized_element_annotations: vec![b"scene".to_vec()],
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
    let (encoder, _manifest) = builder.build()?;
    encoder
        .descriptors()
        .audio_elements
        .first()
        .cloned()
        .ok_or(iamf::Error::new(
            ErrorKind::InvalidDescriptorReference,
            iamf::Location::Unlocated,
        ))
}

/// Select the minimum profile for these elements, as one Mix Presentation.
fn select(elements: &[AudioElement]) -> iamf::Result<(Profile, Profile)> {
    let refs: Vec<&AudioElement> = elements.iter().collect();
    select_minimum_profile(&refs)
}

// ---------------------------------------------------------------------------
// PROF-01 — the enum
// ---------------------------------------------------------------------------

#[test]
fn every_wire_byte_maps_to_a_profile_without_panicking() {
    assert_eq!(Profile::from_wire(0), Profile::Simple);
    assert_eq!(Profile::from_wire(1), Profile::Base);
    assert_eq!(Profile::from_wire(2), Profile::BaseEnhanced);
    assert_eq!(Profile::from_wire(3), Profile::Reserved(3));
    assert_eq!(Profile::from_wire(255), Profile::Reserved(255));

    // Total over the whole byte: none is rejected, so a foreign file carrying
    // a profile this spec version does not name still round-trips.
    for value in 0..=u8::MAX {
        assert_eq!(Profile::from_wire(value).to_wire(), value);
    }
}

#[test]
fn base_enhanced_is_two_and_the_draft_v2_profiles_are_absent() {
    assert_eq!(Profile::BaseEnhanced.to_wire(), 2);
    // iamf-tools@main adds Base-Advanced (3), Advanced1 (4) and Advanced2 (5)
    // on a draft-v2.0.0 tree. SPEC_VERSION is 1.1.0, so they are Reserved here
    // and the crate can never emit one by name.
    assert_eq!(iamf::SPEC_VERSION, "1.1.0");
    for draft in 3..=5_u8 {
        assert_eq!(Profile::from_wire(draft), Profile::Reserved(draft));
    }
}

#[test]
fn profile_has_no_default_impl() {
    // A compile-time claim, proved by a `compile_fail` doctest on `Profile`
    // rather than at run time — `Default::default()` for a type that has no
    // impl is a compile error, not a value a test can inspect. This test
    // records the *reason*, which is the part that rots: a default would let a
    // caller ship Simple by accident for a configuration that needs Base, and
    // libiamf refuses to decode that at all.
    //
    // What can be checked here is that the two ways of getting a Profile both
    // demand a choice: `from_wire` takes a byte, `select_minimum_profile` takes
    // a configuration. Neither has a zero-argument form.
    assert_eq!(Profile::from_wire(1), Profile::Base);
    assert_eq!(
        select(&stereo_elements(2)).map(|(p, _)| p),
        Ok(Profile::Base)
    );
}

// ---------------------------------------------------------------------------
// PROF-02 — the element-count limits, each with one step either side
// ---------------------------------------------------------------------------

#[test]
fn one_audio_element_selects_simple() {
    assert_eq!(SIMPLE_MAX_AUDIO_ELEMENTS, 1);
    let elements = stereo_elements(1);
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Simple));
}

#[test]
fn two_audio_elements_select_base() {
    assert_eq!(BASE_MAX_AUDIO_ELEMENTS, 2);
    let elements = stereo_elements(2);
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Base));
}

#[test]
fn three_audio_elements_select_base_enhanced() {
    let elements = stereo_elements(3);
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::BaseEnhanced));
}

#[test]
fn twenty_eight_audio_elements_select_base_enhanced() {
    assert_eq!(BASE_ENHANCED_MAX_AUDIO_ELEMENTS, 28);
    // 28 mono elements: 28 elements and 28 channels, both exactly at the
    // Base-Enhanced ceiling.
    let elements: Vec<AudioElement> = (0..28)
        .map(|i| element(i, LoudspeakerLayout::Mono))
        .collect();
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::BaseEnhanced));
}

#[test]
fn twenty_nine_audio_elements_are_a_typed_error() {
    let elements: Vec<AudioElement> = (0..29)
        .map(|i| element(i, LoudspeakerLayout::Mono))
        .collect();
    let err = select(&elements).expect_err("29 elements exceed every profile");
    assert_eq!(err.kind(), &ErrorKind::ElementCountExceedsProfile);
}

// ---------------------------------------------------------------------------
// PROF-02 — the channel-count limits, each with one step either side
// ---------------------------------------------------------------------------

#[test]
fn a_single_five_one_six_channel_element_selects_simple() {
    // Exactly the Phase 1 fixture, and plan 01-08 depends on this answer.
    // profile_filter.cc gives the same one: 1 element (<= 1) and 6 channels
    // (<= 16).
    let elements = vec![element(0, LoudspeakerLayout::Ch5_1)];
    assert_eq!(
        elements.first().map(|e| e.audio_substream_ids.len()),
        Some(6),
        "the 5.1 element carries six channels"
    );
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Simple));
}

#[test]
fn expanded_9_1_6_has_sixteen_channels_and_selects_base_enhanced_when_alone() {
    // 16 channels in one element would fit Simple by count, but an expanded
    // loudspeaker layout needs Base-Enhanced even then: iamf-tools
    // profile_filter.cc:165-171 erases Simple and Base for kLayoutExpanded. The
    // channel count is still checked, because a wrong one would reject this
    // supported layout or mis-size the ceilings.
    assert_eq!(ExpandedLoudspeakerLayout::Ch9_1_6.channel_count(), Some(16));
    let elements = vec![element(
        0,
        LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6),
    )];
    assert_eq!(
        select(&elements).map(|(profile, _)| profile),
        Ok(Profile::BaseEnhanced)
    );
}

#[test]
fn every_named_expanded_layout_has_its_normative_channel_count() {
    // These literal counts are the layout definitions. A swapped or guessed
    // mapping changes profile selection for an otherwise valid element.
    let cases = [
        (ExpandedLoudspeakerLayout::Lfe, Some(1)),
        (ExpandedLoudspeakerLayout::StereoS, Some(2)),
        (ExpandedLoudspeakerLayout::StereoSs, Some(2)),
        (ExpandedLoudspeakerLayout::StereoRs, Some(2)),
        (ExpandedLoudspeakerLayout::StereoTf, Some(2)),
        (ExpandedLoudspeakerLayout::StereoTb, Some(2)),
        (ExpandedLoudspeakerLayout::Top4Ch, Some(4)),
        (ExpandedLoudspeakerLayout::Ch3_0, Some(3)),
        (ExpandedLoudspeakerLayout::Ch9_1_6, Some(16)),
        (ExpandedLoudspeakerLayout::StereoF, Some(2)),
        (ExpandedLoudspeakerLayout::StereoSi, Some(2)),
        (ExpandedLoudspeakerLayout::StereoTpSi, Some(2)),
        (ExpandedLoudspeakerLayout::Top6Ch, Some(6)),
        (ExpandedLoudspeakerLayout::Reserved(13), None),
    ];

    for (layout, expected) in cases {
        assert_eq!(layout.channel_count(), expected, "{layout:?}");
    }
}

#[test]
fn every_named_expanded_layout_alone_selects_base_enhanced() {
    // Literal rows, so a regression in the floor cannot hide behind a derived
    // expectation. iamf-tools profile_filter.cc:165-171 erases Simple and Base
    // for kLayoutExpanded, and the 13 named values keep Base-Enhanced.
    for layout in [
        ExpandedLoudspeakerLayout::Lfe,
        ExpandedLoudspeakerLayout::StereoS,
        ExpandedLoudspeakerLayout::StereoSs,
        ExpandedLoudspeakerLayout::StereoRs,
        ExpandedLoudspeakerLayout::StereoTf,
        ExpandedLoudspeakerLayout::StereoTb,
        ExpandedLoudspeakerLayout::Top4Ch,
        ExpandedLoudspeakerLayout::Ch3_0,
        ExpandedLoudspeakerLayout::Ch9_1_6,
        ExpandedLoudspeakerLayout::StereoF,
        ExpandedLoudspeakerLayout::StereoSi,
        ExpandedLoudspeakerLayout::StereoTpSi,
        ExpandedLoudspeakerLayout::Top6Ch,
    ] {
        let elements = vec![element(0, LoudspeakerLayout::Expanded(layout))];
        assert_eq!(
            select(&elements),
            Ok((Profile::BaseEnhanced, Profile::BaseEnhanced)),
            "expanded {layout:?} alone"
        );
    }

    // Control: the standard layouts alone keep the count tiers.
    for layout in [
        LoudspeakerLayout::Mono,
        LoudspeakerLayout::Stereo,
        LoudspeakerLayout::Ch5_1,
        LoudspeakerLayout::Ch5_1_2,
        LoudspeakerLayout::Ch5_1_4,
        LoudspeakerLayout::Ch7_1,
        LoudspeakerLayout::Ch7_1_2,
        LoudspeakerLayout::Ch7_1_4,
        LoudspeakerLayout::Ch3_1_2,
        LoudspeakerLayout::Binaural,
    ] {
        let elements = vec![element(0, layout)];
        assert_eq!(
            select(&elements),
            Ok((Profile::Simple, Profile::Simple)),
            "standard {layout:?} alone"
        );
    }
}

#[test]
fn ambisonics_plus_an_expanded_lfe_selects_base_enhanced_not_base() {
    // FOA + expanded LFE: 2 elements and 5 channels, which the count tiers
    // alone would make Base. The same FOA + mono is Base, so the floor, not
    // the counts, decides.
    let foa = ambisonics_element(4).expect("an FOA element builds");
    let with_lfe = vec![
        foa.clone(),
        element(
            1,
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe),
        ),
    ];
    assert_eq!(
        select(&with_lfe),
        Ok((Profile::BaseEnhanced, Profile::BaseEnhanced))
    );

    let with_mono = vec![foa, element(1, LoudspeakerLayout::Mono)];
    assert_eq!(select(&with_mono), Ok((Profile::Base, Profile::Base)));
}

#[test]
fn ceilings_are_checked_before_the_expanded_floor() {
    // Expanded 9.1.6 + 7.1.4 is exactly 28 channels: within the ceiling.
    let mut elements = vec![
        element(
            0,
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6),
        ),
        element(1, LoudspeakerLayout::Ch7_1_4),
    ];
    assert_eq!(
        select(&elements),
        Ok((Profile::BaseEnhanced, Profile::BaseEnhanced))
    );

    // One more channel: the ceiling wins over the floor.
    elements.push(element(2, LoudspeakerLayout::Mono));
    let err = select(&elements).expect_err("29 channels exceed every profile");
    assert_eq!(err.kind(), &ErrorKind::ChannelCountExceedsProfile);

    // 29 expanded LFE elements: 29 channels too, but the element ceiling is
    // checked first.
    let lfes: Vec<AudioElement> = (0..29)
        .map(|id| {
            element(
                id,
                LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Lfe),
            )
        })
        .collect();
    let err = select(&lfes).expect_err("29 elements exceed every profile");
    assert_eq!(err.kind(), &ErrorKind::ElementCountExceedsProfile);
}

#[test]
fn sixteen_channels_in_one_element_select_simple() {
    assert_eq!(SIMPLE_MAX_CHANNELS, 16);
    // TOA alone: one element, 16 channels. No channel-based layout without an
    // expanded layout reaches 16, and an expanded one would trip the
    // Base-Enhanced floor instead of the channel limit this test is about.
    let elements = vec![ambisonics_element(16).expect("a TOA element builds")];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Simple));
}

#[test]
fn twelve_channels_in_one_channel_based_element_select_simple() {
    // The largest modelled loudspeaker layout, through the channel-based path
    // rather than the scene-based one the boundary cases use.
    let elements = vec![element(0, LoudspeakerLayout::Ch7_1_4)];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Simple));
}

#[test]
fn seventeen_channels_select_base() {
    // TOA + mono: 17 channels in two elements. At most two elements and no
    // expanded layout, so the channel limit, not the element limit or the
    // expanded floor, decides.
    let elements = vec![
        ambisonics_element(16).expect("a TOA element builds"),
        element(1, LoudspeakerLayout::Mono),
    ];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Base));
}

#[test]
fn eighteen_channels_select_base() {
    assert_eq!(BASE_MAX_CHANNELS, 18);
    // 7.1.4 + 5.1: 18 channels in two elements, no expanded layout, so the
    // channel limit decides.
    let elements = vec![
        element(0, LoudspeakerLayout::Ch7_1_4),
        element(1, LoudspeakerLayout::Ch5_1),
    ];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Base));
}

#[test]
fn nineteen_channels_select_base_enhanced() {
    // SOA + 5.1.4: 19 channels in two elements, no expanded layout, so the
    // channel limit decides.
    let elements = vec![
        ambisonics_element(9).expect("an SOA element builds"),
        element(1, LoudspeakerLayout::Ch5_1_4),
    ];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::BaseEnhanced));
}

#[test]
fn twenty_eight_channels_select_base_enhanced() {
    assert_eq!(BASE_ENHANCED_MAX_CHANNELS, 28);
    // TOA + 7.1.4: 28 channels in two elements, no expanded layout, so the
    // channel limit decides.
    let elements = vec![
        ambisonics_element(16).expect("a TOA element builds"),
        element(1, LoudspeakerLayout::Ch7_1_4),
    ];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::BaseEnhanced));
}

#[test]
fn twenty_nine_channels_are_a_typed_error() {
    // TOA + 7.1.4 + mono: 29 channels in three elements, no expanded layout.
    // Three elements are within the Base-Enhanced element ceiling, so the
    // channel ceiling is the deciding rule.
    let elements = vec![
        ambisonics_element(16).expect("a TOA element builds"),
        element(1, LoudspeakerLayout::Ch7_1_4),
        element(2, LoudspeakerLayout::Mono),
    ];
    let err = select(&elements).expect_err("29 channels exceed every profile");
    assert_eq!(err.kind(), &ErrorKind::ChannelCountExceedsProfile);
}

// ---------------------------------------------------------------------------
// The ordering rule only libiamf enforces
// ---------------------------------------------------------------------------

#[test]
fn selection_never_returns_additional_below_primary() {
    for count in 1..=4_u32 {
        let elements = stereo_elements(count);
        let (primary, additional) =
            select(&elements).unwrap_or((Profile::Reserved(255), Profile::Reserved(0)));
        assert!(
            additional >= primary,
            "libiamf's _valid_profile rejects the whole sequence otherwise"
        );
        assert_eq!(primary, additional, "equal is always safe");
    }
}

#[test]
fn a_manually_inverted_profile_pair_is_a_finding_and_a_write_rejection() {
    // primary Base (1), additional Simple (0): iamf-tools does not check this
    // at all and libiamf rejects the entire sequence, so it passes CONF-06 and
    // fails CONF-05.
    let header = IaSequenceHeader::new(Profile::Base.to_wire(), Profile::Simple.to_wire());
    let findings = header.validate();
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("below primary_profile")),
        "validate() names the inversion: {findings:?}"
    );

    let mut w = iamf::bits::BitWriter::new();
    let written = iamf::obu::write_ia_sequence_header(&mut w, &header);
    assert_eq!(
        written.map_err(|e| e.kind().clone()),
        Err(ErrorKind::AdditionalProfileBelowPrimary)
    );
}

#[test]
fn channel_counts_are_summed_across_elements_with_checked_addition() {
    // T-01-38. Twenty-eight 16-channel elements total 448 — far over every
    // ceiling while remaining within the Audio Element count ceiling.
    // A wrapping `u8` or `u16` sum could land back inside a lower profile,
    // which is the failure mode where the header is accepted and the content
    // is then mis-handled. `checked_add` on a `u32` cannot.
    let elements: Vec<AudioElement> = (0..28)
        .map(|id| {
            element(
                id,
                LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6),
            )
        })
        .collect();
    let err = select(&elements).expect_err("448 channels exceed every profile");
    assert_eq!(err.kind(), &ErrorKind::ChannelCountExceedsProfile);
}

#[test]
fn a_layout_with_no_fixed_channel_count_is_a_typed_error_not_a_guess() {
    // A reserved loudspeaker_layout has no channel labels defined, so there is
    // no count to sum. Guessing one would select a profile from nothing.
    let elements = vec![element(0, LoudspeakerLayout::Reserved(11))];
    let err = select(&elements).expect_err("a reserved layout has no channel count");
    assert_eq!(err.kind(), &ErrorKind::UnsupportedLayout);
}

// ---------------------------------------------------------------------------
// PROF-02 — which limits span the IA Sequence (quick 260913-qk3)
// ---------------------------------------------------------------------------

/// A channel-based Audio Element with two layers: Stereo, then 5.1. It
/// contributes the last layer's six channels.
fn two_layer_element(id: u32) -> AudioElement {
    AudioElement::channel_based(
        id,
        200,
        (0..6).collect(),
        ScalableChannelLayoutConfig {
            reserved: 0,
            layers: vec![
                ChannelAudioLayerConfig::new(LoudspeakerLayout::Stereo, 1, 1),
                ChannelAudioLayerConfig::new(LoudspeakerLayout::Ch5_1, 3, 1),
            ],
        },
    )
}

/// A one-sub-mix Mix Presentation referencing `element_ids` in order.
fn presentation_referencing(element_ids: &[u32]) -> MixPresentation {
    MixPresentation {
        mix_presentation_id: 7,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![b"mix".to_vec()],
        sub_mixes: vec![SubMix {
            elements: element_ids
                .iter()
                .map(|&audio_element_id| SubMixAudioElement {
                    audio_element_id,
                    localized_element_annotations: vec![b"bed".to_vec()],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: MixGainParamDefinition::mode_1(100, 16_000),
                })
                .collect(),
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

#[test]
fn two_multi_layer_channel_elements_select_base_enhanced() {
    // Two elements, 12 channels: inside Base's counts, but Base permits at most
    // one channel-based element with num_layers > 1.
    // ref: IAMF v1.0.0-errata index.bs:1867
    let elements = vec![two_layer_element(0), two_layer_element(1)];
    assert_eq!(
        select(&elements).unwrap(),
        (Profile::BaseEnhanced, Profile::BaseEnhanced)
    );
}

#[test]
fn one_multi_layer_channel_element_plus_ambisonics_selects_base() {
    let elements = vec![two_layer_element(1), ambisonics_element(4).unwrap()];
    assert_eq!(select(&elements).unwrap(), (Profile::Base, Profile::Base));
}

#[test]
fn two_ambisonics_elements_with_distinct_ids_select_base_enhanced() {
    // ref: IAMF v1.0.0-errata index.bs:1868 (Base: at most one Scene-based Audio Element)
    let first = ambisonics_element(4).unwrap();
    let mut second = ambisonics_element(4).unwrap();
    second.audio_element_id = 1;
    assert!(second.audio_element_type.scene_based_config().is_some());
    assert_eq!(
        select(&[first, second]).unwrap(),
        (Profile::BaseEnhanced, Profile::BaseEnhanced)
    );
}

#[test]
fn select_sequence_profile_counts_unique_elements_across_presentations() {
    let first = element(1, LoudspeakerLayout::Stereo);
    let second = element(2, LoudspeakerLayout::Stereo);
    let elements = vec![&first, &second];
    let (one, two) = (
        presentation_referencing(&[1]),
        presentation_referencing(&[2]),
    );
    assert_eq!(
        select_sequence_profile(&elements, &[&one, &two]).unwrap(),
        (Profile::Base, Profile::Base)
    );

    let lone = vec![&first];
    let again = presentation_referencing(&[1]);
    assert_eq!(
        select_sequence_profile(&lone, &[&one, &again]).unwrap(),
        (Profile::Simple, Profile::Simple)
    );
}

#[test]
fn presentation_minimum_profile_rejects_what_no_profile_permits() {
    let stereo = element(1, LoudspeakerLayout::Stereo);
    let elements = vec![&stereo];

    let mut none = presentation_referencing(&[1]);
    none.sub_mixes.clear();
    let error = presentation_minimum_profile(&none, &elements).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::SubMixCountNotOne);
    assert_eq!(error.at(), iamf::Location::Field("num_sub_mixes"));

    let mut two = presentation_referencing(&[1]);
    two.sub_mixes
        .extend(presentation_referencing(&[1]).sub_mixes);
    let error = presentation_minimum_profile(&two, &elements).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::SubMixCountNotOne);
    assert_eq!(error.at(), iamf::Location::Field("num_sub_mixes"));

    let mut reserved = presentation_referencing(&[1]);
    reserved
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .unwrap()
        .rendering_config
        .headphones_rendering_mode = HeadphonesRenderingMode::Reserved(2);
    let error = presentation_minimum_profile(&reserved, &elements).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::ReservedHeadphonesRenderingMode);
    assert_eq!(
        error.at(),
        iamf::Location::Field("headphones_rendering_mode")
    );

    let absent = presentation_referencing(&[9]);
    let error = presentation_minimum_profile(&absent, &elements).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::InvalidDescriptorReference);
    assert_eq!(error.at(), iamf::Location::Field("audio_element_id"));

    assert_eq!(
        presentation_minimum_profile(&presentation_referencing(&[1]), &elements).unwrap(),
        Profile::Simple
    );
}

// ---------------------------------------------------------------------------
// PROF-03 — the Q7.8 loudness helper, and the D-21 escape census
// ---------------------------------------------------------------------------

/// The float-typed half of PROF-03's contract.
///
/// `clippy.toml` bans `f64` crate-wide, and the ban reaches test targets too —
/// `cargo clippy --all-targets` is the gate. Testing a function whose whole
/// purpose is to accept an `f64` therefore needs an escape, and it is confined
/// to this module so that `tests/` has exactly one, mirroring the one `src/`
/// has. The D-21 census counts escapes in `src/` only, so this does not touch
/// it; the census test below re-proves that.
#[allow(
    clippy::disallowed_types,
    reason = "PROF-03's helper takes an f64 by definition; testing it requires \
              naming the type. Scoped to this module so tests/ has exactly one \
              escape, as src/ does."
)]
mod quantisation {
    use super::{ErrorKind, Loudness, Q7_8, lufs_to_q7_8};

    /// `lufs_to_q7_8`, or the raw `i16` a failure should not have produced.
    fn q7_8(lufs: f64) -> Result<i16, ErrorKind> {
        lufs_to_q7_8(lufs)
            .map(Q7_8::to_i16)
            .map_err(|e| e.kind().clone())
    }

    #[test]
    fn the_two_published_loudness_values_of_test_000003_round_trip() {
        // integrated_loudness: -13733, and -13733 / 256 == -53.64453125 exactly.
        assert_eq!(q7_8(-53.644_531_25), Ok(-13733));
        // digital_peak: -12879, and -12879 / 256 == -50.30859375 exactly.
        assert_eq!(q7_8(-50.308_593_75), Ok(-12879));
    }

    #[test]
    fn whole_numbers_scale_by_two_hundred_and_fifty_six() {
        assert_eq!(q7_8(0.0), Ok(0));
        assert_eq!(q7_8(1.0), Ok(256));
        assert_eq!(q7_8(-1.0), Ok(-256));
    }

    #[test]
    fn ties_round_to_even_in_both_directions() {
        // The half-LSB inputs, expressed as the fractions they are so the intent
        // survives: -13733.5 / 256 and -13732.5 / 256.
        assert_eq!(q7_8(-13733.5 / 256.0), Ok(-13734), "-13733.5 -> even");
        assert_eq!(q7_8(-13732.5 / 256.0), Ok(-13732), "-13732.5 -> even");
        // And on the positive side, so "ties to even" is not confused with
        // "ties away from zero" by coincidence of sign.
        assert_eq!(q7_8(13733.5 / 256.0), Ok(13734));
        assert_eq!(q7_8(13732.5 / 256.0), Ok(13732));
    }

    #[test]
    fn truncation_toward_zero_would_bias_every_negative_value_upward() {
        // The specific defect PROF-03 names. `as i16` on -13733.5 yields -13733,
        // one LSB *louder* than the input — and every loudness value in a real
        // file is negative, so the error is systematic rather than a wobble.
        assert_ne!(q7_8(-13733.5 / 256.0), Ok(-13733));
        assert_eq!(q7_8(-13733.5 / 256.0), Ok(-13734));
    }

    #[test]
    fn nan_and_both_infinities_are_typed_errors() {
        assert_eq!(q7_8(f64::NAN), Err(ErrorKind::LoudnessOutOfRange));
        assert_eq!(q7_8(f64::INFINITY), Err(ErrorKind::LoudnessOutOfRange));
        assert_eq!(q7_8(f64::NEG_INFINITY), Err(ErrorKind::LoudnessOutOfRange));
    }

    #[test]
    fn values_outside_the_q7_8_range_are_errors_not_saturations() {
        // i16::MAX / 256 == 127.99609375; i16::MIN / 256 == -128.0.
        assert_eq!(q7_8(127.996_093_75), Ok(i16::MAX));
        assert_eq!(q7_8(-128.0), Ok(i16::MIN));

        assert_eq!(q7_8(128.0), Err(ErrorKind::LoudnessOutOfRange));
        assert_eq!(q7_8(-128.005), Err(ErrorKind::LoudnessOutOfRange));
        assert_eq!(q7_8(1.0e30), Err(ErrorKind::LoudnessOutOfRange));
        assert_eq!(q7_8(-1.0e30), Err(ErrorKind::LoudnessOutOfRange));
    }

    #[test]
    fn the_quantised_pair_reaches_the_loudness_block_without_a_float() {
        // The join between the float→fixed path and the wire model. Any call site
        // scaling by 256 and casting for itself would be a second rounding rule.
        let integrated = lufs_to_q7_8(-53.644_531_25).unwrap_or(Q7_8::from_raw(0));
        let digital_peak = lufs_to_q7_8(-50.308_593_75).unwrap_or(Q7_8::from_raw(0));
        let loudness = Loudness::from_q7_8(integrated, digital_peak);

        assert_eq!(loudness, Loudness::new(-13733, -12879));
        assert_eq!(loudness.info_type(), 0);
    }
}

// ---------------------------------------------------------------------------
// D-21 — the census. Its correct answer is exactly one.
// ---------------------------------------------------------------------------

/// Every `.rs` file under `src/`.
fn rust_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(rust_files_under(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[test]
fn exactly_one_float_lint_escape_exists_in_src_and_it_is_in_loudness_rs() {
    let mut escapes: Vec<String> = Vec::new();

    for file in rust_files_under(Path::new("src")) {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            // Comment lines are filtered so that a doc comment *describing* the
            // attribute — of which this crate has several, deliberately —
            // cannot self-invalidate the count.
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains("disallowed_types") {
                escapes.push(format!("{}:{}", file.display(), index.saturating_add(1)));
            }
        }
    }

    assert_eq!(
        escapes.len(),
        1,
        "D-21 makes the no-DSP guard a census whose correct answer is exactly \
         one: 0 means PROF-03's escape was lost, 2 or more means the scope \
         boundary was breached. Found: {escapes:?}"
    );
    assert!(
        escapes.first().is_some_and(|e| e.contains("loudness.rs")),
        "the one escape belongs to PROF-03's helper: {escapes:?}"
    );
}

#[test]
fn no_file_under_src_but_loudness_rs_mentions_a_64_bit_float() {
    let offenders: Vec<String> = rust_files_under(Path::new("src"))
        .into_iter()
        .filter(|file| fs::read_to_string(file).is_ok_and(|text| text.contains("f64")))
        .map(|file| file.display().to_string())
        .collect();

    assert_eq!(
        offenders.len(),
        1,
        "a float reached the encode path outside the one sanctioned function: \
         {offenders:?}"
    );
    assert!(
        offenders
            .first()
            .is_some_and(|f| f.ends_with("loudness.rs")),
        "{offenders:?}"
    );
}
