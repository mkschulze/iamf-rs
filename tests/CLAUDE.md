# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## How the integration tests are organised

Every `tests/*.rs` file is its own test binary. Run one binary with `cargo test --locked --test <file-stem>`.
Shared helpers live in `tests/support/` and are pulled in with `#[path = "support/<x>.rs"] mod ...;`.
`support/` is not a crate, so a helper only exists in the binaries that include it.

- `support/fixture.rs`: builds the sample-identity fixture. It is shared by `golden`, `fixture`,
  `conformance` and `codec_fixtures`, so the check and the regeneration build bytes through the same
  code.
- `support/test_000003.rs`: expected values decoded by hand from the vendored `test_000003.iamf`.
- `support/reference_expectations.rs`: field-level expectations for every vendored foreign file,
  used by `parse_reference`.
- `support/parallax_contract.rs`: builds encoder input the way a Parallax delivery would.
- `support/sequence_cases.rs`: sequence shapes used by `round_trip`.

## Where evidence comes from

- **Conformance evidence** comes from hand-decoded vectors (`vectors.rs`, `test_000003` expectations)
  and from the two reference oracles (`conformance.rs`, `parse_reference.rs`). Never write an
  expected byte by capturing this crate's own output (D-25). A test built that way only catches
  regressions; it doesn't show the bytes are correct.
- **Change detection** is `golden.rs` plus `fixtures/golden/`: the `.iamf`, its SHA-256, and the
  annotated dump. The golden must be reproduced on all four CI targets. Regenerate it only through
  `IAMF_REGENERATE_GOLDEN=1 cargo test --test golden regenerate_golden_artifacts -- --nocapture` and
  review the `.dump.txt` diff. Never change the check to match new output.
- **Reference-gated tests** (`conformance`, `reference_manifest`) read `IAMF_REF_DECODER` /
  `IAMF_TOOLS_IMAGE`. When those are unset they print `SKIP` and **pass**; they do not use
  `#[ignore]`. Assert on what the tool actually produced, such as decoded unit counts or PCM length
  and content, never on its exit code. `CONFORMANCE-GATE.md` records that `iamfdec` exits 0 on
  corrupt input.
- **Guard tests** keep repository rules true on every target, Windows included:
  - `citations.rs`: every `read_*`/`write_*` has a `// ref:` line.
  - `codec_dependency_boundary.rs`: no codec crate in the root crate. This is the portable form of
    the shell script.
  - `fixtures_cap.rs`: size cap and file count for vendored fixtures.
  - `allocation_bounds.rs`: count fields can't drive large up-front allocations.
  - `error_shape.rs`: the public error surface.
  - `public_api.rs`: the public API surface.

## Fixtures

- `fixtures/reference/`: files copied byte-for-byte from the **pinned** `libiamf` v1.1.0 and
  `iamf-tools` v2.1.0 trees, with textprotos, WAVs, and `negative/` inputs. Parallax uses this crate
  as a path dependency, so every committed byte lands in its developers' clones. A size cap and a
  file count are enforced. Update `fixtures/MANIFEST.md` when adding a file, and never copy from
  either project's `main`.
- `fixtures/codecs/{flac,opus}/`: packets, source PCM and expected PCM, all immutable. Root tests
  only read them. They are generated and verified from `tools/codec-fixtures/`
  (see `fixtures/codecs/README.md` for the test-name matrix).
- `fixtures/golden/`: produced by this crate. It is not vendored and not AOM-licensed.

## Lints in tests

Clippy runs with `--all-targets -D warnings`, so the crate's deny list applies to tests too.
`clippy.toml` allows `unwrap`/`expect`/`panic` only **inside `#[test]` functions**. A non-test helper
that calls `expect` needs a local `#[allow(clippy::expect_used)]` with a comment giving the reason,
which is how the existing helpers do it. `indexing_slicing` and `arithmetic_side_effects` are not
exempted anywhere: use `.get()` and `checked_*` in tests as well.
