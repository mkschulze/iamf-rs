# Task 1 report — four-target root gates and Linux generator gate

## Delivered

- Added a named root-matrix regression command covering `descriptors`,
  `error_shape`, `round_trip`, `temporal`, `sequence_parse`,
  `codec_fixtures`, and `codec_dependency_boundary`. The command keeps
  `--target ${{ matrix.target }}` and therefore runs on every existing matrix
  entry.
- Kept the four original `- os:` entries unchanged, preserving macOS native,
  macOS Rosetta x86_64, Windows, and Linux coverage as well as the existing
  LF, pinned-toolchain, golden-fixture, and fuzz-corpus steps.
- Added Linux-only guardrail steps for the root CODEC-07 boundary script, a
  no-run compile of the excluded codec-fixture workspace, and a cargo-deny
  policy check scoped to that workspace and its own deny configuration.
- Added the two documented ignored corpus verifier commands exactly with
  `CODEC_FIXTURE_INPUT` paths. No `generate_*` command was added; committed
  artifacts are verified but never regenerated in CI.
- Documented that the generator stays out of the root matrix and remains a
  Linux-only build, so bundled Opus failures cannot broaden the shipping graph.

## Verification

- Parsed `.github/workflows/ci.yml` with Ruby YAML, confirmed all requested
  command fragments, confirmed no generator command, and asserted exactly four
  `- os:` matrix entries.
- `git diff --check`
- `cargo test --locked --test descriptors --test error_shape --test round_trip --test temporal --test sequence_parse --test codec_fixtures --test codec_dependency_boundary`
  — 157 tests passed.
- `bash tools/check-codec-dependency-boundary.sh` — passed.
- `cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --no-run`
  — passed, including the bundled Opus build.
- `cargo deny --manifest-path tools/codec-fixtures/Cargo.toml --config tools/codec-fixtures/deny.toml --all-features check`
  — advisories, bans, licences, and sources passed.
- Both ignored verifier commands passed with their committed FLAC and Opus
  input directories.

## Local platform note

The literal matrix-target command was compiled for `aarch64-apple-darwin` on
this host but could not be executed because the current machine runs a
different macOS CPU architecture. The same selected root tests passed when run
for the host target; GitHub executes the ARM matrix command on its native ARM
macOS runner and retains the existing Rosetta runner configuration for x86_64.
