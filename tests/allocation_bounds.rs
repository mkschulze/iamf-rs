//! GUARD-03 / PARSE-01 — count fields may not drive reservations past a fixed cap.
//!
//! Closes the `.planning/codebase/CONCERNS.md` entry "Allocation amplification
//! from count fields". Every parser checks a parsed count against
//! `bytes_remaining()` before looping, but that check bounds the count in
//! *bytes*, not in bytes times element size: inside the 2 MiB OBU cap a count
//! equal to the bytes left, times a `size_of` of tens or hundreds of bytes, is
//! hundreds of MB reserved per nesting level before the first element read
//! fails. The remedy is `crate::bits::bounded_vec`, which reserves at most
//! `MAX_PREALLOCATION_BYTES` (64 KiB) and lets anything beyond grow by push.
//!
//! What this file proves:
//!
//! - A hostile count equal to the bytes remaining, in both a small buffer and a
//!   full 2 MiB buffer, still yields the exact typed error (kind and offset) it
//!   did before the fix — the early `bytes_remaining()` checks are unchanged.
//! - Structurally, no module under `src/` other than the two that reserve from
//!   caller-owned in-memory collections contains a capacity reservation outside
//!   the one inside `bounded_vec` (the source-scan gate below).
//!
//! What it does **not** prove: the allocator is never measured. A counting
//! global allocator needs `unsafe`, and `unsafe_code` is `forbid` on every
//! target. The numeric bound itself is proved by the unit tests beside
//! `bounded_capacity` in `src/bits/reader.rs`; this file proves every parse-path
//! reservation goes through it.

use iamf::bits::BitCursor;
use iamf::obu::read_mix_presentation;
use iamf::{ErrorKind, Location};

/// The reference's whole-OBU ceiling, `kEntireObuSizeMaxTwoMegabytes`.
const TWO_MEBIBYTES: usize = 1 << 21;

/// `prefix` zero-extended to exactly 2 MiB.
fn two_mebibyte_buffer(prefix: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(prefix);
    buffer.resize(TWO_MEBIBYTES, 0);
    buffer
}

#[test]
fn hostile_sub_mix_count_equal_to_remaining_bytes_in_small_mix_presentation_errors_at_offset_3() {
    // offset 0: mix_presentation_id = 0
    // offset 1: count_label = 0 (no annotation strings)
    // offset 2: num_sub_mixes = 5, equal to the 5 bytes left, so it passes the
    //           early check
    // offset 3: first sub-mix's num_audio_elements = FF FF FF 7F = 268_435_455,
    //           above the 1 byte left -> UnexpectedEndOfInput at its start, 3
    let bytes = [0x00, 0x00, 0x05, 0xFF, 0xFF, 0xFF, 0x7F, 0x00];
    let mut r = BitCursor::new(&bytes);
    let error = read_mix_presentation(&mut r).expect_err("hostile count must not parse");
    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert_eq!(error.at(), Location::InputOffset(3));
}

#[test]
fn hostile_sub_mix_count_in_two_mebibyte_mix_presentation_errors_at_offset_5() {
    // offset 0: mix_presentation_id = 0
    // offset 1: count_label = 0
    // offset 2..5: num_sub_mixes = FB FF 7F
    //              = 123 + 127*128 + 127*16384 = 2_097_147
    //              = 2_097_152 - 5, the bytes left, so it passes the early check
    //              and, before the fix, reserved 2_097_147 x size_of::<SubMix>()
    // offset 5: first sub-mix's num_audio_elements = FF FF FF 7F = 268_435_455,
    //           above what remains -> UnexpectedEndOfInput at 5
    let bytes = two_mebibyte_buffer(&[0x00, 0x00, 0xFB, 0xFF, 0x7F, 0xFF, 0xFF, 0xFF, 0x7F]);
    let mut r = BitCursor::new(&bytes);
    let error = read_mix_presentation(&mut r).expect_err("hostile count must not parse");
    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert_eq!(error.at(), Location::InputOffset(5));
}
