//! D-25 hand-decoded vectors for the three time-varying OBU types.
//!
//! Every expected byte string here was written from two independent
//! derivations that agree: the field-by-field hand-decode of
//! `tests/fixtures/reference/test_000003.iamf` recorded in `01-RESEARCH.md`
//! under "Verified byte layouts", and the vendored `.iamf` itself, which
//! several tests slice directly so a transcription slip in the table cannot
//! pass silently.
//!
//! The offsets in the test names are absolute offsets into that file. The
//! descriptor prologue ends at `0x78`, which is where the first Audio Frame
//! OBU begins.

use hex_literal::hex;
use iamf::bits::{BitCursor, BitWriter};
use iamf::error::ErrorKind;
use iamf::obu::{
    AudioFrame, Obu, ObuHeader, ObuType, Trimming, obu_type_for, read_audio_frame,
    read_obu_with_header, substream_id_for, write_audio_frame, write_obu, write_obu_with_header,
};

/// The vendored reference file the whole suite is measured against.
const TEST_000003: &[u8] = include_bytes!("fixtures/reference/test_000003.iamf");

/// Offset of the first Audio Frame OBU: `30 80 04` then 512 payload bytes.
const FIRST_FRAME: usize = 0x78;
/// Offset of the last, trimmed Audio Frame OBU: `32 82 04 40 00` then 512
/// payload bytes.
const TRIMMED_FRAME: usize = 0x7D32;

/// Serialise one whole OBU and hand back its bytes.
///
/// The helpers here are ordinary functions rather than `#[test]` bodies, and
/// GUARD-04's `allow-expect-in-tests` carve-out does not reach them, so they
/// are written without a panic path at all — the convention
/// `tests/obu_header.rs` and `tests/descriptors.rs` established.
fn frame_bytes(obu: &Obu<AudioFrame>) -> Vec<u8> {
    let mut w = BitWriter::new();
    let written = write_obu_with_header(&mut w, obu, write_audio_frame);
    assert!(written.is_ok(), "the vector serialises: {written:?}");
    w.finish().unwrap_or_default()
}

/// The bytes of `test_000003.iamf` from `at`, for `len` bytes.
fn slice_of_reference(at: usize, len: usize) -> Vec<u8> {
    let end = at.saturating_add(len);
    TEST_000003.get(at..end).unwrap_or_default().to_vec()
}

// ---------------------------------------------------------------------------
// The Audio Frame OBU (TIME-01)
// ---------------------------------------------------------------------------

#[test]
fn an_untrimmed_audio_frame_for_substream_0_reproduces_offset_0x78() {
    // `30` = type 6 (`00110`) in the top five bits, redundant-copy 0,
    // trimming 0, extension 0. `80 04` = obu_size 512 = 128 samples x 2 ch
    // x 2 B.
    let payload = slice_of_reference(FIRST_FRAME.saturating_add(3), 512);
    let obu = AudioFrame::new(0, payload).into_obu(None);

    let bytes = frame_bytes(&obu);

    assert_eq!(
        bytes.get(0..3).unwrap_or_default(),
        &hex!("30 80 04"),
        "byte 0 plus the two-byte obu_size of the first Audio Frame"
    );
    assert_eq!(
        bytes,
        slice_of_reference(FIRST_FRAME, 515),
        "the whole OBU reproduces offsets 0x78..0x27B of test_000003.iamf"
    );
}

#[test]
fn the_trimmed_final_audio_frame_reproduces_offset_0x7d32() {
    // `32` = type 6 with bit 6 set. `82 04` = obu_size 514 = 2 trim bytes +
    // 512 payload bytes. `40` = num_samples_to_trim_at_end = 64, written
    // FIRST; `00` = num_samples_to_trim_at_start = 0, written second.
    let payload = slice_of_reference(TRIMMED_FRAME.saturating_add(5), 512);
    let obu = AudioFrame::new(0, payload).into_obu(Some(Trimming {
        at_end: 64,
        at_start: 0,
    }));

    let bytes = frame_bytes(&obu);

    assert_eq!(
        bytes.get(0..5).unwrap_or_default(),
        &hex!("32 82 04 40 00"),
        "END trim (64) precedes START trim (0) — OBU-05"
    );
    assert_eq!(
        bytes,
        slice_of_reference(TRIMMED_FRAME, 517),
        "the whole OBU reproduces offsets 0x7D32..0x7F37 of test_000003.iamf"
    );
}

#[test]
fn a_substream_id_of_17_is_implicit_in_obu_type_23_and_18_is_explicit_in_type_5() {
    assert_eq!(obu_type_for(17), ObuType::AudioFrameId17);
    assert_eq!(obu_type_for(17).value(), 23);
    assert_eq!(obu_type_for(18), ObuType::AudioFrame);
    assert_eq!(obu_type_for(18).value(), 5);

    let id17 = frame_bytes(&AudioFrame::new(17, vec![0xAA, 0xBB]).into_obu(None));
    assert_eq!(
        id17,
        hex!("b8 02 aa bb"),
        "type 23 carries the id in the header and NO id field in the payload"
    );

    let id18 = frame_bytes(&AudioFrame::new(18, vec![0xAA, 0xBB]).into_obu(None));
    assert_eq!(
        id18,
        hex!("28 03 12 aa bb"),
        "type 5 carries an explicit uleb128 id (0x12 = 18) at the head of the payload"
    );
}

#[test]
fn obu_type_for_and_substream_id_for_are_inverses_across_the_boundary() {
    for id in 0..=17_u32 {
        assert_eq!(
            substream_id_for(obu_type_for(id)),
            Some(id),
            "an id of {id} round-trips through the implicit encoding"
        );
    }
    assert_eq!(
        substream_id_for(ObuType::AudioFrame),
        None,
        "type 5 carries no implicit id — the reader must read one"
    );
    assert_eq!(substream_id_for(ObuType::TemporalDelimiter), None);
}

#[test]
fn reading_a_type_6_frame_derives_its_substream_id_and_consumes_no_id_bytes() {
    let bytes = slice_of_reference(FIRST_FRAME, 515);
    let mut r = BitCursor::new(&bytes);

    let obu = match read_obu_with_header(&mut r, read_audio_frame) {
        Ok(obu) => obu,
        Err(error) => panic!("the first Audio Frame parses: {error}"),
    };

    assert_eq!(obu.header.obu_type, ObuType::AudioFrameId0);
    assert_eq!(obu.payload.substream_id, 0, "derived from `obu_type - 6`");
    assert_eq!(
        obu.payload.payload.len(),
        512,
        "the whole remainder is the frame — no id bytes were consumed"
    );
    assert!(obu.trailing.is_empty(), "the frame claims the whole payload");
    assert_eq!(frame_bytes(&obu), bytes, "and it re-serialises unchanged");
}

#[test]
fn a_type_5_frame_carrying_a_small_id_round_trips_as_type_5() {
    // Legal but non-canonical (D-06): the reader stores what the wire said
    // rather than rewriting the frame as `6 + id`, which is what keeps
    // `serialize(parse(bytes)) == bytes` true for foreign files.
    let bytes = hex!("28 03 07 aa bb");
    let mut r = BitCursor::new(&bytes);

    let obu = match read_obu_with_header(&mut r, read_audio_frame) {
        Ok(obu) => obu,
        Err(error) => panic!("a type-5 frame with a small id parses: {error}"),
    };

    assert_eq!(obu.header.obu_type, ObuType::AudioFrame);
    assert_eq!(obu.payload.substream_id, 7);
    assert_eq!(obu.payload.payload, vec![0xAA, 0xBB]);
    assert_eq!(
        frame_bytes(&obu),
        bytes,
        "it is NOT normalised to type 13 on re-serialisation"
    );

    let findings = obu.payload.validate(obu.header.obu_type);
    assert_eq!(
        findings.len(),
        1,
        "validate() reports it could have been implicit: {findings:?}"
    );
}

#[test]
fn obu_redundant_copy_on_an_audio_frame_is_a_typed_error() {
    for obu_type in [
        ObuType::AudioFrame,
        ObuType::AudioFrameId0,
        ObuType::AudioFrameId17,
    ] {
        let header = ObuHeader::new(obu_type).with_redundant_copy(true);
        let mut w = BitWriter::new();
        let written = write_obu(&mut w, &header, &[]);

        assert_eq!(
            written.err().map(|e| e.kind().clone()),
            Some(ErrorKind::RedundantCopyNotAllowed),
            "obu_redundant_copy is forbidden on {obu_type:?}"
        );
    }
}

#[test]
fn a_substream_id_the_element_does_not_declare_is_a_finding_not_a_parse_failure() {
    // T-01-36. `libiamf` accepts it, so rejecting on read would make this
    // crate stricter than the reference.
    let frame = AudioFrame::new(3, vec![0x00]);

    assert!(frame.validate_against_element(&[0, 1, 2, 3]).is_empty());
    assert_eq!(
        frame.validate_against_element(&[0, 1]).len(),
        1,
        "a frame for an undeclared substream is reported, not refused"
    );
}
