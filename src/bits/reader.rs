//! [`BitCursor`] — the read half. Mirrors `writer.rs` method for method, in the
//! same order (D-10).

use crate::bits::leb128;
use crate::error::{Error, ErrorKind, Location, Result};

/// A bounded, bit-addressed cursor over borrowed input.
///
/// Reads are **MSB-first within each byte**, which is what the reference does
/// and therefore what the wire format is.
#[derive(Debug, Clone)]
pub struct BitCursor<'a> {
    data: &'a [u8],
    /// Index of the byte currently being read from. Invariantly `<= data.len()`.
    byte_pos: usize,
    /// Bits already consumed from `data[byte_pos]`. Invariantly `0..=7`.
    bit_off: u32,
}

impl<'a> BitCursor<'a> {
    /// A cursor positioned at bit 0 of `data`.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_off: 0,
        }
    }

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadUnsignedLiteral
    /// Read `bits` (0..=64) as an unsigned value, MSB-first.
    pub fn read_unsigned(&mut self, bits: u32) -> Result<u64> {
        let start = self.byte_position();
        if bits > 64 {
            return Err(Error::new(
                ErrorKind::UnsupportedWidth {
                    bits: u8::try_from(bits).unwrap_or(u8::MAX),
                },
                Location::InputOffset(start),
            ));
        }
        if u64::from(bits) > self.bits_remaining() {
            return Err(Error::new(
                ErrorKind::UnexpectedEndOfInput,
                Location::InputOffset(start),
            ));
        }
        let mut acc: u64 = 0;
        for _ in 0..bits {
            // `wrapping_shl(1)` cannot lose a significant bit here: `acc` starts
            // at zero and the loop runs at most 64 times, so the bit shifted out
            // of the top is always one this loop put there below bit 64.
            acc = acc.wrapping_shl(1) | u64::from(self.read_bit()?);
        }
        Ok(acc)
    }

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadBoolean
    /// Read one bit as a boolean.
    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_unsigned(1)? == 1)
    }

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadULeb128
    /// Read a uleb128, capped at 8 bytes and at `u32::MAX`.
    pub fn read_uleb128(&mut self) -> Result<u32> {
        leb128::read_uleb128(self)
    }

    /// Bits left between the cursor and the end of the input.
    #[must_use]
    pub fn bits_remaining(&self) -> u64 {
        let whole = self.data.len().saturating_sub(self.byte_pos);
        let bits = u64::try_from(whole).unwrap_or(u64::MAX).saturating_mul(8);
        bits.saturating_sub(u64::from(self.bit_off))
    }

    /// `true` exactly when the bit position is a multiple of 8.
    #[must_use]
    pub const fn is_byte_aligned(&self) -> bool {
        self.bit_off == 0
    }

    /// The byte the cursor is currently inside, counted from the start of the
    /// input. This is the number a `Location::InputOffset` carries.
    #[must_use]
    pub fn byte_position(&self) -> u64 {
        u64::try_from(self.byte_pos).unwrap_or(u64::MAX)
    }

    /// Consume one bit, MSB-first within the current byte.
    ///
    /// Private, and the single place a bit is taken from the input: every
    /// public read is expressed in terms of this one, so there is exactly one
    /// piece of code that can get the bit order wrong.
    fn read_bit(&mut self) -> Result<bool> {
        let byte = *self.data.get(self.byte_pos).ok_or_else(|| {
            Error::new(
                ErrorKind::UnexpectedEndOfInput,
                Location::InputOffset(self.byte_position()),
            )
        })?;
        // `bit_off` is invariantly 0..=7, so neither the subtraction nor the
        // shift can be out of range. Saying so with `saturating_`/`checked_` is
        // how the lint set requires it to be said out loud.
        let shift = 7_u32.saturating_sub(self.bit_off);
        let bit = (byte.checked_shr(shift).unwrap_or(0) & 1) == 1;
        self.advance_one_bit();
        Ok(bit)
    }

    /// Move one bit forward, carrying into the next byte at the boundary.
    ///
    /// `saturating_add` on `byte_pos` can never actually saturate — the cursor
    /// is bounded by `data.len()` — and if it somehow did, every subsequent
    /// bounds check would fail closed rather than wrap around to byte 0.
    fn advance_one_bit(&mut self) {
        let next = self.bit_off.saturating_add(1);
        if next >= 8 {
            self.bit_off = 0;
            self.byte_pos = self.byte_pos.saturating_add(1);
        } else {
            self.bit_off = next;
        }
    }
}
