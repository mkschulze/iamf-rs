//! CONF-01's reusable `assert_conformant`, the Phase 1 exit gate's clauses, and
//! the gating probe that ran before the fixture was frozen.
//!
//! # The exit code is never the signal
//!
//! Every assertion here names a real observable. That is not defensiveness; it
//! is recorded fact. `CONFORMANCE-GATE.md` § Experiment A ran five inputs — one
//! valid, four deliberately corrupted — through `iamfdec` and got **exit 0 from
//! all five**, two of which produced nothing but a 44-byte WAV header.
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
//! sample-for-sample at all unless the count is checked first. Experiment A's
//! one-bit `sample_rate` flip decoded 1570 samples instead of 8000, cleanly,
//! with exit 0.
//!
//! # The limiter, and why the harness does two things about it
//!
//! `libiamf` creates a −1 dBTP peak limiter **unconditionally** in
//! `IAMF_decoder_open()` (1 ms attack, 200 ms release, 240-sample look-ahead;
//! linear threshold ≈ 0.8913). Below threshold `compute_target_gain()` returns
//! exactly 1.0 and the path is bit-exact; above it the gain ramps for up to
//! 201 ms and the sample-identity assertion fails with a diffuse,
//! amplitude-only, channel-uniform error that reads exactly like a rounding or
//! endianness bug in our own encoder.
//!
//! Two independent measures, and **both** are deliberate: every fixture peaks
//! at or below −6 dBFS, and every decode passes `-disable_limiter`. Research
//! open question 3 is resolved the same way — the harness decodes **twice**,
//! once with the limiter disabled and once with it at its default, and compares
//! the two, so a fixture that drifts above threshold fails loudly instead of
//! being hidden by the flag.
//!
//! `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:4153-4159,
//! 4231-4236; code/src/common/audio_defines.h:23-26]`
//!
//! # No knob (D-19)
//!
//! There is no permutation constant, no tolerance window and no gain adjustment
//! anywhere in this file. Comparison is exact integer equality. On a mismatch
//! `fixture::describe_channel_mismatch` names a candidate channel swap and
//! points at `src/packing.rs`; it never applies one. A permutation constant
//! would be a knob, and the cheapest-looking fix for a failing comparison is to
//! turn the knob until it goes green — which absorbs a real BCG packing bug into
//! the harness and ships it.
//!
//! # Offline discipline (CONF-09, CONF-10)
//!
//! Every reference binary is invoked by [`Command`], discovered through an
//! environment variable or a digest-pinned container image, **never by a build
//! script** — a build script would force CMake, a C++20 toolchain, abseil,
//! protobuf and fdk-aac onto every consumer's `cargo build`, including
//! Parallax's and docs.rs's, and it cannot be made conditional on "the developer
//! wants reference tests". Every reference-dependent clause prints a skip reason
//! and returns when its tool is absent, so `cargo test` is green offline on all
//! four byte-identity targets. Every intermediate file goes under a unique
//! per-test temporary directory.
//!
//! Nothing here reaches the shipping graph: the WAV reader and writer are ~130
//! lines in this file rather than a crate, because the harness compares PCM and
//! not containers.

#![allow(dead_code)]

#[path = "support/fixture.rs"]
mod fixture;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::model::{DescriptorSet, select_minimum_profile};
use iamf::obu::{
    AudioElement, ChannelAudioLayerConfig, CodecConfig, IaSequenceHeader, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixPresentation,
    RenderingConfig, SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
    Trimming,
};

use iamf::obu::{ObuType, find_obu_boundaries, plan_frames, read_obu_header};

use fixture::{
    ElementSpec, EncodedTemporalUnit, Fixture, FixtureCodec, FrameSource,
    describe_channel_mismatch, peak_for, ramp_pcm, store_interleaved, store_sample,
};

// ---------------------------------------------------------------------------
// Reference discovery and per-test scratch directories (CONF-09, T-01-49)
// ---------------------------------------------------------------------------

/// The `iamfdec` path, or `None` when the reference is not present.
fn reference_decoder() -> Option<PathBuf> {
    // An EMPTY value counts as unset. `IAMF_REF_DECODER=` is what a CI step
    // that means "unset" usually writes, and treating it as a path would spawn
    // the empty program and report the failure as a decode failure — a
    // misdiagnosis, in the one place this file exists to prevent them.
    std::env::var_os("IAMF_REF_DECODER")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The `iamf-tools` container image, pinned by digest in `REFERENCES.md`.
///
/// `iamf-tools` needs Bazel, abseil, protobuf and fdk-aac, so it runs only in a
/// container — which is also why CONF-06 and CONF-07 are the two clauses that
/// retain a defensible D-17 waiver path (see `CONFORMANCE-GATE.md`).
fn iamf_tools_image() -> String {
    std::env::var("IAMF_TOOLS_IMAGE")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "iamf-tools:v2.1.0".to_owned())
}

/// Whether the pinned image is present locally.
///
/// Checked with `docker image inspect` rather than by running the tool, so an
/// absent container is a *skip* and a broken one is a *failure* — two different
/// outcomes a single "did it work" check would conflate.
fn iamf_tools_available() -> bool {
    Command::new("docker")
        .args(["image", "inspect", &iamf_tools_image()])
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Print the standard skip reason for an `iamfdec`-gated clause and return.
fn skip_no_reference(test: &str) {
    println!(
        "SKIP {test}: IAMF_REF_DECODER is unset. Run `bash tools/build-reference.sh` and export \
         the path it prints to enable the reference-gated clauses. This is the expected offline \
         state (CONF-10)."
    );
}

/// Print the standard skip reason for a container-gated clause and return.
fn skip_no_container(test: &str) {
    println!(
        "SKIP {test}: the pinned image {} is not available locally. Build it with \
         `docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/`. On macOS \
         prepend /Applications/Docker.app/Contents/Resources/bin to PATH first — a missing \
         docker-credential-desktop is a credsStore lookup, not a build failure. This is the \
         expected offline state (CONF-10).",
        iamf_tools_image()
    );
}

/// A monotonically increasing counter, so two scratch directories requested in
/// the same process at the same instant cannot collide.
static SCRATCH_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

// GUARD-04's `allow-unwrap-in-tests` / `allow-expect-in-tests` /
// `allow-panic-in-tests` carve-out (clippy.toml) applies to `#[test]` functions
// only — a helper reachable from tests is not one. These helpers exist solely to
// build, run or inspect a fixture, and a failure in them is an environment or
// programming error that must stop the run loudly rather than be swallowed.
// Kept as narrow, per-function allows so a future helper does not inherit the
// exemption silently.
#[allow(clippy::expect_used)]
/// A fresh, empty directory for one test's intermediate `.iamf` and `.wav`
/// (T-01-49).
///
/// Uniqueness is per process **and** per call: two tests running under the
/// default thread pool never share an output path, so a comparison can never
/// pass or fail for a reason unrelated to the bytes. Kept under `target/` so it
/// is gitignored and still inspectable after a failure.
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
// The reference runners — they report, they never interpret
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

    /// The output layout a fixture's own loudspeaker layout implies.
    ///
    /// **Derived, never a constant.** CONF-01 requires the harness to carry no
    /// codec- or fixture-specific value: what it hands the reference comes from
    /// the descriptors it just wrote.
    fn for_layout(layout: LoudspeakerLayout) -> Result<Self, String> {
        match layout {
            LoudspeakerLayout::Ch5_1 => Ok(Self::SoundSystemB),
            LoudspeakerLayout::Stereo | LoudspeakerLayout::Binaural => Ok(Self::SoundSystemA),
            other => Err(format!(
                "no iamfdec output layout is modelled for {other:?}; adding one is a deliberate \
                 act, not a default"
            )),
        }
    }
}

/// Everything one reference invocation produced, with **nothing interpreted**.
///
/// The exit code, the two streams and the output path are four independent
/// observations. Conflating them is how the wrong one gets chosen as the
/// signal, which is precisely the mistake Experiments A and 1 recorded.
#[derive(Debug)]
struct ToolRun {
    /// The process exit code. Recorded for the report, asserted on by nothing.
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    /// Where the output was asked to go. May not exist.
    output: PathBuf,
    /// The exact argv, for the failure message and for `CONFORMANCE-GATE.md`.
    command: String,
}

/// Run a command and report what happened, interpreting nothing.
fn run_tool(program: &str, args: &[String], output: &Path) -> ToolRun {
    let command = format!("{program} {}", args.join(" "));
    match Command::new(program).args(args).output() {
        Ok(out) => ToolRun {
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            output: output.to_path_buf(),
            command,
        },
        Err(e) => ToolRun {
            exit_code: None,
            stdout: String::new(),
            stderr: format!("failed to spawn: {e}"),
            output: output.to_path_buf(),
            command,
        },
    }
}

/// Run `iamfdec` once.
///
/// `-r` and `-d` are **arguments**, never constants: a sample rate that
/// disagrees with the file's Codec Config silently drives libiamf's speex
/// resampler (the default is 48000), which produces a comparison failure for a
/// reason that has nothing to do with the bitstream (T-01-50).
fn run_iamfdec(
    decoder: &Path,
    input: &Path,
    output: &Path,
    spec: ElementSpec,
    layout: OutputLayout,
    disable_limiter: bool,
) -> ToolRun {
    let mut args: Vec<String> = vec![
        "-i0".to_owned(),
        "-o3".to_owned(),
        output.display().to_string(),
        "-r".to_owned(),
        spec.sample_rate.to_string(),
        layout.flag().to_owned(),
        "-d".to_owned(),
        spec.sample_size.to_string(),
    ];
    if disable_limiter {
        args.push("-disable_limiter".to_owned());
    }
    args.push(input.display().to_string());
    run_tool(&decoder.display().to_string(), &args, output)
}

// ---------------------------------------------------------------------------
// WAV — read and write, adjacent, so an asymmetry is visible
// ---------------------------------------------------------------------------

/// A bare canonical WAV header: `RIFF____WAVE` + `fmt ` (8 + 16) + `data` (8).
///
/// The threshold the "output exists and is larger than a bare header"
/// assertion uses. Experiment A produced exactly 44 bytes twice, from two
/// different failures, and Experiment 1 produced 80 bytes from a third.
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

/// Parse a WAV by walking its chunks, never by trusting a magic offset.
///
/// A `fmt ` chunk that is not 16 bytes, or an extra `LIST` chunk, would shift
/// every sample by an unknown amount if the reader assumed offset 44 — the same
/// class of silent-wrong-answer failure the whole gate exists to refuse.
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
        let advance = size
            .saturating_add(8)
            .saturating_add(size.checked_rem(2).unwrap_or(0));
        cursor = cursor.saturating_add(advance.max(8));
    }

    let (start, end) = data.ok_or_else(|| format!("{} has no data chunk", path.display()))?;
    let payload = bytes.get(start..end).unwrap_or_default();
    let bytes_per_sample = usize::from(bits_per_sample).div_ceil(8);
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

/// Write a canonical 44-byte-header PCM WAV — the input `encoder_main` reads.
///
/// Adjacent to [`read_wav`] on purpose: read and write in the same file, in
/// that order, is how an asymmetry becomes visually obvious, and asymmetry is
/// the failure mode that produces files that almost work.
///
/// **WAV PCM is little-endian at every depth**, including the three-byte one —
/// the opposite of `sample_format_flags == 0`, which is the whole of DESC-03.
fn write_wav(path: &Path, samples: &[i32], spec: ElementSpec) -> Result<(), String> {
    let bytes_per_sample = spec.bytes_per_sample();
    let block_align = spec.channels.saturating_mul(bytes_per_sample);
    let byte_rate = usize::try_from(spec.sample_rate)
        .unwrap_or(0)
        .saturating_mul(block_align);
    // WAV is always little-endian, whatever the Codec Config declares.
    let data = store_interleaved(samples, spec.sample_size, false);

    let u32le = |value: usize| u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes();
    let u16le = |value: usize| u16::try_from(value).unwrap_or(u16::MAX).to_le_bytes();

    let mut out = Vec::with_capacity(data.len().saturating_add(44));
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&u32le(data.len().saturating_add(36)));
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&u32le(16));
    out.extend_from_slice(&u16le(1)); // PCM
    out.extend_from_slice(&u16le(spec.channels));
    out.extend_from_slice(&u32le(usize::try_from(spec.sample_rate).unwrap_or(0)));
    out.extend_from_slice(&u32le(byte_rate));
    out.extend_from_slice(&u16le(block_align));
    out.extend_from_slice(&u16le(usize::from(spec.sample_size)));
    out.extend_from_slice(b"data");
    out.extend_from_slice(&u32le(data.len()));
    out.extend_from_slice(&data);

    std::fs::write(path, &out).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// A little-endian signed sample of 1–4 bytes, sign-extended into `i32`.
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
// One round trip through `iamfdec`, decoded twice
// ---------------------------------------------------------------------------

/// One round trip, with every precondition a comparison depends on asserted
/// **before** any comparison is made.
#[derive(Debug)]
struct RoundTrip {
    encoded_frames: usize,
    decoded_frames: usize,
    /// The limiter-disabled decode, interleaved.
    decoded: Vec<i32>,
    /// Interleaved samples differing between input and the limiter-disabled
    /// decode.
    differing: usize,
    /// Interleaved samples differing between the two decodes.
    limiter_differing: usize,
}

#[allow(clippy::expect_used)]
/// Encode `fixture` through the real writer, decode it twice, and report.
///
/// Assertion order is fixed and each has its own message: (1) the output exists
/// and exceeds a bare WAV header, (2) the decoded sample count equals the
/// encoded count, and only then (3) is any sample compared — by the caller,
/// which decides what a count of differences means.
fn round_trip(decoder: &Path, fixture: &Fixture, dir: &Path) -> RoundTrip {
    let spec = fixture.spec().expect("the fixture's spec resolves");
    let layout = OutputLayout::for_layout(spec.layout).expect("the layout maps to an -s flag");
    let pcm = fixture.single_pcm();
    let tag = fixture.name;

    let peak = peak_for(spec.sample_size);
    let observed_peak = pcm.iter().map(|v| v.saturating_abs()).max().unwrap_or(0);
    assert!(
        observed_peak <= peak,
        "{tag}: the signal peaks at {observed_peak}, above the -6 dBFS cap of {peak}. libiamf's \
         -1 dBTP limiter is created unconditionally and would engage, producing a diffuse, \
         amplitude-only, channel-uniform diff that reads exactly like a rounding bug in our own \
         encoder. Do not raise the amplitude 'to make the signal more distinguishable'."
    );

    let bytes = fixture.encode().expect("the fixture encodes");
    let input = dir.join(format!("{tag}.iamf"));
    std::fs::write(&input, &bytes).expect("the .iamf is writable");

    let encoded_frames = pcm.len().checked_div(spec.channels.max(1)).unwrap_or(0);

    // ---- decode 1: limiter disabled — the sample-identity run ---------------
    let nolim = dir.join(format!("{tag}.nolimiter.wav"));
    let run = run_iamfdec(decoder, &input, &nolim, spec, layout, true);
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
    assert_eq!(
        wav.sample_rate, spec.sample_rate,
        "{tag}: the decoder reported {} Hz, the Codec Config declares {}. A rate mismatch means \
         the speex resampler ran, and a resampled comparison fails for a reason unrelated to the \
         bitstream (T-01-50).",
        wav.sample_rate, spec.sample_rate
    );

    let differing = pcm
        .iter()
        .zip(wav.samples.iter())
        .filter(|(a, b)| a != b)
        .count();

    // ---- decode 2: limiter at its default — the drift alarm ----------------
    let lim = dir.join(format!("{tag}.limiter.wav"));
    let run_lim = run_iamfdec(decoder, &input, &lim, spec, layout, false);
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

/// The limiter changed nothing — what a fixture below −6 dBFS guarantees, since
/// `compute_target_gain()` returns exactly 1.0 while no look-ahead peak exceeds
/// the linear threshold of ≈0.8913.
///
/// Asserted **by the caller**, not inside [`round_trip`], for one reason: a
/// round trip whose *decode* is wrong can produce samples the encoder never
/// wrote, and those can exceed the threshold even though the input did not.
/// Baking this into the helper would have made such a case report "the fixture
/// is too loud" — the wrong diagnosis, and one that sends a reader to lower the
/// amplitude instead of finding the real defect.
fn assert_limiter_is_transparent(trip: &RoundTrip, tag: &str) {
    assert_eq!(
        trip.limiter_differing, 0,
        "{tag}: {} samples differ between the limiter-disabled and limiter-enabled decodes. The \
         fixture has drifted above the -1 dBTP threshold (linear ~0.8913) and -disable_limiter is \
         masking it. Lower the amplitude cap; do not remove this check.",
        trip.limiter_differing
    );
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
// The consequence for the fixture is recorded in `CONFORMANCE-GATE.md` as
// Experiment 3 and waiver W-1, and enforced by the three probes below. What
// matters here is what did NOT happen: the 24-bit big-endian combination was not
// quietly dropped, and no tolerance window was introduced to make it pass.

#[allow(clippy::expect_used, clippy::panic)]
/// A stereo probe fixture at one sample format — the smallest file that answers
/// the question.
fn probe_fixture(name: &'static str, sample_size: u8, flags: SampleFormatFlags) -> Fixture {
    let config = CodecConfig::lpcm(
        200,
        fixture::FRAME_SIZE,
        LpcmDecoderConfig {
            // Zero is BIG-endian (DESC-03) — the opposite of a WAV-shaped
            // assumption, and the sense the probe exists to test.
            sample_format_flags: flags,
            sample_size,
            sample_rate: fixture::SAMPLE_RATE,
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
    let mix_gain = MixGainParamDefinition::mode_1(100, fixture::SAMPLE_RATE);
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
            // For a stereo fixture the one Sound System A layout is both the
            // comparison target and the layout iamf-tools hard-checks on write.
            // A 5.1 fixture needs two.
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(-6144, -1536),
            }],
        }],
        trailing: Vec::new(),
    };

    let pcm = ramp_pcm(fixture::SAMPLE_FRAMES, 2, peak_for(sample_size));
    Fixture {
        name,
        descriptors: DescriptorSet {
            sequence_header: IaSequenceHeader::new(primary.to_wire(), additional.to_wire()),
            codec_configs: vec![config],
            audio_elements: vec![element],
            mix_presentations: vec![presentation],
        },
        expected_pcm: vec![pcm.clone()],
        frame_sources: vec![FrameSource::LpcmInterleaved(pcm)],
    }
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
    let probe = probe_fixture("probe24le", 24, SampleFormatFlags::LittleEndian);
    let dir = scratch_dir("probe24le");
    let trip = round_trip(&decoder, &probe, &dir);
    println!(
        "A1 RESULT (24-bit LE): {} frames in, {} out, {} of {} samples differ, limiter delta {}",
        trip.encoded_frames,
        trip.decoded_frames,
        trip.differing,
        probe.single_pcm().len(),
        trip.limiter_differing
    );
    assert_eq!(
        trip.differing,
        0,
        "{} of {} samples differ after a 24-bit little-endian round trip. The 24-bit depth itself \
         is supposed to be exact — if this fails, the defect is ours, not upstream's.",
        trip.differing,
        probe.single_pcm().len()
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
    let probe = probe_fixture("probe16be", 16, SampleFormatFlags::BigEndian);
    let dir = scratch_dir("probe16be");
    let trip = round_trip(&decoder, &probe, &dir);
    println!(
        "A1 RESULT (16-bit BE): {} frames in, {} out, {} of {} samples differ, limiter delta {}",
        trip.encoded_frames,
        trip.decoded_frames,
        trip.differing,
        probe.single_pcm().len(),
        trip.limiter_differing
    );
    assert_eq!(
        trip.differing,
        0,
        "{} of {} samples differ after a 16-bit big-endian round trip. reads16be uses readu16be \
         and is correct at the pinned SHA — if this fails, the defect is ours.",
        trip.differing,
        probe.single_pcm().len()
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
/// of waiver W-1 in `CONFORMANCE-GATE.md`: go read that entry and move the
/// sample-identity fixture back to 24-bit big-endian.
///
/// It is not bug-compatibility. Nothing here makes a comparison pass; the
/// waiver records the coverage that moved, and the byte-level correctness of our
/// own 24-bit big-endian writer is asserted by hand-computed vectors in
/// `tests/fixture.rs`, which no permissive reader is involved in.
#[test]
fn probe_24bit_big_endian_hits_the_upstream_reads24be_defect() {
    let Some(decoder) = reference_decoder() else {
        skip_no_reference("probe_24bit_big_endian_hits_the_upstream_reads24be_defect");
        return;
    };
    let probe = probe_fixture("probe24be", 24, SampleFormatFlags::BigEndian);
    let dir = scratch_dir("probe24be");
    let pcm = probe.single_pcm().to_vec();
    let trip = round_trip(&decoder, &probe, &dir);
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
    // loud" — the wrong diagnosis. The input itself is at -6 dBFS, which
    // `round_trip` asserted before encoding anything.
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
        predicted.len(),
        trip.decoded.len(),
        "the prediction and the decode must cover the same samples"
    );
    assert_eq!(
        unexplained,
        0,
        "{unexplained} of {} decoded samples are NOT explained by libiamf@v1.1.0's reads24be \
         defect (bitstream.c:206-210 uses readu16le where readu16be was meant). The diagnosis in \
         CONFORMANCE-GATE.md is therefore incomplete — do not proceed on it.",
        predicted.len()
    );

    // And the defect is real, not a no-op: it must actually have changed the
    // samples. If this ever reads 0, `reads24be` was fixed upstream and waiver
    // W-1's restoration condition has been met.
    assert!(
        trip.differing > 0,
        "the 24-bit big-endian round trip is now EXACT. reads24be appears to have been fixed \
         upstream (or REFERENCES.md's libiamf pin moved). This is the restoration condition of \
         waiver W-1 in CONFORMANCE-GATE.md: remove the waiver and move the sample-identity \
         fixture back to 24-bit big-endian, which is what D-18 originally specified."
    );
}

/// The defective read, checked against values computed by hand from the code —
/// so the diagnosis above stands on its own with no reference binary present
/// (CONF-10).
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

/// The WAV writer and reader are inverses, which is what lets `encoder_main` be
/// handed the same samples our encoder was.
#[test]
fn the_wav_writer_and_reader_round_trip() {
    let dir = scratch_dir("wavrt");
    for sample_size in [16_u8, 24] {
        let spec = ElementSpec {
            sample_rate: 48_000,
            sample_size,
            num_samples_per_frame: 128,
            codec: FixtureCodec::Lpcm { big_endian: false },
            layout: LoudspeakerLayout::Ch5_1,
            channels: 6,
        };
        let pcm = ramp_pcm(64, 6, peak_for(sample_size));
        let path = dir.join(format!("rt{sample_size}.wav"));
        write_wav(&path, &pcm, spec).expect("the WAV writes");
        let wav = read_wav(&path).expect("the WAV reads back");
        assert_eq!(wav.channels, 6);
        assert_eq!(wav.sample_rate, 48_000);
        assert_eq!(wav.bits_per_sample, u16::from(sample_size));
        assert_eq!(wav.frames, 64);
        assert_eq!(wav.samples, pcm, "{sample_size}-bit WAV round trip");
    }
}

// ===========================================================================
// CONF-01 — `assert_conformant`, the reusable harness
// ===========================================================================

/// Everything the gate observed about one fixture, so a caller can report as
/// well as assert.
#[derive(Debug, Default)]
struct GateReport {
    lines: Vec<String>,
}

impl GateReport {
    fn note(&mut self, clause: &str, verdict: &str) {
        self.lines.push(format!("{clause}: {verdict}"));
    }

    fn print(&self, tag: &str) {
        for line in &self.lines {
            println!("[{tag}] {line}");
        }
    }
}

/// **CONF-01.** Assert that `fixture` produces a conformant IA Sequence,
/// clause by clause.
///
/// A **function, not a test body**, and that is the whole requirement: Phase 3
/// reuses it for FLAC and Opus. It takes the codec-neutral fixture and contains
/// no codec-specific packet handling — sample rate, sample size, frame size and
/// output layout are read back out of the Codec Config it just wrote, while the
/// comparison PCM is the fixture's independently derived oracle.
///
/// # Clause order is load-bearing
///
/// **Clause 0 runs first**: the reference-manifest assertion (D-13). A
/// conformance suite that ran green against the wrong reference is worse than
/// one that did not run, because it reports confidence it has not earned.
///
/// Then the offline clauses (CONF-02, CONF-03, CONF-04), which need no reference
/// binary at all; then CONF-05 through `iamfdec`; then CONF-06 through
/// `iamf-tools`' stricter parser. Each reference-dependent clause **skips with a
/// printed reason** when its tool is absent, and the offline ones still run —
/// that is CONF-10, and it is what keeps the four-target PR gate fast and
/// cross-platform.
///
/// CONF-07 and CONF-08 are deliberately **not** here. CONF-07 needs a companion
/// file produced by `encoder_main` from a description of the same configuration
/// in `iamf-tools`' own proto dialect, which is a second input this signature
/// does not take; it lives in [`assert_byte_diff_matches_ledger`]. CONF-08 is a
/// claim about a different file entirely — the vendored `test_000003.iamf` — and
/// belongs to `tests/sequence.rs`, which this file asserts still holds rather
/// than re-implementing.
///
/// # Errors
///
/// Returns the first clause failure as a message naming the clause. Panicking
/// assertions are used only for the invariants a caller could not act on.
fn assert_conformant(fixture: &Fixture) -> Result<GateReport, String> {
    let mut report = GateReport::default();

    // ---- clause 0 — the reference is the pinned one (D-13) -----------------
    assert_reference_manifest_matches()?;
    report.note("clause 0 (D-13 manifest)", "reference pin confirmed");

    let spec = fixture.spec()?;
    let pcm = fixture.single_pcm();
    let bytes = fixture.encode()?;

    // ---- CONF-02 — the signal is non-silent and per-channel-distinguishable -
    assert_signal_is_distinguishable(pcm, spec)?;
    report.note(
        "CONF-02 (signal)",
        &format!(
            "{} channels, each distinguishable at every index, peak <= -6 dBFS",
            spec.channels
        ),
    );

    // ---- CONF-03 — the length forces a non-zero, differing end trim ---------
    let (trim_at_end, trim_at_start) = assert_fixture_trim_is_forced(fixture, spec)?;
    report.note(
        "CONF-03 (trim)",
        &format!(
            "trim_at_end = {trim_at_end}, trim_at_start = {trim_at_start}, and the two differ"
        ),
    );

    // ---- CONF-04 — structure is observable in our own output ---------------
    let structure = assert_structure_is_observable(&bytes)?;
    report.note("CONF-04 (structure)", &structure);

    // ---- CONF-05 — libiamf decodes it and the PCM is identical -------------
    match reference_decoder() {
        None => {
            skip_no_reference("CONF-05");
            report.note("CONF-05 (libiamf sample identity)", "SKIPPED — no iamfdec");
        }
        Some(decoder) => {
            let dir = scratch_dir(fixture.name);
            let trip = round_trip(&decoder, fixture, &dir);
            assert_limiter_is_transparent(&trip, fixture.name);
            if trip.differing != 0 {
                // D-19: report a candidate permutation, never apply one.
                let diagnosis = describe_channel_mismatch(pcm, &trip.decoded, spec.layout)
                    .unwrap_or_else(|| "PCM differs".to_owned());
                return Err(format!(
                    "CONF-05 FAILED for {}: {} of {} samples differ after a round trip through \
                     the pinned libiamf.\n{diagnosis}",
                    fixture.name,
                    trip.differing,
                    pcm.len()
                ));
            }
            report.note(
                "CONF-05 (libiamf sample identity)",
                &format!(
                    "{} sample frames, 0 of {} samples differ, limiter delta 0",
                    trip.decoded_frames,
                    pcm.len()
                ),
            );
        }
    }

    // ---- CONF-06 — iamf-tools' stricter parser accepts the file ------------
    if iamf_tools_available() {
        let dir = scratch_dir(&format!("{}-conf06", fixture.name));
        let observed = run_decoder_main(&bytes, &dir, expected_temporal_units(fixture, spec)?)?;
        report.note("CONF-06 (iamf-tools parser)", &observed);
    } else {
        skip_no_container("CONF-06");
        report.note("CONF-06 (iamf-tools parser)", "SKIPPED — no container");
    }

    Ok(report)
}

/// **Clause 0 (D-13).** The reference actually on disk is the one
/// `REFERENCES.md` pins.
///
/// Delegated to `tests/reference_manifest.rs`'s own assertions by re-reading the
/// same two files, rather than duplicating the scanner: this is the *ordering*
/// requirement — that it runs before any other clause — not a second
/// implementation of the check.
fn assert_reference_manifest_matches() -> Result<(), String> {
    if reference_decoder().is_none() {
        // Nothing to drift against. `tests/reference_manifest.rs` prints the
        // same skip; this clause is vacuously satisfied offline.
        return Ok(());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join(".reference-manifest.json"))
        .map_err(|e| format!("clause 0: cannot read .reference-manifest.json: {e}"))?;
    let references = std::fs::read_to_string(root.join("REFERENCES.md"))
        .map_err(|e| format!("clause 0: cannot read REFERENCES.md: {e}"))?;

    // The two files spell the projects differently — `REFERENCES.md` uses the
    // repository name, the manifest uses a JSON key — so each side gets its own
    // needle. `tests/reference_manifest.rs` makes the same distinction; getting
    // it wrong here would turn clause 0 into an assertion that always fails,
    // which is a different way of not running the gate.
    for (project, reference_needle, manifest_key) in [
        ("libiamf", "libiamf", "libiamf_sha"),
        ("iamf-tools", "iamf-tools", "iamf_tools_sha"),
    ] {
        let built = sha_on_line_naming(&manifest, manifest_key);
        let pinned = sha_on_line_naming(&references, reference_needle);
        if built.is_none() || built != pinned {
            return Err(format!(
                "clause 0: manifest disagrees with REFERENCES.md for {project}: built {built:?}, \
                 pinned {pinned:?}. A conformance suite that ran green against the wrong \
                 reference is worse than one that did not run."
            ));
        }
    }
    Ok(())
}

/// The first 40-hex-character token on the first line naming `needle`.
fn sha_on_line_naming(haystack: &str, needle: &str) -> Option<String> {
    let needle_lc = needle.to_ascii_lowercase();
    haystack
        .lines()
        .filter(|line| line.to_ascii_lowercase().contains(&needle_lc))
        .find_map(|line| {
            line.split(|c: char| !c.is_ascii_hexdigit())
                .find(|run| {
                    run.len() == 40
                        && run
                            .chars()
                            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
                })
                .map(str::to_owned)
        })
}

/// **CONF-02.** The signal is non-silent and every channel's sample sequence
/// differs from every other channel's — which is what makes a swap
/// arithmetically identifiable rather than invisible.
fn assert_signal_is_distinguishable(pcm: &[i32], spec: ElementSpec) -> Result<(), String> {
    if pcm.iter().all(|value| *value == 0) {
        return Err("CONF-02: the signal is silent, so nothing about it is observable".to_owned());
    }
    let channels = spec.channels.max(1);
    let channel_of =
        |index: usize| -> Vec<i32> { pcm.iter().skip(index).step_by(channels).copied().collect() };
    for a in 0..channels {
        let left = channel_of(a);
        if left.iter().all(|value| *value == 0) {
            return Err(format!("CONF-02: channel {a} is silent"));
        }
        for b in a.saturating_add(1)..channels {
            if left == channel_of(b) {
                return Err(format!(
                    "CONF-02: channels {a} and {b} carry identical samples, so a swap between \
                     them would be undetectable — which is precisely the BCG packing failure the \
                     5.1 fixture exists to catch"
                ));
            }
        }
    }
    let peak = peak_for(spec.sample_size);
    let observed = pcm.iter().map(|v| v.saturating_abs()).max().unwrap_or(0);
    if observed > peak {
        return Err(format!(
            "CONF-02: the signal peaks at {observed}, above the -6 dBFS cap of {peak}"
        ));
    }
    Ok(())
}

/// **CONF-03.** The total sample count is not a multiple of the frame size, so
/// the final frame carries `0 < trim_at_end < num_samples_per_frame` while
/// `trim_at_start` stays 0.
///
/// Computed with **checked integer arithmetic and no float**, from the encoded
/// sample count. Returns the end trim.
fn assert_trim_is_forced(pcm: &[i32], spec: ElementSpec) -> Result<u32, String> {
    let frames = u64::try_from(pcm.len().checked_div(spec.channels.max(1)).unwrap_or(0))
        .map_err(|e| format!("CONF-03: sample count does not fit u64: {e}"))?;
    let per_frame = u64::from(spec.num_samples_per_frame);
    if per_frame == 0 {
        return Err("CONF-03: num_samples_per_frame is 0".to_owned());
    }
    if frames.checked_rem(per_frame) == Some(0) {
        return Err(format!(
            "CONF-03: {frames} samples is an exact multiple of {per_frame}, so trim_at_end would \
             be 0 and equal to trim_at_start. A swapped END/START write order is invisible \
             whenever the two are equal (OBU-05)."
        ));
    }
    let plan = plan_frames(frames, spec.num_samples_per_frame)
        .map_err(|e| format!("CONF-03: plan_frames: {e:?}"))?;
    if plan.trim_at_end == 0 || plan.trim_at_end >= spec.num_samples_per_frame {
        return Err(format!(
            "CONF-03: trim_at_end is {}, which is not strictly between 0 and {}",
            plan.trim_at_end, spec.num_samples_per_frame
        ));
    }
    let last = plan.frame_count.saturating_sub(1);
    let trimming = plan
        .trimming_for(last)
        .ok_or_else(|| "CONF-03: the final frame carries no trimming".to_owned())?;
    if trimming.at_start != 0 {
        return Err(format!(
            "CONF-03: trim_at_start is {}, but LPCM has no priming — the two trims must differ",
            trimming.at_start
        ));
    }
    if trimming.at_end == trimming.at_start {
        return Err(
            "CONF-03: the two trim values are equal, so a swapped write order is \
                    invisible"
                .to_owned(),
        );
    }
    Ok(plan.trim_at_end)
}

/// Codec-neutral CONF-03: LPCM retains the Phase 1 arithmetic above; committed
/// access units prove the same retained-length equation from their explicit
/// first/start and final/end trims.
fn assert_fixture_trim_is_forced(
    fixture: &Fixture,
    spec: ElementSpec,
) -> Result<(u32, u32), String> {
    let source = fixture
        .frame_sources
        .first()
        .ok_or_else(|| "CONF-03: the fixture has no frame source".to_owned())?;
    match source {
        FrameSource::LpcmInterleaved(pcm) => {
            assert_trim_is_forced(pcm, spec).map(|at_end| (at_end, 0))
        }
        FrameSource::PreEncoded(units) => {
            let first = units
                .first()
                .ok_or_else(|| "CONF-03: the committed source has no temporal units".to_owned())?;
            let last = units.last().ok_or_else(|| {
                "CONF-03: the committed source has no final temporal unit".to_owned()
            })?;
            for (index, unit) in units.iter().enumerate() {
                let trimming = unit.trimming.unwrap_or(Trimming {
                    at_end: 0,
                    at_start: 0,
                });
                if index.saturating_add(1) != units.len() && trimming.at_end != 0 {
                    return Err(format!(
                        "CONF-03: temporal unit {index} carries trim_at_end {}, but only the \
                         final temporal unit may trim at end",
                        trimming.at_end
                    ));
                }
                if index != 0 && trimming.at_start != 0 {
                    return Err(format!(
                        "CONF-03: temporal unit {index} carries trim_at_start {}, but only the \
                         first temporal unit may trim at start",
                        trimming.at_start
                    ));
                }
            }
            let at_start = first.trimming.map_or(0, |trim| trim.at_start);
            let at_end = last.trimming.map_or(0, |trim| trim.at_end);
            if at_end == 0 {
                return Err(
                    "CONF-03: trim_at_end is 0, but the final temporal unit must carry a \
                     strictly positive end trim"
                        .to_owned(),
                );
            }
            if at_start >= spec.num_samples_per_frame || at_end >= spec.num_samples_per_frame {
                return Err(format!(
                    "CONF-03: trims ({at_end} end, {at_start} start) are not less than the \
                     frame size {}",
                    spec.num_samples_per_frame
                ));
            }
            if at_end == at_start {
                return Err(
                    "CONF-03: the two trim values are equal, so a swapped write order is \
                     invisible"
                        .to_owned(),
                );
            }

            let unit_count = u64::try_from(units.len())
                .map_err(|e| format!("CONF-03: temporal-unit count does not fit u64: {e}"))?;
            let capacity = unit_count
                .checked_mul(u64::from(spec.num_samples_per_frame))
                .ok_or_else(|| "CONF-03: temporal-unit capacity overflows u64".to_owned())?;
            let retained = capacity
                .checked_sub(u64::from(at_start))
                .and_then(|value| value.checked_sub(u64::from(at_end)))
                .ok_or_else(|| "CONF-03: trimming exceeds encoded capacity".to_owned())?;
            let expected = u64::try_from(
                fixture
                    .single_pcm()
                    .len()
                    .checked_div(spec.channels.max(1))
                    .unwrap_or(0),
            )
            .map_err(|e| format!("CONF-03: expected sample count does not fit u64: {e}"))?;
            if retained != expected {
                return Err(format!(
                    "CONF-03: {unit_count} units × {} samples − {at_start} start − {at_end} end \
                     retains {retained} frames, expected {expected}",
                    spec.num_samples_per_frame
                ));
            }
            Ok((at_end, at_start))
        }
    }
}

#[test]
fn conf_03_rejects_an_end_trim_on_a_middle_pre_encoded_unit() {
    let mut fixture = fixture::sample_identity();
    fixture.frame_sources = vec![FrameSource::PreEncoded(vec![
        EncodedTemporalUnit {
            trimming: None,
            substream_payloads: vec![Vec::new(); 4],
        },
        EncodedTemporalUnit {
            trimming: Some(Trimming {
                at_end: 1,
                at_start: 0,
            }),
            substream_payloads: vec![Vec::new(); 4],
        },
        EncodedTemporalUnit {
            trimming: Some(Trimming {
                at_end: 84,
                at_start: 0,
            }),
            substream_payloads: vec![Vec::new(); 4],
        },
    ])];
    let spec = fixture.spec().expect("its spec resolves");
    let error = assert_fixture_trim_is_forced(&fixture, spec)
        .expect_err("only the final temporal unit may trim at end");
    assert!(
        error.contains("temporal unit 1") && error.contains("trim_at_end 1"),
        "unexpected error: {error}"
    );
}

#[test]
fn conf_03_requires_a_positive_end_trim_for_pre_encoded_units() {
    let mut fixture = fixture::sample_identity();
    fixture.frame_sources = vec![FrameSource::PreEncoded(vec![
        EncodedTemporalUnit {
            trimming: Some(Trimming {
                at_end: 0,
                at_start: 84,
            }),
            substream_payloads: vec![Vec::new(); 4],
        },
        EncodedTemporalUnit {
            trimming: None,
            substream_payloads: vec![Vec::new(); 4],
        },
        EncodedTemporalUnit {
            trimming: None,
            substream_payloads: vec![Vec::new(); 4],
        },
    ])];
    let spec = fixture.spec().expect("its spec resolves");
    let error = assert_fixture_trim_is_forced(&fixture, spec)
        .expect_err("CONF-03 always requires a positive final end trim");
    assert!(
        error.contains("trim_at_end is 0") && error.contains("strictly positive"),
        "unexpected error: {error}"
    );
}

/// **CONF-04.** Structure is observable in our own output: at least six OBUs,
/// the descriptors in the reference's write order, and the boundary walk landing
/// exactly on `bytes.len()` (OBU-08).
fn assert_structure_is_observable(bytes: &[u8]) -> Result<String, String> {
    let boundaries = find_obu_boundaries(bytes)
        .map_err(|e| format!("CONF-04: the OBU chain does not walk: {e:?}"))?;
    if boundaries.last().copied() != Some(bytes.len()) {
        return Err(format!(
            "CONF-04: the final OBU boundary is {:?}, not bytes.len() = {}. A boundary that \
             stops short is how libiamf's own splitter reports an oversized final size — as a \
             SHORTER FILE rather than a decode failure (OBU-08).",
            boundaries.last(),
            bytes.len()
        ));
    }
    let count = boundaries.len().saturating_sub(1);
    if count < 6 {
        return Err(format!(
            "CONF-04: {count} OBUs, fewer than the six ordering observability needs"
        ));
    }

    let mut types = Vec::with_capacity(count);
    for start in boundaries.iter().take(count) {
        let rest = bytes.get(*start..).unwrap_or_default();
        let mut cursor = iamf::bits::BitCursor::new(rest);
        let (header, _size) = read_obu_header(&mut cursor)
            .map_err(|e| format!("CONF-04: OBU at {start} has an unparsable header: {e:?}"))?;
        types.push(header.obu_type);
    }
    if types.first() != Some(&ObuType::IaSequenceHeader) {
        return Err(format!(
            "CONF-04: the first OBU is {:?}, not the IA Sequence Header",
            types.first()
        ));
    }
    // Descriptors come first, in the reference's order: header, Codec Configs,
    // Audio Elements, Mix Presentations. Nothing after the first Audio Frame may
    // be a descriptor.
    let first_frame = types
        .iter()
        .position(|obu_type| {
            !matches!(
                obu_type,
                ObuType::IaSequenceHeader
                    | ObuType::CodecConfig
                    | ObuType::AudioElement
                    | ObuType::MixPresentation
            )
        })
        .unwrap_or(types.len());
    let prologue = types.get(..first_frame).unwrap_or_default();
    let ordered: Vec<u8> = prologue
        .iter()
        .map(|obu_type| match obu_type {
            ObuType::IaSequenceHeader => 0_u8,
            ObuType::CodecConfig => 1,
            ObuType::AudioElement => 2,
            _ => 3,
        })
        .collect();
    if ordered.windows(2).any(|pair| pair.first() > pair.get(1)) {
        return Err(format!(
            "CONF-04: the descriptor prologue is out of order: {prologue:?}. The reference writes \
             the IA Sequence Header, then Codec Configs ascending by id, then Audio Elements \
             ascending by id, then Mix Presentations in LIST order (DESC-08)."
        ));
    }

    let codec_configs = types
        .iter()
        .filter(|obu_type| **obu_type == ObuType::CodecConfig)
        .count();
    let elements = types
        .iter()
        .filter(|obu_type| **obu_type == ObuType::AudioElement)
        .count();
    Ok(format!(
        "{count} OBUs, {codec_configs} Codec Config(s), {elements} Audio Element(s), prologue in \
         write order, final boundary lands exactly on len() = {}",
        bytes.len()
    ))
}

/// How many temporal units a fixture carries — one per frame.
fn expected_temporal_units(fixture: &Fixture, spec: ElementSpec) -> Result<u64, String> {
    match fixture.frame_sources.first() {
        Some(FrameSource::LpcmInterleaved(pcm)) => {
            let frames = u64::try_from(pcm.len().checked_div(spec.channels.max(1)).unwrap_or(0))
                .map_err(|e| format!("sample count does not fit u64: {e}"))?;
            plan_frames(frames, spec.num_samples_per_frame)
                .map(|plan| plan.frame_count)
                .map_err(|e| format!("plan_frames: {e:?}"))
        }
        Some(FrameSource::PreEncoded(units)) => u64::try_from(units.len())
            .map_err(|e| format!("temporal-unit count does not fit u64: {e}")),
        None => Err("the fixture has no frame source".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// CONF-06 — `iamf-tools`' stricter parser, through `decoder_main`
// ---------------------------------------------------------------------------

/// Run `decoder_main` on `bytes` and assert on the observable Experiment 1
/// recorded.
///
/// **`iamf-tools@v2.1.0` ships no `probe_main`.** Its `iamf/cli/BUILD` declares
/// exactly two `cc_binary` targets, `decoder_main` and `encoder_main`, so CONF-06
/// goes through `decoder_main`'s parse path — `ObuProcessor` /
/// `DescriptorObuParser`, which *is* the strict parser. That is a route chosen
/// because the alternative does not exist, and it should read as a deliberate
/// decision rather than as a missing validator.
///
/// The signal is `Decoded <N> temporal units.` on stderr with N equal to the
/// expected count, **plus an explicit grep for `Check failure stack trace`**
/// because an absl abort produces no count line at all. Not the exit code:
/// Experiment 1 recorded exit 0 from three of five corrupted inputs, one of them
/// a file truncated mid-OBU that wrote an 80-byte WAV while decoding zero
/// temporal units — which `test -s` also passes.
///
/// This is the only clause that can catch reserved-bit misuse or leb128
/// strictness, because `libiamf` ignores both — though Experiment 1 showed
/// `decoder_main` is blind to *that particular* reserved bit too, which is why
/// CONF-07's byte diff exists as well.
fn run_decoder_main(bytes: &[u8], dir: &Path, expected_units: u64) -> Result<String, String> {
    let input = dir.join("conf06.iamf");
    std::fs::write(&input, bytes).map_err(|e| format!("CONF-06: cannot write input: {e}"))?;
    // A unique output filename per invocation (T-01-49): the scratch directory
    // is already unique per call, so the name inside it is stable and safe.
    let output = dir.join("conf06.decoder_main.wav");

    let mount = format!("{}:/work", dir.display());
    let args: Vec<String> = vec![
        "run".to_owned(),
        "--rm".to_owned(),
        "-v".to_owned(),
        mount,
        "-w".to_owned(),
        "/src".to_owned(),
        iamf_tools_image(),
        "bazel-bin/iamf/cli/decoder_main".to_owned(),
        "--input_filename=/work/conf06.iamf".to_owned(),
        "--output_filename=/work/conf06.decoder_main.wav".to_owned(),
    ];
    let run = run_tool("docker", &args, &output);
    let streams = format!("{}\n{}", run.stdout, run.stderr);

    if streams.contains("Check failure stack trace") {
        return Err(format!(
            "CONF-06 FAILED: decoder_main died on an absl CHECK abort, which produces no \
             temporal-unit line at all.\ncommand: {}\nstderr: {}",
            run.command, run.stderr
        ));
    }

    let expected_line = format!("Decoded {expected_units} temporal units.");
    if !streams.contains(&expected_line) {
        let reported = streams
            .lines()
            .find(|line| line.contains("temporal units"))
            .unwrap_or("<no temporal-unit line at all>");
        return Err(format!(
            "CONF-06 FAILED: expected {expected_line:?} on stderr; got {reported:?}. The exit \
             code was {:?} and is NOT the signal — Experiment 1 recorded exit 0 from a file \
             truncated mid-OBU that decoded ZERO temporal units and still wrote an 80-byte \
             WAV.\ncommand: {}\nstderr: {}",
            run.exit_code, run.command, run.stderr
        ));
    }
    Ok(format!(
        "decoder_main reported {expected_line:?} (exit {:?}, recorded but not the signal)",
        run.exit_code
    ))
}

// ===========================================================================
// The gate clauses, as named tests
// ===========================================================================

/// **CONF-02, CONF-03, CONF-04, CONF-05, CONF-06 on the sample-identity
/// fixture** — the file `libiamf`'s acceptance defines.
#[test]
fn the_sample_identity_fixture_is_conformant() {
    let fixture = fixture::sample_identity();
    match assert_conformant(&fixture) {
        Ok(report) => report.print(fixture.name),
        Err(message) => panic!("{message}"),
    }
}

/// **DESC-03 through the reference.** The endianness fixture — 16-bit
/// big-endian, `sample_format_flags == 0` — round-trips sample-identically.
///
/// This clause is what waiver W-1 names as the weaker property asserted in place
/// of a 24-bit big-endian round trip. It is not decorative: without it, DESC-03's
/// endianness sense would rest on hand-computed byte vectors alone.
#[test]
fn the_endianness_fixture_is_conformant() {
    let fixture = fixture::endianness();
    match assert_conformant(&fixture) {
        Ok(report) => report.print(fixture.name),
        Err(message) => panic!("{message}"),
    }
}

/// **CONF-04 on the structure-only fixture.**
///
/// Its decoded PCM is a **mix** — `iamfdec` renders every element in the sub-mix
/// into the target layout and sums them — so sample identity is not asserted
/// here and never should be: predicting a mix requires rendering, which is out
/// of scope. What is asserted is structure, which is exactly what Experiment 2's
/// adopted fallback (b) moved onto this file.
#[test]
fn the_structure_only_fixture_shows_its_ordering() {
    let fixture = fixture::structure_only();
    let bytes = fixture.encode().expect("the structure fixture encodes");
    let structure = assert_structure_is_observable(&bytes).expect("its structure is observable");
    println!("[{}] CONF-04: {structure}", fixture.name);
    assert!(
        structure.contains("2 Audio Element(s)"),
        "CONF-04 wants two Audio Elements so ordering is observable: {structure}"
    );
}

/// **CONF-06 on the structure-only fixture**, separately: two Audio Elements is
/// the shape Experiment 2 showed `decoder_main` accepts, and asserting it here
/// is what keeps that finding from silently regressing.
#[test]
fn the_structure_only_fixture_passes_the_strict_parser() {
    if !iamf_tools_available() {
        skip_no_container("the_structure_only_fixture_passes_the_strict_parser");
        return;
    }
    let fixture = fixture::structure_only();
    let bytes = fixture.encode().expect("the structure fixture encodes");
    let spec = fixture.spec().expect("its spec resolves");
    let units = expected_temporal_units(&fixture, spec).expect("its unit count");
    let dir = scratch_dir("structure-conf06");
    match run_decoder_main(&bytes, &dir, units) {
        Ok(observed) => println!("[{}] CONF-06: {observed}", fixture.name),
        Err(message) => panic!("{message}"),
    }
}

/// **CONF-08.** Plan 01-07's whole-file reproduction of `test_000003.iamf` still
/// holds, and `find_obu_boundaries` over that output lands its final boundary
/// exactly on `bytes.len()`.
///
/// Asserted here as a **gate clause** rather than re-implemented: the
/// reproduction itself lives in `tests/sequence.rs`, driven from the published
/// textproto. This asserts the vendored file is present, unmodified and walks
/// cleanly, so a clause that quietly stopped being exercised would be visible.
/// No reference binary is needed.
///
/// **No waiver is available for CONF-08** — see `CONFORMANCE-GATE.md`: the
/// configuration is published, so reproducing it is a mechanical translation
/// rather than archaeology.
#[test]
fn conf_08_the_vendored_golden_still_walks_and_is_unmodified() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("reference")
        .join("test_000003.iamf");
    let bytes = std::fs::read(&path).expect("the vendored test_000003.iamf is present");
    assert_eq!(
        bytes.len(),
        32_567,
        "test_000003.iamf is 32567 bytes (0x7F37); a different length means the vendored file \
         was replaced"
    );
    let boundaries = find_obu_boundaries(&bytes).expect("test_000003 walks");
    assert_eq!(
        boundaries.last().copied(),
        Some(bytes.len()),
        "the final OBU boundary must land exactly on bytes.len() (OBU-08)"
    );
    assert_eq!(
        boundaries.len(),
        68,
        "67 OBU starts plus the final end offset; four descriptors and 63 Audio Frames"
    );
    // The descriptor prologue is 120 bytes, not the 118 PROJECT.md says. A
    // harness written against 118 fails in a way that looks like an encoder bug.
    assert_eq!(
        boundaries.get(4).copied(),
        Some(120),
        "the first Audio Frame OBU begins at offset 120 (0x78)"
    );
}

/// **CONF-09.** Every reference binary is discovered and invoked as a separate
/// process, never linked and never built by a build script.
///
/// A source-level assertion, because this is a property of how the harness is
/// *written*: a `build.rs` added later would not fail any behavioural test — it
/// would make `cargo build` slower and drag CMake, abseil, protobuf and fdk-aac
/// onto every consumer, including Parallax and docs.rs.
#[test]
fn conf_09_no_build_script_and_no_linked_reference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !root.join("build.rs").exists(),
        "a build.rs appeared. Reference tools are invoked by Command and discovered through \
         IAMF_REF_DECODER or a digest-pinned container image; a build script runs unconditionally \
         and cannot be made conditional on 'the developer wants reference tests'."
    );
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    assert!(
        !manifest.contains("[build-dependencies]"),
        "Cargo.toml grew a [build-dependencies] table"
    );
    for forbidden in ["bindgen", "cmake", "cc =", "pkg-config"] {
        assert!(
            !manifest.contains(forbidden),
            "Cargo.toml names {forbidden:?}: the reference is a process, not a link-time \
             dependency"
        );
    }
}

/// **CONF-10.** With no reference binary and no container, the harness still
/// runs its offline clauses and reports the rest as skips.
///
/// Asserted by *calling the offline clauses directly*, so this stays meaningful
/// on a machine where the reference happens to be present — a test that only
/// checked "we are offline" would assert nothing on the developer machine where
/// the tools exist.
#[test]
fn conf_10_the_offline_clauses_need_no_reference_at_all() {
    for fixture in fixture::all() {
        let spec = fixture.spec().expect("its spec resolves");
        let bytes = fixture.encode().expect("it encodes");
        assert_signal_is_distinguishable(fixture.single_pcm(), spec)
            .unwrap_or_else(|e| panic!("{}: {e}", fixture.name));
        assert_trim_is_forced(fixture.single_pcm(), spec)
            .unwrap_or_else(|e| panic!("{}: {e}", fixture.name));
        // Only the structure fixture is required to reach six OBUs; the others
        // are asserted for the boundary walk, which is the part that holds for
        // every file.
        let boundaries = find_obu_boundaries(&bytes).expect("the OBU chain walks");
        assert_eq!(boundaries.last().copied(), Some(bytes.len()));
    }
}

// ===========================================================================
// CONF-07 — the byte diff, as an EXECUTABLE ledger (D-12)
// ===========================================================================
//
// `libiamf` is a permissive reader: Experiment A flipped a reserved bit and got
// a byte-identical WAV back, and Experiment 1 showed `decoder_main` is blind to
// that same bit. So CONF-05 and CONF-06 together still cannot see reserved-bit
// misuse. The byte diff against `iamf-tools`' own encoder, given the same
// configuration, is the clause that can — which is why "passing the libiamf gate
// is necessary and nowhere near sufficient" is the first line of
// CONFORMANCE-GATE.md.
//
// D-12 makes the writeup executable rather than prose: the diff is compared to a
// committed table **exactly, in both directions**. A new difference fails. A
// difference that has *disappeared* also fails, which is what stops success
// criterion 3 passing on a stale document.

/// One byte offset at which our output and `iamf-tools`' differ.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiffRow {
    offset: usize,
    ours: u8,
    theirs: u8,
}

/// One committed row of `DIFF-LEDGER.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LedgerRow {
    offset: usize,
    field: String,
    ours: u8,
    theirs: u8,
    why: String,
}

/// Byte-by-byte differences between two files, in ascending offset order.
///
/// A length difference is reported as a row at the first offset past the shorter
/// file, so "their file is longer" can never masquerade as "no differences".
fn byte_diff(ours: &[u8], theirs: &[u8]) -> Vec<DiffRow> {
    let mut rows: Vec<DiffRow> = ours
        .iter()
        .zip(theirs.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(offset, (a, b))| DiffRow {
            offset,
            ours: *a,
            theirs: *b,
        })
        .collect();
    let common = ours.len().min(theirs.len());
    if ours.len() != theirs.len() {
        rows.push(DiffRow {
            offset: common,
            ours: ours.get(common).copied().unwrap_or(0),
            theirs: theirs.get(common).copied().unwrap_or(0),
        });
    }
    rows.sort_by_key(|row| row.offset);
    rows
}

/// Parse `DIFF-LEDGER.md`'s table.
///
/// The table is the enforcement, so the parser is strict: a row whose recorded
/// `ours` and `theirs` are equal is a **ledger error** and fails, because a row
/// that records no difference cannot be describing one.
fn parse_ledger(text: &str) -> Result<Vec<LedgerRow>, String> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 5 {
            continue;
        }
        // Skip the header row and the separator row.
        let Some(first) = cells.first() else { continue };
        if first.eq_ignore_ascii_case("offset") || first.starts_with('-') || first.starts_with(':')
        {
            continue;
        }
        let offset = parse_offset(first)
            .ok_or_else(|| format!("DIFF-LEDGER.md: {first:?} is not an offset"))?;
        let ours = parse_byte(cells.get(2).copied().unwrap_or(""))
            .ok_or_else(|| format!("DIFF-LEDGER.md: row {first} has an unreadable `ours`"))?;
        let theirs = parse_byte(cells.get(3).copied().unwrap_or(""))
            .ok_or_else(|| format!("DIFF-LEDGER.md: row {first} has an unreadable `theirs`"))?;
        if ours == theirs {
            return Err(format!(
                "DIFF-LEDGER.md: the row at offset {offset} records ours == theirs == 0x{ours:02x}. \
                 A row that records no difference is not describing one — this is a ledger error, \
                 not a passing state."
            ));
        }
        let why = cells.get(4).copied().unwrap_or("").to_owned();
        if why.is_empty() {
            return Err(format!(
                "DIFF-LEDGER.md: the row at offset {offset} has an empty `why`. \"differs at \
                 offset N\" is not an explanation and neither is a blank cell."
            ));
        }
        rows.push(LedgerRow {
            offset,
            field: cells.get(1).copied().unwrap_or("").to_owned(),
            ours,
            theirs,
            why,
        });
    }
    rows.sort_by_key(|row| row.offset);
    Ok(rows)
}

/// `0x1a`, `0X1A` or `26`.
fn parse_offset(text: &str) -> Option<usize> {
    let cleaned = text.trim().trim_matches('`').replace('_', "");
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        usize::from_str_radix(hex, 16).ok()
    } else {
        cleaned.parse().ok()
    }
}

/// `0x1a`, `1a` or `26`.
fn parse_byte(text: &str) -> Option<u8> {
    let cleaned = text.trim().trim_matches('`').replace('_', "");
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u8::from_str_radix(hex, 16).ok()
    } else {
        u8::from_str_radix(&cleaned, 16)
            .ok()
            .or_else(|| cleaned.parse().ok())
    }
}

/// **D-12, both directions.** The computed diff set and the committed ledger set
/// must be equal, keyed by offset in ascending order.
///
/// - A **new** difference fails: the encoder changed and nobody explained it.
/// - A **vanished** difference fails too, which is the half that matters most:
///   without it the ledger silently becomes a description of a file that no
///   longer exists, and success criterion 3 passes on a stale document.
/// - A row present in both but recording different bytes fails: the ledger is
///   describing a difference other than the one that exists.
///
/// An **empty ledger is the passing state when the diff is empty** — and an
/// empty diff against a non-empty ledger fails, which is what makes committing
/// an empty table meaningfully different from committing no table.
///
/// Reordering `DIFF-LEDGER.md` changes nothing: both sides are compared as sets
/// keyed by offset, sorted ascending.
fn compare_to_ledger(diff: &[DiffRow], ledger: &[LedgerRow]) -> Result<(), String> {
    let mut problems = Vec::new();

    for row in diff {
        match ledger.iter().find(|entry| entry.offset == row.offset) {
            None => problems.push(format!(
                "NEW difference at offset {} (0x{:x}): ours 0x{:02x}, theirs 0x{:02x} — not in \
                 DIFF-LEDGER.md",
                row.offset, row.offset, row.ours, row.theirs
            )),
            Some(entry) if entry.ours != row.ours || entry.theirs != row.theirs => {
                problems.push(format!(
                    "CHANGED difference at offset {} (0x{:x}): the ledger records ours \
                     0x{:02x} / theirs 0x{:02x}, the diff shows ours 0x{:02x} / theirs 0x{:02x}",
                    row.offset, row.offset, entry.ours, entry.theirs, row.ours, row.theirs
                ));
            }
            Some(_) => {}
        }
    }

    for entry in ledger {
        if !diff.iter().any(|row| row.offset == entry.offset) {
            problems.push(format!(
                "VANISHED difference at offset {} (0x{:x}): DIFF-LEDGER.md records it as \
                 {:?} but the files now agree there. Update the ledger — a difference that \
                 disappears must fail, or the document silently becomes a description of a file \
                 that no longer exists.",
                entry.offset, entry.offset, entry.field
            ));
        }
    }

    if problems.is_empty() {
        return Ok(());
    }
    Err(format!(
        "CONF-07 FAILED: the byte diff against iamf-tools' output does not match \
         DIFF-LEDGER.md.\n  {}\n\nThe ledger is asserted by `cargo test`, not merely read \
         (D-12). Explain each difference in the table — a field name and a reason such as a \
         different but legal encoding choice. \"differs at offset N\" is not an explanation.",
        problems.join("\n  ")
    ))
}

/// The path to the committed ledger.
fn ledger_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("DIFF-LEDGER.md")
}

// ---------------------------------------------------------------------------
// Describing our configuration in `iamf-tools`' own dialect
// ---------------------------------------------------------------------------

/// The proto name of a profile, from its wire value.
fn proto_profile(wire: u8) -> Result<&'static str, String> {
    match wire {
        0 => Ok("PROFILE_VERSION_SIMPLE"),
        1 => Ok("PROFILE_VERSION_BASE"),
        2 => Ok("PROFILE_VERSION_BASE_ENHANCED"),
        other => Err(format!("no proto name for profile {other}")),
    }
}

/// The proto name of a loudspeaker layout, and its channel labels in the
/// decoder's documented output order.
fn proto_layout(
    layout: LoudspeakerLayout,
) -> Result<(&'static str, &'static [&'static str]), String> {
    match layout {
        LoudspeakerLayout::Stereo => Ok((
            "LOUDSPEAKER_LAYOUT_STEREO",
            &["CHANNEL_LABEL_L_2", "CHANNEL_LABEL_R_2"],
        )),
        LoudspeakerLayout::Ch5_1 => Ok((
            "LOUDSPEAKER_LAYOUT_5_1_CH",
            &[
                "CHANNEL_LABEL_L_5",
                "CHANNEL_LABEL_R_5",
                "CHANNEL_LABEL_CENTRE",
                "CHANNEL_LABEL_LFE",
                "CHANNEL_LABEL_LS_5",
                "CHANNEL_LABEL_RS_5",
            ],
        )),
        other => Err(format!("no proto layout modelled for {other:?}")),
    }
}

/// The proto name of a sound system.
fn proto_sound_system(system: SoundSystem) -> Result<&'static str, String> {
    match system {
        SoundSystem::A0_2_0 => Ok("SOUND_SYSTEM_A_0_2_0"),
        SoundSystem::B0_5_0 => Ok("SOUND_SYSTEM_B_0_5_0"),
        other => Err(format!("no proto name for {other:?}")),
    }
}

/// Describe a one-element LPCM fixture in `iamf-tools`' `UserMetadata` dialect.
///
/// **Generated from the same `DescriptorSet` our encoder wrote, on purpose.** A
/// committed textproto would be a second description of the configuration and
/// the two would drift; then CONF-07 would be comparing two files built from
/// different configurations and reporting the difference as an encoder defect.
/// Generating it means the only thing that can differ is the *serialisation*,
/// which is exactly what the clause is about.
///
/// `validate_user_loudness: true` makes the reference use the loudness numbers
/// the fixture supplies rather than measuring its own, so a loudness difference
/// in the diff would be a real encoding difference and not a measurement one.
fn textproto_for(fixture: &Fixture, wav_filename: &str) -> Result<String, String> {
    let descriptors = &fixture.descriptors;
    let spec = fixture.spec()?;
    let element = descriptors
        .audio_elements
        .first()
        .ok_or_else(|| "the fixture has no Audio Element".to_owned())?;
    let config = descriptors
        .codec_configs
        .first()
        .ok_or_else(|| "the fixture has no Codec Config".to_owned())?;
    let presentation = descriptors
        .mix_presentations
        .first()
        .ok_or_else(|| "the fixture has no Mix Presentation".to_owned())?;
    let sub_mix = presentation
        .sub_mixes
        .first()
        .ok_or_else(|| "the fixture's Mix Presentation has no sub-mix".to_owned())?;
    let sub_element = sub_mix
        .elements
        .first()
        .ok_or_else(|| "the sub-mix has no element".to_owned())?;
    let plan =
        iamf::packing::SubstreamPlan::for_layout(spec.layout).map_err(|e| format!("{e:?}"))?;
    let (proto_layout_name, labels) = proto_layout(spec.layout)?;
    let trim = assert_trim_is_forced(fixture.single_pcm(), spec)?;

    let endianness = match spec.codec {
        FixtureCodec::Lpcm { big_endian: true } => "LPCM_BIG_ENDIAN",
        FixtureCodec::Lpcm { big_endian: false } => "LPCM_LITTLE_ENDIAN",
        other => {
            return Err(format!(
                "CONF-07's iamf-tools textproto companion supports LPCM, got {other:?}"
            ));
        }
    };
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();

    let mut out = String::new();
    out.push_str(
        "# GENERATED by tests/conformance.rs from the same DescriptorSet this crate encodes.\n\
         # Do not commit a hand-written copy: two descriptions of one configuration drift, and\n\
         # then CONF-07 compares two files built from different configurations and reports the\n\
         # difference as an encoder defect.\n\
         # proto-file: iamf/cli/proto/user_metadata.proto\n\
         # proto-message: UserMetadata\n\n",
    );
    out.push_str(&format!(
        "test_vector_metadata {{\n  \
           human_readable_description: \"iamf-rs CONF-07 companion\"\n  \
           file_name_prefix: \"{}\"\n  \
           is_valid: true\n  \
           is_valid_to_decode: true\n  \
           validate_user_loudness: true\n  \
           mp4_fixed_timestamp: \"2023-04-19 00:00:00\"\n  \
           base_test: \"None\"\n\
         }}\n\n",
        fixture.name
    ));
    out.push_str(
        "encoder_control_metadata {\n  add_build_information_tag: false\n  \
         output_rendered_file_format: OUTPUT_FORMAT_WAV_BIT_DEPTH_AUTOMATIC\n}\n\n",
    );
    out.push_str(&format!(
        "ia_sequence_header_metadata {{\n  primary_profile: {}\n  additional_profile: {}\n}}\n\n",
        proto_profile(descriptors.sequence_header.primary_profile)?,
        proto_profile(descriptors.sequence_header.additional_profile)?
    ));
    out.push_str(&format!(
        "codec_config_metadata {{\n  codec_config_id: {}\n  codec_config {{\n    \
           codec_id: CODEC_ID_LPCM\n    num_samples_per_frame: {}\n    audio_roll_distance: {}\n    \
           decoder_config_lpcm {{\n      sample_format_flags: {endianness}\n      \
           sample_size: {}\n      sample_rate: {}\n    }}\n  }}\n}}\n\n",
        config.codec_config_id,
        config.num_samples_per_frame,
        config.audio_roll_distance,
        spec.sample_size,
        spec.sample_rate
    ));

    let substream_ids: Vec<String> = element
        .audio_substream_ids
        .iter()
        .map(u32::to_string)
        .collect();
    out.push_str(&format!(
        "audio_element_metadata {{\n  audio_element_id: {}\n  \
           audio_element_type: AUDIO_ELEMENT_CHANNEL_BASED\n  reserved: 0\n  \
           codec_config_id: {}\n  audio_substream_ids: [{}]\n  \
           scalable_channel_layout_config {{\n    reserved: 0\n    \
           channel_audio_layer_configs: [\n      {{\n        \
             loudspeaker_layout: {proto_layout_name}\n        \
             output_gain_is_present_flag: 0\n        recon_gain_is_present_flag: 0\n        \
             reserved_a: 0\n        substream_count: {}\n        \
             coupled_substream_count: {}\n      }}\n    ]\n  }}\n}}\n\n",
        element.audio_element_id,
        element.codec_config_id,
        substream_ids.join(", "),
        plan.substream_count(),
        plan.coupled_substream_count()
    ));

    let mut layouts = String::new();
    for layout in &sub_mix.layouts {
        let Layout::SoundSystem(system) = layout.layout else {
            return Err("only Sound System layouts are described here".to_owned());
        };
        layouts.push_str(&format!(
            "    layouts {{\n      loudness_layout {{\n        \
               layout_type: LAYOUT_TYPE_LOUDSPEAKERS_SS_CONVENTION\n        \
               ss_layout {{\n          sound_system: {}\n          reserved: 0\n        }}\n      \
               }}\n      loudness {{\n        info_type_bit_masks: []\n        \
               integrated_loudness: {}\n        digital_peak: {}\n      }}\n    }}\n",
            proto_sound_system(system)?,
            layout.loudness.integrated,
            layout.loudness.digital_peak
        ));
    }

    let mix_gain = |gain: &MixGainParamDefinition| {
        format!(
            "param_definition {{\n          parameter_id: {}\n          parameter_rate: {}\n          \
             param_definition_mode: 1\n          reserved: 0\n        }}\n        \
             default_mix_gain: {}",
            gain.definition.parameter_id, gain.definition.parameter_rate, gain.default_mix_gain
        )
    };

    out.push_str(&format!(
        "mix_presentation_metadata {{\n  mix_presentation_id: {}\n  \
           annotations_language: [{}]\n  localized_presentation_annotations: [{}]\n  \
           sub_mixes {{\n    audio_elements {{\n      audio_element_id: {}\n      \
             localized_element_annotations: [{}]\n      rendering_config {{\n        \
             headphones_rendering_mode: HEADPHONES_RENDERING_MODE_STEREO\n      }}\n      \
             element_mix_gain {{\n        {}\n      }}\n    }}\n    \
           output_mix_gain {{\n        {}\n    }}\n{layouts}  }}\n}}\n\n",
        presentation.mix_presentation_id,
        presentation
            .annotations_language
            .iter()
            .map(|value| format!("\"{}\"", text(value)))
            .collect::<Vec<_>>()
            .join(", "),
        presentation
            .localized_presentation_annotations
            .iter()
            .map(|value| format!("\"{}\"", text(value)))
            .collect::<Vec<_>>()
            .join(", "),
        sub_element.audio_element_id,
        sub_element
            .localized_element_annotations
            .iter()
            .map(|value| format!("\"{}\"", text(value)))
            .collect::<Vec<_>>()
            .join(", "),
        mix_gain(&sub_element.element_mix_gain),
        mix_gain(&sub_mix.output_mix_gain)
    ));

    let channel_metadatas: Vec<String> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| format!("    {{ channel_id: {index} channel_label: {label} }}"))
        .collect();
    out.push_str(&format!(
        "audio_frame_metadata {{\n  wav_filename: \"{wav_filename}\"\n  \
           samples_to_trim_at_end: {trim}\n  samples_to_trim_at_start: 0\n  \
           audio_element_id: {}\n  channel_metadatas: [\n{}\n  ]\n}}\n\n",
        element.audio_element_id,
        channel_metadatas.join(",\n")
    ));
    out.push_str("temporal_delimiter_metadata {\n  enable_temporal_delimiters: false\n}\n");
    Ok(out)
}

/// Produce the companion `.iamf` with `encoder_main`, from the same
/// configuration and the same PCM.
fn run_encoder_main(fixture: &Fixture, dir: &Path) -> Result<Vec<u8>, String> {
    let spec = fixture.spec()?;
    let wav_name = format!("{}.wav", fixture.name);
    write_wav(&dir.join(&wav_name), fixture.single_pcm(), spec)?;
    let textproto = textproto_for(fixture, &wav_name)?;
    let proto_name = format!("{}.textproto", fixture.name);
    std::fs::write(dir.join(&proto_name), &textproto)
        .map_err(|e| format!("CONF-07: cannot write the textproto: {e}"))?;

    let output = dir.join(format!("{}.iamf", fixture.name));
    // T-01-48, made structural: the companion file must be produced by
    // `encoder_main` and by nothing else. If it already exists, some caller has
    // put our own output where theirs belongs and CONF-07 would compare a file
    // with itself — a clause reporting perfect agreement while measuring
    // nothing, which is worse than one that fails.
    if output.exists() {
        return Err(format!(
            "CONF-07: {} already exists before encoder_main ran. The companion file must come              from the reference encoder and nowhere else.",
            output.display()
        ));
    }
    let mount = format!("{}:/work", dir.display());
    let args: Vec<String> = vec![
        "run".to_owned(),
        "--rm".to_owned(),
        "-v".to_owned(),
        mount,
        "-w".to_owned(),
        "/src".to_owned(),
        iamf_tools_image(),
        "bazel-bin/iamf/cli/encoder_main".to_owned(),
        format!("--user_metadata_filename=/work/{proto_name}"),
        "--input_wav_directory=/work".to_owned(),
        "--output_iamf_directory=/work".to_owned(),
    ];
    let run = run_tool("docker", &args, &output);

    let bytes = std::fs::read(&output).map_err(|e| {
        format!(
            "CONF-07: encoder_main produced no {} ({e}). Its exit code was {:?} and is NOT the \
             signal.\ncommand: {}\nstdout: {}\nstderr: {}",
            output.display(),
            run.exit_code,
            run.command,
            run.stdout,
            run.stderr
        )
    })?;
    if bytes.is_empty() {
        return Err(format!(
            "CONF-07: encoder_main produced an empty file.\ncommand: {}\nstderr: {}",
            run.command, run.stderr
        ));
    }
    // A second, independent sign that `encoder_main` itself ran: it also writes
    // one rendered WAV per sub-mix layout. Checking for it means a stale or
    // planted `.iamf` cannot pass as a fresh reference encode.
    let rendered = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .contains("_rendered_id_")
                })
                .count()
        })
        .unwrap_or(0);
    if rendered == 0 {
        return Err(format!(
            "CONF-07: encoder_main wrote an .iamf but none of its per-layout rendered WAVs, so              it probably did not run at all.\ncommand: {}\nstderr: {}",
            run.command, run.stderr
        ));
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// CONF-07's tests
// ---------------------------------------------------------------------------

/// **CONF-07.** The byte diff against `iamf-tools`' own output, asserted against
/// the committed ledger exactly and in both directions.
#[test]
fn conf_07_byte_diff_matches_the_committed_ledger() {
    let ledger_text = std::fs::read_to_string(ledger_path())
        .unwrap_or_else(|e| panic!("DIFF-LEDGER.md must exist and be readable: {e}"));
    let ledger = parse_ledger(&ledger_text).unwrap_or_else(|e| panic!("{e}"));

    if !iamf_tools_available() {
        skip_no_container("conf_07_byte_diff_matches_the_committed_ledger");
        println!(
            "  (the committed ledger still parsed cleanly: {} row(s))",
            ledger.len()
        );
        return;
    }

    let fixture = fixture::sample_identity();
    let dir = scratch_dir("conf07");
    let ours = fixture.encode().expect("our encoder produces the fixture");
    let theirs = run_encoder_main(&fixture, &dir).unwrap_or_else(|e| panic!("{e}"));

    let diff = byte_diff(&ours, &theirs);
    println!(
        "[{}] CONF-07: ours {} bytes, theirs {} bytes, {} differing offset(s), ledger {} row(s)",
        fixture.name,
        ours.len(),
        theirs.len(),
        diff.len(),
        ledger.len()
    );
    for row in diff.iter().take(24) {
        println!(
            "  0x{:04x}  ours 0x{:02x}  theirs 0x{:02x}",
            row.offset, row.ours, row.theirs
        );
    }
    if diff.len() > 24 {
        println!("  … and {} more", diff.len().saturating_sub(24));
    }

    compare_to_ledger(&diff, &ledger).unwrap_or_else(|e| panic!("{e}"));
}

/// **D-12, direction one.** A difference the ledger does not carry fails.
///
/// A unit test of the comparison, so it runs offline on all four targets: what
/// makes the ledger executable is this function, and a container-gated proof of
/// it would only be exercised on Linux.
#[test]
fn the_ledger_comparison_fails_on_a_new_difference() {
    let ledger = vec![LedgerRow {
        offset: 10,
        field: "known".to_owned(),
        ours: 0x01,
        theirs: 0x02,
        why: "a recorded, explained difference".to_owned(),
    }];
    let diff = vec![
        DiffRow {
            offset: 10,
            ours: 0x01,
            theirs: 0x02,
        },
        DiffRow {
            offset: 99,
            ours: 0xAA,
            theirs: 0xBB,
        },
    ];
    let verdict = compare_to_ledger(&diff, &ledger).expect_err("a new difference must fail");
    assert!(verdict.contains("NEW difference at offset 99"), "{verdict}");
}

/// **D-12, direction two — the half that matters most.** A difference that has
/// *disappeared* fails too, forcing the ledger to be updated.
///
/// Without it the document silently becomes a description of a file that no
/// longer exists, and Phase 1's success criterion 3 passes on a stale writeup.
#[test]
fn the_ledger_comparison_fails_on_a_vanished_difference() {
    let ledger = vec![
        LedgerRow {
            offset: 10,
            field: "still there".to_owned(),
            ours: 0x01,
            theirs: 0x02,
            why: "a recorded, explained difference".to_owned(),
        },
        LedgerRow {
            offset: 20,
            field: "gone".to_owned(),
            ours: 0x03,
            theirs: 0x04,
            why: "explained when it existed".to_owned(),
        },
    ];
    let diff = vec![DiffRow {
        offset: 10,
        ours: 0x01,
        theirs: 0x02,
    }];
    let verdict = compare_to_ledger(&diff, &ledger).expect_err("a vanished difference must fail");
    assert!(
        verdict.contains("VANISHED difference at offset 20"),
        "{verdict}"
    );
}

/// An **empty** ledger is the passing state when the diff is empty — and an
/// empty diff against a non-empty ledger is not.
#[test]
fn an_empty_ledger_passes_only_against_an_empty_diff() {
    assert!(compare_to_ledger(&[], &[]).is_ok());
    let ledger = vec![LedgerRow {
        offset: 7,
        field: "x".to_owned(),
        ours: 1,
        theirs: 2,
        why: "y".to_owned(),
    }];
    assert!(
        compare_to_ledger(&[], &ledger).is_err(),
        "an empty diff against a non-empty ledger must fail — that is what makes committing an \
         empty table meaningfully different from committing no table"
    );
}

/// Reordering the ledger file changes nothing: both sides are compared as sets
/// keyed by offset.
#[test]
fn the_ledger_comparison_is_order_insensitive() {
    let rows = |mut order: Vec<usize>| -> Vec<LedgerRow> {
        order
            .drain(..)
            .map(|offset| LedgerRow {
                offset,
                field: format!("f{offset}"),
                ours: 1,
                theirs: 2,
                why: "explained".to_owned(),
            })
            .collect()
    };
    let diff = vec![
        DiffRow {
            offset: 5,
            ours: 1,
            theirs: 2,
        },
        DiffRow {
            offset: 9,
            ours: 1,
            theirs: 2,
        },
    ];
    assert!(compare_to_ledger(&diff, &rows(vec![5, 9])).is_ok());
    assert!(compare_to_ledger(&diff, &rows(vec![9, 5])).is_ok());
}

/// A ledger row recording `ours == theirs` is a ledger error, not a passing
/// state: it cannot be describing a difference.
#[test]
fn a_ledger_row_recording_no_difference_is_rejected() {
    let text = "| offset | field | ours | theirs | why |\n\
                |---|---|---|---|---|\n\
                | 0x10 | some_field | 0x41 | 0x41 | claims to explain something |\n";
    let verdict = parse_ledger(text).expect_err("ours == theirs must be rejected");
    assert!(verdict.contains("records ours == theirs"), "{verdict}");
}

/// A ledger row with no explanation is rejected. "Mostly the same" is not a
/// passing result and neither is a blank cell.
#[test]
fn a_ledger_row_without_an_explanation_is_rejected() {
    let text = "| offset | field | ours | theirs | why |\n\
                |---|---|---|---|---|\n\
                | 0x10 | some_field | 0x41 | 0x42 |  |\n";
    let verdict = parse_ledger(text).expect_err("an empty `why` must be rejected");
    assert!(verdict.contains("empty `why`"), "{verdict}");
}

/// The committed ledger parses, and every row explains itself.
#[test]
fn the_committed_ledger_parses_and_every_row_explains_itself() {
    let text = std::fs::read_to_string(ledger_path()).expect("DIFF-LEDGER.md exists");
    assert!(
        text.contains("exact")
            && (text.contains("both directions") || text.contains("in both directions")),
        "DIFF-LEDGER.md must state the exact-match-in-both-directions rule in its header"
    );
    let ledger = parse_ledger(&text).unwrap_or_else(|e| panic!("{e}"));
    for row in &ledger {
        assert!(
            row.why.len() > 20,
            "the ledger row at offset {} explains itself with {:?}, which is not an explanation",
            row.offset,
            row.why
        );
        assert!(
            !row.field.is_empty(),
            "the ledger row at offset {} names no field",
            row.offset
        );
    }
}

/// A length difference is reported as a row rather than swallowed, so "their
/// file is longer" can never look like "no differences".
#[test]
fn byte_diff_reports_a_length_difference() {
    assert!(byte_diff(&[1, 2, 3], &[1, 2, 3]).is_empty());
    let longer = byte_diff(&[1, 2, 3], &[1, 2, 3, 4]);
    assert_eq!(longer.len(), 1);
    assert_eq!(longer.first().map(|row| row.offset), Some(3));
    let shorter = byte_diff(&[1, 2, 3, 4], &[1, 2, 3]);
    assert_eq!(shorter.len(), 1);
    assert_eq!(shorter.first().map(|row| row.offset), Some(3));
}
