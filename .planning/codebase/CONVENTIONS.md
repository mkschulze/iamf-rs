# Coding Conventions

**Analysis Date:** 2026-09-13

## Naming Patterns

**Files:**
- `snake_case.rs`, one file per OBU or primitive: `src/obu/codec_config.rs`, `src/obu/mix_presentation.rs`, `src/bits/leb128.rs`.
- Module roots use `mod.rs` (`src/obu/mod.rs`, `src/bits/mod.rs`, `src/model/mod.rs`). Do not use `foo.rs` + `foo/` for these.
- Integration tests are named after what they check: `tests/golden.rs`, `tests/bits_oracle.rs`, `tests/citations.rs`, `tests/conformance.rs`. Shared test helpers go in `tests/support/*.rs`.

**Functions:**
- `snake_case`. Serialisation uses the prefixes `write_*` and `read_*`: `write_obu`, `read_obu_header`, `write_uleb128_fixed`, `read_uleb128`, `read_codec_config` / `write_codec_config`. `tests/citations.rs` finds functions by these prefixes, so use them.
- Builder-style setters are `with_*` and take `mut self`: `ObuHeader::with_type_specific`, `with_extension` (`src/obu/header.rs:230`).
- Internal variants that return more data get a suffix: `read_obu_header_parts`, `read_obu_with` / `write_obu_with`.

**Variables:**
- `snake_case`, descriptive (`group_index`, `requested`, `payload_len`). Short names only for the reader/writer: `r: &mut BitCursor<'_>`, `w: &mut BitWriter`.

**Types:**
- `PascalCase`. Reference names are translated, not copied: `ObuType`, `CodecConfig`, `FlacDecoderConfig`, `ParamDefinition`.
- Reference constants keep their meaning with a Rust name, and the doc comment gives the reference name: `ENTIRE_OBU_SIZE_MAX` (`kEntireObuSizeMaxTwoMegabytes`), `MAX_LEB128_SIZE` (`kMaxLeb128Size`).
- Put `#[non_exhaustive]` on public enums (`ObuType`, `ErrorKind`, `Location`).

## Code Style

**Formatting:**
- `rustfmt` with default settings. The repo has no `rustfmt.toml`. The component comes from `rust-toolchain.toml`.
- Numeric literals use `_` grouping: `0x0000_007f`, `16_383`, `1_u64`.

**Toolchain:**
- `rust-toolchain.toml` pins `channel = "1.85.0"` exactly, on 4 targets. `Cargo.toml` sets `edition = "2024"` and `rust-version = "1.85"`. CI fails when the running toolchain is not the pinned one (`.github/workflows/ci.yml`).

**Linting (enforced, not optional):**
`Cargo.toml` sets these levels:
```toml
[lints.rust]
unsafe_code = "forbid"          # also restated as #![forbid(unsafe_code)] in src/lib.rs

[lints.clippy]
indexing_slicing = "deny"        # GUARD-03
arithmetic_side_effects = "deny" # GUARD-03
unwrap_used = "deny"             # GUARD-04
expect_used = "deny"             # GUARD-04
panic = "deny"                   # GUARD-04
disallowed_types = "deny"        # GUARD-02 + GUARD-11
disallowed_methods = "deny"      # GUARD-11
```
`clippy.toml` holds the lists those lints read:
- `allow-unwrap-in-tests`, `allow-expect-in-tests` and `allow-panic-in-tests` are all `true`. The test carve-out lives here, not in `#[allow]` attributes.
- `disallowed-types`: `std::collections::HashMap`, `HashSet`, `f32`, `f64`. Each entry has a reason string.
- `disallowed-methods`: `sin/cos/tan/powf/powi/exp/ln/log10/sqrt` on `f32` and `f64`. `round_ties_even` is left off the list on purpose. Do not add it.
- CI runs `cargo clippy --locked --all-targets -- -D warnings`. `tools/prove-guards.sh` adds one deliberate violation for each guard and checks that the lint fires.

**Rules this imposes on `src/`:**
- Do not index with `[]`. Use `.get(..)` and turn `None` into a typed error with `.ok_or_else(...)`.
- Do not use bare `+ - * <<`. Use `checked_*`, `saturating_*` or `checked_shl(..).unwrap_or(0)`. For casts use `u8::try_from(x)` or `u64::from(x)`.
- For ordered collections use `Vec` in bitstream order plus a `by_id()` lookup. Do not use `BTreeMap` for anything whose order can reach the bitstream.
- No floats. The one allowed exception is in `src/model/loudness.rs:94` (`lufs_to_q7_8`), marked `#[allow(clippy::disallowed_types, reason = "...")]`. `rg 'allow.*disallowed_types' src/` must return exactly that one hit.
- Any `#[allow]` in `src/` needs a `reason = "..."`. Keep them to a minimum, because the escape census counts them.

## Import Organization

**Order** (rustfmt sorts within each group):
1. `core::` / `std::` (e.g. `use core::fmt;`)
2. External crates (`bitstream_io`, `proptest`, `hex_literal`, `sha2`). These appear in tests only.
3. `crate::` paths in `src/`, or `iamf::` paths in `tests/`

**Path style:**
- In `src/`, import with absolute `crate::` paths: `use crate::error::{Error, ErrorKind, Location, Result};`. Test modules use `super::`.
- The crate root re-exports `Error, ErrorKind, Finding, Location, Result` (`src/lib.rs`).
- Tests pull in shared helpers with `#[path = "support/fixture.rs"] mod fixture;`.
- `bitstream-io` may only be imported in `tests/bits_oracle.rs`. A verify gate greps `src/` for it.

## Error Handling

**Patterns:**
- There is one error type, `Error { kind: ErrorKind, at: Location }`, in `src/error.rs`. It is built with `thiserror` 2. `anyhow` is never used.
- `Location` records where the error happened: `InputOffset(u64)` for read errors, `OutputOffset(u64)` for write errors, `Field(&'static str)` for validate errors, or `Unlocated`. Do not add an `offset` field to individual `ErrorKind` variants.
- Construct errors at the call site with the current position:
```rust
return Err(Error::new(
    ErrorKind::Leb128SizeInvalid { size },
    Location::OutputOffset(w.output_offset()),
));
```
- Report the offset of the start of the field when the whole field is invalid (`src/bits/leb128.rs` `read_uleb128` saves `start = r.byte_position()` first).
- Size budget: `const _: () = assert!(size_of::<Error>() <= 32);` (`src/error.rs:240`). Keep variant payloads small (`u8`/`u32`).
- Add new `ErrorKind` variants with a short lowercase `#[error("...")]` message. Adding a variant is non-breaking. Changing an existing one breaks the Parallax contract.
- `validate()` returns `Vec<Finding>`, not `Result<(), Error>`, so it can report every problem at once.
- Avoid `#[from]` blanket conversions, because they drop the `Location`.
- Never panic on malformed input. An impossible case falls back to a harmless value (e.g. `u8::try_from(minimal_len(value)).unwrap_or(5)`), with a comment saying why it cannot happen.

## Logging

**Framework:** None. The library does not log. For diagnostics use `src/dump.rs` (`dump_annotated`), which produces a text structural dump. Tests use `println!("SKIP ...")` to report skipped reference-gated clauses.

## Comments

**`// ref:` citations (mechanically enforced by `tests/citations.rs`):**
- Every `fn read_*` / `fn write_*` in `src/` must have a `// ref:` line in the plain-comment block directly above it. Blank lines, attributes and doc comments can sit in between. Constants and types derived from the reference also carry one.
- Format: `// ref: <repo>@<pinned tag> <path> <Symbol>`
```rust
// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadULeb128
// NOTE: optional rider, e.g. the reference's comment is wrong; code is authoritative.
/// Decode a uleb128 of 1..=8 bytes into a `u32`.
pub(crate) fn read_uleb128(r: &mut BitCursor<'_>) -> Result<u32> {
```
- Pinned references used: `iamf-tools@v2.1.0` and `libiamf@v1.1.0` (e.g. `code/src/iamf_dec/IAMF_OBU.c iamf_sequence_header_new`). Use these tags, not HEAD. SHAs are listed in `REFERENCES.md`.
- Sources you may cite: iamf-tools, libiamf, eclipsa-audio-plugin, libear, obr. Never read or cite gpac or libspatialaudio.

**Read/write pairing (D-10):**
- The write function and its reader live in the same file, write first and reader immediately after. The module doc says so (see `src/obu/header.rs`, `src/bits/leb128.rs`). This keeps any asymmetry between the two visible.
- Where possible, build one primitive on another so the two cannot drift. For example, `write_uleb128_minimal` calls `write_uleb128_fixed` with the minimal length.

**Doc comments:**
- Every module starts with `//!` docs covering purpose, the requirement IDs it satisfies (`BITS-03`, `GUARD-09`, `D-20`, ...) and the reasons for its decisions. Always cite requirement and decision IDs.
- Public and `pub(crate)` items get `///` docs explaining **why** (edge cases, reference behaviour), not a restatement of the signature.
- Use inline `//` comments to explain invariants that make an arithmetic or cast safe.

## Function Design

**Size:** Keep functions small and linear, mirroring the reference's hand-written recursive descent. Do not use parser combinators or derive macros for wire layout.

**Parameters:** Pass the cursor or writer first: `fn write_x(w: &mut BitWriter, value: &T) -> Result<()>` and `fn read_x(r: &mut BitCursor<'_>) -> Result<T>`.

**Return Values:** `crate::error::Result<T>`. Mark pure helpers `#[must_use]` and make them `const fn` where possible (`minimal_len`).

**Visibility:** Default to `pub(crate)`. Deliberately keep internals private that would otherwise become public knobs affecting byte identity (e.g. no public LEB mode, D-03).

## Module Design

**Exports:** `src/lib.rs` declares the modules `bits, dump, encoder, error, model, obu, packing, sequence`. `fuzzing` is behind `#[cfg(feature = "fuzzing")] #[doc(hidden)]`. There is one version literal, `pub const SPEC_VERSION: &str = "1.1.0";`.

**Barrel Files:** The `mod.rs` files re-export public items (`iamf::obu::{CodecConfig, read_codec_config, ...}`, `iamf::bits::{BitCursor, BitWriter}`). `tests/public_api.rs` guards the public surface.

**Dependencies:** State each dependency's licence in a comment on the line that adds it (`Cargo.toml`). The shipped linked graph is `iamf` + `thiserror` (check with `cargo tree -e normal,no-proc-macro`).

---

*Convention analysis: 2026-09-13*
