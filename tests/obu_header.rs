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

#[test]
fn generic_obu_parsing_rejects_an_oversized_claim_before_payload_access() {
    // obu_size = 2 MiB, encoded in four bytes. The complete input is tiny on
    // purpose: the ceiling violation must win over the later truncation.
    let bytes = hex!("f8 80 80 80 01");
    let mut r = BitCursor::new(&bytes);

    let err = iamf::obu::read_obu_with(&mut r, |_payload| Ok(()))
        .expect_err("generic parsing applies the same ceiling as the boundary walker");

    assert_eq!(err.kind(), &ErrorKind::ObuTooLarge);
}

#[test]
fn generic_obu_parsing_accepts_a_legal_non_minimal_size_field() {
    // obu_size 1 encoded as the legal fixed-width `81 00`, followed by one
    // payload byte. Measuring those two bytes is required for the ceiling.
    let bytes = hex!("f8 81 00 aa");
    let mut r = BitCursor::new(&bytes);

    let obu = iamf::obu::read_obu_with(&mut r, |payload| payload.read_unsigned(8))
        .expect("non-minimal ULEB128 remains accepted");

    assert_eq!(obu.payload, 0xaa);
    assert_eq!(r.byte_position(), 4);
}

// ---------------------------------------------------------------------------
// The extension header (OBU-06)
// ---------------------------------------------------------------------------

/// `f9` is `11111` (type 31) with the extension bit set. The after-size fields
/// are then `02 aa bb` — a uleb128 length and that many bytes — so `obu_size`
/// is `3 + 6 = 9`, which is again OBU-02's rule.
#[test]
fn an_extension_header_writes_its_size_then_its_bytes_and_obu_size_counts_both() {
    let header = ObuHeader::new(ObuType::IaSequenceHeader).with_extension(hex!("aa bb").to_vec());
    let payload = hex!("69 61 6d 66 00 00");

    assert_eq!(
        emit(&header, &payload).as_slice(),
        hex!("f9 09 02 aa bb 69 61 6d 66 00 00").as_slice()
    );
}

#[test]
fn an_extension_header_reads_back_verbatim() {
    let bytes = hex!("f9 09 02 aa bb 69 61 6d 66 00 00");
    let mut r = BitCursor::new(&bytes);

    let (header, obu_size) = iamf::obu::read_obu_header(&mut r).expect("well-formed header");

    assert!(header.extension_flag());
    assert_eq!(header.extension.as_deref(), Some(hex!("aa bb").as_slice()));
    assert_eq!(obu_size, 9);
}

/// The after-size fields are written trim-first, extension-second — the order
/// `WriteFieldsAfterObuSize` uses.
#[test]
fn the_extension_header_is_written_after_both_trim_fields() {
    let header = ObuHeader::new(ObuType::AudioFrameId0)
        .with_type_specific(TypeSpecific::Trimming(Some(Trimming {
            at_end: 64,
            at_start: 7,
        })))
        .with_extension(hex!("aa").to_vec());

    let bytes = emit(&header, &[]);

    // 0x33 = type 6, redundant 0, trimming 1, extension 1. obu_size = 4.
    assert_eq!(
        bytes.as_slice(),
        hex!("33 04 40 07 01 aa").as_slice(),
        "end trim, start trim, extension size, extension bytes"
    );
}

/// T-01-19: `extension_header_size` is attacker-controlled and drives an
/// allocation, so it is capped against the remaining input before anything is
/// reserved.
#[test]
fn an_extension_length_past_the_end_of_the_input_is_refused_without_allocating() {
    // Claims a 200-byte extension inside a five-byte buffer.
    let bytes = hex!("f9 09 c8 01 00");
    let mut r = BitCursor::new(&bytes);

    let err = iamf::obu::read_obu_header(&mut r)
        .expect_err("the length is past the end of the input")
        .kind()
        .clone();

    assert_eq!(err, ErrorKind::UnexpectedEndOfInput);
}

// ---------------------------------------------------------------------------
// The two legality rules (OBU-03, OBU-04)
// ---------------------------------------------------------------------------

/// The rule this whole plan exists to get right. `IsTrimmingStatusFlagAllowed`
/// returns true only for Audio Frames, and `libiamf@v1.1.0` reads the trim
/// fields for *any* type whose bit 6 is set — so a trimming flag on a
/// descriptor shifts its payload two bytes with no error from either side.
#[test]
fn the_trimming_flag_is_refused_on_every_non_audio_frame_type() {
    for obu_type in [
        ObuType::CodecConfig,
        ObuType::AudioElement,
        ObuType::MixPresentation,
        ObuType::ParameterBlock,
        ObuType::TemporalDelimiter,
        ObuType::IaSequenceHeader,
        ObuType::Reserved(24),
    ] {
        let header =
            ObuHeader::new(obu_type).with_type_specific(TypeSpecific::Trimming(Some(Trimming {
                at_end: 1,
                at_start: 0,
            })));

        assert_eq!(
            emit_err(&header, &[]),
            Some(ErrorKind::TrimmingFlagNotAllowed),
            "bit 6 must not be settable on type {}",
            obu_type.value()
        );
    }
}

#[test]
fn the_trimming_flag_is_accepted_on_every_audio_frame_type() {
    for raw in 5_u8..=23 {
        let header = ObuHeader::new(ObuType::from_value(raw)).with_type_specific(
            TypeSpecific::Trimming(Some(Trimming {
                at_end: 64,
                at_start: 0,
            })),
        );

        assert_eq!(emit_err(&header, &[]), None, "type {raw} is an Audio Frame");
    }
}

#[test]
fn a_redundant_copy_is_refused_on_parameter_blocks_delimiters_and_audio_frames() {
    for raw in 3_u8..=23 {
        let obu_type = ObuType::from_value(raw);
        let type_specific = if obu_type.is_audio_frame() {
            TypeSpecific::Trimming(None)
        } else {
            TypeSpecific::Reserved
        };
        let header = ObuHeader::new(obu_type)
            .with_type_specific(type_specific)
            .with_redundant_copy(true);

        assert_eq!(
            emit_err(&header, &[]),
            Some(ErrorKind::RedundantCopyNotAllowed),
            "obu_redundant_copy must be refused on type {raw}"
        );
    }
}

/// On a descriptor it is legal, and it round-trips. Byte 0 for a Codec Config
/// with the redundant-copy bit set is `00000` + `1` + `0` + `0` = `0x04`.
#[test]
fn a_redundant_copy_round_trips_on_a_descriptor() {
    let header = ObuHeader::new(ObuType::CodecConfig).with_redundant_copy(true);

    let bytes = emit(&header, &[]);
    assert_eq!(bytes.as_slice(), hex!("04 00").as_slice());

    let mut r = BitCursor::new(&bytes);
    let (read_back, _) = iamf::obu::read_obu_header(&mut r).expect("well-formed header");
    assert!(read_back.obu_redundant_copy);
    assert_eq!(read_back, header);
}

// ---------------------------------------------------------------------------
// OBU-07 — the central `trailing` drain
// ---------------------------------------------------------------------------

/// A payload parser that stops short leaves the remainder in `trailing`, and
/// `trailing`'s length is exactly `obu_size` minus what it consumed.
#[test]
fn a_payload_parser_that_under_reads_leaves_the_remainder_in_trailing() {
    let bytes = hex!("f8 0c 00 01 02 03 04 05 06 07 08 09 0a 0b");
    let mut r = BitCursor::new(&bytes);

    let obu = iamf::obu::read_obu_with(&mut r, |payload| {
        payload.read_uint8_span(10).map(<[u8]>::to_vec)
    })
    .expect("well-formed OBU");

    assert_eq!(obu.payload.len(), 10);
    assert_eq!(obu.trailing.as_slice(), hex!("0a 0b").as_slice());
    assert_eq!(obu.trailing.len(), 2, "obu_size 12 minus 10 consumed");
    assert_eq!(r.bytes_remaining(), 0, "the parent is at the next OBU");
}

#[test]
fn a_payload_parser_that_consumes_everything_leaves_trailing_empty() {
    let bytes = hex!("f8 0c 00 01 02 03 04 05 06 07 08 09 0a 0b");
    let mut r = BitCursor::new(&bytes);

    let obu = iamf::obu::read_obu_with(&mut r, |payload| {
        payload.read_uint8_span(12).map(<[u8]>::to_vec)
    })
    .expect("well-formed OBU");

    assert!(obu.trailing.is_empty());
}

#[test]
fn a_freshly_constructed_obu_has_empty_trailing_and_serialises_without_it() {
    let obu = iamf::obu::Obu::new(
        ObuHeader::new(ObuType::IaSequenceHeader),
        hex!("69 61 6d 66 00 00").to_vec(),
    );

    assert!(obu.trailing.is_empty());

    let mut w = BitWriter::new();
    iamf::obu::write_obu_with(&mut w, &obu, |scratch, payload| {
        scratch.write_bytes(payload)
    })
    .expect("the header is legal");

    assert_eq!(
        w.finish().expect("byte-aligned").as_slice(),
        hex!("f8 06 69 61 6d 66 00 00").as_slice()
    );
}

/// `trailing` is appended **last**, after the type-specific payload, so a
/// read-then-write of an OBU whose payload we only partly understand is
/// byte-identical.
#[test]
fn an_under_read_obu_re_serialises_to_the_bytes_it_was_read_from() {
    let original = hex!("f8 0c 00 01 02 03 04 05 06 07 08 09 0a 0b");
    let mut r = BitCursor::new(&original);
    let obu = iamf::obu::read_obu_with(&mut r, |payload| {
        payload.read_uint8_span(10).map(<[u8]>::to_vec)
    })
    .expect("well-formed OBU");

    let mut w = BitWriter::new();
    iamf::obu::write_obu_with(&mut w, &obu, |scratch, payload| {
        scratch.write_bytes(payload)
    })
    .expect("the header is legal");

    assert_eq!(
        w.finish().expect("byte-aligned").as_slice(),
        original.as_slice()
    );
}
