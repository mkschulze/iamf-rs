//! TIME-02 — BCG channel→substream packing, proven three independent ways.
//!
//! `PROJECT.md` names this the phase's **highest silent-failure risk**: get the
//! order wrong and you get a clean decode with scrambled channels — no error,
//! no crash, correct byte count. Nothing downstream of this file notices, so
//! the tests are the only detector there is (threat T-01-31).
//!
//! Research closed the 5.1 answer three ways and this file asserts all three,
//! mirroring how it was closed:
//!
//! 1. the plan's **shape** for 5.1, stereo and mono, asserted directly against
//!    the ordering rule `libiamf`'s own comment block states;
//! 2. the **byte-size signature** the plan implies — for 256 samples at 16 bit
//!    the four 5.1 substream payloads are 1024, 1024, 512 and 512 bytes;
//! 3. a **structural read of a shipped 5.1 reference file**, whose four Audio
//!    Frames are types 6, 7, 8, 9 with exactly those `obu_size` values and
//!    whose Audio Element declares `substream_count = 4`,
//!    `coupled_substream_count = 2`.

use iamf::bits::BitCursor;
use iamf::error::ErrorKind;
use iamf::model::layout::LoudspeakerLayout;
use iamf::obu::{
    AudioElementType, ObuType, find_obu_boundaries, read_audio_element, read_obu_with,
};
use iamf::packing::{SubstreamChannels, SubstreamPlan, pack_channels_to_substreams};

/// A shipped 5.1 LPCM file from the pinned reference tree.
///
/// **This is the vendored NEGATIVE fixture.** Its `num_samples_per_frame` is 0
/// and it decodes to zero samples, which is why plan 01-02 filed it under
/// `negative/`. It is used here for its **structure** only — the OBU types and
/// sizes its encoder chose for a 5.1 element, and the counts its Audio Element
/// declares — which is sound evidence about packing regardless of whether the
/// file decodes. It is never a golden, and nothing here compares PCM against
/// it.
const TONES_5P1: &[u8] = include_bytes!("fixtures/reference/negative/tones_256samp_5p1_pcm.iamf");

/// 256 samples, the frame length the shipped 5.1 file's frames carry.
const SAMPLES: usize = 256;

/// Interleaved test PCM in the decoder's documented output order for Sound
/// System B — `L, R, C, LFE, Ls, Rs` — at `bytes_per_sample` bytes per sample.
///
/// Every byte of channel `c`, sample `s` is `0xC0 | c` in its first byte and
/// the sample index in the rest, so a scrambled channel is visible by
/// inspection rather than only as a length.
fn interleaved_5p1(samples: usize, bytes_per_sample: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for sample in 0..samples {
        for channel in 0..6_usize {
            out.push(u8::try_from(0xC0 | channel).unwrap_or(0xFF));
            for byte in 1..bytes_per_sample {
                let mixed = sample.wrapping_add(byte);
                out.push(u8::try_from(mixed & 0xFF).unwrap_or(0));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// (1) The plan's shape
// ---------------------------------------------------------------------------

#[test]
fn five_one_packs_as_two_coupled_pairs_then_two_monos() {
    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };

    let shape: Vec<(ObuType, SubstreamChannels)> = plan
        .substreams()
        .iter()
        .map(|spec| (spec.obu_type, spec.channels))
        .collect();

    assert_eq!(
        shape,
        vec![
            // Coupled substreams come FIRST, then non-coupled. The source
            // indices are positions in the decoder's documented output order
            // `L, R, C, LFE, Ls, Rs`, which is NOT the packing order — that is
            // the whole of TIME-02.
            (ObuType::AudioFrameId0, SubstreamChannels::Coupled(0, 1)), // L, R
            (ObuType::AudioFrameId1, SubstreamChannels::Coupled(4, 5)), // Ls, Rs
            (ObuType::AudioFrameId2, SubstreamChannels::Mono(2)),       // C
            (ObuType::AudioFrameId3, SubstreamChannels::Mono(3)),       // LFE
        ]
    );
}

#[test]
fn five_one_declares_four_substreams_and_two_coupled() {
    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };

    assert_eq!(plan.substream_count(), 4);
    assert_eq!(plan.coupled_substream_count(), 2);
    assert_eq!(plan.channel_count(), 6);
}

#[test]
fn stereo_is_one_coupled_substream_and_mono_is_one_mono_substream() {
    let stereo = match SubstreamPlan::for_layout(LoudspeakerLayout::Stereo) {
        Ok(plan) => plan,
        Err(error) => panic!("stereo has a packing plan: {error}"),
    };
    assert_eq!(stereo.substream_count(), 1, "matches test_000003's element");
    assert_eq!(stereo.coupled_substream_count(), 1);
    assert_eq!(
        stereo.substreams().first().map(|spec| spec.channels),
        Some(SubstreamChannels::Coupled(0, 1))
    );

    let mono = match SubstreamPlan::for_layout(LoudspeakerLayout::Mono) {
        Ok(plan) => plan,
        Err(error) => panic!("mono has a packing plan: {error}"),
    };
    assert_eq!(mono.substream_count(), 1);
    assert_eq!(mono.coupled_substream_count(), 0);
    assert_eq!(
        mono.substreams().first().map(|spec| spec.channels),
        Some(SubstreamChannels::Mono(0))
    );
}

// ---------------------------------------------------------------------------
// (2) The byte-size signature and the interleaving
// ---------------------------------------------------------------------------

#[test]
fn five_one_at_256_samples_and_16_bit_reproduces_the_shipped_size_signature() {
    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };
    let pcm = interleaved_5p1(SAMPLES, 2);

    let packed = match pack_channels_to_substreams(&plan, &pcm, 6, 2) {
        Ok(packed) => packed,
        Err(error) => panic!("6 channels of 16-bit PCM pack: {error}"),
    };

    let sizes: Vec<usize> = packed.iter().map(Vec::len).collect();
    assert_eq!(
        sizes,
        vec![1024, 1024, 512, 512],
        "256 samples x 2 ch x 2 B twice, then 256 x 1 x 2 twice — exactly the \
         signature the shipped 5.1 file exhibits"
    );
}

#[test]
fn five_one_at_24_bit_scales_the_signature_by_the_sample_width() {
    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };
    let pcm = interleaved_5p1(SAMPLES, 3);

    let packed = match pack_channels_to_substreams(&plan, &pcm, 6, 3) {
        Ok(packed) => packed,
        Err(error) => panic!("6 channels of 24-bit PCM pack: {error}"),
    };

    assert_eq!(
        packed.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1536, 1536, 768, 768],
        "N * 2 * 3 for a coupled substream, N * 1 * 3 for a mono one"
    );
}

#[test]
fn a_coupled_substream_payload_is_sample_interleaved_not_planar() {
    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };
    // Four samples, one byte per sample, so the payload reads as its own map.
    let pcm = interleaved_5p1(4, 1);
    assert_eq!(pcm, [0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5].repeat(4));

    let packed = match pack_channels_to_substreams(&plan, &pcm, 6, 1) {
        Ok(packed) => packed,
        Err(error) => panic!("packs: {error}"),
    };

    assert_eq!(
        packed.first().map(Vec::as_slice),
        Some(&[0xC0, 0xC1, 0xC0, 0xC1, 0xC0, 0xC1, 0xC0, 0xC1][..]),
        "substream 0 is L0 R0 L1 R1 ... — sample-interleaved, NOT planar. The \
         decoder writes its OUTPUT planar per channel; conflating the two \
         layouts is the same class of error as the packing order itself."
    );
    assert_eq!(
        packed.get(1).map(Vec::as_slice),
        Some(&[0xC4, 0xC5, 0xC4, 0xC5, 0xC4, 0xC5, 0xC4, 0xC5][..]),
        "substream 1 is Ls, Rs — source indices 4 and 5"
    );
    assert_eq!(
        packed.get(2).map(Vec::as_slice),
        Some(&[0xC2, 0xC2, 0xC2, 0xC2][..]),
        "substream 2 is mono C — source index 2"
    );
    assert_eq!(
        packed.get(3).map(Vec::as_slice),
        Some(&[0xC3, 0xC3, 0xC3, 0xC3][..]),
        "substream 3 is mono LFE — source index 3"
    );
}

#[test]
fn a_channel_count_disagreeing_with_the_layout_is_a_typed_error() {
    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };
    let pcm = interleaved_5p1(SAMPLES, 2);

    assert_eq!(
        pack_channels_to_substreams(&plan, &pcm, 5, 2)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::ChannelCountMismatch),
        "a short or long frame would be mis-framed by the decoder, not rejected"
    );

    // A buffer that is not a whole number of interleaved frames is the same
    // defect seen from the other side.
    assert_eq!(
        pack_channels_to_substreams(&plan, pcm.get(1..).unwrap_or_default(), 6, 2)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::ChannelCountMismatch)
    );

    assert_eq!(
        pack_channels_to_substreams(&plan, &pcm, 6, 0)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::ChannelCountMismatch),
        "a zero sample width is a division by zero waiting to happen"
    );
}

// ---------------------------------------------------------------------------
// (3) The shipped reference file's own OBU size signature
// ---------------------------------------------------------------------------

#[test]
fn the_shipped_five_one_reference_file_reads_out_the_same_answer() {
    let boundaries = match find_obu_boundaries(TONES_5P1) {
        Ok(boundaries) => boundaries,
        Err(error) => panic!("the vendored 5.1 file walks: {error}"),
    };

    let mut frames: Vec<(ObuType, u32)> = Vec::new();
    for start in &boundaries {
        let Some(rest) = TONES_5P1.get(*start..) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let mut r = BitCursor::new(rest);
        let Ok((header, obu_size)) = iamf::obu::read_obu_header(&mut r) else {
            continue;
        };
        if header.obu_type.is_audio_frame() {
            frames.push((header.obu_type, obu_size));
        }
    }

    assert_eq!(
        frames,
        vec![
            (ObuType::AudioFrameId0, 1024),
            (ObuType::AudioFrameId1, 1024),
            (ObuType::AudioFrameId2, 512),
            (ObuType::AudioFrameId3, 512),
        ],
        "two coupled substreams (256 x 2 ch x 2 B) then two mono ones \
         (256 x 1 ch x 2 B), in types 6, 7, 8, 9"
    );
}

#[test]
fn the_shipped_five_one_audio_element_declares_the_counts_the_plan_derives() {
    // The Audio Element OBU is the third in the file.
    let start = match find_obu_boundaries(TONES_5P1) {
        Ok(boundaries) => boundaries.get(2).copied().unwrap_or(0),
        Err(error) => panic!("the vendored 5.1 file walks: {error}"),
    };
    let rest = TONES_5P1.get(start..).unwrap_or_default();
    let mut r = BitCursor::new(rest);

    let obu = match read_obu_with(&mut r, read_audio_element) {
        Ok(obu) => obu,
        Err(error) => panic!("the Audio Element parses: {error}"),
    };
    assert_eq!(obu.header.obu_type, ObuType::AudioElement);

    let layer = match &obu.payload.audio_element_type {
        AudioElementType::ChannelBased(config) => config
            .scalable_channel_layout
            .layers
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("the element has one layer")),
        other => panic!("the shipped 5.1 file is channel-based, not {other:?}"),
    };

    assert_eq!(layer.loudspeaker_layout, LoudspeakerLayout::Ch5_1);
    assert_eq!(obu.payload.audio_substream_ids, vec![0, 1, 2, 3]);

    let plan = match SubstreamPlan::for_layout(LoudspeakerLayout::Ch5_1) {
        Ok(plan) => plan,
        Err(error) => panic!("5.1 has a packing plan: {error}"),
    };
    assert_eq!(
        (layer.substream_count, layer.coupled_substream_count),
        (plan.substream_count(), plan.coupled_substream_count()),
        "the counts the shipped file declares and the counts our plan derives \
         are the same two numbers — they come from one source or they drift"
    );
}

// ---------------------------------------------------------------------------
// Scope boundary
// ---------------------------------------------------------------------------

#[test]
fn a_layout_this_phase_does_not_model_is_a_typed_error_not_a_guess() {
    // Being stricter than the reference is a defect class; so is inventing an
    // ordering the reference never states. Phase 1 implements the three
    // layouts whose packing research closed, and says so.
    assert_eq!(
        SubstreamPlan::for_layout(LoudspeakerLayout::Ch7_1_4)
            .err()
            .map(|e| e.kind().clone()),
        Some(ErrorKind::UnsupportedLayout)
    );
}
