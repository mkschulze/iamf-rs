---
gsd_state_version: "1.0"
milestone: v1
current_phase: 1
current_phase_name: Conformant LPCM Bitstream
status: executing
stopped_at: Phase 1 context gathered
last_updated: "2026-09-08T01:26:36.214Z"
last_activity: 2026-09-08
last_activity_desc: DEC-01/02/03 settled by the user; Phase 4 entry condition satisfied
state_head: 14b408e240a412dc6c7ff62ae2c7a0b81f103428
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 8
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-08)

**Core value:** A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM sample-identical — and accepted by `iamf-tools`' stricter parser, because `libiamf` alone is a permissive oracle.
**Current focus:** Phase 1 — Conformant LPCM Bitstream

## Current Position

Phase: 1 (Conformant LPCM Bitstream) — READY TO EXECUTE
Plan: 0 of 8 in current phase
Status: Ready to execute
Last activity: 2026-09-08 — DEC-01/02/03 settled by the user; Phase 4 entry condition satisfied

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

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: Four phases, one per milestone M1–M4. M1 may not be reordered — everything downstream inherits its byte-level understanding silently.
- [Roadmap]: All 13 guardrails land in Phase 1. Installing them later means fixing violations rather than never writing them.
- [Roadmap]: Phase 1 exits through a seven-clause gate (CONF-02..CONF-08), not through "`libiamf` returned OK" — `libiamf` is a permissive reader.
- [Roadmap]: Bazel fixture generation and the pinned `libiamf` CI job start on Phase 1 day one, in parallel and off the code critical path.
- [Research]: `Vec` in bitstream order plus `by_id()` for descriptors — `BTreeMap` trades a determinism bug for a round-trip bug.

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

Last session: 2026-09-07T23:53:37.652Z
Stopped at: Phase 1 context gathered
Resume file: .planning/phases/01-conformant-lpcm-bitstream/01-CONTEXT.md
