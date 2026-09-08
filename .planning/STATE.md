---
gsd_state_version: "1.0"
milestone: v1
current_phase: 01
current_phase_name: Conformant LPCM Bitstream
status: executing
stopped_at: Completed 01-02-PLAN.md
last_updated: "2026-09-08T02:47:41.092Z"
last_activity: 2026-09-08
last_activity_desc: Phase 01 execution started
state_head: 79a354bcc24093a735943e82182afd5a5bb60e1a
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 8
  completed_plans: 2
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-08)

**Core value:** A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM sample-identical — and accepted by `iamf-tools`' stricter parser, because `libiamf` alone is a permissive oracle.
**Current focus:** Phase 01 — Conformant LPCM Bitstream

## Current Position

Phase: 01 (Conformant LPCM Bitstream) — EXECUTING
Plan: 3 of 8
Status: Ready to execute
Last activity: 2026-09-08 — Phase 01 execution started

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: —
- Total execution time: —

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**

- Last 5 plans: —
- Trend: —

*Updated after each plan completion*
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 23min | 4 tasks | 18 files |
| Phase 01 P02 | 35min | 3 tasks | 89 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: Four phases, one per milestone M1–M4. M1 may not be reordered — everything downstream inherits its byte-level understanding silently.
- [Roadmap]: All 13 guardrails land in Phase 1. Installing them later means fixing violations rather than never writing them.
- [Roadmap]: Phase 1 exits through a seven-clause gate (CONF-02..CONF-08), not through "`libiamf` returned OK" — `libiamf` is a permissive reader.
- [Roadmap]: Bazel fixture generation and the pinned `libiamf` CI job start on Phase 1 day one, in parallel and off the code critical path.
- [Research]: `Vec` in bitstream order plus `by_id()` for descriptors — `BTreeMap` trades a determinism bug for a round-trip bug.
- [Phase 01]: Error surface locked as D-08/D-09: struct Error { kind, at } with Location = InputOffset|OutputOffset|Field|Unlocated; validate() returns Vec<Finding>. size_of::<Error>() is exactly 32, enforced by a const assertion proven to bite.
- [Phase 01]: cargo tree -e normal is the wrong instrument for D-02's zero-linked-dependencies claim (it includes proc-macro edges). Use cargo tree -e normal,no-proc-macro; CI asserts it.
- [Phase 01]: Toolchain pinned to exactly 1.85.0 (the MSRV floor), not the machine's 1.92.0 — so the advertised MSRV is the one we compile with. Every 01-03 dev-dependency must resolve under 1.85; proptest 1.11.0 sits exactly there.
- [Phase 01]: Fixture supply needs no Bazel run: libiamf@v1.1.0 commits 221 .iamf files with matching configurations and rendered WAVs under BSD-3-Clause-Clear, so CONF-11's one-time-Bazel-build premise is retired (research correction 6).
- [Phase 01]: A reference tool's exit code is never the conformance signal — proven by execution: iamfdec returns 0 on all five of a control and four corruptions, including two that decode to zero samples. Harnesses assert file existence, frame count and decoded-sample count.
- [Phase 01]: Vendored fixture policy (D-14): a 65 536-byte per-file cap and a 39-file curated subset, both enforced by tests/fixtures_cap.rs, because Parallax's path dependency puts every byte in every developer's clone and removal needs a history rewrite.
- [Phase 01]: Bazel is pinned by iamf-tools' own committed .bazelversion (7.4.1), not restated in the Dockerfile — one owner per pin. Bazelisk is pinned by release v1.29.0 AND the sha256 of the binary.

### Pending Todos

None yet.

### Blockers/Concerns

- **DEC-04** (`iamf-tools` v2.0.0/v2.1.0 tags unfetched — one may be a v1.1.0-exact tree) and **DEC-05**
  (`libiamf` payload rejection rules only partly traced) must be settled inside Phase 1, before the
  type model is written.
- **Decimation policy now sits in Parallax.** DEC-03 chose pre-decimated blocks, so Parallax owns
  position-curve decimation and must share it with the ADM BWF exporter — otherwise the two exports
  disagree on the same project. Not a blocker for this crate; a tracked consequence for its consumer.
- **AOM Patent License 1.0 §1.2 still unread.** No longer blocking now that the licence is settled;
  remains due diligence on the inbound grant.
- ISO-BMFF (v2) is the licence contamination milestone and was not researched at all. `gpac` is
  LGPL-2.1 and forbidden.
- Phase 1 work is on branch gsd/phase-01-conformant-lpcm-bitstream, not main: git.branching_strategy is "none" but main is the repo's protected default branch and the executor may not commit to it. Either fast-forward main, or set git.allow_default_branch_commits: true in .planning/config.json before plan 01-02.

### Resolved

- ~~DEC-01 spec version~~ → **IAMF v1.1.0** (2026-09-08)
- ~~DEC-02 crate licence~~ → **`MIT OR Apache-2.0`**, licence files landed (2026-09-08)
- ~~DEC-03 parameter tick rate~~ → **pre-decimated blocks**; Phase 4 entry condition satisfied (2026-09-08)

## Deferred Items

Items acknowledged and deferred at milestone close, most recent first:

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| *(none)* | | | | |

## Session Continuity

Last session: 2026-09-08T02:47:28.754Z
Stopped at: Completed 01-02-PLAN.md
Resume file: None
