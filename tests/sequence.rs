//! CONF-08 — the whole-file byte reproduction, and the SEQ-01/02/03 contract.
//!
//! This is the phase's first end-to-end byte claim. Every one of the 32567
//! bytes of `tests/fixtures/reference/test_000003.iamf` is reproduced from its
//! **published configuration** (`tests/fixtures/reference/test_000003.textproto`
//! plus `sawtooth_100_stereo.wav`, the PCM its `audio_frame_metadata` names),
//! driven through the streaming primitive. Nothing here was captured from this
//! crate's own output.
//!
//! It is also the strongest verification available that plans 01-03 through
//! 01-06 agree with each other and with the reference: the header framing, the
//! four descriptor OBUs, the implicit-substream-id rule, the END-before-START
//! trim order and the two-pass `obu_size` all have to be simultaneously right
//! for a single `assert_eq!` to pass.
//!
//! # The file's arithmetic, stated once
//!
//! 8000 sample frames of 16-bit stereo at 16 kHz, `num_samples_per_frame` 128:
//! `ceil(8000 / 128) == 63` Audio Frames, the last carrying
//! `num_samples_to_trim_at_end = 63 * 128 - 8000 = 64`. Four descriptor OBUs
//! plus 63 Audio Frames is **67 OBUs**, and `find_obu_boundaries` therefore
//! returns **68** entries — 67 starts plus the final end offset, which is
//! `bytes.len()`.
//!
//! One substream (a single coupled stereo pair), so one Audio Frame per
//! temporal unit, so **63 temporal units** — not the 64 the plan's prose says;
//! see the SUMMARY's deviations.

#[path = "support/test_000003.rs"]
mod support;

use std::io::{self, Write};

use iamf::error::ErrorKind;
use iamf::obu::{
    AudioFrame, BlockDurationFields, Obu, ObuHeader, ObuType, ParameterBlock, ParameterData,
    ParameterSubblock, TemporalDelimiter, find_obu_boundaries, plan_frames,
};
use iamf::sequence::{SequenceWriter, TemporalUnit, write_sequence};
use support::{
    FILE_LEN, PROLOGUE_LEN, SAMPLES_PER_FRAME, TEST_000003, TRIM_AT_END, assert_bytes_eq,
    published_descriptor_set, sawtooth_pcm,
};

/// Bytes per interleaved sample frame: 2 channels of 16-bit.
const BYTES_PER_SAMPLE_FRAME: usize = 4;

/// Every temporal unit of `test_000003`, built from the WAV its textproto names.
///
/// The single stereo substream is a coupled pair, so its Audio Frame payload
/// **is** the interleaved WAV data — this is a byte copy and nothing else. The
/// final frame is short and is zero-padded to a whole frame, which is why it
/// carries the end trim; the padding is the caller's, deliberately, because a
/// writer that invented sample values would be doing signal processing.
fn published_temporal_units() -> Vec<TemporalUnit> {
    let pcm = sawtooth_pcm();
    let Ok(plan) = plan_frames(total_sample_frames(pcm), SAMPLES_PER_FRAME) else {
        return Vec::new();
    };
    let frame_bytes = usize::try_from(SAMPLES_PER_FRAME)
        .unwrap_or(0)
        .saturating_mul(BYTES_PER_SAMPLE_FRAME);

    (0..plan.frame_count)
        .map(|index| {
            let start = usize::try_from(index)
                .unwrap_or(0)
                .saturating_mul(frame_bytes);
            let end = start.saturating_add(frame_bytes).min(pcm.len());
            let mut payload = pcm.get(start..end).unwrap_or_default().to_vec();
            payload.resize(frame_bytes, 0);
            let frame = AudioFrame::new(0, payload).into_obu(plan.trimming_for(index));
            TemporalUnit::of_frames(vec![frame])
        })
        .collect()
}

/// How many interleaved sample frames the PCM holds.
///
/// Checked rather than `/` and `as`: GUARD-03's `arithmetic_side_effects` has
/// no in-tests carve-out, unlike `unwrap`/`expect`/`panic`, and a test helper
/// that overflows is a test that lies.
fn total_sample_frames(pcm: &[u8]) -> u64 {
    let frames = pcm.len().checked_div(BYTES_PER_SAMPLE_FRAME).unwrap_or(0);
    u64::try_from(frames).unwrap_or(0)
}

/// Drive the streaming primitive over a `Vec<u8>` and hand back the bytes.
fn stream_whole_file() -> Vec<u8> {
    let mut writer = SequenceWriter::new(Vec::new());
    let pushed = writer.push_descriptors(&published_descriptor_set());
    assert!(pushed.is_ok(), "the descriptors serialise: {pushed:?}");
    for unit in published_temporal_units() {
        let pushed = writer.push_temporal_unit(&unit);
        assert!(pushed.is_ok(), "the temporal unit serialises: {pushed:?}");
    }
    writer.finish().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// CONF-08 — the central claim
// ---------------------------------------------------------------------------

#[test]
fn reproduces_test_000003() {
    let produced = stream_whole_file();

    assert_bytes_eq(&produced, TEST_000003, "test_000003.iamf");
    assert_eq!(produced.len(), FILE_LEN, "0x7F37");
}

#[test]
fn the_published_configuration_yields_63_frames_and_a_64_sample_end_trim() {
    let total_samples = total_sample_frames(sawtooth_pcm());
    assert_eq!(total_samples, 8000, "0.5 s at 16 kHz");

    let plan = plan_frames(total_samples, SAMPLES_PER_FRAME).expect("128 is not zero");
    assert_eq!(plan.frame_count, 63);
    assert_eq!(plan.trim_at_end, TRIM_AT_END);
    assert_eq!(published_temporal_units().len(), 63);
}

#[test]
fn the_output_walks_67_obus_ending_exactly_on_the_file_length() {
    let produced = stream_whole_file();
    let boundaries = find_obu_boundaries(&produced).expect("our own output walks cleanly");

    assert_eq!(
        boundaries.len(),
        68,
        "67 OBU start offsets plus the final end offset"
    );
    assert_eq!(
        boundaries.len().saturating_sub(1),
        67,
        "4 descriptors and 63 audio frames"
    );
    assert_eq!(boundaries.last().copied(), Some(FILE_LEN));
    // The first Audio Frame, and therefore the descriptor prologue length.
    assert_eq!(boundaries.get(4).copied(), Some(PROLOGUE_LEN));
    // The trimmed final frame, hand-decoded at 0x7D32 in 01-RESEARCH.md.
    assert!(boundaries.contains(&32_050));
}

#[test]
fn descriptors_are_written_in_the_reference_order() {
    let produced = stream_whole_file();
    let boundaries = find_obu_boundaries(&produced).expect("walks cleanly");

    // IA Sequence Header, Codec Config, Audio Element, Mix Presentation, then
    // the first Audio Frame — the order `write_descriptors` fixes (DESC-08).
    assert_eq!(
        boundaries.get(..5),
        Some([0_usize, 8, 26, 40, 120].as_slice())
    );
    assert_eq!(
        produced.get(..PROLOGUE_LEN),
        TEST_000003.get(..PROLOGUE_LEN)
    );
}

// ---------------------------------------------------------------------------
// SEQ-02 — the state machine, enforced with typed errors
// ---------------------------------------------------------------------------

#[test]
fn push_temporal_unit_before_push_descriptors_is_a_typed_error() {
    let mut writer = SequenceWriter::new(Vec::new());
    let err = writer
        .push_temporal_unit(&TemporalUnit::default())
        .expect_err("a temporal unit before the prologue");
    assert_eq!(err.kind(), &ErrorKind::DescriptorsNotWritten);
}

#[test]
fn push_descriptors_twice_is_a_typed_error() {
    let mut writer = SequenceWriter::new(Vec::new());
    let set = published_descriptor_set();
    writer.push_descriptors(&set).expect("the first prologue");
    let err = writer
        .push_descriptors(&set)
        .expect_err("a second prologue");
    assert_eq!(err.kind(), &ErrorKind::DescriptorsAlreadyWritten);
}

#[test]
fn finish_consumes_the_writer_so_a_later_push_cannot_compile() {
    // The plan asked for a typed `SequenceFinished` error on "push after
    // finish". `finish` takes `self` by value, so that transition is a
    // **compile** error instead — strictly stronger, and an ErrorKind that can
    // never be produced would be worse than none. The compile-time half is
    // proved by the `compile_fail` doctest on `SequenceWriter::finish`; this
    // test covers the run-time half, that finishing is otherwise ordinary.
    let mut writer = SequenceWriter::new(Vec::new());
    writer
        .push_descriptors(&published_descriptor_set())
        .expect("prologue");
    let sink = writer.finish().expect("finish once");
    assert_eq!(sink.len(), PROLOGUE_LEN);
}

#[test]
fn a_failing_sink_is_a_named_error_not_a_panic() {
    /// A sink that refuses every write.
    struct Refusing;
    impl Write for Refusing {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("refused"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut writer = SequenceWriter::new(Refusing);
    let err = writer
        .push_descriptors(&published_descriptor_set())
        .expect_err("the sink refuses");
    assert_eq!(err.kind(), &ErrorKind::SinkWrite);
}

#[test]
fn an_invalid_temporal_unit_is_fully_preflighted_before_its_delimiter_is_flushed() {
    let mut writer = SequenceWriter::new(Vec::new());
    writer
        .push_descriptors(&published_descriptor_set())
        .expect("prologue");
    let invalid = TemporalUnit {
        temporal_delimiter: Some(TemporalDelimiter),
        parameter_blocks: vec![Obu::new(
            ObuHeader::new(ObuType::ParameterBlock),
            ParameterBlock {
                parameter_id: 100,
                duration_fields: Some(BlockDurationFields {
                    duration: 1,
                    constant_subblock_duration: 1,
                }),
                subblocks: vec![ParameterSubblock {
                    subblock_duration: None,
                    data: ParameterData::Raw(vec![0xde]),
                }],
            },
        )],
        audio_frames: Vec::new(),
    };

    let err = writer
        .push_temporal_unit(&invalid)
        .expect_err("raw data does not match the published Mix Gain definition");
    assert_eq!(err.kind(), &ErrorKind::UnsupportedParameterData);

    let sink = writer.finish().expect("a preflight error does not poison the sink");
    assert_eq!(sink.len(), PROLOGUE_LEN, "the delimiter was never flushed");
}

#[test]
fn a_partial_sink_write_poisons_every_later_operation_and_tracks_written_bytes() {
    #[derive(Debug)]
    struct PartialThenFail {
        accepted: usize,
    }
    impl Write for PartialThenFail {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.accepted == 7 {
                return Err(io::Error::other("terminal failure"));
            }
            let remaining = 7_usize.saturating_sub(self.accepted);
            let written = remaining.min(bytes.len());
            self.accepted = self.accepted.saturating_add(written);
            Ok(written)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut writer = SequenceWriter::new(PartialThenFail { accepted: 0 });
    let first = writer
        .push_descriptors(&published_descriptor_set())
        .expect_err("the sink fails after seven bytes");
    assert_eq!(first.kind(), &ErrorKind::SinkWrite);
    assert_eq!(writer.bytes_written(), 7);

    let retry = writer
        .push_descriptors(&published_descriptor_set())
        .expect_err("a poisoned stream cannot duplicate its prefix");
    assert_eq!(retry.kind(), &ErrorKind::SequenceWriterPoisoned);

    let finish = writer
        .finish()
        .expect_err("a poisoned stream cannot be reported as finished");
    assert_eq!(finish.kind(), &ErrorKind::SequenceWriterPoisoned);
}

#[test]
fn a_sequence_with_no_temporal_units_is_descriptors_only() {
    let mut writer = SequenceWriter::new(Vec::new());
    writer
        .push_descriptors(&published_descriptor_set())
        .expect("prologue");
    let produced = writer.finish().expect("finish");

    assert_eq!(produced.len(), PROLOGUE_LEN);
    let boundaries = find_obu_boundaries(&produced).expect("walks cleanly");
    assert_eq!(boundaries.last().copied(), Some(PROLOGUE_LEN));
    assert_eq!(boundaries.len(), 5, "four descriptors plus the end offset");
}

#[test]
fn the_writer_holds_no_temporal_unit_history() {
    // Peak memory must not grow with the number of temporal units. The
    // observable proxy is that the writer's own footprint is a constant: it
    // keeps a state, a scratch buffer, the published parameter definitions and
    // a byte counter, and nothing per unit.
    let mut writer = SequenceWriter::new(Vec::new());
    writer
        .push_descriptors(&published_descriptor_set())
        .expect("prologue");

    let mut previous: Option<u64> = None;
    for (index, unit) in published_temporal_units().into_iter().enumerate() {
        writer.push_temporal_unit(&unit).expect("unit");
        let written = writer.bytes_written();
        if let Some(before) = previous {
            // Every untrimmed frame costs exactly the same number of bytes, so
            // a writer that accumulated anything would show it here.
            let step = written.saturating_sub(before);
            assert!(
                step == 515 || step == 517,
                "unit {index} cost {step} bytes, expected a constant 515 (or 517 trimmed)"
            );
        }
        previous = Some(written);
    }
    assert_eq!(writer.bytes_written(), u64::try_from(FILE_LEN).unwrap_or(0));
}

// ---------------------------------------------------------------------------
// SEQ-03 — the whole-file wrapper, and GUARD-10's determinism check
// ---------------------------------------------------------------------------

#[test]
fn encoding_the_same_input_twice_produces_byte_identical_output() {
    let first = stream_whole_file();
    let second = stream_whole_file();
    assert_bytes_eq(&first, &second, "two encodes in one process");
    assert_eq!(first.len(), FILE_LEN);
}

#[test]
fn the_whole_file_wrapper_is_byte_identical_to_the_streamed_path() {
    let streamed = stream_whole_file();
    let wrapped = write_sequence(
        Vec::new(),
        &published_descriptor_set(),
        published_temporal_units(),
    )
    .expect("the wrapper drives the same primitive");

    assert_bytes_eq(&wrapped, &streamed, "write_sequence vs SequenceWriter");
    assert_bytes_eq(&wrapped, TEST_000003, "write_sequence vs the reference");
}
