//! [`BitWriter`] — the write half. Mirrors `reader.rs` method for method, in the
//! same order (D-10).

use crate::bits::leb128;
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
        Ok(())
    }

    // ref: iamf-tools@v2.1.0 iamf/common/write_bit_buffer.h WriteBitBuffer::WriteBoolean
    /// Write one bit.
    pub fn write_bool(&mut self, value: bool) -> Result<()> {
        self.write_unsigned(u64::from(value), 1)
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
    fn output_offset(&self) -> u64 {
        u64::try_from(self.out.len()).unwrap_or(u64::MAX)
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
