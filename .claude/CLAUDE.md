<!-- GSD:project-start source:PROJECT.md -->

## Project

**iamf-rs**

A Rust implementation of **IAMF** — the Alliance for Open Media's Immersive Audio Model and Formats
bitstream. It is an OBU serialiser and parser, the descriptor model, and an encoder that produces
conformant `.iamf` files. Native codec decoding and timed Audio Element reconstruction live in the
sibling `iamf-decode-rs` repository. It exists because there is **no Rust IAMF
implementation on crates.io** (verified 2026-09-07) and Parallax — a deterministic spatial-audio DAW —
needs IAMF metadata/bitstream support for its `iamf-render-rs` preview path and native export format.

The crate is `iamf`; the repo is `iamf-rs`. Parallax consumes it as a path dependency during
development.

**Core Value:** **A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM
sample-identical.** Everything else — API shape, codec coverage, containers, the decoder — is
secondary to producing bytes the reference implementation accepts.

Sharpened by research (2026-09-08): `libiamf` is a *permissive* reader — it ignores reserved-bit
misuse and clamps an overlong leb128 rather than erroring. **Passing the `libiamf` gate is necessary
but not sufficient for conformance.** The real exit criterion is a byte-diff against an
`iamf-tools`-produced file that is either identical or fully explained in writing.

### Constraints

- **Determinism**: No `HashMap`/`HashSet` anywhere the output byte order can see them. Prefer a `Vec`
  in bitstream order plus a `by_id` lookup — **not** `BTreeMap`, which reorders bitstream-visible
  descriptor order. — `parallax render` is byte-identical across macOS arm64, macOS x86_64, Windows
  MSVC and Linux x64, run to run and target to target. An export must be too.
- **Determinism (CI)**: Byte-identity as a **committed golden fixture** every target must reproduce —
  not a cross-job artifact comparison. — Makes an output change a reviewable PR diff rather than a red
  job with no explanation.
- **No platform transcendentals** in anything affecting output bytes; use `libm` if any appear. — Not
  currently binding: the wire format has no floating-point fields. Keep the rule; expect it to cost
  nothing.
- **Licence allow-list**: Every transitive dependency must satisfy Parallax's `deny.toml`. Preferred:
  MIT, BSD, Apache-2.0, ISC, Zlib, Unlicense, CC0. Allowed with a recorded reason: MPL-2.0 for an
  *unmodified upstream* crate; **NCSA**, added to Parallax's allow-list by user decision 2026-09-07
  (permissive, BSD/MIT-style, no copyleft) to admit `libfuzzer-sys`. **`Unicode-3.0` is mandatory** or
  a clean build fails (`unicode-ident` ← `syn` ← `thiserror-impl`). Rejected outright: GPL, LGPL, AGPL,
  SSPL, EUPL, CDDL, OSL. State each dependency's licence on the line that adds it. — If Parallax cannot
  consume this crate, the crate has no purpose.
- **`cargo-deny` config shape**: The `deny`, `copyleft`, `allow-osi-fsf-free`, `default` and `version`
  keys have been removed from cargo-deny and now error. Rejection is expressed **by omission** from
  `allow`, not by a deny list — record that in a comment so the intent survives review. — Verified
  against cargo-deny 0.20.2.
- **Source licence hygiene**: `libspatialaudio` (LGPL-2.1+) and `gpac` (LGPL-2.1) may not be read or
  ported. `iamf-tools`, `libiamf`, `eclipsa-audio-plugin`, `libear` and `obr` may. — Contamination is
  irreversible; relicensing later needs every contributor's agreement. M5 (ISO-BMFF) is the
  contamination milestone, because `gpac` is the obvious and forbidden reference there.
- **Pin the references**: `iamf-tools` **v2.1.0 `848c6ff`** and `libiamf` **v1.1.0 `f06e919`**, pinned
  by commit in `REFERENCES.md` and cited as `// ref: <project>@<tag>` in the code. — Never `main`:
  both have moved on to a draft-v2.0.0 tree whose files `libiamf@v1.1.0` rejects. (`iamf-tools`' tag
  numbering is not the spec version; v2.1.0 is an IAMF v1.1.0 tree — see `REFERENCES.md` § "The
  version-number caveat".)
- **Pin the spec version**: `SPEC_VERSION = "1.1.0"`, named in the code. — **Decided 2026-09-08.**
  `libiamf` is a v1.1.0 decoder and the Core Value is that it accepts the file; Base-Enhanced does not
  exist in v1.0 at all. The profile enum's legal range and the expanded-layout set follow from this.
- **Error handling**: `thiserror` for errors, never `anyhow` in a library.
- **Parser hardening**: no `unwrap()`/`expect()` outside tests, **plus** `clippy::indexing_slicing` and
  `clippy::arithmetic_side_effects`. — The `unwrap` ban is necessary and nowhere near sufficient; the
  other two matter more and are the two people disable first.
- **Rust edition**: Rust 2024.
- **Fuzzing**: A fuzz target on the OBU parser ships **in M2, alongside the parser** — it may not slip
  past that milestone. It lives in an independent `fuzz/` workspace with its own lockfile, so its
  dependencies never reach Parallax's graph. — Parallax's testing strategy requires a fuzzer on every
  parser.
- **Milestone ordering**: M1 may not be reordered. — A serialiser never read by the reference decoder
  is an untested guess, however tidy the types are.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->

## Technology Stack

## Languages

- Rust, edition 2024 (`rust-version = "1.85"`): all library code in `src/`, integration tests in `tests/`, fuzz targets in `fuzz/fuzz_targets/`, and the codec fixture generator in `tools/codec-fixtures/tests/`
- Bash: CI and guardrail tooling (`tools/build-reference.sh`, `tools/prove-guards.sh`, `tools/check-codec-dependency-boundary.sh`, `tools/check-codec-dev-deps.sh`, `tools/cargo-test-tap.sh`, `tools/red-evidence.sh`)
- Python 3: `tools/experiments/corrupt-fixture.py`, plus inline JSON parsing in `.github/workflows/reference.yml` and `tools/build-reference.sh`
- Dockerfile: `tools/iamf-tools.Dockerfile` (the reference encoder container)
- Textproto: `tools/experiments/two-codec-configs.textproto` (iamf-tools encoder configuration)

## Runtime

- Rust toolchain pinned to exactly `1.85.0` in `rust-toolchain.toml`, with components `clippy`, `rustfmt` and `rust-analyzer`, profile `minimal`
- Targets pinned in `rust-toolchain.toml`: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`
- The fuzz workspace runs on a dated nightly, `nightly-2026-09-01` (`.github/workflows/fuzz.yml`)
- A library crate with no binary, no async runtime, no FFI (`unsafe_code = "forbid"` in `Cargo.toml` `[lints.rust]`)
- Cargo (the version bundled with 1.85.0)
- Lockfiles: present and committed. `Cargo.lock` (root), `fuzz/Cargo.lock`, `tools/codec-fixtures/Cargo.lock`. CI always builds with `--locked`.

## Frameworks

- None. The bitstream reader and writer are written by hand (`src/bits/reader.rs`, `src/bits/writer.rs`, `src/bits/leb128.rs`). There are no parser combinators and no derive macros.
- libtest (`cargo test`): integration tests in `tests/*.rs`, with shared helpers in `tests/support/`
- `proptest` 1.11.0: property and differential round-trip tests
- `cargo-fuzz` 0.13.2 with `libfuzzer-sys` =0.4.13: coverage-guided fuzzing in `fuzz/`
- `clippy`: hardening lints set in `Cargo.toml` `[lints.clippy]`, with lists in `clippy.toml`
- `cargo-deny` 0.20.2: licence, advisory, ban and source gate (`deny.toml`, `fuzz/deny.toml`, `tools/codec-fixtures/deny.toml`)
- CMake plus a C/C++ toolchain: used only to build the `libiamf` reference, never a build dependency of the crate
- Docker, Bazel 7.4.1 (via Bazelisk v1.29.0): used only to build `iamf-tools` inside `tools/iamf-tools.Dockerfile`

## Key Dependencies

- `thiserror` 2.0.20 (MIT OR Apache-2.0): typed errors. CI asserts the linked graph (`cargo tree -e normal,no-proc-macro`) contains exactly `iamf` and `thiserror`.
- `arbitrary` 1.4.2 (MIT OR Apache-2.0), enabled only by the `fuzzing` feature (`fuzzing = ["dep:arbitrary"]`). It provides the structured generator in `src/fuzzing.rs`.
- `bitstream-io` 4.10.0: differential oracle for the hand-rolled bit I/O (`tests/bits_oracle.rs`). It must never be imported from `src/`.
- `proptest` 1.11.0: property tests. Its MSRV of 1.85 sets the crate floor.
- `hex-literal` 1.1.0: hand-computed byte vectors
- `sha2` 0.10.9: golden fixture digest (`tests/golden.rs`)
- `syn` =2.0.119 (`full`, `visit`): AST oracle for the codec dependency boundary (`tests/codec_dependency_boundary.rs`)
- `toml` =0.8.23: manifest and lockfile oracle for the same boundary test
- `fuzz/Cargo.toml` (`iamf-fuzz`): `libfuzzer-sys` =0.4.13 ((MIT OR Apache-2.0) AND NCSA) and `iamf` as a path dependency. The `roundtrip-model` feature enables `iamf/fuzzing`.
- `tools/codec-fixtures/Cargo.toml` (`iamf-codec-fixtures`): `flacenc` =0.5.1, `claxon` =0.4.3, `opus` =0.4.0 (bundles libopus, BSD-3-Clause), `sha2` =0.10.9. It generates and verifies the committed FLAC and Opus fixtures under `tests/fixtures/codecs/`.

## Configuration

- The crate has no runtime configuration. Environment variables only gate tests; see `INTEGRATIONS.md`.
- `RUSTFLAGS="-D warnings"` in CI (`.github/workflows/ci.yml`, `.github/workflows/reference.yml`)
- `Cargo.toml`: `[lints.clippy]` denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`, `panic`, `disallowed_types` and `disallowed_methods`
- `clippy.toml`: bans `HashMap`, `HashSet`, `f32` and `f64` as types, and bans float transcendentals (`sin`, `cos`, `powf`, `sqrt`, and so on). Tests may use `unwrap`, `expect` and `panic` (`allow-*-in-tests`).
- `deny.toml`: licences are allow-list only (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, ISC, Zlib, Unlicense, CC0-1.0, Unicode-3.0). Yanked crates and wildcard versions are denied, and only the crates.io registry is allowed.
- `fuzz/deny.toml` and `tools/codec-fixtures/deny.toml`: the same policy plus `NCSA`, scoped to those workspaces only
- `rust-toolchain.toml`: the single source of truth for the toolchain. CI does not use a toolchain-installer action.
- Features: `default = []`, `fuzzing`. The `fuzz_regression` test requires `fuzzing`.
- Spec pin: `pub const SPEC_VERSION: &str = "1.1.0";` in `src/lib.rs`

## Platform Requirements

- rustup (it installs the pinned 1.85.0 automatically)
- Optional, for reference tests: CMake, a C/C++ compiler, git, python3 and network access (`tools/build-reference.sh`). Docker is needed for the `iamf-tools` tests.
- Optional, for fuzzing: nightly toolchain and `cargo-fuzz` 0.13.2
- Distributed as a library crate (`publish = false`), consumed by Parallax as a path dependency
- Must produce byte-identical output on all four pinned targets

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

## Naming Patterns

- `snake_case.rs`, one file per OBU or primitive: `src/obu/codec_config.rs`, `src/obu/mix_presentation.rs`, `src/bits/leb128.rs`.
- Module roots use `mod.rs` (`src/obu/mod.rs`, `src/bits/mod.rs`, `src/model/mod.rs`). Do not use `foo.rs` + `foo/` for these.
- Integration tests are named after what they check: `tests/golden.rs`, `tests/bits_oracle.rs`, `tests/citations.rs`, `tests/conformance.rs`. Shared test helpers go in `tests/support/*.rs`.
- `snake_case`. Serialisation uses the prefixes `write_*` and `read_*`: `write_obu`, `read_obu_header`, `write_uleb128_fixed`, `read_uleb128`, `read_codec_config` / `write_codec_config`. `tests/citations.rs` finds functions by these prefixes, so use them.
- Builder-style setters are `with_*` and take `mut self`: `ObuHeader::with_type_specific`, `with_extension` (`src/obu/header.rs:230`).
- Internal variants that return more data get a suffix: `read_obu_header_parts`, `read_obu_with` / `write_obu_with`.
- `snake_case`, descriptive (`group_index`, `requested`, `payload_len`). Short names only for the reader/writer: `r: &mut BitCursor<'_>`, `w: &mut BitWriter`.
- `PascalCase`. Reference names are translated, not copied: `ObuType`, `CodecConfig`, `FlacDecoderConfig`, `ParamDefinition`.
- Reference constants keep their meaning with a Rust name, and the doc comment gives the reference name: `ENTIRE_OBU_SIZE_MAX` (`kEntireObuSizeMaxTwoMegabytes`), `MAX_LEB128_SIZE` (`kMaxLeb128Size`).
- Put `#[non_exhaustive]` on public enums (`ObuType`, `ErrorKind`, `Location`).

## Code Style

- `rustfmt` with default settings. The repo has no `rustfmt.toml`. The component comes from `rust-toolchain.toml`.
- Numeric literals use `_` grouping: `0x0000_007f`, `16_383`, `1_u64`.
- `rust-toolchain.toml` pins `channel = "1.85.0"` exactly, on 4 targets. `Cargo.toml` sets `edition = "2024"` and `rust-version = "1.85"`. CI fails when the running toolchain is not the pinned one (`.github/workflows/ci.yml`).
- `allow-unwrap-in-tests`, `allow-expect-in-tests` and `allow-panic-in-tests` are all `true`. The test carve-out lives here, not in `#[allow]` attributes.
- `disallowed-types`: `std::collections::HashMap`, `HashSet`, `f32`, `f64`. Each entry has a reason string.
- `disallowed-methods`: `sin/cos/tan/powf/powi/exp/ln/log10/sqrt` on `f32` and `f64`. `round_ties_even` is left off the list on purpose. Do not add it.
- CI runs `cargo clippy --locked --all-targets -- -D warnings`. `tools/prove-guards.sh` adds one deliberate violation for each guard and checks that the lint fires.
- Do not index with `[]`. Use `.get(..)` and turn `None` into a typed error with `.ok_or_else(...)`.
- Do not use bare `+ - * <<`. Use `checked_*`, `saturating_*` or `checked_shl(..).unwrap_or(0)`. For casts use `u8::try_from(x)` or `u64::from(x)`.
- For ordered collections use `Vec` in bitstream order plus a `by_id()` lookup. Do not use `BTreeMap` for anything whose order can reach the bitstream.
- No floats. The one allowed exception is in `src/model/loudness.rs:94` (`lufs_to_q7_8`), marked `#[allow(clippy::disallowed_types, reason = "...")]`. `rg 'allow.*disallowed_types' src/` must return exactly that one hit.
- Any `#[allow]` in `src/` needs a `reason = "..."`. Keep them to a minimum, because the escape census counts them.

## Import Organization

- In `src/`, import with absolute `crate::` paths: `use crate::error::{Error, ErrorKind, Location, Result};`. Test modules use `super::`.
- The crate root re-exports `Error, ErrorKind, Finding, Location, Result` (`src/lib.rs`).
- Tests pull in shared helpers with `#[path = "support/fixture.rs"] mod fixture;`.
- `bitstream-io` may only be imported in `tests/bits_oracle.rs`. A verify gate greps `src/` for it.

## Error Handling

- There is one error type, `Error { kind: ErrorKind, at: Location }`, in `src/error.rs`. It is built with `thiserror` 2. `anyhow` is never used.
- `Location` records where the error happened: `InputOffset(u64)` for read errors, `OutputOffset(u64)` for write errors, `Field(&'static str)` for validate errors, or `Unlocated`. Do not add an `offset` field to individual `ErrorKind` variants.
- Construct errors at the call site with the current position:
- Report the offset of the start of the field when the whole field is invalid (`src/bits/leb128.rs` `read_uleb128` saves `start = r.byte_position()` first).
- Size budget: `const _: () = assert!(size_of::<Error>() <= 32);` (`src/error.rs:240`). Keep variant payloads small (`u8`/`u32`).
- Add new `ErrorKind` variants with a short lowercase `#[error("...")]` message. Adding a variant is non-breaking. Changing an existing one breaks the Parallax contract.
- `validate()` returns `Vec<Finding>`, not `Result<(), Error>`, so it can report every problem at once.
- Avoid `#[from]` blanket conversions, because they drop the `Location`.
- Never panic on malformed input. An impossible case falls back to a harmless value (e.g. `u8::try_from(minimal_len(value)).unwrap_or(5)`), with a comment saying why it cannot happen.

## Logging

## Comments

- Every `fn read_*` / `fn write_*` in `src/` must have a `// ref:` line in the plain-comment block directly above it. Blank lines, attributes and doc comments can sit in between. Constants and types derived from the reference also carry one.
- Format: `// ref: <repo>@<pinned tag> <path> <Symbol>`
- Pinned references used: `iamf-tools@v2.1.0` and `libiamf@v1.1.0` (e.g. `code/src/iamf_dec/IAMF_OBU.c iamf_sequence_header_new`). Use these tags, not HEAD. SHAs are listed in `REFERENCES.md`.
- Sources you may cite: iamf-tools, libiamf, eclipsa-audio-plugin, libear, obr. Never read or cite gpac or libspatialaudio.
- The read and write functions for a type live in the same file, next to each other. OBU modules put `read_*` first, as `CONTRIBUTING.md` requires (e.g. `src/obu/codec_config.rs`, `src/obu/sequence_header.rs`). `src/bits/leb128.rs` is the exception and has its writers first. Keeping the pair together makes any asymmetry between them visible.
- Where possible, build one primitive on another so the two cannot drift. For example, `write_uleb128_minimal` calls `write_uleb128_fixed` with the minimal length.
- Every module starts with `//!` docs covering purpose, the requirement IDs it satisfies (`BITS-03`, `GUARD-09`, `D-20`, ...) and the reasons for its decisions. Always cite requirement and decision IDs.
- Public and `pub(crate)` items get `///` docs explaining **why** (edge cases, reference behaviour), not a restatement of the signature.
- Use inline `//` comments to explain invariants that make an arithmetic or cast safe.

## Function Design

## Module Design

<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

## System Overview

```text

```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Crate root | `SPEC_VERSION = "1.1.0"`, module declarations, re-export of error types | `src/lib.rs` |
| Bit cursor | Bounded, offset-carrying bit/byte reads; `sub_reader` for OBU payload bounds | `src/bits/reader.rs` |
| Bit writer | Mirror of the cursor, method-for-method in the same order | `src/bits/writer.rs` |
| uleb128 | Minimal encoder (public), `pub(crate)` fixed-size encoder, 8-byte/`u32` capped decoder | `src/bits/leb128.rs` |
| OBU header | `ObuType`, `ObuHeader`, `Trimming`, `obu_size` computation and 2 MiB ceiling | `src/obu/header.rs` |
| OBU framing | Generic `Obu<T>` + `read_obu_with[_header]` / `write_obu_with[_header]` | `src/obu/mod.rs` |
| Per-OBU codecs | Sequence header, codec config (LPCM/FLAC/Opus/AAC decoder configs), audio element, mix presentation, param definition, parameter block, audio frame, temporal delimiter | `src/obu/*.rs` |
| Boundary scan | Structural OBU offset walk without payload parsing | `src/obu/boundaries.rs` |
| Descriptor model | `DescriptorSet` (Vec in bitstream order + `by_id`), `validate() -> Vec<Finding>`, `write_descriptors` | `src/model/mod.rs` |
| Profile selection | `Profile`, `select_minimum_profile` | `src/model/profile.rs` |
| Layouts / loudness | Loudspeaker/ambisonics layouts; `Q7_8`, `lufs_to_q7_8` | `src/model/layout.rs`, `src/model/loudness.rs` |
| Sequence | `parse_sequence`, `ParsedSequence`, streaming `SequenceWriter`, `write_sequence`, `write_parsed_sequence` | `src/sequence.rs` |
| Encoder | `EncoderBuilder` handles -> `build()` -> `(Encoder, IdManifest)` -> `EncodingWriter` | `src/encoder.rs` |
| Channel packing | `SubstreamPlan::for_layout`, `pack_channels_to_substreams` | `src/packing.rs` |
| Dump | `dump_annotated(bytes) -> String` human-readable annotated OBU dump (golden `.dump.txt`) | `src/dump.rs` |
| Fuzz generator | `sequence_from_fuzz_bytes` bounded `arbitrary` model generator (feature `fuzzing`) | `src/fuzzing.rs` |

## Pattern Overview

- Every `fn read_*` / `fn write_*` sits adjacent in the same file (read first) and carries a `// ref: <repo>@<tag> <file> <Symbol>` citation; `tests/citations.rs` enforces it.
- Models are plain owned structs with `pub` fields; ordered collections are `Vec` in wire order. No `HashMap`/`HashSet`/float (enforced via `clippy.toml` disallowed types/methods).
- Unknown/unparsed bytes are preserved (`Obu::trailing`, `UnknownObu`) so parse -> write is byte-identical.
- Zero linked dependencies besides `thiserror`; no `unsafe` (`forbid`); no DSP (no resampling, gain, mixing).
- Offline, streaming-for-memory design: `SequenceWriter` holds no per-unit state.

## Layers

- Purpose: The only module that knows bit offsets; produces `Error` with `Location::InputOffset`/`OutputOffset` natively.
- Contains: `BitCursor<'a>` over `&[u8]`, `BitWriter` over `Vec<u8>`, uleb128.
- Depends on: `src/error.rs`. Used by: `src/obu/`, `src/model/mod.rs`, `src/sequence.rs`, `src/dump.rs`.
- `bitstream-io` is a dev-dependency oracle only (`tests/bits_oracle.rs`); never import it in `src/`.
- Purpose: Serialise/parse individual OBUs. Submodules are private; the public API is flat re-exports from `src/obu/mod.rs`.
- Depends on: bits, error, `model::layout`. Used by: model, sequence, encoder, dump, fuzzing.
- Context-dependent parsing uses explicit context types: `ParamDefinitionRegistry`, `ParameterDataContext` (`src/obu/param_definition.rs`).
- Purpose: Whole-descriptor-set semantics — cross-OBU validation, id lookup (`Identified` trait, `by_id`), profile, layout, loudness.
- Purpose: IA Sequence = descriptor prologue + temporal units. Parse to `ParsedSequence { Vec<SequenceObu> }`; write via state machine.
- Purpose: High-level authoring API for Parallax; assigns wire ids after validation, selects minimum profile, drives `SequenceWriter`.

## Data Flow

### Parse path (`.iamf` bytes -> model)

### Write path (model -> bytes)

### Encoder authoring flow

- No global mutable state except `NEXT_BUILDER_GENERATION: AtomicU64` in `src/encoder.rs` (handle provenance, not output-affecting).
- Writer state lives in `SequenceWriter` (state machine + scratch buffer + param definitions).

## Key Abstractions

## Entry Points

## Architectural Constraints

- **Threading:** Single-threaded, synchronous, offline. No async, no real-time constraints.
- **Global state:** Only `NEXT_BUILDER_GENERATION` in `src/encoder.rs`.
- **Circular imports:** `src/model/mod.rs` imports `crate::obu` types while `src/obu/*` imports `crate::model::layout`; acceptable intra-crate cycle, keep `model::layout` free of `obu` imports.
- **Lints:** `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`, `panic`, `disallowed_types`, `disallowed_methods` are `deny` in `Cargo.toml`; test carve-outs in `clippy.toml`.
- **Dependency graph:** `cargo tree -e normal,no-proc-macro` must list only `iamf` and `thiserror` (CI-asserted). Codec crates live only in `tools/codec-fixtures/`.
- **Spec version:** `SPEC_VERSION` is the only version literal in `src/`.

## Anti-Patterns

### Per-type trailing-byte draining

### Wrapping a foreign bit-stream crate in `src/`

### Signal processing in the writer

## Error Handling

- `Error::new(ErrorKind::X, Location::InputOffset(pos))`, with `checked_*` arithmetic and `.ok_or_else(...)`.
- No `#[from]` conversions; convert at call site to keep the offset.

## Cross-Cutting Concerns

<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
