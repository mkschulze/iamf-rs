//! [`BitCursor`] — the read half. Mirrors `writer.rs` method for method, in the
//! same order (D-10).
//!
//! # One rule, every count in the format (ASVS V5, threat T-01-12)
//!
//! **Never allocate or reserve from a parsed length before capping it against
//! [`BitCursor::bytes_remaining`].** Every length, count and width this crate
//! acts on originates in attacker-shaped bytes: `obu_size`,
//! `extension_header_size`, `info_type_size`, `num_substreams`, `num_layouts`,
//! and every count-driven loop in every later plan. A length field that reaches
//! a `Vec::with_capacity` before a bounds check is the OOM, and it is the first
//! thing a fuzzer finds.
//!
//! [`BitCursor::read_uint8_span`] is where that rule is first enforced, and it
//! is stated here rather than there because it governs code that does not exist
//! yet.

use crate::bits::leb128;
use crate::error::{Error, ErrorKind, Location, Result};

/// The reference's cap on a string field: 128 bytes **including** the NUL
/// terminator. An unterminated 128-byte run is an error, not a truncation.
// ref: iamf-tools@v2.1.0 iamf/obu/types.h kIamfMaxStringSize
pub(crate) const MAX_STRING_BYTES: usize = 128;

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

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadSigned8 / ReadSigned9 / ReadSigned16
    /// Read `bits` (1..=64) as a two's-complement signed value, MSB-first.
    ///
    /// The reference exposes this as three separate functions because C++ makes
    /// you pick a return type; one width-taking function covers all three. The
    /// 9-bit case is the one that rules out every byte-oriented approach, and
    /// the return type is `i64` for the same reason a 9-bit field does not fit
    /// an `i8` — callers narrow, this does not guess.
    pub fn read_signed(&mut self, bits: u32) -> Result<i64> {
        let start = self.byte_position();
        if bits == 0 || bits > 64 {
            return Err(Error::new(
                ErrorKind::UnsupportedWidth {
                    bits: u8::try_from(bits).unwrap_or(u8::MAX),
                },
                Location::InputOffset(start),
            ));
        }
        let raw = self.read_unsigned(bits)?;
        // Sign-extend by shifting the field up to the top of an i64 and back
        // down arithmetically. Both shift amounts are `64 - bits`, which is
        // 0..=63 for a width in 1..=64, so neither can be out of range.
        let pad = 64_u32.saturating_sub(bits);
        let widened = raw.wrapping_shl(pad) as i64;
        Ok(widened.wrapping_shr(pad))
    }

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadBoolean
    /// Read one bit as a boolean.
    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_unsigned(1)? == 1)
    }

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadString
    /// Read a NUL-terminated byte string, terminator included, capped at 128
    /// bytes **including** that terminator.
    ///
    /// The value is a **byte string**, never UTF-8-validated. The wire carries
    /// bytes; validating them here would make this parser stricter on read than
    /// the reference is, and rejecting a file the reference accepts is as much
    /// a conformance failure as accepting one it rejects.
    pub fn read_string(&mut self) -> Result<Vec<u8>> {
        let start = self.byte_position();
        // Grows one byte at a time. Nothing is reserved from a parsed length
        // here because there is no length field — the terminator is the length,
        // and the cap is the only thing standing between an unterminated field
        // and the whole input.
        let mut out = Vec::new();
        loop {
            if out.len() >= MAX_STRING_BYTES {
                // Checked before the next read, so exhausting the input can
                // never be reported as this error, nor this as that one.
                return Err(Error::new(
                    ErrorKind::StringNotTerminated,
                    Location::InputOffset(start),
                ));
            }
            let byte = u8::try_from(self.read_unsigned(8)? & 0xff).unwrap_or(0);
            out.push(byte);
            if byte == 0 {
                return Ok(out);
            }
        }
    }

    // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadUint8Span
    /// Borrow the next `len` bytes without copying them.
    pub fn read_uint8_span(&mut self, len: usize) -> Result<&'a [u8]> {
        let start = self.byte_position();
        if !self.is_byte_aligned() {
            // A borrowed slice has no sub-byte start, so this is a caller bug
            // rather than bad input. Say which.
            return Err(Error::new(
                ErrorKind::NotByteAligned,
                Location::InputOffset(start),
            ));
        }
        // The bound is checked BEFORE anything is reserved or sliced: `len`
        // came off the wire and is attacker-controlled. See the module comment.
        if len > self.bytes_remaining() {
            return Err(Error::new(
                ErrorKind::UnexpectedEndOfInput,
                Location::InputOffset(start),
            ));
        }
        let end = self.byte_pos.checked_add(len).ok_or_else(|| {
            Error::new(
                ErrorKind::UnexpectedEndOfInput,
                Location::InputOffset(start),
            )
        })?;
        let span = self.data.get(self.byte_pos..end).ok_or_else(|| {
            Error::new(
                ErrorKind::UnexpectedEndOfInput,
                Location::InputOffset(start),
            )
        })?;
        self.byte_pos = end;
        Ok(span)
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

    /// Whole bytes left between the cursor and the end of the input. A pending
    /// partial byte does not count as a byte.
    #[must_use]
    pub fn bytes_remaining(&self) -> usize {
        usize::try_from(self.bits_remaining().checked_div(8).unwrap_or(0)).unwrap_or(usize::MAX)
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

    /// A cursor over exactly the next `len_bytes` of this one, with the parent
    /// advanced past them (Pattern 3).
    ///
    /// This is how every OBU payload gets a bounded view: the child cannot read
    /// past the payload into the next OBU, and when the child is drained the
    /// parent is exactly where the next OBU begins. An under-read is otherwise
    /// silent.
    pub fn sub_reader(&mut self, len_bytes: usize) -> Result<BitCursor<'a>> {
        // `read_uint8_span` already owns the alignment check, the
        // bounds-check-before-reserve rule and the advance. Expressing the
        // sub-reader in terms of it means there is one implementation of that
        // rule, not two that can drift.
        let span = self.read_uint8_span(len_bytes)?;
        Ok(BitCursor::new(span))
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
