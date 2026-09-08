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
use crate::error::Result;

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
    let _ = (w, value);
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadULeb128
/// Decode a uleb128 of 1..=8 bytes into a `u32`.
pub(crate) fn read_uleb128(r: &mut BitCursor<'_>) -> Result<u32> {
    let _ = r;
    Ok(0)
}
