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
    #[error("channel count exceeds what the profile permits")]
    ChannelCountExceedsProfile,
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
