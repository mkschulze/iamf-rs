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
use crate::error::{Error, ErrorKind, Finding, Location, Result};

/// `ipcm` — LPCM.
pub const CODEC_ID_LPCM: [u8; 4] = *b"ipcm";
/// `Opus` — Opus. Modelled as an opaque decoder config until Phase 3.
pub const CODEC_ID_OPUS: [u8; 4] = *b"Opus";
/// `fLaC` — FLAC. Typed as [`FlacDecoderConfig`] when its prefix is complete.
pub const CODEC_ID_FLAC: [u8; 4] = *b"fLaC";
/// `mp4a` — AAC-LC. Modelled as an opaque decoder config.
pub const CODEC_ID_AAC: [u8; 4] = *b"mp4a";

/// `kMaxPracticalFrameSize` — `iamf-tools` rejects above this.
// ref: iamf-tools@v2.1.0 iamf/obu/codec_config.h kMaxPracticalFrameSize
pub const MAX_SAMPLES_PER_FRAME: u32 = 96_000;

/// The sample rates `iamf-tools`' `ValidateSampleRate` admits.
const VALID_SAMPLE_RATES: [u32; 5] = [16_000, 32_000, 44_100, 48_000, 96_000];

/// A FLAC metadata header followed by its fixed-size STREAMINFO payload.
const FLAC_DECODER_CONFIG_BYTES: usize = 38;
const FLAC_STREAMINFO_BYTES: u32 = 34;
const FLAC_MAX_SAMPLE_RATE: u32 = 655_350;
const FLAC_MAX_TOTAL_SAMPLES: u64 = 0x0f_ffff_ffff;

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

/// The IAMF FLAC `decoder_config`: one metadata header and STREAMINFO block.
///
/// Bit-packed fields retain their raw wire values. In particular,
/// `bits_per_sample` stores the FLAC value-minus-one representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlacDecoderConfig {
    pub last_metadata_block: bool,
    pub metadata_block_type: u8,
    pub metadata_data_block_length: u32,
    pub minimum_block_size: u16,
    pub maximum_block_size: u16,
    pub minimum_frame_size: u32,
    pub maximum_frame_size: u32,
    pub sample_rate: u32,
    pub number_of_channels: u8,
    pub bits_per_sample: u8,
    pub total_samples_in_stream: u64,
    pub md5_signature: [u8; 16],
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
    /// `codec_id == "fLaC"` with a complete 38-byte STREAMINFO prefix.
    Flac(FlacDecoderConfig),
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
/// path derives 0 for LPCM and FLAC, and a foreign file carrying 2 keeps its 2 and gets
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
            DecoderConfig::Flac(_) | DecoderConfig::Raw { .. } => None,
        }
    }

    /// Construct the canonical one-block FLAC STREAMINFO configuration.
    ///
    /// # Errors
    ///
    /// Returns a codec capability error when the sample rate, samples per
    /// frame, or actual PCM bit depth is outside FLAC's supported range.
    pub fn flac(
        codec_config_id: u32,
        num_samples_per_frame: u32,
        sample_rate: u32,
        bits_per_sample: u8,
    ) -> Result<Self> {
        if sample_rate == 0 || sample_rate > FLAC_MAX_SAMPLE_RATE {
            return Err(Error::new(
                ErrorKind::SampleRateNotSupportedByCodec,
                Location::Field("sample_rate"),
            ));
        }
        if num_samples_per_frame < 16 {
            return Err(Error::new(
                ErrorKind::SamplesPerFrameNotSupportedByCodec,
                Location::Field("num_samples_per_frame"),
            ));
        }
        let block_size = u16::try_from(num_samples_per_frame).map_err(|_| {
            Error::new(
                ErrorKind::SamplesPerFrameNotSupportedByCodec,
                Location::Field("num_samples_per_frame"),
            )
        })?;
        if !(4..=32).contains(&bits_per_sample) {
            return Err(Error::new(
                ErrorKind::BitsPerSampleNotSupportedByCodec,
                Location::Field("bits_per_sample"),
            ));
        }
        let stored_bits_per_sample = bits_per_sample.checked_sub(1).ok_or_else(|| {
            Error::new(
                ErrorKind::BitsPerSampleNotSupportedByCodec,
                Location::Field("bits_per_sample"),
            )
        })?;
        let decoder_config = DecoderConfig::Flac(FlacDecoderConfig {
            last_metadata_block: true,
            metadata_block_type: 0,
            metadata_data_block_length: FLAC_STREAMINFO_BYTES,
            minimum_block_size: block_size,
            maximum_block_size: block_size,
            minimum_frame_size: 0,
            maximum_frame_size: 0,
            sample_rate,
            number_of_channels: 1,
            bits_per_sample: stored_bits_per_sample,
            total_samples_in_stream: 0,
            md5_signature: [0; 16],
        });
        Ok(Self {
            codec_config_id,
            codec_id: CODEC_ID_FLAC,
            num_samples_per_frame,
            audio_roll_distance: required_audio_roll_distance(&decoder_config),
            decoder_config,
            trailing: Vec::new(),
        })
    }

    /// The `FlacDecoderConfig`, when this is one.
    #[must_use]
    pub const fn flac_config(&self) -> Option<&FlacDecoderConfig> {
        match &self.decoder_config {
            DecoderConfig::Flac(cfg) => Some(cfg),
            DecoderConfig::Lpcm(_) | DecoderConfig::Raw { .. } => None,
        }
    }

    /// Report every modelled LPCM and FLAC semantic contradiction as a
    /// Finding rather than rejecting or normalising parsed bytes.
    ///
    /// LPCM checks its format flag, sample size and selected sample rates. FLAC
    /// checks the one-block STREAMINFO header, block sizes, packed rate/channel/
    /// depth/sample-count fields, zero roll distance, and the encoder-side
    /// zero-frame-size and zero-MD5 recommendations. The pinned readers accept
    /// several of these contradictions, so preserving first and diagnosing
    /// separately is what keeps foreign input reproducible.
    // ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.cc LpcmDecoderConfig::Validate
    // ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/flac_decoder_config.cc FlacDecoderConfig::ReadAndValidate / ValidateEncodingRestrictions
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
        if let Some(flac) = self.flac_config() {
            if !flac.last_metadata_block {
                findings.push(field(
                    "last_metadata_block",
                    "last_metadata_block is false, expected true for the one-block FLAC config"
                        .to_owned(),
                ));
            }
            if flac.metadata_block_type != 0 {
                findings.push(field(
                    "metadata_block_type",
                    format!(
                        "metadata_block_type is {}, expected 0 (STREAMINFO)",
                        flac.metadata_block_type
                    ),
                ));
            }
            if flac.metadata_data_block_length != FLAC_STREAMINFO_BYTES {
                findings.push(field(
                    "metadata_data_block_length",
                    format!(
                        "metadata_data_block_length is {}, expected {FLAC_STREAMINFO_BYTES}",
                        flac.metadata_data_block_length
                    ),
                ));
            }
            if u32::from(flac.minimum_block_size) != self.num_samples_per_frame
                || flac.minimum_block_size < 16
            {
                findings.push(field(
                    "minimum_block_size",
                    format!(
                        "minimum_block_size is {}, expected num_samples_per_frame {} and at least 16",
                        flac.minimum_block_size, self.num_samples_per_frame
                    ),
                ));
            }
            if u32::from(flac.maximum_block_size) != self.num_samples_per_frame
                || flac.maximum_block_size < 16
            {
                findings.push(field(
                    "maximum_block_size",
                    format!(
                        "maximum_block_size is {}, expected num_samples_per_frame {} and at least 16",
                        flac.maximum_block_size, self.num_samples_per_frame
                    ),
                ));
            }
            if flac.minimum_frame_size != 0 {
                findings.push(field(
                    "minimum_frame_size",
                    format!(
                        "minimum_frame_size is {}, expected 0",
                        flac.minimum_frame_size
                    ),
                ));
            }
            if flac.maximum_frame_size != 0 {
                findings.push(field(
                    "maximum_frame_size",
                    format!(
                        "maximum_frame_size is {}, expected 0",
                        flac.maximum_frame_size
                    ),
                ));
            }
            if flac.sample_rate == 0 || flac.sample_rate > FLAC_MAX_SAMPLE_RATE {
                findings.push(field(
                    "sample_rate",
                    format!(
                        "sample_rate is {}, outside 1..={FLAC_MAX_SAMPLE_RATE}",
                        flac.sample_rate
                    ),
                ));
            }
            if flac.number_of_channels != 1 {
                findings.push(field(
                    "number_of_channels",
                    format!(
                        "number_of_channels is {}, expected 1",
                        flac.number_of_channels
                    ),
                ));
            }
            if !(3..=31).contains(&flac.bits_per_sample) {
                findings.push(field(
                    "bits_per_sample",
                    format!(
                        "bits_per_sample raw value is {}, outside 3..=31 (actual 4..=32)",
                        flac.bits_per_sample
                    ),
                ));
            }
            if flac.total_samples_in_stream > FLAC_MAX_TOTAL_SAMPLES {
                findings.push(field(
                    "total_samples_in_stream",
                    format!(
                        "total_samples_in_stream is {}, outside 0..={FLAC_MAX_TOTAL_SAMPLES}",
                        flac.total_samples_in_stream
                    ),
                ));
            }
            if flac.md5_signature != [0; 16] {
                findings.push(field(
                    "md5_signature",
                    "md5_signature is not the all-zero IAMF value".to_owned(),
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

/// `GetRequiredAudioRollDistance()` — `0` for LPCM and FLAC.
///
/// This is the **encoder-path derivation** (DESC-02). The reader never calls
/// it to replace a wire value; `validate()` calls it to name a mismatch (D-06).
// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.h LpcmDecoderConfig::GetRequiredAudioRollDistance
#[must_use]
pub const fn required_audio_roll_distance(decoder_config: &DecoderConfig) -> i16 {
    match decoder_config {
        DecoderConfig::Lpcm(_) | DecoderConfig::Flac(_) => 0,
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
    } else if codec_id == CODEC_ID_FLAC && r.bytes_remaining() >= FLAC_DECODER_CONFIG_BYTES {
        DecoderConfig::Flac(read_flac_decoder_config(r)?)
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
        DecoderConfig::Flac(cfg) => write_flac_decoder_config(w, cfg)?,
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

// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/flac_decoder_config.cc FlacDecoderConfig::ReadAndValidate / ReadStreamInfo
/// Read the fixed 38-byte FLAC metadata-header/STREAMINFO prefix without
/// validating or normalising any field.
fn read_flac_decoder_config(r: &mut BitCursor<'_>) -> Result<FlacDecoderConfig> {
    let last_metadata_block = r.read_bool()?;
    let metadata_block_type = u8::try_from(r.read_unsigned(7)?).unwrap_or(0);
    let metadata_data_block_length = u32::try_from(r.read_unsigned(24)?).unwrap_or(0);
    let minimum_block_size = u16::try_from(r.read_unsigned(16)?).unwrap_or(0);
    let maximum_block_size = u16::try_from(r.read_unsigned(16)?).unwrap_or(0);
    let minimum_frame_size = u32::try_from(r.read_unsigned(24)?).unwrap_or(0);
    let maximum_frame_size = u32::try_from(r.read_unsigned(24)?).unwrap_or(0);
    let sample_rate = u32::try_from(r.read_unsigned(20)?).unwrap_or(0);
    let number_of_channels = u8::try_from(r.read_unsigned(3)?).unwrap_or(0);
    let bits_per_sample = u8::try_from(r.read_unsigned(5)?).unwrap_or(0);
    let total_samples_in_stream = r.read_unsigned(36)?;
    let mut md5_signature = [0_u8; 16];
    for byte in &mut md5_signature {
        *byte = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    }
    Ok(FlacDecoderConfig {
        last_metadata_block,
        metadata_block_type,
        metadata_data_block_length,
        minimum_block_size,
        maximum_block_size,
        minimum_frame_size,
        maximum_frame_size,
        sample_rate,
        number_of_channels,
        bits_per_sample,
        total_samples_in_stream,
        md5_signature,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/flac_decoder_config.cc FlacDecoderConfig::ValidateAndWrite / WriteStreamInfo
/// Write the fixed FLAC metadata-header/STREAMINFO prefix exactly as held.
fn write_flac_decoder_config(w: &mut BitWriter, v: &FlacDecoderConfig) -> Result<()> {
    w.write_bool(v.last_metadata_block)?;
    w.write_unsigned(u64::from(v.metadata_block_type), 7)?;
    w.write_unsigned(u64::from(v.metadata_data_block_length), 24)?;
    w.write_unsigned(u64::from(v.minimum_block_size), 16)?;
    w.write_unsigned(u64::from(v.maximum_block_size), 16)?;
    w.write_unsigned(u64::from(v.minimum_frame_size), 24)?;
    w.write_unsigned(u64::from(v.maximum_frame_size), 24)?;
    w.write_unsigned(u64::from(v.sample_rate), 20)?;
    w.write_unsigned(u64::from(v.number_of_channels), 3)?;
    w.write_unsigned(u64::from(v.bits_per_sample), 5)?;
    w.write_unsigned(v.total_samples_in_stream, 36)?;
    w.write_bytes(&v.md5_signature)
}
