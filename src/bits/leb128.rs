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

// ref: iamf-tools@v2.1.0 iamf/common/leb_generator.h LebGenerator (kFixedSize mode)
/// Emit `value` in exactly `size` bytes (1..=8), padding with continuation
/// bytes rather than shortening — the reference's `kFixedSize` mode.
///
/// Deliberately `pub(crate)` (D-03). Byte-identity must be caller-independent,
/// so there is no public `LebMode` knob; see the module doc comment on
/// `src/bits/mod.rs` for what this is actually for, which is **not** what D-03
/// records.
pub(crate) fn write_uleb128_fixed(w: &mut BitWriter, value: u32, size: u8) -> Result<()> {
    let requested = usize::from(size);
    if requested == 0 || requested > MAX_LEB128_SIZE {
        return Err(Error::new(
            ErrorKind::Leb128SizeInvalid { size },
            Location::OutputOffset(w.output_offset()),
        ));
    }
    if minimal_len(value) > requested {
        // A shorter field than the value needs would encode a different number.
        return Err(Error::new(
            ErrorKind::Leb128ValueTooLarge,
            Location::OutputOffset(w.output_offset()),
        ));
    }
    let mut shift: u32 = 0;
    for group_index in 1..=requested {
        let group = u64::from(value.checked_shr(shift).unwrap_or(0) & 0x7f);
        let byte = if group_index == requested {
            group
        } else {
            group | 0x80
        };
        w.write_unsigned(byte, 8)?;
        shift = shift.saturating_add(7);
    }
    Ok(())
}

// ref: iamf-tools@v2.1.0 iamf/common/leb_generator.h LebGenerator (kMinimum mode)
/// Emit `value` in minimal form: no trailing continuation byte, and never a
/// longer encoding than [`minimal_len`] says.
pub(crate) fn write_uleb128_minimal(w: &mut BitWriter, value: u32) -> Result<()> {
    // Minimal form *is* fixed-size form at the minimal length — which is what
    // the reference does too: `LebGenerator` computes a length and then emits
    // that many groups, and `kMinimum` differs from `kFixedSize` only in where
    // the length comes from. Expressing it that way means there is one encoder
    // loop rather than two that can drift apart, and minimality is structural:
    // the loop never decides when to stop, so it cannot append a trailing
    // `0x80 0x00`.
    //
    // `minimal_len` returns 1..=5, so the conversion cannot fail; 5 is the
    // unreachable fallback rather than a value that would change the output.
    let len = u8::try_from(minimal_len(value)).unwrap_or(5);
    write_uleb128_fixed(w, value, len)
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
                Error::new(ErrorKind::Leb128ValueTooLarge, Location::InputOffset(start))
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

#[cfg(test)]
mod tests {
    //! `minimal_len` and the fixed-size encoder are `pub(crate)`, so their
    //! vectors live here rather than in `tests/vectors.rs`. They are
    //! hand-computed the same way (D-25): the group boundaries are powers of
    //! 2^7, worked out from the encoding, not captured from this code.

    use super::{MAX_LEB128_SIZE, minimal_len, write_uleb128_fixed};
    use crate::bits::{BitCursor, BitWriter};
    use crate::error::ErrorKind;

    #[test]
    fn minimal_len_changes_at_every_seven_bit_boundary() {
        assert_eq!(minimal_len(0), 1);
        assert_eq!(minimal_len(127), 1);
        assert_eq!(minimal_len(128), 2);
        assert_eq!(minimal_len(16_383), 2);
        assert_eq!(minimal_len(16_384), 3);
        assert_eq!(minimal_len((1 << 21) - 1), 3);
        assert_eq!(minimal_len(1 << 21), 4);
        assert_eq!(minimal_len((1 << 28) - 1), 4);
        assert_eq!(minimal_len(1 << 28), 5);
        assert_eq!(minimal_len(u32::MAX), 5);
    }

    #[test]
    fn fixed_size_encodes_6_in_five_bytes_that_still_decode_to_6() {
        let mut w = BitWriter::new();
        write_uleb128_fixed(&mut w, 6, 5).expect("6 fits in five bytes");
        let bytes = w.finish().expect("byte-aligned");

        assert_eq!(bytes, [0x86, 0x80, 0x80, 0x80, 0x00]);
        assert_eq!(bytes.len(), 5, "non-minimal, and legal on the wire");
        assert_eq!(
            BitCursor::new(&bytes).read_uleb128().expect("decodes"),
            6,
            "the decoder accepts a non-minimal encoding — PARSE-04's caveat"
        );
    }

    #[test]
    fn fixed_size_at_one_byte_matches_the_minimal_encoding() {
        let mut w = BitWriter::new();
        write_uleb128_fixed(&mut w, 6, 1).expect("6 fits in one byte");
        assert_eq!(w.finish().expect("byte-aligned"), [0x06]);
    }

    #[test]
    fn fixed_size_refuses_a_size_outside_the_reference_cap() {
        let mut w = BitWriter::new();
        let size = u8::try_from(MAX_LEB128_SIZE)
            .unwrap_or(u8::MAX)
            .saturating_add(1);
        let err = w_err(&mut w, 6, size);
        assert_eq!(err, ErrorKind::Leb128SizeInvalid { size });

        let mut w = BitWriter::new();
        assert_eq!(
            w_err(&mut w, 6, 0),
            ErrorKind::Leb128SizeInvalid { size: 0 }
        );
    }

    #[test]
    fn fixed_size_refuses_a_value_that_does_not_fit_the_requested_size() {
        let mut w = BitWriter::new();
        // 128 needs two groups; one byte cannot carry it without lying.
        assert_eq!(w_err(&mut w, 128, 1), ErrorKind::Leb128ValueTooLarge);
    }

    fn w_err(w: &mut BitWriter, value: u32, size: u8) -> ErrorKind {
        write_uleb128_fixed(w, value, size)
            .expect_err("expected a rejection")
            .kind()
            .clone()
    }
}
