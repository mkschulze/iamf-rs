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
        Vec::new() // STUB(GREEN)
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
    let _ = r; // STUB(GREEN)
    Ok(CodecConfig::lpcm(0, 0, LpcmDecoderConfig { sample_format_flags: SampleFormatFlags::BigEndian, sample_size: 0, sample_rate: 0 }))
}

// ref: iamf-tools@v2.1.0 iamf/obu/codec_config.cc CodecConfigObu::ValidateAndWriteObu
/// Write a Codec Config payload, `trailing` last.
///
/// **Faithful, not validating** (D-07): whatever the model holds is what goes
/// on the wire, so `serialize(parse(bytes)) == bytes` survives for foreign
/// files. `validate()` is the separate, explicit pass.
pub fn write_codec_config(w: &mut BitWriter, v: &CodecConfig) -> Result<()> {
    let _ = (w, v); // STUB(GREEN)
    Ok(())
}

// ref: libiamf@v1.1.0 code/src/iamf_dec/pcm/IAMF_pcm_decoder.c pcm_init
// NOTE: `if (!ths->flags) ctx->func = reads16be;` — a FALSY flag selects the
// BIG-endian reader. libiamf@main's comment above the equivalent line claims
// the opposite; the code is authoritative.
/// Read the six LPCM `decoder_config` bytes.
fn read_lpcm_decoder_config(r: &mut BitCursor<'_>) -> Result<LpcmDecoderConfig> {
    let _ = r; // STUB(GREEN)
    Ok(LpcmDecoderConfig { sample_format_flags: SampleFormatFlags::BigEndian, sample_size: 0, sample_rate: 0 })
}

// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.cc LpcmDecoderConfig::ValidateAndWrite
/// Write the six LPCM `decoder_config` bytes: flags, size, then the rate as an
/// unsigned 32-bit **big-endian** field.
fn write_lpcm_decoder_config(w: &mut BitWriter, v: &LpcmDecoderConfig) -> Result<()> {
    let _ = (w, v); // STUB(GREEN)
    Ok(())
}
