---
phase: 01-conformant-lpcm-bitstream
verified: 2026-09-08T22:47:05Z
status: passed
score: 5/5 must-haves verified
---

# Phase 1: Conformant LPCM Bitstream Verification Report

**Phase Goal:** A standalone `.iamf` file this crate writes is accepted by both
reference implementations, is byte-explicable against reference output, and is
reproducible byte-for-byte on every target.

**Verified:** 2026-09-08T22:47:05Z
**Status:** passed

## Goal Achievement

### Observable Truths

ROADMAP success criteria are the authoritative truths. Plan-level must-haves identify
the supporting artifacts and connections but do not relax that contract.

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | Reproduce `libiamf/tests/test_000003.iamf` byte-for-byte and walk our output exactly to `bytes.len()` | ✓ VERIFIED | Fresh `cargo test --locked --all-targets` passed `reproduces_test_000003` and the boundary-walk regression: 32567 bytes, 67 OBUs, final boundary at the file length. |
| 2 | The pinned `libiamf` decodes the non-silent, distinguishable, trimmed sample fixture sample-identically; the separate structure fixture exposes repeated descriptors | ✓ VERIFIED | Fresh reference-gated execution reported 300/300 frames and 0/1800 differing samples for the six-channel 24-bit LE fixture, `trim_at_end = 84`, and 16 OBUs. The structure fixture reported 11 OBUs and two Audio Elements. ROADMAP criterion 2 now names this approved split. |
| 3 | The pinned IAMF-v1.1.0-compatible `iamf-tools` release accepts the fixture and produces an identical or exhaustively ledgered file | ✓ VERIFIED | `iamf-tools@v2.1.0` at `848c6ff4968ff8cc6f728259892ab4f90cb83256` accepted all three fixtures. CONF-07 freshly produced 7073 bytes on both sides, 0 differing offsets, and an empty executable ledger. |
| 4 | The golden fixture and same-process double encode succeed on macOS arm64, macOS x86_64, Windows MSVC and Linux x64 | ✓ VERIFIED | Exact-candidate CI run [34287100850](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850) has `headSha` `5fea02a00d23665353fed8e317d7cb7ab8325694`, terminal `success`, and four successful named golden steps. Windows preserved LF checkout bytes; the x86_64 macOS binaries executed under Rosetta 2. Durable job evidence is in `CONFORMANCE-GATE.md`. |
| 5 | Licence, container, unchecked-operation, unwrap/expect and no-DSP guards bite while the complete suite remains green offline | ✓ VERIFIED | Fresh full tests passed 285/285, strict Clippy passed, cargo-deny passed all four checks, and `tools/prove-guards.sh` reported 6/6 deliberate violations rejected. CI's separate licence/guardrail job also passed. |

**Score:** 5/5 authoritative truths verified.

### Required Artifacts

All 58 artifact declarations across plans 01-01 through 01-10 resolve to 56 unique
repository paths; all 56 exist. The full suite and focused checks exercise the Rust,
test, fixture, workflow, reference-manifest and planning-contract artifacts. The
`gsd-tools verify artifacts` helper still reports `No must_haves.artifacts found`
for these YAML frontmatters, so a fresh direct YAML extraction and existence check
was used.

| Area | Status | Evidence |
|---|---|---|
| Guardrails and toolchain | ✓ EXISTS + SUBSTANTIVE + WIRED | `cargo deny check`, strict Clippy, pinned-toolchain CI steps and 6/6 guard canaries passed. |
| Reference tooling and manifests | ✓ EXISTS + SUBSTANTIVE + WIRED | The manifest test passed against the pinned local `iamfdec`; the pinned `iamf-tools:v2.1.0` image ran the strict parser and reference encoder. |
| Bit, OBU, descriptor, temporal and sequence implementation | ✓ EXISTS + SUBSTANTIVE + WIRED | Fresh all-target execution passed every unit and integration target, including the exact 120-byte prologue, 2 MiB bounds, reserved round trips, streaming poisoning and 32567-byte reproduction. |
| Conformance fixtures and ledger | ✓ EXISTS + SUBSTANTIVE + WIRED | The three fixtures, golden bytes/hash/dump and empty bidirectional diff ledger all reproduced. Golden SHA-256 is `3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c`. |
| Gap-closure contracts and CI evidence | ✓ EXISTS + SUBSTANTIVE + WIRED | ROADMAP/REQUIREMENTS match the implemented decisions; `CONFORMANCE-GATE.md` records one exact-SHA four-target outcome with canonical run/job URLs. |

**Artifacts:** 56/56 unique paths exist and are substantive; 56/56 are wired.

### Key Link Verification

All 36 declared key links were checked. Direct pattern checks find 34; the two
remaining links are deliberately indirect and were traced through their production
call paths:

- `src/sequence.rs` consumes the already packed `TemporalUnit` frame payloads;
  `SubstreamPlan` is constructed by the fixture/conformance path before those units
  are sent through `SequenceWriter`.
- `tests/conformance.rs` calls the production sequence path through
  `tests/support/fixture.rs::Fixture::encode`, which constructs `SequenceWriter`;
  the test intentionally does not duplicate that implementation detail.

The new evidence links also pass: each durable target row maps to the exact matrix
job and named golden step, and this report consumes the exact SHA/run recorded in
`CONFORMANCE-GATE.md`.

**Wiring:** 36/36 connections verified semantically.

## Requirements Coverage

Fresh contract validation found exactly 63 checked Phase 1 definitions and exactly
63 unique `Phase 1 | Complete` traceability rows with identical ID sets.

| Requirement group | Status | Evidence |
|---|---|---|
| BITS-01..07 | ✓ SATISFIED | Hand-rolled cursor/writer, located native errors, minimal and fixed-size capped ULEB128, vectors and dev-only `bitstream-io` differential tests pass. Reconciled BITS-01/BITS-07 describe this boundary. |
| OBU-01..08 | ✓ SATISFIED | Header packing, size origin/cap, type-specific flag, legality, trimming order, extension/trailing preservation and exact boundary walking pass. |
| DESC-01..09 | ✓ SATISFIED | Descriptor models, ordering, annotations, profiles, layouts and the exact reference prologue pass. W-1 precisely scopes the pinned decoder's 24-bit-BE defect without weakening the encoder evidence. |
| TIME-01..05 | ✓ SATISFIED | Frame IDs, BCG packing, final trim, delimiter and parameter-definition context pass all focused regressions. |
| SEQ-01..03, PROF-01..03 | ✓ SATISFIED | Streaming and whole-file output agree; failure poisoning, profile selection and exact ties-to-even Q7.8 behavior pass. |
| CONF-01..11 | ✓ SATISFIED | Both pinned external oracles, exact empty diff, sample identity, structure, corpus provenance, offline behavior and manifest checks pass. CONF-11 now describes the committed pinned-source corpus. |
| GUARD-01..13 | ✓ SATISFIED | Local policy proofs pass, and GUARD-09 now has four successful exact-candidate target executions in one run. |
| DEC-01, DEC-02, DEC-04, DEC-05 | ✓ SATISFIED | Spec, licensing, tool release and payload-reference decisions are pinned and reflected in code and contracts. |

**Coverage:** 63/63 satisfied.

## Exact-Candidate CI Evidence

The verifier independently queried run 34287100850 after it reached a terminal
state. It returned `headSha = 5fea02a00d23665353fed8e317d7cb7ab8325694`,
`status = completed`, and `conclusion = success`.

| target | job | route | named golden step |
|---|---:|---|---|
| aarch64-apple-darwin | [102265091867](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265091867) | native arm64 macOS | completed / success |
| x86_64-apple-darwin | [102265092177](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092177) | x86_64 execution under Rosetta 2 | completed / success |
| x86_64-pc-windows-msvc | [102265092145](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092145) | native Windows with LF preserved | completed / success |
| x86_64-unknown-linux-gnu | [102265092207](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092207) | native Linux x86_64 | completed / success |

This is the `normal-four-target` branch. No D-17 runner waiver is active, and no
jobs from the failed predecessor candidate were combined with this result.

## Closure Ancestry and Path Boundary

The tested SHA is an ancestor of the current closure work. At verification time,
every descendant or uncommitted path is inside plan 01-10's explicit allowlist:
`CONFORMANCE-GATE.md`, this report, the forthcoming `01-10-SUMMARY.md`, and the
listed `.planning` state/roadmap/config files. No `src/**`, `tests/**`, fixture,
workflow, toolchain or dependency path follows the tested candidate. The same check
must run again after the summary and final state commit; that terminal result belongs
in `01-10-SUMMARY.md`.

## Anti-Patterns Found

| Finding | Severity | Disposition |
|---|---|---|
| Two `TODO(b/339855338)` references in `mix_presentation.rs` and its test | ℹ️ Info | Citations to an upstream `iamf-tools` read-path TODO, not local incomplete work. |
| `DIFF-LEDGER.md` contains “placeholder” | ℹ️ Info | The sentence explicitly states the empty, executable ledger is not a placeholder. |
| `cargo fmt --all -- --check` reports repository-wide drift beginning in `src/dump.rs` | ⚠️ Warning | Pre-existing and documented; it does not affect compilation, strict Clippy, tests, reference results or byte identity. Keep as a separate mechanical cleanup. |
| GitHub annotation for Node.js 20 deprecation in pinned checkout v4 | ℹ️ Info | GitHub forced Node.js 24 and the action succeeded. This is maintenance input, not a Phase 1 correctness gap. |

**Blockers:** none.

## Human / External Verification

### Maintainer legal/provenance attestation

**Test:** The maintainer or counsel confirms the checked-in `PATENTS`/`NOTICE`
treatment and applies the `CONTRIBUTING.md` prohibited-source attestation.

**Expected:** The AOM Patent License treatment is accepted and contributors attest
they did not consult prohibited LGPL sources.

**Why human:** Wording and file presence are machine-checkable; legal sufficiency and
contributor provenance are human assertions. This remains explicitly human-owned
due diligence and is not an implementation, contract or byte-identity gap.

## Verification Metadata

**Automated checks:**

- `cargo test --locked --all-targets` — PASS, 285 tests.
- `cargo clippy --locked --all-targets -- -D warnings` — PASS.
- `cargo deny check` — PASS: advisories, bans, licences and sources.
- `bash tools/prove-guards.sh` — PASS, 6/6 canaries.
- Linked shipping graph — PASS: exactly `iamf` and `thiserror`.
- Pinned manifest plus external-oracle conformance run — PASS, 4 + 22 tests;
  300/300 frames, 0/1800 sample differences, strict parser acceptance and 0-byte diff.
- Golden hash recomputation — PASS: `3e53f10b…1282c`.
- Requirement identity/traceability — PASS: 63 checked definitions and 63 unique
  Complete rows.
- Artifact existence — PASS: 58 declarations, 56 unique paths, 0 missing.
- Key links — PASS: 36/36 after tracing two intentional indirect production paths.
- Evidence-shape validator — PASS: exactly one `normal-four-target` marker and exactly
  four unique successful target rows.
- Exact-run re-query — PASS: run 34287100850 completed successfully at the recorded SHA.
- `cargo fmt --all -- --check` — known non-blocking warning only.

**Code review gate:** `01-REVIEW.md` remains resolved. Candidate remediation
`5fea02a` changes only `.github/workflows/ci.yml`; no Rust source or test change was
made after review.

**Overall decision:** `passed`. All five ROADMAP truths and all 63 Phase 1
requirements have current executable evidence. The remaining legal/provenance
attestation is deliberately human-owned due diligence, not an unresolved Phase 1
implementation or contract gap.

---
*Verified: 2026-09-08T22:47:05Z*
*Verifier: Codex (fresh goal-backward Phase 1 verification)*
