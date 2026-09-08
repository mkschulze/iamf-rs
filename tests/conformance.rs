//! The conformance harness — CONF-01's reusable `assert_conformant`, the seven
//! gate clauses, and the gating probe that had to run before the fixture was
//! frozen.
//!
//! # The exit code is never the signal
//!
//! Every assertion here names a real observable. That is not defensiveness: it
//! is recorded fact. `CONFORMANCE-GATE.md` § Experiment A ran five inputs —
//! one valid, four deliberately corrupted — through `iamfdec` and got **exit 0
//! from all five**, two of which produced nothing but a 44-byte WAV header.
//! Experiment 1 did the same through `iamf-tools`' `decoder_main` and got exit
//! 0 from three of five, one being a file truncated mid-OBU that wrote an
//! 80-byte WAV while decoding **zero** temporal units — which `test -s` also
//! passes.
//!
//! So the assertions run in this order, each with its own message:
//!
//! 1. the output file exists and is larger than a bare WAV header;
//! 2. the decoded sample count equals the encoded count;
//! 3. every sample is bit-identical.
//!
//! (2) is not redundant with (3): a wrong-length decode is never compared
//! sample-for-sample at all unless the count is checked first.
//!
//! # The limiter, and why the harness does two things about it
//!
//! `libiamf` creates a −1 dBTP peak limiter **unconditionally** in
//! `IAMF_decoder_open()` (1 ms attack, 200 ms release, 240-sample look-ahead;
//! linear threshold ≈ 0.8913). Below threshold `compute_target_gain()` returns
//! exactly 1.0 and the path is bit-exact; above it, the gain ramps for up to
//! 201 ms and the sample-identity assertion fails with a diffuse,
//! amplitude-only, channel-uniform error that reads exactly like a rounding or
//! endianness bug in our own encoder.
//!
//! Two independent measures, and **both** are deliberate:
//!
//! - every fixture here peaks at or below half full scale (−6 dBFS), and
//! - every decode passes `-disable_limiter`.
//!
//! Written down because the failure mode is a later maintainer raising the
//! amplitude "to make the signal more distinguishable" and reintroducing it.
//! Research open question 3 is resolved the same way: the harness decodes
//! **twice**, once with the limiter disabled and once with it at its default,
//! and compares the two. A fixture that drifts above threshold then fails
//! loudly instead of being masked by the flag.
//!
//! `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:4153-4159,
//! 4231-4236; code/src/common/audio_defines.h:23-26]`
//!
//! # Offline discipline (CONF-10)
//!
//! Every reference-dependent test prints a skip reason and returns when
//! `IAMF_REF_DECODER` is unset. `cargo test` is green with no reference binary
//! present, on all four byte-identity targets, which is what keeps the PR gate
//! fast and cross-platform.
//!
//! Nothing in this file reaches the shipping graph: the WAV reader is ~60 lines
//! here rather than a crate, because the harness compares PCM and not
//! containers.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::model::{DescriptorSet, select_minimum_profile};
use iamf::obu::{
    AudioElement, AudioFrame, ChannelAudioLayerConfig, CodecConfig, IaSequenceHeader, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixPresentation,
    RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
    plan_frames,
};
use iamf::packing::{SubstreamPlan, pack_channels_to_substreams};
use iamf::sequence::{SequenceWriter, TemporalUnit};

// ---------------------------------------------------------------------------
// Reference-binary discovery and per-test scratch directories (CONF-09)
// ---------------------------------------------------------------------------

/// The `iamfdec` path, or `None` when the reference is not present.
///
/// Discovered through the environment and invoked by [`Command`] — never by a
/// build script. A build script would force CMake and a C++20 toolchain onto
/// every consumer's `cargo build`, including Parallax's and docs.rs's, and it
/// cannot be made conditional on "the developer wants reference tests".
fn reference_decoder() -> Option<PathBuf> {
    // An EMPTY value counts as unset. `IAMF_REF_DECODER=` is what a CI step
    // that means "unset" usually writes, and treating it as a path would spawn
    // the empty program and report the failure as a decode failure — a
    // misdiagnosis, in the one place this file exists to prevent them.
    std::env::var_os("IAMF_REF_DECODER")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Print the standard skip reason for a reference-gated test and return.
fn skip_no_reference(test: &str) {
    println!(
        "SKIP {test}: IAMF_REF_DECODER is unset. Run `bash tools/build-reference.sh` and export \
         the path it prints to enable the reference-gated clauses. This is the expected offline \
         state (CONF-10)."
    );
}

/// A monotonically increasing counter, so two scratch directories requested in
/// the same process at the same instant cannot collide.
static SCRATCH_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty directory for one test's intermediate `.iamf` and `.wav`
/// (T-01-49).
///
/// Uniqueness is per process **and** per call: two tests running under the
/// default thread pool never share an output path, so a comparison can never
/// pass or fail for a reason unrelated to the bytes. Kept under `target/` so it
/// is gitignored and still inspectable after a failure.
// GUARD-04's `allow-expect-in-tests` carve-out (clippy.toml) applies to `#[test]`
// functions only — a helper in a test binary is not one. These three helpers are
// reachable from tests and nowhere else, and an `expect` here fails the run with
// a named message rather than swallowing a broken environment, which is exactly
// what the carve-out is for. Kept as three narrow allows rather than a file-wide
// one so a future helper does not inherit the exemption silently.
#[allow(clippy::expect_used)]
fn scratch_dir(tag: &str) -> PathBuf {
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("conformance")
        .join(format!("{tag}-{}-{sequence}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory is creatable under target/");
    dir
}

// ---------------------------------------------------------------------------
// The `iamfdec` runner — reports, never interprets
// ---------------------------------------------------------------------------

/// Which output layout `-s` selects.
///
/// `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/test/tools/iamfdec/src/test_iamfdec.c:87-103]`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputLayout {
    /// `-s0` — Sound System A (0+2+0), stereo.
    SoundSystemA,
    /// `-s1` — Sound System B (0+5+0), 5.1.
    SoundSystemB,
}

impl OutputLayout {
    /// The `-s` flag, as one joined argument.
    const fn flag(self) -> &'static str {
        match self {
            Self::SoundSystemA => "-s0",
            Self::SoundSystemB => "-s1",
        }
    }
}

/// Everything one `iamfdec` invocation produced, with **nothing interpreted**.
///
/// The exit code, the two streams and the output path are four independent
/// observations. Conflating them is how the wrong one gets chosen as the
/// signal, which is precisely the mistake Experiments A and 1 recorded.
#[derive(Debug)]
struct DecodeRun {
    /// The process exit code. Recorded for the report, asserted on by nothing.
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    /// Where the WAV was asked to go. May not exist.
    output: PathBuf,
    /// The exact argv, for the failure message and for `CONFORMANCE-GATE.md`.
    command: String,
}

/// Run `iamfdec` once and report what happened.
///
/// `-r` and `-d` are **arguments**, never constants: a sample rate that
/// disagrees with the file's Codec Config silently drives libiamf's speex
/// resampler (the default is 48000), which produces a comparison failure for a
/// reason that has nothing to do with the bitstream (T-01-50).
fn run_iamfdec(
    decoder: &Path,
    input: &Path,
    output: &Path,
    sample_rate: u32,
    sample_size: u8,
    layout: OutputLayout,
    disable_limiter: bool,
) -> DecodeRun {
    let rate = sample_rate.to_string();
    let depth = sample_size.to_string();
    let mut args: Vec<String> = vec![
        "-i0".to_owned(),
        "-o3".to_owned(),
        output.display().to_string(),
        "-r".to_owned(),
        rate,
        layout.flag().to_owned(),
        "-d".to_owned(),
        depth,
    ];
    if disable_limiter {
        args.push("-disable_limiter".to_owned());
    }
    args.push(input.display().to_string());

    let command = format!("{} {}", decoder.display(), args.join(" "));
    let produced = Command::new(decoder).args(&args).output();
    match produced {
        Ok(out) => DecodeRun {
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            output: output.to_path_buf(),
            command,
        },
        Err(e) => DecodeRun {
            exit_code: None,
            stdout: String::new(),
            stderr: format!("failed to spawn: {e}"),
            output: output.to_path_buf(),
            command,
        },
    }
}

// ---------------------------------------------------------------------------
// The WAV reader — frame count and interleaved samples, nothing else
// ---------------------------------------------------------------------------

/// A bare canonical WAV header: `RIFF____WAVE` + `fmt ` (8 + 16) + `data` (8).
///
/// The threshold the "output exists and is larger than a bare header"
/// assertion uses. Experiment A produced exactly 44 bytes twice, from two
/// different failures.
const BARE_WAV_HEADER_LEN: u64 = 44;

/// What a decoded WAV carries, in the only terms the harness compares.
#[derive(Debug)]
struct Wav {
    channels: usize,
    bits_per_sample: u16,
    sample_rate: u32,
    /// Sample frames — `samples.len() / channels`.
    frames: usize,
    /// Interleaved samples, sign-extended into `i32`.
    samples: Vec<i32>,
}

impl Wav {
    /// Channel `channel`'s samples, in order.
    fn channel(&self, channel: usize) -> Vec<i32> {
        self.samples
            .iter()
            .skip(channel)
            .step_by(self.channels.max(1))
            .copied()
            .collect()
    }
}

/// Parse a WAV by walking its chunks, never by trusting a magic offset.
///
/// A `fmt ` chunk that is not 16 bytes, or an extra `LIST` chunk, would shift
/// every sample by an unknown amount if the reader assumed offset 44 — which is
/// the same class of silent-wrong-answer failure the whole gate exists to
/// refuse.
fn read_wav(path: &Path) -> Result<Wav, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let tag = |at: usize| bytes.get(at..at.saturating_add(4)).map(<[u8]>::to_vec);
    let le32 = |at: usize| {
        bytes
            .get(at..at.saturating_add(4))
            .and_then(|s| <[u8; 4]>::try_from(s).ok())
            .map(u32::from_le_bytes)
    };
    let le16 = |at: usize| {
        bytes
            .get(at..at.saturating_add(2))
            .and_then(|s| <[u8; 2]>::try_from(s).ok())
            .map(u16::from_le_bytes)
    };

    if tag(0).as_deref() != Some(b"RIFF") || tag(8).as_deref() != Some(b"WAVE") {
        return Err(format!(
            "{} is not a RIFF/WAVE file ({} bytes)",
            path.display(),
            bytes.len()
        ));
    }

    let mut channels = 0_usize;
    let mut bits_per_sample = 0_u16;
    let mut sample_rate = 0_u32;
    let mut data: Option<(usize, usize)> = None;

    let mut cursor = 12_usize;
    while cursor.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let id = tag(cursor).unwrap_or_default();
        let size = usize::try_from(le32(cursor.saturating_add(4)).unwrap_or(0)).unwrap_or(0);
        let body = cursor.saturating_add(8);
        let end = body.saturating_add(size).min(bytes.len());
        if id == b"fmt " {
            channels = usize::from(le16(body.saturating_add(2)).unwrap_or(0));
            sample_rate = le32(body.saturating_add(4)).unwrap_or(0);
            bits_per_sample = le16(body.saturating_add(14)).unwrap_or(0);
        } else if id == b"data" {
            data = Some((body, end));
        }
        // Chunks are word-aligned: an odd size carries one pad byte.
        let advance = size.saturating_add(8).saturating_add(size.checked_rem(2).unwrap_or(0));
        cursor = cursor.saturating_add(advance.max(8));
    }

    let (start, end) = data.ok_or_else(|| format!("{} has no data chunk", path.display()))?;
    let payload = bytes.get(start..end).unwrap_or_default();
    let bytes_per_sample = usize::from(bits_per_sample).saturating_add(7) / 8;
    if channels == 0 || bytes_per_sample == 0 {
        return Err(format!(
            "{} declares {channels} channels at {bits_per_sample} bits",
            path.display()
        ));
    }

    let mut samples = Vec::with_capacity(payload.len().checked_div(bytes_per_sample).unwrap_or(0));
    let mut at = 0_usize;
    while at.saturating_add(bytes_per_sample) <= payload.len() {
        let slice = payload
            .get(at..at.saturating_add(bytes_per_sample))
            .unwrap_or_default();
        samples.push(decode_le_sample(slice));
        at = at.saturating_add(bytes_per_sample);
    }
    let frames = samples.len().checked_div(channels).unwrap_or(0);

    Ok(Wav {
        channels,
        bits_per_sample,
        sample_rate,
        frames,
        samples,
    })
}

/// A little-endian signed sample of 1–4 bytes, sign-extended into `i32`.
///
/// WAV PCM is little-endian at every depth, including the 3-byte one. That is
/// the *opposite* of the fixture's own storage (`sample_format_flags == 0` is
/// big-endian), which is the whole point of DESC-03 and the reason a 24-bit
/// fixture is worth having: the two orders are visible on the same round trip.
fn decode_le_sample(bytes: &[u8]) -> i32 {
    // Assembled unsigned, then sign-extended once. Doing it as `i32` would let
    // a byte's own high bit smear into the slots above it.
    let mut raw: u32 = 0;
    for (index, byte) in bytes.iter().enumerate() {
        let shift = u32::try_from(index).unwrap_or(0).saturating_mul(8);
        raw |= u32::from(*byte).checked_shl(shift).unwrap_or(0);
    }
    let width = u32::try_from(bytes.len()).unwrap_or(0).saturating_mul(8);
    sign_extend(raw, width)
}

/// Sign-extend the low `width` bits of `raw` into an `i32`.
///
/// `from_ne_bytes(to_ne_bytes(..))` rather than `as`: a bit-for-bit
/// reinterpretation stated as one, so a reader does not have to remember which
/// direction `as` truncates.
fn sign_extend(raw: u32, width: u32) -> i32 {
    let value = i32::from_ne_bytes(raw.to_ne_bytes());
    if width == 0 || width >= 32 {
        return value;
    }
    let shift = 32_u32.saturating_sub(width);
    value
        .checked_shl(shift)
        .unwrap_or(0)
        .checked_shr(shift)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Encoding a fixture through the real writer
// ---------------------------------------------------------------------------

/// Everything needed to turn interleaved PCM into an `.iamf`, all of it read
/// back out of the Codec Config so no constant is duplicated.
#[derive(Debug, Clone, Copy)]
struct EncodeSpec {
    sample_rate: u32,
    sample_size: u8,
    num_samples_per_frame: u32,
    layout: LoudspeakerLayout,
    channels: usize,
}

impl EncodeSpec {
    /// Bytes per stored sample.
    fn bytes_per_sample(self) -> usize {
        usize::from(self.sample_size).div_ceil(8)
    }
}

/// The spec a descriptor set implies — derived, never supplied twice.
///
/// CONF-01 turns on this: the rate, sample size and frame size the harness
/// hands the reference come from the Codec Config it just wrote, so the
/// function carries no codec-specific constant and Phase 3 reuses it unchanged.
fn spec_from(descriptors: &DescriptorSet) -> Result<EncodeSpec, String> {
    let element = descriptors
        .audio_elements
        .first()
        .ok_or_else(|| "the descriptor set has no Audio Element".to_owned())?;
    let config = descriptors
        .codec_config_by_id(element.codec_config_id)
        .ok_or_else(|| {
            format!(
                "no Codec Config carries id {}, which Audio Element {} references",
                element.codec_config_id, element.audio_element_id
            )
        })?;
    let lpcm = config
        .lpcm_config()
        .ok_or_else(|| "the Codec Config is not LPCM".to_owned())?;
    let layout = first_layout(element)?;
    let plan = SubstreamPlan::for_layout(layout).map_err(|e| format!("{e:?}"))?;
    Ok(EncodeSpec {
        sample_rate: lpcm.sample_rate,
        sample_size: lpcm.sample_size,
        num_samples_per_frame: config.num_samples_per_frame,
        layout,
        channels: plan.channel_count(),
    })
}

/// The `loudspeaker_layout` of an element's first channel layer.
fn first_layout(element: &AudioElement) -> Result<LoudspeakerLayout, String> {
    match &element.audio_element_type {
        iamf::obu::AudioElementType::ChannelBased(config) => config
            .scalable_channel_layout
            .layers
            .first()
            .map(|layer| layer.loudspeaker_layout)
            .ok_or_else(|| "the channel layout has no layers".to_owned()),
        _ => Err("only channel-based elements are encoded by this harness".to_owned()),
    }
}

/// Store one sample big- or little-endian at `sample_size` bits.
///
/// A byte copy driven by a flag, and no arithmetic on the sample value.
fn store_sample(value: i32, sample_size: u8, big_endian: bool, into: &mut Vec<u8>) {
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

/// Interleaved samples → interleaved stored bytes, in the Codec Config's
/// declared endianness.
fn store_interleaved(samples: &[i32], spec: EncodeSpec, big_endian: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len().saturating_mul(spec.bytes_per_sample()));
    for value in samples {
        store_sample(*value, spec.sample_size, big_endian, &mut bytes);
    }
    bytes
}

/// Encode a whole IA Sequence through the **real** [`SequenceWriter`].
///
/// Deliberately not a test-only serialisation path: a harness that encoded its
/// fixture some other way would prove that the other way works.
fn encode_iamf(descriptors: &DescriptorSet, samples: &[i32]) -> Result<Vec<u8>, String> {
    let spec = spec_from(descriptors)?;
    let big_endian = descriptors
        .codec_configs
        .first()
        .and_then(CodecConfig::lpcm_config)
        .map(|c| matches!(c.sample_format_flags, SampleFormatFlags::BigEndian))
        .unwrap_or(true);

    let total_samples =
        u64::try_from(samples.len().checked_div(spec.channels).unwrap_or(0)).unwrap_or(0);
    let plan = plan_frames(total_samples, spec.num_samples_per_frame).map_err(|e| format!("{e:?}"))?;
    let substreams = SubstreamPlan::for_layout(spec.layout).map_err(|e| format!("{e:?}"))?;

    let interleaved = store_interleaved(samples, spec, big_endian);
    let frame_bytes = usize::try_from(spec.num_samples_per_frame)
        .unwrap_or(0)
        .saturating_mul(spec.channels)
        .saturating_mul(spec.bytes_per_sample());

    let mut writer = SequenceWriter::new(Vec::new());
    writer
        .push_descriptors(descriptors)
        .map_err(|e| format!("push_descriptors: {e:?}"))?;

    for index in 0..plan.frame_count {
        let start = usize::try_from(index).unwrap_or(0).saturating_mul(frame_bytes);
        let end = start.saturating_add(frame_bytes).min(interleaved.len());
        let mut chunk = interleaved.get(start..end).unwrap_or_default().to_vec();
        // The final frame is short and is zero-padded to a whole frame by the
        // caller, which is why it carries the end trim. The writer never
        // invents sample values — that would be signal processing.
        chunk.resize(frame_bytes, 0);

        let payloads = pack_channels_to_substreams(
            &substreams,
            &chunk,
            spec.channels,
            spec.bytes_per_sample(),
        )
        .map_err(|e| format!("pack_channels_to_substreams: {e:?}"))?;

        let trimming = plan.trimming_for(index);
        let frames: Vec<_> = payloads
            .into_iter()
            .enumerate()
            .map(|(substream, payload)| {
                let id = u32::try_from(substream).unwrap_or(0);
                AudioFrame::new(id, payload).into_obu(trimming)
            })
            .collect();
        writer
            .push_temporal_unit(&TemporalUnit::of_frames(frames))
            .map_err(|e| format!("push_temporal_unit: {e:?}"))?;
    }

    writer.finish().map_err(|e| format!("finish: {e:?}"))
}

// ---------------------------------------------------------------------------
// The probe's signal
// ---------------------------------------------------------------------------

/// Half of 24-bit full scale (2^23), which is exactly −6 dBFS.
///
/// The cap, not a suggestion. See the module documentation on the limiter.
const PEAK_24: i32 = 0x0040_0000;

/// Half of full scale at `sample_size` bits — `2^(n-2)`, which is −6 dBFS at
/// every depth.
fn peak_for(sample_size: u8) -> i32 {
    let shift = u32::from(sample_size).saturating_sub(2);
    1_i32.checked_shl(shift).unwrap_or(PEAK_24)
}

/// A per-channel deterministic ramp, folded into `[-peak, +peak]`.
///
/// Channel `c` advances by `STEP` per sample from a base of `c * OFFSET`. Since
/// `OFFSET` is neither zero nor a multiple of the fold span, two channels differ
/// at **every** sample index — which is what makes a swapped channel
/// arithmetically identifiable rather than merely "PCM differs" (D-19).
fn ramp_sample(channel: usize, index: usize, peak: i32) -> i32 {
    const STEP: i64 = 7919;
    const OFFSET: i64 = 104_729;
    let span = i64::from(peak).saturating_mul(2).saturating_add(1);
    let base = i64::try_from(channel).unwrap_or(0).saturating_mul(OFFSET);
    let walk = i64::try_from(index).unwrap_or(0).saturating_mul(STEP);
    let folded = base.saturating_add(walk).checked_rem(span).unwrap_or(0);
    i32::try_from(folded).unwrap_or(0).saturating_sub(peak)
}

/// `frames * channels` interleaved samples of the ramp.
fn ramp_pcm(frames: usize, channels: usize, peak: i32) -> Vec<i32> {
    let mut pcm = Vec::with_capacity(frames.saturating_mul(channels));
    for index in 0..frames {
        for channel in 0..channels {
            pcm.push(ramp_sample(channel, index, peak));
        }
    }
    pcm
}

// ---------------------------------------------------------------------------
// The probe's descriptor set — the smallest 24-bit big-endian file there is
// ---------------------------------------------------------------------------

/// `num_samples_per_frame` for the probe.
const PROBE_FRAME_SIZE: u32 = 128;
/// A deliberate non-multiple of [`PROBE_FRAME_SIZE`]: 300 = 2×128 + 44, so the
/// final frame carries `trim_at_end = 84` and `trim_at_start = 0`. The two
/// differ, which is the only configuration that can catch a swapped END/START
/// write order (OBU-05).
const PROBE_SAMPLE_FRAMES: usize = 300;

/// One Codec Config, one stereo Audio Element, one Mix Presentation with the
/// mandatory Sound System A layout, at 48 kHz and the requested sample format.
///
/// Parameterised over depth and endianness because the probe's whole job is to
/// establish **which combinations the pinned reference can evaluate at all**.
#[allow(clippy::expect_used)]
fn probe_descriptors(sample_size: u8, sample_format_flags: SampleFormatFlags) -> DescriptorSet {
    let codec_config = CodecConfig::lpcm(
        200,
        PROBE_FRAME_SIZE,
        LpcmDecoderConfig {
            // Zero is BIG-endian (DESC-03) — the opposite of a WAV-shaped
            // assumption, and the sense the probe exists to test.
            sample_format_flags,
            sample_size,
            sample_rate: 48_000,
        },
    );
    let element = AudioElement::channel_based(
        300,
        200,
        vec![0],
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            LoudspeakerLayout::Stereo,
            1,
            1,
        )),
    );
    let (primary, additional) =
        select_minimum_profile(&[&element]).expect("one stereo element fits Simple profile");

    let mix_gain = MixGainParamDefinition::mode_1(100, 48_000);
    let presentation = MixPresentation {
        mix_presentation_id: 42,
        annotations_language: vec![b"en-us".to_vec()],
        localized_presentation_annotations: vec![b"probe_lpcm".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 300,
                localized_element_annotations: vec![b"probe_element_0".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: mix_gain.clone(),
            }],
            output_mix_gain: mix_gain,
            // Sound System A is the target here AND the layout iamf-tools
            // hard-checks on write ("Every sub-mix must have a stereo layout").
            // For a stereo fixture one entry serves both roles; a 5.1 fixture
            // needs two.
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                // Caller-supplied numbers, as the wire requires. -24 LUFS and
                // -6 dBFS in Q7.8: -6144 / 256 and -1536 / 256.
                loudness: Loudness::new(-6144, -1536),
            }],
        }],
        trailing: Vec::new(),
    };

    DescriptorSet {
        sequence_header: IaSequenceHeader::new(primary.to_wire(), additional.to_wire()),
        codec_configs: vec![codec_config],
        audio_elements: vec![element],
        mix_presentations: vec![presentation],
    }
}

// ---------------------------------------------------------------------------
// TASK 1 — the gating probe: research assumption A1
// ---------------------------------------------------------------------------
//
// A1 was research's highest-risk `[ASSUMED]` claim: that the reference's
// float→int output conversion is exact at 24 bits, so D-18's 24-bit
// **big-endian** fixture round-trips sample-identically. The empirical proof on
// record was run at 16 bit.
//
// It ran here, before the fixture was frozen, and **A1 is REFUTED** — though
// not where it was expected. The output conversion is fine. The *input* is not:
//
//     libiamf@v1.1.0 code/src/iamf_dec/bitstream.c:206-210
//     int reads24be(uint8_t *data, int offset) {
//       uint32_t ret = readu16le(data, offset) << 8 | data[offset + 2];
//                      ^^^^^^^^^ readu16be was meant
//
// Its unsigned sibling `readu24be`, two lines above, uses `readu16be` and is
// correct. So 24-bit **big-endian** LPCM is decoded by the pinned reference
// with its top two bytes transposed, and no other sample format is affected:
// `reads16be` uses `readu16be`, `reads24le`/`reads16le` use `readu16le`, and
// all three are right.
//
// The consequence for the fixture is recorded in `CONFORMANCE-GATE.md` and
// enforced by the three probes below. What matters here is what did NOT happen:
// the 24-bit big-endian combination was not quietly dropped, and no tolerance
// window was introduced to make it pass.

/// One round trip through `iamfdec`, decoded twice, with every precondition a
/// comparison depends on asserted **before** any comparison is made.
#[derive(Debug)]
struct RoundTrip {
    /// Sample frames handed to the encoder.
    encoded_frames: usize,
    /// Sample frames the reference reported back.
    decoded_frames: usize,
    /// The limiter-disabled decode, interleaved.
    decoded: Vec<i32>,
    /// Interleaved samples that differ between input and the limiter-disabled
    /// decode.
    differing: usize,
    /// Interleaved samples that differ between the two decodes. Must be 0: the
    /// limiter's gain is exactly 1.0 below threshold, so a difference means the
    /// fixture has drifted above −1 dBTP and `-disable_limiter` is masking it.
    limiter_differing: usize,
}

/// Encode `pcm` through the real writer, decode it twice, and report.
///
/// Assertion order is fixed and each has its own message: (1) the output exists
/// and exceeds a bare WAV header, (2) the decoded sample count equals the
/// encoded count, and only then (3) is any sample compared — by the caller,
/// which decides what the count of differences means.
///
/// The second decode, with the limiter at its default, is research open
/// question 3 resolved: run both, so a fixture that drifts above threshold
/// fails loudly instead of being hidden by the flag.
#[allow(clippy::expect_used)]
fn round_trip(
    decoder: &Path,
    descriptors: &DescriptorSet,
    pcm: &[i32],
    layout: OutputLayout,
    tag: &str,
) -> RoundTrip {
    let spec = spec_from(descriptors).expect("the probe's descriptor set is well formed");
    let peak = peak_for(spec.sample_size);
    let observed_peak = pcm.iter().map(|v| v.saturating_abs()).max().unwrap_or(0);
    assert!(
        observed_peak <= peak,
        "{tag}: the signal peaks at {observed_peak}, above the -6 dBFS cap of {peak}. libiamf's \
         -1 dBTP limiter is created unconditionally and would engage, producing a diffuse, \
         amplitude-only, channel-uniform diff that reads exactly like a rounding bug in our own \
         encoder. Do not raise the amplitude 'to make the signal more distinguishable'."
    );

    let bytes = encode_iamf(descriptors, pcm).expect("the probe encodes");
    let dir = scratch_dir(tag);
    let input = dir.join(format!("{tag}.iamf"));
    std::fs::write(&input, &bytes).expect("the probe .iamf is writable");

    let encoded_frames = pcm.len().checked_div(spec.channels.max(1)).unwrap_or(0);

    // ---- decode 1: limiter disabled — the sample-identity run ---------------
    let nolim = dir.join(format!("{tag}.nolimiter.wav"));
    let run = run_iamfdec(
        decoder,
        &input,
        &nolim,
        spec.sample_rate,
        spec.sample_size,
        layout,
        true,
    );
    println!("{tag} command: {}", run.command);
    println!("{tag} exit: {:?} (recorded, NOT the signal)", run.exit_code);
    println!("{tag} stderr: {}", run.stderr.trim());

    // Assertion 1 — the output exists and is larger than a bare WAV header.
    let size = std::fs::metadata(&nolim).map(|m| m.len()).unwrap_or(0);
    assert!(
        size > BARE_WAV_HEADER_LEN,
        "{tag}: iamfdec wrote {size} bytes to {} (a bare WAV header is {BARE_WAV_HEADER_LEN}). \
         The exit code was {:?} and is NOT the signal: Experiment A recorded exit 0 from runs \
         that wrote exactly 44 bytes.\ncommand: {}\nstdout: {}\nstderr: {}",
        nolim.display(),
        run.exit_code,
        run.command,
        run.stdout,
        run.stderr
    );

    let wav = read_wav(&nolim).expect("the decoded WAV parses");
    println!(
        "{tag} wav: {} ch, {} bits, {} Hz, {} frames",
        wav.channels, wav.bits_per_sample, wav.sample_rate, wav.frames
    );

    // Assertion 2 — the decoded sample count equals the encoded count.
    assert_eq!(
        wav.frames, encoded_frames,
        "{tag}: decoded {} sample frames, encoded {encoded_frames}. A wrong-length decode is \
         never compared sample-for-sample at all unless the count is checked first (Experiment A: \
         a one-bit sample_rate flip yielded 1570 samples instead of 8000, exit 0, no error).",
        wav.frames
    );
    assert_eq!(
        wav.channels, spec.channels,
        "{tag}: the decoder reported {} channels, the fixture carries {}",
        wav.channels, spec.channels
    );

    let differing = pcm
        .iter()
        .zip(wav.samples.iter())
        .filter(|(a, b)| a != b)
        .count();

    // ---- decode 2: limiter at its default — the drift alarm ----------------
    let lim = dir.join(format!("{tag}.limiter.wav"));
    let run_lim = run_iamfdec(
        decoder,
        &input,
        &lim,
        spec.sample_rate,
        spec.sample_size,
        layout,
        false,
    );
    let lim_size = std::fs::metadata(&lim).map(|m| m.len()).unwrap_or(0);
    assert!(
        lim_size > BARE_WAV_HEADER_LEN,
        "{tag}: the limiter-enabled decode wrote {lim_size} bytes.\ncommand: {}\nstderr: {}",
        run_lim.command,
        run_lim.stderr
    );
    let wav_lim = read_wav(&lim).expect("the limiter-enabled WAV parses");
    assert_eq!(
        wav_lim.frames, wav.frames,
        "{tag}: the limiter changed the decoded frame count"
    );
    let limiter_differing = wav
        .samples
        .iter()
        .zip(wav_lim.samples.iter())
        .filter(|(a, b)| a != b)
        .count();

    RoundTrip {
        encoded_frames,
        decoded_frames: wav.frames,
        decoded: wav.samples,
        differing,
        limiter_differing,
    }
}

/// The probe signal for one format: [`PROBE_SAMPLE_FRAMES`] frames of the
/// per-channel ramp, capped at −6 dBFS for that depth.
fn probe_pcm(spec_channels: usize, sample_size: u8) -> Vec<i32> {
    ramp_pcm(PROBE_SAMPLE_FRAMES, spec_channels, peak_for(sample_size))
}

/// The limiter changed nothing — which is what a fixture below −6 dBFS
/// guarantees, since `compute_target_gain()` returns exactly 1.0 while no
/// look-ahead peak exceeds the linear threshold of ≈0.8913.
///
/// Asserted **by the caller**, not inside [`round_trip`], for one reason: a
/// round trip whose *decode* is wrong can produce samples the encoder never
/// wrote, and those can exceed the threshold even though the input did not.
/// Baking this into the helper would have made such a case report "the fixture
/// is too loud", which is the wrong diagnosis and would have sent a reader to
/// lower the amplitude instead of finding the real defect. Every fixture whose
/// round trip is expected to be exact asserts it; the one probe that documents
/// an upstream misread explains why it does not.
fn assert_limiter_is_transparent(trip: &RoundTrip, tag: &str) {
    assert_eq!(
        trip.limiter_differing, 0,
        "{tag}: {} samples differ between the limiter-disabled and limiter-enabled decodes. The \
         fixture has drifted above the -1 dBTP threshold (linear ~0.8913) and -disable_limiter is \
         masking it. Lower the amplitude cap; do not remove this check.",
        trip.limiter_differing
    );
}

/// **A1, positive half.** 24-bit *little*-endian round-trips exactly, so the
/// depth itself — three bytes per sample, `scale_i2f = 2^23`, `FLOAT2INT24` —
/// is exact and only the big-endian *read* is at fault.
///
/// This is the evidence the sample-identity fixture's depth is frozen on.
#[test]
fn probe_24bit_little_endian_round_trips_exactly() {
    let Some(decoder) = reference_decoder() else {
        skip_no_reference("probe_24bit_little_endian_round_trips_exactly");
        return;
    };
    let descriptors = probe_descriptors(24, SampleFormatFlags::LittleEndian);
    let pcm = probe_pcm(2, 24);
    let trip = round_trip(
        &decoder,
        &descriptors,
        &pcm,
        OutputLayout::SoundSystemA,
        "probe24le",
    );
    println!(
        "A1 RESULT (24-bit LE): {} frames in, {} out, {} of {} samples differ, limiter delta {}",
        trip.encoded_frames,
        trip.decoded_frames,
        trip.differing,
        pcm.len(),
        trip.limiter_differing
    );
    assert_eq!(
        trip.differing,
        0,
        "{} of {} samples differ after a 24-bit little-endian round trip. The 24-bit depth itself \
         is supposed to be exact — if this fails, the defect is ours, not upstream's.",
        trip.differing,
        pcm.len()
    );
    assert_limiter_is_transparent(&trip, "probe24le");
}

/// **A1, endianness half.** 16-bit *big*-endian round-trips exactly, so
/// `sample_format_flags == 0` is evaluable by the reference at 16 bits.
///
/// This is the evidence DESC-03's endianness-sense sample identity is kept on a
/// second fixture rather than lost.
#[test]
fn probe_16bit_big_endian_round_trips_exactly() {
    let Some(decoder) = reference_decoder() else {
        skip_no_reference("probe_16bit_big_endian_round_trips_exactly");
        return;
    };
    let descriptors = probe_descriptors(16, SampleFormatFlags::BigEndian);
    let pcm = probe_pcm(2, 16);
    let trip = round_trip(
        &decoder,
        &descriptors,
        &pcm,
        OutputLayout::SoundSystemA,
        "probe16be",
    );
    println!(
        "A1 RESULT (16-bit BE): {} frames in, {} out, {} of {} samples differ, limiter delta {}",
        trip.encoded_frames,
        trip.decoded_frames,
        trip.differing,
        pcm.len(),
        trip.limiter_differing
    );
    assert_eq!(
        trip.differing,
        0,
        "{} of {} samples differ after a 16-bit big-endian round trip. reads16be uses readu16be \
         and is correct at the pinned SHA — if this fails, the defect is ours.",
        trip.differing,
        pcm.len()
    );
    assert_limiter_is_transparent(&trip, "probe16be");
}

/// What `libiamf@v1.1.0`'s `reads24be` actually computes, transcribed from
/// `code/src/iamf_dec/bitstream.c:206-210`.
///
/// Not a workaround and never applied to anything: it is the *diagnosis*, made
/// executable. `readu16le(data, offset) << 8 | data[offset + 2]` composes
/// `b0<<8 | b1<<16 | b2` where big-endian means `b0<<16 | b1<<8 | b2`, which
/// transposes the top two bytes.
fn upstream_reads24be(bytes: [u8; 3]) -> i32 {
    let [b0, b1, b2] = bytes;
    let readu16le = u32::from(b0) | (u32::from(b1).checked_shl(8).unwrap_or(0));
    let raw = readu16le.checked_shl(8).unwrap_or(0) | u32::from(b2);
    sign_extend(raw, 24)
}

/// **A1, refuted — and refuted precisely.**
///
/// The 24-bit big-endian round trip does *not* survive the pinned reference,
/// and this asserts exactly why: every decoded sample equals what the defective
/// `reads24be` would produce from the bytes we wrote. That is a much stronger
/// claim than "the PCM differs" — it proves our encoder wrote correct
/// big-endian bytes and the reference misread them, rather than the reverse.
///
/// **This test failing is good news.** It means `reads24be` was fixed upstream
/// (or the pin moved), which is exactly the "what would restore it" condition
/// of the D-17 waiver in `CONFORMANCE-GATE.md`: go read that entry and move the
/// sample-identity fixture back to 24-bit big-endian.
///
/// It is not bug-compatibility. Nothing here makes a comparison pass; the
/// waiver records the coverage that moved, and the byte-level correctness of our
/// own 24-bit big-endian writer is asserted by hand-computed vectors
/// ([`store_sample_writes_the_declared_byte_order`]), which no permissive reader
/// is involved in.
#[test]
fn probe_24bit_big_endian_hits_the_upstream_reads24be_defect() {
    let Some(decoder) = reference_decoder() else {
        skip_no_reference("probe_24bit_big_endian_hits_the_upstream_reads24be_defect");
        return;
    };
    let descriptors = probe_descriptors(24, SampleFormatFlags::BigEndian);
    let spec = spec_from(&descriptors).expect("the probe's descriptor set is well formed");
    let pcm = probe_pcm(spec.channels, 24);
    let trip = round_trip(
        &decoder,
        &descriptors,
        &pcm,
        OutputLayout::SoundSystemA,
        "probe24be",
    );
    println!(
        "A1 RESULT (24-bit BE): {} frames in, {} out, {} of {} samples differ, limiter delta {}",
        trip.encoded_frames,
        trip.decoded_frames,
        trip.differing,
        pcm.len(),
        trip.limiter_differing
    );

    // The frame count and the channel count are right — the file is structurally
    // fine and only the sample values are wrong. That is what makes this a read
    // defect rather than a framing one.
    assert_eq!(trip.decoded_frames, trip.encoded_frames);

    // `assert_limiter_is_transparent` is deliberately NOT called here. The
    // transposed read produces samples the encoder never wrote, some of which
    // exceed -1 dBTP, so the limiter genuinely engages on values that are
    // already wrong. Asserting transparency would report "the fixture is too
    // loud" — the wrong diagnosis, and one that would send a reader to lower
    // the amplitude instead of finding the misread. The input itself is at
    // -6 dBFS, which `round_trip` asserted before encoding anything.
    println!(
        "probe24be limiter delta {} (expected non-zero: the misread values exceed -1 dBTP)",
        trip.limiter_differing
    );

    // Every decoded sample equals the defective read of the bytes we wrote.
    let predicted: Vec<i32> = pcm
        .iter()
        .map(|value| {
            let mut stored = Vec::new();
            store_sample(*value, 24, true, &mut stored);
            let bytes = <[u8; 3]>::try_from(stored.as_slice()).unwrap_or([0, 0, 0]);
            upstream_reads24be(bytes)
        })
        .collect();
    let unexplained = predicted
        .iter()
        .zip(trip.decoded.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        unexplained,
        0,
        "{unexplained} of {} decoded samples are NOT explained by libiamf@v1.1.0's reads24be \
         defect (bitstream.c:206-210 uses readu16le where readu16be was meant). The diagnosis in \
         CONFORMANCE-GATE.md is therefore incomplete — do not proceed on it.",
        predicted.len()
    );
    assert_eq!(
        predicted.len(),
        trip.decoded.len(),
        "the prediction and the decode must cover the same samples"
    );

    // And the defect is real, not a no-op: it must actually have changed the
    // samples. If this ever reads 0, `reads24be` was fixed upstream and the
    // D-17 waiver's restoration condition has been met.
    assert!(
        trip.differing > 0,
        "the 24-bit big-endian round trip is now EXACT. reads24be appears to have been fixed \
         upstream (or REFERENCES.md's libiamf pin moved). This is the restoration condition of \
         the D-17 waiver in CONFORMANCE-GATE.md: remove the waiver and move the sample-identity \
         fixture back to 24-bit big-endian, which is what D-18 originally specified."
    );
}

/// The defective read, checked against values computed by hand from the code —
/// so the diagnosis above stands on its own even with no reference binary
/// present (CONF-10).
#[test]
fn the_upstream_reads24be_transposes_the_top_two_bytes() {
    // 0xC0 0x00 0x00 is -4194304 read correctly; the defect composes
    // 0x00 << 16 | 0xC0 << 8 | 0x00 = 0x00C000 = 49152.
    assert_eq!(upstream_reads24be([0xC0, 0x00, 0x00]), 49_152);
    // 0xC1 0x99 0x19 is -4089575 read correctly; the defect composes
    // 0x99 << 16 | 0xC1 << 8 | 0x19 = 0x99C119, which is negative at 24 bits.
    assert_eq!(upstream_reads24be([0xC1, 0x99, 0x19]), -6_700_775);
    // A value whose top two bytes are EQUAL is unaffected by a transposition,
    // which is why a fixture of mostly-zero or slowly-varying samples would
    // have hidden this entirely. 0x7F7F01 reads the same either way.
    assert_eq!(upstream_reads24be([0x7F, 0x7F, 0x01]), 0x007F_7F01);
}

/// The ramp is per-channel distinguishable at every index, which is what D-19's
/// diagnostic depends on: if two channels could ever carry the same value at
/// the same index, a swap would be undetectable there.
#[test]
fn the_ramp_distinguishes_every_channel_at_every_index() {
    let channels: usize = 6;
    for index in 0..512 {
        for a in 0..channels {
            for b in a.saturating_add(1)..channels {
                assert_ne!(
                    ramp_sample(a, index, PEAK_24),
                    ramp_sample(b, index, PEAK_24),
                    "channels {a} and {b} collide at sample index {index}"
                );
            }
        }
    }
}

/// The cap is a property of the signal generator, not of one call site.
#[test]
fn the_ramp_never_exceeds_the_minus_six_dbfs_cap() {
    for index in 0..4096 {
        for channel in 0..6 {
            let value = ramp_sample(channel, index, PEAK_24);
            assert!(
                value.saturating_abs() <= PEAK_24,
                "ramp({channel}, {index}) = {value} exceeds the cap {PEAK_24}"
            );
        }
    }
}

/// Big-endian and little-endian storage of the same value differ in byte order
/// and nothing else — the property DESC-03 turns on.
#[test]
fn store_sample_writes_the_declared_byte_order() {
    let mut be = Vec::new();
    let mut le = Vec::new();
    store_sample(0x0012_3456, 24, true, &mut be);
    store_sample(0x0012_3456, 24, false, &mut le);
    assert_eq!(be, vec![0x12, 0x34, 0x56]);
    assert_eq!(le, vec![0x56, 0x34, 0x12]);

    let mut negative = Vec::new();
    store_sample(-1, 24, true, &mut negative);
    assert_eq!(negative, vec![0xFF, 0xFF, 0xFF]);
}

/// The WAV reader sign-extends a 3-byte little-endian sample correctly, which
/// is the one decode step that silently corrupts every negative sample if it is
/// wrong.
#[test]
fn the_wav_reader_sign_extends_24_bit_samples() {
    assert_eq!(decode_le_sample(&[0x56, 0x34, 0x12]), 0x0012_3456);
    assert_eq!(decode_le_sample(&[0xFF, 0xFF, 0xFF]), -1);
    assert_eq!(decode_le_sample(&[0x00, 0x00, 0x80]), -0x0080_0000);
    assert_eq!(decode_le_sample(&[0x00, 0x00, 0x00]), 0);
}
