# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Scripts here run from the repository root (`bash tools/<script>`). None of them is part of
`cargo build`.

| Script | Purpose |
|---|---|
| `prove-guards.sh` | Adds one deliberate violation per guardrail in a scratch copy under `target/`: an LGPL dependency, a `HashMap` (directly and through an alias), a slice index, an unchecked `+` and `unwrap`, and an `f64` with `sin`. Each case must fail **and** name the expected lint or licence. Expects 6 PASS lines. If you edit `clippy.toml` or `deny.toml`, add or update the matching case here. |
| `build-reference.sh` | Clones `libiamf` at the pinned SHA into `.reference/`, builds `iamfdec`, runs a smoke decode on a shipped file, and writes `.reference-manifest.json` (the SHA and binary hashes that `tests/reference_manifest.rs` checks against `REFERENCES.md`). Needs CMake, a C/C++ toolchain, python3 and network access. It targets the v1.1.0 tree, which has no submodules and no `-DIAMF_TEST_TOOL` flag. |
| `iamf-tools.Dockerfile` | Builds `iamf-tools` v2.1.0 (`encoder_main`, `decoder_main`) with Bazel inside a container. The base image and Bazelisk are pinned by digest. Build it with `docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/` and pass the result through `IAMF_TOOLS_IMAGE`. |
| `check-codec-dependency-boundary.sh` | CODEC-07: fails if any codec or resampler crate is resolved or imported by the root crate. `tests/codec_dependency_boundary.rs` is the portable version that also runs on Windows. |
| `check-codec-dev-deps.sh` | A one-off preflight for codec crate candidates. It deliberately never runs Cargo in this repository. See `CODEC-DEPENDENCY-PREFLIGHT.md`. |
| `cargo-test-tap.sh`, `red-evidence.sh` | Convert a real `cargo test` run into TAP and wrap it in the JSON record that GSD's `gsd-tools check tdd-red-evidence` gate expects. The exit code is passed through unchanged. Use `--target-test <test fn name>`, because the binary name alone cannot be matched. |
| `experiments/` | Inputs for the experiments recorded in `CONFORMANCE-GATE.md`. |

`codec-fixtures/` is a separate Cargo workspace. See its own `CLAUDE.md`.
