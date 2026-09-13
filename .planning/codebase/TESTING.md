# Testing Patterns

**Analysis Date:** 2026-09-13

## Test Framework

**Runner:**
- Built-in `libtest` (`cargo test`), stable toolchain 1.85.0 (`rust-toolchain.toml`).
- Config: `Cargo.toml` `[dev-dependencies]` and `[[test]]` entries. `tests/fuzz_regression.rs` needs `required-features = ["fuzzing"]`.

**Assertion / helper libraries (dev-only):**
- `assert_eq!` / `assert!` from std. There is no `pretty_assertions`.
- `hex-literal` 1.1.0: `hex!("f8 06 ...")` for hand-computed vectors.
- `proptest` 1.11.0: property and differential tests.
- `bitstream-io` 4.10.0: differential oracle, used only in `tests/bits_oracle.rs`.
- `sha2` 0.10.9: golden and fixture digests (works on Windows, unlike `shasum`).
- `syn` =2.0.119 / `toml` =0.8.23: AST and manifest oracles (`tests/codec_dependency_boundary.rs`).

**Run Commands:**
```bash
cargo test --locked                                   # PR gate (offline, all 4 targets)
cargo test --locked --features fuzzing --test fuzz_regression   # corpus replay
cargo test --locked -- --include-ignored              # reference job (with IAMF_REF_DECODER set)
cargo test --locked --test conformance -- --nocapture --test-threads=1
IAMF_REGENERATE_GOLDEN=1 cargo test --test golden regenerate -- --nocapture  # regenerate golden, then READ the diff
bash tools/cargo-test-tap.sh [--test NAME]            # TAP 13 output for the GSD TDD RED gate
bash tools/prove-guards.sh                            # proves each clippy guard fires
cargo fuzz run parse_sequence                         # nightly, from fuzz/
```
There is no coverage tool and no watch mode.

## Test File Organization

**Location:**
- Most tests are integration tests under `tests/`, one binary per concern, using only the public `iamf::` API.
- Unit tests are inline `#[cfg(test)] mod tests` blocks, used only for `pub(crate)` items that integration tests cannot reach: `src/bits/leb128.rs:119`, `src/obu/header.rs:496`, `src/encoder.rs:1239`. Their `//!` doc says why they live there.

**Naming:**
- Files: `tests/<concern>.rs` (`vectors`, `bits_oracle`, `golden`, `citations`, `error_shape`, `public_api`, `descriptors`, `temporal`, `sequence_parse`, `parse_reference`, `conformance`, `refcorpus`, `reference_manifest`, `fuzz_regression`, `codec_fixtures`, `codec_dependency_boundary`, `encoder_builder`, `encoder_streaming`, `fixtures_cap`, `profile`).
- Test functions are long sentences stating the property: `offset_0x00_ia_sequence_header_header_byte_writes_as_f8`, `minimal_len_changes_at_every_seven_bit_boundary`, `expectation_inventory_is_a_bijection`.
- A vector test is named after the byte offset in the reference file it reproduces.

**Structure:**
```
tests/
├── *.rs                     # one test binary per concern
├── support/                 # shared helpers, pulled in with #[path]
│   ├── fixture.rs           # fixture construction, channel-mismatch diagnosis
│   ├── reference_expectations.rs  # POSITIVE_/NEGATIVE_EXPECTATIONS tables
│   ├── sequence_cases.rs, test_000003.rs, parallax_contract.rs
└── fixtures/
    ├── MANIFEST.md, PHASE2-EXPECTATION-PROVENANCE.md
    ├── golden/              # phase1_sample_identity.{iamf,iamf.sha256,dump.txt}
    ├── reference/           # vendored test_NNNNNN.iamf + .textproto; negative/
    └── codecs/{flac,opus}/  # packet-*.bin, source/expected .s16le, MANIFEST.md
fuzz/corpus/{parse_sequence,obu_roundtrip}/, fuzz/artifacts/
```

## Test Structure

**Suite Organization:**
```rust
//! BITS-06 / D-25 — hand-computed bit-primitive vectors.
//! (module doc: what the file proves, what it does NOT prove, provenance)

use hex_literal::hex;
use iamf::bits::{BitCursor, BitWriter};

/// `0x00` of `test_000003.iamf`: obu_type(5)=11111, three flags clear -> 0xF8.
#[test]
fn offset_0x00_ia_sequence_header_header_byte_writes_as_f8() {
    let mut w = BitWriter::new();
    w.write_unsigned(31, 5).expect("obu_type fits in 5 bits");
    ...
}
```

**Patterns:**
- Every test file opens with a `//!` doc giving the requirement IDs, what the test proves and what it does not (e.g. `tests/golden.rs` says the golden detects change but does not prove correctness).
- Inside `#[test]` fns, `expect("reason")` / `unwrap` / `panic!` are allowed through `clippy.toml`. **Helper functions are not covered by that allowance.** Mark them with a narrow `#[allow(clippy::panic)]` plus a comment (`tests/golden.rs` `read_golden`), or put `#![allow(clippy::expect_used, clippy::panic)]` at the top of the file (`tests/parse_reference.rs`, `tests/fuzz_regression.rs`).
- Panic messages on fixture failures say how to fix the problem, e.g. the regeneration command.
- Find fixtures with `env!("CARGO_MANIFEST_DIR")`. Sort directory listings so the order is deterministic, and normalise `\\` to `/` for Windows.
- No setup/teardown framework. Tests that shell out create their own scratch directories.

## Mocking

**Framework:** None. Nothing is mocked.

**Instead, use independent derivations that must agree:**
- Hand-decoded vectors (`tests/vectors.rs`) from `tests/fixtures/reference/test_000003.iamf` and its `.textproto`. **Never write expected bytes by capturing this crate's output** (D-25).
- A differential oracle: `bitstream-io` vs `BitCursor`/`BitWriter` under proptest (`tests/bits_oracle.rs`). Independent helpers such as `expected_minimal_len` compute results a second way.
- Real reference binaries (`iamfdec`, the `iamf-tools` container), not stubs.

**What NOT to Mock:** the reference decoder, codec packets (commit real fixtures in `tests/fixtures/codecs/`), or the filesystem.

## Fixtures and Factories

**Test Data:**
```rust
const PINNED_POSITIVE_FIXTURES: [(&str, &str); 4] = [
    ("noise_1024samp_5p1_opus.iamf", "2115fd08ee...1cae"),
    ...
];
```
- Pin fixtures by SHA-256. Every foreign `.iamf` needs an entry in `tests/support/reference_expectations.rs`. `expectation_inventory_is_a_bijection` fails when a file has no expectation or an expectation has no file.
- Golden = three artifacts (`.iamf`, `.sha256`, annotated `.dump.txt` from `iamf::dump::dump_annotated`), so an output change shows up as a readable PR diff.
- Record provenance in `tests/fixtures/MANIFEST.md`, `codecs/*/MANIFEST.md` and `PHASE2-EXPECTATION-PROVENANCE.md`. Codec fixtures are generated by the separate `tools/codec-fixtures` crate, which is excluded from the workspace.
- `tests/fixtures_cap.rs` enforces a size cap on committed fixtures.

## Coverage

**Requirements:** No percentage target. Coverage is enforced structurally instead:
- `tests/citations.rs`: every `read_*`/`write_*` has a `// ref:`.
- `tests/public_api.rs`, `tests/error_shape.rs`: API and error-type shape.
- `tests/codec_dependency_boundary.rs` + `tools/check-codec-dependency-boundary.sh`: dependency containment.
- `tools/prove-guards.sh`: lints actually fire.

## Test Types

**Unit Tests:** inline modules for `pub(crate)` primitives. The expected values are computed by hand.

**Property / differential tests:** `proptest!` blocks with dependent strategies (`prop_flat_map`) so that every generated input is valid. Do not mask values after generation. When a property fails, shrinking gives a minimal counterexample.

**Golden / byte-identity:** `tests/golden.rs` runs offline on all 4 targets (aarch64/x86_64 macOS, Windows MSVC, Linux) in `.github/workflows/ci.yml`. It includes the same-process double-encode check (GUARD-10).

**Reference-parse tests:** `tests/parse_reference.rs`, `tests/sequence_parse.rs`, `tests/refcorpus.rs`. They parse vendored `iamf-tools`/`libiamf` fixtures, check fields and re-serialise to the identical bytes. Negative fixtures have their disposition recorded.

**Reference-decoder conformance:** `tests/conformance.rs`.
- Finds `iamfdec` through `IAMF_REF_DECODER`. An empty value counts as unset. The `iamf-tools` container comes from `IAMF_TOOLS_IMAGE` (default `iamf-tools:v2.1.0`), checked with `docker image inspect`.
- When the reference is absent the test **prints `SKIP ...` and returns**, so it passes offline (CONF-10). The Linux `.github/workflows/reference.yml` job builds the reference with `tools/build-reference.sh`, runs `--include-ignored`, then `--test conformance --test-threads=1`.
- The exit code is never trusted. Assertions run in this order: output larger than a WAV header, then sample count equal, then every sample bit-identical. Decode with `-disable_limiter` and also with the default limiter, then compare the two. No tolerances and no permutation knobs (D-19).

**Fuzz:**
- `fuzz/` is an excluded workspace with its own `Cargo.lock` and `deny.toml`, pinning `libfuzzer-sys =0.4.13`. Targets are `fuzz_targets/parse_sequence.rs` (`let _ = iamf::sequence::parse_sequence(data);`) and `obu_roundtrip.rs` (feature `roundtrip-model` → `iamf/fuzzing`, `arbitrary`). The nightly cron is `.github/workflows/fuzz.yml`.
- Corpus regression (`tests/fuzz_regression.rs`) runs on the stable PR gate. It replays `fuzz/corpus/*` and `fuzz/artifacts/*`, rejects symlinks, and checks pinned positive fixtures by hash. When a fuzzer finds a crash, commit it to `fuzz/artifacts/<target>/` so it is replayed from then on.

**E2E Tests:** The conformance encode→`iamfdec`→PCM compare above is the end-to-end test.

## Common Patterns

**Error Testing:** match both kind and location exactly:
```rust
let err = read_uleb128(&mut r).unwrap_err();
assert_eq!(err.kind(), &ErrorKind::Leb128TooLong);
assert_eq!(err.at(), Location::InputOffset(0));
```
(See `tests/error_shape.rs`, `tests/vectors.rs`. `Error` is `Clone + PartialEq + Eq` so it can be asserted directly.)

**Round-trip:** `parse_sequence(bytes)` → `write_parsed_sequence(..)` → `assert_eq!` against the original bytes.

**Async Testing:** Not applicable (synchronous crate).

**TDD evidence:** `tools/red-evidence.sh` and `tools/cargo-test-tap.sh` record RED runs for GSD's TDD gate.

---

*Testing analysis: 2026-09-13*
