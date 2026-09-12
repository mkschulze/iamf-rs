//! OBU framing — the header every OBU type shares, and the structural walk over
//! a whole `.iamf` byte stream.
//!
//! # The one `trailing` drain site (OBU-07), and its precedence rule (D-05)
//!
//! Every OBU carries a `trailing: Vec<u8>` holding whatever its type-specific
//! parser did not consume. It is filled in exactly one place — [`read_obu_with`]
//! below — rather than once per OBU type, because a per-type drain is a
//! per-type opportunity to forget one, and an under-read is otherwise
//! completely silent.
//!
//! **Two mechanisms will claim "everything after the fields I understood", and
//! their precedence is defined.** `trailing` is the **OBU-level** remainder:
//! `obu_size` minus whatever the type-specific parser read. A *payload-level*
//! raw-bytes field — the one plan 01-05 gives an Audio Element type this crate
//! does not model — consumes to the end of **its own** payload, which means it
//! consumes to the end of the OBU payload too, and `trailing` is then left
//! empty. The payload-level mechanism wins; `trailing` is what is left when
//! nothing else claimed it.
//!
//! Without that rule the boundary between the two is ambiguous and Phase 2's
//! decode→encode byte-identity property becomes untestable — which is the only
//! property that can prove we preserved something we did not understand.

mod audio_element;
mod audio_frame;
mod boundaries;
mod codec_config;
mod header;
mod mix_presentation;
mod param_definition;
mod parameter_block;
mod sequence_header;
mod temporal_delimiter;

pub use audio_element::{
    AudioElement, AudioElementParam, AudioElementType, ChannelAudioLayerConfig, ChannelBasedConfig,
    OutputGain, PARAM_DEFINITION_DEMIXING, PARAM_DEFINITION_RECON_GAIN,
    ScalableChannelLayoutConfig, read_audio_element, write_audio_element,
};
pub use audio_frame::{
    AudioFrame, FramePlan, MAX_IMPLICIT_SUBSTREAM_ID, obu_type_for, plan_frames, read_audio_frame,
    substream_id_for, validate_temporal_unit, write_audio_frame,
};
pub use boundaries::find_obu_boundaries;
pub use codec_config::{
    AacLcDecoderConfig, CODEC_ID_AAC, CODEC_ID_FLAC, CODEC_ID_LPCM, CODEC_ID_OPUS, CodecConfig,
    DecoderConfig, FlacDecoderConfig, LpcmDecoderConfig, MAX_SAMPLES_PER_FRAME, OpusDecoderConfig,
    SampleFormatFlags, read_codec_config, required_audio_roll_distance,
    required_opus_audio_roll_distance, write_codec_config,
};
pub use header::{ObuHeader, ObuType, Trimming, TypeSpecific, read_obu_header, write_obu};
pub use mix_presentation::{
    AnchorElement, AnchoredLoudness, HeadphonesRenderingMode, Layout, LayoutWithLoudness, Loudness,
    LoudnessExtension, MixGainParamDefinition, MixPresentation, RenderingConfig, SubMix,
    SubMixAudioElement, read_mix_presentation, write_mix_presentation,
};
pub use param_definition::{
    DurationFields, ParamDefinition, ParamDefinitionRegistry, ParameterDataContext,
    RegisteredParamDefinition, read_param_definition, write_param_definition,
};
pub use parameter_block::{
    AnimationType, BlockDurationFields, DemixingInfoParameterData, MixGainParameterData,
    ParamDefinitionType, ParameterBlock, ParameterData, ParameterSubblock, ReconGainElement,
    ReconGainInfoParameterData, read_parameter_block, write_parameter_block,
};
pub use sequence_header::{
    IA_CODE, IaSequenceHeader, PROFILE_COUNT, read_ia_sequence_header, write_ia_sequence_header,
};
pub use temporal_delimiter::{
    TemporalDelimiter, read_temporal_delimiter, write_temporal_delimiter,
};

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Location, Result};

/// One OBU: its header, its parsed payload, and the OBU-level remainder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obu<T> {
    /// The common header.
    pub header: ObuHeader,
    /// Whatever the type-specific parser produced.
    pub payload: T,
    /// Bytes inside `obu_size` that the type-specific parser did not consume.
    /// Empty on every freshly constructed OBU, and appended **last** on write.
    pub trailing: Vec<u8>,
}

impl<T> Obu<T> {
    /// A freshly constructed OBU, with nothing trailing.
    #[must_use]
    pub const fn new(header: ObuHeader, payload: T) -> Self {
        Self {
            header,
            payload,
            trailing: Vec::new(),
        }
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
/// Read one whole OBU: the header, then the type-specific payload through a
/// bounded sub-reader (Pattern 3), then the OBU-level remainder.
///
/// This is **the** `trailing` drain site (OBU-07). The sub-reader is what makes
/// an under-read visible at all: the child cannot read past `obu_size` into the
/// next OBU, and whatever it left behind is exactly what `trailing` holds. The
/// parent is advanced past the whole payload either way, so the caller is
/// standing on the next OBU's first byte regardless of how much the payload
/// parser understood.
pub fn read_obu_with<T, F>(r: &mut BitCursor<'_>, parse: F) -> Result<Obu<T>>
where
    F: FnOnce(&mut BitCursor<'_>) -> Result<T>,
{
    read_obu_with_header(r, |_header, payload| parse(payload))
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
/// As [`read_obu_with`], but handing the already-parsed header to the payload
/// parser.
///
/// Some payloads cannot be parsed without their header: an Audio Frame's
/// `obu_type` is what decides whether an `audio_substream_id` field is present
/// at all (TIME-01). Rather than let such a parser read the header itself —
/// which would give the crate a second `trailing` drain site, the thing OBU-07
/// exists to prevent — the header is passed in and the single drain below
/// stays single.
pub fn read_obu_with_header<T, F>(r: &mut BitCursor<'_>, parse: F) -> Result<Obu<T>>
where
    F: FnOnce(&ObuHeader, &mut BitCursor<'_>) -> Result<T>,
{
    let before = r.byte_position();
    let (header, obu_size, after_size_bytes) = header::read_obu_header_parts(r)?;

    // `obu_size` counts the after-size fields as well as the payload, so the
    // payload length is what is left of it once those are subtracted. The
    // after-size count is measured by the header reader rather than recomputed
    // here, so the two can never disagree.
    let payload_len = u64::from(obu_size)
        .checked_sub(after_size_bytes)
        .ok_or_else(|| Error::new(ErrorKind::TruncatedObu, Location::InputOffset(before)))?;
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(before)))?;

    let mut payload_reader = r.sub_reader(payload_len)?;
    let payload = parse(&header, &mut payload_reader)?;

    // The one drain. Anything the type-specific parser did not claim is the
    // OBU-level remainder — see the module comment for its precedence against a
    // payload-level remainder.
    let remaining = payload_reader.bytes_remaining();
    let trailing = payload_reader.read_uint8_span(remaining)?.to_vec();

    Ok(Obu {
        header,
        payload,
        trailing,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite
/// Write one whole OBU, appending `trailing` **last** — after the type-specific
/// payload — so an OBU read with an under-reading parser re-serialises to the
/// bytes it came from.
pub fn write_obu_with<T, F>(w: &mut BitWriter, obu: &Obu<T>, write_payload: F) -> Result<()>
where
    F: FnOnce(&mut BitWriter, &T) -> Result<()>,
{
    write_obu_with_header(w, obu, |writer, _header, payload| {
        write_payload(writer, payload)
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite
/// As [`write_obu_with`], but handing the header to the payload writer.
///
/// The mirror of [`read_obu_with_header`], and adjacent to it for the same
/// reason: an Audio Frame writer must see `obu_type` to know whether to emit
/// an explicit `audio_substream_id`, and read and write have to agree on that
/// or the crate produces files it cannot read back.
pub fn write_obu_with_header<T, F>(w: &mut BitWriter, obu: &Obu<T>, write_payload: F) -> Result<()>
where
    F: FnOnce(&mut BitWriter, &ObuHeader, &T) -> Result<()>,
{
    let mut payload = BitWriter::new();
    write_payload(&mut payload, &obu.header, &obu.payload)?;
    let mut bytes = payload.finish()?;
    bytes.extend_from_slice(&obu.trailing);
    write_obu(w, &obu.header, &bytes)
}
