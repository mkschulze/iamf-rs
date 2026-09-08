//! The Codec Config OBU (type 0) — DESC-02 and DESC-03.
//!
//! # `sample_format_flags == 0` means BIG-endian (DESC-03)
//!
//! The sense is the opposite of a WAV-shaped assumption, which is why
//! [`SampleFormatFlags`] is a named enum and never a bare integer or a `bool`
//! called `little_endian`. It is confirmed three ways:
//!
//! - `iamf-tools@v2.1.0` names the values itself:
//!   `kLpcmBigEndian = 0x00, kLpcmLittleEndian = 0x01`
//!   (`iamf/obu/decoder_config/lpcm_decoder_config.h`).
//! - `libiamf@v1.1.0`'s PCM reader selects a **big**-endian reader when the
//!   flag is falsy: `if (!ths->flags) ctx->func = reads16be;`
//!   (`code/src/iamf_dec/pcm/IAMF_pcm_decoder.c`).
//! - `libiamf@main` does `param->big_endian = !ior_8(r);`.
//!
//! **NOTE:** `libiamf@main`'s comment one line above that assignment says
//! `0x01 - big endian, 0x00 - little endian`. It is **inverted**; the code is
//! authoritative. Following the comment produces samples with their bytes
//! swapped — white noise that reads as a DSP bug rather than a serialiser bug.
//!
//! # There is no length field for `decoder_config`
//!
//! `decoder_config` is the **entire remainder of the OBU payload**. Both
//! `libiamf` revisions compute `decoder_config_size = payload_size −
//! bytes_consumed_so_far`. Get `obu_size` wrong and the LPCM config silently
//! absorbs or loses bytes — there is no second copy of the length to disagree
//! with it, so nothing detects the slip. That is the failure mode this field
//! ordering hides, and it is why `obu_size` is measured (Pattern 1) rather than
//! reserved and backfilled.

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Finding, Location, Result};

/// `ipcm` — LPCM.
pub const CODEC_ID_LPCM: [u8; 4] = *b"ipcm";
/// `Opus` — Opus. Modelled as an opaque decoder config until Phase 3.
pub const CODEC_ID_OPUS: [u8; 4] = *b"Opus";
/// `fLaC` — FLAC. Modelled as an opaque decoder config until Phase 3.
pub const CODEC_ID_FLAC: [u8; 4] = *b"fLaC";
/// `mp4a` — AAC-LC. Modelled as an opaque decoder config.
pub const CODEC_ID_AAC: [u8; 4] = *b"mp4a";

/// `kMaxPracticalFrameSize` — `iamf-tools` rejects above this.
// ref: iamf-tools@v2.1.0 iamf/obu/codec_config.h kMaxPracticalFrameSize
pub const MAX_SAMPLES_PER_FRAME: u32 = 96_000;

/// The sample rates `iamf-tools`' `ValidateSampleRate` admits.
const VALID_SAMPLE_RATES: [u32; 5] = [16_000, 32_000, 44_100, 48_000, 96_000];

/// `sample_format_flags` — an 8-bit field with exactly two defined values.
///
/// **`BigEndian` is zero.** See the module comment for the three-way
/// confirmation and for the inverted reference comment that makes this worth
/// asserting by name.
// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.h LpcmFormatFlagsBitmask
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormatFlags {
    /// `kLpcmBigEndian = 0x00`.
    BigEndian,
    /// `kLpcmLittleEndian = 0x01`.
    LittleEndian,
    /// `kLpcmBeginReserved = 0x02` … `kLpcmEndReserved = 0xff`.
    Reserved(u8),
}

impl SampleFormatFlags {
    /// The byte this flag occupies on the wire.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::BigEndian => 0x00,
            Self::LittleEndian => 0x01,
            Self::Reserved(raw) => raw,
        }
    }

    /// The flag a wire byte carries. Reserved values are preserved, not
    /// rejected: `libiamf` does not check them either, and `validate()` is
    /// where they are reported.
    #[must_use]
    pub const fn from_value(value: u8) -> Self {
        match value {
            0x00 => Self::BigEndian,
            0x01 => Self::LittleEndian,
            other => Self::Reserved(other),
        }
    }
}

/// The LPCM `decoder_config` — six bytes, and nothing that needs a codec crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LpcmDecoderConfig {
    /// `sample_format_flags`. **Zero is big-endian.**
    pub sample_format_flags: SampleFormatFlags,
    /// `sample_size` in bits — 16, 24 or 32.
    pub sample_size: u8,
    /// `sample_rate` in Hz, an unsigned 32-bit **big-endian** field.
    pub sample_rate: u32,
}

/// The `decoder_config` blob, which is the whole remainder of the payload.
///
/// `Raw` exists so a Codec Config for a codec Phase 1 does not model still
/// round-trips byte-identically. Phase 3 adds `Flac` and `Opus` beside `Lpcm`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderConfig {
    /// `codec_id == "ipcm"`.
    Lpcm(LpcmDecoderConfig),
    /// Any other `codec_id`, preserved verbatim.
    Raw {
        /// The `codec_id` these bytes belong to.
        codec_id: [u8; 4],
        /// The bytes, exactly as they were on the wire.
        bytes: Vec<u8>,
    },
}

/// The Codec Config OBU payload.
///
/// `audio_roll_distance` is **stored**, not derived on read (D-06): the encoder
/// path derives 0 for LPCM, and a foreign file carrying 2 keeps its 2 and gets
/// a `validate()` Finding. Normalising it would make that file un-reproducible
/// and hide the defect that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecConfig {
    /// The id Audio Elements reference through `by_id()`.
    pub codec_config_id: u32,
    /// A four-character code: `ipcm`, `Opus`, `fLaC` or `mp4a`.
    pub codec_id: [u8; 4],
    /// Samples per frame per channel.
    pub num_samples_per_frame: u32,
    /// A **signed 16-bit big-endian** field. `0` for LPCM.
    pub audio_roll_distance: i16,
    /// The entire remainder of the payload — there is no length field.
    pub decoder_config: DecoderConfig,
    /// Payload bytes past what `decoder_config` claimed.
    pub trailing: Vec<u8>,
}

impl CodecConfig {
    /// An LPCM Codec Config through the **encoder path**, with
    /// `audio_roll_distance` derived rather than supplied (DESC-02).
    #[must_use]
    pub fn lpcm(
        codec_config_id: u32,
        num_samples_per_frame: u32,
        decoder_config: LpcmDecoderConfig,
    ) -> Self {
        let decoder_config = DecoderConfig::Lpcm(decoder_config);
        Self {
            codec_config_id,
            codec_id: CODEC_ID_LPCM,
            num_samples_per_frame,
            audio_roll_distance: required_audio_roll_distance(&decoder_config),
            decoder_config,
            trailing: Vec::new(),
        }
    }

    /// The `LpcmDecoderConfig`, when this is one.
    #[must_use]
    pub const fn lpcm_config(&self) -> Option<&LpcmDecoderConfig> {
        match &self.decoder_config {
            DecoderConfig::Lpcm(cfg) => Some(cfg),
            DecoderConfig::Raw { .. } => None,
        }
    }

    /// The six rules `iamf-tools@v2.1.0` rejects on and **neither** `libiamf`
    /// revision checks, reported as Findings rather than parse failures.
    ///
    /// Reporting them matters even though the decoder accepts them: an
    /// out-of-range `sample_size` falls through `libiamf`'s PCM init to a
    /// 16-bit little-endian reader with **no error**, producing
    /// plausible-sounding garbage. The Finding is the only signal there is.
    // ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.cc LpcmDecoderConfig::Validate
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let field = |name: &'static str, message: String| Finding {
            at: Location::Field(name),
            message,
        };

        if !matches!(
            self.codec_id,
            CODEC_ID_LPCM | CODEC_ID_OPUS | CODEC_ID_FLAC | CODEC_ID_AAC
        ) {
            findings.push(field(
                "codec_id",
                format!(
                    "codec_id {:?} is not one of ipcm/Opus/fLaC/mp4a; both libiamf revisions \
                     reject an unknown codec_id outright",
                    String::from_utf8_lossy(&self.codec_id)
                ),
            ));
        }
        if self.num_samples_per_frame == 0 || self.num_samples_per_frame > MAX_SAMPLES_PER_FRAME {
            findings.push(field(
                "num_samples_per_frame",
                format!(
                    "num_samples_per_frame is {}, outside 1..={MAX_SAMPLES_PER_FRAME}",
                    self.num_samples_per_frame
                ),
            ));
        }
        if let Some(lpcm) = self.lpcm_config() {
            if matches!(lpcm.sample_format_flags, SampleFormatFlags::Reserved(_)) {
                findings.push(field(
                    "sample_format_flags",
                    format!(
                        "sample_format_flags is {}, outside {{0 (big-endian), 1 (little-endian)}}",
                        lpcm.sample_format_flags.value()
                    ),
                ));
            }
            if !matches!(lpcm.sample_size, 16 | 24 | 32) {
                findings.push(field(
                    "sample_size",
                    format!(
                        "sample_size is {}, outside {{16, 24, 32}}; libiamf@v1.1.0 does not check \
                         this and falls through to a 16-bit little-endian reader with no error",
                        lpcm.sample_size
                    ),
                ));
            }
            if !VALID_SAMPLE_RATES.contains(&lpcm.sample_rate) {
                findings.push(field(
                    "sample_rate",
                    format!(
                        "sample_rate is {}, outside {VALID_SAMPLE_RATES:?}",
                        lpcm.sample_rate
                    ),
                ));
            }
        }
        let required = required_audio_roll_distance(&self.decoder_config);
        if self.audio_roll_distance != required {
            findings.push(field(
                "audio_roll_distance",
                format!(
                    "audio_roll_distance is {}, expected {required} for this codec",
                    self.audio_roll_distance
                ),
            ));
        }
        findings
    }
}

/// `GetRequiredAudioRollDistance()` — `0` for LPCM.
///
/// This is the **encoder-path derivation** (DESC-02). The reader never calls
/// it to replace a wire value; `validate()` calls it to name a mismatch (D-06).
// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.h LpcmDecoderConfig::GetRequiredAudioRollDistance
#[must_use]
pub const fn required_audio_roll_distance(decoder_config: &DecoderConfig) -> i16 {
    match decoder_config {
        DecoderConfig::Lpcm(_) => 0,
        // Phase 1 models no other codec, and a roll distance it cannot derive
        // is one it must not claim a value for. Reported as "expected 0" would
        // be a lie for Opus (-32 at 48 kHz); Phase 3 replaces this arm.
        DecoderConfig::Raw { .. } => 0,
    }
}

// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c iamf_codec_conf_new
// NOTE: the decoder_config length is the payload REMAINDER — there is no
// length field. The reference computes `decoder_config_size = payload_size -
// bytes_consumed_so_far` and reads that many bytes.
/// Read a Codec Config payload from a **bounded** payload reader.
///
/// The bound is what makes the "remainder" meaningful: the sub-reader supplied
/// by [`crate::obu::read_obu_with`] ends exactly at `obu_size`, so
/// `bytes_remaining()` is the decoder-config length the reference computes.
pub fn read_codec_config(r: &mut BitCursor<'_>) -> Result<CodecConfig> {
    let codec_config_id = r.read_uleb128()?;
    let mut codec_id = [0_u8; 4];
    for slot in &mut codec_id {
        *slot = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    }
    let num_samples_per_frame = r.read_uleb128()?;
    let audio_roll_distance = i16::try_from(r.read_signed(16)?).unwrap_or(0);

    let decoder_config = if codec_id == CODEC_ID_LPCM && r.bytes_remaining() >= 6 {
        DecoderConfig::Lpcm(read_lpcm_decoder_config(r)?)
    } else {
        // Everything left, for a codec this phase does not model — or for an
        // `ipcm` config too short to be one, which stays reproducible rather
        // than becoming a parse failure the reference would not have raised.
        let remaining = r.bytes_remaining();
        DecoderConfig::Raw {
            codec_id,
            bytes: r.read_uint8_span(remaining)?.to_vec(),
        }
    };

    // The payload-level drain (D-05 precedence): whatever the decoder config
    // did not claim is captured here, so the OBU-level `trailing` stays empty.
    let remaining = r.bytes_remaining();
    let trailing = r.read_uint8_span(remaining)?.to_vec();

    Ok(CodecConfig {
        codec_config_id,
        codec_id,
        num_samples_per_frame,
        audio_roll_distance,
        decoder_config,
        trailing,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/codec_config.cc CodecConfigObu::ValidateAndWriteObu
/// Write a Codec Config payload, `trailing` last.
///
/// **Faithful, not validating** (D-07): whatever the model holds is what goes
/// on the wire, so `serialize(parse(bytes)) == bytes` survives for foreign
/// files. `validate()` is the separate, explicit pass.
pub fn write_codec_config(w: &mut BitWriter, v: &CodecConfig) -> Result<()> {
    w.write_uleb128_minimal(v.codec_config_id)?;
    w.write_bytes(&v.codec_id)?;
    w.write_uleb128_minimal(v.num_samples_per_frame)?;
    w.write_signed(i64::from(v.audio_roll_distance), 16)?;
    match &v.decoder_config {
        DecoderConfig::Lpcm(cfg) => write_lpcm_decoder_config(w, cfg)?,
        DecoderConfig::Raw { bytes, .. } => w.write_bytes(bytes)?,
    }
    w.write_bytes(&v.trailing)
}

// ref: libiamf@v1.1.0 code/src/iamf_dec/pcm/IAMF_pcm_decoder.c pcm_init
// NOTE: `if (!ths->flags) ctx->func = reads16be;` — a FALSY flag selects the
// BIG-endian reader. libiamf@main's comment above the equivalent line claims
// the opposite; the code is authoritative.
/// Read the six LPCM `decoder_config` bytes.
fn read_lpcm_decoder_config(r: &mut BitCursor<'_>) -> Result<LpcmDecoderConfig> {
    let sample_format_flags =
        SampleFormatFlags::from_value(u8::try_from(r.read_unsigned(8)?).unwrap_or(0));
    let sample_size = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    let sample_rate = u32::try_from(r.read_unsigned(32)?).unwrap_or(0);
    Ok(LpcmDecoderConfig {
        sample_format_flags,
        sample_size,
        sample_rate,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.cc LpcmDecoderConfig::ValidateAndWrite
/// Write the six LPCM `decoder_config` bytes: flags, size, then the rate as an
/// unsigned 32-bit **big-endian** field.
fn write_lpcm_decoder_config(w: &mut BitWriter, v: &LpcmDecoderConfig) -> Result<()> {
    w.write_unsigned(u64::from(v.sample_format_flags.value()), 8)?;
    w.write_unsigned(u64::from(v.sample_size), 8)?;
    w.write_unsigned(u64::from(v.sample_rate), 32)
}
