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
use iamf::model::layout::{ExpandedLoudspeakerLayout, LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, AudioElementType, ChannelAudioLayerConfig, CodecConfig, DecoderConfig,
    IaSequenceHeader, LpcmDecoderConfig, Obu, ObuHeader, ObuType, SampleFormatFlags,
    ScalableChannelLayoutConfig, read_audio_element, read_codec_config, read_ia_sequence_header,
    read_obu_with, write_audio_element, write_codec_config, write_ia_sequence_header,
    write_obu_with,
};

/// The vendored reference file the whole suite is measured against.
const TEST_000003: &[u8] = include_bytes!("fixtures/reference/test_000003.iamf");

/// Serialise one whole OBU (header + payload) and hand back its bytes.
///
/// The helpers here are ordinary functions rather than `#[test]` bodies, and
/// GUARD-04's `allow-expect-in-tests` carve-out does not reach them, so they
/// are written without a panic path at all — the same convention
/// `tests/obu_header.rs` established.
fn obu_bytes<T>(
    obu: &Obu<T>,
    write_payload: impl FnOnce(&mut BitWriter, &T) -> iamf::Result<()>,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    let written = write_obu_with(&mut w, obu, write_payload);
    assert!(written.is_ok(), "the vector serialises: {written:?}");
    w.finish().unwrap_or_default()
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

// ---------------------------------------------------------------------------
// Audio Element — offsets 0x1A..0x28
// ---------------------------------------------------------------------------

/// The `test_000003` Audio Element, as its textproto publishes it: channel
/// based, one substream (id 0), one stereo layer, both gate flags clear.
fn published_audio_element() -> Obu<AudioElement> {
    Obu::new(
        ObuHeader::new(ObuType::AudioElement),
        AudioElement::channel_based(
            300,
            200,
            vec![0],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Stereo,
                1,
                1,
            )),
        ),
    )
}

/// An Audio Element OBU whose `audio_element_type` is `value`, with four bytes
/// of payload after the common fields that this crate cannot interpret.
fn reserved_element_obu(value: u8) -> Vec<u8> {
    let type_byte = value.wrapping_shl(5);
    vec![
        0x08, 0x0c, // Audio Element, obu_size 12
        0xac, 0x02, // audio_element_id 300
        type_byte, // audio_element_type(3) / reserved(5)
        0xc8, 0x01, // codec_config_id 200
        0x01, 0x00, // num_substreams 1, id 0
        0x00, // num_parameters 0
        0xde, 0xad, 0xbe, 0xef, // an element config we do not model
    ]
}

#[test]
fn audio_element_reproduces_offsets_0x1a_through_0x27() {
    // 08 = type(5)=1 Audio Element. 0c = obu_size 12.
    // 00 = audio_element_type(3)=0 channel-based / reserved(5)=0.
    // 20 = num_layers(3)=1 / reserved(5)=0.
    // 10 = loudspeaker_layout(4)=1 Stereo / output_gain(1)=0 /
    //      recon_gain(1)=0 / reserved_a(2)=0.
    assert_eq!(
        obu_bytes(&published_audio_element(), write_audio_element),
        hex!("08 0c ac 02 00 c8 01 01 00 00 20 10 01 01"),
    );
}

#[test]
fn audio_element_matches_the_vendored_reference_file_at_0x1a() {
    let expected = TEST_000003.get(0x1a..0x28).expect("the file is longer");
    assert_eq!(
        obu_bytes(&published_audio_element(), write_audio_element),
        expected,
    );
}

#[test]
fn audio_element_type_occupies_the_top_three_bits_of_one_byte() {
    let bytes = obu_bytes(&published_audio_element(), write_audio_element);
    assert_eq!(bytes.get(4), Some(&0x00), "channel-based is 0x00");

    // The same byte with element type 3 in the top three bits is 0x60.
    assert_eq!(reserved_element_obu(3).get(4), Some(&0x60));
}

#[test]
fn num_layers_occupies_the_top_three_bits_of_one_byte() {
    let bytes = obu_bytes(&published_audio_element(), write_audio_element);
    assert_eq!(bytes.get(10), Some(&0x20), "one layer is 0x20");
}

#[test]
fn the_layer_configuration_byte_packs_stereo_as_0x10() {
    let bytes = obu_bytes(&published_audio_element(), write_audio_element);
    assert_eq!(bytes.get(11), Some(&0x10));
}

#[test]
fn the_layer_configuration_byte_packs_five_one_as_0x20() {
    let obu = Obu::new(
        ObuHeader::new(ObuType::AudioElement),
        AudioElement::channel_based(
            300,
            200,
            vec![0, 1, 2, 3],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Ch5_1,
                4,
                2,
            )),
        ),
    );
    let bytes = obu_bytes(&obu, write_audio_element);
    // ... ac 02 | 00 | c8 01 | 04 00 01 02 03 | 00 | 20 | 20 | 04 | 02
    assert_eq!(bytes.get(14), Some(&0x20), "5.1 with both flags clear");
}

#[test]
fn a_five_one_single_layer_element_carries_four_substreams_and_two_coupled() {
    // BCG packing for 5.1: L/R, Ls/Rs, C, LFE — four substreams of which two
    // are coupled stereo pairs.
    let obu = Obu::new(
        ObuHeader::new(ObuType::AudioElement),
        AudioElement::channel_based(
            300,
            200,
            vec![0, 1, 2, 3],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Ch5_1,
                4,
                2,
            )),
        ),
    );
    let bytes = obu_bytes(&obu, write_audio_element);
    assert_eq!(bytes.get(15), Some(&0x04), "substream_count");
    assert_eq!(bytes.get(16), Some(&0x02), "coupled_substream_count");

    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_audio_element).expect("the vector parses");
    assert_eq!(parsed.payload.num_substreams(), 4);
    assert!(parsed.payload.validate().is_empty(), "the layers agree");
}

#[test]
fn audio_element_round_trips_through_read() {
    let bytes = obu_bytes(&published_audio_element(), write_audio_element);
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_audio_element).expect("the vector parses");

    assert_eq!(parsed.payload.audio_element_id, 300);
    assert_eq!(parsed.payload.codec_config_id, 200);
    assert_eq!(parsed.payload.audio_substream_ids, vec![0]);
    assert_eq!(parsed.payload.num_parameters(), 0);
    assert!(parsed.trailing.is_empty());
    assert_eq!(obu_bytes(&parsed, write_audio_element), bytes);
}

#[test]
fn reserved_element_type_round_trips_byte_identically() {
    // The companion invariant test D-04 exists to buy. `Reserved { value, raw }`
    // is the ONLY mechanism in this crate that can assert byte-identity on
    // something we did not understand — this goes red the instant a future
    // phase reintroduces the channel-only assumption.
    for value in 2_u8..=7 {
        let bytes = reserved_element_obu(value);
        let mut r = BitCursor::new(&bytes);
        let parsed = read_obu_with(&mut r, read_audio_element)
            .unwrap_or_else(|e| panic!("element type {value} parses: {e}"));

        match &parsed.payload.audio_element_type {
            AudioElementType::Reserved { value: got, raw } => {
                assert_eq!(*got, value);
                assert_eq!(raw, &vec![0xde, 0xad, 0xbe, 0xef]);
            }
            other => panic!("element type {value} should be Reserved, got {other:?}"),
        }
        assert!(
            parsed.payload.trailing.is_empty(),
            "raw consumed the payload, so the element-level trailing is empty"
        );
        assert!(
            parsed.trailing.is_empty(),
            "D-05 precedence: raw wins, so the OBU-level trailing stays empty"
        );
        assert_eq!(
            obu_bytes(&parsed, write_audio_element),
            bytes,
            "element type {value} re-serialises byte-identically"
        );
    }
}

#[test]
fn an_expanded_layout_carries_its_expanded_value_inside_the_variant() {
    // The wire rule "expanded_loudspeaker_layout is present only when
    // loudspeaker_layout == 15" is encoded in the TYPE: there is no way to
    // build an expanded layout with a different loudspeaker_layout.
    let layout = LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch9_1_6);
    assert_eq!(layout.value(), 15);
    assert_eq!(layout.expanded(), Some(ExpandedLoudspeakerLayout::Ch9_1_6));
    assert_eq!(LoudspeakerLayout::Stereo.expanded(), None);
    assert_eq!(
        LoudspeakerLayout::from_value(15),
        None,
        "15 is incomplete without the byte that follows it"
    );

    let obu = Obu::new(
        ObuHeader::new(ObuType::AudioElement),
        AudioElement::channel_based(
            300,
            200,
            vec![0],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(layout, 1, 0)),
        ),
    );
    let bytes = obu_bytes(&obu, write_audio_element);
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_audio_element).expect("the vector parses");
    assert_eq!(obu_bytes(&parsed, write_audio_element), bytes);
}

#[test]
fn every_four_bit_loudspeaker_layout_value_round_trips_except_the_expanded_one() {
    for raw in 0_u8..15 {
        let layout = LoudspeakerLayout::from_value(raw)
            .unwrap_or_else(|| panic!("{raw} is a complete layout"));
        assert_eq!(layout.value(), raw);
    }
    for raw in 0_u8..=255 {
        assert_eq!(ExpandedLoudspeakerLayout::from_value(raw).value(), raw);
    }
    for raw in 0_u8..16 {
        assert_eq!(SoundSystem::from_value(raw).value(), raw);
    }
}

#[test]
fn a_substream_count_past_the_end_of_the_input_is_refused_without_allocating() {
    // num_substreams = 0x7f with two payload bytes left.
    let bytes = hex!("08 06 ac 02 00 c8 01 7f");
    let mut r = BitCursor::new(&bytes);
    let err = read_obu_with(&mut r, read_audio_element)
        .expect_err("a count past the end of the input is refused");
    assert_eq!(err.kind(), &ErrorKind::UnexpectedEndOfInput);
}
