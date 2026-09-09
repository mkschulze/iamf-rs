//! D-20's annotated structural dumper — one deterministic, diff-friendly text
//! rendering of any `.iamf` byte slice.
//!
//! # This dump aids review. It never establishes correctness.
//!
//! Stated in the imperative because the temptation is real. The dumper parses
//! with the same readers the writer mirrors, so it is **self-consistent with
//! this crate by construction**: a field this crate writes wrongly and reads
//! back wrongly renders here as a tidy, plausible line. The same caveat applies
//! to the committed golden hash. Both are *change detection* — they make an
//! output change a reviewable PR diff, which is GUARD-09's whole stated
//! rationale and which neither a bare hash (a one-line diff that explains
//! nothing) nor a bare binary blob delivers.
//!
//! Conformance evidence comes from exactly two places: the two reference
//! oracles (`libiamf`'s `iamfdec` and `iamf-tools`' `decoder_main` /
//! `encoder_main`), and hand-decoded byte vectors. Never from here.
//!
//! # What is dumped, and what is not
//!
//! Every OBU gets a header line carrying its absolute start offset, its type,
//! its `obu_size` and its flags; then one line per decoded field; then its raw
//! bytes in a 16-per-row hex block carrying absolute offsets.
//!
//! The field lines are rendered from the parsed **model** and carry the OBU's
//! offset rather than each field's own. That is a deliberate limit: a per-field
//! offset would require a second walker over the wire layout — a duplicate of
//! the parser, and therefore a second thing to keep in step with `iamf-tools`,
//! which is the failure this crate spends most of its comments avoiding. The
//! hex block is what locates a wrong byte, and it is exact.
//!
//! An Audio Frame's payload is summarised rather than dumped: a 5.1 fixture's
//! frames are most of the file, and a hex dump of every sample would bury the
//! structure the dump exists to show. Its first bytes are printed, which is
//! enough to see an endianness or packing change.
//!
//! A Parameter Block is dumped as raw bytes with a stated reason: its payload
//! cannot be parsed without the governing `ParamDefinition` from the
//! descriptors, and threading that through would make the dumper stateful.
//! Recorded rather than silently skipped.
//!
//! # Determinism is a hard requirement here
//!
//! This text is committed and compared on four targets. There is no timestamp,
//! no absolute path, no address, no hashed container, and no iteration over
//! anything but a `Vec` in bitstream order.

use crate::bits::BitCursor;
use crate::error::Result;
use crate::model::layout::LoudspeakerLayout;
use crate::obu::{
    AudioElement, AudioElementParam, AudioElementType, CodecConfig, DecoderConfig,
    IaSequenceHeader, Layout, LayoutWithLoudness, Loudness, MixGainParamDefinition,
    MixPresentation, ObuHeader, ObuType, ParamDefinition, SubMix, TypeSpecific,
    find_obu_boundaries, read_audio_element, read_audio_frame, read_codec_config,
    read_ia_sequence_header, read_mix_presentation, read_obu_header, read_obu_with,
    read_obu_with_header, substream_id_for,
};

/// How many payload bytes of an Audio Frame are shown.
const FRAME_PREVIEW_BYTES: usize = 16;

/// How many bytes of a non-frame OBU are shown in its hex block before the
/// block is elided. Descriptors are small; this only bites on a pathological
/// one, and eliding is stated rather than silent.
const HEX_BLOCK_MAX: usize = 256;

/// Render `bytes` as a deterministic, annotated structural dump.
///
/// # Errors
///
/// Whatever [`find_obu_boundaries`] reports for a malformed OBU chain — an
/// `obu_size` above the reference's ceiling, or one that would carry an OBU
/// past the end of the buffer. A payload this crate cannot *parse* is rendered
/// with a stated reason rather than failing the whole dump: a dumper that
/// refuses to dump the thing you are debugging is no use.
pub fn dump_annotated(bytes: &[u8]) -> Result<String> {
    let boundaries = find_obu_boundaries(bytes)?;
    // The last boundary is the end offset, so the OBU starts are all but the
    // last. `saturating_sub` because an empty input yields `[0]`.
    let obu_count = boundaries.len().saturating_sub(1);

    let mut out = String::new();
    out.push_str(&format!(
        "IA Sequence: {obu_count} OBUs, {} bytes\n",
        bytes.len()
    ));
    out.push_str(&format!(
        "spec {}, dumped by iamf::dump — REVIEW AID, NOT CONFORMANCE EVIDENCE\n",
        crate::SPEC_VERSION
    ));
    out.push_str(
        "offset    field                             value\n\
         --------  --------------------------------  -----\n",
    );

    for (index, start) in boundaries.iter().take(obu_count).enumerate() {
        let end = boundaries
            .get(index.saturating_add(1))
            .copied()
            .unwrap_or(*start);
        let obu = bytes.get(*start..end).unwrap_or_default();
        out.push('\n');
        out.push_str(&dump_one_obu(*start, obu));
    }

    Ok(out)
}

/// One OBU: its header line, its decoded fields, and its raw bytes.
fn dump_one_obu(offset: usize, obu: &[u8]) -> String {
    let mut out = String::new();

    let mut cursor = BitCursor::new(obu);
    let Ok((header, obu_size)) = read_obu_header(&mut cursor) else {
        out.push_str(&format!(
            "{offset:08x}  <unparsable OBU header, {} bytes>\n",
            obu.len()
        ));
        out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
        return out;
    };

    out.push_str(&format!(
        "{offset:08x}  OBU {:<28}  type={} obu_size={} len={}\n",
        type_name(header.obu_type),
        header.obu_type.value(),
        obu_size,
        obu.len()
    ));
    out.push_str(&field(
        offset,
        "obu_redundant_copy",
        header.obu_redundant_copy,
    ));
    match header.type_specific {
        TypeSpecific::Trimming(Some(trimming)) => {
            out.push_str(&field(offset, "obu_trimming_status_flag", true));
            // END before START, which is the wire order and the one thing a
            // fixture with equal trims could never catch.
            out.push_str(&field(
                offset,
                "num_samples_to_trim_at_end",
                trimming.at_end,
            ));
            out.push_str(&field(
                offset,
                "num_samples_to_trim_at_start",
                trimming.at_start,
            ));
        }
        TypeSpecific::Trimming(None) => {
            out.push_str(&field(offset, "obu_trimming_status_flag", false));
        }
        TypeSpecific::Reserved => {}
    }
    if let Some(extension) = header.extension.as_ref() {
        out.push_str(&field(
            offset,
            "extension_header_bytes",
            format!("{} bytes {}", extension.len(), hex_inline(extension, 16)),
        ));
    }

    out.push_str(&dump_payload(offset, obu, &header));
    out
}

/// The decoded payload of one OBU, and its raw bytes.
fn dump_payload(offset: usize, obu: &[u8], header: &ObuHeader) -> String {
    let mut out = String::new();
    let mut cursor = BitCursor::new(obu);

    match header.obu_type {
        ObuType::IaSequenceHeader => match read_obu_with(&mut cursor, read_ia_sequence_header) {
            Ok(parsed) => {
                out.push_str(&dump_sequence_header(offset, &parsed.payload));
                out.push_str(&dump_trailing(offset, &parsed.trailing));
                out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
            }
            Err(e) => out.push_str(&unparsed(offset, obu, &format!("{:?}", e.kind()))),
        },
        ObuType::CodecConfig => match read_obu_with(&mut cursor, read_codec_config) {
            Ok(parsed) => {
                out.push_str(&dump_codec_config(offset, &parsed.payload));
                out.push_str(&dump_trailing(offset, &parsed.trailing));
                out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
            }
            Err(e) => out.push_str(&unparsed(offset, obu, &format!("{:?}", e.kind()))),
        },
        ObuType::AudioElement => match read_obu_with(&mut cursor, read_audio_element) {
            Ok(parsed) => {
                out.push_str(&dump_audio_element(offset, &parsed.payload));
                out.push_str(&dump_trailing(offset, &parsed.trailing));
                out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
            }
            Err(e) => out.push_str(&unparsed(offset, obu, &format!("{:?}", e.kind()))),
        },
        ObuType::MixPresentation => match read_obu_with(&mut cursor, read_mix_presentation) {
            Ok(parsed) => {
                out.push_str(&dump_mix_presentation(offset, &parsed.payload));
                out.push_str(&dump_trailing(offset, &parsed.trailing));
                out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
            }
            Err(e) => out.push_str(&unparsed(offset, obu, &format!("{:?}", e.kind()))),
        },
        ObuType::TemporalDelimiter => {
            out.push_str(&field(offset, "(no payload)", "temporal delimiter"));
            out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
        }
        ObuType::ParameterBlock => {
            // Recorded rather than silently skipped: the payload needs the
            // governing ParamDefinition from the descriptors, and threading
            // that through would make the dumper stateful.
            out.push_str(&field(
                offset,
                "(payload not decoded)",
                "a Parameter Block cannot be parsed without its governing ParamDefinition",
            ));
            out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
        }
        ObuType::Reserved(value) => {
            out.push_str(&field(
                offset,
                "(payload not decoded)",
                format!("obu_type {value} is not defined by this spec version"),
            ));
            out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
        }
        _ => {
            // Audio Frames: types 5 and 6..=23.
            match read_obu_with_header(&mut cursor, read_audio_frame) {
                Ok(parsed) => {
                    let implicit = substream_id_for(header.obu_type);
                    out.push_str(&field(
                        offset,
                        "audio_substream_id",
                        parsed.payload.substream_id,
                    ));
                    out.push_str(&field(
                        offset,
                        "  id source",
                        if implicit.is_some() {
                            "implicit in obu_type (6 + substream_index)"
                        } else {
                            "explicit field (obu_type 5)"
                        },
                    ));
                    out.push_str(&field(
                        offset,
                        "audio_frame (bytes)",
                        parsed.payload.payload.len(),
                    ));
                    out.push_str(&field(
                        offset,
                        "audio_frame[..16]",
                        hex_inline(&parsed.payload.payload, FRAME_PREVIEW_BYTES),
                    ));
                    out.push_str(&dump_trailing(offset, &parsed.trailing));
                    // Deliberately no hex block: a 5.1 fixture's frames are most
                    // of the file and would bury the structure. The preview above
                    // is enough to see an endianness or packing change.
                }
                Err(e) => out.push_str(&unparsed(offset, obu, &format!("{:?}", e.kind()))),
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Per-descriptor renderings
// ---------------------------------------------------------------------------

fn dump_sequence_header(offset: usize, value: &IaSequenceHeader) -> String {
    let mut out = String::new();
    out.push_str(&field(
        offset,
        "ia_code",
        format!("0x{:08x} {:?}", value.ia_code, ia_code_text(value.ia_code)),
    ));
    out.push_str(&field(offset, "primary_profile", value.primary_profile));
    out.push_str(&field(
        offset,
        "additional_profile",
        value.additional_profile,
    ));
    out
}

/// `ia_code` as its four ASCII bytes, for readability. Never used for a
/// comparison — the numeric value above is the one that matters.
fn ia_code_text(code: u32) -> String {
    code.to_be_bytes()
        .iter()
        .map(|byte| {
            if byte.is_ascii_graphic() {
                char::from(*byte)
            } else {
                '.'
            }
        })
        .collect()
}

fn dump_codec_config(offset: usize, value: &CodecConfig) -> String {
    let mut out = String::new();
    out.push_str(&field(offset, "codec_config_id", value.codec_config_id));
    out.push_str(&field(
        offset,
        "codec_id",
        format!("{:?}", String::from_utf8_lossy(&value.codec_id)),
    ));
    out.push_str(&field(
        offset,
        "num_samples_per_frame",
        value.num_samples_per_frame,
    ));
    out.push_str(&field(
        offset,
        "audio_roll_distance",
        value.audio_roll_distance,
    ));
    match &value.decoder_config {
        DecoderConfig::Lpcm(lpcm) => {
            out.push_str(&field(
                offset,
                "  sample_format_flags",
                format!(
                    "{} ({})",
                    lpcm.sample_format_flags.value(),
                    // DESC-03: zero is BIG-endian, the opposite of a WAV-shaped
                    // assumption. Spelled out because a bare `0` here is exactly
                    // what a reader mis-reads.
                    match lpcm.sample_format_flags.value() {
                        0 => "big-endian",
                        1 => "little-endian",
                        _ => "reserved",
                    }
                ),
            ));
            out.push_str(&field(offset, "  sample_size", lpcm.sample_size));
            out.push_str(&field(offset, "  sample_rate", lpcm.sample_rate));
        }
        DecoderConfig::Flac(flac) => {
            out.push_str(&field(
                offset,
                "  last_metadata_block",
                flac.last_metadata_block,
            ));
            out.push_str(&field(
                offset,
                "  metadata_block_type",
                flac.metadata_block_type,
            ));
            out.push_str(&field(
                offset,
                "  metadata_data_block_length",
                flac.metadata_data_block_length,
            ));
            out.push_str(&field(
                offset,
                "  minimum_block_size",
                flac.minimum_block_size,
            ));
            out.push_str(&field(
                offset,
                "  maximum_block_size",
                flac.maximum_block_size,
            ));
            out.push_str(&field(
                offset,
                "  minimum_frame_size",
                flac.minimum_frame_size,
            ));
            out.push_str(&field(
                offset,
                "  maximum_frame_size",
                flac.maximum_frame_size,
            ));
            out.push_str(&field(offset, "  sample_rate", flac.sample_rate));
            out.push_str(&field(
                offset,
                "  number_of_channels",
                flac.number_of_channels,
            ));
            out.push_str(&field(offset, "  bits_per_sample", flac.bits_per_sample));
            out.push_str(&field(
                offset,
                "  total_samples_in_stream",
                flac.total_samples_in_stream,
            ));
            out.push_str(&field(
                offset,
                "  md5_signature",
                hex_inline(&flac.md5_signature, flac.md5_signature.len()),
            ));
        }
        DecoderConfig::Opus(opus) => {
            out.push_str(&field(offset, "  version", opus.version));
            out.push_str(&field(
                offset,
                "  output_channel_count",
                opus.output_channel_count,
            ));
            out.push_str(&field(offset, "  pre_skip", opus.pre_skip));
            out.push_str(&field(
                offset,
                "  input_sample_rate",
                opus.input_sample_rate,
            ));
            out.push_str(&field(offset, "  output_gain", opus.output_gain));
            out.push_str(&field(offset, "  mapping_family", opus.mapping_family));
        }
        DecoderConfig::Raw { codec_id, bytes } => {
            out.push_str(&field(
                offset,
                "  decoder_config (raw)",
                format!(
                    "codec_id {:?}, {} bytes {}",
                    String::from_utf8_lossy(codec_id),
                    bytes.len(),
                    hex_inline(bytes, 24)
                ),
            ));
        }
    }
    out
}

fn dump_audio_element(offset: usize, value: &AudioElement) -> String {
    let mut out = String::new();
    out.push_str(&field(offset, "audio_element_id", value.audio_element_id));
    out.push_str(&field(
        offset,
        "audio_element_type",
        format!(
            "{} ({})",
            value.audio_element_type.value(),
            match &value.audio_element_type {
                AudioElementType::ChannelBased(_) => "channel-based",
                AudioElementType::SceneBased(_) => "scene-based",
                AudioElementType::Reserved { .. } => "reserved",
            }
        ),
    ));
    out.push_str(&field(offset, "audio_element_reserved", value.reserved));
    out.push_str(&field(offset, "codec_config_id", value.codec_config_id));
    out.push_str(&field(
        offset,
        "num_substreams",
        value.audio_substream_ids.len(),
    ));
    for (index, id) in value.audio_substream_ids.iter().enumerate() {
        out.push_str(&field(
            offset,
            &format!("  audio_substream_id[{index}]"),
            id,
        ));
    }
    out.push_str(&field(offset, "num_parameters", value.params.len()));
    for (index, param) in value.params.iter().enumerate() {
        out.push_str(&field(
            offset,
            &format!("  param_definition_type[{index}]"),
            param.param_definition_type(),
        ));
        match param {
            AudioElementParam::Demixing {
                definition,
                default_dmixp_mode,
                default_reserved,
                default_w,
                default_w_reserved,
            } => {
                out.push_str(&dump_param_definition(offset, "    ", definition));
                out.push_str(&field(offset, "    default_dmixp_mode", default_dmixp_mode));
                out.push_str(&field(
                    offset,
                    "    default_demixing_reserved",
                    default_reserved,
                ));
                out.push_str(&field(offset, "    default_w", default_w));
                out.push_str(&field(offset, "    default_w_reserved", default_w_reserved));
            }
            AudioElementParam::ReconGain { definition } => {
                out.push_str(&dump_param_definition(offset, "    ", definition));
            }
            AudioElementParam::Extension { .. } => {}
        }
    }

    if let AudioElementType::ChannelBased(config) = &value.audio_element_type {
        let layers = &config.scalable_channel_layout.layers;
        out.push_str(&field(offset, "num_layers", layers.len()));
        out.push_str(&field(
            offset,
            "scalable_channel_layout_reserved",
            config.scalable_channel_layout.reserved,
        ));
        for (index, layer) in layers.iter().enumerate() {
            let prefix = format!("  layer[{index}]");
            out.push_str(&field(
                offset,
                &format!("{prefix} loudspeaker_layout"),
                format!(
                    "{} ({})",
                    layer.loudspeaker_layout.value(),
                    layout_name(layer.loudspeaker_layout)
                ),
            ));
            out.push_str(&field(
                offset,
                &format!("{prefix} substream_count"),
                layer.substream_count,
            ));
            out.push_str(&field(
                offset,
                &format!("{prefix} coupled_substream_count"),
                layer.coupled_substream_count,
            ));
            out.push_str(&field(
                offset,
                &format!("{prefix} output_gain_is_present"),
                layer.output_gain_is_present(),
            ));
            out.push_str(&field(
                offset,
                &format!("{prefix} recon_gain_is_present"),
                layer.recon_gain_is_present(),
            ));
            out.push_str(&field(
                offset,
                &format!("{prefix} reserved"),
                layer.reserved,
            ));
            if let Some(gain) = layer.output_gain.as_ref() {
                out.push_str(&field(
                    offset,
                    &format!("{prefix} output_gain"),
                    format!("flags={} gain={}", gain.flags, gain.gain),
                ));
                out.push_str(&field(
                    offset,
                    &format!("{prefix} output_gain_reserved"),
                    gain.reserved,
                ));
            }
        }
    }
    out
}

fn dump_mix_presentation(offset: usize, value: &MixPresentation) -> String {
    let mut out = String::new();
    out.push_str(&field(
        offset,
        "mix_presentation_id",
        value.mix_presentation_id,
    ));
    out.push_str(&field(offset, "count_label", value.count_label()));
    for (index, language) in value.annotations_language.iter().enumerate() {
        out.push_str(&field(
            offset,
            &format!("  annotations_language[{index}]"),
            format!("{:?}", String::from_utf8_lossy(language)),
        ));
    }
    for (index, annotation) in value.localized_presentation_annotations.iter().enumerate() {
        out.push_str(&field(
            offset,
            &format!("  localized_presentation_annotation[{index}]"),
            format!("{:?}", String::from_utf8_lossy(annotation)),
        ));
    }
    out.push_str(&field(offset, "num_sub_mixes", value.sub_mixes.len()));
    for (index, sub_mix) in value.sub_mixes.iter().enumerate() {
        out.push_str(&dump_sub_mix(offset, index, sub_mix));
    }
    out
}

fn dump_sub_mix(offset: usize, index: usize, sub_mix: &SubMix) -> String {
    let mut out = String::new();
    let prefix = format!("  sub_mix[{index}]");
    out.push_str(&field(
        offset,
        &format!("{prefix} num_audio_elements"),
        sub_mix.elements.len(),
    ));
    for (element_index, element) in sub_mix.elements.iter().enumerate() {
        let element_prefix = format!("{prefix} element[{element_index}]");
        out.push_str(&field(
            offset,
            &format!("{element_prefix} audio_element_id"),
            element.audio_element_id,
        ));
        for (annotation_index, annotation) in
            element.localized_element_annotations.iter().enumerate()
        {
            out.push_str(&field(
                offset,
                &format!("{element_prefix} annotation[{annotation_index}]"),
                format!("{:?}", String::from_utf8_lossy(annotation)),
            ));
        }
        out.push_str(&field(
            offset,
            &format!("{element_prefix} headphones_rendering_mode"),
            element.rendering_config.headphones_rendering_mode.value(),
        ));
        out.push_str(&field(
            offset,
            &format!("{element_prefix} rendering_config_reserved"),
            element.rendering_config.reserved,
        ));
        out.push_str(&field(
            offset,
            &format!("{element_prefix} rendering_config_extension"),
            format!("{} bytes", element.rendering_config.extension.len()),
        ));
        out.push_str(&dump_mix_gain(
            offset,
            &format!("{element_prefix} element_mix_gain"),
            &element.element_mix_gain,
        ));
    }
    out.push_str(&dump_mix_gain(
        offset,
        &format!("{prefix} output_mix_gain"),
        &sub_mix.output_mix_gain,
    ));
    out.push_str(&field(
        offset,
        &format!("{prefix} num_layouts"),
        sub_mix.layouts.len(),
    ));
    for (layout_index, layout) in sub_mix.layouts.iter().enumerate() {
        out.push_str(&dump_layout(
            offset,
            &format!("{prefix} layout[{layout_index}]"),
            layout,
        ));
    }
    out
}

fn dump_mix_gain(offset: usize, prefix: &str, gain: &MixGainParamDefinition) -> String {
    let mut out = dump_param_definition(offset, &format!("{prefix} "), &gain.definition);
    out.push_str(&field(
        offset,
        &format!("{prefix} default_mix_gain"),
        q7_8(gain.default_mix_gain),
    ));
    out
}

fn dump_param_definition(offset: usize, prefix: &str, definition: &ParamDefinition) -> String {
    let mut out = String::new();
    out.push_str(&field(
        offset,
        &format!("{prefix}parameter_id"),
        definition.parameter_id,
    ));
    out.push_str(&field(
        offset,
        &format!("{prefix}parameter_rate"),
        definition.parameter_rate,
    ));
    out.push_str(&field(
        offset,
        &format!("{prefix}param_definition_mode"),
        definition.param_definition_mode(),
    ));
    out.push_str(&field(
        offset,
        &format!("{prefix}param_definition_reserved"),
        definition.reserved,
    ));
    if let Some(duration) = definition.duration_fields.as_ref() {
        out.push_str(&field(
            offset,
            &format!("{prefix}duration"),
            duration.duration,
        ));
        out.push_str(&field(
            offset,
            &format!("{prefix}constant_subblock_duration"),
            duration.constant_subblock_duration,
        ));
        out.push_str(&field(
            offset,
            &format!("{prefix}num_subblocks"),
            duration.subblock_durations.len(),
        ));
    }
    out
}

fn dump_layout(offset: usize, prefix: &str, layout: &LayoutWithLoudness) -> String {
    let mut out = String::new();
    out.push_str(&field(
        offset,
        &format!("{prefix} layout_type"),
        layout.layout.layout_type(),
    ));
    if let Layout::SoundSystem(system) = layout.layout {
        out.push_str(&field(
            offset,
            &format!("{prefix} sound_system"),
            format!("{} ({system:?})", system.value()),
        ));
    }
    out.push_str(&field(
        offset,
        &format!("{prefix} reserved"),
        layout.reserved,
    ));
    out.push_str(&dump_loudness(offset, prefix, &layout.loudness));
    out
}

fn dump_loudness(offset: usize, prefix: &str, loudness: &Loudness) -> String {
    let mut out = String::new();
    out.push_str(&field(
        offset,
        &format!("{prefix} info_type"),
        loudness.info_type(),
    ));
    out.push_str(&field(
        offset,
        &format!("{prefix} integrated_loudness"),
        q7_8(loudness.integrated),
    ));
    out.push_str(&field(
        offset,
        &format!("{prefix} digital_peak"),
        q7_8(loudness.digital_peak),
    ));
    if let Some(true_peak) = loudness.true_peak {
        out.push_str(&field(
            offset,
            &format!("{prefix} true_peak"),
            q7_8(true_peak),
        ));
    }
    if let Some(anchored) = loudness.anchored.as_ref() {
        out.push_str(&field(
            offset,
            &format!("{prefix} num_anchored_loudness"),
            anchored.anchor_elements.len(),
        ));
        for (index, anchor) in anchored.anchor_elements.iter().enumerate() {
            out.push_str(&field(
                offset,
                &format!("{prefix} anchor[{index}]"),
                format!(
                    "element={} loudness={}",
                    anchor.anchor_element,
                    q7_8(anchor.anchored_loudness)
                ),
            ));
        }
    }
    if let Some(extension) = loudness.extension.as_ref() {
        out.push_str(&field(
            offset,
            &format!("{prefix} info_type_extension"),
            format!(
                "bits=0x{:02x} {} bytes",
                extension.info_type_bits,
                extension.bytes.len()
            ),
        ));
    }
    out
}

/// A Q7.8 value as its raw `i16` plus an exact decimal.
///
/// **No floating-point type appears here, deliberately.** `raw / 256` and
/// `raw % 256` are exact in integers, and a float would be a GUARD-11 escape in
/// `src/` bought for a cosmetic reason — `tests/profile.rs`'s D-21 census scans
/// this directory as raw text, comments included, so even naming the type in a
/// comment would trip it. That bluntness is the point: the census's correct
/// answer is exactly one file, and PROF-03's helper is it. The sign is handled
/// explicitly so `-1/256` renders as `-0.00390625` rather than `0.-390625`.
fn q7_8(raw: i16) -> String {
    let value = i32::from(raw);
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let whole = magnitude.checked_div(256).unwrap_or(0);
    let fraction = magnitude.checked_rem(256).unwrap_or(0);
    // 8 fractional bits, so the exact decimal has at most 8 places:
    // fraction/256 = fraction * 390625 / 100000000.
    let scaled = u64::from(fraction).saturating_mul(390_625);
    let sign = if negative { "-" } else { "" };
    format!("{sign}{whole}.{scaled:08} ({raw})")
}

/// Whatever the OBU-level parser did not claim (OBU-07).
fn dump_trailing(offset: usize, trailing: &[u8]) -> String {
    if trailing.is_empty() {
        return String::new();
    }
    field(
        offset,
        "obu_trailing",
        format!("{} bytes {}", trailing.len(), hex_inline(trailing, 32)),
    )
}

/// An OBU whose payload this crate could not parse: say so, name the error,
/// and dump the bytes anyway.
fn unparsed(offset: usize, obu: &[u8], reason: &str) -> String {
    let mut out = field(offset, "(payload not decoded)", reason);
    out.push_str(&hex_block(offset, obu, HEX_BLOCK_MAX));
    out
}

/// The name of an OBU type, for the header line.
fn type_name(obu_type: ObuType) -> String {
    match obu_type {
        ObuType::CodecConfig => "CodecConfig".to_owned(),
        ObuType::AudioElement => "AudioElement".to_owned(),
        ObuType::MixPresentation => "MixPresentation".to_owned(),
        ObuType::ParameterBlock => "ParameterBlock".to_owned(),
        ObuType::TemporalDelimiter => "TemporalDelimiter".to_owned(),
        ObuType::AudioFrame => "AudioFrame(explicit id)".to_owned(),
        ObuType::IaSequenceHeader => "IaSequenceHeader".to_owned(),
        ObuType::Reserved(value) => format!("Reserved({value})"),
        other => match substream_id_for(other) {
            Some(id) => format!("AudioFrame(substream {id})"),
            None => format!("Unknown({})", other.value()),
        },
    }
}

/// The name of a loudspeaker layout, so a reader does not have to decode a
/// 4-bit integer by hand.
fn layout_name(layout: LoudspeakerLayout) -> String {
    format!("{layout:?}")
}

// ---------------------------------------------------------------------------
// Formatting primitives
// ---------------------------------------------------------------------------

/// One field line: the containing OBU's offset, the field name, the value.
///
/// Fixed-width columns so a diff of two dumps lines up and a changed value is a
/// one-line change rather than a reflow.
fn field(offset: usize, name: &str, value: impl std::fmt::Display) -> String {
    format!("{offset:08x}  {name:<32}  {value}\n")
}

/// Up to `limit` bytes as space-separated hex, with an elision marker when
/// there are more. Never wraps, so it stays on one field line.
fn hex_inline(bytes: &[u8], limit: usize) -> String {
    let shown = bytes.get(..limit.min(bytes.len())).unwrap_or_default();
    let mut text = shown
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    if bytes.len() > shown.len() {
        text.push_str(&format!(
            " … (+{} more)",
            bytes.len().saturating_sub(shown.len())
        ));
    }
    text
}

/// A canonical hex block: 16 bytes per row, each row carrying its **absolute**
/// offset in the file.
///
/// This is what locates a wrong byte. The field lines above name what a byte
/// means; this says exactly where it is.
fn hex_block(offset: usize, bytes: &[u8], limit: usize) -> String {
    let shown = bytes.get(..limit.min(bytes.len())).unwrap_or_default();
    let mut out = String::new();
    for (row, chunk) in shown.chunks(16).enumerate() {
        let at = offset.saturating_add(row.saturating_mul(16));
        let hex = chunk
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let ascii: String = chunk
            .iter()
            .map(|byte| {
                if byte.is_ascii_graphic() {
                    char::from(*byte)
                } else {
                    '.'
                }
            })
            .collect();
        out.push_str(&format!("{at:08x}  | {hex:<47}  |{ascii}|\n"));
    }
    if bytes.len() > shown.len() {
        out.push_str(&format!(
            "{:08x}  | … {} further bytes elided at {HEX_BLOCK_MAX}\n",
            offset.saturating_add(shown.len()),
            bytes.len().saturating_sub(shown.len())
        ));
    }
    out
}
