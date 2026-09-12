//! The Phase 1 conformance fixtures, asserted as properties.
//!
//! The generator lives in `tests/support/fixture.rs` because three integration
//! binaries drive it and a second copy is a second thing to keep in step; this
//! target is where its properties are checked. Every assertion below
//! corresponds to a fixture property that exists to detect a specific bug — see
//! that module's documentation for which bug each one catches.
//!
//! These run **offline** with no reference binary and no container, so they are
//! part of CONF-10's always-on layer on all four byte-identity targets.

#[path = "support/fixture.rs"]
mod fixture;

use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::model::{Profile, select_minimum_profile};
use iamf::obu::{
    Layout, ObuType, Trimming, TypeSpecific, find_obu_boundaries, plan_frames, read_obu_header,
};
use iamf::packing::SubstreamPlan;
use iamf::sequence::{SequenceObu, parse_sequence};

use fixture::{
    CHANNELS_5_1, EXPECTED_TRIM_AT_END, EncodedTemporalUnit, FRAME_SIZE, Fixture, FixtureCodec,
    FrameSource, SAMPLE_FRAMES, SAMPLE_RATE, describe_channel_mismatch, peak_for, ramp_pcm,
    ramp_sample, store_sample,
};

/// The `ObuType` of every OBU in a byte slice, in order.
// GUARD-04's `allow-unwrap-in-tests` / `allow-expect-in-tests` /
// `allow-panic-in-tests` carve-out (clippy.toml) applies to `#[test]` functions
// only — a helper reachable from tests is not one. These helpers exist solely to
// build or inspect a fixture, and a failure in them is an environment or
// programming error that must stop the run loudly rather than be swallowed.
// Kept as narrow, per-function allows so a future helper does not inherit the
// exemption silently.
#[allow(clippy::expect_used)]
fn obu_types(bytes: &[u8]) -> Vec<ObuType> {
    let boundaries = find_obu_boundaries(bytes).expect("the fixture's OBU chain walks");
    let mut types = Vec::new();
    // The last boundary is `bytes.len()`, so the OBU starts are all but the last.
    let count = boundaries.len().saturating_sub(1);
    for start in boundaries.iter().take(count) {
        let rest = bytes.get(*start..).unwrap_or_default();
        let mut cursor = iamf::bits::BitCursor::new(rest);
        let (header, _obu_size) = read_obu_header(&mut cursor).expect("every OBU header parses");
        types.push(header.obu_type);
    }
    types
}

// ---------------------------------------------------------------------------
// The signal (CONF-02, D-19)
// ---------------------------------------------------------------------------

/// Every channel differs from every other at **every** sample index, which is
/// what makes a swapped channel arithmetically identifiable. A signal whose
/// channels ever coincide would make a swap invisible at those indices.
#[test]
fn the_ramp_distinguishes_every_channel_at_every_index() {
    for sample_size in [16_u8, 24] {
        let peak = peak_for(sample_size);
        let channels: usize = CHANNELS_5_1.len();
        for index in 0..1024 {
            for a in 0..channels {
                for b in a.saturating_add(1)..channels {
                    assert_ne!(
                        ramp_sample(a, index, peak),
                        ramp_sample(b, index, peak),
                        "at {sample_size} bits, channels {a} and {b} collide at sample index \
                         {index}"
                    );
                }
            }
        }
    }
}

/// The −6 dBFS cap is a property of the generator, not of one call site.
#[test]
fn the_ramp_never_exceeds_the_minus_six_dbfs_cap() {
    for sample_size in [16_u8, 24] {
        let peak = peak_for(sample_size);
        for index in 0..4096 {
            for channel in 0..CHANNELS_5_1.len() {
                let value = ramp_sample(channel, index, peak);
                assert!(
                    value.saturating_abs() <= peak,
                    "ramp({channel}, {index}) = {value} exceeds the {sample_size}-bit cap {peak}"
                );
            }
        }
    }
}

/// The ramp is not a constant and not a near-constant: it actually spans its
/// range, so a wrong byte in a multi-byte sample changes the value visibly.
#[test]
fn the_ramp_spans_its_range() {
    let peak = peak_for(24);
    let pcm = ramp_pcm(SAMPLE_FRAMES, CHANNELS_5_1.len(), peak);
    let min = pcm.iter().copied().min().unwrap_or(0);
    let max = pcm.iter().copied().max().unwrap_or(0);
    // At least a quarter of full scale of spread, and both signs present.
    assert!(min < 0, "the ramp never goes negative: min {min}");
    assert!(max > 0, "the ramp never goes positive: max {max}");
    let spread = i64::from(max).saturating_sub(i64::from(min));
    assert!(
        spread > i64::from(peak),
        "the ramp spans only {spread}, less than the cap {peak} — a wrong high byte would be \
         hard to see"
    );
}

/// Big- and little-endian storage differ in byte order and nothing else, for a
/// positive and a negative sample, against values computed by hand.
///
/// **This is the evidence DESC-03's 24-bit big-endian coverage rests on**, now
/// that waiver W-1 has moved the sample-identity fixture to little-endian: it
/// involves no permissive reader at all, which makes it stronger than the round
/// trip it replaces.
#[test]
fn store_sample_writes_the_declared_byte_order() {
    let mut be = Vec::new();
    let mut le = Vec::new();
    store_sample(0x0012_3456, 24, true, &mut be);
    store_sample(0x0012_3456, 24, false, &mut le);
    assert_eq!(be, vec![0x12, 0x34, 0x56], "24-bit big-endian");
    assert_eq!(le, vec![0x56, 0x34, 0x12], "24-bit little-endian");

    let mut negative_be = Vec::new();
    let mut negative_le = Vec::new();
    store_sample(-1, 24, true, &mut negative_be);
    store_sample(-1, 24, false, &mut negative_le);
    assert_eq!(negative_be, vec![0xFF, 0xFF, 0xFF]);
    assert_eq!(negative_le, vec![0xFF, 0xFF, 0xFF]);

    let mut most_negative = Vec::new();
    store_sample(-0x0080_0000, 24, true, &mut most_negative);
    assert_eq!(most_negative, vec![0x80, 0x00, 0x00], "24-bit minimum");

    let mut sixteen_be = Vec::new();
    let mut sixteen_le = Vec::new();
    store_sample(0x0000_1234, 16, true, &mut sixteen_be);
    store_sample(0x0000_1234, 16, false, &mut sixteen_le);
    assert_eq!(sixteen_be, vec![0x12, 0x34]);
    assert_eq!(sixteen_le, vec![0x34, 0x12]);
}

// ---------------------------------------------------------------------------
// The sample-identity fixture (CONF-02, CONF-03, CONF-05)
// ---------------------------------------------------------------------------

#[test]
fn the_sample_identity_fixture_is_5_1_at_48_khz_24_bit_little_endian() {
    let fixture = fixture::sample_identity();
    let spec = fixture.spec().expect("its spec resolves");
    assert_eq!(spec.layout, LoudspeakerLayout::Ch5_1, "5.1, not stereo");
    assert_eq!(spec.channels, 6);
    assert_eq!(spec.sample_rate, SAMPLE_RATE);
    assert_eq!(spec.sample_size, 24, "three bytes per sample");
    assert_eq!(
        spec.codec,
        FixtureCodec::Lpcm { big_endian: false },
        "little-endian, per waiver W-1: libiamf@v1.1.0's reads24be misreads 24-bit big-endian. \
         DESC-03's endianness sense lives on the endianness fixture and on \
         store_sample_writes_the_declared_byte_order."
    );
    assert_eq!(spec.num_samples_per_frame, FRAME_SIZE);
}

#[test]
fn the_sample_identity_fixture_packs_two_coupled_pairs_and_two_monos() {
    let plan = SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1).expect("5.1 has a plan");
    assert_eq!(plan.substream_count(), 4);
    assert_eq!(plan.coupled_substream_count(), 2);
    assert_eq!(
        plan.channel_count(),
        6,
        "stereo could not detect a BCG packing defect: one coupled pair, zero monos"
    );
}

#[test]
fn the_sample_identity_fixture_carries_two_layouts_and_two_loudness_blocks() {
    let fixture = fixture::sample_identity();
    let presentation = fixture
        .descriptors
        .mix_presentations
        .first()
        .expect("one Mix Presentation");
    let sub_mix = presentation.sub_mixes.first().expect("one sub-mix");
    assert_eq!(
        sub_mix.num_layouts(),
        2,
        "Sound System B is the comparison target and Sound System A is MANDATORY — iamf-tools \
         refuses to write a sub-mix without a stereo layout"
    );
    assert!(
        sub_mix.has_stereo_layout(),
        "\"Every sub-mix must have a stereo layout.\" — iamf-tools@v2.1.0 mix_presentation.cc"
    );
    let systems: Vec<Option<SoundSystem>> = sub_mix
        .layouts
        .iter()
        .map(|entry| match entry.layout {
            Layout::SoundSystem(system) => Some(system),
            _ => None,
        })
        .collect();
    assert_eq!(
        systems,
        vec![Some(SoundSystem::B0_5_0), Some(SoundSystem::A0_2_0)],
        "the comparison target is written first"
    );

    let loudness: Vec<(i16, i16)> = sub_mix
        .layouts
        .iter()
        .map(|entry| (entry.loudness.integrated, entry.loudness.digital_peak))
        .collect();
    assert_eq!(loudness.len(), 2, "two layouts means two Loudness blocks");
    assert_ne!(
        loudness.first(),
        loudness.get(1),
        "the two Loudness blocks carry DIFFERENT numbers on purpose, so swapping the two layouts \
         changes the bytes"
    );
}

#[test]
fn the_sample_identity_fixture_forces_a_non_zero_end_trim_that_differs_from_the_start_trim() {
    let fixture = fixture::sample_identity();
    let spec = fixture.spec().expect("its spec resolves");
    let total = u64::try_from(fixture.sample_frames()).unwrap_or(0);
    assert_eq!(total, u64::try_from(SAMPLE_FRAMES).unwrap_or(0));

    let remainder = total.checked_rem(u64::from(spec.num_samples_per_frame));
    assert_ne!(
        remainder,
        Some(0),
        "{total} samples is a multiple of {}, so trim_at_end would be 0 and a swapped END/START \
         write order would be invisible (OBU-05, CONF-03)",
        spec.num_samples_per_frame
    );

    let plan = plan_frames(total, spec.num_samples_per_frame).expect("the frame plan");
    assert_eq!(plan.trim_at_end, EXPECTED_TRIM_AT_END);
    assert!(plan.trim_at_end > 0, "strictly greater than 0");
    assert!(
        plan.trim_at_end < spec.num_samples_per_frame,
        "strictly less than the frame size"
    );

    let last = plan.frame_count.saturating_sub(1);
    let trimming = plan.trimming_for(last).expect("the last frame trims");
    assert_eq!(trimming.at_end, EXPECTED_TRIM_AT_END);
    assert_eq!(
        trimming.at_start, 0,
        "LPCM has no priming, so the two trim values DIFFER — the only configuration that can \
         catch a swapped END/START write order"
    );
    assert_ne!(trimming.at_end, trimming.at_start);
    // Every earlier frame trims nothing at all.
    for index in 0..last {
        assert_eq!(
            plan.trimming_for(index),
            None,
            "frame {index} trims nothing"
        );
    }
}

#[test]
fn the_sample_identity_fixture_peaks_at_or_below_minus_six_dbfs() {
    let fixture = fixture::sample_identity();
    let peak = peak_for(24);
    let observed = fixture
        .single_pcm()
        .iter()
        .map(|v| v.saturating_abs())
        .max()
        .unwrap_or(0);
    assert!(
        observed <= peak,
        "the fixture peaks at {observed}, above the -6 dBFS cap of {peak}. libiamf's -1 dBTP \
         limiter is created unconditionally and would engage."
    );
}

#[test]
fn the_sample_identity_fixture_selects_simple_profile() {
    let fixture = fixture::sample_identity();
    let elements: Vec<_> = fixture.descriptors.audio_elements.iter().collect();
    let (primary, additional) = select_minimum_profile(&elements).expect("a profile fits");
    assert_eq!(primary, Profile::Simple, "one element, six channels");
    assert_eq!(primary, additional, "always equal — libiamf enforces <=");
    assert_eq!(
        fixture.descriptors.sequence_header.primary_profile,
        primary.to_wire()
    );
}

// ---------------------------------------------------------------------------
// The endianness fixture (DESC-03, waiver W-1)
// ---------------------------------------------------------------------------

#[test]
fn the_endianness_fixture_is_stereo_16_bit_big_endian() {
    let fixture = fixture::endianness();
    let spec = fixture.spec().expect("its spec resolves");
    assert_eq!(spec.layout, LoudspeakerLayout::Stereo);
    assert_eq!(spec.sample_size, 16);
    assert_eq!(
        spec.codec,
        FixtureCodec::Lpcm { big_endian: true },
        "sample_format_flags == 0 is BIG-endian (DESC-03), and reads16be in the pinned libiamf \
         uses readu16be and is correct — which is why the endianness sense lives here"
    );
    assert_eq!(spec.sample_rate, SAMPLE_RATE);
}

#[test]
fn the_endianness_fixture_stores_its_first_sample_most_significant_byte_first() {
    let fixture = fixture::endianness();
    let bytes = fixture.encode().expect("it encodes");
    let first_sample = fixture.single_pcm().first().copied().unwrap_or(0);
    let mut expected = Vec::new();
    store_sample(first_sample, 16, true, &mut expected);
    assert_eq!(
        expected.len(),
        2,
        "16-bit is two bytes; the assertion below depends on it"
    );
    // The first Audio Frame's payload begins somewhere after the descriptors;
    // finding the exact two bytes anywhere in the file is enough to prove the
    // order, and the byte-level offsets are covered by the golden.
    assert!(
        bytes.windows(2).any(|window| window == expected.as_slice()),
        "the big-endian bytes {expected:02x?} of the first sample {first_sample} do not appear \
         in the encoded file"
    );
}

// ---------------------------------------------------------------------------
// Codec-neutral frame sources
// ---------------------------------------------------------------------------

#[test]
fn lpcm_frame_sources_preserve_the_committed_phase_1_bytes() {
    let produced = fixture::sample_identity().encode().expect("it encodes");
    let committed = include_bytes!("fixtures/golden/phase1_sample_identity.iamf");
    assert_eq!(
        produced, committed,
        "routing LPCM through FrameSource must not change a single Phase 1 byte"
    );
}

#[test]
fn pre_encoded_payloads_are_copied_exactly_and_share_each_units_trim() {
    let first_trim = Some(Trimming {
        at_end: 7,
        at_start: 3,
    });
    let units = vec![
        EncodedTemporalUnit {
            trimming: first_trim,
            substream_payloads: vec![
                vec![0x10, 0x00, 0xff],
                vec![0x21],
                vec![0x32, 0x33],
                vec![0x43, 0x44, 0x45, 0x46],
            ],
        },
        EncodedTemporalUnit {
            trimming: None,
            substream_payloads: vec![
                vec![0x50],
                vec![0x61, 0x62],
                vec![0x73, 0x74, 0x75],
                vec![0x86, 0x87, 0x88, 0x89],
            ],
        },
    ];
    let expected_payloads: Vec<Vec<u8>> = units
        .iter()
        .flat_map(|unit| unit.substream_payloads.iter().cloned())
        .collect();
    let mut fixture = fixture::sample_identity();
    fixture.frame_sources = vec![FrameSource::PreEncoded(units)];

    let parsed = parse_sequence(&fixture.encode().expect("pre-encoded units encode"))
        .expect("the encoded fixture parses");
    let frames: Vec<_> = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioFrame(frame) => Some(frame),
            _ => None,
        })
        .collect();
    let actual_payloads: Vec<Vec<u8>> = frames
        .iter()
        .map(|frame| frame.payload.payload.clone())
        .collect();
    assert_eq!(
        actual_payloads, expected_payloads,
        "opaque bytes are copied"
    );

    assert_eq!(frames.len(), 8);
    for frame in frames.iter().take(4) {
        assert_eq!(
            frame.header.type_specific,
            TypeSpecific::Trimming(first_trim),
            "every substream in temporal unit 0 carries the same trim"
        );
    }
    for frame in frames.iter().skip(4) {
        assert_eq!(
            frame.header.type_specific,
            TypeSpecific::Trimming(None),
            "every substream in temporal unit 1 carries the same trim"
        );
    }
}

#[test]
fn pre_encoded_payloads_use_each_elements_declared_substream_ids() {
    let mut fixture = fixture::structure_only();
    let declared: Vec<(u32, Vec<u32>)> = fixture
        .descriptors
        .audio_elements
        .iter()
        .map(|element| {
            (
                element.audio_element_id,
                element.audio_substream_ids.clone(),
            )
        })
        .collect();
    assert_eq!(
        declared,
        vec![(401, vec![1]), (400, vec![0])],
        "the regression requires descriptor order opposite to substream-id order"
    );
    fixture.frame_sources = vec![
        FrameSource::PreEncoded(vec![EncodedTemporalUnit {
            trimming: None,
            substream_payloads: vec![vec![0xaa]],
        }]),
        FrameSource::PreEncoded(vec![EncodedTemporalUnit {
            trimming: None,
            substream_payloads: vec![vec![0xbb]],
        }]),
    ];

    let parsed = parse_sequence(&fixture.encode().expect("pre-encoded units encode"))
        .expect("the encoded fixture parses");
    let frames: Vec<(u32, Vec<u8>)> = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioFrame(frame) => {
                Some((frame.payload.substream_id, frame.payload.payload.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        frames,
        vec![(1, vec![0xaa]), (0, vec![0xbb])],
        "each source payload must stay attached to its Audio Element's declared substream id"
    );
}

#[test]
fn an_element_and_frame_source_count_mismatch_is_an_error() {
    let mut fixture = fixture::sample_identity();
    fixture.frame_sources.clear();
    let error = fixture.encode().expect_err("one element needs one source");
    assert!(
        error.contains("1 Audio Elements but 0 frame sources"),
        "unexpected error: {error}"
    );
}

#[test]
fn temporal_unit_count_mismatches_between_elements_are_errors() {
    let one_unit = EncodedTemporalUnit {
        trimming: None,
        substream_payloads: vec![vec![0x11]],
    };
    let mut fixture = fixture::structure_only();
    fixture.frame_sources = vec![
        FrameSource::PreEncoded(vec![one_unit.clone()]),
        FrameSource::PreEncoded(vec![one_unit.clone(), one_unit]),
    ];
    let error = fixture
        .encode()
        .expect_err("all element sources need the same unit count");
    assert!(
        error.contains("element 401 has 1 temporal units") && error.contains("element 400 has 2"),
        "unexpected error: {error}"
    );
}

#[test]
fn a_pre_encoded_substream_count_mismatch_is_an_error() {
    let mut fixture = fixture::sample_identity();
    fixture.frame_sources = vec![FrameSource::PreEncoded(vec![EncodedTemporalUnit {
        trimming: None,
        substream_payloads: vec![vec![0x10], vec![0x20], vec![0x30]],
    }])];
    let error = fixture
        .encode()
        .expect_err("5.1 declares four substreams per temporal unit");
    assert!(
        error.contains("element 300 temporal unit 0 has 3 substream payloads but declares 4"),
        "unexpected error: {error}"
    );
}

// ---------------------------------------------------------------------------
// The structure-only fixture (CONF-04, PROF-02)
// ---------------------------------------------------------------------------

#[test]
fn the_structure_only_fixture_has_two_audio_elements() {
    let fixture = fixture::structure_only();
    assert_eq!(fixture.descriptors.audio_elements.len(), 2);
    assert_eq!(fixture.descriptors.codec_configs.len(), 1);
    let findings = fixture.descriptors.validate();
    assert_eq!(findings.len(), 3, "unexpected findings: {findings:?}");
    assert!(findings.iter().all(|finding| {
        finding.message.contains("parameter_id 100")
            && finding.message.contains("first in bitstream order")
    }));
}

#[test]
fn the_structure_only_fixture_selects_base_profile() {
    let fixture = fixture::structure_only();
    let elements: Vec<_> = fixture.descriptors.audio_elements.iter().collect();
    let (primary, additional) = select_minimum_profile(&elements).expect("a profile fits");
    assert_eq!(
        primary,
        Profile::Base,
        "two Audio Elements push the minimum profile Simple -> Base; four channels would have \
         fit Simple twice over, and the element count is what decides"
    );
    assert_eq!(primary, additional);
    assert_eq!(
        fixture.descriptors.sequence_header.primary_profile,
        Profile::Base.to_wire(),
        "the fixture's header carries the selected profile, not a hard-coded one"
    );
}

#[test]
fn the_structure_only_fixture_writes_its_audio_elements_ascending_by_id() {
    let fixture = fixture::structure_only();
    // Declared 401-first on purpose, so a writer that emitted list order would
    // be caught here.
    let declared: Vec<u32> = fixture
        .descriptors
        .audio_elements
        .iter()
        .map(|element| element.audio_element_id)
        .collect();
    assert_eq!(declared, vec![401, 400], "declared in descending order");

    let bytes = fixture.encode().expect("it encodes");
    let first = bytes.windows(1).len().min(bytes.len());
    let _ = first;
    // The two Audio Element OBUs, in the order they were written.
    let boundaries = find_obu_boundaries(&bytes).expect("the OBU chain walks");
    let mut written = Vec::new();
    let count = boundaries.len().saturating_sub(1);
    for start in boundaries.iter().take(count) {
        let rest = bytes.get(*start..).unwrap_or_default();
        let mut cursor = iamf::bits::BitCursor::new(rest);
        let obu = iamf::obu::read_obu_with(&mut cursor, iamf::obu::read_audio_element);
        if let Ok(element) = obu {
            written.push(element.payload.audio_element_id);
        }
    }
    // Only the Audio Element OBUs parse as Audio Elements; the descriptor order
    // is what is under test.
    assert!(
        written.windows(2).all(|pair| pair.first() < pair.get(1)),
        "Audio Elements were written as {written:?}, which is not ascending by id (DESC-08)"
    );
}

#[test]
fn the_structure_only_fixture_carries_at_least_six_obus_with_two_audio_elements() {
    let fixture = fixture::structure_only();
    let bytes = fixture.encode().expect("it encodes");
    let types = obu_types(&bytes);
    assert!(
        types.len() >= 6,
        "CONF-04 needs at least six OBUs so ordering is observable; got {}",
        types.len()
    );
    let elements = types
        .iter()
        .filter(|obu_type| **obu_type == ObuType::AudioElement)
        .count();
    assert_eq!(elements, 2, "two Audio Elements, so ordering is observable");
    assert_eq!(
        types.first(),
        Some(&ObuType::IaSequenceHeader),
        "the IA Sequence Header is always first"
    );
    assert_eq!(
        types.get(1),
        Some(&ObuType::CodecConfig),
        "Codec Configs follow the header"
    );
}

// ---------------------------------------------------------------------------
// The Opus fixture (CODEC-02 through CODEC-05)
// ---------------------------------------------------------------------------

/// The committed raw Opus packets must enter the ordinary fixture adapter in
/// manifest order.  Moving priming to the final frame, equalising L and E, or
/// swapping the header's END/START fields changes the audible timeline while
/// still producing a structurally valid IAMF sequence.
#[test]
fn opus_fixture_keeps_manifest_packets_and_asymmetric_priming_on_the_wire() {
    let corpus = fixture::load_opus_corpus();
    let fixture = fixture::opus();
    let spec = fixture.spec().expect("Opus spec resolves");
    let config = fixture
        .descriptors
        .codec_configs
        .first()
        .expect("one Opus Codec Config");
    let opus = config.opus_config().expect("typed Opus config");

    assert_eq!(fixture.name, "phase3_opus");
    assert_eq!(spec.layout, LoudspeakerLayout::Stereo);
    assert_eq!(spec.channels, 2);
    assert_eq!(spec.sample_rate, 48_000);
    assert_eq!(spec.sample_size, 16);
    assert_eq!(spec.num_samples_per_frame, 960);
    assert_eq!(spec.codec, FixtureCodec::Opus);
    assert_eq!(config.audio_roll_distance, -4);
    assert_eq!(opus.version, 1);
    assert_eq!(opus.output_channel_count, 2);
    assert_eq!(
        opus.pre_skip,
        u16::try_from(corpus.lookahead).expect("u16 lookahead")
    );
    assert_eq!(opus.input_sample_rate, 48_000);
    assert_eq!(opus.output_gain, 0);
    assert_eq!(opus.mapping_family, 0);
    assert_eq!(fixture.single_pcm(), corpus.libiamf_expected);
    assert_eq!(fixture.sample_frames(), corpus.source_frames);

    let source = fixture.frame_sources.first().expect("one frame source");
    let FrameSource::PreEncoded(units) = source else {
        panic!("Opus packets must stay opaque pre-encoded payloads");
    };
    assert_eq!(units.len(), corpus.packets);
    assert_eq!(units, &corpus.units, "packets remain in manifest order");
    assert_ne!(
        corpus.lookahead, corpus.end_trim,
        "asymmetric trims catch a swap"
    );
    assert_eq!(
        units.first().and_then(|unit| unit.trimming),
        Some(Trimming {
            at_end: 0,
            at_start: u32::try_from(corpus.lookahead).expect("u32 lookahead"),
        })
    );
    assert_eq!(
        units.last().and_then(|unit| unit.trimming),
        Some(Trimming {
            at_end: u32::try_from(corpus.end_trim).expect("u32 end trim"),
            at_start: 0,
        })
    );
    for unit in units.iter().skip(1).take(units.len().saturating_sub(2)) {
        assert_eq!(unit.trimming, None, "middle packets have no trim fields");
    }

    let bytes = fixture.encode().expect("Opus fixture encodes");
    let parsed = parse_sequence(&bytes).expect("Opus IAMF parses");
    let frames = parsed
        .obus
        .iter()
        .filter_map(|obu| match obu {
            SequenceObu::AudioFrame(frame) => Some(frame),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), corpus.packets);
    for (frame, unit) in frames.iter().zip(&corpus.units) {
        assert_eq!(
            frame.payload.payload.as_slice(),
            unit.substream_payloads
                .first()
                .expect("one stereo Opus packet")
                .as_slice()
        );
        assert_eq!(
            frame.header.type_specific,
            TypeSpecific::Trimming(unit.trimming)
        );
    }

    let boundaries = find_obu_boundaries(&bytes).expect("Opus OBU boundaries");
    let mut wire_trims = Vec::new();
    for start in boundaries.iter().take(boundaries.len().saturating_sub(1)) {
        let mut cursor = iamf::bits::BitCursor::new(bytes.get(*start..).unwrap_or_default());
        let byte = cursor.read_unsigned(8).expect("OBU header byte");
        let _size = cursor.read_uleb128().expect("OBU size");
        let obu_type = (byte >> 3) & 0x1f;
        if (5..=23).contains(&obu_type) {
            if byte & 0x02 != 0 {
                let end = cursor.read_uleb128().expect("END trim is first");
                let start = cursor.read_uleb128().expect("START trim is second");
                wire_trims.push(Some(Trimming {
                    at_end: end,
                    at_start: start,
                }));
            } else {
                wire_trims.push(None);
            }
        }
    }
    assert_eq!(
        wire_trims,
        corpus
            .units
            .iter()
            .map(|unit| unit.trimming)
            .collect::<Vec<_>>(),
        "the encoded trim fields decode END then START for each temporal unit"
    );
}

/// The independent Opus decoder and the pinned libiamf reference decoder each
/// retain their own exact PCM oracle.  The reference path differs from the
/// standalone libopus 1.6.1 decoder at four documented rounding points.
#[test]
fn opus_fixture_uses_the_pinned_libiamf_pcm_oracle() {
    let corpus = fixture::load_opus_corpus();
    let fixture = fixture::opus();

    let differences = fixture
        .single_pcm()
        .iter()
        .zip(&corpus.expected)
        .enumerate()
        .filter(|(_, (reference, standalone))| reference != standalone)
        .map(|(index, (reference, standalone))| (index, *standalone, *reference))
        .collect::<Vec<_>>();

    assert_eq!(
        differences,
        [
            (887, 1433, 1432),
            (1642, -5421, -5422),
            (2798, -1473, -1474),
            (3008, -7284, -7285),
        ]
    );
}

// ---------------------------------------------------------------------------
// Properties every fixture shares
// ---------------------------------------------------------------------------

#[test]
fn every_fixture_encodes_with_only_the_intentional_shared_parameter_id_findings() {
    for fixture in fixture::all() {
        let findings = fixture.descriptors.validate();
        assert!(
            findings.iter().all(|finding| {
                finding.message.contains("parameter_id 100")
                    && finding.message.contains("first in bitstream order")
            }),
            "{} has unexpected descriptor findings: {findings:?}",
            fixture.name
        );
        let bytes = fixture.encode().unwrap_or_else(|e| {
            panic!("{} does not encode: {e}", fixture.name);
        });
        assert!(!bytes.is_empty(), "{} encoded to nothing", fixture.name);
    }
}

#[test]
fn every_fixture_walks_its_obu_chain_to_exactly_its_own_length() {
    for fixture in fixture::all() {
        let bytes = fixture.encode().expect("it encodes");
        let boundaries = find_obu_boundaries(&bytes).expect("the OBU chain walks");
        assert_eq!(
            boundaries.last().copied(),
            Some(bytes.len()),
            "{}: the final boundary is {:?}, not bytes.len() = {} (OBU-08)",
            fixture.name,
            boundaries.last(),
            bytes.len()
        );
    }
}

/// Two builds of the same fixture are equal as models, which is the precondition
/// for GUARD-10's byte-level double-encode meaning anything.
#[test]
fn every_fixture_builds_identically_twice() {
    let build =
        |index: usize| -> Fixture { fixture::all().into_iter().nth(index).expect("a fixture") };
    for index in 0..fixture::all().len() {
        let first = build(index);
        let second = build(index);
        assert_eq!(
            first.descriptors, second.descriptors,
            "{} builds two different descriptor sets",
            first.name
        );
        assert_eq!(
            first.expected_pcm, second.expected_pcm,
            "{} builds two different signals",
            first.name
        );
        assert_eq!(
            first.frame_sources, second.frame_sources,
            "{} builds two different frame sources",
            first.name
        );
    }
}

// ---------------------------------------------------------------------------
// D-19 — the diagnostic reports a permutation and never applies one
// ---------------------------------------------------------------------------

#[test]
fn the_diagnostic_is_silent_when_the_pcm_matches() {
    let pcm = ramp_pcm(64, 6, peak_for(24));
    assert_eq!(
        describe_channel_mismatch(&pcm, &pcm, LoudspeakerLayout::Ch5_1),
        None
    );
}

#[test]
fn the_diagnostic_names_the_swapped_channels_by_position_and_label() {
    let channels = CHANNELS_5_1.len();
    let frames = 64;
    let expected = ramp_pcm(frames, channels, peak_for(24));

    // Swap Ls (ch4) and Rs (ch5) — the exact failure a BCG packing defect
    // produces: a clean decode, correct length, wrong slots.
    let mut scrambled = expected.clone();
    for frame in 0..frames {
        let base = frame.saturating_mul(channels);
        let a = base.saturating_add(4);
        let b = base.saturating_add(5);
        scrambled.swap(a, b);
    }

    let message = describe_channel_mismatch(&expected, &scrambled, LoudspeakerLayout::Ch5_1)
        .expect("a swap is a mismatch");
    assert!(
        message.contains("channels scrambled"),
        "the diagnostic reported {message:?} rather than naming the swap"
    );
    assert!(message.contains("ch4"), "it names the source channel");
    assert!(message.contains("ch5"), "it names the target channel");
    assert!(message.contains("Ls"), "it names the source by label");
    assert!(message.contains("Rs"), "it names the target by label");
    assert!(
        message.contains("src/packing.rs"),
        "it points at where the defect actually is, not at the harness"
    );
}

#[test]
fn the_diagnostic_reports_a_plain_difference_when_no_permutation_explains_it() {
    let channels = CHANNELS_5_1.len();
    let expected = ramp_pcm(64, channels, peak_for(24));
    let mut corrupted = expected.clone();
    if let Some(slot) = corrupted.get_mut(7) {
        *slot = slot.saturating_add(1);
    }
    let message = describe_channel_mismatch(&expected, &corrupted, LoudspeakerLayout::Ch5_1)
        .expect("a corrupted sample is a mismatch");
    assert!(
        message.contains("no channel permutation explains it"),
        "got {message:?}"
    );
    assert!(
        message.contains("frame 1"),
        "it locates the frame: {message}"
    );
    assert!(message.contains("ch1"), "it locates the channel: {message}");
}

/// The harness carries no permutation constant, no tolerance and no gain.
///
/// A source-level assertion, because this is a property of the *text* of the
/// harness rather than of its behaviour: the whole point of D-19 is that there
/// is no knob for a failing comparison to be tuned against, and a knob added
/// later would not fail any behavioural test — it would make one pass.
#[test]
fn the_harness_source_contains_no_permutation_constant_or_tolerance() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    for name in ["support/fixture.rs", "conformance.rs", "golden.rs"] {
        let path = root.join(name);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        // Strip comments and doc comments — the prohibition is discussed at
        // length in those, and it is the *code* that must be free of it.
        let code: String = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("*")
            })
            .collect::<Vec<_>>()
            .join("\n");
        for banned in [
            "CHANNEL_PERMUTATION",
            "PERMUTATION:",
            "TOLERANCE",
            "EPSILON",
            "abs_diff() <=",
            "GAIN_ADJUST",
        ] {
            assert!(
                !code.contains(banned),
                "{name} contains {banned:?}. D-19: no permutation constant, no tolerance window \
                 and no gain adjustment anywhere in the harness. A failing comparison is \
                 evidence; the correct responses are to find the defect or to file a written \
                 D-17 waiver."
            );
        }
    }
}
