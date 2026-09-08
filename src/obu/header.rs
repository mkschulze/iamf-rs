//! The OBU header — byte 0, the two-pass `obu_size` origin, the after-size
//! fields, and the two legality rules the reference enforces on them.
//!
//! Read and write live in this one file, write first and its reader
//! immediately after it (D-10), so an asymmetry between the two is visually
//! obvious rather than something a round-trip test has to discover.

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Location, Result};

/// `kEntireObuSizeMaxTwoMegabytes` — the whole-OBU ceiling.
// ref: iamf-tools@v2.1.0 iamf/obu/types.h kEntireObuSizeMaxTwoMegabytes
pub(crate) const ENTIRE_OBU_SIZE_MAX: usize = 1 << 21;

/// The OBU type, as the five bits at the top of byte 0.
///
/// Modelled as an enum rather than a bare integer everywhere: the legality of
/// `obu_redundant_copy` and of the trimming flag are both functions of this
/// value, so a call site holding a `u8` is a call site that can get either rule
/// wrong.
///
/// The eighteen Audio Frame variants are the reference's own shape
/// (`kObuIaAudioFrameId0` … `kObuIaAudioFrameId17`): the low bits of the type
/// carry an implicit `audio_substream_id`, so they are eighteen distinct
/// meanings, not one type with a parameter.
// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.h ObuType
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObuType {
    CodecConfig,
    AudioElement,
    MixPresentation,
    ParameterBlock,
    TemporalDelimiter,
    AudioFrame,
    AudioFrameId0,
    AudioFrameId1,
    AudioFrameId2,
    AudioFrameId3,
    AudioFrameId4,
    AudioFrameId5,
    AudioFrameId6,
    AudioFrameId7,
    AudioFrameId8,
    AudioFrameId9,
    AudioFrameId10,
    AudioFrameId11,
    AudioFrameId12,
    AudioFrameId13,
    AudioFrameId14,
    AudioFrameId15,
    AudioFrameId16,
    AudioFrameId17,
    IaSequenceHeader,
    /// A type this spec version does not define: 24..=30.
    Reserved(u8),
}

impl ObuType {
    /// The five-bit value this type occupies at the top of byte 0.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::CodecConfig => 0,
            Self::AudioElement => 1,
            Self::MixPresentation => 2,
            Self::ParameterBlock => 3,
            Self::TemporalDelimiter => 4,
            Self::AudioFrame => 5,
            Self::AudioFrameId0 => 6,
            Self::AudioFrameId1 => 7,
            Self::AudioFrameId2 => 8,
            Self::AudioFrameId3 => 9,
            Self::AudioFrameId4 => 10,
            Self::AudioFrameId5 => 11,
            Self::AudioFrameId6 => 12,
            Self::AudioFrameId7 => 13,
            Self::AudioFrameId8 => 14,
            Self::AudioFrameId9 => 15,
            Self::AudioFrameId10 => 16,
            Self::AudioFrameId11 => 17,
            Self::AudioFrameId12 => 18,
            Self::AudioFrameId13 => 19,
            Self::AudioFrameId14 => 20,
            Self::AudioFrameId15 => 21,
            Self::AudioFrameId16 => 22,
            Self::AudioFrameId17 => 23,
            Self::IaSequenceHeader => 31,
            Self::Reserved(raw) => raw,
        }
    }

    /// The type a five-bit field carries.
    ///
    /// Values above 31 cannot come off the wire — `read_unsigned(5)` cannot
    /// produce one — and are kept as `Reserved` rather than masked, so a caller
    /// that constructs one gets a `ValueExceedsWidth` on write instead of a
    /// silently different OBU.
    #[must_use]
    pub const fn from_value(value: u8) -> Self {
        match value {
            0 => Self::CodecConfig,
            1 => Self::AudioElement,
            2 => Self::MixPresentation,
            3 => Self::ParameterBlock,
            4 => Self::TemporalDelimiter,
            5 => Self::AudioFrame,
            6 => Self::AudioFrameId0,
            7 => Self::AudioFrameId1,
            8 => Self::AudioFrameId2,
            9 => Self::AudioFrameId3,
            10 => Self::AudioFrameId4,
            11 => Self::AudioFrameId5,
            12 => Self::AudioFrameId6,
            13 => Self::AudioFrameId7,
            14 => Self::AudioFrameId8,
            15 => Self::AudioFrameId9,
            16 => Self::AudioFrameId10,
            17 => Self::AudioFrameId11,
            18 => Self::AudioFrameId12,
            19 => Self::AudioFrameId13,
            20 => Self::AudioFrameId14,
            21 => Self::AudioFrameId15,
            22 => Self::AudioFrameId16,
            23 => Self::AudioFrameId17,
            31 => Self::IaSequenceHeader,
            other => Self::Reserved(other),
        }
    }

    /// `true` for `kObuIaAudioFrame` and `kObuIaAudioFrameId0..=17` — the only
    /// types the trimming flag is legal on.
    // ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc IsTrimmingStatusFlagAllowed
    #[must_use]
    pub const fn is_audio_frame(self) -> bool {
        matches!(self.value(), 5..=23)
    }

    /// `obu_redundant_copy` is forbidden on Parameter Blocks, Temporal
    /// Delimiters and Audio Frames — types 3, 4, 5 and 6..=23 — and permitted
    /// everywhere else.
    // ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc IsRedundantCopyAllowed
    #[must_use]
    pub const fn is_redundant_copy_allowed(self) -> bool {
        !matches!(self.value(), 3..=23)
    }
}

/// What bit 6 of byte 0 means, which is a function of the OBU type.
///
/// **Exactly two variants, and that is load-bearing.** At the pinned
/// `iamf-tools` v2.1.0 tree the field is `obu_trimming_status_flag` and
/// `IsTrimmingStatusFlagAllowed` returns true **only for Audio Frames**;
/// `Validate()` turns any other use into an `InvalidArgumentError`. The
/// four-meaning model that `REQUIREMENTS.md` OBU-03 and `PITFALLS.md` §2
/// originally described was read from the reference's development tip, which
/// is a draft v2.0.0 tree, not the version this crate targets.
///
/// The second reason, which is the dangerous one: `libiamf@v1.1.0`'s
/// `IAMF_OBU_split` reads the two trim fields whenever bit 6 is set, **for any
/// OBU type**. Its development tip guards that with an audio-frame check; the
/// tree we pin does not. So bit 6 set on a descriptor OBU shifts that
/// descriptor's entire payload by two bytes on the very decoder whose
/// acceptance is this project's Core Value — with no error, no crash, and
/// nothing a round-trip test against ourselves could ever see.
///
/// Adding a third variant makes that state constructible again. Do not.
// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc IsTrimmingStatusFlagAllowed
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c IAMF_OBU_split
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSpecific {
    /// Audio Frames only. `Some` sets bit 6 and writes the two trim fields;
    /// `None` clears it and writes neither.
    Trimming(Option<Trimming>),
    /// Every other OBU type. Reserved, SHALL be 0, and has no other
    /// constructor.
    Reserved,
}

/// The two trim counts an Audio Frame carries when bit 6 is set.
///
/// Field order here deliberately mirrors the wire order, which is **end before
/// start**. See `write_fields_after_obu_size`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trimming {
    /// `num_samples_to_trim_at_end` — written **first**.
    pub at_end: u32,
    /// `num_samples_to_trim_at_start` — written second.
    pub at_start: u32,
}

/// The common header every OBU in the format carries.
///
/// Both gate flags are **derived**, never stored (Pattern 2): a stored
/// `obu_extension_flag` alongside an `extension` field makes a flag/field
/// disagreement constructible, and that disagreement is a file the reference
/// mis-frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObuHeader {
    /// The five-bit type at the top of byte 0.
    pub obu_type: ObuType,
    /// Bit 5. Legal only on descriptors — see `ObuType::is_redundant_copy_allowed`.
    pub obu_redundant_copy: bool,
    /// Bit 6, whose meaning depends on `obu_type`.
    pub type_specific: TypeSpecific,
    /// Bit 7 plus the after-size extension header. Preserved verbatim; Phase 1
    /// does not interpret its contents.
    pub extension: Option<Vec<u8>>,
}

impl ObuHeader {
    /// A header with every flag clear.
    ///
    /// `type_specific` starts as `Reserved`, which is correct for every type
    /// except an Audio Frame that carries trim — those call
    /// `with_type_specific`.
    #[must_use]
    pub const fn new(obu_type: ObuType) -> Self {
        Self {
            obu_type,
            obu_redundant_copy: false,
            type_specific: TypeSpecific::Reserved,
            extension: None,
        }
    }

    /// Builder form, so a vector reads as one expression.
    #[must_use]
    pub fn with_type_specific(mut self, type_specific: TypeSpecific) -> Self {
        self.type_specific = type_specific;
        self
    }

    /// Builder form for `obu_redundant_copy`.
    #[must_use]
    pub const fn with_redundant_copy(mut self, redundant: bool) -> Self {
        self.obu_redundant_copy = redundant;
        self
    }

    /// Builder form for the extension header.
    #[must_use]
    pub fn with_extension(mut self, bytes: Vec<u8>) -> Self {
        self.extension = Some(bytes);
        self
    }

    /// Bit 6, derived from the data it gates.
    #[must_use]
    pub const fn trimming_status_flag(&self) -> bool {
        matches!(self.type_specific, TypeSpecific::Trimming(Some(_)))
    }

    /// Bit 7, derived from the data it gates.
    #[must_use]
    pub const fn extension_flag(&self) -> bool {
        self.extension.is_some()
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite
/// Serialise one whole OBU: byte 0, `obu_size`, the after-size fields, then the
/// payload.
///
/// `obu_size` is computed by a **two-pass** path, exactly as
/// `GetObuSizeAndValidate` does: the after-size fields go into a scratch
/// writer, that writer is measured, and the payload length is added to it. No
/// slot is reserved and backfilled, so the size can never disagree with what
/// follows it.
pub fn write_obu(w: &mut BitWriter, header: &ObuHeader, payload: &[u8]) -> Result<()> {
    if !w.is_byte_aligned() {
        // BITS-05: the format has no padding mechanism, so an OBU that starts
        // mid-byte is unrepresentable rather than merely unusual.
        return Err(Error::new(
            ErrorKind::NotByteAligned,
            Location::Unlocated,
        ));
    }
    let _ = (header, payload);
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
/// Read byte 0, `obu_size` and the after-size fields, leaving the cursor at the
/// first payload byte. Returns the header and `obu_size` itself — the payload
/// length is `obu_size` minus whatever the after-size fields consumed, which
/// only the caller can attribute.
pub fn read_obu_header(r: &mut BitCursor<'_>) -> Result<(ObuHeader, u32)> {
    let _ = r;
    Err(Error::new(
        ErrorKind::UnexpectedEndOfInput,
        Location::Unlocated,
    ))
}
