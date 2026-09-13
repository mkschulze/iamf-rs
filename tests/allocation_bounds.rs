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
//! - A Demixing or Recon Gain subblock count other than 1, which can be derived
//!   rather than paid for in wire bytes, is refused before any subblock is
//!   reserved (quick 260914-1mq).
//! - Structurally, no module under `src/` other than the two that reserve from
//!   caller-owned in-memory collections contains a capacity reservation outside
//!   the one inside `bounded_vec` (the source-scan gate below).
//!
//! Why the scan gate exists: a future parser that reserves directly from a
//! count reintroduces the OOM silently — every existing test still passes,
//! because the reservation only hurts on hostile input. The fuzzer would find it
//! first only on the nightly Linux job; this gate finds it on the stable PR gate
//! on all four targets, and names the line.
//!
//! What it does **not** prove: the allocator is never measured. A counting
//! global allocator needs `unsafe`, and `unsafe_code` is `forbid` on every
//! target. The numeric bound itself is proved by the unit tests beside
//! `bounded_capacity` in `src/bits/reader.rs`; this file proves every parse-path
//! reservation goes through it.

use std::fs;
use std::path::{Path, PathBuf};

use iamf::bits::BitCursor;
use iamf::obu::{
    ParamDefinition, ParamDefinitionRegistry, ParameterDataContext, read_audio_element,
    read_mix_presentation, read_parameter_block,
};
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

#[test]
fn hostile_parameter_count_equal_to_remaining_bytes_in_small_audio_element_errors_at_offset_10() {
    // offset 0: audio_element_id = 0
    // offset 1: audio_element_type (3 bits) + reserved (5 bits) = 0
    // offset 2: codec_config_id = 0
    // offset 3: num_substreams = 0
    // offset 4: num_parameters = 5, equal to the 5 bytes left
    // offset 5: param_definition_type = 127 (extension branch)
    // offset 6..10: param_definition_size = FF FF FF 7F = 268_435_455
    // offset 10: read_uint8_span with 0 bytes left -> UnexpectedEndOfInput at 10
    let bytes = [0x00, 0x00, 0x00, 0x00, 0x05, 0x7F, 0xFF, 0xFF, 0xFF, 0x7F];
    let mut r = BitCursor::new(&bytes);
    let error = read_audio_element(&mut r).expect_err("hostile count must not parse");
    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert_eq!(error.at(), Location::InputOffset(10));
}

#[test]
fn hostile_parameter_count_in_two_mebibyte_audio_element_errors_at_offset_12() {
    // offset 0..4: id, type/reserved, codec_config_id, num_substreams, all 0
    // offset 4..7: num_parameters = F9 FF 7F
    //              = 121 + 127*128 + 127*16384 = 2_097_145
    //              = 2_097_152 - 7, the bytes left, so it passes the early check
    // offset 7: param_definition_type = 127 (extension branch)
    // offset 8..12: param_definition_size = FF FF FF 7F = 268_435_455
    // offset 12: read_uint8_span beyond what remains -> UnexpectedEndOfInput at 12
    let bytes = two_mebibyte_buffer(&[
        0x00, 0x00, 0x00, 0x00, 0xF9, 0xFF, 0x7F, 0x7F, 0xFF, 0xFF, 0xFF, 0x7F,
    ]);
    let mut r = BitCursor::new(&bytes);
    let error = read_audio_element(&mut r).expect_err("hostile count must not parse");
    assert_eq!(error.kind(), &ErrorKind::UnexpectedEndOfInput);
    assert_eq!(error.at(), Location::InputOffset(12));
}

#[test]
fn hostile_recon_gain_subblock_count_in_two_mebibyte_parameter_block_errors_at_offset_0() {
    // offset 0: parameter_id = 1 (mode 1 definition, Recon Gain, 7 absent layers)
    // offset 1..4: duration = FB FF 7F
    //              = 123 + 127*128 + 127*16384 = 2_097_147
    //              = 2_097_152 - 5, the bytes left after the csd byte
    // offset 4: constant_subblock_duration = 1
    //           => implied num_subblocks = 2_097_147, each a zero-byte Recon Gain
    //              subblock with ReconGain [false; 7], so it equals the bytes left
    //              and passed the bytes_remaining() rule; before the fix this parsed
    //              Ok and built 2_097_147 subblocks
    // -> SubblockCountNotOne at the payload start, 0, before any reservation
    let bytes = two_mebibyte_buffer(&[0x01, 0xFB, 0xFF, 0x7F, 0x01]);
    let mut registry = ParamDefinitionRegistry::new();
    registry.register(
        ParamDefinition::mode_1(1, 48_000),
        ParameterDataContext::ReconGain {
            recon_gain_is_present: vec![false; 7],
        },
    );
    let mut r = BitCursor::new(&bytes);
    let error = read_parameter_block(&mut r, &registry).expect_err("hostile count must not parse");
    assert_eq!(error.kind(), &ErrorKind::SubblockCountNotOne);
    assert_eq!(error.at(), Location::InputOffset(0));
}

/// Files under `src/` the scan skips, each because its reservations are sized
/// from caller-owned in-memory collections, never from parsed bytes.
const EXCLUDED_FROM_SCAN: &[&str] = &[
    // Sized from `input.frames` / `input.parameter_blocks` lengths the caller built.
    "src/encoder.rs",
    // Sized from `plan.substreams` and the caller's interleaved PCM length.
    "src/packing.rs",
];

/// The tokens that request capacity up front.
const RESERVATION_TOKENS: &[&str] = &[
    "with_capacity(",
    ".reserve(",
    ".reserve_exact(",
    ".try_reserve(",
];

/// The one sanctioned reservation: the body of `bounded_vec`.
const SANCTIONED_FILE: &str = "src/bits/reader.rs";

#[test]
fn every_parse_path_reservation_under_src_goes_through_bounded_vec() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits: Vec<(String, usize)> = Vec::new();
    let mut scanned = 0_usize;

    for file in rust_files_under(&root.join("src")) {
        let relative = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if EXCLUDED_FROM_SCAN.contains(&relative.as_str()) {
            continue;
        }
        scanned = scanned.saturating_add(1);
        let text = fs::read_to_string(&file).expect("src file is readable");
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let occurrences: usize = RESERVATION_TOKENS
                .iter()
                .map(|token| line.matches(token).count())
                .sum();
            for _ in 0..occurrences {
                hits.push((relative.clone(), index.saturating_add(1)));
            }
        }
    }

    assert!(
        scanned > 0,
        "the walker found no .rs files under src/ — it has stopped measuring anything"
    );
    let only_the_sanctioned_one =
        hits.len() == 1 && hits.iter().all(|(path, _)| path == SANCTIONED_FILE);
    assert!(
        only_the_sanctioned_one,
        "expected exactly one capacity reservation under src/ (inside bounded_vec in \
         {SANCTIONED_FILE}), found {}. Route each parse-path reservation through \
         crate::bits::bounded_vec:\n  {}",
        hits.len(),
        hits.iter()
            .map(|(path, line)| format!("{path}:{line}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// Every `.rs` file under `root`, recursively, in sorted order.
fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_files_under(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}
