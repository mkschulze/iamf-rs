//! The Temporal Delimiter OBU (type 4) — the emptiest OBU in the format.
//!
//! Read lives immediately before write (D-10), as everywhere else in this
//! module, even though both are no-ops: the pair is what makes it visible that
//! neither side reads or writes a payload, which is the whole content of the
//! type.
//!
//! # Two bytes, and bit 6 is reserved
//!
//! A Temporal Delimiter is `20 00`: type 4 (`00100`) in the top five bits with
//! all three flags clear, then `obu_size` 0 as a single `0x00`. It carries no
//! payload at all.
//!
//! **Bit 6 is reserved and SHALL be 0 at IAMF v1.1.0.** The
//! `is_not_key_frame` reading that `REQUIREMENTS.md` OBU-03 originally
//! described is a draft-v2.0.0 artefact and was corrected in plan 01-04:
//! `ObuHeader::type_specific` has exactly two variants, and `TypeSpecific::
//! Reserved` is the only one a Temporal Delimiter can hold.
//!
//! That is not a tidiness point. `libiamf@v1.1.0`'s `IAMF_OBU_split` reads the
//! two trim fields whenever bit 6 is set, **for any OBU type** — its
//! development tip guards that with an audio-frame check, the tree we pin does
//! not. A Temporal Delimiter with bit 6 set therefore makes the pinned decoder
//! read two trim bytes out of the **next** OBU, shifting that OBU's entire
//! framing, with no error and no crash on either side. `write_obu` refuses it.

use crate::bits::{BitCursor, BitWriter};
use crate::error::Result;

/// The Temporal Delimiter OBU payload — which is nothing.
///
/// A unit struct rather than a struct with a `trailing: Vec<u8>`: `obu_size`
/// is 0, so the bounded payload sub-reader `read_obu_with` builds is empty and
/// the OBU-level `trailing` is empty with it. A payload-level remainder field
/// here would be permanently empty and would only invite a second drain site
/// (OBU-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemporalDelimiter;

// ref: iamf-tools@v2.1.0 iamf/obu/temporal_delimiter.h TemporalDelimiterObu
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c IAMF_OBU_split
/// Read a Temporal Delimiter payload: there is none.
pub fn read_temporal_delimiter(_r: &mut BitCursor<'_>) -> Result<TemporalDelimiter> {
    Ok(TemporalDelimiter)
}

// ref: iamf-tools@v2.1.0 iamf/obu/temporal_delimiter.h TemporalDelimiterObu
/// Write a Temporal Delimiter payload: there is none. `obu_size` is measured
/// from what this emits, so emitting nothing is what makes it 0.
pub fn write_temporal_delimiter(_w: &mut BitWriter, _v: &TemporalDelimiter) -> Result<()> {
    Ok(())
}
