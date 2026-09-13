---
phase: quick-260913-vcc
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - tests/conformance.rs
  - .github/workflows/reference.yml
  - CONFORMANCE-GATE.md
autonomous: true
requirements: [CONF-05, CONF-09, CONF-10]

estimate:
  tokens: 60000
  raw_tokens: 60000
  tasks: 2
  confidence: low

must_haves:
  truths:
    - "With IAMF_REF_DECODER set and .reference-manifest.json carrying \"dep_codecs_disabled\": true (the macOS dev host), the_flac_fixture_is_conformant and the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode PASS. Each prints exactly one line starting `SKIP CONF-05 (libiamf FLAC/Opus) for <fixture name>: reference decoder built without FLAC/Opus codecs (dep_codecs_disabled=true in .reference-manifest.json; only x86_64 Linux reference hosts ship them — tools/build-reference.sh)`. Clause 0, CONF-02, CONF-03, CONF-04 and CONF-06 still run and print their lines for phase3_flac and phase3_opus"
    - "The skip is decided only from the manifest, never from iamfdec output. With \"dep_codecs_disabled\": false, CONF-05 for FLAC and Opus runs exactly as before, so on this host the_flac_fixture_is_conformant FAILS with iamfdec's `fail to configure decoder`. With the key missing or its value neither true nor false, the gate returns an Err and never skips. With true while host_triple is x86_64-unknown-linux-gnu, it also returns an Err"
    - "LPCM fixtures (phase1_sample_identity, phase1_endianness, the probes, expanded layouts) never read the codec flag. Their CONF-05 path is byte-for-byte the pre-change code, and the codec match in assert_conformant is exhaustive, with no wildcard arm"
    - "The reference workflow (x86_64 Linux) fails before any cargo test step if the manifest's dep_codecs_disabled is not literally false. So the skip can never hide in CI"
    - "CONFORMANCE-GATE.md has one short subsection that records the host-dependent skip as a skip, not a D-17 waiver. src/, fixtures, golden, DIFF-LEDGER.md, tests/support/, tests/reference_manifest.rs, tools/ and all reference expectation hashes are unchanged against 9c905ce. The six foreign-modified files are neither edited nor committed"
  artifacts:
    - path: "tests/conformance.rs"
      provides: "manifest-value reader, libiamf_flac_opus_skip_reason(fixture_name, manifest) -> Result<Option<String>, String>, the exhaustive codec match in assert_conformant's CONF-05 branch, and offline unit test conf05_flac_opus_skip_is_decided_by_the_manifest"
      contains: "libiamf_flac_opus_skip_reason"
    - path: ".github/workflows/reference.yml"
      provides: "a step between the reference build and the first cargo test that fails unless dep_codecs_disabled is false"
      contains: "dep_codecs_disabled"
    - path: "CONFORMANCE-GATE.md"
      provides: "### Host-dependent skip: CONF-05 for FLAC and Opus (not a waiver)"
      contains: "dep_codecs_disabled"
  key_links:
    - from: "tests/conformance.rs assert_conformant, CONF-05 Some(decoder) branch"
      to: ".reference-manifest.json dep_codecs_disabled / host_triple"
      via: "read_to_string(CARGO_MANIFEST_DIR/.reference-manifest.json) only for FixtureCodec::Flac | FixtureCodec::Opus, then libiamf_flac_opus_skip_reason(...)?"
      pattern: "libiamf_flac_opus_skip_reason\\("
    - from: ".github/workflows/reference.yml guard step"
      to: "tools/build-reference.sh manifest stamp (DEP_CODECS_DISABLED)"
      via: "python3 json.load of .reference-manifest.json, exit non-zero unless the value is False"
      pattern: "dep_codecs_disabled"
---

<objective>
Make the libiamf CONF-05 clause for the FLAC and Opus fixtures skip cleanly, with a printed reason, when
the local reference decoder was built without those codecs. Keep every other clause running, and make
the skip impossible in CI.

Purpose: on macOS arm64/x86_64, `tools/build-reference.sh` moves aside the x86_64-Linux-only FLAC/Opus
archives and stamps `"dep_codecs_disabled": true`. `iamfdec` then fails with "errno: -6, fail to
configure decoder." and writes a 44-byte WAV, and the two Phase 3 conformance tests fail on every dev
host (qk3 deferred item 5). A red suite that is always red for a known host reason hides real failures.
Output: the manifest-driven skip in `tests/conformance.rs`, a CI guard step in
`.github/workflows/reference.yml`, and a short gate-ledger note.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@tests/CLAUDE.md
@.planning/quick/260913-qk3-enforce-iamf-v1-1-0-profile-restrictions/260913-qk3-deferred-items.md

<interfaces>
tests/conformance.rs (2825 lines, `#![allow(dead_code)]` at :72; `mod fixture` via `#[path]` at :74-75):
- `reference_decoder() -> Option<PathBuf>` :105 (empty value = unset).
- `skip_no_reference(test)` :140, `skip_no_container(test)` :149: the existing SKIP println style.
- `struct GateReport { lines: Vec<String> }` :959, `note(clause, verdict)` pushes `"{clause}: {verdict}"`,
  `print(tag)` prints `"[{tag}] {line}"`.
- `comparison_oracle_label(spec)` :977 shows the exhaustive `FixtureCodec` match style
  (`FixtureCodec::Lpcm { .. } | FixtureCodec::Flac => ...`).
- `assert_conformant(fixture) -> Result<GateReport, String>` :1029. The doc comment at :995-1028
  describes the clause order and skips. The CONF-05 block is :1071-1103:
  `match reference_decoder() { None => { skip_no_reference("CONF-05"); report.note(...SKIPPED — no iamfdec) }
  Some(decoder) => { scratch_dir; round_trip (panics on a 44-byte WAV); ... } }`. The CONF-06 block
  follows at :1105-1113 and must stay reachable after a CONF-05 skip.
- `assert_reference_manifest_matches()` :1125 reads `.reference-manifest.json` from
  `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` and maps errors to `String`. The CONF-05 read uses the same
  path construction.
- `sha_on_line_naming` :1160 is a line-scoped scanner (no JSON dependency, same approach as
  tests/reference_manifest.rs:32).
- Tests: `the_flac_fixture_is_conformant` :1644, `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode`
  :1656 (also asserts on the CONF-03 report line; untouched).
- `spec.codec: FixtureCodec` (tests/support/fixture.rs:203): `Lpcm { big_endian: bool }`, `Flac`, `Opus`.
  Fixture names: `phase1_sample_identity`, `phase1_endianness`, `phase3_flac`, `phase3_opus`,
  `phase1_structure_only`.

Manifest (.reference-manifest.json, gitignored, written by tools/build-reference.sh:302-314) holds one key per line:
`  "host_triple": "x86_64-apple-darwin",` and `  "dep_codecs_disabled": true,` (bare JSON boolean,
trailing comma). `DEP_CODECS_DISABLED=false` only in the `uname -s = Linux && uname -m = x86_64` branch
(:104-122), and that branch FATALs if FLAC/Opus are not configured (:151-156). HOST_TRIPLE comes from
`rustc -vV` (`x86_64-unknown-linux-gnu` on ubuntu-latest).

.github/workflows/reference.yml: the steps are "Build the pinned reference decoder" (:62), "Export IAMF_REF_DECODER"
(:65, inline `python3 -c` with `json.load(open('.reference-manifest.json'))`, the style to copy),
"Show the reference manifest (D-13)" (:72), "Assert the manifest matches REFERENCES.md" (:79, the first
cargo test), and so on.

CONFORMANCE-GATE.md: no current text mentions FLAC, Opus or dep_codecs_disabled. `### Waivers in force at exit`
(:583) ends at :597, and `## Cross-target byte-identity evidence` starts at :599.

Lints: `indexing_slicing` and `arithmetic_side_effects` are denied in tests too. `unwrap`/`expect`/`panic`
are allowed only inside `#[test]` fns. Rust 1.85, so no let-chains.

Environment: iamfdec at `/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec`.
The manifest says `x86_64-apple-darwin`, `dep_codecs_disabled: true`. docker is at `/usr/local/bin/docker`,
and the `iamf-tools:v2.1.0` image is present. There is no `timeout` binary; perl is at `/usr/bin/perl`. ruby with
yaml is available; python3 has no yaml. Base commit is 9c905ce. Foreign uncommitted changes (never edit or
stage): README.md, REFERENCES.md, .planning/PROJECT.md, .planning/codebase/CONVENTIONS.md, .gitignore,
.claude/CLAUDE.md, plus the untracked files in `git status`.

Bounded-run wrapper W (arguments: SECONDS CMD...):
`perl -e 'my $s=shift; my $p=fork; if(!$p){setpgrp(0,0); exec @ARGV; exit 127} $SIG{ALRM}=sub{kill "KILL", -$p; print STDERR "alarm fired after $s s\n"; exit 124}; alarm $s; waitpid($p,0); exit($? >> 8)'`
</interfaces>
</context>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: End-to-end "FLAC/Opus CONF-05 skips from the manifest": parser, gate wiring, offline unit test, real reference run</name>
  <files>tests/conformance.rs</files>
  <behavior>
    New offline `#[test] fn conf05_flac_opus_skip_is_decided_by_the_manifest()` in tests/conformance.rs.
    It runs with IAMF_REF_DECODER unset on all four targets and uses inline manifest strings in the
    real file's shape (one `"key": value,` per line):
    - darwin manifest (`"host_triple": "x86_64-apple-darwin"`, `"dep_codecs_disabled": true`): returns
      `Ok(Some(msg))`, where msg starts with `SKIP CONF-05 (libiamf FLAC/Opus) for phase3_flac: ` and contains
      `dep_codecs_disabled=true in .reference-manifest.json`.
    - same host with `false`: `Ok(None)`.
    - x86_64-unknown-linux-gnu with `false`: `Ok(None)`.
    - key absent: `Err(e)`, where e contains `dep_codecs_disabled` and `tools/build-reference.sh`.
    - malformed value (`"dep_codecs_disabled": "yes",`): `Err(e)`, where e contains `dep_codecs_disabled` and `"yes"`.
    - `true` on `"host_triple": "x86_64-unknown-linux-gnu"`: `Err(e)`, where e contains `x86_64-unknown-linux-gnu`.
    - `true` with host_triple absent: `Err(e)`, where e contains `host_triple`.
    - a key-prefix decoy (a line `"dep_codecs_disabled_note": false,` placed before the real `true` line)
      must not be matched: the result is still `Ok(Some(_))`.
    RED is this test failing to compile because the helper does not exist yet.
  </behavior>
  <action>
    All edits go in tests/conformance.rs, placed next to `assert_reference_manifest_matches` / `sha_on_line_naming`.

    1. Add `fn manifest_raw_value<'a>(manifest: &'a str, key: &str) -> Option<&'a str>`. For the first
       line whose trimmed text starts with the quoted key (`"` + key + `"`), take the remainder. Its
       left-trimmed form must start with `:`. Return the rest trimmed, with one trailing `,` removed
       and trimmed again. Otherwise keep scanning, so the `"dep_codecs_disabled_note"` decoy
       fails the `:` check and is skipped. Use `strip_prefix`/`trim`/`strip_suffix` only. No indexing
       and no arithmetic (lint rules), and no new dependency (same line-scan approach as
       tests/reference_manifest.rs, per the locked decision to reuse its reading approach).

    2. Add `fn libiamf_flac_opus_skip_reason(fixture_name: &str, manifest: &str) -> Result<Option<String>, String>`,
       following the orchestrator's locked decisions:
       - `dep_codecs_disabled` absent: Err saying `.reference-manifest.json` has no `dep_codecs_disabled`
         field while IAMF_REF_DECODER is set, so it was written by an older tools/build-reference.sh.
         Tell the user to re-run it. Never skip silently.
       - raw value `false`: Ok(None). CONF-05 runs as today, and any failure is real.
       - raw value `true`: read `host_triple` (strip the surrounding `"` with strip_prefix/strip_suffix).
         If it is absent, return Err naming `host_triple`. If it equals `x86_64-unknown-linux-gnu`, return Err:
         tools/build-reference.sh ships FLAC/Opus on that host, so `dep_codecs_disabled=true` there means a broken
         or stale build, and skipping would hide CONF-05. Otherwise return Ok(Some(reason)), where reason
         is exactly `SKIP CONF-05 (libiamf FLAC/Opus) for {fixture_name}: reference decoder built without FLAC/Opus codecs (dep_codecs_disabled=true in .reference-manifest.json; only x86_64 Linux reference hosts ship them — tools/build-reference.sh)`.
       - any other raw value: Err quoting the raw value.
       Add a doc comment giving the why: the local decision comes from the build manifest, not
       iamfdec stderr, because iamfdec exits 0 on failure (CONFORMANCE-GATE.md Experiment A), and CI
       forbids the skip through the reference workflow guard.

    3. Wire it into `assert_conformant`'s CONF-05 `Some(decoder)` arm, before `scratch_dir`/`round_trip`.
       Compute `let skip = match spec.codec { FixtureCodec::Lpcm { .. } => None, FixtureCodec::Flac | FixtureCodec::Opus => { read manifest; libiamf_flac_opus_skip_reason(fixture.name, &manifest)? } };`.
       The match is exhaustive with no wildcard, so LPCM never reads the flag and a future codec forces
       a decision. Read the manifest with
       `std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".reference-manifest.json"))`,
       mapping the error to `CONF-05: cannot read .reference-manifest.json: {e}. IAMF_REF_DECODER is set, so tools/build-reference.sh must have stamped it; re-run it.`
       If `Some(reason)`: `println!("{reason}")`, then
       `report.note("CONF-05 (libiamf sample identity)", "SKIPPED — reference decoder built without FLAC/Opus (dep_codecs_disabled=true)")`,
       and fall through to CONF-06. Do NOT return early. If `None`: run the existing body
       unchanged. Put the existing body inside the `None` arm (or an `if let`/`else`) without changing
       any statement in it.
       Extend the `assert_conformant` doc comment (the "Each reference-dependent clause skips with a
       printed reason" paragraph) by one sentence about this manifest-driven, FLAC/Opus-only CONF-05 skip.

    4. Write the behavior test first (RED: it does not compile), then implement (GREEN). Do not touch any
       other test, fixture, `src/`, tests/support/ or tests/reference_manifest.rs.

    5. Tracer end-to-end run. First `cargo test --locked --test conformance --no-run`. Then, under W with 900 s:
       `env IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec cargo test --locked --test conformance -- --nocapture --test-threads=1 the_flac_fixture_is_conformant the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode conf05_flac_opus_skip`.
       Send stdout and stderr to `target/vcc-tracer.log`, then append a line `conf_exit=<code>`. If the docker
       probe `docker image inspect iamf-tools:v2.1.0` (under W, 60 s) does not exit 0, stop and report: CONF-06
       lines are a required observable here, so no shim.

    6. Commit only by explicit path:
       `git commit -m "fix(quick-260913-vcc): skip libiamf CONF-05 for FLAC/Opus when the reference lacks those codecs" -- tests/conformance.rs`.
       Put the reason in the body: qk3 deferred item 5, the manifest-driven decision, and that LPCM is unaffected.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -D warnings && env -u IAMF_REF_DECODER cargo test --locked --test conformance conf05_flac_opus_skip_is_decided_by_the_manifest && grep -q '^conf_exit=0$' target/vcc-tracer.log && grep -q '^test result: ok. 3 passed' target/vcc-tracer.log && grep -F 'SKIP CONF-05 (libiamf FLAC/Opus) for ' target/vcc-tracer.log > target/vcc-tracer-skips.txt && [ -z "$(sort target/vcc-tracer-skips.txt | uniq -d)" ] && [ -z "$(grep -vE 'for phase3_(flac|opus): ' target/vcc-tracer-skips.txt)" ] && grep -qF 'SKIP CONF-05 (libiamf FLAC/Opus) for phase3_flac: reference decoder built without FLAC/Opus codecs (dep_codecs_disabled=true in .reference-manifest.json' target/vcc-tracer.log && grep -qF 'SKIP CONF-05 (libiamf FLAC/Opus) for phase3_opus: ' target/vcc-tracer.log && grep -qF '[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported' target/vcc-tracer.log && grep -qF '[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported' target/vcc-tracer.log && grep -qF '[phase3_flac] CONF-04 (structure):' target/vcc-tracer.log && grep -qF '[phase3_opus] clause 0 (D-13 manifest): reference pin confirmed' target/vcc-tracer.log && git diff --quiet 9c905ce -- src tests/support tests/fixtures tests/reference_manifest.rs tests/golden.rs DIFF-LEDGER.md tools</automated>
  </verify>
  <done>The offline unit test passes. With the real darwin manifest and IAMF_REF_DECODER set, both FLAC/Opus tests pass, each prints exactly one `SKIP CONF-05 (libiamf FLAC/Opus) for <name>` line, and CONF-06, CONF-04 and clause 0 lines still print. fmt and clippy are clean. Only tests/conformance.rs changed and it is committed.</done>
</task>

<task type="auto">
  <name>Task 2: CI guard step, gate-ledger note, full gate, manifest-flip negative proof</name>
  <files>.github/workflows/reference.yml, CONFORMANCE-GATE.md</files>
  <action>
    1. In .github/workflows/reference.yml, insert one step directly after "Show the reference manifest (D-13)"
       and before "Assert the manifest matches REFERENCES.md". Name it
       `Assert the reference decoder has FLAC and Opus (CONF-05 may not skip here)`. Give it a 3-5 line
       comment in the file's voice: tests/conformance.rs skips CONF-05 for FLAC/Opus when
       `dep_codecs_disabled` is true (the non-x86_64-Linux dev-host build), and this job is the one place
       that clause must execute, so a true value here is a failure, not a skip. Its `run` uses the same inline
       `python3 -c` + `json.load(open('.reference-manifest.json'))` style as "Export IAMF_REF_DECODER".
       Read `m.get('dep_codecs_disabled')`. Unless the value `is False`, call `sys.exit(...)` with a message
       that names the observed value (so an absent key reports None) and points at tools/build-reference.sh
       and CONF-05. Otherwise print `dep_codecs_disabled=false: FLAC and Opus CONF-05 will execute`.
       This implements the orchestrator's CI-guard decision.

    2. In CONFORMANCE-GATE.md, insert a subsection immediately before
       `## Cross-target byte-identity evidence (GUARD-09 / ROADMAP criterion 4)`:
       `### Host-dependent skip: CONF-05 for FLAC and Opus (not a waiver)`, one paragraph of at most 6 lines.
       It says that pinned libiamf bundles FLAC/Opus only as x86_64-Linux archives, so
       tools/build-reference.sh disables them elsewhere and stamps `dep_codecs_disabled`. When that is
       true, `assert_conformant` skips only CONF-05 for the FLAC and Opus fixtures with a printed reason,
       and every other clause still runs. A missing or malformed flag, or true on
       `x86_64-unknown-linux-gnu`, fails. The reference workflow asserts the flag is false before any test,
       so CONF-05 for FLAC and Opus executes on every PR. That is why this is a skip and not a D-17 waiver.
       Edit nothing else in the file. The Phase 1 exit table is historical. No other tracked doc states
       that FLAC/Opus CONF-05 runs on every host (checked: tests/fixtures/codecs/README.md:56 already says
       "reports a skip when unavailable"), so no other doc changes. Never touch the foreign-modified
       README.md / REFERENCES.md / .planning/PROJECT.md / .planning/codebase/CONVENTIONS.md / .gitignore /
       .claude/CLAUDE.md.

    3. Workflow guard proof (local, no GitHub). Use ruby yaml to load the workflow. Find the step whose
       `run` contains `dep_codecs_disabled`, assert its index is greater than the "Build the pinned reference
       decoder" step and less than the "Assert the manifest matches REFERENCES.md" step, and write its `run`
       text to `target/vcc-ci-guard.sh`. Create `target/vcc-ci-false/` and `target/vcc-ci-true/`, each holding a
       `.reference-manifest.json` derived from the real one with sed (value false and true respectively).
       Also create `target/vcc-ci-absent/` with the `dep_codecs_disabled` line deleted. Run
       `bash ../vcc-ci-guard.sh` inside each. Append `false_exit=<code>`, `true_exit=<code>` and
       `absent_exit=<code>` lines to `target/vcc-ci-guard.log`. The real manifest is only read here.

    4. Full gate, in order (fix findings at the source, never with `#[allow]`):
       `cargo fmt --all -- --check`; `cargo clippy --locked --all-targets -- -D warnings`;
       `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`;
       `env -u IAMF_REF_DECODER cargo test --locked` (write to `target/vcc-offline.log`).

    5. Full conformance with the reference: under W with 1500 s,
       `env IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec cargo test --locked --test conformance -- --nocapture --test-threads=1`.
       Send stdout and stderr to `target/vcc-conformance.log`, then append `conf_exit=<code>`.

    6. Manifest-flip negative proof (shows the skip is manifest-driven and that `false` runs the real clause).
       Run in ONE shell invocation so restoration always happens:
       `cp .reference-manifest.json target/vcc-manifest-backup.json`, then
       `shasum -a 256 .reference-manifest.json > target/vcc-manifest.sha256`, then write
       `sed 's/"dep_codecs_disabled": true/"dep_codecs_disabled": false/' target/vcc-manifest-backup.json` into
       `.reference-manifest.json`. Run W 900 `env IAMF_REF_DECODER=<iamfdec path> cargo test --locked --test conformance -- --nocapture --test-threads=1 --exact the_flac_fixture_is_conformant`
       with output to `target/vcc-flip.log`, capture its exit code into a variable, then ALWAYS
       `cp target/vcc-manifest-backup.json .reference-manifest.json`, append `flip_exit=<code>` to the log,
       and run `shasum -a 256 -c target/vcc-manifest.sha256`. If restoration fails, stop and report it before
       anything else.

    7. Commit by explicit path only:
       `git commit -m "ci(quick-260913-vcc): fail the reference job if its libiamf build lacks FLAC/Opus" -- .github/workflows/reference.yml`
       then `git commit -m "docs(quick-260913-vcc): record the host-dependent FLAC/Opus CONF-05 skip" -- CONFORMANCE-GATE.md`.
       If a fmt/clippy fix to tests/conformance.rs is needed, make a separate `style(quick-260913-vcc): ...`
       commit by explicit path. Never amend.

    In the SUMMARY, quote: the two SKIP lines, and the phase3_flac/phase3_opus CONF-06 lines from
    vcc-conformance.log; every LPCM `CONF-05 (libiamf sample identity): ... 0 of ... samples differ` line; the
    `test result:` lines from the offline, conformance and flip logs; the three guard exit codes; the
    manifest shasum check result. State that the flip run failing with `fail to configure decoder` is the
    expected negative proof, not a regression.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && grep -q '^test result: ok' target/vcc-offline.log && [ -z "$(grep '^test result: ' target/vcc-offline.log | grep -v '^test result: ok')" ] && grep -q '^false_exit=0$' target/vcc-ci-guard.log && grep -qE '^true_exit=[1-9][0-9]*$' target/vcc-ci-guard.log && grep -qE '^absent_exit=[1-9][0-9]*$' target/vcc-ci-guard.log && ruby -ryaml -e 's=YAML.load_file(".github/workflows/reference.yml")["jobs"]["reference"]["steps"]; n=->(t){s.index{|x| x["name"]==t}}; g=s.index{|x| x["run"].to_s.include?("dep_codecs_disabled")}; b=n.("Build the pinned reference decoder"); a=n.("Assert the manifest matches REFERENCES.md"); exit((g && b && a && b < g && g < a) ? 0 : 1)' && grep -q '^### Host-dependent skip: CONF-05 for FLAC and Opus (not a waiver)$' CONFORMANCE-GATE.md && grep -q 'dep_codecs_disabled' CONFORMANCE-GATE.md && grep -q '^conf_exit=0$' target/vcc-conformance.log && grep -q '^test result: ok' target/vcc-conformance.log && grep -F 'SKIP CONF-05 (libiamf FLAC/Opus) for ' target/vcc-conformance.log > target/vcc-conformance-skips.txt && grep -qF 'for phase3_flac: ' target/vcc-conformance-skips.txt && grep -qF 'for phase3_opus: ' target/vcc-conformance-skips.txt && [ -z "$(sort target/vcc-conformance-skips.txt | uniq -d)" ] && [ -z "$(grep -vE 'for phase3_(flac|opus): ' target/vcc-conformance-skips.txt)" ] && grep -qF '[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported' target/vcc-conformance.log && grep -qF '[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported' target/vcc-conformance.log && grep -qF 'CONF-05 (libiamf sample identity): 300 sample frames, 0 of' target/vcc-conformance.log && grep -qE '^flip_exit=[1-9][0-9]*$' target/vcc-flip.log && ! grep -q '^flip_exit=124$' target/vcc-flip.log && grep -q 'fail to configure decoder' target/vcc-flip.log && grep -q '^test result: FAILED' target/vcc-flip.log && shasum -a 256 -c target/vcc-manifest.sha256 && grep -q '"dep_codecs_disabled": true' .reference-manifest.json && git diff --quiet 9c905ce -- src tests/support tests/fixtures tests/reference_manifest.rs tests/golden.rs DIFF-LEDGER.md tools && [ -z "$(git diff --name-only 9c905ce HEAD -- README.md REFERENCES.md .planning/PROJECT.md .planning/codebase/CONVENTIONS.md .gitignore .claude/CLAUDE.md)" ] && git diff --name-only 9c905ce HEAD > target/vcc-committed.txt && [ -z "$(grep -vE '^(tests/conformance\.rs|\.github/workflows/reference\.yml|CONFORMANCE-GATE\.md|\.planning/quick/260913-vcc-skip-libiamf-flac-and-opus-conformance-c/.*)$' target/vcc-committed.txt)" ]</automated>
  </verify>
  <done>The reference workflow has a guard step between the build and the first cargo test. Locally it exits 0 for false and non-zero for true or an absent key. CONFORMANCE-GATE.md has the one subsection. fmt, both clippy runs and the offline `cargo test --locked` are clean. The full conformance run with IAMF_REF_DECODER ends `test result: ok` with exactly two FLAC/Opus SKIP lines, CONF-06 lines for phase3_flac and phase3_opus, and LPCM CONF-05 sample-identity lines. The flip run proves `false` executes the real clause, and the real manifest is restored byte-identically. Boundary paths are unchanged, only allowed paths are committed, and foreign changes are untouched.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| build script → manifest → test harness | `.reference-manifest.json` is a local, gitignored file whose `dep_codecs_disabled` value now decides whether a conformance clause executes |
| CI runner → reference job | the only environment where the FLAC/Opus CONF-05 clause is required to execute |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-vcc-01 | Tampering | `.reference-manifest.json` flag hides CONF-05 in CI | medium | mitigate | reference.yml guard step fails unless the value is literally false, before any cargo test (Task 2 step 1). `libiamf_flac_opus_skip_reason` also errors on true + x86_64-unknown-linux-gnu (Task 1) |
| T-vcc-02 | Repudiation | a skip reported as a pass | low | mitigate | exact `SKIP CONF-05 (libiamf FLAC/Opus) for <name>` println plus a `SKIPPED — ...` GateReport line; CONFORMANCE-GATE.md records it as a skip, not a waiver |
| T-vcc-03 | Denial of Service | stale or older manifest without the key silently skips | low | mitigate | absent or malformed key returns Err (loud failure), covered by the offline unit test and the `absent_exit` guard check |
| T-vcc-04 | Tampering | flip proof leaves the real manifest modified | low | mitigate | backup + shasum before, unconditional restore in the same shell, `shasum -c` in verify |
</threat_model>

<verification>
- Offline: `env -u IAMF_REF_DECODER cargo test --locked` is green on this host. The new unit test runs on all four CI targets with no reference.
- Reference: the conformance suite with IAMF_REF_DECODER set ends `test result: ok` and shows 2 SKIP lines and the CONF-06 lines.
- Negative: the manifest flipped to false makes the FLAC CONF-05 clause execute and fail with iamfdec's configure error, and the manifest is restored.
- CI guard: the extracted workflow step exits 0 for false and non-zero for true or absent, ordered before the first cargo test.
- Boundary: no change to src/, tests/support/, fixtures, golden, DIFF-LEDGER.md, tests/reference_manifest.rs, tools/.
</verification>

<success_criteria>
- `the_flac_fixture_is_conformant` and `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode` pass on the macOS dev host with IAMF_REF_DECODER set, printing the manifest-derived SKIP reason and still running clause 0, CONF-02..04 and CONF-06.
- The decision comes from `dep_codecs_disabled` alone. True skips (off x86_64 Linux), false runs, and a missing or malformed value fails.
- The reference workflow cannot pass with `dep_codecs_disabled` true.
- 3 atomic commits (fix, ci, docs) by explicit path, and no foreign file staged.
</success_criteria>

<output>
Create `.planning/quick/260913-vcc-skip-libiamf-flac-and-opus-conformance-c/260913-vcc-SUMMARY.md` when done
</output>
