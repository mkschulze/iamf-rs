//! D-25 hand-decoded descriptor vectors.
//!
//! Every expected byte string below was written from two independent
//! derivations that agree: `tests/fixtures/reference/test_000003.textproto`
//! (the *published configuration* the reference encoder was given) and the
//! field-by-field hand-decode of `tests/fixtures/reference/test_000003.iamf`
//! recorded in `01-RESEARCH.md` under "Verified byte layouts". The offsets in
//! each test name are absolute offsets into that file.
//!
//! The hand-decode is the primary source and the vendored `.iamf` is the second
//! assert: several tests slice the real file and compare against it, so a
//! transcription slip in the table cannot pass silently.
//!
//! The prologue is **120 bytes**, `0x00..0x78` — not the 118 `PROJECT.md`
//! states. The first Audio Frame OBU begins at `0x78`.

use hex_literal::hex;
use iamf::bits::{BitCursor, BitWriter};
use iamf::error::ErrorKind;
use iamf::obu::{
    CodecConfig, DecoderConfig, IaSequenceHeader, LpcmDecoderConfig, Obu, ObuHeader, ObuType,
    SampleFormatFlags, read_codec_config, read_ia_sequence_header, read_obu_with,
    write_codec_config, write_ia_sequence_header, write_obu_with,
};

/// The vendored reference file the whole suite is measured against.
const TEST_000003: &[u8] = include_bytes!("fixtures/reference/test_000003.iamf");

/// Serialise one whole OBU (header + payload) and hand back its bytes.
fn obu_bytes<T>(
    obu: &Obu<T>,
    write_payload: impl FnOnce(&mut BitWriter, &T) -> iamf::Result<()>,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    write_obu_with(&mut w, obu, write_payload).expect("the vector serialises");
    w.finish().expect("the writer ends byte-aligned")
}

/// The `test_000003` IA Sequence Header, as its textproto publishes it:
/// `primary_profile: PROFILE_VERSION_SIMPLE`, `additional_profile:
/// PROFILE_VERSION_SIMPLE`.
fn published_sequence_header() -> Obu<IaSequenceHeader> {
    Obu::new(
        ObuHeader::new(ObuType::IaSequenceHeader),
        IaSequenceHeader::new(0, 0),
    )
}

/// The `test_000003` Codec Config, as its textproto publishes it.
fn published_codec_config() -> Obu<CodecConfig> {
    Obu::new(
        ObuHeader::new(ObuType::CodecConfig),
        CodecConfig::lpcm(
            200,
            128,
            LpcmDecoderConfig {
                sample_format_flags: SampleFormatFlags::LittleEndian,
                sample_size: 16,
                sample_rate: 16000,
            },
        ),
    )
}

// ---------------------------------------------------------------------------
// IA Sequence Header — offsets 0x00..0x08
// ---------------------------------------------------------------------------

#[test]
fn ia_sequence_header_reproduces_offsets_0x00_through_0x07() {
    // f8 = type(5)=31, redundant=0, trimming=0, extension=0.
    // 06 = obu_size: 4 bytes of ia_code + 2 profile bytes.
    // 69 61 6d 66 = "iamf" = 0x69616d66.
    assert_eq!(
        obu_bytes(&published_sequence_header(), write_ia_sequence_header),
        hex!("f8 06 69 61 6d 66 00 00"),
    );
}

#[test]
fn ia_sequence_header_leaves_the_writer_at_absolute_offset_8() {
    // PITFALLS.md §8's absolute-offset assertion: the Codec Config OBU begins
    // at byte 8, not at byte 6 or 7.
    assert_eq!(
        obu_bytes(&published_sequence_header(), write_ia_sequence_header).len(),
        8,
    );
}

#[test]
fn ia_sequence_header_matches_the_vendored_reference_file_at_0x00() {
    let expected = TEST_000003.get(0x00..0x08).expect("the file is longer");
    assert_eq!(
        obu_bytes(&published_sequence_header(), write_ia_sequence_header),
        expected,
    );
}

#[test]
fn ia_sequence_header_round_trips_through_read() {
    let bytes = obu_bytes(&published_sequence_header(), write_ia_sequence_header);
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_ia_sequence_header).expect("the vector parses");

    assert_eq!(parsed.payload.ia_code, 0x6961_6d66);
    assert_eq!(parsed.payload.primary_profile, 0);
    assert_eq!(parsed.payload.additional_profile, 0);
    assert!(
        parsed.trailing.is_empty(),
        "the payload parser consumed the whole payload"
    );
    assert_eq!(obu_bytes(&parsed, write_ia_sequence_header), bytes);
}

#[test]
fn writing_additional_profile_below_primary_is_a_typed_error() {
    // libiamf@v1.1.0 `_valid_profile` returns `primary <= additional`; it
    // rejects the WHOLE sequence otherwise. iamf-tools does not check the
    // field at all, so such a file passes CONF-06 and fails CONF-05.
    let obu = Obu::new(
        ObuHeader::new(ObuType::IaSequenceHeader),
        IaSequenceHeader::new(1, 0),
    );
    let mut w = BitWriter::new();
    let err = write_obu_with(&mut w, &obu, write_ia_sequence_header)
        .expect_err("additional_profile below primary_profile is refused");

    assert_eq!(err.kind(), &ErrorKind::AdditionalProfileBelowPrimary);
}

#[test]
fn equal_profiles_are_always_accepted() {
    for profile in 0_u8..3 {
        let obu = Obu::new(
            ObuHeader::new(ObuType::IaSequenceHeader),
            IaSequenceHeader::new(profile, profile),
        );
        let mut w = BitWriter::new();
        write_obu_with(&mut w, &obu, write_ia_sequence_header)
            .expect("equal profiles satisfy `primary <= additional`");
    }
}

#[test]
fn a_wrong_ia_code_is_a_finding_and_not_a_parse_failure() {
    // `f8 06` then "iamg" instead of "iamf".
    let bytes = hex!("f8 06 69 61 6d 67 00 00");
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_ia_sequence_header).expect("the parse SUCCEEDS");

    assert_eq!(parsed.payload.ia_code, 0x6961_6d67);
    let findings = parsed.payload.validate();
    assert!(
        findings.iter().any(|f| f.message.contains("ia_code")),
        "validate() names the field: {findings:?}"
    );
}

// ---------------------------------------------------------------------------
// Codec Config — offsets 0x08..0x1A
// ---------------------------------------------------------------------------

#[test]
fn codec_config_reproduces_offsets_0x08_through_0x19() {
    // 00 = type(5)=0 Codec Config, all flags clear.
    // 10 = obu_size 16 = 2 (id) + 4 (codec_id) + 2 (nspf) + 2 (roll) + 6 (lpcm)
    assert_eq!(
        obu_bytes(&published_codec_config(), write_codec_config),
        hex!("00 10 c8 01 69 70 63 6d 80 01 00 00 01 10 00 00 3e 80"),
    );
}

#[test]
fn codec_config_matches_the_vendored_reference_file_at_0x08() {
    let expected = TEST_000003.get(0x08..0x1a).expect("the file is longer");
    assert_eq!(obu_bytes(&published_codec_config(), write_codec_config), expected);
}

#[test]
fn audio_roll_distance_is_signed_16_big_endian() {
    // -1 is `ff ff`, and -2 is `ff fe` — two bytes, most significant first.
    let mut obu = published_codec_config();
    obu.payload.audio_roll_distance = -2;
    let bytes = obu_bytes(&obu, write_codec_config);
    assert_eq!(bytes.get(10..12), Some(hex!("ff fe").as_slice()));

    obu.payload.audio_roll_distance = 1;
    let bytes = obu_bytes(&obu, write_codec_config);
    assert_eq!(bytes.get(10..12), Some(hex!("00 01").as_slice()));
}

#[test]
fn sample_rate_is_unsigned_32_big_endian() {
    // 16000 = 0x00003e80, most significant byte first.
    let bytes = obu_bytes(&published_codec_config(), write_codec_config);
    assert_eq!(bytes.get(14..18), Some(hex!("00 00 3e 80").as_slice()));

    let mut obu = published_codec_config();
    if let DecoderConfig::Lpcm(ref mut lpcm) = obu.payload.decoder_config {
        lpcm.sample_rate = 48000;
    }
    let bytes = obu_bytes(&obu, write_codec_config);
    assert_eq!(bytes.get(14..18), Some(hex!("00 00 bb 80").as_slice()));
}

#[test]
fn sample_format_flags_big_endian_serialises_as_zero() {
    // DESC-03. The sense is the OPPOSITE of a WAV-shaped assumption, and
    // libiamf@main's own comment above the line that acts on it is inverted.
    // Asserted by name, in both directions, because nothing else in the suite
    // can catch a swap: both values are legal and both decode without error.
    assert_eq!(SampleFormatFlags::BigEndian.value(), 0x00);

    let mut obu = published_codec_config();
    if let DecoderConfig::Lpcm(ref mut lpcm) = obu.payload.decoder_config {
        lpcm.sample_format_flags = SampleFormatFlags::BigEndian;
    }
    let bytes = obu_bytes(&obu, write_codec_config);
    assert_eq!(bytes.get(12), Some(&0x00));
}

#[test]
fn sample_format_flags_little_endian_serialises_as_one() {
    assert_eq!(SampleFormatFlags::LittleEndian.value(), 0x01);
    let bytes = obu_bytes(&published_codec_config(), write_codec_config);
    assert_eq!(bytes.get(12), Some(&0x01));
}

#[test]
fn decoder_config_has_no_length_field_and_is_the_payload_remainder() {
    // Two extra bytes inside obu_size that the 6-byte LPCM config does not
    // claim end up in `trailing` and re-serialise verbatim. There is no
    // decoder_config length field to disagree with them.
    let bytes = hex!("00 12 c8 01 69 70 63 6d 80 01 00 00 01 10 00 00 3e 80 aa bb");
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_codec_config).expect("the vector parses");

    assert_eq!(parsed.payload.trailing, vec![0xaa, 0xbb]);
    assert!(
        parsed.trailing.is_empty(),
        "the payload-level remainder wins; the OBU-level drain stays empty"
    );
    assert_eq!(obu_bytes(&parsed, write_codec_config), bytes);
}

#[test]
fn the_encoder_path_derives_audio_roll_distance_zero_for_lpcm() {
    assert_eq!(published_codec_config().payload.audio_roll_distance, 0);
}

#[test]
fn a_wire_audio_roll_distance_of_two_is_preserved_and_reported() {
    // D-06: the reader stores what was on the wire and validate() names the
    // mismatch. Normalising to 0 would make the foreign file un-reproducible
    // and hide the defect that produced it.
    let bytes = hex!("00 10 c8 01 69 70 63 6d 80 01 00 02 01 10 00 00 3e 80");
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_codec_config).expect("the parse SUCCEEDS");

    assert_eq!(parsed.payload.audio_roll_distance, 2);
    let findings = parsed.payload.validate();
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("audio_roll_distance")),
        "validate() names the field: {findings:?}"
    );
    assert_eq!(obu_bytes(&parsed, write_codec_config), bytes);
}

#[test]
fn codec_config_validate_reports_all_six_out_of_range_conditions() {
    // Every one of these is rejected by iamf-tools@v2.1.0 and checked by
    // NEITHER libiamf revision. `sample_size = 8` in particular falls through
    // to a 16-bit little-endian reader with no error at all.
    let mut cfg = CodecConfig::lpcm(
        200,
        0,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::Reserved(0x07),
            sample_size: 8,
            sample_rate: 12345,
        },
    );
    cfg.codec_id = *b"zzzz";
    cfg.audio_roll_distance = -1;

    let messages: Vec<String> = cfg.validate().into_iter().map(|f| f.message).collect();
    let joined = messages.join("\n");

    for field in [
        "codec_id",
        "num_samples_per_frame",
        "sample_format_flags",
        "sample_size",
        "sample_rate",
        "audio_roll_distance",
    ] {
        assert!(joined.contains(field), "no finding named `{field}`:\n{joined}");
    }
    assert!(
        messages.len() >= 6,
        "validate() returns ALL findings in one run (D-09): {messages:?}"
    );
}

#[test]
fn the_first_26_bytes_of_test_000003_are_reproduced_from_its_published_configuration() {
    let mut w = BitWriter::new();
    write_obu_with(&mut w, &published_sequence_header(), write_ia_sequence_header)
        .expect("sequence header");
    write_obu_with(&mut w, &published_codec_config(), write_codec_config).expect("codec config");
    let produced = w.finish().expect("byte-aligned");

    assert_eq!(produced.len(), 0x1a);
    assert_eq!(produced.as_slice(), TEST_000003.get(0x00..0x1a).expect("longer"));
}
