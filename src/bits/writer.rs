//! [`BitWriter`] — the write half. Mirrors `reader.rs` method for method, in the
//! same order (D-10).

use crate::bits::leb128;
use crate::bits::reader::MAX_STRING_BYTES;
use crate::error::{Error, ErrorKind, Location, Result};

/// A bit-addressed writer over an owned buffer.
///
/// Writes are **MSB-first within each byte**, mirroring [`crate::bits::BitCursor`].
#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    out: Vec<u8>,
    /// The byte under construction. Only the top `bit_off` bits are meaningful.
    partial: u8,
    /// Bits already placed in `partial`. Invariantly `0..=7`.
    bit_off: u32,
}

impl BitWriter {
    /// An empty writer, byte-aligned at offset 0.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            out: Vec::new(),
            partial: 0,
            bit_off: 0,
        }
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteUnsignedLiteral
    /// Write the low `bits` (0..=64) of `value`, MSB-first.
    pub fn write_unsigned(&mut self, value: u64, bits: u32) -> Result<()> {
        if bits > 64 {
            return Err(Error::new(
                ErrorKind::UnsupportedWidth {
                    bits: u8::try_from(bits).unwrap_or(u8::MAX),
                },
                Location::OutputOffset(self.output_offset()),
            ));
        }
        // `checked_shl` returns `None` at `bits == 64`, where every `u64` fits
        // and there is nothing to check. Written as a nested `if` rather than a
        // let-chain: let-chains are Rust 1.88, and GUARD-12 pins 1.85.
        if let Some(limit) = 1_u64.checked_shl(bits) {
            if value >= limit {
                return Err(Error::new(
                    ErrorKind::ValueExceedsWidth {
                        bits: u8::try_from(bits).unwrap_or(u8::MAX),
                    },
                    Location::OutputOffset(self.output_offset()),
                ));
            }
        }
        for i in (0..bits).rev() {
            self.write_bit((value.checked_shr(i).unwrap_or(0) & 1) == 1);
        }
        Ok(())
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteSigned8 / WriteSigned9 / WriteSigned16
    /// Write `value` as `bits` (1..=64) of two's complement, MSB-first.
    ///
    /// Always big-endian on the wire — `audio_roll_distance`,
    /// `integrated_loudness`, `digital_peak` and `default_mix_gain` are all
    /// signed-16 big-endian fields.
    pub fn write_signed(&mut self, value: i64, bits: u32) -> Result<()> {
        if bits == 0 || bits > 64 {
            return Err(Error::new(
                ErrorKind::UnsupportedWidth {
                    bits: u8::try_from(bits).unwrap_or(u8::MAX),
                },
                Location::OutputOffset(self.output_offset()),
            ));
        }
        // Range check first: silently truncating a value that does not fit its
        // field is exactly the kind of divergence a byte-diff finds and nothing
        // else does.
        let pad = 64_u32.saturating_sub(bits);
        let widened = value.wrapping_shl(pad).wrapping_shr(pad);
        if widened != value {
            return Err(Error::new(
                ErrorKind::ValueExceedsWidth {
                    bits: u8::try_from(bits).unwrap_or(u8::MAX),
                },
                Location::OutputOffset(self.output_offset()),
            ));
        }
        // Mask to the field width, then hand the two's-complement pattern to
        // the unsigned path — one place decides bit order.
        let mask = u64::MAX.wrapping_shr(pad);
        self.write_unsigned((value as u64) & mask, bits)
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteBoolean
    /// Write one bit.
    pub fn write_bool(&mut self, value: bool) -> Result<()> {
        self.write_unsigned(u64::from(value), 1)
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteString
    /// Write `payload` followed by a NUL terminator.
    ///
    /// **Asymmetric with `BitCursor::read_string` on purpose.** The reader
    /// returns the terminator, because what was on the wire is what the caller
    /// needs to see and the field's byte count includes it. The writer takes the
    /// payload *without* a terminator and appends one, because a caller must not
    /// be able to emit an unterminated string at all. Round-tripping a read
    /// value therefore means dropping its last byte.
    pub fn write_string(&mut self, payload: &[u8]) -> Result<()> {
        if payload.len().saturating_add(1) > MAX_STRING_BYTES {
            return Err(Error::new(
                ErrorKind::StringTooLong,
                Location::OutputOffset(self.output_offset()),
            ));
        }
        if payload.contains(&0) {
            // An interior NUL reads back as a shorter string than was written.
            // Refusing it here is the only place that asymmetry can be stopped.
            return Err(Error::new(
                ErrorKind::StringHasInteriorNul,
                Location::OutputOffset(self.output_offset()),
            ));
        }
        self.write_bytes(payload)?;
        self.write_unsigned(0, 8)
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteUint8Span
    /// Write raw bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        if self.is_byte_aligned() {
            // The common case, and the only one an OBU payload ever hits.
            self.out.extend_from_slice(bytes);
            return Ok(());
        }
        for byte in bytes {
            self.write_unsigned(u64::from(*byte), 8)?;
        }
        Ok(())
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteUleb128
    /// Write a uleb128 in **minimal form** — the only form this crate emits
    /// through a public entry point (D-03).
    pub fn write_uleb128_minimal(&mut self, value: u32) -> Result<()> {
        leb128::write_uleb128_minimal(self, value)
    }

    /// Complete bytes emitted so far. A pending partial byte is not counted.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.out.len()
    }

    /// `true` exactly when the bit position is a multiple of 8.
    #[must_use]
    pub const fn is_byte_aligned(&self) -> bool {
        self.bit_off == 0
    }

    /// The output byte offset a `Location::OutputOffset` carries.
    ///
    /// `pub(super)` so `leb128.rs` reports at the same position this file does;
    /// a write error's offset must not depend on which module noticed it.
    pub(super) fn output_offset(&self) -> u64 {
        u64::try_from(self.out.len()).unwrap_or(u64::MAX)
    }

    /// Place one bit, MSB-first within the byte under construction.
    ///
    /// Private, and the single place a bit reaches the output — the mirror of
    /// `BitCursor::read_bit`, so bit order is decided once per direction.
    fn write_bit(&mut self, bit: bool) {
        // `bit_off` is invariantly 0..=7, so the subtraction and the shift are
        // both in range; `saturating_`/`checked_` is the lint set's way of
        // saying so.
        let shift = 7_u32.saturating_sub(self.bit_off);
        if bit {
            self.partial |= 1_u8.checked_shl(shift).unwrap_or(0);
        }
        let next = self.bit_off.saturating_add(1);
        if next >= 8 {
            self.out.push(self.partial);
            self.partial = 0;
            self.bit_off = 0;
        } else {
            self.bit_off = next;
        }
    }

    /// Consume the writer and yield its bytes.
    ///
    /// Errors with [`ErrorKind::NotByteAligned`] if a partial byte is pending:
    /// silently zero-padding would produce a file that is one byte longer than
    /// the caller believes, which is the kind of divergence a byte-diff finds
    /// and nothing else does.
    pub fn finish(self) -> Result<Vec<u8>> {
        if self.bit_off != 0 {
            return Err(Error::new(
                ErrorKind::NotByteAligned,
                Location::OutputOffset(self.output_offset()),
            ));
        }
        Ok(self.out)
    }
}
