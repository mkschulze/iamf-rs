# Codec Boundary Lexer Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the CODEC-07 Rust and shell source guards reject every valid forbidden import form while ignoring comments and string content.

**Architecture:** Both guards implement the same deliberately narrow contract: sanitize Rust comments and ordinary/raw strings to whitespace while preserving structural separators; then scan complete `use`, visibility-qualified `use`, and `extern crate` statements rather than line prefixes. Grouped imports are split into every branch and compare crate identifiers after `-` → `_` normalization. The workspace `exclude` collector consumes exactly one TOML array and compares only quoted array entries.

**Tech Stack:** Rust standard library test APIs; Bash plus the existing Linux-only Perl interpreter used by the shell guard; Cargo test and cargo tree.

**Spec:** `.planning/phases/03-flac-and-opus-framing/03-06-PLAN.md`, `.planning/REQUIREMENTS.md` CODEC-06/07, `.planning/phases/03-flac-and-opus-framing/03-CONTEXT.md`.

## Global Constraints

- Root codec dependencies are forbidden in both dependencies and dev-dependencies; `tools/codec-fixtures` is the only codec graph.
- Root workspace `exclude` must contain `tools/codec-fixtures`; root `Cargo.lock` must contain no codec packages.
- Source scans target real `use` and `extern crate` syntax, not comments, constants such as `CODEC_ID_OPUS`, ordinary strings, or raw strings.
- The Rust test is std-only and never invokes the shell script, so it remains Windows-MSVC portable.
- The root normal graph remains exactly `iamf` and `thiserror`; no root dependency, lockfile, lint, or deny-policy change is permitted.
- Shell self-test temporary cleanup may remove only its validated dedicated temporary directory.

---

### Task 1: Define and prove the shared lexical import boundary

**Files:**
- Modify: `tests/codec_dependency_boundary.rs`
- Modify: `tools/check-codec-dependency-boundary.sh`

**Interfaces:**
- Consumes: `FORBIDDEN = [claxon, flacenc, opus, opusic-sys, audiopus, rubato]` and the existing `check_src_imports`, `file_has_codec_import`, `assert_src_has_no_codec_imports`, and `assert_root_workspace_excludes_codec_fixtures` entry points.
- Produces: both implementations accept source with codec-looking comments/ordinary strings/raw strings and reject every named valid import form with an error naming `src import` and the original package spelling.

- [ ] **Step 1: Add RED canaries before changing scanner logic**

  Add Rust canary entries and shell `expect_import_canary` calls for each failing import below. Add positive canaries for the two harmless raw-string forms and the valid multiline `exclude` value; add negative canary for comment-only exclusion and an `exclude` array followed by `members = ["tools/codec-fixtures"]`.

  ```rust
  // Each must fail in both guards:
  "use {opus};\n"
  "extern crate opus;\n"
  "fn harmless() {}\nuse opus::Encoder;\n"
  // Each must pass in both guards:
  "const HELP: &str = r#\"; use opus::Encoder;\"#;\n"
  "const HELP: &str = r##\"use opus::Encoder;\"##;\n"
  ```

  Run:

  ```text
  cargo +1.85.0 test --locked --test codec_dependency_boundary -- --nocapture
  bash tools/check-codec-dependency-boundary.sh --self-test
  ```

  Expected: both commands fail specifically because the new real-import canaries are still accepted or raw-string cases are falsely rejected.

- [ ] **Step 2: Implement one lexical contract in the Rust portable validator**

  Replace statement-prefix matching with a stateful sanitizer and statement extractor. The sanitizer must preserve newlines and separators while replacing content inside line comments, nested block comments, escaped ordinary strings, byte strings, character literals, and raw strings (`r#*"..."#*`) with whitespace. The extractor must locate `use`, `pub use`, `pub(...) use`, and `extern crate` at token boundaries anywhere in the remaining code, stop at their semicolon, and inspect every comma/brace-separated import branch. Compare each leading branch identifier after `-` → `_` normalization.

  The `exclude` validator must strip only TOML comments outside strings, begin at the `exclude = [` value inside `[workspace]`, stop at that value's closing `]`, and inspect quoted entries only. It must not let later tables or arrays satisfy the check.

- [ ] **Step 3: Implement the identical contract in the Linux shell guard**

  Keep the public shell interface and its `--root`/`--self-test` behavior. Replace the Perl sanitizer so it emits whitespace/newlines—not raw string content—for all strings and comments, then use a global token-boundary matcher over sanitized source (not `split /;/` fragments that can contain a preceding function block). Recognize `extern crate` and the visibility-qualified `use` variants; inspect every group branch, including a final `}` immediately following the package name.

  In the AWK workspace check, set collection state only for the `exclude` value, clear it immediately after its closing bracket, and match `tools/codec-fixtures` only as a quoted value inside that collected array.

- [ ] **Step 4: Green verification and regression matrix**

  Run:

  ```text
  cargo +1.85.0 test --locked --test codec_dependency_boundary -- --nocapture
  bash -n tools/check-codec-dependency-boundary.sh
  bash tools/check-codec-dependency-boundary.sh --self-test
  bash tools/check-codec-dependency-boundary.sh
  normal_deps=$(cargo +1.85.0 tree --locked -e normal,no-proc-macro --prefix none | awk 'NF {print $1}' | sort -u)
  test "$normal_deps" = "$(printf 'iamf\nthiserror')"
  git diff --check
  ```

  Expected: all canaries have their intended polarity, the committed tree passes, the normal graph is exact, and whitespace validation passes.

- [ ] **Step 5: Commit**

  ```text
  git add tests/codec_dependency_boundary.rs tools/check-codec-dependency-boundary.sh
  git commit -m "fix(03-06): harden codec boundary lexer"
  ```

## Plan self-review

- CODEC-07 source, manifest, lock, workspace exclusion, and exact normal-graph constraints are all covered by Task 1's regression matrix.
- CODEC-06 remains covered by the existing scoped `cargo-deny` policy and is explicitly protected from change.
- The task names exact source forms for every final-review bypass, has RED and GREEN commands, and defines no new dependency or unspecified parser.
- Placeholder scan: no TODO/TBD or unqualified validation steps remain.
