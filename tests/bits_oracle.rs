//! D-01's differential oracle — the entire justification for hand-rolling.
//!
//! `bitstream-io` has ~50 million downloads of exercise behind its bit packing.
//! `src/bits/` has a few hundred lines written this week. Hand-rolling is only
//! defensible if the two are made to agree on random input, and that is what
//! this file does: for random widths, values and read schedules, our
//! primitives must produce byte-identical output and identical decoded values.
//!
//! **The oracle crate is imported here and nowhere else.** That is what makes
//! the containment claim checkable rather than merely stated: a verify gate
//! greps `src/` for `use bitstream_io` and fails on any hit. Wiring it into
//! `src/` would reintroduce the `std::io::Error` mapping boundary BITS-07
//! exists to remove, and with it the native byte offsets every `Error.at`
//! value in the crate flows from.
//!
//! These are proptests rather than loops for one reason: when a property fails,
//! shrinking hands back a minimal `(value, width)` counterexample. A loop hands
//! back the first failure it happened to reach.

use bitstream_io::{
    BigEndian, BitRead, BitReader as OracleReader, BitWrite, BitWriter as OracleWriter,
};
use iamf::bits::{BitCursor, BitWriter};
use proptest::prelude::*;

/// Our writer padded to a byte boundary with zero bits, which is what the
/// oracle's `byte_align()` does, so the two buffers are comparable including
/// their final partial byte.
fn ours_padded(write: impl FnOnce(&mut BitWriter)) -> Vec<u8> {
    let mut w = BitWriter::new();
    write(&mut w);
    while !w.is_byte_aligned() {
        w.write_bool(false).expect("padding a partial byte");
    }
    w.finish().expect("padded to a byte boundary")
}

/// `value` reduced to the low `bits` bits, so a generated `u64` is always a
/// legal input for the width under test.
fn mask_to_width(value: u64, bits: u32) -> u64 {
    match 1_u64.checked_shl(bits) {
        Some(limit) => value & limit.wrapping_sub(1),
        None => value,
    }
}

/// A signed width paired with a value that genuinely fits it. Generated as a
/// dependent strategy rather than by masking, so the test never reimplements
/// the sign extension it is trying to check.
fn signed_case() -> impl Strategy<Value = (u32, i64)> {
    (2_u32..=16).prop_flat_map(|bits| {
        let half = 1_i64.checked_shl(bits.saturating_sub(1)).unwrap_or(1);
        (Just(bits), -half..half)
    })
}

/// The minimal uleb128 length, derived independently of `minimal_len`'s range
/// table — by counting seven-bit groups. Two derivations that must agree.
fn expected_minimal_len(value: u32) -> usize {
    let mut len = 1_usize;
    let mut rest = value.checked_shr(7).unwrap_or(0);
    while rest != 0 {
        len = len.saturating_add(1);
        rest = rest.checked_shr(7).unwrap_or(0);
    }
    len
}

proptest! {
    /// Byte-for-byte agreement on the write side, including the zero padding of
    /// the final partial byte.
    #[test]
    fn unsigned_writes_match_the_oracle_byte_for_byte(bits in 1_u32..=64, raw in any::<u64>()) {
        let value = mask_to_width(raw, bits);

        let ours = ours_padded(|w| {
            w.write_unsigned(value, bits).expect("value fits the width");
        });

        let mut oracle = OracleWriter::endian(Vec::new(), BigEndian);
        oracle.write_var(bits, value).expect("oracle write");
        oracle.byte_align().expect("oracle pad");
        let theirs = oracle.into_writer();

        prop_assert_eq!(ours, theirs);
    }

    /// Identical decoded values at every step of a random read schedule, which
    /// is what catches a cursor that drifts only after an odd number of bits.
    #[test]
    fn unsigned_reads_match_the_oracle_at_every_step(
        data in prop::collection::vec(any::<u8>(), 1..64),
        widths in prop::collection::vec(1_u32..=32, 1..16),
    ) {
        let mut ours = BitCursor::new(&data);
        let mut theirs = OracleReader::endian(data.as_slice(), BigEndian);

        for bits in widths {
            if ours.bits_remaining() < u64::from(bits) {
                break;
            }
            let a = ours.read_unsigned(bits).expect("bounds already checked");
            let b = theirs.read_var::<u64>(bits).expect("oracle read");
            prop_assert_eq!(a, b);
        }
    }

    /// Signed widths round-trip through our own pair AND agree with the oracle
    /// in both directions. The 9-bit field is inside this range, and it is the
    /// one that rules out every byte-oriented approach.
    #[test]
    fn signed_widths_round_trip_and_match_the_oracle((bits, value) in signed_case()) {
        let ours = ours_padded(|w| {
            w.write_signed(value, bits).expect("value fits the width");
        });

        let mut oracle = OracleWriter::endian(Vec::new(), BigEndian);
        oracle.write_signed_var(bits, value).expect("oracle write");
        oracle.byte_align().expect("oracle pad");
        prop_assert_eq!(ours.clone(), oracle.into_writer());

        let mut back = BitCursor::new(&ours);
        prop_assert_eq!(back.read_signed(bits).expect("our read"), value);

        let mut oracle_back = OracleReader::endian(ours.as_slice(), BigEndian);
        prop_assert_eq!(oracle_back.read_signed_var::<i64>(bits).expect("oracle read"), value);
    }

    /// uleb128 round-trips, and its encoded length is the minimal one — checked
    /// against a length derived by counting groups rather than by the range
    /// table the encoder itself uses.
    #[test]
    fn uleb128_round_trips_at_the_minimal_length(value in any::<u32>()) {
        let bytes = ours_padded(|w| {
            w.write_uleb128_minimal(value).expect("every u32 encodes");
        });

        prop_assert_eq!(bytes.len(), expected_minimal_len(value));
        prop_assert_eq!(
            BitCursor::new(&bytes).read_uleb128().expect("decodes"),
            value
        );
    }
}

/// The seven-bit group boundaries, exhaustively, alongside the shrinkable
/// random sample above. These are the values a random `u32` almost never hits.
#[test]
fn uleb128_group_boundaries_round_trip() {
    let cases = [
        0_u32,
        1,
        127,
        128,
        16_383,
        16_384,
        (1 << 21) - 1,
        1 << 21,
        (1 << 28) - 1,
        1 << 28,
        u32::MAX,
    ];

    for value in cases {
        let bytes = ours_padded(|w| {
            w.write_uleb128_minimal(value).expect("encodes");
        });
        assert_eq!(
            bytes.len(),
            expected_minimal_len(value),
            "{value} did not encode at its minimal length"
        );
        assert_eq!(
            BitCursor::new(&bytes).read_uleb128().expect("decodes"),
            value,
            "{value} did not survive the round trip"
        );
    }
}
