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
/// `Opus` — Opus. Typed as [`OpusDecoderConfig`] when its prefix is complete.
pub const CODEC_ID_OPUS: [u8; 4] = *b"Opus";
/// `fLaC` — FLAC. Typed as [`FlacDecoderConfig`] when its prefix is complete.
pub const CODEC_ID_FLAC: [u8; 4] = *b"fLaC";
/// `mp4a` — AAC-LC.
pub const CODEC_ID_AAC: [u8; 4] = *b"mp4a";

/// `kMaxPracticalFrameSize` — `iamf-tools` rejects above this.
// ref: iamf-tools@v2.1.0 iamf/obu/codec_config.h kMaxPracticalFrameSize
pub const MAX_SAMPLES_PER_FRAME: u32 = 96_000;

/// The sample rates `iamf-tools`' `ValidateSampleRate` admits.
const VALID_SAMPLE_RATES: [u32; 5] = [16_000, 32_000, 44_100, 48_000, 96_000];

const OPUS_DECODER_CONFIG_BYTES: usize = 11;
const OPUS_SAMPLE_RATE: u32 = 48_000;

/// The fixed MPEG-4 DecoderConfigDescriptor and AudioSpecificConfig size IAMF
/// AAC-LC carries: `04 0d` + 13 bytes, then `05 02` + two bytes.
const AAC_LC_DECODER_CONFIG_BYTES: usize = 19;
const AAC_LC_OBJECT_TYPE_INDICATION: u8 = 0x40;
const AAC_LC_STREAM_TYPE: u8 = 0x05;
const AAC_LC_AUDIO_OBJECT_TYPE: u8 = 2;
const AAC_LC_CHANNEL_CONFIGURATION: u8 = 2;
const AAC_LC_NUM_SAMPLES_PER_FRAME: u32 = 1024;
const AAC_LC_AUDIO_ROLL_DISTANCE: i16 = -1;

/// MPEG-4 `samplingFrequencyIndex` values other than the three reserved
/// values 13 through 15.
const AAC_LC_SAMPLE_RATES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000,
    7_350,
];

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

/// The eleven IAMF Opus decoder-config bytes, with big-endian multibyte fields.
/// Parsed values are retained even when they contradict IAMF constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpusDecoderConfig {
    pub version: u8,
    pub output_channel_count: u8,
    /// Encoder lookahead measured in samples at the 48 kHz output clock.
    pub pre_skip: u16,
    pub input_sample_rate: u32,
    /// The signed Q7.8 wire value; IAMF requires zero.
    pub output_gain: i16,
    pub mapping_family: u8,
}

/// The fixed fields of IAMF's AAC-LC MPEG-4 `DecoderConfigDescriptor`.
///
/// The descriptor tags and lengths are deliberately not exposed: a typed
/// value always writes the IAMF-required `04 0d ... 05 02` form. Parsed values
/// retain the semantic fields even when they violate IAMF constraints so that
/// `validate()` can diagnose them without changing their bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AacLcDecoderConfig {
    pub object_type_indication: u8,
    pub stream_type: u8,
    pub upstream: bool,
    pub buffer_size_db: u32,
    pub max_bitrate: u32,
    pub avg_bitrate: u32,
    pub audio_object_type: u8,
    pub sampling_frequency_index: u8,
    pub channel_configuration: u8,
    pub frame_length_flag: bool,
    pub depends_on_core_coder: bool,
    pub extension_flag: bool,
}

/// The `decoder_config` blob, which is the whole remainder of the payload.
///
/// `Raw` preserves unsupported codecs and structurally short known configs.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderConfig {
    /// `codec_id == "ipcm"`.
    Lpcm(LpcmDecoderConfig),
    /// `codec_id == "fLaC"` with a complete 38-byte STREAMINFO prefix.
    Flac(FlacDecoderConfig),
    /// `codec_id == "Opus"` with a complete 11-byte prefix.
    Opus(OpusDecoderConfig),
    /// `codec_id == "mp4a"` with IAMF's complete 19-byte descriptor shape.
    AacLc(AacLcDecoderConfig),
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
            DecoderConfig::Flac(_)
            | DecoderConfig::Opus(_)
            | DecoderConfig::AacLc(_)
            | DecoderConfig::Raw { .. } => None,
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
            DecoderConfig::Lpcm(_)
            | DecoderConfig::Opus(_)
            | DecoderConfig::AacLc(_)
            | DecoderConfig::Raw { .. } => None,
        }
    }

    /// Construct an IAMF Opus configuration with derived roll distance.
    ///
    /// `pre_skip` is the measured encoder lookahead. A zero value is retained
    /// and diagnosed by [`Self::validate`].
    ///
    /// # Errors
    ///
    /// Rejects rates other than 48 kHz and zero samples per frame.
    pub fn opus(
        codec_config_id: u32,
        num_samples_per_frame: u32,
        sample_rate: u32,
        pre_skip: u16,
    ) -> Result<Self> {
        if sample_rate != OPUS_SAMPLE_RATE {
            return Err(Error::new(
                ErrorKind::SampleRateNotSupportedByCodec,
                Location::Field("sample_rate"),
            ));
        }
        Ok(Self {
            codec_config_id,
            codec_id: CODEC_ID_OPUS,
            num_samples_per_frame,
            audio_roll_distance: required_opus_audio_roll_distance(num_samples_per_frame)?,
            decoder_config: DecoderConfig::Opus(OpusDecoderConfig {
                version: 1,
                output_channel_count: 2,
                pre_skip,
                input_sample_rate: sample_rate,
                output_gain: 0,
                mapping_family: 0,
            }),
            trailing: Vec::new(),
        })
    }

    /// The `OpusDecoderConfig`, when this is one.
    #[must_use]
    pub const fn opus_config(&self) -> Option<&OpusDecoderConfig> {
        match &self.decoder_config {
            DecoderConfig::Opus(cfg) => Some(cfg),
            DecoderConfig::Lpcm(_)
            | DecoderConfig::Flac(_)
            | DecoderConfig::AacLc(_)
            | DecoderConfig::Raw { .. } => None,
        }
    }

    /// Construct IAMF's canonical AAC-LC descriptor for one of MPEG-4's
    /// thirteen non-reserved sampling-frequency indices.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::SampleRateNotSupportedByCodec`] if `sample_rate`
    /// has no non-reserved MPEG-4 sampling-frequency index.
    pub fn aac_lc(codec_config_id: u32, sample_rate: u32) -> Result<Self> {
        let sampling_frequency_index =
            aac_lc_sampling_frequency_index(sample_rate).ok_or_else(|| {
                Error::new(
                    ErrorKind::SampleRateNotSupportedByCodec,
                    Location::Field("sample_rate"),
                )
            })?;
        let decoder_config = DecoderConfig::AacLc(AacLcDecoderConfig {
            object_type_indication: AAC_LC_OBJECT_TYPE_INDICATION,
            stream_type: AAC_LC_STREAM_TYPE,
            upstream: false,
            buffer_size_db: 0,
            max_bitrate: 0,
            avg_bitrate: 0,
            audio_object_type: AAC_LC_AUDIO_OBJECT_TYPE,
            sampling_frequency_index,
            channel_configuration: AAC_LC_CHANNEL_CONFIGURATION,
            frame_length_flag: false,
            depends_on_core_coder: false,
            extension_flag: false,
        });
        Ok(Self {
            codec_config_id,
            codec_id: CODEC_ID_AAC,
            num_samples_per_frame: AAC_LC_NUM_SAMPLES_PER_FRAME,
            audio_roll_distance: AAC_LC_AUDIO_ROLL_DISTANCE,
            decoder_config,
            trailing: Vec::new(),
        })
    }

    /// The `AacLcDecoderConfig`, when this is one.
    #[must_use]
    pub const fn aac_lc_config(&self) -> Option<&AacLcDecoderConfig> {
        match &self.decoder_config {
            DecoderConfig::AacLc(cfg) => Some(cfg),
            DecoderConfig::Lpcm(_)
            | DecoderConfig::Flac(_)
            | DecoderConfig::Opus(_)
            | DecoderConfig::Raw { .. } => None,
        }
    }

    /// Report every modelled LPCM, FLAC and Opus semantic contradiction as a
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
    // ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/opus_decoder_config.cc ValidatePayload / ValidateAudioRollDistance
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
        if let Some(opus) = self.opus_config() {
            if !(1..=15).contains(&opus.version) {
                findings.push(field(
                    "version",
                    format!("version is {}, outside the supported 1..=15", opus.version),
                ));
            }
            if opus.output_channel_count != 2 {
                findings.push(field(
                    "output_channel_count",
                    format!(
                        "output_channel_count is {}, expected 2",
                        opus.output_channel_count
                    ),
                ));
            }
            if opus.pre_skip == 0 {
                findings.push(field(
                    "pre_skip",
                    "pre_skip is zero, expected measured non-zero encoder lookahead".to_owned(),
                ));
            }
            if opus.input_sample_rate != OPUS_SAMPLE_RATE {
                findings.push(field(
                    "input_sample_rate",
                    format!(
                        "input_sample_rate is {}, expected {OPUS_SAMPLE_RATE}",
                        opus.input_sample_rate
                    ),
                ));
            }
            if opus.output_gain != 0 {
                findings.push(field(
                    "output_gain",
                    format!("output_gain is {}, expected 0", opus.output_gain),
                ));
            }
            if opus.mapping_family != 0 {
                findings.push(field(
                    "mapping_family",
                    format!("mapping_family is {}, expected 0", opus.mapping_family),
                ));
            }
        }
        if let Some(aac_lc) = self.aac_lc_config() {
            if aac_lc.object_type_indication != AAC_LC_OBJECT_TYPE_INDICATION {
                findings.push(field(
                    "object_type_indication",
                    format!(
                        "object_type_indication is {:#04x}, expected {AAC_LC_OBJECT_TYPE_INDICATION:#04x}",
                        aac_lc.object_type_indication
                    ),
                ));
            }
            if aac_lc.stream_type != AAC_LC_STREAM_TYPE {
                findings.push(field(
                    "stream_type",
                    format!(
                        "stream_type is {}, expected {AAC_LC_STREAM_TYPE}",
                        aac_lc.stream_type
                    ),
                ));
            }
            if aac_lc.upstream {
                findings.push(field(
                    "upstream",
                    "upstream is true, expected false".to_owned(),
                ));
            }
            if aac_lc.audio_object_type != AAC_LC_AUDIO_OBJECT_TYPE {
                findings.push(field(
                    "audio_object_type",
                    format!(
                        "audio_object_type is {}, expected {AAC_LC_AUDIO_OBJECT_TYPE}",
                        aac_lc.audio_object_type
                    ),
                ));
            }
            if aac_lc.sampling_frequency_index >= AAC_LC_SAMPLE_RATES.len() as u8 {
                findings.push(field(
                    "sampling_frequency_index",
                    format!(
                        "sampling_frequency_index is {}, outside 0..={}",
                        aac_lc.sampling_frequency_index,
                        AAC_LC_SAMPLE_RATES.len().saturating_sub(1)
                    ),
                ));
            }
            if aac_lc.channel_configuration != AAC_LC_CHANNEL_CONFIGURATION {
                findings.push(field(
                    "channel_configuration",
                    format!(
                        "channel_configuration is {}, expected {AAC_LC_CHANNEL_CONFIGURATION}",
                        aac_lc.channel_configuration
                    ),
                ));
            }
            if aac_lc.frame_length_flag {
                findings.push(field(
                    "frame_length_flag",
                    "frame_length_flag is true, expected false".to_owned(),
                ));
            }
            if aac_lc.depends_on_core_coder {
                findings.push(field(
                    "depends_on_core_coder",
                    "depends_on_core_coder is true, expected false".to_owned(),
                ));
            }
            if aac_lc.extension_flag {
                findings.push(field(
                    "extension_flag",
                    "extension_flag is true, expected false".to_owned(),
                ));
            }
            if self.num_samples_per_frame != AAC_LC_NUM_SAMPLES_PER_FRAME {
                findings.push(field(
                    "num_samples_per_frame",
                    format!(
                        "num_samples_per_frame is {}, expected {AAC_LC_NUM_SAMPLES_PER_FRAME}",
                        self.num_samples_per_frame
                    ),
                ));
            }
        }
        let required = match &self.decoder_config {
            // A zero frame size already has its own Finding above; no roll
            // can be derived for it, so do not invent an expected value.
            DecoderConfig::Opus(_) => {
                required_opus_audio_roll_distance(self.num_samples_per_frame).ok()
            }
            DecoderConfig::AacLc(_) => Some(AAC_LC_AUDIO_ROLL_DISTANCE),
            _ => Some(required_audio_roll_distance(&self.decoder_config)),
        };
        if let Some(required) = required {
            if self.audio_roll_distance != required {
                findings.push(field(
                    "audio_roll_distance",
                    format!(
                        "audio_roll_distance is {}, expected {required} for this codec",
                        self.audio_roll_distance
                    ),
                ));
            }
        }
        findings
    }
}

/// `GetRequiredAudioRollDistance()` — `0` for LPCM and FLAC, `-1` for AAC-LC.
///
/// This is the **encoder-path derivation** (DESC-02). The reader never calls
/// it to replace a wire value; `validate()` calls it to name a mismatch (D-06).
/// Other variants retain the legacy zero fallback. For Opus use
/// [`required_opus_audio_roll_distance`], which also needs the frame size.
// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.h LpcmDecoderConfig::GetRequiredAudioRollDistance
#[must_use]
pub const fn required_audio_roll_distance(decoder_config: &DecoderConfig) -> i16 {
    match decoder_config {
        DecoderConfig::Lpcm(_) | DecoderConfig::Flac(_) => 0,
        DecoderConfig::AacLc(_) => AAC_LC_AUDIO_ROLL_DISTANCE,
        DecoderConfig::Opus(_) | DecoderConfig::Raw { .. } => 0,
    }
}

/// Derive the Opus roll distance as `-ceil(3840 / num_samples_per_frame)`.
///
/// # Errors
///
/// Returns [`ErrorKind::ZeroSamplesPerFrame`] for zero. Checked arithmetic
/// and signed conversion protect the calculation against overflow.
// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/opus_decoder_config.cc OpusDecoderConfig::GetRequiredAudioRollDistance
pub fn required_opus_audio_roll_distance(num_samples_per_frame: u32) -> Result<i16> {
    let zero = || {
        Error::new(
            ErrorKind::ZeroSamplesPerFrame,
            Location::Field("num_samples_per_frame"),
        )
    };
    let overflow = || {
        Error::new(
            ErrorKind::FramePlanOverflow,
            Location::Field("num_samples_per_frame"),
        )
    };
    let quotient = 3840_u32
        .checked_div(num_samples_per_frame)
        .ok_or_else(zero)?;
    let remainder = 3840_u32
        .checked_rem(num_samples_per_frame)
        .ok_or_else(zero)?;
    let ceiling = quotient
        .checked_add(u32::from(remainder != 0))
        .ok_or_else(overflow)?;
    i16::try_from(ceiling)
        .ok()
        .and_then(i16::checked_neg)
        .ok_or_else(overflow)
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
    } else if codec_id == CODEC_ID_OPUS && r.bytes_remaining() >= OPUS_DECODER_CONFIG_BYTES {
        DecoderConfig::Opus(read_opus_decoder_config(r)?)
    } else if codec_id == CODEC_ID_AAC && r.bytes_remaining() >= AAC_LC_DECODER_CONFIG_BYTES {
        let descriptor = r.read_uint8_span(AAC_LC_DECODER_CONFIG_BYTES)?.to_vec();
        if is_aac_lc_descriptor(&descriptor) {
            read_aac_lc_decoder_config(&descriptor)?
        } else {
            let remaining = r.bytes_remaining();
            let mut bytes = descriptor;
            bytes.extend_from_slice(r.read_uint8_span(remaining)?);
            DecoderConfig::Raw { codec_id, bytes }
        }
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
        DecoderConfig::Opus(cfg) => write_opus_decoder_config(w, cfg)?,
        DecoderConfig::AacLc(cfg) => write_aac_lc_decoder_config(w, cfg)?,
        DecoderConfig::Raw { bytes, .. } => w.write_bytes(bytes)?,
    }
    w.write_bytes(&v.trailing)
}

/// Find the MPEG-4 sampling-frequency index for an AAC-LC encoder input.
#[must_use]
const fn aac_lc_sampling_frequency_index(sample_rate: u32) -> Option<u8> {
    let mut index = 0;
    while index < AAC_LC_SAMPLE_RATES.len() {
        if AAC_LC_SAMPLE_RATES[index] == sample_rate {
            // `AAC_LC_SAMPLE_RATES` has thirteen entries, so this conversion
            // is bounded by 12.
            return Some(index as u8);
        }
        index += 1;
    }
    None
}

/// The only descriptor envelope shape this model types. A malformed MPEG-4
/// descriptor remains a raw blob so it can round-trip byte-for-byte.
fn is_aac_lc_descriptor(descriptor: &[u8]) -> bool {
    descriptor.len() == AAC_LC_DECODER_CONFIG_BYTES
        && descriptor[0] == 0x04
        && descriptor[1] == 13
        && descriptor[3] & 1 == 1
        && descriptor[15] == 0x05
        && descriptor[16] == 2
}

// ref: IAMF v1.1.0 §3.11.2 AAC-LC Specific; ISO/IEC 14496-1 DecoderConfigDescriptor
/// Read the fixed 19-byte MPEG-4 descriptor, preserving all semantic field
/// values even when they contradict IAMF's AAC-LC restrictions.
fn read_aac_lc_decoder_config(descriptor: &[u8]) -> Result<DecoderConfig> {
    let mut r = BitCursor::new(descriptor);
    let _tag = r.read_unsigned(8)?;
    let _length = r.read_unsigned(8)?;
    let object_type_indication = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    let stream_type = u8::try_from(r.read_unsigned(6)?).unwrap_or(0);
    let upstream = r.read_bool()?;
    let _reserved = r.read_bool()?;
    let buffer_size_db = u32::try_from(r.read_unsigned(24)?).unwrap_or(0);
    let max_bitrate = u32::try_from(r.read_unsigned(32)?).unwrap_or(0);
    let avg_bitrate = u32::try_from(r.read_unsigned(32)?).unwrap_or(0);
    let _specific_tag = r.read_unsigned(8)?;
    let _specific_length = r.read_unsigned(8)?;
    let audio_object_type = u8::try_from(r.read_unsigned(5)?).unwrap_or(0);
    let sampling_frequency_index = u8::try_from(r.read_unsigned(4)?).unwrap_or(0);
    let channel_configuration = u8::try_from(r.read_unsigned(4)?).unwrap_or(0);
    let frame_length_flag = r.read_bool()?;
    let depends_on_core_coder = r.read_bool()?;
    let extension_flag = r.read_bool()?;
    Ok(DecoderConfig::AacLc(AacLcDecoderConfig {
        object_type_indication,
        stream_type,
        upstream,
        buffer_size_db,
        max_bitrate,
        avg_bitrate,
        audio_object_type,
        sampling_frequency_index,
        channel_configuration,
        frame_length_flag,
        depends_on_core_coder,
        extension_flag,
    }))
}

// ref: IAMF v1.1.0 §3.11.2 AAC-LC Specific; ISO/IEC 14496-1 DecoderConfigDescriptor
/// Write the fixed IAMF MPEG-4 `DecoderConfigDescriptor` envelope and its
/// two-byte `AudioSpecificConfig` faithfully from the typed semantic fields.
fn write_aac_lc_decoder_config(w: &mut BitWriter, v: &AacLcDecoderConfig) -> Result<()> {
    w.write_unsigned(0x04, 8)?;
    w.write_unsigned(13, 8)?;
    w.write_unsigned(u64::from(v.object_type_indication), 8)?;
    w.write_unsigned(u64::from(v.stream_type), 6)?;
    w.write_bool(v.upstream)?;
    w.write_bool(true)?;
    w.write_unsigned(u64::from(v.buffer_size_db), 24)?;
    w.write_unsigned(u64::from(v.max_bitrate), 32)?;
    w.write_unsigned(u64::from(v.avg_bitrate), 32)?;
    w.write_unsigned(0x05, 8)?;
    w.write_unsigned(2, 8)?;
    w.write_unsigned(u64::from(v.audio_object_type), 5)?;
    w.write_unsigned(u64::from(v.sampling_frequency_index), 4)?;
    w.write_unsigned(u64::from(v.channel_configuration), 4)?;
    w.write_bool(v.frame_length_flag)?;
    w.write_bool(v.depends_on_core_coder)?;
    w.write_bool(v.extension_flag)
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

// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/opus_decoder_config.cc OpusDecoderConfig::ReadAndValidate
// ref: libiamf@v1.1.0 code/src/iamf_dec/opus/IAMF_opus_decoder.c iamf_opus_init
/// Read exactly eleven bytes, retaining all fields; multibyte values are big-endian.
fn read_opus_decoder_config(r: &mut BitCursor<'_>) -> Result<OpusDecoderConfig> {
    Ok(OpusDecoderConfig {
        version: u8::try_from(r.read_unsigned(8)?).unwrap_or(0),
        output_channel_count: u8::try_from(r.read_unsigned(8)?).unwrap_or(0),
        pre_skip: u16::try_from(r.read_unsigned(16)?).unwrap_or(0),
        input_sample_rate: u32::try_from(r.read_unsigned(32)?).unwrap_or(0),
        output_gain: i16::try_from(r.read_signed(16)?).unwrap_or(0),
        mapping_family: u8::try_from(r.read_unsigned(8)?).unwrap_or(0),
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/decoder_config/opus_decoder_config.cc OpusDecoderConfig::ValidateAndWrite
/// Write the eleven IAMF bytes faithfully, with big-endian multibyte fields.
fn write_opus_decoder_config(w: &mut BitWriter, v: &OpusDecoderConfig) -> Result<()> {
    w.write_unsigned(u64::from(v.version), 8)?;
    w.write_unsigned(u64::from(v.output_channel_count), 8)?;
    w.write_unsigned(u64::from(v.pre_skip), 16)?;
    w.write_unsigned(u64::from(v.input_sample_rate), 32)?;
    w.write_signed(i64::from(v.output_gain), 16)?;
    w.write_unsigned(u64::from(v.mapping_family), 8)
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
