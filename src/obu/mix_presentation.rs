//! The Mix Presentation OBU (type 2) — DESC-05, DESC-06 and the Mix
//! Presentation half of DESC-07.
//!
//! # The mandatory stereo layout is a **write-only** rule
//!
//! `iamf-tools@v2.1.0` refuses to write a sub-mix without a Sound System A
//! (0+2+0) layout — `"Every sub-mix must have a stereo layout."` — and
//! `found_stereo_layout` is set **only** by `sound_system ==
//! kSoundSystemA_0_2_0`. Its *read* path carries an explicit
//! `TODO(b/339855338)` and checks nothing. See [`SubMix::validate`] for how
//! this crate splits the difference three ways.
//!
//! # Two mandatory Mix Gain param definitions, and zero Parameter Blocks
//!
//! Every sub-mix element carries an `element_mix_gain` and every sub-mix an
//! `output_mix_gain`, whether or not the file contains a single Parameter Block
//! OBU — `test_000003` contains none. They are therefore **non-optional fields
//! of their parents** rather than `Option`s: a sub-mix that structurally cannot
//! omit one is a stronger guarantee than one that validates that it did not,
//! and it means no interleaving or partial write can produce a sub-mix carrying
//! one definition and not the other (DESC-06).

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Finding, Location, Result};
use crate::model::layout::SoundSystem;
use crate::model::loudness::Q7_8;
use crate::obu::param_definition::{
    read_param_definition, write_param_definition, ParamDefinition,
};

/// `headphones_rendering_mode` — a 2-bit field in the rendering config.
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.h RenderingConfig::HeadphonesRenderingMode
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadphonesRenderingMode {
    /// `kHeadphonesRenderingModeStereo = 0`.
    Stereo,
    /// `kHeadphonesRenderingModeBinaural = 1`.
    Binaural,
    /// `kHeadphonesRenderingModeReserved2 = 2`, `…Reserved3 = 3`.
    Reserved(u8),
}

impl HeadphonesRenderingMode {
    /// The 2-bit value this mode occupies.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Stereo => 0,
            Self::Binaural => 1,
            Self::Reserved(raw) => raw,
        }
    }

    /// The mode a 2-bit field carries.
    #[must_use]
    pub const fn from_value(value: u8) -> Self {
        match value {
            0 => Self::Stereo,
            1 => Self::Binaural,
            other => Self::Reserved(other),
        }
    }
}

/// `rendering_config` — the mode plus an opaque, length-prefixed extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderingConfig {
    /// `headphones_rendering_mode`.
    pub headphones_rendering_mode: HeadphonesRenderingMode,
    /// Six reserved bits following `headphones_rendering_mode`.
    pub reserved: u8,
    /// `rendering_config_extension_bytes`, verbatim. Its size is derived from
    /// this length.
    pub extension: Vec<u8>,
}

impl RenderingConfig {
    /// Stereo headphones rendering with no extension — the Phase 1 path.
    #[must_use]
    pub const fn stereo() -> Self {
        Self {
            headphones_rendering_mode: HeadphonesRenderingMode::Stereo,
            reserved: 0,
            extension: Vec::new(),
        }
    }
}

/// A Mix Gain parameter definition: the shared fields plus `default_mix_gain`.
///
/// Held by value, never as an `Option` — see the module comment.
// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.h MixGainParamDefinition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixGainParamDefinition {
    /// The shared parameter-definition fields.
    pub definition: ParamDefinition,
    /// `default_mix_gain`, a signed 16-bit big-endian Q7.8 value.
    pub default_mix_gain: i16,
}

impl MixGainParamDefinition {
    /// A mode-1 definition with a zero default gain — the Phase 1 path, which
    /// emits the mandatory structure while the file carries zero Parameter
    /// Block OBUs (DESC-06).
    #[must_use]
    pub const fn mode_1(parameter_id: u32, parameter_rate: u32) -> Self {
        Self {
            definition: ParamDefinition::mode_1(parameter_id, parameter_rate),
            default_mix_gain: 0,
        }
    }
}

/// One `anchored_loudness` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorElement {
    /// `anchor_element`.
    pub anchor_element: u8,
    /// `anchored_loudness`, signed 16-bit.
    pub anchored_loudness: i16,
}

/// `anchored_loudness` — `num_anchored_loudness` is derived from the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchoredLoudness {
    /// The anchor elements, in bitstream order.
    pub anchor_elements: Vec<AnchorElement>,
}

/// The `info_type & 0xFC` extension: the bits that selected it, and its bytes.
///
/// The bits are preserved because `info_type` is derived, not stored: without
/// them a file using bit 4 would re-serialise with bit 2 and stop being
/// byte-identical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoudnessExtension {
    /// The `info_type` bits in the `0xFC` mask that were set.
    pub info_type_bits: u8,
    /// `info_type_bytes`, verbatim.
    pub bytes: Vec<u8>,
}

/// `loudness_info`.
///
/// `info_type` is **computed** from the three `Option`s (Pattern 2) and never
/// stored, which is what makes a flag/field disagreement unrepresentable.
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.h LoudnessInfo
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loudness {
    /// `integrated_loudness`, signed 16-bit big-endian Q7.8.
    pub integrated: i16,
    /// `digital_peak`, signed 16-bit big-endian Q7.8.
    pub digital_peak: i16,
    /// `true_peak` — `info_type & 0x01`.
    pub true_peak: Option<i16>,
    /// `anchored_loudness` — `info_type & 0x02`.
    pub anchored: Option<AnchoredLoudness>,
    /// The extension — `info_type & 0xFC`.
    pub extension: Option<LoudnessExtension>,
}

impl Loudness {
    /// Integrated loudness and digital peak only — `info_type == 0`.
    #[must_use]
    pub const fn new(integrated: i16, digital_peak: i16) -> Self {
        Self {
            integrated,
            digital_peak,
            true_peak: None,
            anchored: None,
            extension: None,
        }
    }

    /// The same, from values that have been through PROF-03's range check.
    ///
    /// This is the join between the float→fixed path and the wire model. A
    /// caller measuring loudness converts once, at one named place
    /// ([`crate::model::lufs_to_q7_8`]), and hands the results here — rather
    /// than each call site scaling by 256 and casting for itself, which is how
    /// a rounding rule ends up applied three different ways in one crate.
    ///
    /// This module never sees a float; the conversion is confined to
    /// `src/model/loudness.rs`, which is what the D-21 census asserts.
    #[must_use]
    pub const fn from_q7_8(integrated: Q7_8, digital_peak: Q7_8) -> Self {
        Self::new(integrated.to_i16(), digital_peak.to_i16())
    }

    /// `info_type`, computed from the optional members it gates.
    #[must_use]
    pub fn info_type(&self) -> u8 {
        let mut info_type = 0_u8;
        if self.true_peak.is_some() {
            info_type |= 0x01;
        }
        if self.anchored.is_some() {
            info_type |= 0x02;
        }
        if let Some(extension) = self.extension.as_ref() {
            info_type |= extension.info_type_bits & 0xfc;
        }
        info_type
    }
}

/// `loudness_layout` — the 2-bit `layout_type` and what it selects.
///
/// [`Self::SoundSystem`] carries a [`SoundSystem`], which is the **Mix
/// Presentation**'s 4-bit layout field. It is a different field, in a different
/// OBU, from the Audio Element's `loudspeaker_layout`.
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.h Layout
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `kLayoutTypeLoudspeakersSsConvention = 2`.
    SoundSystem(SoundSystem),
    /// `kLayoutTypeBinaural = 3`.
    Binaural,
    /// `kLayoutTypeReserved0 = 0`, `kLayoutTypeReserved1 = 1`.
    Reserved(u8),
}

impl Layout {
    /// The 2-bit `layout_type` this layout occupies.
    #[must_use]
    pub const fn layout_type(self) -> u8 {
        match self {
            Self::SoundSystem(_) => 2,
            Self::Binaural => 3,
            Self::Reserved(raw) => raw,
        }
    }

    /// The `sound_system`, when this is the sound-system convention.
    #[must_use]
    pub const fn sound_system(self) -> Option<SoundSystem> {
        match self {
            Self::SoundSystem(sound_system) => Some(sound_system),
            _ => None,
        }
    }
}

/// A layout and its loudness, structurally paired so one cannot appear without
/// the other.
///
/// A sub-mix carrying a 5.1 target **and** the mandatory stereo layout has two
/// of these, and therefore two `Loudness` blocks — which is exactly the shape
/// plan 01-08's 5.1 fixture needs (research correction 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutWithLoudness {
    /// `loudness_layout`.
    pub layout: Layout,
    /// Reserved bits following the layout selector: two bits for Sound System
    /// layouts and six bits for every other layout type.
    pub reserved: u8,
    /// `loudness`.
    pub loudness: Loudness,
}

/// One entry of a sub-mix's `audio_elements`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubMixAudioElement {
    /// `audio_element_id`, resolved forward-only through `by_id`.
    pub audio_element_id: u32,
    /// `localized_element_annotations`, one per `count_label`, without their
    /// NUL terminators.
    pub localized_element_annotations: Vec<Vec<u8>>,
    /// `rendering_config`.
    pub rendering_config: RenderingConfig,
    /// `element_mix_config` — mandatory structure (DESC-06).
    pub element_mix_gain: MixGainParamDefinition,
}

/// One `sub_mixes` entry.
///
/// `num_audio_elements` and `num_layouts` are derived from the `Vec` lengths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubMix {
    /// The elements this sub-mix mixes, in bitstream order.
    pub elements: Vec<SubMixAudioElement>,
    /// `output_mix_config` — mandatory structure (DESC-06).
    pub output_mix_gain: MixGainParamDefinition,
    /// `layouts`, each paired with its loudness.
    pub layouts: Vec<LayoutWithLoudness>,
}

impl SubMix {
    /// `num_audio_elements`, derived.
    #[must_use]
    pub fn num_audio_elements(&self) -> usize {
        self.elements.len()
    }

    /// `num_layouts`, derived.
    #[must_use]
    pub fn num_layouts(&self) -> usize {
        self.layouts.len()
    }

    /// Whether this sub-mix carries the mandatory Sound System A (0+2+0)
    /// stereo layout.
    #[must_use]
    pub fn has_stereo_layout(&self) -> bool {
        self.layouts
            .iter()
            .any(|entry| entry.layout.sound_system() == Some(SoundSystem::A0_2_0))
    }

    /// Findings for this sub-mix.
    ///
    /// **The missing-stereo-layout rule is handled as three separate things,
    /// and all three matter. Do not "tighten" any of them.**
    ///
    /// 1. The **model supports two layouts** with two `Loudness` blocks per
    ///    sub-mix, because plan 01-08's 5.1 fixture needs Sound System B as the
    ///    comparison target *and* Sound System A because `iamf-tools` demands
    ///    it.
    /// 2. **`validate()` reports its absence** as a Finding, quoting the
    ///    reference's own message.
    /// 3. **The reader must not reject it.** `iamf-tools` enforces the rule on
    ///    write only; its read path carries `TODO(b/339855338)` and checks
    ///    nothing. Rejecting on read would make this crate stricter than the
    ///    reference and break Phase 2's foreign-file round-trip.
    // ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc MixPresentationSubMix::ValidateAndWrite
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if !self.has_stereo_layout() {
            findings.push(Finding {
                at: Location::Field("sub_mix.layouts"),
                message: "no Sound System A (0+2+0) stereo layout: iamf-tools refuses to write \
                          this sub-mix — \"Every sub-mix must have a stereo layout.\" — while \
                          both libiamf revisions and iamf-tools' own read path accept it"
                    .to_owned(),
            });
        }
        if self.elements.is_empty() {
            findings.push(Finding {
                at: Location::Field("sub_mix.num_audio_elements"),
                message: "num_audio_elements is 0; a sub-mix with no element mixes nothing"
                    .to_owned(),
            });
        }
        findings
    }
}

/// The Mix Presentation OBU payload.
///
/// `count_label` is derived from `annotations_language.len()`, never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixPresentation {
    /// `mix_presentation_id`.
    pub mix_presentation_id: u32,
    /// `annotations_language`, without NUL terminators. `count_label` is this
    /// length.
    pub annotations_language: Vec<Vec<u8>>,
    /// `localized_presentation_annotations`, without NUL terminators.
    pub localized_presentation_annotations: Vec<Vec<u8>>,
    /// `sub_mixes`, in bitstream order.
    pub sub_mixes: Vec<SubMix>,
    /// Payload bytes past what this type understood.
    pub trailing: Vec<u8>,
}

impl MixPresentation {
    /// `count_label`, derived from the language list.
    #[must_use]
    pub fn count_label(&self) -> usize {
        self.annotations_language.len()
    }

    /// Findings for this Mix Presentation and every sub-mix it holds.
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if self.localized_presentation_annotations.len() != self.count_label() {
            findings.push(Finding {
                at: Location::Field("localized_presentation_annotations"),
                message: format!(
                    "there are {} localized presentation annotations but count_label is {}",
                    self.localized_presentation_annotations.len(),
                    self.count_label()
                ),
            });
        }
        for sub_mix in &self.sub_mixes {
            findings.extend(sub_mix.validate());
            for element in &sub_mix.elements {
                push_reserved_finding(
                    &mut findings,
                    element.rendering_config.reserved,
                    "rendering_config.reserved",
                    6,
                );
                findings.extend(element.element_mix_gain.definition.validate());
                if element.localized_element_annotations.len() != self.count_label() {
                    findings.push(Finding {
                        at: Location::Field("localized_element_annotations"),
                        message: format!(
                            "audio element {} carries {} localized annotations but count_label \
                             is {}",
                            element.audio_element_id,
                            element.localized_element_annotations.len(),
                            self.count_label()
                        ),
                    });
                }
            }
            findings.extend(sub_mix.output_mix_gain.definition.validate());
            for layout in &sub_mix.layouts {
                let width = if layout.layout.sound_system().is_some() {
                    2
                } else {
                    6
                };
                push_reserved_finding(&mut findings, layout.reserved, "layout.reserved", width);
            }
        }
        findings
    }
}

fn push_reserved_finding(findings: &mut Vec<Finding>, value: u8, field: &'static str, width: u8) {
    if value != 0 {
        findings.push(Finding {
            at: Location::Field(field),
            message: format!("{field} is non-zero ({value:#x}) in its {width}-bit reserved field"),
        });
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc MixPresentationObu::ReadAndValidatePayloadDerived
/// Read a Mix Presentation payload from a **bounded** payload reader.
///
/// Every count is checked against `bytes_remaining()` before anything is
/// reserved (T-01-24, T-01-26): a NUL-terminated string is at least one byte,
/// as is every uleb128, so a count above the bytes left is unreadable by
/// construction.
pub fn read_mix_presentation(r: &mut BitCursor<'_>) -> Result<MixPresentation> {
    let mix_presentation_id = r.read_uleb128()?;
    let count_label = read_bounded_count(r)?;
    let annotations_language = read_strings(r, count_label)?;
    let localized_presentation_annotations = read_strings(r, count_label)?;

    let num_sub_mixes = read_bounded_count(r)?;
    let mut sub_mixes = Vec::with_capacity(num_sub_mixes);
    for _ in 0..num_sub_mixes {
        sub_mixes.push(read_sub_mix(r, count_label)?);
    }

    let remaining = r.bytes_remaining();
    let trailing = r.read_uint8_span(remaining)?.to_vec();

    Ok(MixPresentation {
        mix_presentation_id,
        annotations_language,
        localized_presentation_annotations,
        sub_mixes,
        trailing,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc MixPresentationObu::ValidateAndWritePayload
/// Write a Mix Presentation payload, `trailing` last.
///
/// Faithful, not validating (D-07): a sub-mix without a stereo layout is
/// written, and `validate()` is where its absence is reported.
pub fn write_mix_presentation(w: &mut BitWriter, v: &MixPresentation) -> Result<()> {
    validate_annotation_counts(v)?;

    w.write_uleb128_minimal(v.mix_presentation_id)?;
    write_derived_count(w, v.count_label(), "count_label")?;
    for language in &v.annotations_language {
        w.write_string(language)?;
    }
    for annotation in &v.localized_presentation_annotations {
        w.write_string(annotation)?;
    }
    write_derived_count(w, v.sub_mixes.len(), "num_sub_mixes")?;
    for sub_mix in &v.sub_mixes {
        write_sub_mix(w, sub_mix)?;
    }
    w.write_bytes(&v.trailing)
}

fn validate_annotation_counts(v: &MixPresentation) -> Result<()> {
    let count_label = v.count_label();
    if v.localized_presentation_annotations.len() != count_label {
        return Err(Error::new(
            ErrorKind::AnnotationCountMismatch,
            Location::Field("localized_presentation_annotations"),
        ));
    }
    for sub_mix in &v.sub_mixes {
        for element in &sub_mix.elements {
            if element.localized_element_annotations.len() != count_label {
                return Err(Error::new(
                    ErrorKind::AnnotationCountMismatch,
                    Location::Field("localized_element_annotations"),
                ));
            }
        }
    }
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadULeb128
// NOTE: the bound, not the read, is what a reviewer must check here. The
// smallest item any count in this OBU governs is one byte, so a count above
// `bytes_remaining()` is unreadable by construction and must not reserve.
/// Read a uleb128 count and refuse one larger than the input can supply.
fn read_bounded_count(r: &mut BitCursor<'_>) -> Result<usize> {
    let start = r.byte_position();
    let count = r.read_uleb128()?;
    let count = usize::try_from(count)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
    if count > r.bytes_remaining() {
        return Err(Error::new(
            ErrorKind::UnexpectedEndOfInput,
            Location::InputOffset(start),
        ));
    }
    Ok(count)
}

// ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteUleb128
/// Write a derived count, refusing one that cannot be represented.
fn write_derived_count(w: &mut BitWriter, count: usize, field: &'static str) -> Result<()> {
    let count = u32::try_from(count)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::Field(field)))?;
    w.write_uleb128_minimal(count)
}

// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadString
/// Read `count` NUL-terminated strings, storing each **without** its
/// terminator so it can go straight back through `write_string`.
fn read_strings(r: &mut BitCursor<'_>, count: usize) -> Result<Vec<Vec<u8>>> {
    if count > r.bytes_remaining() {
        return Err(Error::new(
            ErrorKind::UnexpectedEndOfInput,
            Location::InputOffset(r.byte_position()),
        ));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut bytes = r.read_string()?;
        // `read_string` returns the terminator; `write_string` appends one.
        bytes.pop();
        out.push(bytes);
    }
    Ok(out)
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc MixPresentationSubMix::ReadAndValidate
/// Read one sub-mix. `count_label` comes from the enclosing Mix Presentation —
/// the element annotation arrays have no count of their own.
fn read_sub_mix(r: &mut BitCursor<'_>, count_label: usize) -> Result<SubMix> {
    let num_audio_elements = read_bounded_count(r)?;
    let mut elements = Vec::with_capacity(num_audio_elements);
    for _ in 0..num_audio_elements {
        elements.push(read_sub_mix_audio_element(r, count_label)?);
    }
    let output_mix_gain = read_mix_gain_param_definition(r)?;
    let num_layouts = read_bounded_count(r)?;
    let mut layouts = Vec::with_capacity(num_layouts);
    for _ in 0..num_layouts {
        layouts.push(read_layout_with_loudness(r)?);
    }
    Ok(SubMix {
        elements,
        output_mix_gain,
        layouts,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc MixPresentationSubMix::ValidateAndWrite
// NOTE: the reference REJECTS a sub-mix with no Sound System A layout here.
// This writer does not — D-07 makes the writer faithful and `SubMix::validate`
// explicit, so a foreign file round-trips. See `SubMix::validate` for the three
// facts that must stay separated.
/// Write one sub-mix.
fn write_sub_mix(w: &mut BitWriter, v: &SubMix) -> Result<()> {
    write_derived_count(w, v.elements.len(), "num_audio_elements")?;
    for element in &v.elements {
        write_sub_mix_audio_element(w, element)?;
    }
    write_mix_gain_param_definition(w, &v.output_mix_gain)?;
    write_derived_count(w, v.layouts.len(), "num_layouts")?;
    for layout in &v.layouts {
        write_layout_with_loudness(w, layout)?;
    }
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc SubMixAudioElement::ReadAndValidate
/// Read one sub-mix element.
fn read_sub_mix_audio_element(
    r: &mut BitCursor<'_>,
    count_label: usize,
) -> Result<SubMixAudioElement> {
    let audio_element_id = r.read_uleb128()?;
    let localized_element_annotations = read_strings(r, count_label)?;
    let rendering_config = read_rendering_config(r)?;
    let element_mix_gain = read_mix_gain_param_definition(r)?;
    Ok(SubMixAudioElement {
        audio_element_id,
        localized_element_annotations,
        rendering_config,
        element_mix_gain,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc SubMixAudioElement::Write
/// Write one sub-mix element.
fn write_sub_mix_audio_element(w: &mut BitWriter, v: &SubMixAudioElement) -> Result<()> {
    w.write_uleb128_minimal(v.audio_element_id)?;
    for annotation in &v.localized_element_annotations {
        w.write_string(annotation)?;
    }
    write_rendering_config(w, &v.rendering_config)?;
    write_mix_gain_param_definition(w, &v.element_mix_gain)
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc RenderingConfig::ReadAndValidate
/// Read `headphones_rendering_mode(2)`, six reserved bits, then the
/// length-prefixed extension.
fn read_rendering_config(r: &mut BitCursor<'_>) -> Result<RenderingConfig> {
    let headphones_rendering_mode =
        HeadphonesRenderingMode::from_value(u8::try_from(r.read_unsigned(2)?).unwrap_or(0));
    let reserved = u8::try_from(r.read_unsigned(6)?).unwrap_or(0);
    let start = r.byte_position();
    let size = r.read_uleb128()?;
    let size = usize::try_from(size)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
    Ok(RenderingConfig {
        headphones_rendering_mode,
        reserved,
        extension: r.read_uint8_span(size)?.to_vec(),
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc RenderingConfig::Write
/// Write the rendering config, its extension size derived from the bytes.
fn write_rendering_config(w: &mut BitWriter, v: &RenderingConfig) -> Result<()> {
    w.write_unsigned(u64::from(v.headphones_rendering_mode.value()), 2)?;
    w.write_unsigned(u64::from(v.reserved), 6)?;
    write_derived_count(w, v.extension.len(), "rendering_config_extension_size")?;
    w.write_bytes(&v.extension)
}

// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.cc MixGainParamDefinition::ReadAndValidate
/// Read a Mix Gain param definition: the shared fields, then
/// `default_mix_gain`.
///
/// With `param_definition_mode == 1` the gain follows **immediately** — there
/// are no `duration`, `constant_subblock_duration` or `num_subblocks` fields.
fn read_mix_gain_param_definition(r: &mut BitCursor<'_>) -> Result<MixGainParamDefinition> {
    let definition = read_param_definition(r)?;
    let default_mix_gain = i16::try_from(r.read_signed(16)?).unwrap_or(0);
    Ok(MixGainParamDefinition {
        definition,
        default_mix_gain,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.cc MixGainParamDefinition::ValidateAndWrite
/// Write a Mix Gain param definition.
///
/// Emitted from **this** function rather than from a separate pass over the
/// sub-mix, so no interleaving or partial write can produce a sub-mix carrying
/// one of its two mandatory definitions and not the other (DESC-06).
fn write_mix_gain_param_definition(w: &mut BitWriter, v: &MixGainParamDefinition) -> Result<()> {
    write_param_definition(w, &v.definition)?;
    w.write_signed(i64::from(v.default_mix_gain), 16)
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc MixPresentationLayout::ReadAndValidate
/// Read `layout_type(2)` and what it selects, then the loudness that is
/// structurally paired with it.
fn read_layout_with_loudness(r: &mut BitCursor<'_>) -> Result<LayoutWithLoudness> {
    let layout_type = u8::try_from(r.read_unsigned(2)?).unwrap_or(0);
    let (layout, reserved) = if layout_type == 2 {
        let sound_system = u8::try_from(r.read_unsigned(4)?).unwrap_or(0);
        let reserved = u8::try_from(r.read_unsigned(2)?).unwrap_or(0);
        (
            Layout::SoundSystem(SoundSystem::from_value(sound_system)),
            reserved,
        )
    } else {
        let reserved = u8::try_from(r.read_unsigned(6)?).unwrap_or(0);
        let layout = if layout_type == 3 {
            Layout::Binaural
        } else {
            Layout::Reserved(layout_type)
        };
        (layout, reserved)
    };
    Ok(LayoutWithLoudness {
        layout,
        reserved,
        loudness: read_loudness(r)?,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc ValidateAndWriteLayout
/// Write the layout and its loudness.
fn write_layout_with_loudness(w: &mut BitWriter, v: &LayoutWithLoudness) -> Result<()> {
    w.write_unsigned(u64::from(v.layout.layout_type()), 2)?;
    match v.layout.sound_system() {
        Some(sound_system) => {
            w.write_unsigned(u64::from(sound_system.value()), 4)?;
            w.write_unsigned(u64::from(v.reserved), 2)?;
        }
        None => w.write_unsigned(u64::from(v.reserved), 6)?,
    }
    write_loudness(w, &v.loudness)
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc LoudnessInfo::ReadAndValidate
/// Read `info_type` and the members it gates.
fn read_loudness(r: &mut BitCursor<'_>) -> Result<Loudness> {
    let info_type = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    let integrated = i16::try_from(r.read_signed(16)?).unwrap_or(0);
    let digital_peak = i16::try_from(r.read_signed(16)?).unwrap_or(0);

    let true_peak = if info_type & 0x01 == 0 {
        None
    } else {
        Some(i16::try_from(r.read_signed(16)?).unwrap_or(0))
    };
    let anchored = if info_type & 0x02 == 0 {
        None
    } else {
        let start = r.byte_position();
        let count = usize::from(u8::try_from(r.read_unsigned(8)?).unwrap_or(0));
        // Three bytes per anchor element, checked before reserving.
        let bytes_needed = count
            .checked_mul(3)
            .ok_or_else(|| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
        if bytes_needed > r.bytes_remaining() {
            return Err(Error::new(
                ErrorKind::UnexpectedEndOfInput,
                Location::InputOffset(start),
            ));
        }
        let mut anchor_elements = Vec::with_capacity(count);
        for _ in 0..count {
            anchor_elements.push(AnchorElement {
                anchor_element: u8::try_from(r.read_unsigned(8)?).unwrap_or(0),
                anchored_loudness: i16::try_from(r.read_signed(16)?).unwrap_or(0),
            });
        }
        Some(AnchoredLoudness { anchor_elements })
    };
    let extension = if info_type & 0xfc == 0 {
        None
    } else {
        let start = r.byte_position();
        let size = r.read_uleb128()?;
        let size = usize::try_from(size)
            .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
        Some(LoudnessExtension {
            info_type_bits: info_type & 0xfc,
            bytes: r.read_uint8_span(size)?.to_vec(),
        })
    };

    Ok(Loudness {
        integrated,
        digital_peak,
        true_peak,
        anchored,
        extension,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc LoudnessInfo::ValidateAndWrite
/// Write `info_type` — **computed** from the members it gates — then those
/// members.
fn write_loudness(w: &mut BitWriter, v: &Loudness) -> Result<()> {
    w.write_unsigned(u64::from(v.info_type()), 8)?;
    w.write_signed(i64::from(v.integrated), 16)?;
    w.write_signed(i64::from(v.digital_peak), 16)?;
    if let Some(true_peak) = v.true_peak {
        w.write_signed(i64::from(true_peak), 16)?;
    }
    if let Some(anchored) = v.anchored.as_ref() {
        let count = u8::try_from(anchored.anchor_elements.len()).map_err(|_| {
            Error::new(
                ErrorKind::ObuTooLarge,
                Location::Field("num_anchored_loudness"),
            )
        })?;
        w.write_unsigned(u64::from(count), 8)?;
        for anchor in &anchored.anchor_elements {
            w.write_unsigned(u64::from(anchor.anchor_element), 8)?;
            w.write_signed(i64::from(anchor.anchored_loudness), 16)?;
        }
    }
    if let Some(extension) = v.extension.as_ref() {
        write_derived_count(w, extension.bytes.len(), "info_type_size")?;
        w.write_bytes(&extension.bytes)?;
    }
    Ok(())
}
