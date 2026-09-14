# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`fuzz/` is an **independent Cargo workspace** with its own `Cargo.lock` and `deny.toml`. The root
workspace excludes it, so `libfuzzer-sys`, which is licensed `(MIT OR Apache-2.0) AND NCSA`, never
enters the root lockfile, Parallax's dependency graph, or the root licence allow-list. Don't add
`NCSA` to the root `deny.toml`, and don't make the root crate depend on anything in here. CI checks
that `libfuzzer-sys` is absent from the root lock.

## Targets

- `parse_sequence`: raw bytes → both `iamf::sequence::parse_sequence` and `iamf::sequence::SequenceReader`.
  Any panic, or any disagreement between the two, counts as a failure.
  Seeds are 4 positive `iamf-tools@v2.1.0` files whose SHA-256 is pinned in `CORPUS.md`.
- `obu_roundtrip` (needs `--features roundtrip-model`, which enables `iamf/fuzzing`):
  `Unstructured` → a bounded model from `src/fuzzing.rs` → bytes → parse, then checks the model
  round-trips. The grammar lives in the root crate's `src/fuzzing.rs` so the stable replay test uses
  exactly the same generator.

## Commands

The nightly date and cargo-fuzz version are the ones CI pins (`.github/workflows/fuzz.yml`). Run
from the repository root:

```sh
cargo +stable install cargo-fuzz --version 0.13.2 --locked
cargo +nightly-2026-09-01 fuzz run parse_sequence fuzz/corpus/parse_sequence -- -max_total_time=300
cargo +nightly-2026-09-01 fuzz run obu_roundtrip --features roundtrip-model fuzz/corpus/obu_roundtrip -- -max_total_time=300
cargo deny --manifest-path fuzz/Cargo.toml --config fuzz/deny.toml --all-features check
```

Stable replay of every committed corpus and artifact file. It runs on all four CI targets:
`cargo test --locked --features fuzzing --test fuzz_regression -- --nocapture`.

## Promoting a crash

Follow `CORPUS.md` § "Promoting a failure":

1. Minimise with `fuzz tmin`.
2. Commit **only the minimised input** under `fuzz/artifacts/<target>/`.
3. Add a named stable regression test if the crash points to a specific invariant.

The replay test fails on corpus files that are empty, symlinked or unreadable, so don't commit
placeholders. `fuzz/target/` is gitignored; `corpus/` and `artifacts/` are tracked.
