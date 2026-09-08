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

// ---------------------------------------------------------------------------
// Signed widths — the 9-bit case is why a byte-oriented approach cannot work
// ---------------------------------------------------------------------------

/// `1_1111_1111` in nine bits is −1. Packed MSB-first that is `0xFF` followed
/// by one more set bit, so the fixture byte is `ff 80`.
#[test]
fn signed_9_all_ones_is_minus_one() {
    let mut r = BitCursor::new(&hex!("ff 80"));
    assert_eq!(r.read_signed(9).expect("nine bits"), -1);
}

/// `0_1111_1111` in nine bits is 255 — the same eight set bits, one place
/// lower, with the sign bit clear. The boundary the sign extension must not
/// cross.
#[test]
fn signed_9_without_the_sign_bit_is_255() {
    let mut r = BitCursor::new(&hex!("7f 80"));
    assert_eq!(r.read_signed(9).expect("nine bits"), 255);
}

/// The write side reproduces the same nine bits. Seven zero bits pad the
/// buffer to a byte boundary so `finish()` can succeed.
#[test]
fn write_signed_minus_one_in_9_bits_reproduces_the_pattern() {
    let mut w = BitWriter::new();
    w.write_signed(-1, 9).expect("−1 fits in nine bits");
    for _ in 0..7 {
        w.write_bool(false).expect("padding");
    }
    assert_eq!(w.finish().expect("byte-aligned"), hex!("ff 80"));
}

/// A value that does not fit the requested signed width is refused rather than
/// silently truncated: 256 needs ten bits.
#[test]
fn write_signed_refuses_a_value_wider_than_its_field() {
    let mut w = BitWriter::new();
    let err = w.write_signed(256, 9).expect_err("256 needs ten bits");
    assert_eq!(*err.kind(), ErrorKind::ValueExceedsWidth { bits: 9 });
}

/// `0x12` of `test_000003.iamf`: `audio_roll_distance`, signed-16 big-endian,
/// which IAMF pins to 0 for LPCM.
#[test]
fn offset_0x12_audio_roll_distance_zero_is_signed_16() {
    let mut r = BitCursor::new(&hex!("00 00"));
    assert_eq!(r.read_signed(16).expect("signed 16"), 0);
}

/// `0x74`: `integrated_loudness`. `0xCA5B` = 51803; 51803 − 65536 = −13733.
#[test]
fn offset_0x74_integrated_loudness_is_minus_13733() {
    let mut r = BitCursor::new(&hex!("ca 5b"));
    assert_eq!(r.read_signed(16).expect("signed 16"), -13733);

    let mut w = BitWriter::new();
    w.write_signed(-13733, 16).expect("fits");
    assert_eq!(w.finish().expect("byte-aligned"), hex!("ca 5b"));
}

/// `0x76`: `digital_peak`. `0xCDB1` = 52657; 52657 − 65536 = −12879.
#[test]
fn offset_0x76_digital_peak_is_minus_12879() {
    let mut r = BitCursor::new(&hex!("cd b1"));
    assert_eq!(r.read_signed(16).expect("signed 16"), -12879);

    let mut w = BitWriter::new();
    w.write_signed(-12879, 16).expect("fits");
    assert_eq!(w.finish().expect("byte-aligned"), hex!("cd b1"));
}

// ---------------------------------------------------------------------------
// Strings — byte strings, terminator included, capped at 128 including the NUL
// ---------------------------------------------------------------------------

/// `0x2C`: `annotations_language[0]` = `"en-us\0"`. Six bytes read, six bytes
/// returned, and the cursor has advanced by six.
#[test]
fn offset_0x2c_annotations_language_includes_its_nul() {
    let mut r = BitCursor::new(&hex!("65 6e 2d 75 73 00 ff"));
    let s = r.read_string().expect("terminated");

    assert_eq!(s, b"en-us\0");
    assert_eq!(r.byte_position(), 6);
    assert_eq!(r.bytes_remaining(), 1);
}

/// 128 bytes with no terminator is `StringNotTerminated`, reported at the byte
/// the field started at — and reported *before* a 129th byte is demanded, so
/// running out of input cannot masquerade as this error.
#[test]
fn read_string_rejects_128_bytes_without_a_terminator() {
    let data = [b'a'; 128];
    let mut r = BitCursor::new(&data);

    let err = r.read_string().expect_err("no terminator within the cap");
    assert_eq!(*err.kind(), ErrorKind::StringNotTerminated);
    assert_eq!(err.at(), Location::InputOffset(0));
}

/// 127 bytes and a terminator is exactly the cap, and it succeeds.
#[test]
fn read_string_accepts_127_bytes_and_a_terminator_at_the_cap() {
    let mut data = [b'a'; 128];
    data[127] = 0;
    let mut r = BitCursor::new(&data);

    let s = r.read_string().expect("terminated at the cap");
    assert_eq!(s.len(), 128);
    assert_eq!(s.last(), Some(&0));
}

/// The writer appends the terminator, so `"en-us"` in gives `"en-us\0"` out —
/// byte-identical with offset `0x2C`.
#[test]
fn write_string_appends_the_terminator() {
    let mut w = BitWriter::new();
    w.write_string(b"en-us").expect("fits");
    assert_eq!(w.finish().expect("byte-aligned"), hex!("65 6e 2d 75 73 00"));
}

/// A 128-byte payload would need 129 bytes with its terminator, one over the
/// cap.
#[test]
fn write_string_rejects_a_payload_that_would_exceed_the_cap() {
    let mut w = BitWriter::new();
    let err = w
        .write_string(&[b'a'; 128])
        .expect_err("128 + NUL exceeds 128");
    assert_eq!(*err.kind(), ErrorKind::StringTooLong);
}

/// An interior NUL would be read back as a shorter string than was written —
/// an asymmetry the writer refuses rather than propagates.
#[test]
fn write_string_rejects_an_interior_nul() {
    let mut w = BitWriter::new();
    let err = w.write_string(b"en\0us").expect_err("interior NUL");
    assert_eq!(*err.kind(), ErrorKind::StringHasInteriorNul);
}

// ---------------------------------------------------------------------------
// Spans and the bounds-check-before-allocate rule (ASVS V5, T-01-12)
// ---------------------------------------------------------------------------

/// A zero-length span is an empty slice and advances nothing. Degenerate, and
/// exactly the case a count-driven loop hits on an empty collection.
#[test]
fn read_uint8_span_zero_is_an_empty_slice_and_advances_nothing() {
    let mut r = BitCursor::new(&hex!("f8 06"));
    assert_eq!(r.read_uint8_span(0).expect("empty span"), &[] as &[u8]);
    assert_eq!(r.byte_position(), 0);
    assert_eq!(r.bytes_remaining(), 2);
}

/// A length longer than the input is refused. The length field is
/// attacker-controlled, so the bound is checked before anything is reserved.
#[test]
fn read_uint8_span_bounds_checks_its_length_before_allocating() {
    let mut r = BitCursor::new(&hex!("f8 06"));
    let err = r
        .read_uint8_span(usize::MAX)
        .expect_err("no allocation on a hostile length");
    assert_eq!(*err.kind(), ErrorKind::UnexpectedEndOfInput);
    assert_eq!(err.at(), Location::InputOffset(0));
    assert_eq!(r.byte_position(), 0, "a failed span must not advance");
}

/// A span borrows, it does not copy.
#[test]
fn read_uint8_span_borrows_the_requested_bytes() {
    let data = hex!("69 61 6d 66");
    let mut r = BitCursor::new(&data);
    assert_eq!(r.read_uint8_span(4).expect("four bytes"), b"iamf");
    assert_eq!(r.bytes_remaining(), 0);
}

/// Writing no bytes changes nothing, including alignment.
#[test]
fn write_bytes_empty_advances_nothing() {
    let mut w = BitWriter::new();
    w.write_bytes(&[]).expect("empty write");
    assert_eq!(w.len_bytes(), 0);
    assert!(w.is_byte_aligned());
}

// ---------------------------------------------------------------------------
// Sub-readers — Pattern 3's bounded payload view
// ---------------------------------------------------------------------------

/// A sub-reader ending exactly on the parent's end leaves the parent empty,
/// not in error. This is the ordinary case at the last OBU in a file.
#[test]
fn sub_reader_ending_on_the_parent_end_leaves_the_parent_empty() {
    let data = hex!("69 61 6d 66");
    let mut parent = BitCursor::new(&data);
    let mut child = parent.sub_reader(4).expect("exactly the remainder");

    assert_eq!(parent.bits_remaining(), 0);
    assert_eq!(child.read_unsigned(32).expect("ia_code"), 0x6961_6d66);
}

/// A zero-length sub-reader is legal and immediately exhausted — an OBU with
/// an empty payload, which the Temporal Delimiter is.
#[test]
fn sub_reader_zero_yields_a_reader_that_immediately_ends() {
    let data = hex!("f8 06");
    let mut parent = BitCursor::new(&data);
    let mut child = parent.sub_reader(0).expect("empty payload");

    assert_eq!(child.bits_remaining(), 0);
    let err = child.read_unsigned(1).expect_err("nothing to read");
    assert_eq!(*err.kind(), ErrorKind::UnexpectedEndOfInput);
    assert_eq!(parent.bytes_remaining(), 2, "the parent is untouched");
}

/// A sub-reader longer than the parent is an error, never a truncated view.
/// A silently shortened payload is how an under-read becomes a wrong file.
#[test]
fn sub_reader_longer_than_the_parent_is_an_error() {
    let data = hex!("f8 06");
    let mut parent = BitCursor::new(&data);

    let err = parent.sub_reader(3).expect_err("only two bytes remain");
    assert_eq!(*err.kind(), ErrorKind::UnexpectedEndOfInput);
    assert_eq!(parent.bytes_remaining(), 2, "a failed sub_reader is inert");
}

/// OBU payloads are byte-aligned by construction and the reference pads
/// nothing, so a sub-reader taken mid-byte is a bug in the caller.
#[test]
fn sub_reader_refuses_an_unaligned_parent() {
    let data = hex!("f8 06");
    let mut parent = BitCursor::new(&data);
    parent.read_unsigned(5).expect("obu_type");

    let err = parent.sub_reader(1).expect_err("mid-byte");
    assert_eq!(*err.kind(), ErrorKind::NotByteAligned);
}

// ---------------------------------------------------------------------------
// uleb128 caps — kMaxLeb128Size = 8, and the value must fit u32
// ---------------------------------------------------------------------------

/// Nine bytes each asking for another is `Leb128TooLong`, reported at the first
/// byte of the field. The ninth byte is never even read.
#[test]
fn read_uleb128_rejects_a_ninth_continuation_byte() {
    let data = [0x80_u8; 9];
    let mut r = BitCursor::new(&data);

    let err = r.read_uleb128().expect_err("nine continuation bytes");
    assert_eq!(*err.kind(), ErrorKind::Leb128TooLong);
    assert_eq!(err.at(), Location::InputOffset(0));
}

/// Eight legal bytes whose accumulated value exceeds `u32::MAX` is
/// `Leb128ValueTooLarge` — a different failure from the length cap, and it must
/// be rejected before the narrowing rather than wrapped by it.
#[test]
fn read_uleb128_rejects_a_value_above_u32_max() {
    // 2^35, which needs six groups and does not fit a u32.
    let data = hex!("80 80 80 80 80 02");
    let mut r = BitCursor::new(&data);

    let err = r.read_uleb128().expect_err("2^35 does not fit u32");
    assert_eq!(*err.kind(), ErrorKind::Leb128ValueTooLarge);
    assert_eq!(err.at(), Location::InputOffset(0));
}

/// `u32::MAX` is five bytes and round-trips; one below the cap is still five.
#[test]
fn uleb128_round_trips_at_the_u32_boundary() {
    let mut w = BitWriter::new();
    w.write_uleb128_minimal(u32::MAX).expect("encodes");
    let bytes = w.finish().expect("byte-aligned");

    assert_eq!(bytes, hex!("ff ff ff ff 0f"));
    assert_eq!(
        BitCursor::new(&bytes).read_uleb128().expect("decodes"),
        u32::MAX
    );
}

/// Zero is one byte, and the writer never appends a trailing continuation byte.
#[test]
fn uleb128_zero_is_exactly_one_byte() {
    let mut w = BitWriter::new();
    w.write_uleb128_minimal(0).expect("encodes");
    assert_eq!(w.finish().expect("byte-aligned"), hex!("00"));
}
