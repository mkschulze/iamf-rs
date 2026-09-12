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

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc WriteFieldsAfterObuSize
// NOTE: the trim fields are written **END first, then START** — the opposite
// of the order their names suggest and of the order they are usually spoken
// in. Whenever the two values are equal, which is true of every Audio Frame in
// every vendored reference fixture, the two orders produce identical bytes, so
// no golden file can catch a swap. `tests/obu_header.rs`'s differing-value
// vector is the only thing that can. Do not "tidy" this order.
/// Write the fields that live between `obu_size` and the payload, in the
/// reference's order: end-trim, start-trim, extension size, extension bytes.
fn write_fields_after_obu_size(w: &mut BitWriter, header: &ObuHeader) -> Result<()> {
    if let TypeSpecific::Trimming(Some(trim)) = header.type_specific {
        w.write_uleb128_minimal(trim.at_end)?;
        w.write_uleb128_minimal(trim.at_start)?;
    }
    if let Some(bytes) = header.extension.as_ref() {
        // Phase 1 preserves the extension verbatim and interprets nothing
        // inside it (OBU-06).
        let len = u32::try_from(bytes.len())
            .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::OutputOffset(0)))?;
        w.write_uleb128_minimal(len)?;
        w.write_bytes(bytes)?;
    }
    Ok(())
}

/// The two rules `Validate()` enforces on byte 0, checked before a single bit
/// is emitted.
///
/// Both are rejections rather than validations-after-the-fact because the file
/// they would otherwise produce is one the reference mis-reads *silently*:
/// `libiamf@v1.1.0`'s `IAMF_OBU_split` reads the two trim fields for any type
/// whose bit 6 is set, so a trimming flag on a descriptor shifts that
/// descriptor's payload by two bytes with no error and no crash on either side.
// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::Validate
fn validate_header(header: &ObuHeader) -> Result<()> {
    if header.trimming_status_flag() && !header.obu_type.is_audio_frame() {
        return Err(Error::new(
            ErrorKind::TrimmingFlagNotAllowed,
            Location::Field("obu_trimming_status_flag"),
        ));
    }
    if header.obu_redundant_copy && !header.obu_type.is_redundant_copy_allowed() {
        return Err(Error::new(
            ErrorKind::RedundantCopyNotAllowed,
            Location::Field("obu_redundant_copy"),
        ));
    }
    Ok(())
}

/// `obu_size` for an already-serialised after-size buffer and payload.
///
/// Split out from [`write_obu`] because it is the whole of Pattern 1 and the
/// whole of OBU-02: `obu_size` is **measured**, never reserved and backfilled,
/// and it counts the after-size fields as well as the payload.
///
/// The alignment check mirrors `GetObuSizeAndValidate`'s own
/// `!temp_wb_after_obu_size.IsByteAligned()` guard. It is unreachable through
/// the public path today — every after-size field is a whole number of bytes —
/// which is exactly why it is checked rather than assumed: the field set grows.
// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc GetObuSizeAndValidate
pub(crate) fn obu_size_for(after: &BitWriter, payload_len: usize) -> Result<u32> {
    if !after.is_byte_aligned() {
        return Err(Error::new(
            ErrorKind::UnalignedAfterSizeFields,
            Location::OutputOffset(0),
        ));
    }
    let total = after
        .len_bytes()
        .checked_add(payload_len)
        .ok_or_else(|| Error::new(ErrorKind::ObuSizeOverflow, Location::OutputOffset(0)))?;
    let obu_size = u32::try_from(total)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::OutputOffset(0)))?;

    // max_obu_size = kEntireObuSizeMaxTwoMegabytes - 1 - size_of_obu_size.
    // The size field's own length depends on the value it carries, so the bound
    // is computed from the candidate rather than from a constant.
    let size_of_obu_size = crate::bits::minimal_uleb128_len(obu_size);
    validate_obu_size(obu_size, size_of_obu_size, Location::OutputOffset(0))?;
    Ok(obu_size)
}

pub(crate) fn validate_obu_size(
    obu_size: u32,
    size_of_obu_size: usize,
    at: Location,
) -> Result<()> {
    let obu_size = usize::try_from(obu_size).map_err(|_| Error::new(ErrorKind::ObuTooLarge, at))?;
    let max = ENTIRE_OBU_SIZE_MAX
        .checked_sub(1)
        .and_then(|v| v.checked_sub(size_of_obu_size))
        .ok_or_else(|| Error::new(ErrorKind::ObuTooLarge, at))?;
    if obu_size > max {
        return Err(Error::new(ErrorKind::ObuTooLarge, at));
    }
    Ok(())
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
        return Err(Error::new(ErrorKind::NotByteAligned, Location::Unlocated));
    }
    validate_header(header)?;

    // Pass one: the fields between `obu_size` and the payload, into a scratch
    // buffer that exists only to be measured.
    let mut after = BitWriter::new();
    write_fields_after_obu_size(&mut after, header)?;
    let obu_size = obu_size_for(&after, payload.len())?;

    // Pass two: byte 0, the measured size, then the bytes it counted.
    w.write_unsigned(u64::from(header.obu_type.value()), 5)?;
    w.write_bool(header.obu_redundant_copy)?;
    w.write_bool(header.trimming_status_flag())?;
    w.write_bool(header.extension_flag())?;
    w.write_uleb128_minimal(obu_size)?;
    w.write_bytes(&after.finish()?)?;
    w.write_bytes(payload)?;
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
/// Read byte 0, `obu_size` and the after-size fields, leaving the cursor at the
/// first payload byte. Returns the header and `obu_size` itself — the payload
/// length is `obu_size` minus whatever the after-size fields consumed, which
/// only the caller can attribute.
pub fn read_obu_header(r: &mut BitCursor<'_>) -> Result<(ObuHeader, u32)> {
    let (header, obu_size, _) = read_obu_header_parts(r)?;
    Ok((header, obu_size))
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
/// As [`read_obu_header`], but also reporting how many bytes the after-size
/// fields consumed.
///
/// The payload length is `obu_size` minus that count, and it is **measured**
/// rather than recomputed: `size_of_obu_size` cannot be derived from the value
/// by way of `minimal_len`, because a foreign file may legally carry a
/// non-minimal, fixed-size uleb128 (`LebGenerator::kFixedSize`; Phase 2's
/// PARSE-04). Deriving it would put every subsequent byte offset in the file
/// wrong for exactly the files that are hardest to debug.
pub(crate) fn read_obu_header_parts(r: &mut BitCursor<'_>) -> Result<(ObuHeader, u32, u64)> {
    let obu_type = ObuType::from_value(u8::try_from(r.read_unsigned(5)?).unwrap_or(0));
    let obu_redundant_copy = r.read_bool()?;
    let trimming_status_flag = r.read_bool()?;
    let extension_flag = r.read_bool()?;
    let size_start = r.byte_position();
    let obu_size = r.read_uleb128()?;
    let size_of_obu_size = r
        .byte_position()
        .checked_sub(size_start)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(size_start)))?;
    validate_obu_size(
        obu_size,
        size_of_obu_size,
        Location::InputOffset(size_start.saturating_sub(1)),
    )?;
    let after_size_start = r.byte_position();

    // The reader is deliberately NOT stricter than the reference. It does not
    // apply `validate_header`'s two rules: `libiamf@v1.1.0` reads the trim
    // fields for *any* type whose bit 6 is set, so this is what the pinned
    // decoder sees, and rejecting a file the reference accepts is as much a
    // conformance failure as accepting one it rejects. `write_obu` is where the
    // rules bite, so we never emit such a file ourselves.
    //
    // END first, then START — see the NOTE on `write_fields_after_obu_size`.
    let type_specific = if trimming_status_flag {
        let at_end = r.read_uleb128()?;
        let at_start = r.read_uleb128()?;
        TypeSpecific::Trimming(Some(Trimming { at_end, at_start }))
    } else if obu_type.is_audio_frame() {
        TypeSpecific::Trimming(None)
    } else {
        TypeSpecific::Reserved
    };

    let extension = if extension_flag {
        Some(read_extension_header(r)?)
    } else {
        None
    };

    let after_size_bytes = r
        .byte_position()
        .checked_sub(after_size_start)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::ObuSizeOverflow,
                Location::InputOffset(after_size_start),
            )
        })?;

    Ok((
        ObuHeader {
            obu_type,
            obu_redundant_copy,
            type_specific,
            extension,
        },
        obu_size,
        after_size_bytes,
    ))
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ReadFieldsAfterObuSize
/// Read `extension_header_size` and exactly that many bytes.
///
/// The length is attacker-controlled and drives an allocation, so it goes
/// through `read_uint8_span`, which caps it against `bytes_remaining()` before
/// anything is reserved — the one rule every count in this format obeys
/// (`src/bits/reader.rs`, threat T-01-19).
fn read_extension_header(r: &mut BitCursor<'_>) -> Result<Vec<u8>> {
    let start = r.byte_position();
    let len = r.read_uleb128()?;
    let len = usize::try_from(len)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
    Ok(r.read_uint8_span(len)?.to_vec())
}

#[cfg(test)]
mod tests {
    //! `obu_size_for` is `pub(crate)`, so its vectors live here rather than in
    //! `tests/obu_header.rs`. The alignment guard in particular is unreachable
    //! through the public path — every after-size field is a whole number of
    //! bytes — and this is the only place it can be exercised at all.

    use super::{obu_size_for, ObuType, ENTIRE_OBU_SIZE_MAX};
    use crate::bits::BitWriter;
    use crate::error::ErrorKind;

    #[test]
    fn an_unaligned_after_size_buffer_is_refused_before_any_byte_is_emitted() {
        let mut after = BitWriter::new();
        after.write_bool(true).expect("one bit fits");

        let err = obu_size_for(&after, 0)
            .expect_err("an unaligned after-size buffer is refused")
            .kind()
            .clone();

        assert_eq!(err, ErrorKind::UnalignedAfterSizeFields);
    }

    #[test]
    fn obu_size_counts_the_after_size_bytes_as_well_as_the_payload() {
        let mut after = BitWriter::new();
        after.write_uleb128_minimal(64).expect("end trim");
        after.write_uleb128_minimal(0).expect("start trim");

        assert_eq!(
            obu_size_for(&after, 512).expect("well under the ceiling"),
            514,
            "2 trim bytes + 512 payload bytes — offset 0x7D33 of test_000003.iamf"
        );
    }

    #[test]
    fn the_ceiling_is_derived_from_the_size_fields_own_length() {
        let after = BitWriter::new();
        // A payload of exactly `(1 << 21) - 1 - 3` fits; one more byte does not,
        // because `obu_size` at that magnitude occupies three uleb128 bytes.
        let limit = ENTIRE_OBU_SIZE_MAX - 1 - 3;

        assert!(obu_size_for(&after, limit).is_ok());
        assert_eq!(
            obu_size_for(&after, limit.saturating_add(1))
                .expect_err("one byte over the derived bound")
                .kind()
                .clone(),
            ErrorKind::ObuTooLarge
        );
    }

    #[test]
    fn the_two_legality_predicates_match_the_references_tables() {
        // IsTrimmingStatusFlagAllowed: audio frames only.
        assert!(ObuType::AudioFrame.is_audio_frame());
        assert!(ObuType::AudioFrameId0.is_audio_frame());
        assert!(ObuType::AudioFrameId17.is_audio_frame());
        assert!(!ObuType::TemporalDelimiter.is_audio_frame());
        assert!(!ObuType::MixPresentation.is_audio_frame());
        assert!(!ObuType::IaSequenceHeader.is_audio_frame());

        // IsRedundantCopyAllowed: forbidden on 3, 4, 5 and 6..=23.
        assert!(ObuType::CodecConfig.is_redundant_copy_allowed());
        assert!(ObuType::AudioElement.is_redundant_copy_allowed());
        assert!(ObuType::MixPresentation.is_redundant_copy_allowed());
        assert!(ObuType::IaSequenceHeader.is_redundant_copy_allowed());
        assert!(!ObuType::ParameterBlock.is_redundant_copy_allowed());
        assert!(!ObuType::TemporalDelimiter.is_redundant_copy_allowed());
        assert!(!ObuType::AudioFrame.is_redundant_copy_allowed());
        assert!(!ObuType::AudioFrameId17.is_redundant_copy_allowed());
    }

    #[test]
    fn every_five_bit_value_round_trips_through_the_type_enum() {
        for raw in 0_u8..32 {
            assert_eq!(ObuType::from_value(raw).value(), raw);
        }
    }
}
