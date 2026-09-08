//! PROF-01 and PROF-02 — the profile enum, and minimum-profile selection.
//!
//! Two profile bytes sit in the IA Sequence Header and decide whether a
//! decoder will play the sequence at all. This module owns which values exist,
//! which one a configuration needs, and the ordering rule between the pair.
//!
//! # The rule only the decoder enforces
//!
//! `libiamf`'s `_valid_profile` requires `primary <= additional` and rejects
//! the **whole sequence** otherwise; `iamf-tools` does not validate
//! `additional_profile` at all. That is the reverse of the usual strictness
//! asymmetry, and it means a file carrying the violation is written happily by
//! the reference encoder and then refused wholesale by the reference decoder —
//! it passes CONF-06 and fails CONF-05.
//!
//! `PITFALLS.md` §11 advises "do not be stricter than the reference on
//! `additional_profile`". **That advice is misleading here.** `libiamf` *is*
//! the stricter of the two references, and `libiamf`'s acceptance is this
//! project's Core Value. Emitting both profiles equal is always safe, and
//! [`select_minimum_profile`] never returns a pair that could trip the rule.

use crate::error::{Error, ErrorKind, Location, Result};
use crate::obu::{AudioElement, AudioElementType};

/// A profile byte, as `primary_profile` or `additional_profile` carries it.
///
/// The legal range follows `crate::SPEC_VERSION`, which is `"1.1.0"`: three
/// named profiles and nothing else.
///
/// **`iamf-tools@main` adds `kIamfBaseAdvancedProfile = 3`,
/// `kIamfAdvanced1Profile = 4` and `kIamfAdvanced2Profile = 5`, and they are
/// deliberately absent here.** That tree is a draft v2.0.0 development tip, not
/// the version this crate targets — the same class of drift plan 01-04
/// corrected for the meaning of bit 6 in the OBU header. Mirroring the tip
/// would let this crate emit a profile byte the pinned `libiamf@v1.1.0`
/// rejects.
///
/// There is **no `Default`**, on purpose: a configuration either names a
/// profile or goes through [`select_minimum_profile`]. A default would let a
/// caller ship Simple by accident for a configuration that needs Base, which
/// `libiamf` refuses to decode at all.
// ref: iamf-tools@v2.1.0 iamf/obu/ia_sequence_header.h ProfileVersion
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_types.h IAMF_PROFILE_COUNT
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
    /// `kIamfSimpleProfile = 0`.
    Simple,
    /// `kIamfBaseProfile = 1`.
    Base,
    /// `kIamfBaseEnhancedProfile = 2`. Does not exist in IAMF v1.0 at all,
    /// which is one reason `SPEC_VERSION` is 1.1.0.
    BaseEnhanced,
    /// `3..=255`, including the reference's own `kIamfReserved255Profile`.
    ///
    /// Reserved values are **accepted, not rejected**: a parser that refused
    /// one could not round-trip a foreign file, and Phase 2's
    /// `serialize(parse(bytes)) == bytes` property is what proves this crate
    /// preserved something it did not understand.
    Reserved(u8),
}

impl Profile {
    /// The profile a wire byte carries. Total: no input panics, none is
    /// rejected.
    #[must_use]
    pub const fn from_wire(value: u8) -> Self {
        match value {
            0 => Self::Simple,
            1 => Self::Base,
            2 => Self::BaseEnhanced,
            other => Self::Reserved(other),
        }
    }

    /// The byte this profile occupies on the wire.
    #[must_use]
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Simple => 0,
            Self::Base => 1,
            Self::BaseEnhanced => 2,
            Self::Reserved(raw) => raw,
        }
    }
}

// The limits, quoted rather than inferred.
//
// ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:34-40
//   constexpr int kSimpleProfileMaxAudioElements = 1;
//   constexpr int kBaseProfileMaxAudioElements = 2;
//   constexpr int kBaseEnhancedProfileMaxAudioElements = 28;
//   constexpr int kSimpleProfileMaxChannels = 16;
//   constexpr int kBaseProfileMaxChannels = 18;
//   constexpr int kBaseEnhancedProfileMaxChannels = 28;

/// `kSimpleProfileMaxAudioElements`.
pub const SIMPLE_MAX_AUDIO_ELEMENTS: usize = 1;
/// `kBaseProfileMaxAudioElements`.
pub const BASE_MAX_AUDIO_ELEMENTS: usize = 2;
/// `kBaseEnhancedProfileMaxAudioElements`.
pub const BASE_ENHANCED_MAX_AUDIO_ELEMENTS: usize = 28;
/// `kSimpleProfileMaxChannels`.
pub const SIMPLE_MAX_CHANNELS: u32 = 16;
/// `kBaseProfileMaxChannels`.
pub const BASE_MAX_CHANNELS: u32 = 18;
/// `kBaseEnhancedProfileMaxChannels`.
pub const BASE_ENHANCED_MAX_CHANNELS: u32 = 28;

/// PROF-02 — the lowest profile that permits this configuration, as the
/// `(primary, additional)` pair the IA Sequence Header carries.
///
/// **Both counts are per Mix Presentation, not per sequence.** `elements` is
/// the Audio Elements of *one* Mix Presentation's sub-mixes. Summing across a
/// whole multi-presentation sequence is the obvious wrong reading and would
/// select a higher profile than the file needs — possibly one the decoder in
/// front of it does not implement.
///
/// The pair is always returned **equal**. See the module documentation: only
/// `libiamf` enforces `primary <= additional`, it rejects the entire sequence
/// when the rule is broken, and equal profiles can never break it.
///
/// # Errors
///
/// [`ErrorKind::ElementCountExceedsProfile`] above 28 Audio Elements and
/// [`ErrorKind::ChannelCountExceedsProfile`] above 28 channels — the
/// Base-Enhanced ceilings, above which no profile in this spec version
/// permits the configuration.
// ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc ProfileFilter::FilterProfilesForAudioElement
pub fn select_minimum_profile(elements: &[&AudioElement]) -> Result<(Profile, Profile)> {
    let element_count = elements.len();
    let channel_count = total_channel_count(elements)?;

    if element_count > BASE_ENHANCED_MAX_AUDIO_ELEMENTS {
        return Err(Error::new(
            ErrorKind::ElementCountExceedsProfile,
            Location::Field("num_audio_elements"),
        ));
    }
    if channel_count > BASE_ENHANCED_MAX_CHANNELS {
        return Err(Error::new(
            ErrorKind::ChannelCountExceedsProfile,
            Location::Field("loudspeaker_layout"),
        ));
    }

    // STUB(GREEN): every configuration answers Simple.
    let _ = (element_count, channel_count);
    let profile = Profile::Simple;

    // Equal, always. The ordering rule can then never be tripped.
    Ok((profile, profile))
}

/// The channels the Audio Elements of one Mix Presentation carry, summed with
/// `checked_add`.
///
/// T-01-38: a wrapping sum would select a *lower* profile than the
/// configuration needs, which is the failure mode where a decoder accepts the
/// header and then mis-handles the content.
fn total_channel_count(elements: &[&AudioElement]) -> Result<u32> {
    let mut total: u32 = 0;
    for element in elements {
        total = total
            .checked_add(element_channel_count(element)?)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::ChannelCountExceedsProfile,
                    Location::Field("loudspeaker_layout"),
                )
            })?;
    }
    Ok(total)
}

/// The channels one Audio Element contributes to its Mix Presentation.
///
/// For a scalable channel layout that is the **highest** layer's count — the
/// lower layers are downmixes of it, not additional channels, so summing the
/// layers would multiply the element's contribution by its layer count and
/// select far too high a profile.
///
/// # Errors
///
/// [`ErrorKind::UnsupportedLayout`] for a layout whose channel count this spec
/// version does not fix (reserved and expanded layouts), and for a reserved
/// `audio_element_type`. A guess there would select a profile from nothing.
fn element_channel_count(element: &AudioElement) -> Result<u32> {
    let unsupported = || {
        Error::new(
            ErrorKind::UnsupportedLayout,
            Location::Field("loudspeaker_layout"),
        )
    };
    match &element.audio_element_type {
        AudioElementType::ChannelBased(config) => config
            .scalable_channel_layout
            .layers
            .last()
            .ok_or_else(unsupported)?
            .loudspeaker_layout
            .channel_count()
            .ok_or_else(unsupported),
        AudioElementType::SceneBased(config) => config
            .output_channel_count()
            .map(u32::from)
            .ok_or_else(unsupported),
        AudioElementType::Reserved { .. } => Err(unsupported()),
    }
}
