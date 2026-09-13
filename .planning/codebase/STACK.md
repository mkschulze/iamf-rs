# Technology Stack

**Analysis Date:** 2026-09-13

## Languages

**Primary:**
- Rust, edition 2024 (`rust-version = "1.85"`): all library code in `src/`, integration tests in `tests/`, fuzz targets in `fuzz/fuzz_targets/`, and the codec fixture generator in `tools/codec-fixtures/tests/`

**Secondary:**
- Bash: CI and guardrail tooling (`tools/build-reference.sh`, `tools/prove-guards.sh`, `tools/check-codec-dependency-boundary.sh`, `tools/check-codec-dev-deps.sh`, `tools/cargo-test-tap.sh`, `tools/red-evidence.sh`)
- Python 3: `tools/experiments/corrupt-fixture.py`, plus inline JSON parsing in `.github/workflows/reference.yml` and `tools/build-reference.sh`
- Dockerfile: `tools/iamf-tools.Dockerfile` (the reference encoder container)
- Textproto: `tools/experiments/two-codec-configs.textproto` (iamf-tools encoder configuration)

## Runtime

**Environment:**
- Rust toolchain pinned to exactly `1.85.0` in `rust-toolchain.toml`, with components `clippy`, `rustfmt` and `rust-analyzer`, profile `minimal`
- Targets pinned in `rust-toolchain.toml`: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`
- The fuzz workspace runs on a dated nightly, `nightly-2026-09-01` (`.github/workflows/fuzz.yml`)
- A library crate with no binary, no async runtime, no FFI (`unsafe_code = "forbid"` in `Cargo.toml` `[lints.rust]`)

**Package Manager:**
- Cargo (the version bundled with 1.85.0)
- Lockfiles: present and committed. `Cargo.lock` (root), `fuzz/Cargo.lock`, `tools/codec-fixtures/Cargo.lock`. CI always builds with `--locked`.

## Frameworks

**Core:**
- None. The bitstream reader and writer are written by hand (`src/bits/reader.rs`, `src/bits/writer.rs`, `src/bits/leb128.rs`). There are no parser combinators and no derive macros.

**Testing:**
- libtest (`cargo test`): integration tests in `tests/*.rs`, with shared helpers in `tests/support/`
- `proptest` 1.11.0: property and differential round-trip tests
- `cargo-fuzz` 0.13.2 with `libfuzzer-sys` =0.4.13: coverage-guided fuzzing in `fuzz/`

**Build/Dev:**
- `clippy`: hardening lints set in `Cargo.toml` `[lints.clippy]`, with lists in `clippy.toml`
- `cargo-deny` 0.20.2: licence, advisory, ban and source gate (`deny.toml`, `fuzz/deny.toml`, `tools/codec-fixtures/deny.toml`)
- CMake plus a C/C++ toolchain: used only to build the `libiamf` reference, never a build dependency of the crate
- Docker, Bazel 7.4.1 (via Bazelisk v1.29.0): used only to build `iamf-tools` inside `tools/iamf-tools.Dockerfile`

## Key Dependencies

**Critical (linked, shipping graph):**
- `thiserror` 2.0.20 (MIT OR Apache-2.0): typed errors. CI asserts the linked graph (`cargo tree -e normal,no-proc-macro`) contains exactly `iamf` and `thiserror`.

**Optional:**
- `arbitrary` 1.4.2 (MIT OR Apache-2.0), enabled only by the `fuzzing` feature (`fuzzing = ["dep:arbitrary"]`). It provides the structured generator in `src/fuzzing.rs`.

**Dev-dependencies (root `Cargo.toml`):**
- `bitstream-io` 4.10.0: differential oracle for the hand-rolled bit I/O (`tests/bits_oracle.rs`). It must never be imported from `src/`.
- `proptest` 1.11.0: property tests. Its MSRV of 1.85 sets the crate floor.
- `hex-literal` 1.1.0: hand-computed byte vectors
- `sha2` 0.10.9: golden fixture digest (`tests/golden.rs`)
- `syn` =2.0.119 (`full`, `visit`): AST oracle for the codec dependency boundary (`tests/codec_dependency_boundary.rs`)
- `toml` =0.8.23: manifest and lockfile oracle for the same boundary test

**Isolated workspaces (excluded from the root workspace via `exclude = ["fuzz", "tools/codec-fixtures"]`):**
- `fuzz/Cargo.toml` (`iamf-fuzz`): `libfuzzer-sys` =0.4.13 ((MIT OR Apache-2.0) AND NCSA) and `iamf` as a path dependency. The `roundtrip-model` feature enables `iamf/fuzzing`.
- `tools/codec-fixtures/Cargo.toml` (`iamf-codec-fixtures`): `flacenc` =0.5.1, `claxon` =0.4.3, `opus` =0.4.0 (bundles libopus, BSD-3-Clause), `sha2` =0.10.9. It generates and verifies the committed FLAC and Opus fixtures under `tests/fixtures/codecs/`.

## Configuration

**Environment:**
- The crate has no runtime configuration. Environment variables only gate tests; see `INTEGRATIONS.md`.
- `RUSTFLAGS="-D warnings"` in CI (`.github/workflows/ci.yml`, `.github/workflows/reference.yml`)

**Build and lint:**
- `Cargo.toml`: `[lints.clippy]` denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`, `panic`, `disallowed_types` and `disallowed_methods`
- `clippy.toml`: bans `HashMap`, `HashSet`, `f32` and `f64` as types, and bans float transcendentals (`sin`, `cos`, `powf`, `sqrt`, and so on). Tests may use `unwrap`, `expect` and `panic` (`allow-*-in-tests`).
- `deny.toml`: licences are allow-list only (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, ISC, Zlib, Unlicense, CC0-1.0, Unicode-3.0). Yanked crates and wildcard versions are denied, and only the crates.io registry is allowed.
- `fuzz/deny.toml` and `tools/codec-fixtures/deny.toml`: the same policy plus `NCSA`, scoped to those workspaces only
- `rust-toolchain.toml`: the single source of truth for the toolchain. CI does not use a toolchain-installer action.
- Features: `default = []`, `fuzzing`. The `fuzz_regression` test requires `fuzzing`.
- Spec pin: `pub const SPEC_VERSION: &str = "1.1.0";` in `src/lib.rs`

## Platform Requirements

**Development:**
- rustup (it installs the pinned 1.85.0 automatically)
- Optional, for reference tests: CMake, a C/C++ compiler, git, python3 and network access (`tools/build-reference.sh`). Docker is needed for the `iamf-tools` tests.
- Optional, for fuzzing: nightly toolchain and `cargo-fuzz` 0.13.2

**Production:**
- Distributed as a library crate (`publish = false`), consumed by Parallax as a path dependency
- Must produce byte-identical output on all four pinned targets

---

*Stack analysis: 2026-09-13*
