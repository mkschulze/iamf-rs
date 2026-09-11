# Phase 03 Plan 06: Codec dependency boundary summary

## Delivered

- `tests/codec_dependency_boundary.rs` is the portable CODEC-07 oracle. It
  uses test-only `syn = "=2.0.119"` and `toml = "=0.8.23"` to parse Rust
  imports and Cargo manifest/lock semantics without invoking Bash.
- `tools/check-codec-dependency-boundary.sh` retains `--root` and
  `--self-test`, delegates semantic validation through `CODEC_BOUNDARY_ROOT`,
  and independently asserts the locked normal graph is exactly `iamf` and
  `thiserror`.
- `tools/codec-fixtures/deny.toml` supplies a separate policy for the excluded
  fixture generator; root, fuzz, and generator deny scopes remain distinct.

## Evidence

- Parser-valid source canaries cover `use`, grouped and nested imports,
  `extern crate`, raw identifiers, literals/comments, C strings, Unicode
  whitespace, and precise-capture syntax.
- Parser-valid TOML canaries cover quoted/target/nested dependency tables,
  renamed `package` values, decoded workspace exclusion strings, root lock
  package names, and malformed path-bearing diagnostics.
- Fresh branch verification passed: `cargo +1.85.0 test --locked --quiet`,
  boundary test and shell self-test/guard, root/fuzz/generator `cargo deny`,
  exact normal graph, and `git diff --check`.

## Boundary decisions

- Parser crates are direct dev dependencies only; no parser or codec reaches
  the linked production graph.
- The shell guard no longer maintains a second Rust/TOML lexer.
- Root tests consume committed codec artifacts only; regeneration remains in
  the excluded `tools/codec-fixtures` package.

## Commits

- `d5f1a88` through `6ef69c7` — boundary guard, scoped policy, parser-backed
  remediation, and exact parser pins.

## Next

Plan 03-07 closes the offline regression matrix and proves root tests do not
mutate committed codec artifacts.
