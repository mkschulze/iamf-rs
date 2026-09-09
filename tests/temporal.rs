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
use iamf::error::{ErrorKind, Location};
use iamf::obu::{
    AnimationType, AudioFrame, BlockDurationFields, DemixingInfoParameterData, DurationFields,
    MixGainParameterData, Obu, ObuHeader, ObuType, ParamDefinition, ParamDefinitionRegistry,
    ParameterBlock, ParameterData, ParameterDataContext, ParameterSubblock, ReconGainElement,
    ReconGainInfoParameterData, TemporalDelimiter, Trimming, TypeSpecific, obu_type_for,
    plan_frames, read_audio_frame, read_obu_with, read_obu_with_header, read_parameter_block,
    read_temporal_delimiter, substream_id_for, validate_temporal_unit, write_audio_frame,
    write_obu, write_obu_with, write_obu_with_header, write_parameter_block,
    write_temporal_delimiter,
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
    assert!(
        obu.trailing.is_empty(),
        "the frame claims the whole payload"
    );
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

// ---------------------------------------------------------------------------
// The frame planner (TIME-03)
// ---------------------------------------------------------------------------

#[test]
fn a_non_multiple_sample_count_produces_exactly_one_trimmed_frame_at_the_end() {
    // 1000 samples at 128 per frame => 8 frames, the last carrying
    // 8 * 128 - 1000 = 24 samples of end trim.
    let plan = match plan_frames(1000, 128) {
        Ok(plan) => plan,
        Err(error) => panic!("a non-multiple length plans: {error}"),
    };

    assert_eq!(plan.frame_count, 8);
    assert_eq!(plan.trim_at_end, 24);
    assert!(
        plan.trim_at_end > 0 && plan.trim_at_end < 128,
        "strictly bounded: 0 < {} < 128",
        plan.trim_at_end
    );

    for index in 0..7 {
        assert_eq!(
            plan.trimming_for(index),
            None,
            "frame {index} carries no trimming"
        );
    }
    assert_eq!(
        plan.trimming_for(7),
        Some(Trimming {
            at_end: 24,
            at_start: 0
        }),
        "LPCM has no priming, so the two trim values DIFFER — which is what \
         makes a swapped END/START write order detectable at all"
    );
}

#[test]
fn an_exact_multiple_sample_count_produces_no_trimmed_frame() {
    let plan = match plan_frames(1024, 128) {
        Ok(plan) => plan,
        Err(error) => panic!("an exact multiple plans: {error}"),
    };

    assert_eq!(plan.frame_count, 8);
    assert_eq!(plan.trim_at_end, 0);
    assert_eq!(
        plan.trimming_for(7),
        None,
        "a zero end-trim sets no trimming flag at all"
    );
}

#[test]
fn a_zero_num_samples_per_frame_is_a_typed_error_not_a_division_by_zero() {
    // T-01-34. The shipped `tones_256samp_5p1_pcm.iamf` demonstrates that this
    // value occurs in the wild.
    assert_eq!(
        plan_frames(1000, 0).err().map(|e| e.kind().clone()),
        Some(ErrorKind::ZeroSamplesPerFrame)
    );
}

#[test]
fn the_frame_planner_refuses_a_sample_count_that_overflows_its_arithmetic() {
    assert_eq!(
        plan_frames(u64::MAX, 2).err().map(|e| e.kind().clone()),
        Some(ErrorKind::FramePlanOverflow),
        "checked arithmetic throughout — a frame capacity that cannot be \
         represented is an error, not a wrap"
    );
}

#[test]
fn a_temporal_unit_whose_frames_disagree_on_trim_is_rejected() {
    // Research assumption A4, re-verified at the pinned tag before being
    // enforced — see CONFORMANCE-GATE.md, Experiment B.
    let trim = Some(Trimming {
        at_end: 24,
        at_start: 0,
    });
    let agreeing = vec![
        AudioFrame::new(0, vec![0x00]).into_obu(trim),
        AudioFrame::new(1, vec![0x00]).into_obu(trim),
    ];
    assert!(validate_temporal_unit(&agreeing).is_ok());

    let disagreeing = vec![
        AudioFrame::new(0, vec![0x00]).into_obu(trim),
        AudioFrame::new(1, vec![0x00]).into_obu(None),
    ];
    assert_eq!(
        validate_temporal_unit(&disagreeing)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::TemporalUnitTrimMismatch)
    );

    let duplicated = vec![
        AudioFrame::new(0, vec![0x00]).into_obu(trim),
        AudioFrame::new(0, vec![0x00]).into_obu(trim),
    ];
    assert_eq!(
        validate_temporal_unit(&duplicated)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::DuplicateSubstreamId)
    );
}

// ---------------------------------------------------------------------------
// The Temporal Delimiter (TIME-04)
// ---------------------------------------------------------------------------

#[test]
fn a_temporal_delimiter_emits_exactly_two_bytes() {
    let obu = Obu::new(
        ObuHeader::new(ObuType::TemporalDelimiter),
        TemporalDelimiter,
    );
    let mut w = BitWriter::new();
    let written = write_obu_with(&mut w, &obu, write_temporal_delimiter);
    assert!(
        written.is_ok(),
        "a Temporal Delimiter serialises: {written:?}"
    );

    assert_eq!(
        w.finish().unwrap_or_default(),
        hex!("20 00"),
        "type 4 (`00100`) with all three flags clear, then obu_size 0"
    );
}

#[test]
fn the_trimming_flag_and_redundant_copy_are_both_refused_on_a_temporal_delimiter() {
    let trimmed = ObuHeader::new(ObuType::TemporalDelimiter).with_type_specific(
        TypeSpecific::Trimming(Some(Trimming {
            at_end: 1,
            at_start: 0,
        })),
    );
    let mut w = BitWriter::new();
    assert_eq!(
        write_obu(&mut w, &trimmed, &[])
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::TrimmingFlagNotAllowed),
        "libiamf@v1.1.0's splitter would read two trim bytes out of the NEXT OBU"
    );

    let redundant = ObuHeader::new(ObuType::TemporalDelimiter).with_redundant_copy(true);
    let mut w = BitWriter::new();
    assert_eq!(
        write_obu(&mut w, &redundant, &[])
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::RedundantCopyNotAllowed)
    );
}

#[test]
fn a_temporal_delimiter_round_trips_through_the_reader() {
    let bytes = hex!("20 00");
    let mut r = BitCursor::new(&bytes);
    let obu = match read_obu_with(&mut r, read_temporal_delimiter) {
        Ok(obu) => obu,
        Err(error) => panic!("a Temporal Delimiter parses: {error}"),
    };

    assert_eq!(obu.header.obu_type, ObuType::TemporalDelimiter);
    assert!(obu.trailing.is_empty(), "it carries no payload at all");

    let mut w = BitWriter::new();
    let written = write_obu_with(&mut w, &obu, write_temporal_delimiter);
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(w.finish().unwrap_or_default(), bytes);
}

// ---------------------------------------------------------------------------
// The Parameter Block, parsed with an explicit context (TIME-05)
// ---------------------------------------------------------------------------

#[test]
fn a_mode_1_parameter_block_carries_its_own_duration_fields() {
    // parameter_id 100, duration 128, constant_subblock_duration 128 =>
    // num_subblocks is IMPLICIT (ceil(128/128) = 1) and NOT on the wire.
    // Then one subblock: animation_type 0 (Step), start_point_value 0.
    let bytes = hex!("64 80 01 80 01 00 00 00");
    let definition = ParamDefinition::mode_1(100, 16000);
    let registry = registry_with(definition.clone(), ParameterDataContext::MixGain);
    let mut r = BitCursor::new(&bytes);

    let block = match read_parameter_block(&mut r, &registry) {
        Ok(block) => block,
        Err(error) => panic!("a mode-1 Mix Gain block parses: {error}"),
    };

    assert_eq!(block.parameter_id, 100);
    assert_eq!(block.subblocks.len(), 1);
    assert_eq!(
        block
            .subblocks
            .first()
            .and_then(ParameterSubblock::mix_gain),
        Some(&MixGainParameterData::Step {
            start_point_value: 0
        }),
        "animation type 0 is Step and carries one i16"
    );

    let mut w = BitWriter::new();
    let written =
        write_parameter_block(&mut w, &definition, &ParameterDataContext::MixGain, &block);
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(w.finish().unwrap_or_default(), bytes);

    assert_eq!(AnimationType::from_value(0), AnimationType::Step);
    assert_eq!(AnimationType::from_value(1), AnimationType::Linear);
    assert_eq!(AnimationType::from_value(2), AnimationType::Bezier);
    assert_eq!(AnimationType::from_value(3), AnimationType::Reserved(3));
}

#[test]
fn a_mode_0_parameter_block_takes_its_duration_from_the_definition() {
    // The definition supplies duration and constant_subblock_duration, so the
    // block carries ONLY the parameter id and the subblock data. The same
    // bytes read against a mode-1 definition would be a different block —
    // which is exactly why the definition is an argument and not hidden state.
    let definition = ParamDefinition {
        parameter_id: 100,
        parameter_rate: 16000,
        reserved: 0,
        duration_fields: Some(DurationFields {
            duration: 256,
            constant_subblock_duration: 128,
            subblock_durations: Vec::new(),
        }),
    };
    let bytes = hex!("64 01 00 00 ff ff 01 00 00 ff ff");
    let registry = registry_with(definition.clone(), ParameterDataContext::MixGain);
    let mut r = BitCursor::new(&bytes);

    let block = match read_parameter_block(&mut r, &registry) {
        Ok(block) => block,
        Err(error) => panic!("a mode-0 Mix Gain block parses: {error}"),
    };

    assert!(
        block.duration_fields.is_none(),
        "under mode 0 the block omits them — they come from the definition"
    );
    assert_eq!(
        block.subblocks.len(),
        2,
        "num_subblocks is ceil(256 / 128) = 2, implied by the DEFINITION"
    );

    let mut w = BitWriter::new();
    let written =
        write_parameter_block(&mut w, &definition, &ParameterDataContext::MixGain, &block);
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(w.finish().unwrap_or_default(), bytes);
}

#[test]
fn the_three_animation_shapes_have_the_reference_field_widths() {
    let definition = ParamDefinition::mode_1(1, 48000);
    let registry = registry_with(definition, ParameterDataContext::MixGain);
    let read_one = |body: &[u8]| -> Option<MixGainParameterData> {
        let mut bytes = vec![0x01, 0x01, 0x01];
        bytes.extend_from_slice(body);
        let mut r = BitCursor::new(&bytes);
        read_parameter_block(&mut r, &registry)
            .ok()?
            .subblocks
            .first()
            .and_then(ParameterSubblock::mix_gain)
            .cloned()
    };

    assert_eq!(
        read_one(&hex!("00 ff 9c")),
        Some(MixGainParameterData::Step {
            start_point_value: -100
        }),
        "Step: one i16"
    );
    assert_eq!(
        read_one(&hex!("01 ff 9c 00 64")),
        Some(MixGainParameterData::Linear {
            start_point_value: -100,
            end_point_value: 100
        }),
        "Linear: two i16"
    );
    assert_eq!(
        read_one(&hex!("02 ff 9c 00 64 00 32 80")),
        Some(MixGainParameterData::Bezier {
            start_point_value: -100,
            end_point_value: 100,
            control_point_value: 50,
            control_point_relative_time: 0x80
        }),
        "Bezier: three i16 plus a u8 Q0.8 relative time"
    );
}

#[test]
fn an_unmodelled_parameter_definition_type_preserves_its_payload_verbatim() {
    // param_definition_type 7 is not modelled: its subblock data is an
    // explicit `parameter_data_size` uleb128 followed by that many bytes,
    // exactly as `ExtensionParameterData` defines it — so "verbatim" has a
    // length on the wire rather than an invented boundary.
    let definition = ParamDefinition::mode_1(5, 48000);
    let bytes = hex!("05 01 01 03 de ad be");
    let registry = registry_with(definition.clone(), ParameterDataContext::Reserved(7));
    let mut r = BitCursor::new(&bytes);

    let block = match read_parameter_block(&mut r, &registry) {
        Ok(block) => block,
        Err(error) => panic!("an unmodelled parameter type parses: {error}"),
    };

    assert_eq!(
        block.subblocks.first().and_then(ParameterSubblock::raw),
        Some(&[0xDE, 0xAD, 0xBE][..]),
        "the bytes are kept verbatim so Phase 2's PARSE-06 has something to assert on"
    );

    let mut w = BitWriter::new();
    let written = write_parameter_block(
        &mut w,
        &definition,
        &ParameterDataContext::Reserved(7),
        &block,
    );
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(
        w.finish().unwrap_or_default(),
        bytes,
        "and it re-serialises unchanged"
    );
}

#[test]
fn a_mix_gain_definition_rejects_raw_parameter_data_before_writing() {
    let definition = ParamDefinition::mode_1(5, 48_000);
    let block = ParameterBlock {
        parameter_id: 5,
        duration_fields: Some(BlockDurationFields {
            duration: 1,
            constant_subblock_duration: 1,
        }),
        subblocks: vec![ParameterSubblock {
            subblock_duration: None,
            data: ParameterData::Raw(vec![0xde, 0xad]),
        }],
    };
    let mut w = BitWriter::new();

    let err = write_parameter_block(&mut w, &definition, &ParameterDataContext::MixGain, &block)
        .expect_err("raw data cannot be framed as Mix Gain syntax");

    assert_eq!(err.kind(), &ErrorKind::UnsupportedParameterData);
    assert_eq!(w.finish().unwrap_or_default(), Vec::<u8>::new());
}

#[test]
fn canonical_parameter_kinds_cannot_be_smuggled_through_reserved_aliases() {
    let definition = ParamDefinition::mode_1(5, 48_000);
    let block = ParameterBlock {
        parameter_id: 5,
        duration_fields: Some(BlockDurationFields {
            duration: 1,
            constant_subblock_duration: 1,
        }),
        subblocks: vec![ParameterSubblock {
            subblock_duration: None,
            data: ParameterData::Raw(vec![0xde, 0xad]),
        }],
    };

    for raw_kind in [0_u32, 1, 2] {
        let mut w = BitWriter::new();
        let err = write_parameter_block(
            &mut w,
            &definition,
            &ParameterDataContext::Reserved(raw_kind),
            &block,
        )
        .expect_err("values 0, 1, and 2 are canonical kinds, not extensions");

        assert_eq!(err.kind(), &ErrorKind::UnsupportedParameterData);
        assert_eq!(w.finish().unwrap_or_default(), Vec::<u8>::new());
    }
}

fn step_subblock(duration: Option<u32>) -> ParameterSubblock {
    ParameterSubblock {
        subblock_duration: duration,
        data: ParameterData::MixGain(MixGainParameterData::Step {
            start_point_value: 0,
        }),
    }
}

#[test]
fn an_implied_parameter_subblock_count_mismatch_is_rejected_before_writing() {
    let definition = ParamDefinition::mode_1(5, 48_000);
    let block = ParameterBlock {
        parameter_id: 5,
        duration_fields: Some(BlockDurationFields {
            duration: 4,
            constant_subblock_duration: 2,
        }),
        subblocks: vec![step_subblock(None)],
    };
    let mut w = BitWriter::new();

    let err = write_parameter_block(&mut w, &definition, &ParameterDataContext::MixGain, &block)
        .expect_err("ceil(4 / 2) requires two subblocks");

    assert_eq!(err.kind(), &ErrorKind::SubblockDurationMismatch);
    assert_eq!(w.finish().unwrap_or_default(), Vec::<u8>::new());
}

#[test]
fn explicit_parameter_subblock_durations_are_required_and_must_sum_to_duration() {
    let definition = ParamDefinition::mode_1(5, 48_000);
    let missing = ParameterBlock {
        parameter_id: 5,
        duration_fields: Some(BlockDurationFields {
            duration: 3,
            constant_subblock_duration: 0,
        }),
        subblocks: vec![step_subblock(None)],
    };
    let wrong_sum = ParameterBlock {
        subblocks: vec![step_subblock(Some(2))],
        ..missing.clone()
    };

    let mut missing_writer = BitWriter::new();
    let missing_err = write_parameter_block(
        &mut missing_writer,
        &definition,
        &ParameterDataContext::MixGain,
        &missing,
    )
    .expect_err("mode 1 with no constant duration carries each duration");
    assert_eq!(missing_err.kind(), &ErrorKind::SubblockDurationMismatch);
    assert_eq!(
        missing_writer.finish().unwrap_or_default(),
        Vec::<u8>::new()
    );

    let mut sum_writer = BitWriter::new();
    let sum_err = write_parameter_block(
        &mut sum_writer,
        &definition,
        &ParameterDataContext::MixGain,
        &wrong_sum,
    )
    .expect_err("the explicit durations must total three");
    assert_eq!(sum_err.kind(), &ErrorKind::SubblockDurationMismatch);
    assert_eq!(sum_writer.finish().unwrap_or_default(), Vec::<u8>::new());
}

#[test]
fn parameter_subblock_durations_are_forbidden_when_the_duration_is_implied() {
    let definition = ParamDefinition::mode_1(5, 48_000);
    let block = ParameterBlock {
        parameter_id: 5,
        duration_fields: Some(BlockDurationFields {
            duration: 2,
            constant_subblock_duration: 2,
        }),
        subblocks: vec![step_subblock(Some(2))],
    };
    let mut w = BitWriter::new();

    let err = write_parameter_block(&mut w, &definition, &ParameterDataContext::MixGain, &block)
        .expect_err("an implied duration has no per-subblock field on the wire");

    assert_eq!(err.kind(), &ErrorKind::SubblockDurationMismatch);
    assert_eq!(w.finish().unwrap_or_default(), Vec::<u8>::new());
}

#[test]
fn an_unknown_animation_type_is_the_same_typed_error_the_reference_returns() {
    // `iamf-tools` returns UnimplementedError("Unknown animation type= ") for
    // values above 2, and it has to: the wire carries no length for an
    // unrecognised animation, so there is no boundary to preserve verbatim
    // without inventing one. Matching the reference exactly is the rule; being
    // looser here would mean guessing where the next subblock starts.
    let definition = ParamDefinition::mode_1(1, 48000);
    let bytes = hex!("01 01 01 03 00 00");
    let registry = registry_with(definition, ParameterDataContext::MixGain);
    let mut r = BitCursor::new(&bytes);

    assert_eq!(
        read_parameter_block(&mut r, &registry)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::UnsupportedParameterData)
    );
}

#[test]
fn a_parameter_id_disagreeing_with_the_definition_is_a_typed_error() {
    let definition = ParamDefinition::mode_1(100, 16000);
    let registry = registry_with(definition, ParameterDataContext::MixGain);
    let bytes = hex!("63 01 01 00 00 00");
    let mut r = BitCursor::new(&bytes);

    assert_eq!(
        read_parameter_block(&mut r, &registry)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::NoGoverningParamDefinition),
        "registry dispatch refuses an id that no descriptor governs"
    );
}

#[test]
fn a_subblock_count_larger_than_the_bytes_remaining_is_refused_before_reserving() {
    // T-01-32: `num_subblocks` is an attacker-controlled count driving an
    // allocation. duration 0x0FFFFFFF, constant_subblock_duration 0 =>
    // num_subblocks is explicit, and here claims 0x0FFFFFFF subblocks with
    // nothing left in the buffer.
    let definition = ParamDefinition::mode_1(1, 48000);
    let bytes = hex!("01 ff ff ff 7f 00 ff ff ff 7f");
    let registry = registry_with(definition, ParameterDataContext::MixGain);
    let mut r = BitCursor::new(&bytes);

    assert_eq!(
        read_parameter_block(&mut r, &registry)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::UnexpectedEndOfInput),
        "bounded against bytes_remaining() before a single element is reserved"
    );
}

fn registry_with(
    definition: ParamDefinition,
    context: ParameterDataContext,
) -> ParamDefinitionRegistry {
    let mut registry = ParamDefinitionRegistry::new();
    registry.register(definition, context);
    registry
}

#[test]
fn demixing_data_uses_exactly_three_mode_bits_and_five_reserved_bits() {
    // id 5, duration 1, constant duration 1, then 0b101_11011.
    let bytes = hex!("05 01 01 bb");
    let definition = ParamDefinition::mode_1(5, 48_000);
    let registry = registry_with(definition.clone(), ParameterDataContext::Demixing);
    let mut reader = BitCursor::new(&bytes);

    let block = read_parameter_block(&mut reader, &registry).expect("demixing data parses");
    assert_eq!(
        block.subblocks.first().map(|subblock| &subblock.data),
        Some(&ParameterData::Demixing(DemixingInfoParameterData {
            dmixp_mode: 5,
            reserved: 0x1b,
        }))
    );

    let mut writer = BitWriter::new();
    write_parameter_block(
        &mut writer,
        &definition,
        &ParameterDataContext::Demixing,
        &block,
    )
    .expect("demixing data writes");
    assert_eq!(writer.finish().unwrap_or_default(), bytes);
}

#[test]
fn test_000059_two_layer_recon_gain_shape_round_trips_exact_bytes() {
    // id 101, duration 960, constant duration 960. Layer 0 is absent and
    // consumes no bytes. Layer 1 flag 0x1d selects gains 0, 2, 3, and 4.
    let bytes = hex!("65 c0 07 c0 07 1d ff ff ff ff");
    let definition = ParamDefinition::mode_1(101, 48_000);
    let context = ParameterDataContext::ReconGain {
        recon_gain_is_present: vec![false, true],
    };
    let registry = registry_with(definition.clone(), context.clone());
    let mut reader = BitCursor::new(&bytes);

    let block = read_parameter_block(&mut reader, &registry).expect("test_000059 shape parses");
    let mut gains = [0_u8; 12];
    gains[0] = 0xff;
    gains[2] = 0xff;
    gains[3] = 0xff;
    gains[4] = 0xff;
    assert_eq!(
        block.subblocks.first().map(|subblock| &subblock.data),
        Some(&ParameterData::ReconGain(ReconGainInfoParameterData {
            layers: vec![
                None,
                Some(ReconGainElement {
                    recon_gain_flag: 0x1d,
                    recon_gain: gains,
                }),
            ],
        }))
    );

    let mut writer = BitWriter::new();
    write_parameter_block(&mut writer, &definition, &context, &block)
        .expect("test_000059 shape writes");
    assert_eq!(writer.finish().unwrap_or_default(), bytes);
}

#[test]
fn missing_and_mismatched_recon_gain_context_are_located_errors() {
    let bytes = hex!("65 c0 07 c0 07 1d ff ff ff ff");
    let mut reader = BitCursor::new(&bytes);
    let missing = read_parameter_block(&mut reader, &ParamDefinitionRegistry::new())
        .expect_err("parameter 101 has no governing definition");
    assert_eq!(missing.kind(), &ErrorKind::NoGoverningParamDefinition);
    assert_eq!(missing.at(), Location::InputOffset(0));

    let definition = ParamDefinition::mode_1(101, 48_000);
    let block = ParameterBlock {
        parameter_id: 101,
        duration_fields: Some(BlockDurationFields {
            duration: 960,
            constant_subblock_duration: 960,
        }),
        subblocks: vec![ParameterSubblock {
            subblock_duration: None,
            data: ParameterData::ReconGain(ReconGainInfoParameterData {
                layers: vec![
                    None,
                    Some(ReconGainElement {
                        recon_gain_flag: 0,
                        recon_gain: [0; 12],
                    }),
                ],
            }),
        }],
    };
    let mut writer = BitWriter::new();
    let mismatch = write_parameter_block(
        &mut writer,
        &definition,
        &ParameterDataContext::ReconGain {
            recon_gain_is_present: vec![true, false],
        },
        &block,
    )
    .expect_err("layer options must agree with descriptor flags");
    assert_eq!(mismatch.kind(), &ErrorKind::UnsupportedParameterData);
    assert_eq!(mismatch.at(), Location::Field("recon_gain_is_present"));
}

#[test]
fn reserved_demixing_modes_and_high_recon_flags_are_preserved_but_diagnosed() {
    let demixing = ParameterData::Demixing(DemixingInfoParameterData {
        dmixp_mode: 3,
        reserved: 0,
    });
    assert!(demixing.validate().iter().any(|finding| {
        finding.at == Location::Field("dmixp_mode") && finding.message.contains("reserved")
    }));

    let recon = ParameterData::ReconGain(ReconGainInfoParameterData {
        layers: vec![Some(ReconGainElement {
            recon_gain_flag: 1 << 12,
            recon_gain: [0; 12],
        })],
    });
    assert!(recon.validate().iter().any(|finding| {
        finding.at == Location::Field("recon_gain_flag") && finding.message.contains("above bit 11")
    }));
}

#[test]
fn corrupt_known_demixing_syntax_is_not_reclassified_as_raw_data() {
    let definition = ParamDefinition::mode_1(5, 48_000);
    let registry = registry_with(definition, ParameterDataContext::Demixing);
    let mut reader = BitCursor::new(&hex!("05 01 01"));

    assert_eq!(
        read_parameter_block(&mut reader, &registry)
            .err()
            .map(|error| error.kind().clone()),
        Some(ErrorKind::UnexpectedEndOfInput)
    );
}

#[test]
fn a_parameter_block_may_not_carry_a_redundant_copy_flag() {
    let header = ObuHeader::new(ObuType::ParameterBlock).with_redundant_copy(true);
    let mut w = BitWriter::new();

    assert_eq!(
        write_obu(&mut w, &header, &[])
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::RedundantCopyNotAllowed)
    );
}
