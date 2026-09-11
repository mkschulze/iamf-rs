# CODEC-07 parser-backed boundary implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace both hand-written source/manifest lexers with a single parser-backed portable boundary oracle and make the shell guard delegate to it.

**Architecture:** The Rust integration test owns semantic validation of Cargo TOML, Cargo lock TOML, and Rust source ASTs. `syn 2.0.119` parses source with `full`/`visit`; `toml 0.8.23` decodes manifest and lock values. The Bash guard has no source/TOML parser: it invokes the test with `CODEC_BOUNDARY_ROOT` and retains only target validation, safe canary setup, and the exact locked normal graph assertion.

**Tech Stack:** Rust 1.85; `syn = 2.0.119` features `full,visit`; `toml = 0.8.23`; Cargo; Bash.

**Spec:** `.planning/phases/03-flac-and-opus-framing/03-06-PARSER-BOUNDARY-SPEC.md`; original acceptance requirements `.planning/phases/03-flac-and-opus-framing/03-06-PLAN.md`; CODEC-06/07 in `.planning/REQUIREMENTS.md`.

## Global Constraints

- `syn` and `toml` are exact direct `[dev-dependencies]` only; no parser or codec enters `[dependencies]`, production source, or the linked normal graph.
- Root codec dependencies remain forbidden in root dependency and dev-dependency declarations; `tools/codec-fixtures` remains the only codec graph.
- Root workspace `exclude` must contain decoded `tools/codec-fixtures`; root lock must contain no forbidden package name.
- The portable test invokes no shell and runs on Windows MSVC; the shell guard owns no duplicated Rust/TOML syntax parser.
- Every malformed Rust/TOML source is a failing boundary check naming its path; comments, literals, raw identifiers, Unicode whitespace, valid Cargo tables/keys, and valid TOML strings are parsed semantically.
- The root normal graph remains exactly `iamf` then `thiserror`; deny/lints/root policy must not be weakened.
- Shell self-test cleanup may remove only its validated temporary canary directory.

---

### Task 1: Build the portable parser-backed boundary oracle

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `tests/codec_dependency_boundary.rs`

**Interfaces:**
- Consumes: `CODEC_BOUNDARY_ROOT: Option<OsString>`; when absent, the test root is `env!("CARGO_MANIFEST_DIR")`.
- Produces: `assert_boundary(root: &Path) -> Result<(), String>` used by the real-tree test and every parser-valid canary; an AST visitor that reports `src import` and the original forbidden package; TOML boundary helpers that report the manifest/lock breach.

- [ ] **Step 1: Add exact test-only parser dependencies and RED parser-valid canaries**

  Add these exact dev dependencies, preserving existing comments and licence annotations:

  ```toml
  syn = { version = "2.0.119", features = ["full", "visit"] } # MIT OR Apache-2.0 — CODEC-07 AST oracle
  toml = "0.8.23" # MIT OR Apache-2.0 — CODEC-07 manifest/lock oracle
  ```

  Regenerate the lock through Cargo, then add canaries that the current lexer either accepts incorrectly or rejects incorrectly:

  ```rust
  // Must reject as real imports, including nested blocks and syntax that a lexer confuses.
  "extern crate opus;"
  "use r#opus::Encoder;"
  "fn f() -> impl Sized + use<> { use opus::Encoder; () }"
  "const HELP: &std::ffi::CStr = cr#\"\"\"#; use opus::Encoder;"
  "use\u{0085}opus::Encoder;"

  // Must accept because these contain no ItemUse/ItemExternCrate.
  "const HELP: &std::ffi::CStr = cr#\"\"; use opus;\"#;"
  "const LABEL: &str = r#\"use opus::Encoder;\"#;"
  ```

  Add parser-valid TOML canaries for:

  ```toml
  [dependencies] # legal table comment
  "opus" = "0.4"

  [target.'cfg(unix)'.dev-dependencies]
  renamed = { package = 'opus', version = "0.4" }

  [workspace]
  exclude = ["""prefix""tools/codec-fixtures""suffix"""]
  ```

  Also add positive manifests using quoted `workspace`/`exclude` keys and a decoded multiline or escaped entry exactly equal to `tools/codec-fixtures`; retain comments that mention forbidden packages as harmless.

  Run:

  ```text
  cargo +1.85.0 test --locked --test codec_dependency_boundary -- --nocapture
  ```

  Expected: RED due to the old textual/lexer boundary behavior before it is deleted.

- [ ] **Step 2: Replace the source lexer with `syn` AST traversal**

  Implement `boundary_root()` that validates `CODEC_BOUNDARY_ROOT` when set. Parse each `src/**/*.rs` file with `syn::parse_file`; parse failure returns `cannot parse Rust source <path>: <syn error>`.

  Implement `Visit` for `syn::ItemUse` and `syn::ItemExternCrate`. Resolve the leading `UseTree` path/rename/group segment and each group branch; normalize the segment identifier by replacing `_` with `-`, then compare it to the forbidden package set. `extern crate r#opus as codec;` normalizes the `Ident` supplied by `syn`. Return `src import must not name codec <package> (<path>)` on the first forbidden AST item. The visitor must recurse into nested modules and function blocks through `syn::visit::Visit` defaults.

- [ ] **Step 3: Replace textual Cargo checks with decoded TOML values**

  Parse `Cargo.toml` and `Cargo.lock` through `toml::Value`, returning `cannot parse <path>: <toml error>` on malformed input. Require a decoded string member exactly equal to `tools/codec-fixtures` in `workspace.exclude`.

  Walk every TOML table recursively. Treat a table as a dependency table when its key path ends in `dependencies`, `dev-dependencies`, or `build-dependencies`, including a target-specific parent. In each such table reject either a direct key equal to a forbidden name or a table value whose decoded `package` string equals a forbidden name. Walk lock `package` arrays and reject decoded `name` values. Comments and arbitrary string content never create tables, keys, or package names.

- [ ] **Step 4: Green verification and commit**

  Run:

  ```text
  cargo +1.85.0 test --locked --test codec_dependency_boundary -- --nocapture
  cargo +1.85.0 deny check licenses
  cargo +1.85.0 deny check advisories bans sources
  normal_deps=$(cargo +1.85.0 tree --locked -e normal,no-proc-macro --prefix none | awk 'NF {print $1}' | sort -u)
  test "$normal_deps" = "$(printf 'iamf\nthiserror')"
  git diff --check
  ```

  Commit:

  ```text
  git add Cargo.toml Cargo.lock tests/codec_dependency_boundary.rs
  git commit -m "test(03-06): parse codec boundary semantics"
  ```

### Task 2: Delegate the Linux shell guard to the portable oracle

**Files:**
- Modify: `tools/check-codec-dependency-boundary.sh`

**Interfaces:**
- Consumes: `CODEC_BOUNDARY_ROOT` and the Task 1 integration-test entry point; retains `--root PATH` and `--self-test`.
- Produces: a shell check that reports Rust-oracle failure output for any target root and separately requires the target's exact locked normal graph.

- [ ] **Step 1: RED shell delegation canaries**

  Extend `--self-test` with a valid TOML source canary for a quoted `"opus" = "0.4"` dependency and a source canary containing `extern crate opus;`. Require each sub-invocation to fail and include `[dependencies]`/`opus` or `src import`/`opus` respectively.

  Run:

  ```text
  bash tools/check-codec-dependency-boundary.sh --self-test
  ```

  Expected: RED because the old shell parser does not correctly classify at least one parser-valid canary.

- [ ] **Step 2: Delete shell Rust/TOML parsing and call the integration test**

  Preserve root path validation and safe canary creation. In `check_root`, invoke the oracle from `SCRIPT_ROOT`:

  ```bash
  CODEC_BOUNDARY_ROOT="$ROOT" cargo +1.85.0 test --locked \
      --manifest-path "${SCRIPT_ROOT}/Cargo.toml" \
      --test codec_dependency_boundary \
      -- --exact codec_dependencies_and_src_imports_remain_outside_the_root_crate
  ```

  Remove Perl/AWK/grep source and Cargo syntax recognition helpers. Keep only shell argument handling, validated temporary cleanup, output-specific self-test assertions, and `check_normal_graph` using the supplied root's manifest.

- [ ] **Step 3: Green verification and commit**

  Run:

  ```text
  bash -n tools/check-codec-dependency-boundary.sh
  bash tools/check-codec-dependency-boundary.sh --self-test
  bash tools/check-codec-dependency-boundary.sh
  cargo +1.85.0 test --locked --test codec_dependency_boundary -- --nocapture
  git diff --check
  ```

  Commit:

  ```text
  git add tools/check-codec-dependency-boundary.sh
  git commit -m "test(03-06): delegate shell boundary guard"
  ```

## Plan self-review

- The spec's one-semantic-oracle design is implemented by Task 1; Task 2 deletes rather than duplicates syntax parsing.
- CODEC-06 remains checked in Task 1 and final verification; CODEC-07 receives parser-valid source, manifest, workspace, and lock canaries.
- Task 2 consumes Task 1's exact environment-variable interface and test name; no task relies on an undefined function or unpinned dependency.
- Placeholder scan: no TODO/TBD or unspecified test behaviors remain.
