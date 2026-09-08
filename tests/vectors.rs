//! BITS-06 / D-25 — hand-computed bit-primitive vectors.
//!
//! **These are the primary derivation, and they exist before any OBU type
//! does.** Every expected byte below was decoded by hand from
//! `libiamf@v1.1.0 tests/test_000003.iamf` and cross-checked against
//! `tests/test_000003.textproto`; none of it was captured from this crate's own
//! output. D-25 rejects capture-first goldens explicitly: a test asserting you
//! match a blob you never understood is a regression test, not a specification.
//!
//! Each test is named after the offset in `test_000003.iamf` it reproduces,
//! where one exists. The vendored copy of that file is at
//! `tests/fixtures/reference/test_000003.iamf`.
//!
//! The differential oracle in `tests/bits_oracle.rs` is the *second*,
//! independent assert (D-01). Two derivations that must agree.

use hex_literal::hex;
use iamf::bits::{BitCursor, BitWriter};
use iamf::{ErrorKind, Location};

/// `0x00` of `test_000003.iamf`: an IA Sequence Header with all three flags
/// clear. `obu_type(5) = 11111` = 31, then `redundant_copy`,
/// `trimming_status_flag` and `extension_flag`, all zero. `11111000` = `0xF8`.
#[test]
fn offset_0x00_ia_sequence_header_header_byte_writes_as_f8() {
    let mut w = BitWriter::new();
    w.write_unsigned(31, 5).expect("obu_type fits in 5 bits");
    w.write_bool(false).expect("redundant_copy");
    w.write_bool(false).expect("trimming_status_flag");
    w.write_bool(false).expect("extension_flag");

    assert_eq!(w.finish().expect("byte-aligned"), hex!("f8"));
}

/// The same byte read back: 31, then three clear flags, then nothing left.
#[test]
fn offset_0x00_ia_sequence_header_header_byte_reads_back() {
    let mut r = BitCursor::new(&hex!("f8"));

    assert_eq!(r.read_unsigned(5).expect("obu_type"), 31);
    assert!(!r.read_bool().expect("redundant_copy"));
    assert!(!r.read_bool().expect("trimming_status_flag"));
    assert!(!r.read_bool().expect("extension_flag"));
    assert_eq!(r.bits_remaining(), 0);
}

/// `0x01`: the IA Sequence Header's `obu_size`, 6, as a one-byte uleb128.
#[test]
fn offset_0x01_obu_size_6_is_one_byte() {
    let mut w = BitWriter::new();
    w.write_uleb128_minimal(6).expect("6 encodes");
    assert_eq!(w.finish().expect("byte-aligned"), hex!("06"));

    let mut r = BitCursor::new(&hex!("06"));
    assert_eq!(r.read_uleb128().expect("decodes"), 6);
    assert_eq!(r.bits_remaining(), 0);
}

/// `0x0A`: the Codec Config's `codec_config_id`, 200, as a two-byte uleb128.
/// 200 = `0b1100_1000`; low seven bits `100_1000` = `0x48` with the
/// continuation bit set gives `0xC8`, and the remaining `1` gives `0x01`.
#[test]
fn offset_0x0a_codec_config_id_200_is_two_bytes() {
    let mut w = BitWriter::new();
    w.write_uleb128_minimal(200).expect("200 encodes");
    assert_eq!(w.finish().expect("byte-aligned"), hex!("c8 01"));

    let mut r = BitCursor::new(&hex!("c8 01"));
    assert_eq!(r.read_uleb128().expect("decodes"), 200);
    assert_eq!(r.bits_remaining(), 0);
}

/// Alignment tracks the partial byte: true before any write, false after five
/// bits, true again once the three flags complete the byte.
#[test]
fn writer_byte_alignment_tracks_the_partial_byte() {
    let mut w = BitWriter::new();
    assert!(w.is_byte_aligned(), "a fresh writer is aligned");

    w.write_unsigned(31, 5).expect("obu_type");
    assert!(!w.is_byte_aligned(), "five bits in, mid-byte");
    assert_eq!(w.len_bytes(), 0, "no complete byte has been emitted yet");

    w.write_bool(false).expect("flag");
    w.write_bool(false).expect("flag");
    w.write_bool(false).expect("flag");
    assert!(w.is_byte_aligned(), "eight bits in, aligned again");
    assert_eq!(w.len_bytes(), 1);
}

/// Reading past the end is a typed error carrying the offset the read *began*
/// at — not the offset it ran out at. That is the number a fuzz report needs.
#[test]
fn reading_past_the_end_reports_the_offset_the_read_began_at() {
    let mut r = BitCursor::new(&hex!("f8 06"));
    r.read_unsigned(8).expect("byte 0");

    let err = r.read_unsigned(16).expect_err("only 8 bits remain");
    assert_eq!(*err.kind(), ErrorKind::UnexpectedEndOfInput);
    assert_eq!(err.at(), Location::InputOffset(1));
}
