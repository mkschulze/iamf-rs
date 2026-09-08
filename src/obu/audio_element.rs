//! The Audio Element OBU (type 1) — DESC-04, and D-04's published
//! [`AudioElementType`].

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Finding, Location, Result};
use crate::model::layout::{
    AmbisonicsConfig, AmbisonicsMonoConfig, AmbisonicsProjectionConfig, ExpandedLoudspeakerLayout,
    LoudspeakerLayout,
};
use crate::obu::param_definition::{ParamDefinition, read_param_definition, write_param_definition};

/// `param_definition_type` for a demixing parameter.
pub const PARAM_DEFINITION_DEMIXING: u32 = 1;
/// `param_definition_type` for a recon-gain parameter.
pub const PARAM_DEFINITION_RECON_GAIN: u32 = 2;

/// `output_gain_flags` plus the signed 16-bit gain they describe, present on
/// the wire only when `output_gain_is_present_flag` is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputGain {
    /// `output_gain_flags`, a 6-bit channel mask.
    pub flags: u8,
    /// `output_gain`, a signed 16-bit Q7.8 value.
    pub gain: i16,
}

/// One entry of `channel_audio_layer_configs`.
///
/// `output_gain_is_present_flag` is **derived** from `output_gain.is_some()`
/// (Pattern 2), which is what makes a flag/field disagreement unrepresentable.
///
/// `recon_gain_is_present` is a plain stored field and deliberately so: unlike
/// every other gate flag in this crate it gates data in a **different OBU** —
/// the Recon Gain Parameter Block — so there is nothing local to derive it
/// from. Pattern 2 applies to a flag and the fields it gates *in the same
/// structure*; this flag has none.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h ChannelAudioLayerConfig
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelAudioLayerConfig {
    /// `loudspeaker_layout`, 4 bits. `Expanded` carries the
    /// `expanded_loudspeaker_layout` byte inside itself.
    pub loudspeaker_layout: LoudspeakerLayout,
    /// `output_gain_flags` + `output_gain`, present only when the flag is set.
    pub output_gain: Option<OutputGain>,
    /// `recon_gain_is_present_flag`. See the type comment for why this one is
    /// stored.
    pub recon_gain_is_present: bool,
    /// `substream_count`.
    pub substream_count: u8,
    /// `coupled_substream_count`.
    pub coupled_substream_count: u8,
}

impl ChannelAudioLayerConfig {
    /// A layer with both gate flags clear — DESC-04's Phase 1 single-layer
    /// path.
    #[must_use]
    pub const fn new(
        loudspeaker_layout: LoudspeakerLayout,
        substream_count: u8,
        coupled_substream_count: u8,
    ) -> Self {
        Self {
            loudspeaker_layout,
            output_gain: None,
            recon_gain_is_present: false,
            substream_count,
            coupled_substream_count,
        }
    }

    /// `output_gain_is_present_flag`, derived from the data it gates.
    #[must_use]
    pub const fn output_gain_is_present(&self) -> bool {
        self.output_gain.is_some()
    }

    /// `recon_gain_is_present_flag`.
    #[must_use]
    pub const fn recon_gain_is_present(&self) -> bool {
        self.recon_gain_is_present
    }
}

/// `scalable_channel_layout_config` — `num_layers` and the layers themselves.
///
/// `num_layers` is derived from `layers.len()`, never stored.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h ScalableChannelLayoutConfig
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalableChannelLayoutConfig {
    /// The layers, in bitstream order.
    pub layers: Vec<ChannelAudioLayerConfig>,
}

impl ScalableChannelLayoutConfig {
    /// A single-layer configuration — the Phase 1 encode path.
    #[must_use]
    pub fn single_layer(layer: ChannelAudioLayerConfig) -> Self {
        Self {
            layers: vec![layer],
        }
    }

    /// `num_layers`, a 3-bit field derived from the layer count.
    #[must_use]
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }
}

/// The channel-based `audio_element_config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelBasedConfig {
    /// `scalable_channel_layout_config`.
    pub scalable_channel_layout: ScalableChannelLayoutConfig,
}

/// `audio_element_type` and the config it gates — **D-04's published enum**.
///
/// # The invariant that makes `Reserved` round-trip (D-05)
///
/// `Reserved` can only round-trip because the parser can skip an unknown
/// element's payload *without understanding it*, and that requires the
/// remaining length to be derivable. It is, because every IAMF OBU carries
/// `obu_size`: the payload is `obu_size` minus the bytes the common fields
/// consumed. **That is the thing that silently breaks if a future field is
/// ever added *before* the type-specific config** — the common fields would
/// consume more, `raw` would start later, and a file written by an older
/// version would re-serialise differently with no error anywhere.
///
/// # Precedence against the OBU-level `trailing` (D-05)
///
/// `raw` consumes to the **end of the Audio Element payload**, so the
/// OBU-level [`crate::obu::Obu::trailing`] drain is left **empty**. Two
/// mechanisms otherwise both claim "everything after the fields I understood"
/// and the byte-identity property becomes untestable — which is the only
/// property that can prove we preserved something we did not understand.
///
/// # Constructibility
///
/// Only [`Self::ChannelBased`] is constructible through the **public encoder
/// path**. [`Self::SceneBased`] has a `pub(crate)` constructor and a
/// `#[doc(hidden)]` test-only one, because Phase 2's PARSE-07 must build
/// scene-based fixtures and "asserted understood" is unmeetable otherwise.
///
/// **Reversibility: one-way.** Parallax's import adapter matches on this enum.
/// Adding a variant is additive under `#[non_exhaustive]`; changing `Reserved`'s
/// shape or making `SceneBased` publicly constructible is a breaking contract
/// change across two repositories.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h AudioElementObu::AudioElementType
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioElementType {
    /// `AUDIO_ELEMENT_CHANNEL_BASED = 0`.
    ChannelBased(ChannelBasedConfig),
    /// `AUDIO_ELEMENT_SCENE_BASED = 1`. Not publicly constructible.
    SceneBased(AmbisonicsConfig),
    /// `2..=7` — a type this spec version does not define.
    Reserved {
        /// The 3-bit `audio_element_type` value that was on the wire.
        value: u8,
        /// Every remaining byte of the Audio Element payload, verbatim.
        raw: Vec<u8>,
    },
}

impl AudioElementType {
    /// A channel-based element — the only variant the public encoder path can
    /// build.
    #[must_use]
    pub const fn channel_based(config: ChannelBasedConfig) -> Self {
        Self::ChannelBased(config)
    }

    /// A scene-based element. Crate-internal: the encoder emits beds only, and
    /// this variant exists for the *decode* side, which is a superset.
    #[must_use]
    pub(crate) const fn scene_based(config: AmbisonicsConfig) -> Self {
        Self::SceneBased(config)
    }

    /// A scene-based element, for tests and fixtures only.
    ///
    /// Phase 2's PARSE-07 asserts that scene-based elements are *understood*,
    /// which is unmeetable without a way to build one.
    #[doc(hidden)]
    #[must_use]
    pub fn scene_based_for_test(config: AmbisonicsConfig) -> Self {
        Self::scene_based(config)
    }

    /// The 3-bit `audio_element_type` value.
    #[must_use]
    pub const fn value(&self) -> u8 {
        match self {
            Self::ChannelBased(_) => 0,
            Self::SceneBased(_) => 1,
            Self::Reserved { value, .. } => *value,
        }
    }
}

/// One entry of the Audio Element's `audio_element_params`.
///
/// The extension arm is what makes an unknown `param_definition_type` skippable
/// at all: the format gives types above 2 an explicit `param_definition_size`,
/// so the length is on the wire rather than implied by a definition we do not
/// have.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h AudioElementParam
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioElementParam {
    /// `PARAMETER_DEFINITION_DEMIXING = 1`.
    Demixing {
        /// The shared parameter-definition fields.
        definition: ParamDefinition,
        /// `default_dmixp_mode`, 3 bits.
        default_dmixp_mode: u8,
        /// `default_w`, 4 bits.
        default_w: u8,
    },
    /// `PARAMETER_DEFINITION_RECON_GAIN = 2`.
    ReconGain {
        /// The shared parameter-definition fields.
        definition: ParamDefinition,
    },
    /// Any other `param_definition_type`, carried with its explicit size.
    Extension {
        /// The `param_definition_type` that was on the wire.
        param_definition_type: u32,
        /// `param_definition_bytes`, verbatim.
        bytes: Vec<u8>,
    },
}

impl AudioElementParam {
    /// The `param_definition_type` this entry carries.
    #[must_use]
    pub const fn param_definition_type(&self) -> u32 {
        match self {
            Self::Demixing { .. } => PARAM_DEFINITION_DEMIXING,
            Self::ReconGain { .. } => PARAM_DEFINITION_RECON_GAIN,
            Self::Extension {
                param_definition_type,
                ..
            } => *param_definition_type,
        }
    }
}

/// The Audio Element OBU payload.
///
/// `audio_substream_ids` is a **`Vec` in bitstream order** (DESC-09), never a
/// map: it is an ordered sequence in the wire format, and a map would trade a
/// determinism guarantee for a round-trip bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioElement {
    /// `audio_element_id`.
    pub audio_element_id: u32,
    /// `audio_element_type` and the config it gates.
    pub audio_element_type: AudioElementType,
    /// `codec_config_id`, resolved forward-only through
    /// `DescriptorSet::codec_config_by_id`.
    pub codec_config_id: u32,
    /// `audio_substream_ids`, in bitstream order. `num_substreams` is derived
    /// from this length.
    pub audio_substream_ids: Vec<u32>,
    /// `audio_element_params`. `num_parameters` is derived from this length.
    pub params: Vec<AudioElementParam>,
    /// Payload bytes past what the type-specific config claimed.
    ///
    /// Always empty when `audio_element_type` is [`AudioElementType::Reserved`],
    /// because `raw` consumes to the end of the payload (D-05 precedence).
    pub trailing: Vec<u8>,
}

impl AudioElement {
    /// A channel-based Audio Element through the **public encoder path**.
    #[must_use]
    pub fn channel_based(
        audio_element_id: u32,
        codec_config_id: u32,
        audio_substream_ids: Vec<u32>,
        scalable_channel_layout: ScalableChannelLayoutConfig,
    ) -> Self {
        Self {
            audio_element_id,
            audio_element_type: AudioElementType::channel_based(ChannelBasedConfig {
                scalable_channel_layout,
            }),
            codec_config_id,
            audio_substream_ids,
            params: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// `num_substreams`, derived from the id list.
    #[must_use]
    pub fn num_substreams(&self) -> usize {
        self.audio_substream_ids.len()
    }

    /// `num_parameters`, derived from the param list.
    #[must_use]
    pub fn num_parameters(&self) -> usize {
        self.params.len()
    }

    /// Findings this element can carry, reported and never raised (D-07/D-09).
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if self.audio_substream_ids.is_empty() {
            findings.push(Finding {
                at: Location::Field("num_substreams"),
                message: "num_substreams is 0; an Audio Element with no substream carries no audio"
                    .to_owned(),
            });
        }
        if let AudioElementType::ChannelBased(config) = &self.audio_element_type {
            let layers = &config.scalable_channel_layout.layers;
            if layers.is_empty() {
                findings.push(Finding {
                    at: Location::Field("num_layers"),
                    message: "num_layers is 0; a channel-based element needs at least one layer"
                        .to_owned(),
                });
            }
            let declared: usize = layers
                .iter()
                .map(|layer| usize::from(layer.substream_count))
                .sum();
            if declared != self.audio_substream_ids.len() {
                findings.push(Finding {
                    at: Location::Field("substream_count"),
                    message: format!(
                        "the layers declare {declared} substreams but num_substreams is {}",
                        self.audio_substream_ids.len()
                    ),
                });
            }
        }
        for param in &self.params {
            if param.param_definition_type() == 0 {
                findings.push(Finding {
                    at: Location::Field("param_definition_type"),
                    message: "param_definition_type 0 is a Mix Gain definition and belongs to a \
                              Mix Presentation, not an Audio Element"
                        .to_owned(),
                });
            }
        }
        findings
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AudioElementObu::ReadAndValidatePayloadDerived
/// Read an Audio Element payload from a **bounded** payload reader.
///
/// Every count — `num_substreams`, `num_parameters`, `num_layers` — is checked
/// against `bytes_remaining()` before anything is reserved (T-01-24). Each of
/// those fields drives an allocation and every one of them is
/// attacker-controlled.
pub fn read_audio_element(r: &mut BitCursor<'_>) -> Result<AudioElement> {
    let audio_element_id = r.read_uleb128()?;
    let type_value = u8::try_from(r.read_unsigned(3)?).unwrap_or(0);
    let _reserved = r.read_unsigned(5)?;
    let codec_config_id = r.read_uleb128()?;

    let audio_substream_ids = read_counted(r, |r| r.read_uleb128())?;
    let params = read_counted(r, read_audio_element_param)?;

    let audio_element_type = match type_value {
        0 => AudioElementType::ChannelBased(ChannelBasedConfig {
            scalable_channel_layout: read_scalable_channel_layout_config(r)?,
        }),
        1 => AudioElementType::SceneBased(read_ambisonics_config(r)?),
        value => {
            // D-05: `raw` consumes to the end of the payload, so `trailing`
            // below — and the OBU-level drain above it — stay empty. The
            // length is derivable only because `obu_size` bounded this reader.
            let remaining = r.bytes_remaining();
            AudioElementType::Reserved {
                value,
                raw: r.read_uint8_span(remaining)?.to_vec(),
            }
        }
    };

    let remaining = r.bytes_remaining();
    let trailing = r.read_uint8_span(remaining)?.to_vec();

    Ok(AudioElement {
        audio_element_id,
        audio_element_type,
        codec_config_id,
        audio_substream_ids,
        params,
        trailing,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AudioElementObu::ValidateAndWritePayload
/// Write an Audio Element payload, `trailing` last.
pub fn write_audio_element(w: &mut BitWriter, v: &AudioElement) -> Result<()> {
    w.write_uleb128_minimal(v.audio_element_id)?;
    w.write_unsigned(u64::from(v.audio_element_type.value()), 3)?;
    w.write_unsigned(0, 5)?;
    w.write_uleb128_minimal(v.codec_config_id)?;

    write_count(w, v.audio_substream_ids.len(), "num_substreams")?;
    for id in &v.audio_substream_ids {
        w.write_uleb128_minimal(*id)?;
    }
    write_count(w, v.params.len(), "num_parameters")?;
    for param in &v.params {
        write_audio_element_param(w, param)?;
    }

    match &v.audio_element_type {
        AudioElementType::ChannelBased(config) => {
            write_scalable_channel_layout_config(w, &config.scalable_channel_layout)?;
        }
        AudioElementType::SceneBased(config) => write_ambisonics_config(w, config)?,
        AudioElementType::Reserved { raw, .. } => w.write_bytes(raw)?,
    }
    w.write_bytes(&v.trailing)
}

// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadULeb128
// NOTE: the reference has no such helper — it repeats the count-then-loop
// shape at each site. The citation points at the primitive this factors over,
// because the thing a reviewer must check here is the bound, not the read.
/// Read a uleb128 count, bounds-check it against the input, then read that many
/// items.
///
/// One helper for every count in this OBU, so the check cannot be present at
/// three sites and forgotten at the fourth. The bound is `bytes_remaining()`:
/// the smallest item any of these loops reads is one byte, so a count above it
/// is unreadable by construction and must not reserve.
fn read_counted<T, F>(r: &mut BitCursor<'_>, mut item: F) -> Result<Vec<T>>
where
    F: FnMut(&mut BitCursor<'_>) -> Result<T>,
{
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
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(item(r)?);
    }
    Ok(out)
}

// ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteUleb128
/// Write a derived count as a uleb128, refusing one that cannot be represented.
fn write_count(w: &mut BitWriter, count: usize, field: &'static str) -> Result<()> {
    let count = u32::try_from(count)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::Field(field)))?;
    w.write_uleb128_minimal(count)
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AudioElementParam::ReadAndValidate
/// Read one `audio_element_params` entry.
fn read_audio_element_param(r: &mut BitCursor<'_>) -> Result<AudioElementParam> {
    let param_definition_type = r.read_uleb128()?;
    match param_definition_type {
        PARAM_DEFINITION_DEMIXING => {
            let definition = read_param_definition(r)?;
            let default_dmixp_mode = u8::try_from(r.read_unsigned(3)?).unwrap_or(0);
            let _reserved = r.read_unsigned(5)?;
            let default_w = u8::try_from(r.read_unsigned(4)?).unwrap_or(0);
            let _reserved = r.read_unsigned(4)?;
            Ok(AudioElementParam::Demixing {
                definition,
                default_dmixp_mode,
                default_w,
            })
        }
        PARAM_DEFINITION_RECON_GAIN => Ok(AudioElementParam::ReconGain {
            definition: read_param_definition(r)?,
        }),
        other => {
            let start = r.byte_position();
            let size = r.read_uleb128()?;
            let size = usize::try_from(size)
                .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
            Ok(AudioElementParam::Extension {
                param_definition_type: other,
                bytes: r.read_uint8_span(size)?.to_vec(),
            })
        }
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AudioElementParam::Write
/// Write one `audio_element_params` entry.
fn write_audio_element_param(w: &mut BitWriter, v: &AudioElementParam) -> Result<()> {
    w.write_uleb128_minimal(v.param_definition_type())?;
    match v {
        AudioElementParam::Demixing {
            definition,
            default_dmixp_mode,
            default_w,
        } => {
            write_param_definition(w, definition)?;
            w.write_unsigned(u64::from(*default_dmixp_mode), 3)?;
            w.write_unsigned(0, 5)?;
            w.write_unsigned(u64::from(*default_w), 4)?;
            w.write_unsigned(0, 4)
        }
        AudioElementParam::ReconGain { definition } => write_param_definition(w, definition),
        AudioElementParam::Extension { bytes, .. } => {
            write_count(w, bytes.len(), "param_definition_size")?;
            w.write_bytes(bytes)
        }
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc ScalableChannelLayoutConfig::ReadAndValidate
/// Read `num_layers` (3 bits, 5 reserved) and that many layer configs.
fn read_scalable_channel_layout_config(
    r: &mut BitCursor<'_>,
) -> Result<ScalableChannelLayoutConfig> {
    let start = r.byte_position();
    let num_layers = usize::try_from(r.read_unsigned(3)?)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
    let _reserved = r.read_unsigned(5)?;
    // Bounds-check before reserving: a layer is at least three bytes, so a
    // count above the bytes left is unreadable by construction.
    if num_layers > r.bytes_remaining() {
        return Err(Error::new(
            ErrorKind::UnexpectedEndOfInput,
            Location::InputOffset(start),
        ));
    }
    let mut layers = Vec::with_capacity(num_layers);
    for _ in 0..num_layers {
        layers.push(read_channel_audio_layer_config(r)?);
    }
    Ok(ScalableChannelLayoutConfig { layers })
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc ScalableChannelLayoutConfig::Write
/// Write `num_layers` and the layers.
fn write_scalable_channel_layout_config(
    w: &mut BitWriter,
    v: &ScalableChannelLayoutConfig,
) -> Result<()> {
    let num_layers = u64::try_from(v.layers.len())
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::Field("num_layers")))?;
    w.write_unsigned(num_layers, 3)?;
    w.write_unsigned(0, 5)?;
    for layer in &v.layers {
        write_channel_audio_layer_config(w, layer)?;
    }
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc ChannelAudioLayerConfig::ReadAndValidate
/// Read one layer: `loudspeaker_layout(4)`, the two gate flags, two reserved
/// bits, the two substream counts, then the gated output gain and — only when
/// `loudspeaker_layout == 15` — `expanded_loudspeaker_layout`.
fn read_channel_audio_layer_config(r: &mut BitCursor<'_>) -> Result<ChannelAudioLayerConfig> {
    let layout_bits = u8::try_from(r.read_unsigned(4)?).unwrap_or(0);
    let output_gain_is_present = r.read_bool()?;
    let recon_gain_is_present = r.read_bool()?;
    let _reserved_a = r.read_unsigned(2)?;
    let substream_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    let coupled_substream_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);

    let output_gain = if output_gain_is_present {
        let flags = u8::try_from(r.read_unsigned(6)?).unwrap_or(0);
        let _reserved_b = r.read_unsigned(2)?;
        let gain = i16::try_from(r.read_signed(16)?).unwrap_or(0);
        Some(OutputGain { flags, gain })
    } else {
        None
    };

    let loudspeaker_layout = match LoudspeakerLayout::from_value(layout_bits) {
        Some(layout) => layout,
        None => {
            // `from_value` returns None for exactly one value — 15 — and the
            // expanded byte that completes it follows here.
            let expanded = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
            LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::from_value(expanded))
        }
    };

    Ok(ChannelAudioLayerConfig {
        loudspeaker_layout,
        output_gain,
        recon_gain_is_present,
        substream_count,
        coupled_substream_count,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc ChannelAudioLayerConfig::Write
/// Write one layer. Both gate flags come from the data they gate, never from a
/// stored copy — see [`ChannelAudioLayerConfig`] for why `recon_gain` is the
/// documented exception.
fn write_channel_audio_layer_config(
    w: &mut BitWriter,
    v: &ChannelAudioLayerConfig,
) -> Result<()> {
    w.write_unsigned(u64::from(v.loudspeaker_layout.value()), 4)?;
    w.write_bool(v.output_gain_is_present())?;
    w.write_bool(v.recon_gain_is_present())?;
    w.write_unsigned(0, 2)?;
    w.write_unsigned(u64::from(v.substream_count), 8)?;
    w.write_unsigned(u64::from(v.coupled_substream_count), 8)?;
    if let Some(gain) = v.output_gain {
        w.write_unsigned(u64::from(gain.flags), 6)?;
        w.write_unsigned(0, 2)?;
        w.write_signed(i64::from(gain.gain), 16)?;
    }
    if let Some(expanded) = v.loudspeaker_layout.expanded() {
        w.write_unsigned(u64::from(expanded.value()), 8)?;
    }
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AmbisonicsConfig::ReadAndValidate
/// Read `ambisonics_mode` and the config it selects.
///
/// A reserved mode reads **nothing further**, matching the reference: the rest
/// of the payload is the OBU's remainder, not part of this config.
fn read_ambisonics_config(r: &mut BitCursor<'_>) -> Result<AmbisonicsConfig> {
    let start = r.byte_position();
    let mode = r.read_uleb128()?;
    match mode {
        0 => {
            let output_channel_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
            let substream_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
            let mapping_len = usize::from(output_channel_count);
            if mapping_len > r.bytes_remaining() {
                return Err(Error::new(
                    ErrorKind::UnexpectedEndOfInput,
                    Location::InputOffset(start),
                ));
            }
            Ok(AmbisonicsConfig::Mono(AmbisonicsMonoConfig {
                output_channel_count,
                substream_count,
                channel_mapping: r.read_uint8_span(mapping_len)?.to_vec(),
            }))
        }
        1 => {
            let output_channel_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
            let substream_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
            let coupled_substream_count = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
            let rows = usize::from(substream_count)
                .checked_add(usize::from(coupled_substream_count))
                .ok_or_else(|| {
                    Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start))
                })?;
            let entries = rows
                .checked_mul(usize::from(output_channel_count))
                .ok_or_else(|| {
                    Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start))
                })?;
            // Two bytes per entry, checked before reserving.
            let bytes_needed = entries.checked_mul(2).ok_or_else(|| {
                Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start))
            })?;
            if bytes_needed > r.bytes_remaining() {
                return Err(Error::new(
                    ErrorKind::UnexpectedEndOfInput,
                    Location::InputOffset(start),
                ));
            }
            let mut demixing_matrix = Vec::with_capacity(entries);
            for _ in 0..entries {
                demixing_matrix.push(i16::try_from(r.read_signed(16)?).unwrap_or(0));
            }
            Ok(AmbisonicsConfig::Projection(AmbisonicsProjectionConfig {
                output_channel_count,
                substream_count,
                coupled_substream_count,
                demixing_matrix,
            }))
        }
        mode => Ok(AmbisonicsConfig::Reserved { mode }),
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AmbisonicsConfig::Write
/// Write `ambisonics_mode` and the config it selects.
fn write_ambisonics_config(w: &mut BitWriter, v: &AmbisonicsConfig) -> Result<()> {
    w.write_uleb128_minimal(v.mode())?;
    match v {
        AmbisonicsConfig::Mono(config) => {
            w.write_unsigned(u64::from(config.output_channel_count), 8)?;
            w.write_unsigned(u64::from(config.substream_count), 8)?;
            w.write_bytes(&config.channel_mapping)
        }
        AmbisonicsConfig::Projection(config) => {
            w.write_unsigned(u64::from(config.output_channel_count), 8)?;
            w.write_unsigned(u64::from(config.substream_count), 8)?;
            w.write_unsigned(u64::from(config.coupled_substream_count), 8)?;
            for entry in &config.demixing_matrix {
                w.write_signed(i64::from(*entry), 16)?;
            }
            Ok(())
        }
        AmbisonicsConfig::Reserved { .. } => Ok(()),
    }
}
