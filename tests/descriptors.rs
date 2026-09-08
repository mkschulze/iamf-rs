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
use iamf::model::by_id;
use iamf::obu::{
    AnchorElement, AnchoredLoudness, AudioElement, AudioElementType, ChannelAudioLayerConfig,
    CodecConfig, DecoderConfig, IaSequenceHeader, Layout, LayoutWithLoudness, Loudness,
    LoudnessExtension, LpcmDecoderConfig, Obu, ObuHeader, ObuType, SampleFormatFlags,
    ScalableChannelLayoutConfig, read_audio_element, read_codec_config, read_ia_sequence_header,
    read_mix_presentation, read_obu_with, write_audio_element, write_codec_config,
    write_ia_sequence_header, write_mix_presentation, write_obu_with,
};

/// The published `test_000003` configuration, shared with `tests/sequence.rs`.
///
/// One transcription of the textproto, used by both the 120-byte prologue proof
/// here and the whole-file proof there — a second copy is a second thing to
/// keep in step with the reference.
#[path = "support/test_000003.rs"]
mod support;

use support::{
    TEST_000003, descriptor_bytes, published_audio_element, published_codec_config,
    published_descriptor_set, published_mix_gain, published_mix_presentation,
    published_sequence_header,
};

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

// ---------------------------------------------------------------------------
// Mix Presentation — offsets 0x28..0x78
// ---------------------------------------------------------------------------

#[test]
fn mix_presentation_reproduces_offsets_0x28_through_0x77() {
    let expected = TEST_000003.get(0x28..0x78).expect("the file is longer");
    assert_eq!(
        obu_bytes(&published_mix_presentation(), write_mix_presentation),
        expected,
    );
}

#[test]
fn the_mix_presentation_header_byte_is_0x10_with_bit_6_clear() {
    // Bit 6 on a Mix Presentation is RESERVED, not an optional-fields flag.
    // libiamf@v1.1.0 reads two trim fields whenever bit 6 is set, for ANY OBU
    // type, so a set bit here shifts the whole payload by two bytes silently.
    let bytes = obu_bytes(&published_mix_presentation(), write_mix_presentation);
    assert_eq!(bytes.first(), Some(&0x10));
    assert_eq!(bytes.get(1), Some(&0x4e), "obu_size 78");
}

#[test]
fn strings_are_nul_terminated_on_the_wire() {
    let bytes = obu_bytes(&published_mix_presentation(), write_mix_presentation);
    // "en-us\0" at payload offset 2 -> file offset 0x2C, OBU offset 4.
    assert_eq!(bytes.get(4..10), Some(hex!("65 6e 2d 75 73 00").as_slice()));
    // "test_mix_pres\0" is fourteen bytes.
    assert_eq!(
        bytes.get(10..24).map(<[u8]>::len),
        Some(14),
        "13 characters plus the terminator"
    );
    assert_eq!(bytes.get(23), Some(&0x00));
}

#[test]
fn param_definition_mode_1_writes_0x80_and_the_gain_follows_immediately() {
    let bytes = obu_bytes(&published_mix_presentation(), write_mix_presentation);
    // OBU offset 0x40 == file offset 0x68: the mode byte, then the gain. No
    // duration / constant_subblock_duration / num_subblocks in between — that
    // is the path that emits zero Parameter Block OBUs while the definitions
    // remain mandatory structure.
    assert_eq!(bytes.get(0x40), Some(&0x80));
    assert_eq!(bytes.get(0x41..0x43), Some(hex!("00 00").as_slice()));
    assert!(published_mix_gain().definition.param_definition_mode());
    assert!(published_mix_gain().definition.duration_fields.is_none());
}

#[test]
fn the_layout_byte_packs_sound_system_a_as_0x80() {
    // layout_type(2)=2 in the top two bits, sound_system(4)=0, reserved(2)=0.
    let bytes = obu_bytes(&published_mix_presentation(), write_mix_presentation);
    assert_eq!(bytes.get(0x4a), Some(&0x80));
    assert_eq!(bytes.get(0x4b), Some(&0x00), "info_type 0");
}

#[test]
fn integrated_loudness_and_digital_peak_are_signed_16_big_endian() {
    let bytes = obu_bytes(&published_mix_presentation(), write_mix_presentation);
    assert_eq!(bytes.get(0x4c..0x4e), Some(hex!("ca 5b").as_slice()), "-13733");
    assert_eq!(bytes.get(0x4e..0x50), Some(hex!("cd b1").as_slice()), "-12879");
}

#[test]
fn info_type_is_derived_from_the_optional_loudness_members() {
    let mut loudness = Loudness::new(-13733, -12879);
    assert_eq!(loudness.info_type(), 0x00);

    loudness.true_peak = Some(-1000);
    assert_eq!(loudness.info_type(), 0x01);

    loudness.anchored = Some(AnchoredLoudness {
        anchor_elements: vec![AnchorElement {
            anchor_element: 1,
            anchored_loudness: -2000,
        }],
    });
    assert_eq!(loudness.info_type(), 0x03);

    loudness.extension = Some(LoudnessExtension {
        info_type_bits: 0x04,
        bytes: vec![0xaa],
    });
    assert_eq!(loudness.info_type(), 0x07);

    // And every one of them round-trips, which is what proves info_type is not
    // merely computed but computed correctly.
    let mut obu = published_mix_presentation();
    if let Some(layout) = obu
        .payload
        .sub_mixes
        .first_mut()
        .and_then(|s| s.layouts.first_mut())
    {
        layout.loudness = loudness;
    }
    let bytes = obu_bytes(&obu, write_mix_presentation);
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_mix_presentation).expect("the vector parses");
    assert_eq!(obu_bytes(&parsed, write_mix_presentation), bytes);
}

#[test]
fn a_sub_mix_with_two_layouts_serialises_two_loudness_blocks() {
    // Research correction 9: a 5.1 fixture needs Sound System B as the
    // comparison target AND Sound System A because iamf-tools demands it.
    let mut obu = published_mix_presentation();
    if let Some(sub_mix) = obu.payload.sub_mixes.first_mut() {
        sub_mix.layouts = vec![
            LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::B0_5_0),
                loudness: Loudness::new(-14000, -13000),
            },
            LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                loudness: Loudness::new(-13733, -12879),
            },
        ];
    }
    let bytes = obu_bytes(&obu, write_mix_presentation);
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_mix_presentation).expect("the vector parses");

    let sub_mix = parsed.payload.sub_mixes.first().expect("one sub-mix");
    assert_eq!(sub_mix.num_layouts(), 2);
    assert!(sub_mix.has_stereo_layout());
    assert!(sub_mix.validate().is_empty(), "{:?}", sub_mix.validate());
    assert_eq!(obu_bytes(&parsed, write_mix_presentation), bytes);
}

#[test]
fn a_sub_mix_without_a_stereo_layout_is_a_finding_and_not_a_parse_failure() {
    let mut obu = published_mix_presentation();
    if let Some(layout) = obu
        .payload
        .sub_mixes
        .first_mut()
        .and_then(|s| s.layouts.first_mut())
    {
        layout.layout = Layout::SoundSystem(SoundSystem::B0_5_0);
    }

    // It SERIALISES — the writer is faithful (D-07).
    let bytes = obu_bytes(&obu, write_mix_presentation);

    // It READS — being stricter than the reference here would break Phase 2's
    // foreign-file round-trip; iamf-tools' own read path has a TODO and checks
    // nothing.
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_mix_presentation).expect("the parse SUCCEEDS");

    // And validate() names it.
    let findings = parsed.payload.validate();
    assert!(
        findings.iter().any(|f| f.message.contains("stereo layout")),
        "validate() reports the missing stereo layout: {findings:?}"
    );
}

#[test]
fn mix_presentation_round_trips_through_read() {
    let bytes = obu_bytes(&published_mix_presentation(), write_mix_presentation);
    let mut r = BitCursor::new(&bytes);
    let parsed = read_obu_with(&mut r, read_mix_presentation).expect("the vector parses");

    assert_eq!(parsed.payload.mix_presentation_id, 42);
    assert_eq!(parsed.payload.count_label(), 1);
    assert_eq!(parsed.payload.annotations_language, vec![b"en-us".to_vec()]);
    assert!(parsed.trailing.is_empty());
    assert_eq!(obu_bytes(&parsed, write_mix_presentation), bytes);
}

#[test]
fn presentation_annotation_count_mismatch_is_rejected_before_writing() {
    let mut presentation = published_mix_presentation().payload;
    presentation.localized_presentation_annotations.clear();
    let mut w = BitWriter::new();

    let err = write_mix_presentation(&mut w, &presentation)
        .expect_err("the wire carries no independent annotation count");

    assert_eq!(err.kind(), &ErrorKind::AnnotationCountMismatch);
    assert_eq!(w.finish().unwrap_or_default(), Vec::<u8>::new());
}

#[test]
fn element_annotation_count_mismatch_is_rejected_before_writing() {
    let mut presentation = published_mix_presentation().payload;
    presentation
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.elements.first_mut())
        .expect("the published presentation has an element")
        .localized_element_annotations
        .clear();
    let mut w = BitWriter::new();

    let err = write_mix_presentation(&mut w, &presentation)
        .expect_err("the parser consumes count_label annotations");

    assert_eq!(err.kind(), &ErrorKind::AnnotationCountMismatch);
    assert_eq!(w.finish().unwrap_or_default(), Vec::<u8>::new());
}

// ---------------------------------------------------------------------------
// Descriptor ordering and collections — DESC-08, DESC-09
// ---------------------------------------------------------------------------

#[test]
fn codec_configs_and_audio_elements_sort_by_id_but_mix_presentations_do_not() {
    let mut set = published_descriptor_set();
    let mut second_config = published_codec_config().payload;
    second_config.codec_config_id = 100;
    set.codec_configs.insert(0, second_config);

    let mut second_element = published_audio_element().payload;
    second_element.audio_element_id = 100;
    second_element.codec_config_id = 100;
    set.audio_elements.push(second_element);

    let mut second_presentation = published_mix_presentation().payload;
    second_presentation.mix_presentation_id = 7;
    set.mix_presentations.push(second_presentation);

    let bytes = descriptor_bytes(&set);
    let mut r = BitCursor::new(&bytes);
    let _header = read_obu_with(&mut r, read_ia_sequence_header).expect("sequence header");

    let mut config_ids = Vec::new();
    for _ in 0..2 {
        let obu = read_obu_with(&mut r, read_codec_config).expect("codec config");
        config_ids.push(obu.payload.codec_config_id);
    }
    let mut element_ids = Vec::new();
    for _ in 0..2 {
        let obu = read_obu_with(&mut r, read_audio_element).expect("audio element");
        element_ids.push(obu.payload.audio_element_id);
    }
    let mut presentation_ids = Vec::new();
    for _ in 0..2 {
        let obu = read_obu_with(&mut r, read_mix_presentation).expect("mix presentation");
        presentation_ids.push(obu.payload.mix_presentation_id);
    }

    assert_eq!(config_ids, vec![100, 200], "ascending by codec_config_id");
    assert_eq!(element_ids, vec![100, 300], "ascending by audio_element_id");
    assert_eq!(
        presentation_ids,
        vec![42, 7],
        "list order — the original ordering may be used downstream when \
         selecting the mix presentation, so sorting would be WRONG"
    );
}

#[test]
fn by_id_returns_none_on_an_empty_collection_and_the_first_match_on_a_duplicate() {
    let empty: Vec<CodecConfig> = Vec::new();
    assert!(by_id(&empty, 200).is_none());

    let mut set = published_descriptor_set();
    assert!(set.codec_config_by_id(999).is_none());
    assert!(set.audio_element_by_id(300).is_some());

    let mut duplicate = published_codec_config().payload;
    duplicate.num_samples_per_frame = 64; // same id, different content
    set.codec_configs.push(duplicate);

    let bound = set.codec_config_by_id(200).expect("the first match");
    assert_eq!(
        bound.num_samples_per_frame, 128,
        "by_id binds to the FIRST in bitstream order, as a forward reader would"
    );

    let findings = set.validate();
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("indices 0 and 1")),
        "the duplicate finding names both indices: {findings:?}"
    );
}

#[test]
fn a_sequence_with_zero_mix_presentations_serialises_without_panicking() {
    let mut set = published_descriptor_set();
    set.mix_presentations.clear();
    let bytes = descriptor_bytes(&set);
    assert_eq!(
        bytes.len(),
        0x28,
        "the sequence header, one Codec Config and one Audio Element"
    );
    assert!(set.mix_presentation_by_id(42).is_none());
}

#[test]
fn an_unresolvable_codec_config_reference_is_a_finding() {
    let mut set = published_descriptor_set();
    set.codec_configs.clear();
    let findings = set.validate();
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("codec_config_id 200")),
        "validate() names the unresolvable reference: {findings:?}"
    );
}

#[test]
fn the_120_byte_descriptor_prologue_of_test_000003_is_reproduced_byte_exact() {
    // This plan's central claim. The model is built entirely from
    // test_000003.textproto — the *published configuration* the reference
    // encoder was given — and the bytes it produces are compared against the
    // `.iamf` the reference encoder produced from it.
    //
    // The prologue is 120 bytes, 0x00..0x78, not the 118 PROJECT.md states.
    let produced = descriptor_bytes(&published_descriptor_set());

    assert_eq!(produced.len(), 120);
    assert_eq!(
        produced.as_slice(),
        TEST_000003.get(0x00..0x78).expect("the file is longer"),
    );
    assert!(
        published_descriptor_set().validate().is_empty(),
        "{:?}",
        published_descriptor_set().validate()
    );
}
