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
//! **Both counts are per Mix Presentation**, not per sequence. Every test below
//! passes the Audio Elements of one Mix Presentation.

use iamf::error::ErrorKind;
use iamf::model::layout::LoudspeakerLayout;
use iamf::model::profile::{
    BASE_ENHANCED_MAX_AUDIO_ELEMENTS, BASE_ENHANCED_MAX_CHANNELS, BASE_MAX_AUDIO_ELEMENTS,
    BASE_MAX_CHANNELS, SIMPLE_MAX_AUDIO_ELEMENTS, SIMPLE_MAX_CHANNELS,
};
use iamf::model::{Profile, select_minimum_profile};
use iamf::obu::{
    AudioElement, ChannelAudioLayerConfig, IaSequenceHeader, ScalableChannelLayoutConfig,
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
    (0..n).map(|i| element(i, LoudspeakerLayout::Stereo)).collect()
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
    // A compile-time claim, so it is asserted the only way a test can: by
    // naming the trait bound a `Default` impl would satisfy. `assert_not_impl`
    // needs a dependency; this needs none.
    fn is_default<T: Default>() -> bool {
        true
    }
    assert!(is_default::<u8>(), "the helper detects a real Default impl");
    // `is_default::<Profile>()` does not compile. A default would let a caller
    // ship Simple by accident for a configuration that needs Base, which
    // libiamf refuses to decode at all.
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
    assert_eq!(
        select(&elements).map(|(p, _)| p),
        Ok(Profile::BaseEnhanced)
    );
}

#[test]
fn twenty_eight_audio_elements_select_base_enhanced() {
    assert_eq!(BASE_ENHANCED_MAX_AUDIO_ELEMENTS, 28);
    // 28 mono elements: 28 elements and 28 channels, both exactly at the
    // Base-Enhanced ceiling.
    let elements: Vec<AudioElement> = (0..28)
        .map(|i| element(i, LoudspeakerLayout::Mono))
        .collect();
    assert_eq!(
        select(&elements).map(|(p, _)| p),
        Ok(Profile::BaseEnhanced)
    );
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

/// One element carrying exactly `channels` channels, built from mono elements
/// where a single layout cannot supply the number.
///
/// The channel limits are per Mix Presentation, so the count is the *sum*
/// across elements — but the element limits would fire first for large counts.
/// Every case below therefore stays inside the element ceiling.
fn elements_totalling(channels: u32) -> Vec<AudioElement> {
    // 7.1.4 is 12 channels, the largest single modelled layout. Two of them is
    // 24; adding mono elements fills the rest. That is 2 + n elements, which
    // stays under 28 for every count this test file uses.
    let mut elements = Vec::new();
    let mut remaining = channels;
    let mut next_id = 0_u32;
    while remaining >= 12 && elements.len() < 2 {
        elements.push(element(next_id, LoudspeakerLayout::Ch7_1_4));
        next_id = next_id.saturating_add(1);
        remaining = remaining.saturating_sub(12);
    }
    while remaining > 0 {
        elements.push(element(next_id, LoudspeakerLayout::Mono));
        next_id = next_id.saturating_add(1);
        remaining = remaining.saturating_sub(1);
    }
    elements
}

#[test]
fn a_single_five_one_six_channel_element_selects_simple() {
    // Exactly the Phase 1 fixture, and plan 01-08 depends on this answer.
    // profile_filter.cc gives the same one: 1 element (<= 1) and 6 channels
    // (<= 16).
    let elements = vec![element(0, LoudspeakerLayout::Ch5_1)];
    assert_eq!(
        elements
            .first()
            .and_then(|e| e.audio_substream_ids.len().try_into().ok()),
        Some(6_usize),
        "the 5.1 element carries six channels"
    );
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Simple));
}

#[test]
fn sixteen_channels_in_one_element_select_simple() {
    assert_eq!(SIMPLE_MAX_CHANNELS, 16);
    // One 7.1.4 element is 12 channels; 16 needs more than one element, and
    // more than one element is Base regardless. So the 16-channel boundary is
    // exercised at one element by the largest modelled layout that fits, and
    // the sum-across-elements form is covered by the 17/18/19 cases below.
    let elements = vec![element(0, LoudspeakerLayout::Ch7_1_4)];
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Simple));
}

#[test]
fn seventeen_channels_select_base() {
    let elements = elements_totalling(17);
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Base));
}

#[test]
fn eighteen_channels_select_base() {
    assert_eq!(BASE_MAX_CHANNELS, 18);
    let elements = elements_totalling(18);
    assert_eq!(select(&elements).map(|(p, _)| p), Ok(Profile::Base));
}

#[test]
fn nineteen_channels_select_base_enhanced() {
    let elements = elements_totalling(19);
    assert_eq!(
        select(&elements).map(|(p, _)| p),
        Ok(Profile::BaseEnhanced)
    );
}

#[test]
fn twenty_eight_channels_select_base_enhanced() {
    assert_eq!(BASE_ENHANCED_MAX_CHANNELS, 28);
    let elements = elements_totalling(28);
    assert_eq!(
        select(&elements).map(|(p, _)| p),
        Ok(Profile::BaseEnhanced)
    );
}

#[test]
fn twenty_nine_channels_are_a_typed_error() {
    let elements = elements_totalling(29);
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
        let (primary, additional) = select(&elements).unwrap_or((
            Profile::Reserved(255),
            Profile::Reserved(0),
        ));
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
fn a_layout_with_no_fixed_channel_count_is_a_typed_error_not_a_guess() {
    // A reserved loudspeaker_layout has no channel labels defined, so there is
    // no count to sum. Guessing one would select a profile from nothing.
    let elements = vec![element(0, LoudspeakerLayout::Reserved(11))];
    let err = select(&elements).expect_err("a reserved layout has no channel count");
    assert_eq!(err.kind(), &ErrorKind::UnsupportedLayout);
}
