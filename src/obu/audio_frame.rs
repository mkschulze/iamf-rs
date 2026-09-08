//! The Audio Frame OBU — TIME-01's implicit substream ids and the payload the
//! whole format exists to carry.
//!
//! Read lives immediately before write (D-10), so an asymmetry between the two
//! is visually obvious rather than something a round-trip test has to
//! discover.
//!
//! # The implicit-id rule, and why the framing has to be right here
//!
//! An `audio_substream_id` of **17 or less is encoded in the OBU type itself**
//! as `6 + id` — types 6 through 23, `kObuIaAudioFrameId0` through
//! `kObuIaAudioFrameId17` — with no `audio_substream_id` field in the payload
//! at all. An id above 17 uses type 5 with an explicit uleb128 id at the head
//! of the payload.
//!
//! `libiamf` performs **no payload-level rejection at all** on an audio frame.
//! It reads the explicit id only for type 5, derives it for 6..=23, copies the
//! trim values from the header, and takes the whole remainder as the frame.
//! Every audio-frame failure mode is caught downstream in the PCM decoder — a
//! frame count that disagrees with `substream_count`, or a coupled substream
//! whose byte length disagrees with substream 0's — or not at all. That is why
//! the framing has to be right *here*: nothing between this code and the
//! decoded PCM will notice if it is not.
//!
//! # The payload is taken by byte offset, never bit-by-bit
//!
//! `obu_header.rs` asserts byte alignment at every OBU boundary and the frame
//! payload is opaque bytes at this layer, so it is copied from a byte-offset
//! slice of the bounded sub-reader. Bit-walking a payload that may be 2 MiB
//! long would be both slow and pointless.

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Finding, Location, Result};
use crate::obu::header::{ObuType, Trimming, TypeSpecific};
use crate::obu::{Obu, ObuHeader};

/// The largest `audio_substream_id` the OBU type can carry implicitly.
///
/// `kObuIaAudioFrameId17 - kObuIaAudioFrameId0`.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_frame.cc GetObuType
pub const MAX_IMPLICIT_SUBSTREAM_ID: u32 = 17;

/// The Audio Frame OBU payload.
///
/// **`trimming` is deliberately absent.** The two trim counts live in the OBU
/// header ([`TypeSpecific::Trimming`]) and nowhere else; a copy here would make
/// a header/payload disagreement constructible, which is exactly the class of
/// defect Pattern 2 exists to prevent — and a trim value that disagrees with
/// the header's is a file the reference mis-frames with no error on either
/// side. Use [`AudioFrame::into_obu`] to attach trimming.
///
/// **A payload-level `trailing` is deliberately absent too.** The frame claims
/// the whole remainder of the payload by construction, so the OBU-level
/// `trailing` drained by `read_obu_with_header` is always empty and a second
/// remainder field would be dead weight (D-05 precedence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    /// `audio_substream_id`, whether it came from the OBU type or from an
    /// explicit field. Which of the two is on the wire is recorded by
    /// `ObuHeader::obu_type`, never duplicated here.
    pub substream_id: u32,
    /// The frame itself, verbatim. For IAMF-LPCM these are PCM samples in the
    /// endianness the Codec Config's `sample_format_flags` declares.
    pub payload: Vec<u8>,
}

impl AudioFrame {
    /// A frame for `substream_id` carrying `payload`.
    #[must_use]
    pub const fn new(substream_id: u32, payload: Vec<u8>) -> Self {
        Self {
            substream_id,
            payload,
        }
    }

    /// Wrap this frame in its **canonical** OBU: the implicit type when the id
    /// permits one, type 5 otherwise, with `trimming` in the header.
    #[must_use]
    pub fn into_obu(self, trimming: Option<Trimming>) -> Obu<Self> {
        let header = ObuHeader::new(obu_type_for(self.substream_id))
            .with_type_specific(TypeSpecific::Trimming(trimming));
        Obu::new(header, self)
    }

    /// Report a frame whose explicit id could have been implicit.
    ///
    /// A type-5 frame carrying an id of 17 or less is legal but non-canonical.
    /// Per D-06 the reader stores what the wire said and this reports it,
    /// rather than the writer rewriting the frame as `6 + id` — which is what
    /// keeps `serialize(parse(bytes)) == bytes` true for foreign files.
    #[must_use]
    pub fn validate(&self, obu_type: ObuType) -> Vec<Finding> {
        let mut findings = Vec::new();
        if obu_type == ObuType::AudioFrame && self.substream_id <= MAX_IMPLICIT_SUBSTREAM_ID {
            findings.push(Finding {
                at: Location::Field("audio_substream_id"),
                message: format!(
                    "audio_substream_id {} is 17 or less, so this frame could have used the \
                     implicit OBU type {} instead of type 5",
                    self.substream_id,
                    obu_type_for(self.substream_id).value()
                ),
            });
        }
        findings
    }

    /// Report a frame claiming a substream the Audio Element does not declare
    /// (T-01-36).
    ///
    /// A Finding rather than a parse failure, because `libiamf` accepts such a
    /// file and rejecting one the reference accepts is as much a conformance
    /// failure as accepting one it rejects.
    #[must_use]
    pub fn validate_against_element(&self, declared_substream_ids: &[u32]) -> Vec<Finding> {
        let mut findings = Vec::new();
        if !declared_substream_ids.contains(&self.substream_id) {
            findings.push(Finding {
                at: Location::Field("audio_substream_id"),
                message: format!(
                    "audio_substream_id {} is not among the audio_substream_ids the referenced \
                     Audio Element declares",
                    self.substream_id
                ),
            });
        }
        findings
    }
}

// The two halves of the implicit-id rule, adjacent and matched. Splitting them
// across the file is how the encoding and the decoding of the same rule drift.

/// The OBU type an `audio_substream_id` is written as.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_frame.cc GetObuType
#[must_use]
pub fn obu_type_for(_substream_id: u32) -> ObuType {
    // STUB(GREEN): the implicit-id encoding lands with the wire behaviour.
    ObuType::AudioFrame
}

/// The `audio_substream_id` an OBU type carries implicitly, or `None` when the
/// type carries an explicit id (type 5) or is not an Audio Frame at all.
// ref: iamf-tools@v2.1.0 iamf/obu/audio_frame.cc AudioFrameObu::ReadAndValidatePayloadDerived
#[must_use]
pub fn substream_id_for(_obu_type: ObuType) -> Option<u32> {
    // STUB(GREEN): the inverse lands with `obu_type_for`.
    None
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_frame.cc AudioFrameObu::ReadAndValidatePayloadDerived
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c IAMF_OBU_split
/// Read an Audio Frame payload, given the OBU type that framed it.
///
/// The type is an **explicit argument** because it is what decides whether an
/// `audio_substream_id` field is present at all. There is no way to answer that
/// from the payload bytes alone.
pub fn read_audio_frame(_obu_type: &ObuHeader, _r: &mut BitCursor<'_>) -> Result<AudioFrame> {
    // STUB(GREEN): reads nothing and claims nothing.
    Ok(AudioFrame::new(0, Vec::new()))
}

// ref: iamf-tools@v2.1.0 iamf/obu/audio_frame.cc AudioFrameObu::ValidateAndWritePayload
/// Write an Audio Frame payload, given the OBU type that frames it.
pub fn write_audio_frame(
    _w: &mut BitWriter,
    _header: &ObuHeader,
    _frame: &AudioFrame,
) -> Result<()> {
    // STUB(GREEN): emits nothing.
    Ok(())
}
