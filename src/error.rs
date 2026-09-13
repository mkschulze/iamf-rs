//! The published error surface (D-08 / D-09).
//!
//! One [`Error`] type everywhere, carrying its position exactly once in a
//! [`Location`] rather than repeating a byte `offset` into every
//! [`ErrorKind`] variant. That is GUARD-13's intent met structurally: it keeps
//! the three error sources — read, write and validate — distinguishable, gives
//! "no position" a representation, and keeps `size_of::<Error>()` inside its
//! budget as variants accumulate across four milestones.
//!
//! [`Finding`] is the validate-side counterpart. `validate()` returns
//! `Vec<Finding>`, never `Result<(), Error>`: one run tells an import adapter
//! everything wrong with a foreign file, which is what it needs to explain a
//! rejection rather than approximate one.
//!
//! These types are part of the contract with Parallax's import adapter.
//! Adding an `ErrorKind` or `Location` variant is additive under
//! `#[non_exhaustive]`; changing an existing one is a breaking change across
//! two repositories.

use core::fmt;

/// Where an error happened.
///
/// Keeping the three sources distinguishable is the point. A single numeric
/// `offset` field repeated into every `ErrorKind` variant would conflate a
/// read position, a write position and a field path, have no representation
/// for "no position at all", and make the 32-byte budget unreachable within a
/// milestone or two.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// A byte position in the input being read.
    InputOffset(u64),
    /// A byte position in the output being written.
    OutputOffset(u64),
    /// A static field path, used by `validate()`.
    Field(&'static str),
    /// The error has no meaningful position.
    Unlocated,
}

/// What went wrong.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorKind {
    #[error("unexpected end of input")]
    UnexpectedEndOfInput,
    #[error("reader is not byte-aligned")]
    NotByteAligned,
    #[error("a bit width of {bits} is outside the supported 0..=64 range")]
    UnsupportedWidth { bits: u8 },
    #[error("value does not fit in the {bits} bits it was asked to occupy")]
    ValueExceedsWidth { bits: u8 },
    #[error("uleb128 continuation chain exceeds the 8-byte maximum")]
    Leb128TooLong,
    #[error("uleb128 value does not fit in u32")]
    Leb128ValueTooLarge,
    #[error("string is not NUL-terminated")]
    StringNotTerminated,
    #[error("string exceeds the 128-byte maximum including its NUL")]
    StringTooLong,
    #[error("string payload contains an interior NUL")]
    StringHasInteriorNul,
    #[error("a uleb128 field size of {size} is outside the supported 1..=8 range")]
    Leb128SizeInvalid { size: u8 },
    #[error("obu_size arithmetic overflowed")]
    ObuSizeOverflow,
    #[error("OBU exceeds the 2 MiB maximum")]
    ObuTooLarge,
    #[error("OBU payload is shorter than obu_size claims")]
    TruncatedObu,
    #[error("payload is not byte-aligned after the size fields")]
    UnalignedAfterSizeFields,
    #[error("obu_redundant_copy is not allowed for this OBU type")]
    RedundantCopyNotAllowed,
    #[error("obu_trimming_status_flag is not allowed for this OBU type")]
    TrimmingFlagNotAllowed,
    #[error("reserved field carries the non-zero value {value}")]
    ReservedValue { value: u8 },
    #[error("loudness value is outside the representable Q7.8 range")]
    LoudnessOutOfRange,
    #[error("additional_profile is below primary_profile")]
    AdditionalProfileBelowPrimary,
    #[error("no profile permits this configuration")]
    ProfileNotFound,
    #[error("this OBU type is not an Audio Frame")]
    NotAnAudioFrame,
    #[error("audio_substream_id disagrees with the id the OBU type implies")]
    SubstreamIdMismatch,
    #[error("channel count disagrees with the layout's")]
    ChannelCountMismatch,
    #[error("coupled_substream_count differs from the count the loudspeaker layout requires")]
    CoupledSubstreamCountMismatch,
    #[error("this loudspeaker layout has no packing plan in this crate")]
    UnsupportedLayout,
    #[error("num_samples_per_frame is zero")]
    ZeroSamplesPerFrame,
    #[error("frame-plan arithmetic overflowed")]
    FramePlanOverflow,
    #[error("audio frames in one temporal unit carry different trim values")]
    TemporalUnitTrimMismatch,
    #[error("two audio frames in one temporal unit claim the same substream id")]
    DuplicateSubstreamId,
    #[error("parameter_id disagrees with the governing parameter definition")]
    ParameterIdMismatch,
    #[error("parameter block and its definition disagree on param_definition_mode")]
    ParameterModeMismatch,
    #[error("this parameter data shape is not modelled by this crate")]
    UnsupportedParameterData,
    #[error("subblock durations do not sum to the block duration")]
    SubblockDurationMismatch,
    #[error("annotation count disagrees with count_label")]
    AnnotationCountMismatch,
    #[error("channel count exceeds what the profile permits")]
    ChannelCountExceedsProfile,
    #[error("audio element count exceeds what the profile permits")]
    ElementCountExceedsProfile,
    #[error("a temporal unit was pushed before the descriptors were written")]
    DescriptorsNotWritten,
    #[error("the descriptors have already been written to this sequence")]
    DescriptorsAlreadyWritten,
    #[error("no parameter definition in the descriptors governs this parameter_id")]
    NoGoverningParamDefinition,
    #[error("the output sink refused a write")]
    SinkWrite,
    #[error("the sequence writer is poisoned after a partial output failure")]
    SequenceWriterPoisoned,
    #[error("sample rate is not supported by the codec")]
    SampleRateNotSupportedByCodec,
    #[error("samples per frame are not supported by the codec")]
    SamplesPerFrameNotSupportedByCodec,
    #[error("bits per sample are not supported by the codec")]
    BitsPerSampleNotSupportedByCodec,
    #[error("a builder declaration references an unknown codec-config handle")]
    UnknownCodecConfigHandle,
    #[error("a builder declaration references an unknown audio-element handle")]
    UnknownAudioElementHandle,
    #[error("a builder declaration references an unknown substream handle")]
    UnknownSubstreamHandle,
    #[error("a builder declaration references an unknown parameter handle")]
    UnknownParameterHandle,
    #[error("a builder declaration has an invalid descriptor reference")]
    InvalidDescriptorReference,
    #[error("a builder declaration is duplicated")]
    DuplicateDeclaration,
    #[error("the builder cannot allocate another IAMF wire id")]
    WireIdAllocationExhausted,
    #[error("an IA sequence permits only one Codec Config")]
    MultipleCodecConfigs,
    /// `num_sub_mixes` is not 1.
    // ref: IAMF v1.1.0 index.bs:1278 (SHALL NOT be 0), :1920 (SHOULD be 1)
    // ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:760-782 (any other count fails the parse)
    #[error("a Mix Presentation must carry exactly one sub-mix")]
    SubMixCountNotOne,
    /// A rendering config carries a reserved `headphones_rendering_mode` (2 or 3).
    // ref: IAMF v1.1.0 index.bs:1337-1341 (parsers SHALL ignore the Mix Presentation)
    #[error("headphones_rendering_mode is reserved")]
    ReservedHeadphonesRenderingMode,
    /// The builder declares no Mix Presentation.
    // ref: IAMF v1.1.0 index.bs:1929 (at least one Mix Presentation SHALL comply with primary_profile)
    // ref: iamf-tools@v2.1.0 iamf/cli/obu_processor.cc:531-533 ("No mix presentation OBUs found.")
    // ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:3157-3160 (IAMF_FLAG_CONFIG needs a Mix Presentation)
    #[error("an IA sequence must carry at least one Mix Presentation")]
    NoMixPresentation,
    /// A Mix Presentation references the same Audio Element more than once.
    // ref: IAMF v1.1.0 index.bs:1280; iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:41-54 ValidateUniqueAudioElementIds
    // DISAGREEMENT: libiamf@v1.1.0 has no check; spec and iamf-tools are stricter and win
    #[error("a Mix Presentation references the same Audio Element more than once")]
    DuplicateMixPresentationAudioElement,
    #[error("a mix-gain parameter_rate differs from the Codec Config output sample rate")]
    ParameterRateMismatch,
    #[error("a temporal unit references an unknown substream handle")]
    UnknownTemporalSubstreamHandle,
    #[error("a temporal unit references an unknown parameter handle")]
    UnknownTemporalParameterHandle,
    #[error("a temporal unit is missing one or more declared substreams")]
    MissingTemporalSubstream,
    #[error("a temporal unit submits the same substream more than once")]
    DuplicateTemporalSubstream,
    #[error("temporal frames are not in declared substream order")]
    TemporalSubstreamOrderMismatch,
    #[error("the submitted frame kind does not match the frozen codec configuration")]
    FrameCodecMismatch,
    #[error("the LPCM frame payload is not sample-byte aligned")]
    LpcmFrameByteAlignment,
    #[error("the LPCM frame payload does not carry the frozen sample count")]
    LpcmFrameSampleCountMismatch,
    #[error("a Parameter Block duration differs from its Audio Frame duration")]
    ParameterBlockDurationMismatch,
    #[error(
        "a temporal unit's Parameter Blocks do not cover the same parameters as the first temporal unit"
    )]
    ParameterSubstreamCoverageMismatch,
    #[error("a temporal unit carries more than one Parameter Block for one parameter")]
    DuplicateTemporalParameterBlock,
    #[error("num_samples_to_trim_at_start follows audio that was not fully trimmed")]
    StartTrimAfterUntrimmedAudio,
    #[error("a temporal unit follows one that trimmed samples at its end")]
    TemporalUnitAfterEndTrim,
    #[error("the total num_samples_to_trim_at_start exceeds the Opus pre_skip")]
    OpusStartTrimExceedsPreSkip,
    #[error("the start trim ended before reaching the Opus pre_skip")]
    OpusStartTrimShortOfPreSkip,
}

/// An error, with the position it happened at attached exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    at: Location,
}

impl Error {
    /// Construct an error at a position.
    #[must_use]
    pub const fn new(kind: ErrorKind, at: Location) -> Self {
        Self { kind, at }
    }

    /// What went wrong.
    #[must_use]
    pub const fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Where it went wrong.
    #[must_use]
    pub const fn at(&self) -> Location {
        self.at
    }

    /// Translate an error raised by a bounded payload reader into the parent
    /// input's coordinate space.
    ///
    /// Header errors already originate on the parent cursor and must not pass
    /// through this helper. Sequence dispatch applies it only inside the
    /// payload callback handed to the central OBU reader.
    pub(crate) fn with_input_base(mut self, base: u64) -> Self {
        if let Location::InputOffset(relative) = self.at {
            self.at = Location::InputOffset(base.saturating_add(relative));
        }
        self
    }
}

impl fmt::Display for Error {
    /// The kind's message, followed by a position fragment when there is a
    /// position. Offsets render in decimal and hex because a fuzz report is
    /// read next to a hex dump.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        match self.at {
            Location::InputOffset(offset) => {
                write!(f, " at input offset {offset} (0x{offset:x})")
            }
            Location::OutputOffset(offset) => {
                write!(f, " at output offset {offset} (0x{offset:x})")
            }
            Location::Field(path) => write!(f, " at field `{path}`"),
            Location::Unlocated => Ok(()),
        }
    }
}

impl core::error::Error for Error {
    /// Always `None`. There are no `#[from]` conversions in this crate — a
    /// blanket `From` loses the offset, so conversion happens at the call site
    /// where the position is known (D-08).
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        None
    }
}

/// `Result<T, Error>` is returned from every primitive in the crate, so the
/// error's size is a cost every call pays. Thirty-two bytes is the budget:
/// `Location` is 24 (a `&'static str` is a fat pointer, plus a discriminant)
/// and `ErrorKind`'s largest payload is a `u8`.
///
/// This is a `const` assertion, not a test, so a future variant that wants to
/// carry a `String` or a `Vec` fails the **build** rather than the suite. Box
/// such a payload.
const _: () = assert!(size_of::<Error>() <= 32);

/// One thing `validate()` found wrong (D-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The field path this finding is about.
    pub at: Location,
    /// A human-readable reason an import adapter can show a user.
    pub message: String,
}

/// The crate's `Result`, with the error type defaulted.
pub type Result<T> = core::result::Result<T, Error>;
