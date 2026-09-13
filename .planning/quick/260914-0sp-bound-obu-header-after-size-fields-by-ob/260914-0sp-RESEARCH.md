# Quick 260914-0sp: Bound OBU header after-size fields by `obu_size` - Research

**Researched:** 2026-09-14
**Domain:** OBU header framing (`src/obu/header.rs`, `src/obu/mod.rs`)
**Confidence:** HIGH (the fix was prototyped in a scratch copy of HEAD; the full offline suite, clippy and fuzz replay pass there; a differential scan covered all 110 committed fixture and corpus files)

<user_constraints>
## User Constraints (binding, from `.planning/.continue-here.md` BLOCKING CONSTRAINTS)

- **Conformance rule:** satisfy BOTH the IAMF v1.1.0 spec AND the pinned references. Where either is stricter, keep the stricter rule. Record every spec/reference disagreement in a `// ref:` comment.
- **Reference hashes:** a `semantic_sha256` in `tests/support/reference_expectations.rs` may change ONLY for a fixture that the pinned `iamf-tools` testdata marks `is_valid: false`. Predict the new hash, then verify it exactly.
- **Pinned trees only:** `git -C docs/iamf-tools show 848c6ff4:<path>`, `git -C docs/iamf show v1.1.0:index.bs`, `git -C docs/libiamf show f06e919e:<path>`.
- **Staging discipline:** never `git add -A` / `.` / `commit -a`. Stage explicit paths and check `git diff --cached --name-only`.
</user_constraints>

## Project Constraints (from CLAUDE.md)

- Clippy denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used` and `panic`. Use `checked_*` and `.get()`. Tests are exempt only inside `#[test]` fns (see Pitfall 3).
- Every `fn read_*` / `fn write_*` in `src/` needs a `// ref: <repo>@<tag> <path> <Symbol>` line above it (`tests/citations.rs`).
- Never silently normalise. Do not change existing `ErrorKind` variants. Adding one is allowed, but reuse is preferred.
- Rust 1.85 (no let-chains). Golden fixture bytes must not change. The linked graph stays `iamf` + `thiserror`.

## Summary

Right now `read_obu_header_parts` (`src/obu/header.rs:415-478`) validates the numeric `obu_size` and then reads the trim pair and the extension header from the **parent** cursor. The only bound on those reads is the end of the input. `read_extension_header` (`:487-493`) copies `len` bytes bounded by `bytes_remaining()` of the parent. Underflow is only rejected later, in `read_obu_with_header` (`src/obu/mod.rs:136-141`, `TruncatedObu` at the OBU start).

I verified this against HEAD. `read_obu_header` returns `Ok` for `32 00 40 07`, `01 00 01 aa`, `f9 01 02 aa bb` and `f9 00 00`. For `f9 02 80 80 40` + 1 MiB of zeros it returns `Ok((2, Some(1048576)))`, which means it allocated a 1 MiB extension for an OBU that declares 2 bytes.

**Primary recommendation:** read the after-size fields through a non-advancing window of `min(obu_size, bytes_remaining)` bytes, made with `r.clone().sub_reader(..)`. Then advance the parent by the measured count. Map `UnexpectedEndOfInput` to `TruncatedObu @ InputOffset(obu_start)` when the window was bounded by `obu_size`. Otherwise rebase with `Error::with_input_base(after_size_start)`. With this change, 110 of 110 committed inputs produce identical `parse_sequence` and `dump_annotated` output, and **no `semantic_sha256` changes**.

## Q1 - Callers and API promises

| Caller | Use | Effect of fix |
|---|---|---|
| `src/sequence.rs:530` `parse_sequence` | `read_obu_header(&mut inspection)` on a clone, then `read_obu_with[_header]` | Error now surfaces at inspection with the same kind and offset as before (`TruncatedObu@obu_start`). Verified: `20 00 01 00 01 aa` gives `TruncatedObu@2` |
| `src/dump.rs:115` `dump_one_obu` | header only, on a per-OBU slice from `find_obu_boundaries` | A malformed OBU now prints `<unparsable OBU header>`. Golden dump is unchanged (scan) |
| `src/obu/mod.rs:136` `read_obu_with_header` (and `read_obu_with`) | via `read_obu_header_parts` | Its `checked_sub` underflow check becomes unreachable. Keep it as defence in depth |
| tests: `obu_header.rs` (69,104,192,303,340,427), `packing.rs:270`, `conformance.rs:1639`, `fixture.rs:46`, `descriptors.rs`, `temporal.rs`, `round_trip.rs`, `parse_reference.rs` | header / whole-OBU reads | All pass with the fix |
| `src/fuzzing.rs` | none directly (builds models, then parses through `parse_sequence`) | covered by `fuzz_regression` (passes) |

- **Public header-only API:** `iamf::obu::read_obu_header` (re-exported at `src/obu/mod.rs:52`). Its doc (`header.rs:396-399`) says: "Read byte 0, `obu_size` and the after-size fields, leaving the cursor at the first payload byte." It does **not** require the payload to be present in the input. The test `a_trimmed_audio_frame_reads_its_trim_values_back_in_the_same_order` (`tests/obu_header.rs:188-200`, bytes `32 82 04 40 07`, obu_size 514 with only 2 bytes present) relies on that. **So a plain `r.sub_reader(obu_size)` is wrong.** It would break that test and change the documented contract. The window must be `min(obu_size, bytes_remaining)`.
- **`find_obu_boundaries`** (`src/obu/boundaries.rs`) reads only byte 0 (`read_unsigned(8)`) and `obu_size`, and never reads the after-size fields. It is unaffected and stays consistent: both paths now treat `obu_size` as the hard OBU boundary.

## Q2 - Pinned reference and spec behaviour

- **Spec** (`docs/iamf v1.1.0:index.bs:548`) [VERIFIED]: "*obu_size* indicates the size in bytes of the OBU immediately following the obu_size field. If the obu_trimming_status_flag and/or obu_extension_flag fields are set to 1, obu_size SHALL include the sizes of the additional fields." After-size fields that exceed `obu_size` are therefore non-conformant.
- **iamf-tools@v2.1.0 `iamf/obu/obu_header.cc` `ObuHeader::ReadAndValidate`** (`:303-342`) [VERIFIED]: it reads the trims and `extension_header_size` from the parent `rb`, and calls `extension_header_bytes.resize(extension_header_size)` (`:327`) **before** `ReadUint8Span`. It then computes `GetObuPayloadSize` and **rejects**: `"obu_size not valid for OBU flags. Negative remaining payload size."` (`:336`). So it rejects, but only after reading, and it allocates up to the leb value. We are stricter on allocation and reach the same accept/reject verdict.
- **libiamf@v1.1.0 `code/src/iamf_dec/IAMF_OBU.c` `IAMF_OBU_split`** (`:64-122`) [VERIFIED]: it checks `ret + bs_tell(&b) > size` against the **input buffer** only (`:82`). Trims and `ext_size` are read with `bs_getAleb128`, which is bounded by the buffer (`bitstream.c:122`). `bs_skipABytes(&b, obu->ext_size)` (`:111`) does no bounds check (`bs_read`, `bitstream.c:161-166`). Then `iamf_obu_get_payload_size` (`:246-248`) returns `obu->size - (uint32_t)(obu->payload - obu->data)`, which is **unsigned wrap-around**: no rejection. libiamf is permissive here, iamf-tools rejects, and the spec says SHALL. Under the conformance rule, reject.
- **Citation fix found:** the existing `// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ReadFieldsAfterObuSize` (`src/obu/header.rs:480`) names a symbol that **does not exist at 848c6ff4**. `git grep ReadFieldsAfterObuSize 848c6ff4 -- iamf` returns nothing, and only `WriteFieldsAfterObuSize` (`:106`) exists. Replace it with `ObuHeader::ReadAndValidate`.
- Suggested `// ref:` + `// NOTE:` on the new reader:
  ```
  // ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
  // ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c IAMF_OBU_split
  // NOTE: iamf-tools rejects a negative remaining payload only after reading the
  // fields (and resizes the extension first); libiamf wraps the uint32 payload size.
  // The spec (obu_size SHALL include these fields) is stricter than libiamf, so the
  // fields are read inside obu_size and an overrun is TruncatedObu.
  ```

## Q3 - Design (recommended: option (a) using a clamped, non-advancing window)

Cursor facts [VERIFIED: `src/bits/reader.rs:193-227, 257-275`]: `sub_reader(len)` is `read_uint8_span(len)` followed by `BitCursor::new(span)`. It **advances the parent**, and the child's `byte_position()` starts at **0 (relative)**. `read_uint8_span` checks `len > bytes_remaining()` before slicing. `Error::with_input_base(base)` (`src/error.rs:252-257`) adds `base` to an `InputOffset`. That is how `sequence.rs` already rebases payload errors.

Prototype. It passes the full suite, clippy (`--all-targets`, with and without `--features fuzzing`) and fuzz replay. Only `cargo fmt` reflow is needed:

```rust
pub(crate) fn read_obu_header_parts(r: &mut BitCursor<'_>) -> Result<(ObuHeader, u32, u64)> {
    let obu_start = r.byte_position();
    // ... byte 0, obu_size, validate_obu_size, after_size_start — unchanged ...
    let declared = usize::try_from(obu_size)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(obu_start)))?;
    let available = r.bytes_remaining();
    let bounded_by_obu_size = declared <= available;
    // Clone: the window must not advance `r` (header-only reads may lack the payload).
    let mut window = r.clone().sub_reader(declared.min(available))?;
    let (type_specific, extension) =
        read_fields_after_obu_size(&mut window, obu_type, trimming_status_flag, extension_flag)
            .map_err(|error| {
                if bounded_by_obu_size && error.kind() == &ErrorKind::UnexpectedEndOfInput {
                    Error::new(ErrorKind::TruncatedObu, Location::InputOffset(obu_start))
                } else {
                    error.with_input_base(after_size_start)
                }
            })?;
    let after_size_bytes = window.byte_position();
    let consumed = usize::try_from(after_size_bytes)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(obu_start)))?;
    r.read_uint8_span(consumed)?; // advance parent past the fields (cannot fail: consumed <= available)
    Ok((ObuHeader { obu_type, obu_redundant_copy, type_specific, extension }, obu_size, after_size_bytes))
}

// ref: ... (see Q2)
fn read_fields_after_obu_size(r: &mut BitCursor<'_>, obu_type: ObuType,
    trimming_status_flag: bool, extension_flag: bool) -> Result<(TypeSpecific, Option<Vec<u8>>)> {
    // existing trim block (END then START) + `read_extension_header(r)` moved here verbatim
}
```

- **Why not option (b)** (check the remaining bytes before each read): uleb128 is variable length, so you cannot pre-check it. It would still read up to 8 bytes past the OBU boundary, so the review's "never read outside" requirement would not hold.
- **ErrorKind:** reuse `TruncatedObu` at `InputOffset(obu_start)`. This is exactly what `read_obu_with_header` reports today for the same defect (`mod.rs:139`), so the observable kind and offset stay stable for `read_obu_with` and `parse_sequence`. No new variant is needed. The variant's message ("OBU payload is shorter than obu_size claims") is slightly loose. Optionally put a doc note on the variant, but **do not change its text** (Parallax contract).
- **Allocation bound:** after the fix, `read_extension_header`'s `read_uint8_span(len)` checks against the **window's** `bytes_remaining()`, which is at most `obu_size`. `.to_vec()` therefore copies at most `obu_size` bytes (at most about 2 MiB, per `validate_obu_size`), and it only runs after the bound check passes. `bounded_vec` is not involved, because this path borrows a slice and then copies it.
- The old `ObuSizeOverflow` `checked_sub` in `read_obu_header_parts` (`:458-466`) goes away. The window position is the count.

## Q4 - Accepted-input and hash impact (scan evidence)

I ran a scratch differential binary (`scratchpad/scan`) that links **HEAD `iamf`** and the **patched copy** side by side. It covered every file under `tests/fixtures/`, `fuzz/corpus/` and `fuzz/artifacts/` (110 files: 44 `.iamf`, 5 `.bin`, the rest text/wav/pcm) and compared `format!("{:?}", parse_sequence(..))` and `dump_annotated(..)`:

```
files=110 identical=110 differing=0
```

An independent raw walker (byte 0 + leb128, re-deriving the trim and extension lengths) found **zero** `.iamf` or `fuzz/corpus/parse_sequence` inputs with after-size > `obu_size`. The only files it flagged were non-IAMF bytes: `.textproto`/`.md`/`.sha256`/`.dump.txt`, and the `obu_roundtrip` corpus, which is `arbitrary` generator input rather than a bitstream.

- **Hash prediction:** **no `semantic_sha256` changes** (confirmed: `tests/parse_reference.rs` passes on the patched copy). The negative expectation `test_000129.iamf` → `StructuralError { kind: "UnexpectedEndOfInput", offset: 53 }` is unchanged (passes).
- The golden `tests/golden` passes, so the output bytes and dump are unchanged.
- Baseline HEAD `cargo test --locked`: 538 passed, 0 failed. Patched copy plus 5 scratch tests: 543 passed, 0 failed. `--features fuzzing --test fuzz_regression`: 5 passed.
- **Accepted-set change (intended):** `read_obu_header` now rejects header-only inputs whose after-size fields exceed `obu_size`. In a narrow class, `read_obu_with`/`parse_sequence` change kind. When `obu_size <= input` and a field overruns **both** `obu_size` and the input (e.g. `f9 01 05 aa`), the result was `UnexpectedEndOfInput@3` and is now `TruncatedObu@0`. The same applies when a leb would have been `Leb128TooLong` beyond `obu_size`. No committed expectation covers this. `tests/sequence_parse.rs:431-434` already accepts either kind.

## Q5 - Test plan (put in `tests/obu_header.rs`, extension/OBU-07 sections)

All vectors below ran green on the patched copy, and the header-only vectors return `Ok` on HEAD (RED before the fix):

| # | Bytes | Call | Expect (kind, at) | HEAD today |
|---|---|---|---|---|
| a1 | `32 00 40 07` (type 6, trim, obu_size 0) | `read_obu_header` and `read_obu_with` | `TruncatedObu`, `InputOffset(0)` | header `Ok(0)`, pos 4 |
| a2 | `32 01 40 07` (obu_size 1, second trim leb outside) | both | `TruncatedObu`, `InputOffset(0)` | header `Ok` |
| b1 | `01 00 01 aa` (review example) | both | `TruncatedObu`, `InputOffset(0)` | header `Ok(0)` |
| b2 | `f9 00 00` (ext flag, obu_size 0, size byte outside) | both | `TruncatedObu`, `InputOffset(0)` | header `Ok(0)` |
| b3 | `f9 01 02 aa bb` (size leb fits, bytes don't) | both | `TruncatedObu`, `InputOffset(0)` | header `Ok(1)` |
| c | `f9 02 80 80 40` + `vec![0; 1 << 20]` (ext len 1 MiB fits input, obu_size 2) | `read_obu_header` | `TruncatedObu`, `InputOffset(0)` | `Ok`, extension len 1048576 |
| d | `33 04 40 07 01 aa` exact fit | `read_obu_header` → trim {64,7}, ext `[aa]`, `byte_position()==6`. `write_obu(header, &[])` == input. `read_obu_with` payload empty | `Ok` | `Ok` |
| e1 | `f9 09 c8 01 00` (obu_size > input, existing test at `:336-347`) | `read_obu_header` | `UnexpectedEndOfInput`, `InputOffset(4)` (absolute, via rebase) | same |
| e2 | `32 06 ff ff ff ff 7f 00` | `read_obu_header` | `Leb128ValueTooLarge`, `InputOffset(2)` (rebased) | same |
| e3 | `20 00 01 00 01 aa` | `parse_sequence` | `TruncatedObu`, `InputOffset(2)` (non-zero OBU start) | same |

Also keep `a_trimmed_audio_frame_reads_its_trim_values_back_in_the_same_order` (`32 82 04 40 07`) as the control for the `min()` clamp. Test (c) proves the bound behaviourally: on HEAD it succeeds with a 1 MiB `Vec`. Also update the doc on `an_extension_length_past_the_end_of_the_input_is_refused_without_allocating` (`:333-335`) so it says "capped against `obu_size`".

## Q6 - Pitfalls

1. **Do not use `r.sub_reader(obu_size)` directly.** It advances the parent past the payload, and it fails for header-only reads on truncated input (breaks `tests/obu_header.rs:188`). Use `r.clone().sub_reader(min)` and then `r.read_uint8_span(consumed)`.
2. **Offsets in the window are relative.** Every non-remapped error must go through `with_input_base(after_size_start)`, or `e1`/`e2` report 2 instead of 4 or 0 instead of 2.
3. **Clippy in integration tests:** `expect_err` inside a non-`#[test]` helper fn in `tests/*.rs` **fails** `clippy --all-targets -D warnings` (observed in the prototype). Keep assertions inside `#[test]` bodies, as `tests/obu_header.rs` does.
4. **Citations:** the new `fn read_fields_after_obu_size` needs a `// ref:` line. Fix the stale `ReadFieldsAfterObuSize` citation (`header.rs:480`). Run `cargo test --locked --test citations`.
5. **Lints:** `declared.min(available)` (`Ord::min`) and `usize::try_from` are lint-clean. Do not write `obu_size - consumed`.
6. **Staging:** code change = `src/obu/header.rs` + `tests/obu_header.rs` (and the optional `src/obu/mod.rs` comment noting the underflow check is now defensive). Stage those paths explicitly, because `docs/` is untracked.
7. Verification gate: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, `cargo test --locked --features fuzzing --test fuzz_regression`. Expect `tests/golden` and `tests/parse_reference` to stay green with no regeneration.

## Assumptions Log

| # | Claim | Risk if wrong |
|---|---|---|
| A1 | Reusing `TruncatedObu` (rather than adding a variant) is acceptable to Parallax | Low: the same kind and offset are already emitted for this defect by `read_obu_with` |

## Sources

- Code read this session: `src/obu/header.rs:330-576`, `src/obu/mod.rs`, `src/obu/boundaries.rs`, `src/bits/reader.rs:75-300`, `src/bits/leb128.rs` `read_uleb128`, `src/error.rs:40-80,252-257`, `src/sequence.rs:512-540`, `src/dump.rs:100-135`, `tests/obu_header.rs`, `tests/fuzz_regression.rs`, `tests/support/reference_expectations.rs`.
- Pinned references: `iamf-tools 848c6ff4 iamf/obu/obu_header.cc:303-342, 106`; `types.h:32`; `libiamf f06e919e code/src/iamf_dec/IAMF_OBU.c:64-122, 246-248`; `bitstream.c:122-142, 161-166`; spec `iamf v1.1.0 index.bs:548`.
- Prototype and scan (scratch only, repo untouched): `/private/tmp/claude-501/-Users-cell-local-iamf-rs/f44e9f64-a98e-4d82-ab0f-6993b52ec19d/scratchpad/{patched,scan,scratch_after_size.rs}`.
