//! Every successful low-level write re-reads as the same model.
//!
//! The 2026-09-13 codebase review and quick 260914-5c5 research confirmed seven
//! model states that the `iamf::obu` writers accepted although their bytes
//! re-read as a different model, or not at all: loudness extension bits outside
//! the `0xFC` mask (B1), Audio Element extension params aliasing Demixing or
//! Recon Gain (B2), reserved Audio Element types 0 and 1 (B3), ambisonics
//! length mismatches and reserved modes 0 and 1 (B4), a constant subblock
//! duration beside explicit subblock durations (B5), reserved layout types 2
//! and 3 (B6) and reserved OBU types aliasing defined ones (B7). Each is now
//! refused before emission. The controls prove that states which already round
//! trip stay writable, and the properties prove "write `Ok` implies an equal
//! re-read" without filtering the aliases out of their strategies.

#[path = "support/test_000003.rs"]
mod support;

use iamf::bits::{BitCursor, BitWriter};
use iamf::error::{Error, ErrorKind, Location};
use iamf::model::layout::{
    AmbisonicsConfig, AmbisonicsMonoConfig, AmbisonicsProjectionConfig, SoundSystem,
};
use iamf::obu::{
    AudioElement, AudioElementParam, AudioElementType, DurationFields, Layout, LoudnessExtension,
    MixPresentation, ObuHeader, ObuType, ParamDefinition, read_audio_element,
    read_mix_presentation, read_obu_header, read_param_definition, write_audio_element,
    write_mix_presentation, write_obu, write_param_definition,
};
use iamf::sequence::{SequenceObu, parse_sequence};
use proptest::prelude::*;
use support::{published_audio_element, published_mix_presentation};

type Outcome = (ErrorKind, Location);

fn outcome(error: &Error) -> Outcome {
    (error.kind().clone(), error.at())
}

/// Serialise with one writer into a fresh `BitWriter`.
fn written(write: impl FnOnce(&mut BitWriter) -> iamf::Result<()>) -> iamf::Result<Vec<u8>> {
    let mut writer = BitWriter::new();
    write(&mut writer)?;
    writer.finish()
}

/// Re-read `bytes` with one reader, returning the model and the unread byte
/// count.
fn re_read<T>(
    bytes: &[u8],
    read: impl FnOnce(&mut BitCursor<'_>) -> iamf::Result<T>,
) -> iamf::Result<(T, usize)> {
    let mut cursor = BitCursor::new(bytes);
    let value = read(&mut cursor)?;
    Ok((value, cursor.bytes_remaining()))
}

fn scene_based_element() -> Option<AudioElement> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/reference/iamf-tools/tones_100ms_3OA_stereo_opus.iamf"
    ))
    .ok()?;
    let parsed = parse_sequence(&bytes).ok()?;
    parsed.obus.into_iter().find_map(|obu| match obu {
        SequenceObu::AudioElement(element)
            if element
                .payload
                .audio_element_type
                .scene_based_config()
                .is_some() =>
        {
            Some(element.payload)
        }
        _ => None,
    })
}

fn mix_presentation_write(value: &MixPresentation) -> iamf::Result<Vec<u8>> {
    written(|w| write_mix_presentation(w, value))
}

fn audio_element_write(value: &AudioElement) -> iamf::Result<Vec<u8>> {
    written(|w| write_audio_element(w, value))
}

fn obu_type_write(value: u8) -> iamf::Result<Vec<u8>> {
    written(|w| write_obu(w, &ObuHeader::new(ObuType::Reserved(value)), &[]))
}

fn param_definition_write(value: &ParamDefinition) -> iamf::Result<Vec<u8>> {
    written(|w| write_param_definition(w, value))
}

// B1
#[test]
fn loudness_extension_bits_outside_the_extension_mask_are_refused() {
    for bits in [0x00, 0x01, 0x85] {
        let mut presentation = published_mix_presentation().payload;
        let layout = presentation
            .sub_mixes
            .first_mut()
            .and_then(|sub_mix| sub_mix.layouts.first_mut())
            .expect("published presentation has one layout");
        layout.loudness.extension = Some(LoudnessExtension {
            info_type_bits: bits,
            bytes: vec![1, 170],
        });
        let error = mix_presentation_write(&presentation).expect_err("bits outside 0xFC");
        assert_eq!(
            outcome(&error),
            (ErrorKind::GatedFieldMismatch, Location::Field("info_type")),
            "info_type_bits {bits:#04x}"
        );
    }
}

#[test]
fn loudness_extension_bits_0x84_write_and_re_read_equal() {
    let mut presentation = published_mix_presentation().payload;
    let layout = presentation
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.layouts.first_mut())
        .expect("published presentation has one layout");
    layout.loudness.extension = Some(LoudnessExtension {
        info_type_bits: 0x84,
        bytes: vec![1, 170],
    });
    let bytes = mix_presentation_write(&presentation).expect("0x84 is inside the mask");
    assert_eq!(
        re_read(&bytes, read_mix_presentation),
        Ok((presentation, 0))
    );
}

// B2
#[test]
fn audio_element_extension_params_aliasing_demixing_or_recon_gain_are_refused() {
    for param_definition_type in [1, 2] {
        let mut element = published_audio_element().payload;
        element.params = vec![AudioElementParam::Extension {
            param_definition_type,
            bytes: vec![0x00],
        }];
        let error = audio_element_write(&element).expect_err("type 1 and 2 are defined");
        assert_eq!(
            outcome(&error),
            (
                ErrorKind::ReservedAliasesDefinedValue,
                Location::Field("param_definition_type")
            ),
            "param_definition_type {param_definition_type}"
        );
    }
}

#[test]
fn audio_element_extension_params_of_type_0_and_3_write_and_re_read_equal() {
    for param_definition_type in [0, 3] {
        let mut element = published_audio_element().payload;
        element.params = vec![AudioElementParam::Extension {
            param_definition_type,
            bytes: vec![0x00],
        }];
        let bytes = audio_element_write(&element).expect("types 0 and 3 re-read as Extension");
        assert_eq!(re_read(&bytes, read_audio_element), Ok((element, 0)));
    }
}

// B3
#[test]
fn reserved_audio_element_types_0_and_1_are_refused() {
    for value in [0, 1] {
        let mut element = published_audio_element().payload;
        element.audio_element_type = AudioElementType::Reserved {
            value,
            raw: vec![0xaa],
        };
        let error = audio_element_write(&element).expect_err("types 0 and 1 are defined");
        assert_eq!(
            outcome(&error),
            (
                ErrorKind::ReservedAliasesDefinedValue,
                Location::Field("audio_element_type")
            ),
            "audio_element_type {value}"
        );
    }
}

#[test]
fn reserved_audio_element_type_2_round_trips_and_8_still_exceeds_width() {
    let mut element = published_audio_element().payload;
    element.audio_element_type = AudioElementType::Reserved {
        value: 2,
        raw: vec![0xaa],
    };
    let bytes = audio_element_write(&element).expect("type 2 is reserved");
    assert_eq!(
        re_read(&bytes, read_audio_element),
        Ok((element.clone(), 0))
    );

    element.audio_element_type = AudioElementType::Reserved {
        value: 8,
        raw: vec![0xaa],
    };
    let error = audio_element_write(&element).expect_err("8 does not fit 3 bits");
    assert_eq!(error.kind(), &ErrorKind::ValueExceedsWidth { bits: 3 });
}

// B4
#[test]
fn ambisonics_mono_mapping_length_mismatch_is_refused() {
    let mut element = scene_based_element().expect("the 3OA fixture has a scene-based element");
    let AudioElementType::SceneBased { 0: config, .. } = &mut element.audio_element_type else {
        panic!("the element is scene based");
    };
    *config = AmbisonicsConfig::Mono(AmbisonicsMonoConfig {
        output_channel_count: 4,
        substream_count: 4,
        channel_mapping: vec![0, 1, 2],
    });
    let error = audio_element_write(&element).expect_err("mapping is one entry short");
    assert_eq!(
        outcome(&error),
        (
            ErrorKind::GatedFieldMismatch,
            Location::Field("channel_mapping")
        )
    );
}

#[test]
fn ambisonics_projection_matrix_length_mismatch_is_refused() {
    let mut element = scene_based_element().expect("the 3OA fixture has a scene-based element");
    let AudioElementType::SceneBased { 0: config, .. } = &mut element.audio_element_type else {
        panic!("the element is scene based");
    };
    // (1 + 0) * 2 = 2 entries are gated, 3 are stored.
    *config = AmbisonicsConfig::Projection(AmbisonicsProjectionConfig {
        output_channel_count: 2,
        substream_count: 1,
        coupled_substream_count: 0,
        demixing_matrix: vec![1, 2, 3],
    });
    let error = audio_element_write(&element).expect_err("matrix has one extra entry");
    assert_eq!(
        outcome(&error),
        (
            ErrorKind::GatedFieldMismatch,
            Location::Field("demixing_matrix")
        )
    );
}

#[test]
fn reserved_ambisonics_modes_0_and_1_are_refused() {
    for mode in [0, 1] {
        let mut element = scene_based_element().expect("the 3OA fixture has a scene-based element");
        let AudioElementType::SceneBased { 0: config, .. } = &mut element.audio_element_type else {
            panic!("the element is scene based");
        };
        *config = AmbisonicsConfig::Reserved { mode };
        let error = audio_element_write(&element).expect_err("modes 0 and 1 are defined");
        assert_eq!(
            outcome(&error),
            (
                ErrorKind::ReservedAliasesDefinedValue,
                Location::Field("ambisonics_mode")
            ),
            "ambisonics_mode {mode}"
        );
    }
}

#[test]
fn the_parsed_3oa_scene_based_element_writes_and_re_reads_equal() {
    let element = scene_based_element().expect("the 3OA fixture has a scene-based element");
    let bytes = audio_element_write(&element).expect("a parsed element writes");
    assert_eq!(re_read(&bytes, read_audio_element), Ok((element, 0)));
}

#[test]
fn reserved_ambisonics_mode_2_writes_and_re_reads_equal() {
    let mut element = scene_based_element().expect("the 3OA fixture has a scene-based element");
    let AudioElementType::SceneBased { 0: config, .. } = &mut element.audio_element_type else {
        panic!("the element is scene based");
    };
    *config = AmbisonicsConfig::Reserved { mode: 2 };
    let bytes = audio_element_write(&element).expect("mode 2 is reserved");
    assert_eq!(re_read(&bytes, read_audio_element), Ok((element, 0)));
}

// B5
#[test]
fn constant_subblock_duration_with_explicit_subblock_durations_is_refused() {
    let definition = ParamDefinition {
        parameter_id: 1,
        parameter_rate: 48_000,
        reserved: 0,
        duration_fields: Some(DurationFields {
            duration: 8,
            constant_subblock_duration: 8,
            subblock_durations: vec![4, 4],
        }),
    };
    let error = param_definition_write(&definition).expect_err("durations are not written");
    assert_eq!(
        outcome(&error),
        (
            ErrorKind::GatedFieldMismatch,
            Location::Field("subblock_durations")
        )
    );
}

#[test]
fn param_definition_duration_fields_that_round_trip_are_accepted() {
    for (constant_subblock_duration, subblock_durations) in [(0, vec![4, 4]), (8, Vec::new())] {
        let definition = ParamDefinition {
            parameter_id: 1,
            parameter_rate: 48_000,
            reserved: 0,
            duration_fields: Some(DurationFields {
                duration: 8,
                constant_subblock_duration,
                subblock_durations,
            }),
        };
        let bytes = param_definition_write(&definition).expect("consistent duration fields");
        assert_eq!(re_read(&bytes, read_param_definition), Ok((definition, 0)));
    }
}

// B6
#[test]
fn reserved_layout_types_2_and_3_are_refused() {
    for layout_type in [2, 3] {
        let mut presentation = published_mix_presentation().payload;
        let layout = presentation
            .sub_mixes
            .first_mut()
            .and_then(|sub_mix| sub_mix.layouts.first_mut())
            .expect("published presentation has one layout");
        layout.layout = Layout::Reserved(layout_type);
        let error = mix_presentation_write(&presentation).expect_err("types 2 and 3 are defined");
        assert_eq!(
            outcome(&error),
            (
                ErrorKind::ReservedAliasesDefinedValue,
                Location::Field("layout_type")
            ),
            "layout_type {layout_type}"
        );
    }
}

#[test]
fn reserved_layout_types_0_and_1_round_trip_and_4_still_exceeds_width() {
    for layout_type in [0, 1] {
        let mut presentation = published_mix_presentation().payload;
        let layout = presentation
            .sub_mixes
            .first_mut()
            .and_then(|sub_mix| sub_mix.layouts.first_mut())
            .expect("published presentation has one layout");
        layout.layout = Layout::Reserved(layout_type);
        let bytes = mix_presentation_write(&presentation).expect("types 0 and 1 are reserved");
        assert_eq!(
            re_read(&bytes, read_mix_presentation),
            Ok((presentation, 0))
        );
    }

    let mut presentation = published_mix_presentation().payload;
    let layout = presentation
        .sub_mixes
        .first_mut()
        .and_then(|sub_mix| sub_mix.layouts.first_mut())
        .expect("published presentation has one layout");
    layout.layout = Layout::Reserved(4);
    let error = mix_presentation_write(&presentation).expect_err("4 does not fit 2 bits");
    assert_eq!(error.kind(), &ErrorKind::ValueExceedsWidth { bits: 2 });
}

// B7
#[test]
fn reserved_obu_types_aliasing_defined_types_are_refused() {
    for value in [0, 3, 23, 31] {
        let error = obu_type_write(value).expect_err("obu_type aliases a defined type");
        assert_eq!(
            outcome(&error),
            (
                ErrorKind::ReservedAliasesDefinedValue,
                Location::Field("obu_type")
            ),
            "obu_type {value}"
        );
    }
}

#[test]
fn reserved_obu_types_24_to_30_round_trip_and_32_still_exceeds_width() {
    for value in 24..=30 {
        let header = ObuHeader::new(ObuType::Reserved(value));
        let bytes = obu_type_write(value).expect("24..=30 are reserved");
        assert_eq!(re_read(&bytes, read_obu_header), Ok(((header, 0), 0)));
    }
    let error = obu_type_write(32).expect_err("32 does not fit 5 bits");
    assert_eq!(error.kind(), &ErrorKind::ValueExceedsWidth { bits: 5 });
}

fn layout_strategy() -> impl Strategy<Value = Layout> {
    prop_oneof![
        (0_u8..16).prop_map(|value| Layout::SoundSystem(SoundSystem::from_value(value))),
        Just(Layout::Binaural),
        (0_u8..=5).prop_map(Layout::Reserved),
    ]
}

fn ambisonics_strategy() -> impl Strategy<Value = AmbisonicsConfig> {
    prop_oneof![
        (
            0_u8..=4,
            0_u8..=4,
            proptest::collection::vec(any::<u8>(), 0..=5)
        )
            .prop_map(|(output_channel_count, substream_count, channel_mapping)| {
                AmbisonicsConfig::Mono(AmbisonicsMonoConfig {
                    output_channel_count,
                    substream_count,
                    channel_mapping,
                })
            }),
        (
            0_u8..=3,
            0_u8..=2,
            0_u8..=2,
            proptest::collection::vec(any::<i16>(), 0..=12)
        )
            .prop_map(
                |(
                    output_channel_count,
                    substream_count,
                    coupled_substream_count,
                    demixing_matrix,
                )| {
                    AmbisonicsConfig::Projection(AmbisonicsProjectionConfig {
                        output_channel_count,
                        substream_count,
                        coupled_substream_count,
                        demixing_matrix,
                    })
                }
            ),
        (0_u32..4).prop_map(|mode| AmbisonicsConfig::Reserved { mode }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn obu_type_write_ok_implies_equal_re_read(value in 0_u8..=40) {
        if let Ok(bytes) = obu_type_write(value) {
            let header = ObuHeader::new(ObuType::Reserved(value));
            prop_assert_eq!(re_read(&bytes, read_obu_header), Ok(((header, 0), 0)));
        }
    }

    #[test]
    fn mix_presentation_write_ok_implies_equal_re_read(
        layout in layout_strategy(),
        extension in proptest::option::of((any::<u8>(), proptest::collection::vec(any::<u8>(), 0..4))),
    ) {
        let mut presentation = published_mix_presentation().payload;
        let Some(target) = presentation
            .sub_mixes
            .first_mut()
            .and_then(|sub_mix| sub_mix.layouts.first_mut())
        else {
            return Err(TestCaseError::fail("published presentation has one layout"));
        };
        target.layout = layout;
        target.loudness.extension =
            extension.map(|(info_type_bits, bytes)| LoudnessExtension { info_type_bits, bytes });
        if let Ok(bytes) = mix_presentation_write(&presentation) {
            prop_assert_eq!(re_read(&bytes, read_mix_presentation), Ok((presentation, 0)));
        }
    }

    #[test]
    fn audio_element_write_ok_implies_equal_re_read(
        reserved_type in proptest::option::of((0_u8..=9, proptest::collection::vec(any::<u8>(), 0..4))),
        params in proptest::collection::vec(
            (0_u32..=6, proptest::collection::vec(any::<u8>(), 0..4)),
            0..3,
        ),
    ) {
        let mut element = published_audio_element().payload;
        if let Some((value, raw)) = reserved_type {
            element.audio_element_type = AudioElementType::Reserved { value, raw };
        }
        element.params = params
            .into_iter()
            .map(|(param_definition_type, bytes)| AudioElementParam::Extension {
                param_definition_type,
                bytes,
            })
            .collect();
        if let Ok(bytes) = audio_element_write(&element) {
            prop_assert_eq!(re_read(&bytes, read_audio_element), Ok((element, 0)));
        }
    }

    #[test]
    fn param_definition_write_ok_implies_equal_re_read(
        parameter_id in any::<u32>(),
        parameter_rate in any::<u32>(),
        reserved in 0_u8..=127,
        duration_fields in proptest::option::of((
            any::<u32>(),
            0_u32..3,
            proptest::collection::vec(any::<u32>(), 0..4),
        )),
    ) {
        let definition = ParamDefinition {
            parameter_id,
            parameter_rate,
            reserved,
            duration_fields: duration_fields.map(
                |(duration, constant_subblock_duration, subblock_durations)| DurationFields {
                    duration,
                    constant_subblock_duration,
                    subblock_durations,
                },
            ),
        };
        if let Ok(bytes) = param_definition_write(&definition) {
            prop_assert_eq!(re_read(&bytes, read_param_definition), Ok((definition, 0)));
        }
    }

    #[test]
    fn ambisonics_write_ok_implies_equal_re_read(config in ambisonics_strategy()) {
        let Some(mut element) = scene_based_element() else {
            return Err(TestCaseError::fail("the 3OA fixture has a scene-based element"));
        };
        let AudioElementType::SceneBased { 0: target, .. } = &mut element.audio_element_type else {
            return Err(TestCaseError::fail("the element is scene based"));
        };
        *target = config;
        if let Ok(bytes) = audio_element_write(&element) {
            prop_assert_eq!(re_read(&bytes, read_audio_element), Ok((element, 0)));
        }
    }
}
