//! PROF-03 — the one place in this crate where a float touches the encode
//! path.
//!
//! IAMF carries loudness as **Q7.8**: a signed 16-bit fixed-point value whose
//! low 8 bits are the fraction, so the wire value is the LUFS reading times
//! 256. `test_000003` publishes `integrated_loudness: -13733`, which is
//! `-53.64453125` LUFS exactly.
//!
//! This crate **carries** the number; it does not compute it. Loudness
//! *measurement* is a BS.1770 meter and lives in Parallax, not here. The only
//! operation in this module is a scale and a round.
//!
//! # The single documented escape (D-21)
//!
//! `clippy.toml` bans `f32` and `f64` crate-wide as GUARD-11's no-DSP guard.
//! [`lufs_to_q7_8`] carries the **one** lint escape in `src/`, on the narrowest
//! possible scope — the function, not the module.
//!
//! That turns the guard into a **census whose correct answer is exactly one**.
//! A second escape appearing anywhere in `src/` is a scope breach to
//! investigate, not a lint to silence; `tests/profile.rs` asserts the count and
//! the file it lives in, and `01-07`'s verification greps for both.
//!
//! The guard is not being weakened. The escape covers a multiplication by
//! `256.0` and `f64::round_ties_even`, and **both are IEEE-754-exact**: the
//! multiply is exact for every representable input because 256 is a power of
//! two, and round-to-nearest-ties-to-even is the IEEE default rounding mode,
//! specified bit-for-bit. Neither consults a platform math library, so the
//! result is identical on macOS arm64, macOS x86_64, Windows MSVC and Linux
//! x64 without `libm`. That is precisely why `round_ties_even` is deliberately
//! **absent** from `clippy.toml`'s transcendental list — it is not a
//! transcendental, and adding it there would break PROF-03 for no gain.

use crate::error::{Error, ErrorKind, Location, Result};

/// A Q7.8 fixed-point loudness value: the LUFS or dBFS reading times 256, as
/// the signed 16-bit field the Loudness block carries.
///
/// A newtype rather than a bare `i16` so that a value which has been through
/// the range check cannot be confused with one that has not, and so that a
/// caller cannot pass a raw LUFS reading where a scaled one belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Q7_8(i16);

impl Q7_8 {
    /// A value already in Q7.8 form — the wire representation, verbatim.
    ///
    /// This is the constructor a *parser* uses: the bytes said `-13733`, and
    /// nothing needs converting. Encoders producing a value from a measurement
    /// go through [`lufs_to_q7_8`].
    #[must_use]
    pub const fn from_raw(raw: i16) -> Self {
        Self(raw)
    }

    /// The wire value.
    #[must_use]
    pub const fn to_i16(self) -> i16 {
        self.0
    }
}

/// The Q7.8 range, as the two ends of the `i16` it is stored in.
const Q7_8_MIN: i64 = i16::MIN as i64;
/// See [`Q7_8_MIN`].
const Q7_8_MAX: i64 = i16::MAX as i64;

// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.h LoudnessInfo::integrated_loudness
// NOTE: the reference stores the already-quantised int16 and never performs
// this conversion itself — its encoder is handed Q7.8 values by its proto
// input. So there is no reference function to mirror line for line; the
// citation names the FIELD whose representation this produces. The rounding
// rule is PROF-03's, not the reference's.
/// Quantise a loudness reading in LUFS (or dBFS, for `digital_peak`) to Q7.8.
///
/// Ties round to **even**, not away from zero and not toward zero. An input
/// landing exactly on a half LSB goes to its even neighbour: `-13733.5 / 256`
/// becomes `-13734` and `-13732.5 / 256` becomes `-13732`.
///
/// That matters because **every loudness value in a real file is negative**. An
/// `as i16` cast truncates toward zero, which would bias every one of them
/// upward by up to a full LSB — a systematic error, not a rounding wobble,
/// and exactly what PROF-03 forbids. There is no `as i16` in this function:
/// the range is checked first, the float→integer step targets a type wide
/// enough that it cannot saturate into a plausible value, and the narrowing to
/// `i16` is a checked `try_from`.
///
/// # Errors
///
/// [`ErrorKind::LoudnessOutOfRange`] for NaN, for either infinity, and for any
/// value whose scaled form falls outside `i16::MIN..=i16::MAX`. Rejected
/// *before* the conversion, so an out-of-range input is a named error rather
/// than a saturating cast.
#[allow(
    clippy::disallowed_types,
    reason = "PROF-03 / D-21: the crate's single \
    sanctioned float escape. See the module documentation — the operations are \
    a power-of-two multiply and round-to-nearest-ties-to-even, both \
    IEEE-754-exact, so no platform math library is involved and the result is \
    identical on all four targets. A SECOND escape anywhere in src/ is a scope \
    breach to investigate, not a lint to silence."
)]
pub fn lufs_to_q7_8(lufs: f64) -> Result<Q7_8> {
    let out_of_range = || {
        Error::new(
            ErrorKind::LoudnessOutOfRange,
            Location::Field("integrated_loudness"),
        )
    };

    // NaN and both infinities go first, before any arithmetic: `NAN * 256.0`
    // is NaN and every comparison against it is false, so a range check alone
    // would let it through (T-01-39).
    if !lufs.is_finite() {
        return Err(out_of_range());
    }

    // Exact: 256 is a power of two, so this only adjusts the exponent.
    let scaled = lufs * 256.0;
    // The IEEE-754 default rounding mode, specified bit for bit. Not a
    // transcendental, and deliberately NOT on clippy.toml's banned list —
    // adding it there would break PROF-03 for no gain.
    let rounded = scaled.round_ties_even();

    // Range-checked BEFORE the conversion, so out-of-range is a typed error
    // rather than whatever a saturating cast happens to produce.
    if rounded < Q7_8_MIN as f64 || rounded > Q7_8_MAX as f64 {
        return Err(out_of_range());
    }

    // `as i64`, never `as i16`. The value is already integral and inside the
    // i16 range, so the widening cast is exact; `try_from` then narrows under
    // a check rather than a truncation, and the two together mean no single
    // mistake in this function can silently produce a wrong value.
    let widened = rounded as i64;
    let raw = i16::try_from(widened).map_err(|_| out_of_range())?;
    Ok(Q7_8::from_raw(raw))
}
