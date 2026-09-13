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
//!
//! # Expanded loudspeaker layouts need Base-Enhanced
//!
//! `loudspeaker_layout = 15` (expanded) is reserved in IAMF v1.0.0-errata, the
//! version Simple and Base comply with, so a channel-based element whose first
//! layer uses an expanded layout needs Base-Enhanced however few elements and
//! channels it has. The failure is silent: pinned `libiamf` given a Simple
//! header over a lone expanded LFE element exits 0, decodes 0 frames and writes
//! a bare 44-byte WAV. Patching only the two profile bytes to `02 02` makes the
//! same file decode 1 frame.
//!
//! # Which limits span the IA Sequence
//!
//! IAMF v1.1.0 hands the Simple and Base profiles to v1.0.0-errata
//! (`index.bs:1936`, `:1943`), and the errata scope each limit differently:
//!
//! | #  | Rule                                                    | Scope             | Source                          |
//! |----|---------------------------------------------------------|-------------------|---------------------------------|
//! | R1 | Simple: at most 1 unique Audio Element                  | IA Sequence       | errata `:1853`                  |
//! | R2 | Simple: at most 16 channels                             | Mix Presentation  | errata `:1857`, v1.1.0 `:1927`  |
//! | R3 | Base: at most 2 unique Audio Elements                   | IA Sequence       | errata `:1866`                  |
//! | R4 | Base: at most 1 scene-based Audio Element               | IA Sequence       | errata `:1868`                  |
//! | R5 | Base: at most 1 channel-based element, `num_layers > 1` | IA Sequence       | errata `:1867`                  |
//! | R6 | Base: at most 18 channels                               | Mix Presentation  | errata `:1878-1879`             |
//! | R7 | Base-Enhanced: at most 28 Audio Elements                | Mix Presentation  | v1.1.0 `:1921`, `:1951`         |
//! | R8 | Base-Enhanced: at most 28 channels in total             | IA Sequence       | v1.1.0 `:1953`                  |
//!
//! Unique means distinct `audio_element_id`: an OBU that only varies by
//! `obu_redundant_copy` is the same OBU (v1.1.0 `:1906`). Channels are counted
//! as `libiamf` counts them — the last layer's layout, or a scene-based
//! element's `output_channel_count` (`IAMF_decoder.c:1284-1337`).
//!
//! Both pinned references count every limit per Mix Presentation and have no
//! R4/R5 rule. The spec is stricter, and a file this crate writes must satisfy
//! both, so the spec's scopes are applied. The profile is raised to satisfy
//! them; a configuration is rejected only above the Base-Enhanced ceilings.

use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::layout::LoudspeakerLayout;
use crate::obu::{AudioElement, AudioElementType, HeadphonesRenderingMode, MixPresentation};

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
///
/// ```compile_fail
/// // There is no Default impl, so this does not compile.
/// let _profile: iamf::model::Profile = Default::default();
/// ```
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

/// PROF-02 — the lowest profile that permits this set of Audio Elements, as
/// the `(primary, additional)` pair the IA Sequence Header carries.
///
/// `elements` is read as **both** one Mix Presentation's references and the
/// whole IA Sequence's Audio Elements, and the higher requirement wins. See
/// "Which limits span the IA Sequence" in the module documentation: the
/// unique-element limits, Base's scene-based and multi-layer limits and the
/// Base-Enhanced channel total span the sequence; the Simple and Base channel
/// limits and the Base-Enhanced element limit are per Mix Presentation. For a
/// multi-presentation sequence use [`select_sequence_profile`], which applies
/// each scope to the right set.
///
/// The pair is always returned **equal**. See the module documentation: only
/// `libiamf` enforces `primary <= additional`, it rejects the entire sequence
/// when the rule is broken, and equal profiles can never break it.
///
/// An element whose **first** layer is an expanded loudspeaker layout needs
/// Base-Enhanced whatever the counts: `loudspeaker_layout = 15` is reserved in
/// IAMF v1.0.0-errata, which Simple and Base comply with, and pinned `libiamf`
/// silently decodes 0 frames from a Simple or Base header carrying one. The
/// Base-Enhanced ceilings are still checked first, so an over-ceiling
/// configuration stays an error rather than becoming Base-Enhanced.
///
/// # Errors
///
/// [`ErrorKind::ElementCountExceedsProfile`] above 28 Audio Elements and
/// [`ErrorKind::ChannelCountExceedsProfile`] above 28 channels — the
/// Base-Enhanced ceilings, above which no profile in this spec version
/// permits the configuration. [`ErrorKind::UnsupportedLayout`] when a channel
/// count is not fixed by this spec version.
// ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc ProfileFilter::FilterProfilesForAudioElement
pub fn select_minimum_profile(elements: &[&AudioElement]) -> Result<(Profile, Profile)> {
    let presentation = presentation_floor(elements)?;
    let sequence = sequence_floor(elements)?;
    let profile = presentation.max(sequence);
    // Equal, always. The ordering rule can then never be tripped.
    Ok((profile, profile))
}

/// The lowest profile one Mix Presentation complies with, on its own.
///
/// `elements` are the IA Sequence's Audio Elements; each sub-mix reference
/// resolves to the first element carrying its `audio_element_id`. Only the
/// per-presentation limits apply here (R2, R6, R7, plus R4/R5 and the
/// expanded-layout floor over the presentation's own references); the
/// sequence-wide limits are [`select_sequence_profile`]'s.
///
/// # Errors
///
/// - [`ErrorKind::SubMixCountNotOne`] at `num_sub_mixes` when the presentation
///   does not carry exactly one sub-mix.
/// - [`ErrorKind::ReservedHeadphonesRenderingMode`] at
///   `headphones_rendering_mode` for a reserved mode.
/// - [`ErrorKind::InvalidDescriptorReference`] at `audio_element_id` for a
///   reference no element carries.
/// - The ceiling and channel-count errors of [`select_minimum_profile`].
// ref: IAMF v1.1.0 index.bs:1278, :1920 (num_sub_mixes), :1337-1341 (reserved headphones_rendering_mode)
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:760-782; iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:220-271
pub fn presentation_minimum_profile(
    presentation: &MixPresentation,
    elements: &[&AudioElement],
) -> Result<Profile> {
    if presentation.sub_mixes.len() != 1 {
        return Err(Error::new(
            ErrorKind::SubMixCountNotOne,
            Location::Field("num_sub_mixes"),
        ));
    }
    let references = || {
        presentation
            .sub_mixes
            .iter()
            .flat_map(|sub_mix| &sub_mix.elements)
    };
    if references().any(|reference| {
        matches!(
            reference.rendering_config.headphones_rendering_mode,
            HeadphonesRenderingMode::Reserved(_)
        )
    }) {
        return Err(Error::new(
            ErrorKind::ReservedHeadphonesRenderingMode,
            Location::Field("headphones_rendering_mode"),
        ));
    }
    let resolved = references()
        .map(|reference| {
            elements
                .iter()
                .copied()
                .find(|element| element.audio_element_id == reference.audio_element_id)
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidDescriptorReference,
                        Location::Field("audio_element_id"),
                    )
                })
        })
        .collect::<Result<Vec<&AudioElement>>>()?;
    presentation_floor(&resolved)
}

/// PROF-02 for a whole IA Sequence: the sequence-wide floor over every Audio
/// Element (referenced or not), raised by each Mix Presentation's own floor,
/// as the equal `(primary, additional)` pair.
///
/// The sequence ceilings run first, then each presentation in list order.
///
/// # Errors
///
/// Those of [`presentation_minimum_profile`], plus
/// [`ErrorKind::ElementCountExceedsProfile`] above 28 unique Audio Elements and
/// [`ErrorKind::ChannelCountExceedsProfile`] above 28 channels summed over the
/// unique Audio Elements of the sequence.
// ref: IAMF v1.0.0-errata index.bs:1852-1853, :1865-1873, :1878-1879 (adopted by v1.1.0 index.bs:1936, :1943)
// ref: IAMF v1.1.0 index.bs:1906, :1927, :1951, :1953
pub fn select_sequence_profile(
    elements: &[&AudioElement],
    presentations: &[&MixPresentation],
) -> Result<(Profile, Profile)> {
    let mut profile = sequence_floor(elements)?;
    for presentation in presentations {
        profile = profile.max(presentation_minimum_profile(presentation, elements)?);
    }
    Ok((profile, profile))
}

/// Whether an element's first layer is an expanded loudspeaker layout.
///
/// It reads the FIRST layer, as the reference does
/// (`channel_audio_layer_configs[0]`, libiamf `layer[0]`), while channel
/// counting keeps the last.
fn is_expanded_first_layer(element: &AudioElement) -> bool {
    matches!(
        &element.audio_element_type,
        AudioElementType::ChannelBased(config)
            if config
                .scalable_channel_layout
                .layers
                .first()
                .is_some_and(|layer| matches!(
                    layer.loudspeaker_layout,
                    LoudspeakerLayout::Expanded(_)
                ))
    )
}

/// Base's combination rules: at most one scene-based Audio Element, and at
/// most one channel-based Audio Element with `num_layers > 1`.
// ref: IAMF v1.0.0-errata index.bs:1867-1873 (adopted by v1.1.0 index.bs:1943)
// DISAGREEMENT: neither iamf-tools@v2.1.0 iamf/cli/profile_filter.cc nor libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:1284-1337 has a scene-based or num_layers rule; the spec is stricter, so the spec wins (qk3 directive, CONTEXT D-SPEC)
fn base_combination_allowed(elements: &[&AudioElement]) -> bool {
    let scene_based = elements
        .iter()
        .filter(|element| matches!(element.audio_element_type, AudioElementType::SceneBased(_)))
        .count();
    let multi_layer = elements
        .iter()
        .filter(|element| {
            matches!(
                &element.audio_element_type,
                AudioElementType::ChannelBased(config)
                    if config.scalable_channel_layout.layers.len() > 1
            )
        })
        .count();
    scene_based <= 1 && multi_layer <= 1
}

/// The unique Audio Elements: the first binding of each distinct
/// `audio_element_id`, in input order. An OBU that differs only by
/// `obu_redundant_copy` repeats its original's id, so it is not counted again.
// ref: IAMF v1.1.0 index.bs:1906 (a unique OBU is still unique if it only varies by obu_redundant_copy)
fn unique_by_id<'a>(elements: &[&'a AudioElement]) -> Vec<&'a AudioElement> {
    let mut unique: Vec<&'a AudioElement> = Vec::new();
    for element in elements {
        if !unique
            .iter()
            .any(|seen| seen.audio_element_id == element.audio_element_id)
        {
            unique.push(element);
        }
    }
    unique
}

/// The per-presentation floor over one Mix Presentation's references.
/// Duplicated references count, as iamf-tools does.
fn presentation_floor(elements: &[&AudioElement]) -> Result<Profile> {
    let element_count = elements.len();
    let channel_count = total_channel_count(elements)?;

    // ref: IAMF v1.1.0 index.bs:1921, :1951 (Base-Enhanced: at most 28 Audio Elements per Mix Presentation)
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

    // The expanded-layout floor. It reads the FIRST layer, as the reference
    // does (`channel_audio_layer_configs[0]`, libiamf `layer[0]`), while channel
    // counting above keeps the last. It runs after both ceilings so that an
    // over-ceiling configuration stays a typed error rather than becoming
    // Base-Enhanced.
    // ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:165-171 FilterChannelBasedConfig (kLayoutExpanded erases Simple and Base)
    // ref: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc:306-313 (the sequencer refuses a header whose profiles the Mix Presentation does not support)
    // ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:643-648 iamf_element_is_valid (layout 15 under Simple or Base drops the element)
    // ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:1402-1410 (effective profile is min(additional_profile, Base-Enhanced))
    // ref: eclipsa-audio-plugin@c964609 common/data_structures/src/FileExport.h:91-106 minimumProfile
    let has_expanded_first_layer = elements
        .iter()
        .any(|element| is_expanded_first_layer(element));

    // The floor first, then the tiers in ascending order. Without an expanded
    // first layer, a configuration takes the first tier that
    // permits BOTH its element count and its channel count — which is what
    // makes two stereo elements Base even though four channels would fit
    // Simple twice over.
    // ref: IAMF v1.1.0 index.bs:1927 (the channel limit sums a Mix Presentation's Audio Elements)
    // ref: IAMF v1.0.0-errata index.bs:1878-1879 (Base: 18 channels per Mix Presentation)
    Ok(if has_expanded_first_layer {
        Profile::BaseEnhanced
    } else if element_count <= SIMPLE_MAX_AUDIO_ELEMENTS && channel_count <= SIMPLE_MAX_CHANNELS {
        Profile::Simple
    } else if element_count <= BASE_MAX_AUDIO_ELEMENTS
        && channel_count <= BASE_MAX_CHANNELS
        && base_combination_allowed(elements)
    {
        Profile::Base
    } else {
        // Both Base-Enhanced ceilings were checked above, so this arm is
        // reached only when the configuration fits them.
        Profile::BaseEnhanced
    })
}

/// The sequence-wide floor from the unique Audio Elements alone, without any
/// channel test: the Simple and Base channel limits are per Mix Presentation.
fn sequence_count_floor(unique: &[&AudioElement]) -> Profile {
    if unique
        .iter()
        .any(|element| is_expanded_first_layer(element))
    {
        Profile::BaseEnhanced
    } else if unique.len() <= SIMPLE_MAX_AUDIO_ELEMENTS {
        Profile::Simple
    } else if unique.len() <= BASE_MAX_AUDIO_ELEMENTS && base_combination_allowed(unique) {
        Profile::Base
    } else {
        Profile::BaseEnhanced
    }
}

/// The sequence-wide floor over every Audio Element of the IA Sequence.
// ref: IAMF v1.0.0-errata index.bs:1852-1853, :1865-1873 (adopted by v1.1.0 index.bs:1936, :1943)
// ref: IAMF v1.1.0 index.bs:1953 (Base-Enhanced: at most 28 channels in total across the IA Sequence)
// DISAGREEMENT: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:282-362 and libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:1284-1337 count per Mix Presentation and have no scene-based or num_layers rule; the spec is stricter, so the spec wins (qk3 directive, CONTEXT D-SPEC)
fn sequence_floor(elements: &[&AudioElement]) -> Result<Profile> {
    let unique = unique_by_id(elements);
    if unique.len() > BASE_ENHANCED_MAX_AUDIO_ELEMENTS {
        return Err(Error::new(
            ErrorKind::ElementCountExceedsProfile,
            Location::Field("num_audio_elements"),
        ));
    }
    if total_channel_count(&unique)? > BASE_ENHANCED_MAX_CHANNELS {
        return Err(Error::new(
            ErrorKind::ChannelCountExceedsProfile,
            Location::Field("loudspeaker_layout"),
        ));
    }
    Ok(sequence_count_floor(&unique))
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
/// [`ErrorKind::UnsupportedLayout`] for a reserved loudspeaker layout or
/// reserved expanded loudspeaker layout, whose channel count this spec version
/// does not fix, and for a reserved `audio_element_type`. Named expanded
/// layouts have normative channel counts. A guess for a reserved value would
/// select a profile from nothing.
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
