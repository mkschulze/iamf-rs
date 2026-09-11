//! The Phase 1 conformance fixtures — D-18 as amended, D-19's no-permutation
//! channel diagnostic, and the one encode path every test drives.
//!
//! It lives in `tests/support/` rather than at the top of `tests/` because
//! three integration binaries need it — `tests/fixture.rs` asserts its
//! properties, `tests/conformance.rs` drives it through the reference oracles,
//! and `tests/golden.rs` compares it against the committed golden — and a
//! second copy is a second thing to keep in step. Files under a subdirectory of
//! `tests/` are not compiled as test targets, so **nothing here is a `#[test]`**:
//! a `#[test]` in this file would be collected into all three including
//! binaries and run three times.
//!
//! # Why the fixtures are what they are
//!
//! D-18 has been amended twice and every property below survives an attempt to
//! "simplify" it only if the reason is written next to it. Each one detects a
//! specific bug.
//!
//! **Three fixtures, not one.** The gate is split across them because two
//! separate findings forced it, both recorded in `CONFORMANCE-GATE.md`:
//!
//! | fixture | shape | clauses | forced by |
//! |---|---|---|---|
//! | [`sample_identity`] | one 5.1 Audio Element, one Codec Config, 48 kHz, 24-bit **little**-endian | CONF-02, CONF-03, CONF-05 | Experiment 3 |
//! | [`endianness`] | one stereo Audio Element, 48 kHz, 16-bit **big**-endian | DESC-03's `sample_format_flags == 0` sample identity | Experiment 3 |
//! | [`structure_only`] | **two** Audio Elements, 48 kHz, 16-bit little-endian | CONF-04 — OBU count, descriptor ordering, the boundary walk | Experiment 2 |
//!
//! The coupling that made "one fixture, seven clauses" valuable is genuinely
//! weakened by the split. What preserves the gate's meaning is that the
//! *sample-identity* half — the half `libiamf`'s acceptance defines — stays
//! whole and un-permuted.

#![allow(dead_code)]

use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::model::{select_minimum_profile, DescriptorSet};
use iamf::obu::{
    plan_frames, AudioElement, AudioElementType, AudioFrame, ChannelAudioLayerConfig, CodecConfig,
    IaSequenceHeader, Layout, LayoutWithLoudness, Loudness, LpcmDecoderConfig,
    MixGainParamDefinition, MixPresentation, RenderingConfig, SampleFormatFlags,
    ScalableChannelLayoutConfig, SubMix, SubMixAudioElement, Trimming, CODEC_ID_OPUS,
};
use iamf::packing::{pack_channels_to_substreams, SubstreamPlan};
use iamf::sequence::{SequenceWriter, TemporalUnit};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/// `num_samples_per_frame` for every Phase 1 fixture.
pub const FRAME_SIZE: u32 = 128;

/// Every fixture is 48 kHz, which is also `iamfdec`'s `-r` default — so a
/// harness that forgot to pass `-r` would still be right here, and CONF-08's
/// 16 kHz `test_000003` reproduction is the case that catches a missing one.
pub const SAMPLE_RATE: u32 = 48_000;

/// **A deliberate non-multiple of [`FRAME_SIZE`].** 300 = 2×128 + 44, so the
/// file carries 3 frames and the last one trims 84 samples at the end.
///
/// This is the only configuration that can catch a swapped END/START write
/// order: LPCM has no priming, so `trim_at_start = 0` and `trim_at_end = 84`
/// **differ**. Whenever the two are equal, both orders produce identical bytes
/// and no golden file can see the difference (OBU-05, CONF-03).
pub const SAMPLE_FRAMES: usize = 300;

/// `trim_at_end` the above implies: `3 × 128 − 300`.
pub const EXPECTED_TRIM_AT_END: u32 = 84;

/// Sound System B (0+5+0) output order, per ITU-R BS.2051-3.
///
/// **This is the order the fixture's PCM is authored in**, which is what makes
/// D-19 work: the 5.1 → BS2051_B render matrix in `libiamf` is the 6×6
/// identity, so the decoder's output *is* our input and the comparison is
/// direct equality with nothing to tune.
///
/// `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_layout.c:72-73;
/// code/src/iamf_dec/m2m_rdr.c:106-108, 1222]`
/// `[CITED: iamf-tools@v2.1.0 iamf/cli/testdata/README.md, "Output WAV files"]`
pub const CHANNELS_5_1: [&str; 6] = ["L", "R", "C", "LFE", "Ls", "Rs"];

/// Sound System A (0+2+0) output order.
pub const CHANNELS_STEREO: [&str; 2] = ["L", "R"];

/// The channel names for a layout, for the D-19 diagnostic's messages.
pub fn channel_names(layout: LoudspeakerLayout) -> &'static [&'static str] {
    match layout {
        LoudspeakerLayout::Ch5_1 => &CHANNELS_5_1,
        _ => &CHANNELS_STEREO,
    }
}

// ---------------------------------------------------------------------------
// The signal
// ---------------------------------------------------------------------------

/// Half of full scale at `sample_size` bits — `2^(n-2)`, which is −6 dBFS at
/// every depth.
///
/// **A cap, not a suggestion.** `libiamf` creates a −1 dBTP peak limiter
/// unconditionally in `IAMF_decoder_open()` (1 ms attack, 200 ms release,
/// 240-sample look-ahead, linear threshold ≈ 0.8913). Below threshold
/// `compute_target_gain()` returns exactly 1.0 and the path is bit-exact; above
/// it the sample-identity assertion fails with a diffuse, amplitude-only,
/// channel-uniform error that reads exactly like a rounding or endianness bug
/// in our own encoder.
///
/// Raising the amplitude "to make the signal more distinguishable" reintroduces
/// that failure. The harness also passes `-disable_limiter`; both measures are
/// deliberate and neither replaces the other.
pub fn peak_for(sample_size: u8) -> i32 {
    let shift = u32::from(sample_size).saturating_sub(2);
    1_i32.checked_shl(shift).unwrap_or(0x0040_0000)
}

/// A per-channel deterministic ramp, folded into `[-peak, +peak]`.
///
/// Channel `c` advances by `STEP` per sample from a base of `c * OFFSET`.
/// `OFFSET` is neither zero nor a multiple of the fold span, so two channels
/// differ at **every** sample index — which is what makes a swapped channel
/// arithmetically identifiable rather than merely "PCM differs" (CONF-02,
/// D-19). A constant or slowly-varying signal would make a swap invisible, and
/// so would a signal whose channels ever coincide.
pub fn ramp_sample(channel: usize, index: usize, peak: i32) -> i32 {
    // `STEP` is a prime large enough to sweep the whole fold span several times
    // over [`SAMPLE_FRAMES`] samples. That matters: a small step leaves the
    // signal in one corner of its range — with a step of 7919 the 24-bit ramp
    // never crosses zero in 300 frames — and a signal that never exercises its
    // high bytes is a signal a wrong high byte can hide in.
    //
    // `OFFSET` separates the channels. It is coprime with the fold span at both
    // 16 and 24 bits and smaller than either span divided by the channel count,
    // so no two of the six channels ever coincide at any index — asserted by
    // `the_ramp_distinguishes_every_channel_at_every_index`.
    const STEP: i64 = 1_299_709;
    const OFFSET: i64 = 104_729;
    let span = i64::from(peak).saturating_mul(2).saturating_add(1);
    let base = i64::try_from(channel).unwrap_or(0).saturating_mul(OFFSET);
    let walk = i64::try_from(index).unwrap_or(0).saturating_mul(STEP);
    let folded = base.saturating_add(walk).checked_rem(span).unwrap_or(0);
    i32::try_from(folded).unwrap_or(0).saturating_sub(peak)
}

/// `frames × channels` interleaved samples of the ramp, in the layout's
/// documented output order.
pub fn ramp_pcm(frames: usize, channels: usize, peak: i32) -> Vec<i32> {
    let mut pcm = Vec::with_capacity(frames.saturating_mul(channels));
    for index in 0..frames {
        for channel in 0..channels {
            pcm.push(ramp_sample(channel, index, peak));
        }
    }
    pcm
}

// ---------------------------------------------------------------------------
// Sample storage — the one place endianness is expressed
// ---------------------------------------------------------------------------

/// Store one sample big- or little-endian at `sample_size` bits.
///
/// A byte copy driven by a flag; no arithmetic is performed on the sample
/// value. **`sample_format_flags == 0` is BIG-endian** (DESC-03), which is the
/// opposite of a WAV-shaped assumption and the sense a three-byte sample gets
/// written wrong.
pub fn store_sample(value: i32, sample_size: u8, big_endian: bool, into: &mut Vec<u8>) {
    let width = usize::from(sample_size).div_ceil(8);
    let be = value.to_be_bytes();
    // The low `width` bytes of the big-endian representation, most significant
    // first. For 24-bit that is `be[1..4]`.
    let start = 4_usize.saturating_sub(width);
    let slice = be.get(start..4).unwrap_or_default();
    if big_endian {
        into.extend_from_slice(slice);
    } else {
        into.extend(slice.iter().rev().copied());
    }
}

/// Interleaved samples → interleaved stored bytes in the declared endianness.
pub fn store_interleaved(samples: &[i32], sample_size: u8, big_endian: bool) -> Vec<u8> {
    let width = usize::from(sample_size).div_ceil(8);
    let mut bytes = Vec::with_capacity(samples.len().saturating_mul(width));
    for value in samples {
        store_sample(*value, sample_size, big_endian, &mut bytes);
    }
    bytes
}

// ---------------------------------------------------------------------------
// Deriving the encode parameters from the descriptors — never from a constant
// ---------------------------------------------------------------------------

/// Everything needed to turn one Audio Element's interleaved PCM into frames,
/// all of it read back out of the Codec Config that element references.
///
/// CONF-01 turns on this: the rate, sample size and frame size the harness
/// hands the reference come from the Codec Config it just wrote, so nothing is
/// LPCM-specific by construction and Phase 3 reuses the harness unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureCodec {
    Lpcm { big_endian: bool },
    Flac,
    Opus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementSpec {
    pub sample_rate: u32,
    pub sample_size: u8,
    pub num_samples_per_frame: u32,
    pub codec: FixtureCodec,
    pub layout: LoudspeakerLayout,
    pub channels: usize,
}

impl ElementSpec {
    /// Bytes per stored sample.
    pub fn bytes_per_sample(self) -> usize {
        usize::from(self.sample_size).div_ceil(8)
    }
}

/// The `loudspeaker_layout` of an element's first channel layer.
pub fn first_layout(element: &AudioElement) -> Result<LoudspeakerLayout, String> {
    match &element.audio_element_type {
        AudioElementType::ChannelBased(config) => config
            .scalable_channel_layout
            .layers
            .first()
            .map(|layer| layer.loudspeaker_layout)
            .ok_or_else(|| "the channel layout has no layers".to_owned()),
        other => Err(format!(
            "only channel-based elements are encoded by this harness; got {other:?}"
        )),
    }
}

/// The spec one Audio Element implies, resolved through the descriptor set.
pub fn spec_for(
    descriptors: &DescriptorSet,
    element: &AudioElement,
) -> Result<ElementSpec, String> {
    let config = descriptors
        .codec_config_by_id(element.codec_config_id)
        .ok_or_else(|| {
            format!(
                "no Codec Config carries id {}, which Audio Element {} references",
                element.codec_config_id, element.audio_element_id
            )
        })?;
    let (sample_rate, sample_size, codec) = if let Some(lpcm) = config.lpcm_config() {
        (
            lpcm.sample_rate,
            lpcm.sample_size,
            FixtureCodec::Lpcm {
                big_endian: matches!(lpcm.sample_format_flags, SampleFormatFlags::BigEndian),
            },
        )
    } else if let Some(flac) = config.flac_config() {
        (
            flac.sample_rate,
            flac.bits_per_sample.saturating_add(1),
            FixtureCodec::Flac,
        )
    } else if config.codec_id == CODEC_ID_OPUS {
        // IAMF Opus always decodes on a 48 kHz clock and the committed oracle
        // is signed 16-bit PCM. The typed config lands in Plan 03; selecting
        // these output properties by FourCC keeps this adapter compatible with
        // both today's Raw form and that typed form without inspecting packets.
        (48_000, 16, FixtureCodec::Opus)
    } else {
        return Err(format!(
            "Codec Config {} uses unsupported codec {:?}",
            config.codec_config_id, config.codec_id
        ));
    };
    let layout = first_layout(element)?;
    let plan = SubstreamPlan::for_layout(layout).map_err(|e| format!("{e:?}"))?;
    Ok(ElementSpec {
        sample_rate,
        sample_size,
        num_samples_per_frame: config.num_samples_per_frame,
        codec,
        layout,
        channels: plan.channel_count(),
    })
}

/// The spec of the descriptor set's **first** Audio Element.
pub fn spec_of_first(descriptors: &DescriptorSet) -> Result<ElementSpec, String> {
    let element = descriptors
        .audio_elements
        .first()
        .ok_or_else(|| "the descriptor set has no Audio Element".to_owned())?;
    spec_for(descriptors, element)
}

// ---------------------------------------------------------------------------
// A fixture, and the single encode path
// ---------------------------------------------------------------------------

/// One already-encoded temporal unit, with opaque payloads in substream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedTemporalUnit {
    pub trimming: Option<Trimming>,
    pub substream_payloads: Vec<Vec<u8>>,
}

/// The committed raw Opus corpus, read by test targets only.  It retains both
/// the independent standalone decode and the exact PCM emitted by the pinned
/// libiamf reference decoder; neither is the source PCM used to encode it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpusCorpus {
    pub lookahead: usize,
    pub packets: usize,
    pub end_trim: usize,
    pub source_frames: usize,
    pub units: Vec<EncodedTemporalUnit>,
    pub libiamf_expected: Vec<i32>,
    pub expected: Vec<i32>,
}

fn opus_corpus_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs/opus")
}

#[allow(clippy::expect_used)] // Missing committed artifacts must fail loudly.
fn opus_artifact(name: &str) -> Vec<u8> {
    std::fs::read(opus_corpus_dir().join(name)).expect("committed Opus artifact must exist")
}

/// Validate the corpus's fixed encoder metadata without accepting a near miss.
pub fn validate_opus_metadata(fields: &BTreeMap<String, String>) -> Result<(), String> {
    for (key, expected) in [
        ("sample_rate", "48000"),
        ("channels", "2"),
        ("frame_samples", "960"),
        ("application", "audio"),
        ("bitrate", "128000"),
        ("vbr", "false"),
        ("complexity", "10"),
        ("force_channels", "stereo"),
        ("max_bandwidth", "fullband"),
        ("signal", "music"),
        ("dtx", "false"),
        ("inband_fec", "false"),
        ("packet_loss_perc", "0"),
        ("lsb_depth", "16"),
    ] {
        if fields.get(key).map(String::as_str) != Some(expected) {
            return Err(format!("Opus manifest metadata {key} = {expected}"));
        }
    }
    for key in [
        "sha256.source.s16le",
        "sha256.expected.s16le",
        "sha256.libiamf-expected.s16le",
    ] {
        if !fields.contains_key(key) {
            return Err(format!("Opus manifest field {key}"));
        }
    }
    Ok(())
}

/// Require an exact committed corpus inventory.
pub fn validate_opus_inventory(actual: &[String], required: &[String]) -> Result<(), String> {
    if actual == required {
        Ok(())
    } else {
        Err("Opus corpus inventory mismatch".to_owned())
    }
}

/// Raw access units must be nonempty, uncontainerized and manifest-sized.
pub fn validate_opus_packet(bytes: &[u8], expected_len: usize) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("Opus packet is empty".to_owned());
    }
    if bytes.windows(8).any(|window| window == b"OpusHead") {
        return Err("OpusHead marker in raw packet".to_owned());
    }
    if bytes.windows(4).any(|window| window == b"OggS") {
        return Err("OggS marker in raw packet".to_owned());
    }
    if bytes.len() != expected_len {
        return Err("Opus packet length mismatch".to_owned());
    }
    Ok(())
}

/// Require digests for precisely the source, both PCM oracles, and all packets.
pub fn validate_opus_digest_keys(
    fields: &BTreeMap<String, String>,
    packet_names: &[String],
) -> Result<(), String> {
    let mut expected = BTreeSet::from([
        "sha256.source.s16le".to_owned(),
        "sha256.expected.s16le".to_owned(),
        "sha256.libiamf-expected.s16le".to_owned(),
    ]);
    expected.extend(packet_names.iter().map(|name| format!("sha256.{name}")));
    let actual = fields
        .keys()
        .filter(|key| key.starts_with("sha256."))
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err("Opus manifest digest key set mismatch".to_owned())
    }
}

#[allow(clippy::expect_used)] // A malformed committed corpus is a hard test failure.
pub fn load_opus_corpus() -> OpusCorpus {
    let text = String::from_utf8(opus_artifact("MANIFEST.md")).expect("UTF-8 Opus manifest");
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(" = ") {
            assert!(fields.insert(key.to_owned(), value.to_owned()).is_none());
        }
    }
    validate_opus_metadata(&fields).expect("complete Opus manifest metadata");
    let number = |key: &str| -> usize {
        fields
            .get(key)
            .expect("Opus manifest field")
            .parse()
            .expect("numeric Opus manifest field")
    };
    let lookahead = number("L");
    let packets = number("P");
    let end_trim = number("E");
    let source_frames = number("S");
    assert!(lookahead > 0, "Opus lookahead must be nonzero");
    let total = source_frames
        .checked_add(lookahead)
        .expect("Opus source plus lookahead overflow");
    assert_eq!(packets, total.div_ceil(960));
    assert!(
        packets >= 2,
        "Opus corpus must contain at least two packets"
    );
    let padded = packets
        .checked_mul(960)
        .expect("Opus padded length overflow");
    let expected_end_trim = padded
        .checked_sub(lookahead)
        .and_then(|value| value.checked_sub(source_frames))
        .expect("Opus trim arithmetic underflow");
    assert_eq!(end_trim, expected_end_trim);
    assert!(end_trim > 0 && end_trim < 960);
    assert_ne!(end_trim, lookahead);

    let order = fields
        .get("packet_order")
        .expect("Opus packet order")
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    let canonical = (0..packets)
        .map(|index| format!("packet-{index:03}.bin"))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        canonical.iter().map(String::as_str).collect::<Vec<_>>()
    );
    validate_opus_digest_keys(&fields, &canonical).expect("Opus manifest digest keys");
    let read_names = || {
        std::fs::read_dir(opus_corpus_dir())
            .expect("committed Opus corpus directory")
            .map(|entry| {
                entry
                    .expect("Opus corpus directory entry")
                    .file_name()
                    .into_string()
                    .expect("Opus corpus filenames must be UTF-8")
            })
            .collect::<Vec<_>>()
    };
    let mut actual = read_names()
        .into_iter()
        .filter(|name| name.starts_with("packet-") && name.ends_with(".bin"))
        .collect::<Vec<_>>();
    actual.sort();
    validate_opus_inventory(&actual, &canonical).expect("Opus packet file set");
    let mut inventory = read_names();
    inventory.sort();
    let mut required = vec![
        "MANIFEST.md".to_owned(),
        "expected.s16le".to_owned(),
        "libiamf-expected.s16le".to_owned(),
        "source.s16le".to_owned(),
    ];
    required.extend(canonical.iter().cloned());
    required.sort();
    validate_opus_inventory(&inventory, &required).expect("Opus corpus inventory");
    let mut digest_names = vec!["source.s16le", "expected.s16le", "libiamf-expected.s16le"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    digest_names.extend(canonical.iter().cloned());
    for name in digest_names {
        let digest_key = format!("sha256.{name}");
        let expected = fields
            .get(&digest_key)
            .expect("required Opus digest manifest field");
        let digest = format!("{:x}", Sha256::digest(opus_artifact(&name)));
        assert_eq!(&digest, expected, "{name}");
    }
    let mut units = Vec::with_capacity(packets);
    for (index, name) in order.iter().enumerate() {
        let bytes = opus_artifact(name);
        let packet_len = fields
            .get(&format!("packet_len.{name}"))
            .expect("Opus packet length manifest field")
            .parse::<usize>()
            .expect("numeric Opus packet length");
        validate_opus_packet(&bytes, packet_len).expect("valid Opus packet");
        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            fields.get(&format!("sha256.{name}")),
            Some(&digest),
            "{name}"
        );
        let trimming = if index == 0 {
            Some(Trimming {
                at_end: 0,
                at_start: u32::try_from(lookahead).expect("u32 Opus lookahead"),
            })
        } else if index.checked_add(1).expect("Opus packet index overflow") == packets {
            Some(Trimming {
                at_end: u32::try_from(end_trim).expect("u32 Opus end trim"),
                at_start: 0,
            })
        } else {
            None
        };
        units.push(EncodedTemporalUnit {
            trimming,
            substream_payloads: vec![bytes],
        });
    }
    let expected_bytes = opus_artifact("expected.s16le");
    let digest = format!("{:x}", Sha256::digest(&expected_bytes));
    assert_eq!(fields.get("sha256.expected.s16le"), Some(&digest));
    let expected = expected_bytes
        .chunks_exact(2)
        .map(|pair| i32::from(i16::from_le_bytes(pair.try_into().expect("two PCM bytes"))))
        .collect::<Vec<_>>();
    let expected_samples = source_frames
        .checked_mul(2)
        .expect("Opus stereo sample length overflow");
    assert_eq!(expected.len(), expected_samples, "expected stereo frames");
    let libiamf_expected_bytes = opus_artifact("libiamf-expected.s16le");
    let digest = format!("{:x}", Sha256::digest(&libiamf_expected_bytes));
    assert_eq!(
        fields.get("sha256.libiamf-expected.s16le"),
        Some(&digest)
    );
    let libiamf_expected = libiamf_expected_bytes
        .chunks_exact(2)
        .map(|pair| i32::from(i16::from_le_bytes(pair.try_into().expect("two PCM bytes"))))
        .collect::<Vec<_>>();
    assert_eq!(
        libiamf_expected.len(),
        expected_samples,
        "libiamf expected stereo frames"
    );
    OpusCorpus {
        lookahead,
        packets,
        end_trim,
        source_frames,
        units,
        libiamf_expected,
        expected,
    }
}

/// The source of one Audio Element's frames, in descriptor order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameSource {
    LpcmInterleaved(Vec<i32>),
    PreEncoded(Vec<EncodedTemporalUnit>),
}

/// One fixture: its descriptors, independently decoded comparison PCM, and
/// frame source for each Audio Element, all in descriptor order.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// A stable name, used for scratch directories and failure messages.
    pub name: &'static str,
    pub descriptors: DescriptorSet,
    /// Independently derived interleaved comparison PCM buffers.
    pub expected_pcm: Vec<Vec<i32>>,
    /// One frame source per Audio Element, in descriptor order.
    pub frame_sources: Vec<FrameSource>,
}

impl Fixture {
    /// The single Audio Element's PCM, for a one-element fixture.
    pub fn single_pcm(&self) -> &[i32] {
        self.expected_pcm.first().map_or(&[], Vec::as_slice)
    }

    /// The single Audio Element's spec.
    pub fn spec(&self) -> Result<ElementSpec, String> {
        spec_of_first(&self.descriptors)
    }

    /// How many sample frames the fixture carries.
    pub fn sample_frames(&self) -> usize {
        let channels = self.spec().map(|s| s.channels).unwrap_or(1).max(1);
        self.single_pcm().len().checked_div(channels).unwrap_or(0)
    }

    /// Encode the whole IA Sequence through the **real** [`SequenceWriter`].
    ///
    /// Deliberately not a test-only serialisation path: a harness that encoded
    /// its own fixture some other way would prove that the other way works.
    ///
    /// Every payload is paired positionally with its Audio Element's declared
    /// `audio_substream_ids`. Descriptor order need not be substream-id order,
    /// so the declarations themselves — not a synthetic global counter — are
    /// the single source of frame ids.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let elements = &self.descriptors.audio_elements;
        if elements.len() != self.frame_sources.len() {
            return Err(format!(
                "{}: {} Audio Elements but {} frame sources",
                self.name,
                elements.len(),
                self.frame_sources.len()
            ));
        }

        let lead = spec_of_first(&self.descriptors)?;
        // Per element: materialize the units once. LPCM retains the Phase 1
        // storage, padding and BCG packing byte-for-byte. Only PreEncoded takes
        // the opaque bypass and its bytes are never inspected or transformed.
        let mut prepared = Vec::with_capacity(elements.len());
        for (element, source) in elements.iter().zip(&self.frame_sources) {
            let spec = spec_for(&self.descriptors, element)?;
            if spec.num_samples_per_frame != lead.num_samples_per_frame {
                return Err(format!(
                    "{}: element {} uses {} samples per frame, element 0 uses {}. iamf-tools \
                     refuses Codec Configs with differing num_samples_per_frame",
                    self.name,
                    element.audio_element_id,
                    spec.num_samples_per_frame,
                    lead.num_samples_per_frame
                ));
            }
            let substreams =
                SubstreamPlan::for_layout(spec.layout).map_err(|e| format!("{e:?}"))?;
            let units = match source {
                FrameSource::LpcmInterleaved(pcm) => {
                    let big_endian = match spec.codec {
                        FixtureCodec::Lpcm { big_endian } => big_endian,
                        other => {
                            return Err(format!(
                                "{}: element {} uses {other:?}, so LPCM samples cannot supply it",
                                self.name, element.audio_element_id
                            ));
                        }
                    };
                    let total_samples =
                        u64::try_from(pcm.len().checked_div(spec.channels.max(1)).unwrap_or(0))
                            .unwrap_or(0);
                    let plan = plan_frames(total_samples, spec.num_samples_per_frame)
                        .map_err(|e| format!("{e:?}"))?;
                    let stored = store_interleaved(pcm, spec.sample_size, big_endian);
                    let frame_bytes = usize::try_from(spec.num_samples_per_frame)
                        .unwrap_or(0)
                        .saturating_mul(spec.channels)
                        .saturating_mul(spec.bytes_per_sample());
                    let mut units =
                        Vec::with_capacity(usize::try_from(plan.frame_count).unwrap_or(0));
                    for index in 0..plan.frame_count {
                        let start = usize::try_from(index)
                            .unwrap_or(0)
                            .saturating_mul(frame_bytes);
                        let end = start.saturating_add(frame_bytes).min(stored.len());
                        let mut chunk = stored.get(start..end).unwrap_or_default().to_vec();
                        // The final frame remains caller-padded exactly as in
                        // Phase 1; SequenceWriter never invents samples.
                        chunk.resize(frame_bytes, 0);
                        let substream_payloads = pack_channels_to_substreams(
                            &substreams,
                            &chunk,
                            spec.channels,
                            spec.bytes_per_sample(),
                        )
                        .map_err(|e| {
                            format!("{}: pack_channels_to_substreams: {e:?}", self.name)
                        })?;
                        units.push(EncodedTemporalUnit {
                            trimming: plan.trimming_for(index),
                            substream_payloads,
                        });
                    }
                    units
                }
                FrameSource::PreEncoded(units) => units.clone(),
            };

            for (index, unit) in units.iter().enumerate() {
                let declared = element.audio_substream_ids.len();
                if unit.substream_payloads.len() != declared {
                    return Err(format!(
                        "{}: element {} temporal unit {index} has {} substream payloads but \
                         declares {declared}",
                        self.name,
                        element.audio_element_id,
                        unit.substream_payloads.len()
                    ));
                }
            }
            prepared.push((
                element.audio_element_id,
                element.audio_substream_ids.as_slice(),
                units,
            ));
        }

        if let Some((lead_id, _, lead_units)) = prepared.first() {
            for (element_id, _, units) in prepared.iter().skip(1) {
                if units.len() != lead_units.len() {
                    return Err(format!(
                        "{}: element {lead_id} has {} temporal units but element {element_id} has {}",
                        self.name,
                        lead_units.len(),
                        units.len()
                    ));
                }
                for (index, (lead_unit, unit)) in lead_units.iter().zip(units).enumerate() {
                    if unit.trimming != lead_unit.trimming {
                        return Err(format!(
                            "{}: temporal unit {index} has different trimming for elements \
                             {lead_id} and {element_id}",
                            self.name
                        ));
                    }
                }
            }
        }

        let mut writer = SequenceWriter::new(Vec::new());
        writer
            .push_descriptors(&self.descriptors)
            .map_err(|e| format!("{}: push_descriptors: {e:?}", self.name))?;

        let unit_count = prepared.first().map_or(0, |(_, _, units)| units.len());
        for index in 0..unit_count {
            let mut frames = Vec::new();

            for (_, substream_ids, units) in &prepared {
                let unit = units.get(index).ok_or_else(|| {
                    format!(
                        "{}: no temporal unit {index} after count validation",
                        self.name
                    )
                })?;
                for (substream_id, payload) in
                    substream_ids.iter().copied().zip(&unit.substream_payloads)
                {
                    frames.push(
                        AudioFrame::new(substream_id, payload.clone()).into_obu(unit.trimming),
                    );
                }
            }

            writer
                .push_temporal_unit(&TemporalUnit::of_frames(frames))
                .map_err(|e| format!("{}: push_temporal_unit: {e:?}", self.name))?;
        }

        writer
            .finish()
            .map_err(|e| format!("{}: finish: {e:?}", self.name))
    }
}

// ---------------------------------------------------------------------------
// The three fixtures
// ---------------------------------------------------------------------------

/// An LPCM Codec Config at [`SAMPLE_RATE`] and [`FRAME_SIZE`].
fn lpcm_config(id: u32, sample_size: u8, flags: SampleFormatFlags) -> CodecConfig {
    CodecConfig::lpcm(
        id,
        FRAME_SIZE,
        LpcmDecoderConfig {
            sample_format_flags: flags,
            sample_size,
            sample_rate: SAMPLE_RATE,
        },
    )
}

/// A channel-based Audio Element whose declared substream counts are **derived
/// from the packing plan**, never supplied twice.
///
/// `libiamf` treats a frame count that disagrees with `substream_count` as a
/// hard error, so two sources for those two numbers is two numbers that
/// eventually differ.
// GUARD-04's `allow-unwrap-in-tests` / `allow-expect-in-tests` /
// `allow-panic-in-tests` carve-out (clippy.toml) applies to `#[test]` functions
// only — a helper reachable from tests is not one. These helpers exist solely to
// build or inspect a fixture, and a failure in them is an environment or
// programming error that must stop the run loudly rather than be swallowed.
// Kept as narrow, per-function allows so a future helper does not inherit the
// exemption silently.
#[allow(clippy::panic)]
fn channel_element(
    element_id: u32,
    codec_config_id: u32,
    layout: LoudspeakerLayout,
    first_substream_id: u32,
) -> AudioElement {
    let plan = SubstreamPlan::for_layout(layout).unwrap_or_else(|_| {
        panic!("layout {layout:?} has no Phase 1 packing plan; see src/packing.rs")
    });
    let ids: Vec<u32> = (0..u32::from(plan.substream_count()))
        .map(|n| first_substream_id.saturating_add(n))
        .collect();
    AudioElement::channel_based(
        element_id,
        codec_config_id,
        ids,
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            layout,
            plan.substream_count(),
            plan.coupled_substream_count(),
        )),
    )
}

/// The IA Sequence Header for a set of elements, with the **minimum** profile
/// the configuration fits (PROF-02) rather than a hard-coded one.
#[allow(clippy::panic)]
fn header_for(elements: &[&AudioElement]) -> IaSequenceHeader {
    let (primary, additional) = select_minimum_profile(elements)
        .unwrap_or_else(|e| panic!("the fixture's element set has no profile: {e:?}"));
    IaSequenceHeader::new(primary.to_wire(), additional.to_wire())
}

/// **CONF-02, CONF-03, CONF-05 — the sample-identity fixture.**
///
/// One 5.1 Audio Element, one Codec Config, 48 kHz, 24-bit **little**-endian,
/// 300 sample frames. Every property below detects a specific bug:
///
/// - **One Audio Element, not two.** `iamfdec` decodes a *mix presentation*,
///   rendering every element in the sub-mix into the target layout and summing
///   them, so a two-element fixture makes the decoded PCM a **mix** we could
///   only predict by rendering — and rendering is out of scope. One sub-mix,
///   one element, unity gain, target layout 5.1 ⇒ the decoder's output *is* our
///   input, and CONF-04's ordering observability moves to
///   [`structure_only`]. (D-18 as amended by Experiment 2: the original
///   two-Codec-Config design makes `decoder_main` abort on an absl `CHECK`.)
/// - **5.1, not stereo.** The phase's highest silent-failure risk is BCG
///   channel→substream packing, which produces a clean decode with scrambled
///   channels: no error, no crash, correct byte count. Stereo cannot detect it —
///   one coupled pair, zero mono channels, nothing observable. 5.1 packs as
///   L/R coupled, Ls/Rs coupled, then C and LFE mono: two pairs, two monos,
///   ordering fully observable.
/// - **24-bit, and little-endian.** Three bytes per sample is where a
///   multi-byte store gets written wrong; 16-bit hides half the mistake. The
///   endianness is *little* and not big only because `libiamf@v1.1.0`'s
///   `reads24be` misreads 24-bit big-endian — see waiver W-1 and
///   [`endianness`], which keeps DESC-03's sense at a depth the reference can
///   evaluate.
/// - **Length a non-multiple of the frame size.** Forces `trim_at_end > 0`
///   while `trim_at_start` stays 0, and a swapped END/START write order is
///   invisible whenever the two are equal.
/// - **Peak at or below −6 dBFS.** See [`peak_for`]. Raising it to "make the
///   signal more distinguishable" reintroduces a limiter failure that reads
///   like a rounding bug in our own encoder.
/// - **Two layouts, two `Loudness` blocks.** Sound System B (0+5+0) is the
///   comparison target; Sound System A (0+2+0) is **mandatory** —
///   `iamf-tools` refuses to write a sub-mix without one
///   ("Every sub-mix must have a stereo layout."). Easy to miss, because
///   `test_000003` is stereo and its single mandatory layout doubles as its
///   target. The two blocks carry deliberately *different* numbers so a swap of
///   the two layouts is visible in the bytes.
pub fn sample_identity() -> Fixture {
    let config = lpcm_config(200, 24, SampleFormatFlags::LittleEndian);
    let element = channel_element(300, 200, LoudspeakerLayout::Ch5_1, 0);
    let mix_gain = MixGainParamDefinition::mode_1(100, SAMPLE_RATE);

    let presentation = MixPresentation {
        mix_presentation_id: 42,
        annotations_language: vec![b"en-us".to_vec()],
        localized_presentation_annotations: vec![b"phase1_sample_identity".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 300,
                localized_element_annotations: vec![b"bed_5_1".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: mix_gain.clone(),
            }],
            output_mix_gain: mix_gain,
            layouts: vec![
                // The comparison target.
                LayoutWithLoudness {
                    layout: Layout::SoundSystem(SoundSystem::B0_5_0),
                    reserved: 0,
                    // -24.0 LUFS and -6.0 dBFS in Q7.8.
                    loudness: Loudness::new(-6144, -1536),
                },
                // Mandatory on write, and given DIFFERENT numbers so that
                // swapping the two layouts changes the bytes.
                LayoutWithLoudness {
                    layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                    reserved: 0,
                    // -23.0 LUFS and -5.5 dBFS in Q7.8.
                    loudness: Loudness::new(-5888, -1408),
                },
            ],
        }],
        trailing: Vec::new(),
    };

    let pcm = ramp_pcm(SAMPLE_FRAMES, CHANNELS_5_1.len(), peak_for(24));
    Fixture {
        name: "phase1_sample_identity",
        descriptors: DescriptorSet {
            sequence_header: header_for(&[&element]),
            codec_configs: vec![config],
            audio_elements: vec![element],
            mix_presentations: vec![presentation],
        },
        expected_pcm: vec![pcm.clone()],
        frame_sources: vec![FrameSource::LpcmInterleaved(pcm)],
    }
}

/// **DESC-03 — the endianness fixture.**
///
/// One stereo Audio Element, 48 kHz, 16-bit **big**-endian
/// (`sample_format_flags == 0`). It exists because waiver W-1 moved
/// [`sample_identity`] to little-endian: `reads16be` in the pinned `libiamf`
/// uses `readu16be` and is correct, so the endianness *sense* is still asserted
/// end to end through the reference — just at 16 bits rather than 24.
///
/// Stereo is sufficient here on purpose: this fixture is about the byte order
/// of a sample, not about BCG packing, which [`sample_identity`] covers.
pub fn endianness() -> Fixture {
    let config = lpcm_config(200, 16, SampleFormatFlags::BigEndian);
    let element = channel_element(300, 200, LoudspeakerLayout::Stereo, 0);
    let mix_gain = MixGainParamDefinition::mode_1(100, SAMPLE_RATE);

    let presentation = MixPresentation {
        mix_presentation_id: 42,
        annotations_language: vec![b"en-us".to_vec()],
        localized_presentation_annotations: vec![b"phase1_endianness".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 300,
                localized_element_annotations: vec![b"stereo_be".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: mix_gain.clone(),
            }],
            output_mix_gain: mix_gain,
            // One layout, which is both the target and the mandatory stereo one.
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(-6144, -1536),
            }],
        }],
        trailing: Vec::new(),
    };

    let pcm = ramp_pcm(SAMPLE_FRAMES, CHANNELS_STEREO.len(), peak_for(16));
    Fixture {
        name: "phase1_endianness",
        descriptors: DescriptorSet {
            sequence_header: header_for(&[&element]),
            codec_configs: vec![config],
            audio_elements: vec![element],
            mix_presentations: vec![presentation],
        },
        expected_pcm: vec![pcm.clone()],
        frame_sources: vec![FrameSource::LpcmInterleaved(pcm)],
    }
}

/// Three committed FLAC access units; generation is exclusively in the
/// excluded `tools/codec-fixtures` integration-test package.
#[allow(clippy::expect_used)] // Missing committed fixtures must stop tests loudly.
pub fn flac() -> Fixture {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codecs/flac");
    let bytes = std::fs::read(dir.join("expected.s16le")).expect("committed FLAC expected PCM");
    assert_eq!(bytes.len(), 1200, "300 stereo s16le frames");
    let pcm = bytes
        .chunks_exact(2)
        .map(|pair| i32::from(i16::from_le_bytes(pair.try_into().expect("two PCM bytes"))))
        .collect();
    let units = (0..3)
        .map(|index| EncodedTemporalUnit {
            trimming: (index == 2).then_some(Trimming {
                at_end: 84,
                at_start: 0,
            }),
            substream_payloads: vec![std::fs::read(dir.join(format!("packet-{index:03}.bin")))
                .expect("committed opaque FLAC frame")],
        })
        .collect();
    let config = CodecConfig::flac(200, 128, 48_000, 16).expect("valid FLAC configuration");
    let element = channel_element(300, 200, LoudspeakerLayout::Stereo, 0);
    let mix_gain = MixGainParamDefinition::mode_1(100, SAMPLE_RATE);
    let presentation = MixPresentation {
        mix_presentation_id: 42,
        annotations_language: vec![b"en-us".to_vec()],
        localized_presentation_annotations: vec![b"phase3_flac".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 300,
                localized_element_annotations: vec![b"stereo_flac".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: mix_gain.clone(),
            }],
            output_mix_gain: mix_gain,
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(-6144, -1536),
            }],
        }],
        trailing: Vec::new(),
    };
    Fixture {
        name: "phase3_flac",
        descriptors: DescriptorSet {
            sequence_header: header_for(&[&element]),
            codec_configs: vec![config],
            audio_elements: vec![element],
            mix_presentations: vec![presentation],
        },
        expected_pcm: vec![pcm],
        frame_sources: vec![FrameSource::PreEncoded(units)],
    }
}

/// The committed raw Opus corpus framed by the ordinary fixture adapter.
///
/// The corpus loader is the sole owner of the measured `L`, `P`, `E` and `S`
/// values.  Packets remain opaque: this helper only supplies the typed IAMF
/// descriptors, the loader's pinned libiamf decode oracle, and the temporal-unit
/// trim metadata that belongs beside each raw packet.  The independent decode
/// remains authenticated in the corpus for a separate oracle check.
#[allow(clippy::expect_used)] // The committed corpus has bounded manifest values.
pub fn opus() -> Fixture {
    let corpus = load_opus_corpus();
    let config = CodecConfig::opus(
        200,
        960,
        SAMPLE_RATE,
        u16::try_from(corpus.lookahead).expect("Opus lookahead fits pre_skip"),
    )
    .expect("valid measured Opus configuration");
    let element = channel_element(300, 200, LoudspeakerLayout::Stereo, 0);
    let mix_gain = MixGainParamDefinition::mode_1(100, SAMPLE_RATE);
    let presentation = MixPresentation {
        mix_presentation_id: 42,
        annotations_language: vec![b"en-us".to_vec()],
        localized_presentation_annotations: vec![b"phase3_opus".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 300,
                localized_element_annotations: vec![b"stereo_opus".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: mix_gain.clone(),
            }],
            output_mix_gain: mix_gain,
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(-6144, -1536),
            }],
        }],
        trailing: Vec::new(),
    };
    Fixture {
        name: "phase3_opus",
        descriptors: DescriptorSet {
            sequence_header: header_for(&[&element]),
            codec_configs: vec![config],
            audio_elements: vec![element],
            mix_presentations: vec![presentation],
        },
        expected_pcm: vec![corpus.libiamf_expected],
        frame_sources: vec![FrameSource::PreEncoded(corpus.units)],
    }
}

/// **CONF-04 — the structure-only fixture.**
///
/// **Two** Audio Elements in one sub-mix, so descriptor ordering is observable
/// in our own output: OBU count, the ascending-`audio_element_id` write order,
/// and `find_obu_boundaries` landing its final boundary exactly on
/// `bytes.len()`.
///
/// Its decoded PCM is a **mix** and is therefore never compared sample for
/// sample — that is the whole reason the gate is split across two files rather
/// than asserted on one. Experiment 2 adopted this route (fallback (b)) after
/// the original two-Codec-Config design made `decoder_main` abort; fallback (a),
/// the `arbitrary_obu` injection, was rejected on its own recorded caveat, that
/// `INSERTION_HOOK_AFTER_CODEC_CONFIGS` makes ordering positional rather than
/// ID-sorted, so DESC-08's ascending-ID property would stop being what is
/// observed.
///
/// **Accepted cost, and added coverage:** two Audio Elements push the minimum
/// profile from Simple to **Base**, which [`header_for`] selects rather than
/// hard-codes — so this fixture also proves PROF-02's selector picks Base when
/// Base is what the configuration fits.
///
/// The elements are declared in **descending** id order (401 before 400) on
/// purpose: `write_descriptors` sorts ascending, so a writer that emitted them
/// in list order would produce different bytes and the ordering assertion would
/// catch it.
pub fn structure_only() -> Fixture {
    let config = lpcm_config(200, 16, SampleFormatFlags::LittleEndian);
    let first = channel_element(400, 200, LoudspeakerLayout::Stereo, 0);
    let second = channel_element(401, 200, LoudspeakerLayout::Stereo, 1);
    let mix_gain = MixGainParamDefinition::mode_1(100, SAMPLE_RATE);

    let presentation = MixPresentation {
        mix_presentation_id: 42,
        annotations_language: vec![b"en-us".to_vec()],
        localized_presentation_annotations: vec![b"phase1_structure_only".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![
                SubMixAudioElement {
                    audio_element_id: 400,
                    localized_element_annotations: vec![b"element_400".to_vec()],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: mix_gain.clone(),
                },
                SubMixAudioElement {
                    audio_element_id: 401,
                    localized_element_annotations: vec![b"element_401".to_vec()],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: mix_gain.clone(),
                },
            ],
            output_mix_gain: mix_gain,
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(-6144, -1536),
            }],
        }],
        trailing: Vec::new(),
    };

    let peak = peak_for(16);
    let pcm_a = ramp_pcm(SAMPLE_FRAMES, CHANNELS_STEREO.len(), peak);
    let pcm_b = ramp_pcm(SAMPLE_FRAMES, CHANNELS_STEREO.len(), peak);
    Fixture {
        name: "phase1_structure_only",
        descriptors: DescriptorSet {
            sequence_header: header_for(&[&first, &second]),
            codec_configs: vec![config],
            // Declared 401-first; `write_descriptors` sorts ascending by id.
            audio_elements: vec![second, first],
            mix_presentations: vec![presentation],
        },
        expected_pcm: vec![pcm_a.clone(), pcm_b.clone()],
        frame_sources: vec![
            FrameSource::LpcmInterleaved(pcm_a),
            FrameSource::LpcmInterleaved(pcm_b),
        ],
    }
}

/// Every fixture, in a stable order, for tests that assert a property of all
/// of them.
pub fn all() -> Vec<Fixture> {
    vec![sample_identity(), endianness(), structure_only()]
}

// ---------------------------------------------------------------------------
// D-19 — the channel-mismatch diagnostic. It REPORTS a permutation; it never
// applies one.
// ---------------------------------------------------------------------------

/// Describe a PCM mismatch, naming a candidate channel swap when one explains
/// it. `None` when the two are equal.
///
/// **There is no permutation constant, tolerance window or gain adjustment
/// anywhere in this harness, and this function is why one is not needed.**
/// TIME-02 says BCG packing order is distinct from presentation channel order,
/// so the fixture's input order, the packing order and `iamfdec`'s WAV order are
/// three different things. A permutation constant would be a knob, and the
/// cheapest-looking fix for a failing comparison is to turn the knob until it
/// goes green — which absorbs a real BCG packing bug into the harness and ships
/// it. No knob, no knob to turn: the diagnostic tells you *which* channels were
/// swapped so you can fix `src/packing.rs`, and stops there.
pub fn describe_channel_mismatch(
    expected: &[i32],
    decoded: &[i32],
    layout: LoudspeakerLayout,
) -> Option<String> {
    if expected == decoded {
        return None;
    }
    let names = channel_names(layout);
    let channels = names.len().max(1);

    let deinterleave = |samples: &[i32], channel: usize| -> Vec<i32> {
        samples
            .iter()
            .skip(channel)
            .step_by(channels)
            .copied()
            .collect()
    };

    if expected.len() != decoded.len() {
        return Some(format!(
            "PCM length differs: expected {} interleaved samples, decoded {}",
            expected.len(),
            decoded.len()
        ));
    }

    // For each expected channel, which decoded channel carries it verbatim?
    let mut mapping: Vec<Option<usize>> = Vec::with_capacity(channels);
    for source in 0..channels {
        let wanted = deinterleave(expected, source);
        mapping.push((0..channels).find(|target| deinterleave(decoded, *target) == wanted));
    }

    let is_total = mapping.iter().all(Option::is_some);
    if is_total {
        let swaps: Vec<String> = mapping
            .iter()
            .enumerate()
            .filter_map(|(source, target)| {
                let target = (*target)?;
                if target == source {
                    return None;
                }
                Some(format!(
                    "ch{source} ({}) arrived as ch{target} ({})",
                    names.get(source).copied().unwrap_or("?"),
                    names.get(target).copied().unwrap_or("?")
                ))
            })
            .collect();
        if !swaps.is_empty() {
            return Some(format!(
                "channels scrambled: {}. Every expected channel is present in the decode, just \
                 in the wrong slot — that is a BCG channel→substream packing defect in \
                 src/packing.rs, NOT something to correct with a permutation constant here \
                 (D-19). The 5.1 → Sound System B render matrix is the 6×6 identity, so the \
                 decoder's output IS our input when the packing is right.",
                swaps.join("; ")
            ));
        }
    }

    let first = expected
        .iter()
        .zip(decoded.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(0);
    let differing = expected
        .iter()
        .zip(decoded.iter())
        .filter(|(a, b)| a != b)
        .count();
    let channel = first.checked_rem(channels).unwrap_or(0);
    let frame = first.checked_div(channels).unwrap_or(0);
    Some(format!(
        "PCM differs in {differing} of {} samples; no channel permutation explains it. First \
         difference at interleaved index {first} = frame {frame}, ch{channel} ({}): expected \
         {:?}, decoded {:?}",
        expected.len(),
        names.get(channel).copied().unwrap_or("?"),
        expected.get(first),
        decoded.get(first)
    ))
}
