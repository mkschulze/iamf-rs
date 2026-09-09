# Codec dependency preflight

**Status:** PASS
**Executed:** 2026-09-09T14:01:21Z

This is CODEC-06 evidence recorded before adding any codec crate to this
repository. The preflight creates a disposable Cargo package outside the
repository, copies this repository's `deny.toml`, and removes the package on
exit. It does not edit `Cargo.toml` or `Cargo.lock`.

## Environment

- `cargo 1.85.0 (d73d2caf9 2024-12-31)`
- `rustc 1.85.0 (4d91de4e4 2025-02-17)`
- `cargo-deny 0.20.2`
- `jq-1.8.2` (extracts resolved package fields from locked metadata)
- Temporary package: `/private/var/folders/8z/1xzmsslj11g7rv7hf1g73j340000gn/T/iamf-codec-preflight.k4uXtB` (removed after this run)

## Candidate resolution and policy result

| Candidate | Resolved version | Licence | Configuration | Result |
| --- | --- | --- | --- | --- |
| `claxon` | 0.4.3 | Apache-2.0 | exact pin | PASS |
| `flacenc` | 0.5.1 | Apache-2.0 | exact pin; `default-features = false` | PASS |
| `opus` | 0.4.0 | MIT/Apache-2.0 | exact pin | PASS |
| `opusic-sys` (transitive from `opus`) | 0.7.5 | BSD-3-Clause | bundled default build | PASS under copied policy |

Versions and licences in this table are extracted from the generated locked
`metadata.json`; extraction requires exactly one resolved package with each
candidate name and a declared licence, otherwise the script stops before it
can write PASS evidence.

The `flacenc` default feature set (`log`, `par`, and `serde`) is
disabled. Its pure-Rust graph still has a `build.rs`; it is a test-tool
candidate only. `opus` reaches `opusic-sys`, whose default bundled build
uses CMake and compiles bundled C/libopus. That C toolchain/FFI risk is confined
to this isolated preflight and must be proved on Windows, macOS, and Linux
before the candidate is accepted into an ordinary dev-dependency graph. It
must never enter the shipping normal dependency graph.

## Commands executed

All commands below ran from the generated package after its manifest declared
edition `2024`, `rust-version = "1.85"`, and
`license = "MIT OR Apache-2.0"`; its source imports one item from each
candidate.

```text
cargo +1.85.0 check --all-targets
cargo deny check licenses
cargo deny check advisories bans sources
cargo metadata --locked --format-version 1 >metadata.json
cargo tree --edges normal,build >tree-normal-build.txt
```

Every command exited 0. The locked metadata and normal/build-edge tree were
generated before this evidence was written; their success proves Cargo locked
the candidate graph and exposes both normal and build dependencies for review.
