# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`iamf-codec-fixtures` is an **independent, excluded Cargo workspace** with its own `Cargo.lock` and
`deny.toml`. It is the only place codec implementations are allowed:

- `flacenc` 0.5.1 and `claxon` 0.4.3 (both Apache-2.0)
- `opus` 0.4.0 (bundles libopus, BSD-3-Clause)

The root crate must never depend on these (CODEC-07). `tools/check-codec-dependency-boundary.sh` and
`tests/codec_dependency_boundary.rs` enforce that.

The package is test-only (`autolib`/`autobins` are off). Generation and verification are
`#[ignore]`d tests, so a normal run only compiles them, and neither root `cargo test` nor
`--include-ignored` reaches them. Run each one explicitly with its environment variable, from the
repository root:

```sh
# regenerate (overwrites committed immutable artifacts — only on purpose)
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact generate_flac_corpus
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact generate_opus_corpus

# verify committed corpora with independent decoders (what CI runs)
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact verify_flac_corpus
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact verify_opus_corpus

cargo deny --manifest-path tools/codec-fixtures/Cargo.toml --config tools/codec-fixtures/deny.toml --all-features check
```

These artifacts are immutable once committed: `packet-*.bin`, `source.s16le`, `expected.s16le`,
`libiamf-expected.s16le` and each `MANIFEST.md`. Regenerating them changes what the root codec
framing tests (`tests/codec_fixtures.rs`) compare against, so the diff needs review.
`libiamf-expected.s16le` comes from the pinned reference decoder, not from this tool.
