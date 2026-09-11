# CODEC-07 parser-backed boundary design

## Decision

Replace the parallel hand-written Rust and Perl lexers with one portable Rust
boundary oracle in `tests/codec_dependency_boundary.rs`. It uses `syn` 2 with
the `full` and `visit` features to parse each `src/**/*.rs` file, and `toml`
to parse `Cargo.toml` and `Cargo.lock`. Both crates are direct
`[dev-dependencies]` only; `cargo tree -e normal,no-proc-macro` remains
exactly `iamf` and `thiserror`.

`tools/check-codec-dependency-boundary.sh` retains its `--root PATH` and
`--self-test` interface but owns no Rust or TOML parsing. It invokes the
portable integration test from the script's repository with
`CODEC_BOUNDARY_ROOT=PATH`, then separately proves the locked normal graph
of the supplied target. The same Rust test remains executable directly on
Windows without Bash.

## Rust oracle contract

`boundary_root()` reads `CODEC_BOUNDARY_ROOT` when set, otherwise
`CARGO_MANIFEST_DIR`. It rejects a non-directory, and every temporary canary
directory is still created under a verified temporary base and removed only
after its dedicated prefix and no-symlink checks pass.

`assert_boundary(root)` performs these checks, in this order:

1. Parse `Cargo.toml` with `toml::Value`; require the root `[workspace]`
   `exclude` array to include the decoded string `tools/codec-fixtures`.
2. Parse root `[dependencies]` and `[dev-dependencies]` tables recursively,
   including target-specific dependency tables. Reject direct package keys
   and `package = "..."` renames matching any forbidden codec name. TOML
   comments, quoted keys, literal strings, multiline strings, tables, and
   whitespace are parser input rather than scanner syntax.
3. Parse `Cargo.lock` with `toml::Value`; reject every package table whose
   decoded `name` matches a forbidden codec name.
4. Parse every source file with `syn::parse_file`; a `Visit` implementation
   rejects `ItemUse` and `ItemExternCrate` paths whose first path segment,
   normalized from `_` to `-`, is a forbidden codec. `syn` therefore handles
   strings, comments, raw identifiers, C strings, Unicode whitespace,
   precise-capture syntax, and nested blocks according to Rust grammar.

The existing test uses deliberate manifest, source, workspace, and lock
canaries. It expands these with parser-valid forms that previously bypassed
the lexer: quoted/target/nested dependency tables, renamed package values,
`extern crate`, raw identifiers, precise captures, raw C strings, Unicode
whitespace, and TOML multiline/quoted keys. Every forbidden canary must name
the breached boundary and package; every harmless syntax canary must pass.

## Shell contract

The script resolves and validates `SCRIPT_ROOT` and the caller-provided root,
then runs:

```text
CODEC_BOUNDARY_ROOT="$root" cargo +1.85.0 test --locked \
  --manifest-path "$SCRIPT_ROOT/Cargo.toml" \
  --test codec_dependency_boundary -- --exact codec_dependencies_and_src_imports_remain_outside_the_root_crate
```

It preserves `--self-test` by building a validated temporary canary and
calling itself with `--root`; the integration test is executed from
`SCRIPT_ROOT`, never from the canary. Its self-test proves a valid TOML
dependency insertion and a valid source import fail with specific output.
The script then keeps its independent `cargo tree --locked -e
normal,no-proc-macro` exact-name assertion for the target root.

## Non-goals

- No codec or parser dependency enters `[dependencies]` or the linked graph.
- No production source imports `syn` or `toml`.
- The boundary does not accept malformed Rust or malformed TOML as a clean
  tree: parsing failure is a guard failure with the relevant path.
- No independent Perl/AWK syntax parser remains.

## Verification

```text
cargo +1.85.0 test --locked --test codec_dependency_boundary -- --nocapture
bash tools/check-codec-dependency-boundary.sh --self-test
bash tools/check-codec-dependency-boundary.sh
cargo +1.85.0 deny check licenses
cargo +1.85.0 deny check advisories bans sources
cargo +1.85.0 test --locked --quiet
normal_deps=$(cargo +1.85.0 tree --locked -e normal,no-proc-macro --prefix none | awk 'NF {print $1}' | sort -u)
test "$normal_deps" = "$(printf 'iamf\nthiserror')"
git diff --check
```
