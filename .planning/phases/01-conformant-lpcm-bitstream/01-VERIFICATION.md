---
phase: 01-conformant-lpcm-bitstream
verified: 2026-09-08T20:37:14Z
status: gaps_found
score: 2/5 must-haves verified
---

# Phase 1: Conformant LPCM Bitstream Verification Report

**Phase Goal:** A standalone `.iamf` file this crate writes is accepted by both reference implementations, is byte-explicable against reference output, and is reproducible byte-for-byte on every target.
**Verified:** 2026-09-08T20:37:14Z
**Status:** gaps_found

## Goal Achievement

### Observable Truths

ROADMAP success criteria are the authoritative truths. Plan-level must-haves were used to identify the artifacts and connections supporting them, but cannot relax the ROADMAP contract.

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | Reproduce `libiamf/tests/test_000003.iamf` byte-for-byte and walk our output exactly to `bytes.len()` | ✓ VERIFIED | Fresh `cargo test --locked --all-targets` passed `tests/sequence.rs::reproduces_test_000003` and `the_output_walks_67_obus_ending_exactly_on_the_file_length` (32567 bytes, 67 OBUs). `tests/sequence.rs:105-141`; `tests/conformance.rs:1487-1524`. |
| 2 | The pinned `libiamf` decodes the same non-silent, per-channel-distinguishable, trimmed fixture sample-identically, and that fixture contains at least six OBUs including two Codec Configs | ✗ FAILED AS WRITTEN | The substantive conformance behavior passes: the fresh reference-gated run reported 300/300 frames and 0/1800 differing samples for the 5.1 fixture, with `trim_at_end = 84`, 16 OBUs, and six distinguishable channels. But the fixture has exactly **one** Codec Config (`tests/support/fixture.rs:500-546`). The two-Codec-Config design was replaced, by recorded user decision, with a separate two-Audio-Element structure fixture after `iamf-tools decoder_main` aborted on an unreferenced Codec Config (`CONFORMANCE-GATE.md:255-375`). ROADMAP criterion 2 was never amended to that decision. |
| 3 | `iamf-tools` at the pinned v1.x tag accepts that same file and its output is byte-identical or exhaustively diff-ledgered | ✗ FAILED AS WRITTEN | The functional oracle passes strongly: the fresh container-gated run reported `Decoded 3 temporal units.` and CONF-07 produced 7073 bytes on each side with **0 differing offsets** and an empty, executable ledger (`tests/conformance.rs:1411-1471,2099-2136`; `DIFF-LEDGER.md:1-63`). However, no v1.x IAMF-1.1-exact tag exists; the repository deliberately pins `iamf-tools` **v2.1.0** / `848c6ff...` (`REFERENCES.md:8-18,83-116`). ROADMAP criterion 3 retained the superseded “v1.x tag” wording. |
| 4 | The fixture matches its committed hash on macOS arm64, macOS x86_64, Windows MSVC, and Linux x64; same-process double encode is identical | ? UNCERTAIN | The hash is committed and locally reproduces as `3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c`; the golden and double-encode tests pass (`tests/golden.rs:150-205,280-307`). CI wires all four targets to the full suite and named golden step (`.github/workflows/ci.yml:45-130`). The only remote run is for stale head `5f6d5f5`, not current `b296786`; Linux and macOS arm64 passed, Windows failed before the golden step, and macOS x86_64 remains queued (run 34226193383). There is no complete four-target result for the current head. |
| 5 | Licence, forbidden-container, unchecked-operation, unwrap/expect, and no-DSP guards prove they bite; the complete suite remains green without reference tools | ✓ VERIFIED | Fresh `cargo deny check` passed. Fresh `bash tools/prove-guards.sh` reported 6/6 deliberate violations rejected, including LGPL, direct and aliased `HashMap`, raw indexing, unchecked arithmetic plus unwrap, and f64/transcendental use. A fresh fully offline run with `IAMF_REF_DECODER` unset and a deliberately absent container image passed all 285 tests and printed explicit skip reasons. |

**Score:** 2/5 authoritative truths verified (2 failed as stale contracts, 1 awaiting cross-target execution)

### Required Artifacts

All 55 paths named by the eight PLAN frontmatters exist. They are substantive and exercised by the fresh test suite; none is a placeholder. The helper `gsd-tools verify artifacts` incorrectly returned `No must_haves.artifacts found` despite the YAML entries being present, so existence, contents, exports, consumers, and tests were checked directly.

| Plan | Required artifacts | Status | Details |
|---|---|---|---|
| 01-01 | `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `deny.toml`, `src/lib.rs`, `src/error.rs`, `tools/prove-guards.sh`, `REFERENCES.md`, `PATENTS`, `NOTICE`, `CONTRIBUTING.md`, `CONFORMANCE-GATE.md`, `.github/workflows/ci.yml` | ✓ EXISTS + SUBSTANTIVE + WIRED | Build, Clippy, cargo-deny, error-shape tests, guard proof, and CI inspection all passed. `Cargo.toml:1-76`; `.github/workflows/ci.yml:45-161`. |
| 01-02 | `tools/build-reference.sh`, `tools/iamf-tools.Dockerfile`, `tests/reference_manifest.rs`, `tests/fixtures/MANIFEST.md`, `.github/workflows/reference.yml` | ✓ EXISTS + SUBSTANTIVE + WIRED | Local manifest points at an executable pinned decoder; manifest test passed with the decoder enabled. The local image digest matches `REFERENCES.md`, and the full reference-gated suite ran successfully. |
| 01-03 | `src/bits/{mod,reader,writer,leb128}.rs`, `tests/vectors.rs`, `tests/bits_oracle.rs`, `tests/citations.rs` | ✓ EXISTS + SUBSTANTIVE + WIRED | Hand-rolled cursor/writer, capped ULEB128, 31 vectors, five differential proptests, and the mechanical citation test all passed. `src/bits/mod.rs:1-109`; `tests/bits_oracle.rs:1-80`. |
| 01-04 | `src/obu/{header,boundaries,mod}.rs`, `tests/refcorpus.rs`, `tests/obu_header.rs` | ✓ EXISTS + SUBSTANTIVE + WIRED | 25 header tests and six corpus/boundary tests passed, including the generic 2 MiB ceiling regression and 39-file offline walk. |
| 01-05 | `src/obu/{sequence_header,codec_config,audio_element,mix_presentation}.rs`, `src/model/layout.rs`, `tests/descriptors.rs` | ✓ EXISTS + SUBSTANTIVE + WIRED | 47 descriptor tests passed, including the complete 120-byte prologue and annotation-count regressions. The standing `01-TDD-GATE-WAIVER.md` was honored; history was not re-investigated or rewritten. |
| 01-06 | `src/obu/{audio_frame,temporal_delimiter,parameter_block}.rs`, `src/packing.rs`, `tests/{packing,temporal}.rs` | ✓ EXISTS + SUBSTANTIVE + WIRED | Ten packing and 29 temporal tests passed, including `Reserved(0..=2)`, duration/count preflight, implicit IDs, and final trimming. |
| 01-07 | `src/sequence.rs`, `src/model/{profile,loudness}.rs`, `tests/{sequence,profile}.rs` | ✓ EXISTS + SUBSTANTIVE + WIRED | 14 sequence and 29 profile tests passed; streaming and whole-file paths agree, failure poisoning works, and `test_000003` reproduces. |
| 01-08 | `tests/conformance.rs`, `tests/fixture.rs`, `src/dump.rs`, `tests/fixtures/golden/`, `DIFF-LEDGER.md`, `tests/golden.rs` | ⚠ SUBSTANTIVE + WIRED, CONTRACT DRIFT | All artifacts exist and their tests pass, including both external oracles and an empty exact diff. The artifact set implements the documented three-fixture fallback, not the original single two-Codec-Config / 24-bit-big-endian PLAN truth. W-1 formally covers the upstream 24-bit-BE decoder defect; the two-Codec-Config substitution was user-approved but not propagated to ROADMAP criterion 2. |

**Artifacts:** 55/55 exist and are substantive; 55/55 are wired. One plan's original fixture contract is deliberately superseded but incompletely propagated to the authoritative planning documents.

### Key Link Verification

All 30 PLAN key links were traced manually because the same frontmatter parser issue prevented `gsd-tools verify key-links` from discovering them.

| Plan | Connections checked | Status | Details |
|---|---|---|---|
| 01-01 | crate root → error exports; Cargo lints → Clippy config; proof script → lint canaries; CI → proof script | ✓ WIRED | `src/lib.rs` re-exports the error surface; `Cargo.toml:69-76` activates the configured lints; `.github/workflows/ci.yml:153-161` runs deny and guard proofs. |
| 01-02 | manifest test → `REFERENCES.md`; build script → `.reference-manifest.json`; reference workflow → Dockerfile/tools | ✓ WIRED | Fresh manifest test and reference-gated conformance run used the pinned native binary and container. `.github/workflows/reference.yml:61-129`. |
| 01-03 | reader/writer → located crate errors; oracle → writer; citation walker → all `src/**/*.rs` | ✓ WIRED | Differential and citation targets passed; shipping dependency graph is exactly `iamf`, `thiserror`. |
| 01-04 | header → `BitWriter`; boundary walker → capped ULEB128; refcorpus → fixtures; common reader → central trailing drain | ✓ WIRED | Header, oversized-OBU, trailing, and 39-file corpus tests passed. |
| 01-05 | Audio Element → Codec Config IDs; Mix Presentation → Audio Element IDs; layout types → their distinct OBUs; reserved raw → trailing precedence | ✓ WIRED | `DescriptorSet::validate` resolves both reference directions (`src/model/mod.rs:134-166`); descriptor vectors and reserved round trips passed. |
| 01-06 | packing plan → Audio Frame OBU types; packing plan → Audio Element counts; Parameter Block → descriptor-supplied definition/kind | ✓ WIRED | `tests/packing.rs` and `tests/temporal.rs` passed, including the post-review context-propagation regressions. |
| 01-07 | sequence writer → `DescriptorSet`; temporal unit → `SubstreamPlan`; selected profile → sequence header; Q7.8 → loudness fields | ✓ WIRED | End-to-end sequence, profile boundaries, quantisation, and deterministic output tests passed. |
| 01-08 | conformance → manifest; conformance → production sequence writer; computed diff → ledger; golden test → committed artifacts | ✓ WIRED | `assert_conformant` checks the manifest before encoding (`tests/conformance.rs:1003-1030`), drives `Fixture::encode`, and invokes both tools. Golden tests recompute bytes/hash/dump (`tests/golden.rs:150-276`). |

**Wiring:** 30/30 connections verified.

## Requirements Coverage

Every one of the 63 Phase 1 requirement IDs from ROADMAP/REQUIREMENTS is accounted for below. Status is based on code and executable evidence, not the checked boxes in `REQUIREMENTS.md`.

| Requirement(s) | Status | Evidence / blocking issue |
|---|---|---|
| BITS-01 | ⚠ CONTRACT DRIFT | The implementation intentionally uses `BitCursor` and a hand-rolled writer; `bitstream-io` is dev-only (`Cargo.toml:21-38`; `src/bits/mod.rs:12-45`). This is the documented D-01 amendment and is tested against `bitstream-io`, but `REQUIREMENTS.md:14` still requires a `BitReader` wrapping it. |
| BITS-02, BITS-03, BITS-04, BITS-05, BITS-06 | ✓ SATISFIED | Mirrored primitives, minimal/capped ULEB128, alignment checks, hand vectors, and differential tests all passed. |
| BITS-07 | ⚠ CONTRACT DRIFT | The intended property is stronger: no foreign I/O error boundary exists in shipping code. But `REQUIREMENTS.md:20` still says `src/bits` touches `bitstream-io` and maps `std::io::Error`; only `tests/bits_oracle.rs` imports it. |
| OBU-01, OBU-02, OBU-03, OBU-04, OBU-05, OBU-06, OBU-07, OBU-08 | ✓ SATISFIED | Header byte, size origin/cap, two-case type-specific flag, legality, END-before-START trim, extension, trailing preservation, and exact boundary walk all have passing vector/regression tests. |
| DESC-01, DESC-02, DESC-03, DESC-04, DESC-05, DESC-06, DESC-07, DESC-08, DESC-09 | ✓ SATISFIED | All descriptor models and ordering rules are exercised by 47 passing tests and the exact 120-byte reference prologue. Big-endian sense also passes through the 16-bit reference fixture; W-1 precisely records the upstream 24-bit-BE defect. |
| TIME-01, TIME-02, TIME-03, TIME-04, TIME-05 | ✓ SATISFIED | Audio Frame types, BCG permutation-only packing, final trim, delimiter, and explicit Parameter Block context are covered by 39 passing tests. |
| SEQ-01, SEQ-02, SEQ-03 | ✓ SATISFIED | Standalone writer, streaming primitive, and whole-file wrapper exist and agree byte-for-byte; failure poisoning regressions pass. |
| PROF-01, PROF-02, PROF-03 | ✓ SATISFIED | Profile domain/selection and exact ties-to-even Q7.8 behavior pass all boundary tests. |
| CONF-01, CONF-02, CONF-03, CONF-04, CONF-05, CONF-06, CONF-07, CONF-08, CONF-09, CONF-10 | ✓ SATISFIED | Fresh external-oracle run passed sample identity, strict parsing, and zero-byte diff; golden reproduction and offline suite also passed. CONF-04 is satisfied exactly as REQUIREMENTS states (two Codec Configs **or two Audio Elements**) by the structure fixture, even though ROADMAP criterion 2 is stricter and stale. |
| CONF-11 | ⚠ CONTRACT DRIFT | The checked requirement still demands a one-time Bazel generation based on the disproven premise that only one `.iamf` ships. Research found 221 committed `libiamf` fixtures; 39 capped files were vendored by pinned `git show` instead (`01-02-SUMMARY.md:205-207`). The corpus exists and is tested, but `REQUIREMENTS.md` was not amended to correction 6. |
| GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-05, GUARD-06, GUARD-07, GUARD-08 | ✓ SATISFIED | Cargo-deny, Clippy configuration/canaries, version pin, reference pins, notice, and contamination checklist exist; machine-verifiable checks passed. |
| GUARD-09 | ? NEEDS CURRENT FOUR-TARGET RUN | Golden/hash tests and four-target CI wiring exist, but there is no complete green four-target result for current head. The stale run has a failed Windows test step and a queued macOS x86_64 job. |
| GUARD-10, GUARD-11, GUARD-12, GUARD-13 | ✓ SATISFIED | Double encode, no-DSP proof, pinned Rust 1.85/edition 2024, and typed located error surface all pass. |
| DEC-01, DEC-02, DEC-04, DEC-05 | ✓ SATISFIED | Spec/license are pinned; both `iamf-tools` tags and the relevant pinned `libiamf` sources were investigated and recorded before implementation. |

**Coverage:** 59/63 satisfied exactly; 3 implemented through documented superseding decisions whose requirement text remains stale (BITS-01, BITS-07, CONF-11); 1 awaits current four-target execution (GUARD-09).

## Anti-Patterns Found

The scan covered phase-authored Rust, tests, tools, workflows, and root conformance/metadata files, excluding binary/vendored fixture payloads.

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `src/obu/mix_presentation.rs` | 10, 318 | `TODO(b/339855338)` | ℹ️ Info | Citation of an upstream `iamf-tools` read-path TODO; not a local incomplete implementation. Local validation is substantive and tested. |
| `tests/descriptors.rs` | 676 | upstream TODO reference | ℹ️ Info | Test explanation, not a local placeholder. |
| `DIFF-LEDGER.md` | 58 | word “placeholder” | ℹ️ Info | Explicitly says the empty asserted ledger is *not* a placeholder; the ledger mutation tests pass. |
| Phase-authored Rust/tests | multiple | rustfmt differences | ⚠️ Warning | `cargo fmt --all -- --check` reports the already-documented repository-wide drift. It does not affect compilation, tests, byte identity, or conformance and is not a phase-goal blocker. |

**Anti-patterns:** 4 contextual findings (0 blockers, 1 warning, 3 informational). No local TODO, stub, empty implementation, log-only function, or placeholder output blocks the goal.

## Human / External Verification Required

### 1. Current four-target byte identity

**Test:** Push current head `b2967866aec8dc5b5ae3392c667eb48646447706` and run `.github/workflows/ci.yml` until the Linux x64, Windows MSVC, macOS arm64, and macOS x86_64 matrix jobs each execute the named golden-fixture step.

**Expected:** All four jobs pass `cargo test --locked --target <target> --test golden --test fixture`, reproducing hash `3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c` and same-process double encoding.

**Why external:** This macOS x86_64 host cannot execute the other three target binaries. Static workflow inspection proves wiring, not target execution.

### 2. Licence/patent and contamination attestation

**Test:** Maintainer confirms the checked-in `PATENTS`/`NOTICE` treatment and uses the `CONTRIBUTING.md` source-contamination checkbox for contributors.

**Expected:** Project counsel/maintainer accepts the AOM Patent License treatment, and contributors attest they did not consult the prohibited sources.

**Why human:** Presence and wording are machine-checkable; legal sufficiency and contributor provenance are human assertions.

## Gaps Summary

### Critical Gaps (Block Phase Closure)

1. **Authoritative ROADMAP criteria do not reflect settled Phase 1 decisions**
   - Missing: Criterion 2 still requires one sample-identical fixture with two Codec Configs, though the user adopted the two-fixture/two-Audio-Element fallback after a pinned `decoder_main` abort. Criterion 3 still says “v1.x tag,” though the investigated and pinned IAMF-1.1-exact tool release is v2.1.0.
   - Impact: Two of five authoritative truths are false as written even though the intended conformance behavior passes both local external oracles.
   - Fix: Reconcile ROADMAP with the documented user decision and reference-version correction, or explicitly reject those decisions and implement a new conforming fixture strategy.

2. **No complete green four-target proof for current head**
   - Missing: A CI run of current head in which all four target jobs execute and pass the golden step.
   - Impact: “Reproducible byte-for-byte on every target” remains unproven. The existing stale run cannot close the gate: Windows failed before the golden step and macOS x86_64 is still queued.
   - Fix: Run the current workflow on current head, diagnose any current Windows failure, and use the documented Rosetta/waiver route only if GitHub can no longer supply macOS x86_64 execution.

3. **Three REQUIREMENTS descriptions retain superseded implementation premises**
   - Missing: BITS-01/BITS-07 still prescribe shipping `bitstream-io`; CONF-11 still prescribes one-time Bazel fixture generation.
   - Impact: The traceability ledger marks requirements complete whose literal descriptions are not implemented, obscuring the deliberate D-01 and correction-6 replacements.
   - Fix: Amend the requirement text with the same evidence and rationale already present in `src/bits/mod.rs`, `Cargo.toml`, and `01-02-SUMMARY.md`.

### Non-Critical Gaps (Can Defer)

1. **Repository-wide rustfmt drift**
   - Issue: `cargo fmt --all -- --check` is not clean.
   - Impact: Formatting only; fresh build, strict Clippy, tests, hashes, and both reference oracles pass.
   - Recommendation: Resolve in one mechanical formatting commit after phase closure so it does not obscure semantic changes.

## Recommended Fix Plans

### 01-09-PLAN.md: Reconcile the Phase 1 Contract

**Objective:** Make ROADMAP and REQUIREMENTS describe the evidence-backed, user-approved implementation without weakening the Core Value.

**Tasks:**
1. Update ROADMAP criterion 2 to describe the sample-identity plus structure-fixture split adopted in `CONFORMANCE-GATE.md`, and criterion 3 to name `iamf-tools@v2.1.0` as the pinned tool release implementing IAMF v1.1.0.
2. Amend BITS-01/BITS-07 and CONF-11 in REQUIREMENTS to their documented D-01 and correction-6 forms, preserving IDs and traceability.
3. Re-run requirement/roadmap consistency and Phase 1 verification.

**Estimated scope:** Small

---

### 01-10-PLAN.md: Close the Four-Target Byte-Identity Gate

**Objective:** Produce a complete green matrix result for the current reviewed head.

**Tasks:**
1. Run the current head through all four CI targets and capture the golden-step results.
2. If current Windows fails, diagnose and fix the target-specific failure; if macOS x86_64 is unavailable, execute the documented Rosetta route or record the explicit waiver the gate requires.
3. Re-run Phase 1 verification against the completed CI evidence.

**Estimated scope:** Small to medium, depending on runner availability and the Windows failure.

## Verification Metadata

**Verification approach:** Goal-backward from ROADMAP Success Criteria, with PLAN artifacts/key links and all mapped REQUIREMENTS cross-checked.
**Must-haves source:** ROADMAP success criteria (authoritative), plus eight PLAN frontmatters for supporting artifacts and links.
**Automated checks:**

- `cargo test --locked --all-targets` — PASS, 285 tests.
- `cargo clippy --locked --all-targets -- -D warnings` — PASS.
- `IAMF_REF_DECODER=<manifest path> IAMF_TOOLS_IMAGE=iamf-tools:v2.1.0 cargo test --locked --test reference_manifest` — PASS, 4 tests.
- Same environment, `cargo test --locked --test conformance -- --nocapture --test-threads=1` — PASS, 22 tests; libiamf PCM identity, strict parser, and zero-byte diff observed.
- `env -u IAMF_REF_DECODER IAMF_TOOLS_IMAGE=iamf-tools:definitely-absent cargo test --locked --all-targets -- --nocapture` — PASS, 285 tests with explicit skips.
- `cargo deny check` — PASS: advisories, bans, licences, sources.
- `bash tools/prove-guards.sh` — PASS, 6/6 canaries fired.
- `cargo tree -e normal,no-proc-macro --prefix none | ...` — PASS: `iamf`, `thiserror` only.
- Golden SHA-256 recomputation — PASS: `3e53f10...1282c` matches committed hash.
- PLAN artifact presence scan — PASS, 55/55 paths.
- Phase-authored anti-pattern scan — PASS with contextual informational hits only.
- `cargo fmt --all -- --check` — WARNING: known repository-wide drift; non-blocking.
- GitHub CI run 34226193383 — INCOMPLETE/STALE: two target jobs green, Windows test failed, macOS x86_64 queued; head differs from current.

**Code review gate:** `01-REVIEW.md` reports all six findings resolved; the focused regressions and full suite passed fresh. `01-TDD-GATE-WAIVER.md` was honored exactly.
**Human/external checks required:** 2.
**Overall decision:** `gaps_found`. Core local conformance is demonstrated, but phase closure must wait for planning-contract reconciliation and a current complete four-target result.

---
*Verified: 2026-09-08T20:37:14Z*
*Verifier: Codex (phase-verification subagent)*
