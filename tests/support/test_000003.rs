//! The published configuration of `test_000003`, as one shared model.
//!
//! Every value below is transcribed from
//! `tests/fixtures/reference/test_000003.textproto` — the *published
//! configuration* the reference encoder was given — and nothing here was
//! captured from this crate's own output. Research CORRECTION 2 is what makes
//! that possible: the configuration is published, so reproducing the file is a
//! mechanical translation rather than archaeology.
//!
//! It lives in `tests/support/` rather than inside one test file because two
//! integration binaries need it — `tests/descriptors.rs` proves the 120-byte
//! prologue and `tests/sequence.rs` proves the whole 32567-byte file — and a
//! second transcription is a second thing to keep in step with the textproto.
//! Files under a subdirectory of `tests/` are not themselves compiled as test
//! targets, so this module runs no tests of its own.
//!
//! `dead_code` is allowed because each including binary uses a different subset;
//! this is a `tests/` allowance and has no bearing on the D-21 escape census,
//! which counts float-lint escapes in `src/` only.

#![allow(dead_code)]

use iamf::bits::BitWriter;
use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::model::{DescriptorSet, write_descriptors};
use iamf::obu::{
    AudioElement, ChannelAudioLayerConfig, CodecConfig, IaSequenceHeader, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixPresentation, Obu,
    ObuHeader, ObuType, RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix,
    SubMixAudioElement,
};

/// The vendored reference file the whole suite is measured against.
pub const TEST_000003: &[u8] = include_bytes!("../fixtures/reference/test_000003.iamf");

/// The WAV `audio_frame_metadata` names as the source of the PCM.
pub const SAWTOOTH_WAV: &[u8] = include_bytes!("../fixtures/reference/sawtooth_100_stereo.wav");

/// `test_000003.iamf` is 32567 bytes — `0x7F37`.
pub const FILE_LEN: usize = 32567;

/// The descriptor prologue is 120 bytes, `0x00..0x78`, which corrects
/// `PROJECT.md`'s 118. The first Audio Frame OBU therefore begins at 120.
pub const PROLOGUE_LEN: usize = 120;

/// `codec_config { num_samples_per_frame: 128 }`.
pub const SAMPLES_PER_FRAME: u32 = 128;

/// `audio_frame_metadata { samples_to_trim_at_end: 64 }`, and equivalently
/// `63 * 128 - 8000`.
pub const TRIM_AT_END: u32 = 64;

/// The `test_000003` IA Sequence Header, as its textproto publishes it:
/// `primary_profile: PROFILE_VERSION_SIMPLE`, `additional_profile:
/// PROFILE_VERSION_SIMPLE`.
pub fn published_sequence_header() -> Obu<IaSequenceHeader> {
    Obu::new(
        ObuHeader::new(ObuType::IaSequenceHeader),
        IaSequenceHeader::new(0, 0),
    )
}

/// The `test_000003` Codec Config, as its textproto publishes it.
pub fn published_codec_config() -> Obu<CodecConfig> {
    Obu::new(
        ObuHeader::new(ObuType::CodecConfig),
        CodecConfig::lpcm(
            200,
            SAMPLES_PER_FRAME,
            LpcmDecoderConfig {
                sample_format_flags: SampleFormatFlags::LittleEndian,
                sample_size: 16,
                sample_rate: 16000,
            },
        ),
    )
}

/// The `test_000003` Audio Element, as its textproto publishes it: channel
/// based, one substream (id 0), one stereo layer, both gate flags clear.
pub fn published_audio_element() -> Obu<AudioElement> {
    Obu::new(
        ObuHeader::new(ObuType::AudioElement),
        AudioElement::channel_based(
            300,
            200,
            vec![0],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Stereo,
                1,
                1,
            )),
        ),
    )
}

/// A mode-1 Mix Gain definition with `parameter_id` 100 at 16 kHz, exactly as
/// `test_000003` publishes both of its mandatory definitions.
pub fn published_mix_gain() -> MixGainParamDefinition {
    MixGainParamDefinition::mode_1(100, 16000)
}

/// The `test_000003` Mix Presentation, as its textproto publishes it.
pub fn published_mix_presentation() -> Obu<MixPresentation> {
    Obu::new(
        ObuHeader::new(ObuType::MixPresentation),
        MixPresentation {
            mix_presentation_id: 42,
            annotations_language: vec![b"en-us".to_vec()],
            localized_presentation_annotations: vec![b"test_mix_pres".to_vec()],
            sub_mixes: vec![SubMix {
                elements: vec![SubMixAudioElement {
                    audio_element_id: 300,
                    localized_element_annotations: vec![
                        b"test_sub_mix_0_audio_element_0".to_vec(),
                    ],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: published_mix_gain(),
                }],
                output_mix_gain: published_mix_gain(),
                layouts: vec![LayoutWithLoudness {
                    layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                    // -13733 / 256 == -53.64453125 LUFS, and
                    // -12879 / 256 == -50.30859375 dBFS. `tests/profile.rs`
                    // proves `lufs_to_q7_8` reproduces both from those inputs.
                    loudness: Loudness::new(-13733, -12879),
                }],
            }],
            trailing: Vec::new(),
        },
    )
}

/// The whole `test_000003` descriptor prologue as a model.
pub fn published_descriptor_set() -> DescriptorSet {
    DescriptorSet {
        sequence_header: published_sequence_header().payload,
        codec_configs: vec![published_codec_config().payload],
        audio_elements: vec![published_audio_element().payload],
        mix_presentations: vec![published_mix_presentation().payload],
    }
}

/// Serialise a whole descriptor set.
pub fn descriptor_bytes(set: &DescriptorSet) -> Vec<u8> {
    let mut w = BitWriter::new();
    let written = write_descriptors(&mut w, set);
    assert!(written.is_ok(), "the descriptor set serialises: {written:?}");
    w.finish().unwrap_or_default()
}

/// The `data` chunk of `sawtooth_100_stereo.wav`: 32000 bytes, which is 8000
/// sample frames of 16-bit stereo at 16 kHz.
///
/// Read by locating the chunk rather than trusting a magic offset, and asserted
/// against the format fields, so a re-vendored WAV cannot silently shift every
/// Audio Frame payload.
pub fn sawtooth_pcm() -> &'static [u8] {
    // "RIFF" .... "WAVE" "fmt " <16> <fmt> "data" <len>
    assert_eq!(SAWTOOTH_WAV.get(0..4), Some(b"RIFF".as_slice()));
    assert_eq!(SAWTOOTH_WAV.get(8..12), Some(b"WAVE".as_slice()));
    assert_eq!(SAWTOOTH_WAV.get(12..16), Some(b"fmt ".as_slice()));
    // 2 channels, 16000 Hz, 16 bits per sample.
    assert_eq!(SAWTOOTH_WAV.get(22..24), Some([0x02, 0x00].as_slice()));
    assert_eq!(SAWTOOTH_WAV.get(24..28), Some([0x80, 0x3e, 0x00, 0x00].as_slice()));
    assert_eq!(SAWTOOTH_WAV.get(34..36), Some([0x10, 0x00].as_slice()));
    assert_eq!(SAWTOOTH_WAV.get(36..40), Some(b"data".as_slice()));

    let len = SAWTOOTH_WAV
        .get(40..44)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .unwrap_or(0);
    assert_eq!(len, 32_000, "8000 sample frames of 16-bit stereo");
    SAWTOOTH_WAV.get(44..44 + len).unwrap_or_default()
}

/// Assert byte equality, naming the **first differing offset** and dumping a
/// window around it.
///
/// "32567 bytes differ" is unactionable; the offset immediately identifies
/// which OBU is wrong, because every OBU boundary in this file is known.
pub fn assert_bytes_eq(produced: &[u8], expected: &[u8], what: &str) {
    if produced == expected {
        return;
    }
    let first = produced
        .iter()
        .zip(expected.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| produced.len().min(expected.len()));

    let start = first.saturating_sub(16);
    let end = (first + 16).min(produced.len().max(expected.len()));
    panic!(
        "{what}: first difference at offset {first} (0x{first:x})\n\
         produced len {} expected len {}\n\
         produced[0x{start:x}..0x{end:x}] = {}\n\
         expected[0x{start:x}..0x{end:x}] = {}",
        produced.len(),
        expected.len(),
        hex_window(produced, start, end),
        hex_window(expected, start, end),
    );
}

/// A space-separated hex dump of `bytes[start..end]`, clamped.
fn hex_window(bytes: &[u8], start: usize, end: usize) -> String {
    let end = end.min(bytes.len());
    let start = start.min(end);
    bytes
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
