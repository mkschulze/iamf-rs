//! OBU-08 — the structural walk over a whole `.iamf` byte stream.
//!
//! Written **before the first encoder test**, deliberately: it is the cheapest
//! possible structural check on our own output later, and today it has the
//! whole vendored reference corpus to prove itself against with no reference
//! binary present at all (CONF-10, `tests/refcorpus.rs`).
//!
//! # Why the walk does not interpret the flags
//!
//! It reads byte 0 and `obu_size` and nothing else. That is sufficient
//! *because* `obu_size` counts the after-size fields — the trim pair and the
//! extension header — as well as the payload (OBU-02). A walker that tried to
//! interpret bit 6 in order to skip the trim fields would be re-deriving a
//! number the file already carries, and would get it wrong on exactly the
//! descriptor OBUs whose bit 6 is set illegally.
//!
//! # Threat T-01-18
//!
//! `obu_size` is attacker-controlled and drives offset arithmetic. Every step
//! here is `checked_*`, the walker allocates **one entry per OBU it has already
//! validated** and never from an `obu_size` value, and a boundary past
//! `bytes.len()` is [`ErrorKind::TruncatedObu`] rather than a boundary list
//! that quietly stops short. That distinction matters: `libiamf@v1.1.0`'s
//! splitter returns 0 in this case and its callers read that as end-of-stream,
//! so an oversized final size manifests there as a *shorter file* rather than a
//! decode failure.

use crate::bits::BitCursor;
use crate::error::{Error, ErrorKind, Location, Result};
use crate::obu::header::validate_obu_size;

/// Every OBU start offset in `bytes`, plus the final end offset.
///
/// The returned vector's **last element is `bytes.len()` for a well-formed
/// sequence**, which makes OBU-08's property a single equality check rather
/// than a length comparison the caller has to derive. An empty input yields
/// `[0]`, so the property holds degenerately instead of being a special case.
///
/// # Errors
///
/// [`ErrorKind::ObuTooLarge`] for an `obu_size` above the reference's derived
/// ceiling, and [`ErrorKind::TruncatedObu`] for one that would carry the OBU
/// past the end of the buffer.
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c IAMF_OBU_split
pub fn find_obu_boundaries(bytes: &[u8]) -> Result<Vec<usize>> {
    let mut boundaries = Vec::new();
    let mut position = 0_usize;

    loop {
        boundaries.push(position);
        if position >= bytes.len() {
            return Ok(boundaries);
        }

        let rest = bytes.get(position..).ok_or_else(|| {
            Error::new(
                ErrorKind::TruncatedObu,
                Location::InputOffset(offset(position)),
            )
        })?;
        let mut cursor = BitCursor::new(rest);

        // Byte 0 in one read: the walk needs none of its three flags, for the
        // reason in the module comment.
        cursor.read_unsigned(8).map_err(|_| truncated(position))?;
        let obu_size = cursor.read_uleb128().map_err(|e| match e.kind() {
            // A malformed length field is a framing defect at this OBU, not a
            // primitive-level surprise; a leb128 cap breach keeps its own kind.
            ErrorKind::UnexpectedEndOfInput => truncated(position),
            _ => e,
        })?;
        // The uleb128 may legally be non-minimal, so its length is measured, not
        // derived from the value (PARSE-04).
        let size_of_obu_size = usize::try_from(cursor.byte_position())
            .unwrap_or(usize::MAX)
            .checked_sub(1)
            .ok_or_else(|| truncated(position))?;

        // The ceiling is checked BEFORE the bounds check, so a 2 MiB claim
        // inside a small buffer reports the real defect rather than the
        // consequence of it.
        validate_obu_size(
            obu_size,
            size_of_obu_size,
            Location::InputOffset(offset(position)),
        )?;
        let obu_size = usize::try_from(obu_size).map_err(|_| too_large(position))?;

        let next = position
            .checked_add(1)
            .and_then(|v| v.checked_add(size_of_obu_size))
            .and_then(|v| v.checked_add(obu_size))
            .ok_or_else(|| truncated(position))?;
        if next > bytes.len() {
            return Err(truncated(position));
        }
        position = next;
    }
}

/// The offset an error at `position` reports.
fn offset(position: usize) -> u64 {
    u64::try_from(position).unwrap_or(u64::MAX)
}

/// This OBU claims more bytes than the buffer holds.
fn truncated(position: usize) -> Error {
    Error::new(
        ErrorKind::TruncatedObu,
        Location::InputOffset(offset(position)),
    )
}

/// This OBU's `obu_size` is above `kEntireObuSizeMaxTwoMegabytes`.
fn too_large(position: usize) -> Error {
    Error::new(
        ErrorKind::ObuTooLarge,
        Location::InputOffset(offset(position)),
    )
}
