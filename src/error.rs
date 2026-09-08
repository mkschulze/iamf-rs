//! The published error surface (D-08 / D-09).
//!
//! **RED-phase stub.** The type *shape* is here so `tests/error_shape.rs`
//! compiles and can assert on it; the *behaviour* is not. `Display` renders
//! the kind's message and nothing else, so every test that asserts a position
//! reaches the rendered string fails. The compile-time size budget is not
//! installed yet either. Both land in the GREEN commit.

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
    #[error("uleb128 continuation chain exceeds the 8-byte maximum")]
    Leb128TooLong,
    #[error("uleb128 value does not fit in u32")]
    Leb128ValueTooLarge,
    #[error("string is not NUL-terminated")]
    StringNotTerminated,
    #[error("string exceeds the 128-byte maximum including its NUL")]
    StringTooLong,
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
    #[error("no profile permits this configuration")]
    ProfileNotFound,
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
    // RED-phase stub: renders the kind only. The position fragment is the
    // behaviour under test and does not exist yet.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl core::error::Error for Error {}

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
