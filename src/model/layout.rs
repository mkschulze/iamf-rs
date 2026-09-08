//! DESC-07's **four distinct layout types**, which live in **two different
//! OBUs** and must never collapse into one flat enum.
//!
//! | Type | Width | OBU | Gate |
//! |---|---|---|---|
//! | [`LoudspeakerLayout`] | 4 bits | Audio Element | — |
//! | [`ExpandedLoudspeakerLayout`] | 8 bits | Audio Element | present **only** when `loudspeaker_layout == 15` |
//! | [`AmbisonicsConfig`] | variable | Audio Element | `audio_element_type == SCENE_BASED` |
//! | [`SoundSystem`] | 4 bits | Mix Presentation | `layout_type == 2` |
//!
//! `PROJECT.md` records a "layouts numbered 0–30 in three bands" claim. That
//! numbering is **Eclipsa's plugin enum**, not an IAMF wire field: it conflates
//! `loudspeaker_layout`, `expanded_loudspeaker_layout` and `sound_system`
//! across two OBUs into one flat index. Reproducing it here would make an
//! Audio Element layout assignable to a Mix Presentation layout field — a
//! clean-decode, wrong-result failure that no round-trip test against
//! ourselves could ever see, because both sides would agree on the same wrong
//! model.
//!
//! The gate in row two is encoded **in the type**: [`LoudspeakerLayout::Expanded`]
//! carries the expanded value inside the variant, so "an expanded layout with a
//! `loudspeaker_layout` other than 15" is unrepresentable rather than checked
//! at run time.
//!
//! # Naming
//!
//! The reference spells these `kSoundSystemA_0_2_0`. Rust's
//! `non_camel_case_types` lint rejects a cased character adjacent to an
//! underscore, so `A_0_2_0` becomes [`SoundSystem::A0_2_0`] — the digits and
//! separators of the reference's own name, with the underscore after the letter
//! dropped. Nothing else is renamed.

/// `loudspeaker_layout` — a **4-bit** field in the **Audio Element**'s
/// `ChannelAudioLayerConfig`.
///
/// Not to be confused with [`SoundSystem`], which is a different 4-bit field
/// with a different value space in a different OBU.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h ChannelAudioLayerConfig::LoudspeakerLayout
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoudspeakerLayout {
    /// `kLayoutMono = 0` — C.
    Mono,
    /// `kLayoutStereo = 1` — L/R.
    Stereo,
    /// `kLayout5_1_ch = 2` — L/C/R/Ls/Rs/LFE.
    Ch5_1,
    /// `kLayout5_1_2_ch = 3`.
    Ch5_1_2,
    /// `kLayout5_1_4_ch = 4`.
    Ch5_1_4,
    /// `kLayout7_1_ch = 5`.
    Ch7_1,
    /// `kLayout7_1_2_ch = 6`.
    Ch7_1_2,
    /// `kLayout7_1_4_ch = 7`.
    Ch7_1_4,
    /// `kLayout3_1_2_ch = 8`.
    Ch3_1_2,
    /// `kLayoutBinaural = 9` — L/R.
    Binaural,
    /// `kLayoutReserved10` … `kLayoutReserved14`.
    Reserved(u8),
    /// `kLayoutExpanded = 15`, carrying the `expanded_loudspeaker_layout` byte
    /// that follows it on the wire.
    ///
    /// The value lives **inside** the variant so that the wire rule "the
    /// expanded byte is present only when the layout is 15" is a property of
    /// the type rather than a run-time check that can be forgotten.
    Expanded(ExpandedLoudspeakerLayout),
}

impl LoudspeakerLayout {
    /// The 4-bit value this layout occupies.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Mono => 0,
            Self::Stereo => 1,
            Self::Ch5_1 => 2,
            Self::Ch5_1_2 => 3,
            Self::Ch5_1_4 => 4,
            Self::Ch7_1 => 5,
            Self::Ch7_1_2 => 6,
            Self::Ch7_1_4 => 7,
            Self::Ch3_1_2 => 8,
            Self::Binaural => 9,
            Self::Reserved(raw) => raw,
            Self::Expanded(_) => 15,
        }
    }

    /// The layout a 4-bit field carries — for every value **except 15**.
    ///
    /// `None` for 15, deliberately: `kLayoutExpanded` is incomplete without the
    /// `expanded_loudspeaker_layout` byte that follows it, so only the reader
    /// (which has that byte) can construct [`Self::Expanded`].
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Mono),
            1 => Some(Self::Stereo),
            2 => Some(Self::Ch5_1),
            3 => Some(Self::Ch5_1_2),
            4 => Some(Self::Ch5_1_4),
            5 => Some(Self::Ch7_1),
            6 => Some(Self::Ch7_1_2),
            7 => Some(Self::Ch7_1_4),
            8 => Some(Self::Ch3_1_2),
            9 => Some(Self::Binaural),
            10..=14 => Some(Self::Reserved(value)),
            _ => None,
        }
    }

    /// The `expanded_loudspeaker_layout` byte this layout carries, if any.
    #[must_use]
    pub const fn expanded(self) -> Option<ExpandedLoudspeakerLayout> {
        match self {
            Self::Expanded(expanded) => Some(expanded),
            _ => None,
        }
    }
}

/// `expanded_loudspeaker_layout` — an **8-bit** field in the **Audio Element**,
/// present on the wire **only** when `loudspeaker_layout == 15`.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h ChannelAudioLayerConfig::ExpandedLoudspeakerLayout
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandedLoudspeakerLayout {
    /// `kExpandedLayoutLFE = 0`.
    Lfe,
    /// `kExpandedLayoutStereoS = 1`.
    StereoS,
    /// `kExpandedLayoutStereoSS = 2`.
    StereoSs,
    /// `kExpandedLayoutStereoRS = 3`.
    StereoRs,
    /// `kExpandedLayoutStereoTF = 4`.
    StereoTf,
    /// `kExpandedLayoutStereoTB = 5`.
    StereoTb,
    /// `kExpandedLayoutTop4Ch = 6`.
    Top4Ch,
    /// `kExpandedLayout3_0_ch = 7`.
    Ch3_0,
    /// `kExpandedLayout9_1_6_ch = 8`.
    Ch9_1_6,
    /// `kExpandedLayoutStereoF = 9`.
    StereoF,
    /// `kExpandedLayoutStereoSi = 10`.
    StereoSi,
    /// `kExpandedLayoutStereoTpSi = 11`.
    StereoTpSi,
    /// `kExpandedLayoutTop6Ch = 12`.
    Top6Ch,
    /// `kExpandedLayoutReserved13` … `kExpandedLayoutReserved255`.
    Reserved(u8),
}

impl ExpandedLoudspeakerLayout {
    /// The byte this layout occupies.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Lfe => 0,
            Self::StereoS => 1,
            Self::StereoSs => 2,
            Self::StereoRs => 3,
            Self::StereoTf => 4,
            Self::StereoTb => 5,
            Self::Top4Ch => 6,
            Self::Ch3_0 => 7,
            Self::Ch9_1_6 => 8,
            Self::StereoF => 9,
            Self::StereoSi => 10,
            Self::StereoTpSi => 11,
            Self::Top6Ch => 12,
            Self::Reserved(raw) => raw,
        }
    }

    /// The layout a wire byte carries.
    #[must_use]
    pub const fn from_value(value: u8) -> Self {
        match value {
            0 => Self::Lfe,
            1 => Self::StereoS,
            2 => Self::StereoSs,
            3 => Self::StereoRs,
            4 => Self::StereoTf,
            5 => Self::StereoTb,
            6 => Self::Top4Ch,
            7 => Self::Ch3_0,
            8 => Self::Ch9_1_6,
            9 => Self::StereoF,
            10 => Self::StereoSi,
            11 => Self::StereoTpSi,
            12 => Self::Top6Ch,
            other => Self::Reserved(other),
        }
    }
}

/// `ambisonics_mono_config` — `output_channel_count`, `substream_count` and one
/// mapping byte per output channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbisonicsMonoConfig {
    /// `output_channel_count`.
    pub output_channel_count: u8,
    /// `substream_count`.
    pub substream_count: u8,
    /// `channel_mapping`, one byte per output channel, in wire order.
    pub channel_mapping: Vec<u8>,
}

/// `ambisonics_projection_config` — a demixing matrix of signed 16-bit values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbisonicsProjectionConfig {
    /// `output_channel_count`.
    pub output_channel_count: u8,
    /// `substream_count`.
    pub substream_count: u8,
    /// `coupled_substream_count`.
    pub coupled_substream_count: u8,
    /// `demixing_matrix`, `(substream_count + coupled_substream_count) *
    /// output_channel_count` signed 16-bit entries, in wire order.
    pub demixing_matrix: Vec<i16>,
}

/// `ambisonics_config` — the **Audio Element**'s scene-based configuration.
///
/// Modelled structurally in Phase 1 and not exercised by the LPCM encode path;
/// Phase 2's PARSE-07 builds scene-based fixtures against it.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h AmbisonicsConfig
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbisonicsConfig {
    /// `kAmbisonicsModeMono = 0`.
    Mono(AmbisonicsMonoConfig),
    /// `kAmbisonicsModeProjection = 1`.
    Projection(AmbisonicsProjectionConfig),
    /// `kAmbisonicsModeReservedStart = 2` and above. The reference reads
    /// **nothing further** for a reserved mode, so the rest of the payload is
    /// the OBU's remainder rather than part of this config.
    Reserved {
        /// The `ambisonics_mode` value that was on the wire.
        mode: u32,
    },
}

impl AmbisonicsConfig {
    /// The `ambisonics_mode` uleb128 that precedes this config.
    #[must_use]
    pub const fn mode(&self) -> u32 {
        match self {
            Self::Mono(_) => 0,
            Self::Projection(_) => 1,
            Self::Reserved { mode } => *mode,
        }
    }
}

/// `sound_system` — a **4-bit** field in the **Mix Presentation**'s layout,
/// present only when `layout_type == 2` (the sound-system convention).
///
/// A different field, in a different OBU, with a different value space, from
/// [`LoudspeakerLayout`]. The two are not interchangeable and the type system
/// is what says so.
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.h LoudspeakersSsConventionLayout::SoundSystem
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundSystem {
    /// `kSoundSystemA_0_2_0 = 0` — **stereo**, the layout every sub-mix must
    /// carry (DESC-05).
    A0_2_0,
    /// `kSoundSystemB_0_5_0 = 1` — 5.1.
    B0_5_0,
    /// `kSoundSystemC_2_5_0 = 2`.
    C2_5_0,
    /// `kSoundSystemD_4_5_0 = 3`.
    D4_5_0,
    /// `kSoundSystemE_4_5_1 = 4`.
    E4_5_1,
    /// `kSoundSystemF_3_7_0 = 5`.
    F3_7_0,
    /// `kSoundSystemG_4_9_0 = 6`.
    G4_9_0,
    /// `kSoundSystemH_9_10_3 = 7`.
    H9_10_3,
    /// `kSoundSystemI_0_7_0 = 8`.
    I0_7_0,
    /// `kSoundSystemJ_4_7_0 = 9`.
    J4_7_0,
    /// `kSoundSystem10_2_7_0 = 10`.
    Ss10_2_7_0,
    /// `kSoundSystem11_2_3_0 = 11`.
    Ss11_2_3_0,
    /// `kSoundSystem12_0_1_0 = 12`.
    Ss12_0_1_0,
    /// `kSoundSystem13_6_9_0 = 13`.
    Ss13_6_9_0,
    /// `kSoundSystemBeginReserved = 14`, `kSoundSystemEndReserved = 15`.
    Reserved(u8),
}

impl SoundSystem {
    /// The 4-bit value this sound system occupies.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::A0_2_0 => 0,
            Self::B0_5_0 => 1,
            Self::C2_5_0 => 2,
            Self::D4_5_0 => 3,
            Self::E4_5_1 => 4,
            Self::F3_7_0 => 5,
            Self::G4_9_0 => 6,
            Self::H9_10_3 => 7,
            Self::I0_7_0 => 8,
            Self::J4_7_0 => 9,
            Self::Ss10_2_7_0 => 10,
            Self::Ss11_2_3_0 => 11,
            Self::Ss12_0_1_0 => 12,
            Self::Ss13_6_9_0 => 13,
            Self::Reserved(raw) => raw,
        }
    }

    /// The sound system a 4-bit field carries.
    #[must_use]
    pub const fn from_value(value: u8) -> Self {
        match value {
            0 => Self::A0_2_0,
            1 => Self::B0_5_0,
            2 => Self::C2_5_0,
            3 => Self::D4_5_0,
            4 => Self::E4_5_1,
            5 => Self::F3_7_0,
            6 => Self::G4_9_0,
            7 => Self::H9_10_3,
            8 => Self::I0_7_0,
            9 => Self::J4_7_0,
            10 => Self::Ss10_2_7_0,
            11 => Self::Ss11_2_3_0,
            12 => Self::Ss12_0_1_0,
            13 => Self::Ss13_6_9_0,
            other => Self::Reserved(other),
        }
    }
}
