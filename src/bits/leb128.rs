//! uleb128, both directions, hand-written (BITS-03, BITS-04).
//!
//! The encoder comes first and the decoder immediately follows it, so the two
//! halves are read together.
//!
//! Two hard caps come from the reference and both are enforced here:
//! `kMaxLeb128Size = 8` bytes, and a decoded value that must fit `uint32_t`.
//! A ninth continuation byte and an over-`u32` accumulation are typed errors,
//! not a silent `u64`.

use crate::bits::reader::BitCursor;
use crate::bits::writer::BitWriter;
use crate::error::{Error, ErrorKind, Location, Result};

/// `kMaxLeb128Size` — the reference's cap on how many bytes a uleb128 may span.
// ref: iamf-tools@v2.1.0 iamf/obu/types.h kMaxLeb128Size
pub(crate) const MAX_LEB128_SIZE: usize = 8;

/// How many bytes the minimal encoding of `value` occupies.
///
/// A `u32` needs at most five 7-bit groups.
// ref: iamf-tools@v2.1.0 iamf/common/leb_generator.h LebGenerator (kMinimum mode)
#[must_use]
pub(crate) const fn minimal_len(value: u32) -> usize {
    match value {
        0x0000_0000..=0x0000_007f => 1,
        0x0000_0080..=0x0000_3fff => 2,
        0x0000_4000..=0x001f_ffff => 3,
        0x0020_0000..=0x0fff_ffff => 4,
        _ => 5,
    }
}

// ref: iamf-tools@v2.1.0 iamf/common/leb_generator.h LebGenerator (kMinimum mode)
/// Emit `value` in minimal form: no trailing continuation byte, and never a
/// longer encoding than [`minimal_len`] says.
pub(crate) fn write_uleb128_minimal(w: &mut BitWriter, value: u32) -> Result<()> {
    // The length is decided up front by `minimal_len`, so minimality is
    // structural rather than emergent: this loop cannot append a trailing
    // `0x80 0x00`, because it does not decide when to stop.
    let len = minimal_len(value);
    let mut shift: u32 = 0;
    for group_index in 1..=len {
        let group = u64::from(value.checked_shr(shift).unwrap_or(0) & 0x7f);
        let byte = if group_index == len { group } else { group | 0x80 };
        w.write_unsigned(byte, 8)?;
        shift = shift.saturating_add(7);
    }
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadULeb128
/// Decode a uleb128 of 1..=8 bytes into a `u32`.
pub(crate) fn read_uleb128(r: &mut BitCursor<'_>) -> Result<u32> {
    // Both caps report the offset of the field's FIRST byte, not the byte the
    // violation was noticed at: the thing that is wrong is the whole field.
    let start = r.byte_position();
    let mut acc: u64 = 0;
    let mut shift: u32 = 0;
    for _ in 0..MAX_LEB128_SIZE {
        let byte = r.read_unsigned(8)?;
        // At most 8 groups of 7 bits reach bit 55, so the accumulator cannot
        // overflow `u64` — which is exactly why it is a `u64` and the narrowing
        // to `u32` happens once, at the end, where it can be rejected.
        let group = (byte & 0x7f).checked_shl(shift).unwrap_or(0);
        acc |= group;
        if byte & 0x80 == 0 {
            return u32::try_from(acc).map_err(|_| {
                Error::new(
                    ErrorKind::Leb128ValueTooLarge,
                    Location::InputOffset(start),
                )
            });
        }
        shift = shift.saturating_add(7);
    }
    // Eight bytes consumed and the eighth still asked for a ninth.
    Err(Error::new(
        ErrorKind::Leb128TooLong,
        Location::InputOffset(start),
    ))
}
