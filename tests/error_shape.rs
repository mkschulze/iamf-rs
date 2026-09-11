//! Behavioural tests for the published error surface (D-08 / D-09).
//!
//! GUARD-04 carve-out: the crate denies `clippy::unwrap_used`,
//! `expect_used` and `panic` **outside tests**. That carve-out is configured
//! in `clippy.toml` (`allow-unwrap-in-tests` / `allow-expect-in-tests` /
//! `allow-panic-in-tests`) rather than by `#[allow]` attributes here, so
//! D-21's escape census — `rg 'allow.*disallowed_types' src/` must return
//! exactly one entry, PROF-03's — stays meaningful. These tests happen not to
//! need any of the three, but the carve-out is what makes `assert!` macros
//! legal in this file at all.
//!
//! These types are the ones Parallax's import adapter matches on. Changing
//! `Location`'s shape or `Finding`'s fields after Parallax depends on them is
//! a coordinated breaking change across two repositories, not a refactor —
//! which is why the shape is asserted here rather than left to review.

use iamf::obu::CodecConfig;
use iamf::{Error, ErrorKind, Finding, Location};

// ---------------------------------------------------------------------------
// Equality — position is part of the error's identity (D-08).
// ---------------------------------------------------------------------------

#[test]
fn errors_with_the_same_kind_at_different_offsets_compare_unequal() {
    let first = Error::new(ErrorKind::ObuTooLarge, Location::OutputOffset(120));
    let second = Error::new(ErrorKind::ObuTooLarge, Location::OutputOffset(121));

    assert_ne!(
        first, second,
        "two errors one byte apart must not be interchangeable: a round-trip \
         test that accepts either has stopped testing the offset"
    );
}

#[test]
fn errors_with_identical_kind_and_location_compare_equal() {
    let first = Error::new(ErrorKind::ObuTooLarge, Location::OutputOffset(120));
    let second = Error::new(ErrorKind::ObuTooLarge, Location::OutputOffset(120));

    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// Display — the offset fragment is present exactly when there is a position.
// ---------------------------------------------------------------------------

#[test]
fn an_unlocated_error_displays_without_an_offset_fragment() {
    let rendered = Error::new(ErrorKind::Leb128TooLong, Location::Unlocated).to_string();

    assert!(
        !rendered.contains("offset"),
        "Location::Unlocated is the representation for an error with no \
         position; it must not invent one. Rendered: {rendered}"
    );
    assert!(
        !rendered.is_empty(),
        "an unlocated error still renders its kind's message"
    );
}

#[test]
fn an_input_offset_error_displays_the_offset() {
    let rendered = Error::new(ErrorKind::Leb128TooLong, Location::InputOffset(0x7D35)).to_string();

    assert!(
        rendered.contains("7d35") || rendered.contains("32053"),
        "an input-offset error must render its position in decimal or hex so a \
         fuzz crash report is a ten-minute bug. Rendered: {rendered}"
    );
    assert!(
        rendered.contains("input"),
        "the three error sources (read / write / validate) must stay \
         distinguishable in the rendered message. Rendered: {rendered}"
    );
}

#[test]
fn an_output_offset_error_displays_the_offset_and_names_the_output_side() {
    let rendered = Error::new(ErrorKind::ObuSizeOverflow, Location::OutputOffset(120)).to_string();

    assert!(
        rendered.contains("120") || rendered.contains("78"),
        "an output-offset error must render its position. Rendered: {rendered}"
    );
    assert!(
        rendered.contains("output"),
        "a write-side position must not be conflated with a read-side one. \
         Rendered: {rendered}"
    );
}

#[test]
fn a_field_location_error_names_the_field_path() {
    let rendered = Error::new(
        ErrorKind::ProfileNotFound,
        Location::Field("audio_roll_distance"),
    )
    .to_string();

    assert!(
        rendered.contains("audio_roll_distance"),
        "validate() names the field that is wrong rather than a byte position \
         — this is Parallax's 'refuse with the reason named' rule. Rendered: \
         {rendered}"
    );
}

#[test]
fn an_error_reports_no_source() {
    use std::error::Error as StdError;

    let error = Error::new(ErrorKind::NotByteAligned, Location::InputOffset(8));

    assert!(
        error.source().is_none(),
        "no #[from] conversions exist (a blanket From loses the offset), so \
         there is never a wrapped source to report"
    );
}

// ---------------------------------------------------------------------------
// Accessors — the fields are private; `kind()` and `at()` are the surface.
// ---------------------------------------------------------------------------

#[test]
fn an_error_exposes_its_kind_and_location_through_accessors() {
    let error = Error::new(ErrorKind::StringTooLong, Location::InputOffset(42));

    assert_eq!(error.kind(), &ErrorKind::StringTooLong);
    assert_eq!(error.at(), Location::InputOffset(42));
}

// ---------------------------------------------------------------------------
// The size budget — asserted at compile time in src/error.rs, restated here so
// a failure reads as a named test rather than a bare `assert!` in a build log.
// ---------------------------------------------------------------------------

#[test]
fn error_fits_the_thirty_two_byte_budget() {
    let size = size_of::<Error>();

    assert!(
        size <= 32,
        "Result<T, Error> is returned from every primitive in the crate; a fat \
         error type is a cost every call pays. A variant that wants a String \
         or a Vec must box it. size_of::<Error>() = {size}"
    );
}

#[test]
fn codec_capability_errors_are_exact_compact_kinds() {
    for kind in [
        ErrorKind::SampleRateNotSupportedByCodec,
        ErrorKind::SamplesPerFrameNotSupportedByCodec,
        ErrorKind::BitsPerSampleNotSupportedByCodec,
    ] {
        let error = Error::new(kind.clone(), Location::Unlocated);
        assert_eq!(error.kind(), &kind);
        assert!(!error.to_string().is_empty());
    }
    assert!(size_of::<Error>() <= 32);
}

#[test]
fn static_builder_errors_are_compact_typed_kinds() {
    for kind in [
        ErrorKind::UnknownCodecConfigHandle,
        ErrorKind::UnknownAudioElementHandle,
        ErrorKind::UnknownSubstreamHandle,
        ErrorKind::UnknownParameterHandle,
        ErrorKind::InvalidDescriptorReference,
        ErrorKind::DuplicateDeclaration,
        ErrorKind::WireIdAllocationExhausted,
    ] {
        let error = Error::new(kind.clone(), Location::Unlocated);
        assert_eq!(error.kind(), &kind);
        assert!(!error.to_string().is_empty());
    }
    assert!(size_of::<Error>() <= 32);
}

#[test]
fn temporal_input_errors_are_compact_typed_kinds() {
    for kind in [
        ErrorKind::UnknownTemporalSubstreamHandle,
        ErrorKind::UnknownTemporalParameterHandle,
        ErrorKind::MissingTemporalSubstream,
        ErrorKind::DuplicateTemporalSubstream,
        ErrorKind::TemporalSubstreamOrderMismatch,
        ErrorKind::FrameCodecMismatch,
        ErrorKind::LpcmFrameByteAlignment,
        ErrorKind::LpcmFrameSampleCountMismatch,
    ] {
        let error = Error::new(kind.clone(), Location::Field("temporal_input"));
        assert_eq!(error.kind(), &kind);
        assert!(!error.to_string().is_empty());
    }
    assert!(size_of::<Error>() <= 32);
}

#[test]
fn fresh_opus_rejects_44_1_khz_with_the_published_typed_error() {
    let error = CodecConfig::opus(2, 960, 44_100, 312)
        .expect_err("the IAMF Opus output clock is fixed at 48 kHz");

    assert_eq!(error.kind(), &ErrorKind::SampleRateNotSupportedByCodec);
    assert_eq!(error.at(), Location::Field("sample_rate"));
    assert!(size_of::<Error>() <= 32);
}

// ---------------------------------------------------------------------------
// Finding — D-09's "all findings, each naming its field path".
// ---------------------------------------------------------------------------

#[test]
fn a_finding_carries_a_field_location_and_a_message() {
    let finding = Finding {
        at: Location::Field("audio_roll_distance"),
        message: "audio_roll_distance is 2, expected 0 for LPCM".to_owned(),
    };

    assert_eq!(finding.at, Location::Field("audio_roll_distance"));
    assert!(finding.message.contains("expected 0 for LPCM"));
}

#[test]
fn an_empty_finding_list_is_the_nothing_wrong_answer() {
    // D-09: validate() returns Vec<Finding>, not Result<(), Error>. There is
    // no Ok/Err split — one run tells an import adapter everything wrong with
    // a foreign file, and an empty Vec is how it says "nothing".
    let findings: Vec<Finding> = Vec::new();

    assert!(findings.is_empty());
}

// ---------------------------------------------------------------------------
// GUARD-05 / DEC-01.
// ---------------------------------------------------------------------------

#[test]
fn spec_version_is_the_pinned_iamf_version() {
    assert_eq!(
        iamf::SPEC_VERSION,
        "1.1.0",
        "libiamf's pinned release implements IAMF v1.1.0, and libiamf \
         accepting our output is the Core Value. The constant is never an \
         Option and never empty."
    );
}

#[test]
fn the_crate_result_alias_defaults_its_error_to_our_error_type() {
    let ok: iamf::Result<u8> = Ok(7);
    let err: iamf::Result<u8> = Err(Error::new(
        ErrorKind::UnexpectedEndOfInput,
        Location::InputOffset(0),
    ));

    assert_eq!(ok, Ok(7));
    assert!(err.is_err());
}
