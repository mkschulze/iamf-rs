//! D-25 hand-decoded OBU header vectors.
//!
//! Every expected byte string below was worked out **by hand** from the bit
//! packing and the uleb128 grouping before the writer that produces it existed,
//! and each one is keyed to the offset in
//! `tests/fixtures/reference/test_000003.iamf` it reproduces. The vendored file
//! is then read with `xxd` as the second, independent derivation D-25 asks for.
//! Capturing the encoder's own output as the expectation would prove only that
//! we match a blob we never understood.
//!
//! The header layout under test, MSB-first in byte 0:
//!
//! ```text
//! [obu_type:5][obu_redundant_copy:1][obu_trimming_status_flag:1][obu_extension_flag:1]
//! ```
//!
//! followed by `obu_size` as a uleb128 counting **every byte after itself** —
//! the trim fields, the extension header and the payload — and excluding byte 0
//! and the size bytes themselves.

use hex_literal::hex;
use iamf::bits::{BitCursor, BitWriter};
use iamf::error::ErrorKind;
use iamf::obu::{ObuHeader, ObuType, Trimming, TypeSpecific};

/// `write_obu` into a fresh writer, returning the finished bytes.
///
/// The helpers below are ordinary functions rather than `#[test]` bodies, and
/// GUARD-04's `allow-expect-in-tests` carve-out does not reach them, so they
/// are written without a panic path at all.
fn emit(header: &ObuHeader, payload: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    let written = iamf::obu::write_obu(&mut w, header, payload);
    assert!(written.is_ok(), "the header is legal: {written:?}");
    w.finish().unwrap_or_default()
}

/// The error kind `write_obu` rejects this header with, or `None` if it did not.
fn emit_err(header: &ObuHeader, payload: &[u8]) -> Option<ErrorKind> {
    let mut w = BitWriter::new();
    iamf::obu::write_obu(&mut w, header, payload)
        .err()
        .map(|e| e.kind().clone())
}

// ---------------------------------------------------------------------------
// Offset 0x00 — the IA Sequence Header, and the whole header in eight bytes
// ---------------------------------------------------------------------------

/// `f8` is `11111` (type 31) followed by three clear flags. `06` is the
/// payload length, because a header with no trim and no extension writes
/// nothing between `obu_size` and the payload.
#[test]
fn offset_0x00_ia_sequence_header_writes_as_f8_06_then_six_payload_bytes() {
    let header = ObuHeader::new(ObuType::IaSequenceHeader);
    let payload = hex!("69 61 6d 66 00 00");

    assert_eq!(
        emit(&header, &payload).as_slice(),
        hex!("f8 06 69 61 6d 66 00 00").as_slice()
    );
}

#[test]
fn offset_0x00_reads_back_as_an_untrimmed_unextended_type_31() {
    let bytes = hex!("f8 06 69 61 6d 66 00 00");
    let mut r = BitCursor::new(&bytes);

    let (header, obu_size) = iamf::obu::read_obu_header(&mut r).expect("well-formed header");

    assert_eq!(header.obu_type, ObuType::IaSequenceHeader);
    assert!(!header.obu_redundant_copy);
    assert_eq!(header.type_specific, TypeSpecific::Reserved);
    assert!(!header.trimming_status_flag());
    assert!(!header.extension_flag());
    assert_eq!(header.extension, None);
    assert_eq!(obu_size, 6);
    assert_eq!(
        r.read_uint8_span(6).expect("six payload bytes"),
        hex!("69 61 6d 66 00 00").as_slice()
    );
    assert_eq!(r.bytes_remaining(), 0, "the OBU ends exactly at byte 8");
}

// ---------------------------------------------------------------------------
// The zero-length payload — the Temporal Delimiter case
// ---------------------------------------------------------------------------

/// Type 4 is `00100`; with three clear flags that is `0x20`. An OBU with no
/// trim, no extension and no payload writes `obu_size` as the single byte
/// `0x00` — the shortest legal OBU in the format is two bytes.
#[test]
fn a_temporal_delimiter_writes_obu_size_as_a_single_zero_byte() {
    let header = ObuHeader::new(ObuType::TemporalDelimiter);

    assert_eq!(emit(&header, &[]).as_slice(), hex!("20 00").as_slice());
}

#[test]
fn a_two_byte_temporal_delimiter_reads_back_with_obu_size_zero() {
    let bytes = hex!("20 00");
    let mut r = BitCursor::new(&bytes);

    let (header, obu_size) = iamf::obu::read_obu_header(&mut r).expect("well-formed header");

    assert_eq!(header.obu_type, ObuType::TemporalDelimiter);
    assert_eq!(obu_size, 0);
    assert_eq!(r.bytes_remaining(), 0);
}

// ---------------------------------------------------------------------------
// Offset 0x78 — an untrimmed Audio Frame, and the two-byte obu_size
// ---------------------------------------------------------------------------

/// Type 6 is `00110`; with three clear flags that is `0x30`. 512 is
/// `0b100_0000000`, so its minimal uleb128 is `0x80 0x04`: seven zero bits with
/// the continuation bit set, then `4`.
#[test]
fn offset_0x78_an_untrimmed_audio_frame_writes_as_30_80_04() {
    let header =
        ObuHeader::new(ObuType::AudioFrameId0).with_type_specific(TypeSpecific::Trimming(None));
    let payload = [0_u8; 512];

    let bytes = emit(&header, &payload);

    assert_eq!(bytes.get(..3), Some(hex!("30 80 04").as_slice()));
    assert_eq!(
        bytes.len(),
        515,
        "1 header byte + 2 size bytes + 512 payload"
    );
}

// ---------------------------------------------------------------------------
// Offset 0x7D32 — the trimmed Audio Frame. This is the vector that proves the
// `obu_size` origin: 514 is two trim bytes PLUS 512 payload bytes.
// ---------------------------------------------------------------------------

/// `0x32` is `00110` (type 6), redundant 0, **trimming 1**, extension 0.
/// `82 04` is 514 = 2 + 512, which is the whole of OBU-02: `obu_size` counts
/// the after-size fields and excludes byte 0 and the size bytes.
#[test]
fn offset_0x7d32_a_trimmed_audio_frame_writes_as_32_82_04_40_00() {
    let header = ObuHeader::new(ObuType::AudioFrameId0).with_type_specific(TypeSpecific::Trimming(
        Some(Trimming {
            at_end: 64,
            at_start: 0,
        }),
    ));
    let payload = [0_u8; 512];

    let bytes = emit(&header, &payload);

    assert_eq!(bytes.get(..5), Some(hex!("32 82 04 40 00").as_slice()));
    assert_eq!(
        bytes.len(),
        517,
        "1 header byte + 2 size bytes + 2 trim bytes + 512 payload"
    );
}

/// The only test that can catch a swapped write order.
///
/// `WriteFieldsAfterObuSize` writes `num_samples_to_trim_at_end` **first** and
/// `num_samples_to_trim_at_start` second. Whenever the two values are equal —
/// which is almost always, and is true of every frame in every vendored
/// fixture — the two orders are byte-identical, so no golden file in the corpus
/// can distinguish them. This one can.
#[test]
fn the_trim_fields_are_written_end_first_then_start() {
    let header = ObuHeader::new(ObuType::AudioFrameId0).with_type_specific(TypeSpecific::Trimming(
        Some(Trimming {
            at_end: 64,
            at_start: 7,
        }),
    ));

    let bytes = emit(&header, &[0_u8; 512]);

    assert_eq!(
        bytes.get(3..5),
        Some(hex!("40 07").as_slice()),
        "END (64 = 0x40) is written before START (7 = 0x07), not after it"
    );
}

#[test]
fn a_trimmed_audio_frame_reads_its_trim_values_back_in_the_same_order() {
    let bytes = hex!("32 82 04 40 07");
    let mut r = BitCursor::new(&bytes);

    let (header, obu_size) = iamf::obu::read_obu_header(&mut r).expect("well-formed header");

    assert_eq!(header.obu_type, ObuType::AudioFrameId0);
    assert_eq!(obu_size, 514);
    assert_eq!(
        header.type_specific,
        TypeSpecific::Trimming(Some(Trimming {
            at_end: 64,
            at_start: 7,
        }))
    );
    assert!(header.trimming_status_flag());
}

// ---------------------------------------------------------------------------
// Negative cases
// ---------------------------------------------------------------------------

/// An OBU never starts mid-byte. BITS-05: the format has no padding mechanism,
/// so an unaligned start is a caller bug that must be a typed error rather than
/// a file the reference silently mis-frames.
#[test]
fn an_obu_refuses_to_start_at_an_unaligned_writer_position() {
    let mut w = BitWriter::new();
    w.write_bool(true).expect("one bit fits");

    let header = ObuHeader::new(ObuType::IaSequenceHeader);
    let err = iamf::obu::write_obu(&mut w, &header, &[])
        .expect_err("an unaligned start is refused")
        .kind()
        .clone();

    assert_eq!(err, ErrorKind::NotByteAligned);
}

/// `kEntireObuSizeMaxTwoMegabytes`, from `iamf/obu/types.h`.
const ENTIRE_OBU_SIZE_MAX: usize = 1 << 21;

/// The largest payload that still fits, for a header with no after-size fields:
/// the reference's derived bound is `kEntireObuSizeMaxTwoMegabytes - 1 -
/// size_of_obu_size`, and a size that large needs three uleb128 bytes.
const LARGEST_PAYLOAD: usize = ENTIRE_OBU_SIZE_MAX - 1 - 3;

#[test]
fn a_payload_past_the_two_megabyte_ceiling_is_refused() {
    let header = ObuHeader::new(ObuType::IaSequenceHeader);
    let payload = vec![0_u8; ENTIRE_OBU_SIZE_MAX];

    assert_eq!(emit_err(&header, &payload), Some(ErrorKind::ObuTooLarge));
}

#[test]
fn the_largest_payload_under_the_ceiling_is_accepted() {
    let header = ObuHeader::new(ObuType::IaSequenceHeader);
    let payload = vec![0_u8; LARGEST_PAYLOAD];

    let bytes = emit(&header, &payload);

    assert_eq!(bytes.len(), LARGEST_PAYLOAD.saturating_add(4));
}
