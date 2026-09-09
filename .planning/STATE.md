---
gsd_state_version: "1.0"
milestone: v1
current_phase: 03
current_phase_name: FLAC and Opus Framing
status: ready_to_plan
stopped_at: Phase 02 verified and complete; ready to plan Phase 03
last_updated: "2026-09-09T03:53:51.644Z"
last_activity: 2026-09-09
last_activity_desc: Phase 02 verified complete — 5/5 truths and 12/12 requirements
state_head: 725193c
progress:
  total_phases: 4
  completed_phases: 2
  total_plans: 17
  completed_plans: 17
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-08)

**Core value:** A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM sample-identical — and accepted by `iamf-tools`' stricter parser, because `libiamf` alone is a permissive oracle.
**Current focus:** Phase 03 — FLAC and Opus Framing

## Current Position

Phase: 03 (FLAC and Opus Framing) — NOT STARTED
Plan: Not started
Status: Ready to plan
Last activity: 2026-09-09 — Phase 02 verified complete

Progress: [░░░░░░░░░░] 0% of Phase 03 plans

## Performance Metrics

*Updated after each plan completion*
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 23min | 4 tasks | 18 files |
| Phase 01 P02 | 35min | 3 tasks | 89 files |
| Phase 01 P03 | 47min | 3 tasks | 12 files |
| Phase 01 P04 | 42min | 4 tasks | 9 files |
| Phase 01 P05 | 29min | 4 tasks | 11 files |
| Phase 01 P06 | 26min | 3 tasks | 10 files |
| Phase 01 P07 | 62 | 3 tasks | 13 files |
| Phase 01 P08 | ~2h | 3 tasks | 17 files |
| Phase 01 P09 | 4min | 3 tasks | 2 files |
| Phase 01 P10 | 34min | 4 tasks | 5 files |
| Phase 02 P01 | 16min | 2 tasks | 7 files |
| Phase 02 P02 | 14min | 2 tasks | 13 files |
| Phase 02 P03 | 15min | 2 tasks | 3 files |
| Phase 02 P04 | 13min | 2 tasks | 3 files |
| Phase 02 P05 | 19min | 3 tasks | 10 files |
| Phase 02 P06 | 16min | 2 tasks | 11 files |
| Phase 02 P07 | 12min | 2 tasks | 19 files |

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
- [Phase 01]: Hand-rolled BitCursor/BitWriter over &[u8] with bitstream-io demoted to a dev-only proptest differential oracle (D-01), imported from exactly one test file and absent from the linked graph
- [Phase 01]: Minimal uleb128 is implemented as fixed-size uleb128 at minimal_len, mirroring LebGenerator: one encoder loop, and minimality is structural rather than emergent
- [Phase 01]: read_string returns the NUL terminator and write_string appends it; the asymmetry makes an unterminated string unrepresentable on the write side
- [Phase 01]: D-03's recorded rationale corrected in src/bits/mod.rs: the fixed-size encoder is for parsing and test_000134, not for CONF-08's test_000003 (research correction 4)
- [Phase 01]: Bit 6 of the OBU header has TWO variants at the pinned v1.1.0 tree, not four. REQUIREMENTS.md OBU-03 amended with its evidence; PITFALLS.md §2 annotated with its provenance.
- [Phase 01]: Derived lengths are MEASURED from the cursor, never recomputed via minimal_len: a legal non-minimal obu_size (PARSE-04) would otherwise shift every later offset in a foreign file.
- [Phase 01]: The OBU reader is deliberately less strict than the writer — write_obu refuses an illegal flag, read_obu_header accepts it, because libiamf@v1.1.0 accepts it too.
- [Phase 01]: find_obu_boundaries returns OBU starts plus the final end offset, so test_000003 yields 68 entries for 67 OBUs; an empty input yields [0] rather than an error.
- [Phase 01]: D-04's AudioElementType published as locked: #[non_exhaustive] ChannelBased/SceneBased/Reserved{value,raw}, only ChannelBased publicly constructible; Reserved round-trips byte-identically (tested over types 2..=7)
- [Phase 01]: param_definition_mode is a derived accessor over Option<DurationFields>, not a stored bool — a stored copy makes the flag/field disagreement constructible
- [Phase 01]: recon_gain_is_present stays a stored field: it is the one gate flag whose gated data lives in a different OBU, so Pattern 2 has no local source to derive it from
- [Phase 01]: SoundSystem variants named A0_2_0..Ss13_6_9_0, not the reference's A_0_2_0 — non_camel_case_types rejects a cased character adjacent to an underscore and -D warnings is a verify gate
- [Phase 01]: AudioFrame carries neither trimming nor trailing — the OBU header owns the trim counts and the frame claims the whole payload remainder
- [Phase 01]: read_parameter_block takes the ParamDefinition AND its ParamDefinitionType as explicit arguments; the shared wire prefix carries no type
- [Phase 01]: Layouts research did not close return UnsupportedLayout rather than an invented packing order
- [Phase 01]: finish(self) makes push-after-finish a compile error, so ErrorKind::SequenceFinished was deliberately not added — an unreachable error variant is worse than none
- [Phase 01]: The float→fixed join is lufs_to_q7_8 plus Loudness::from_q7_8, not an LUFS parameter on write_sequence — the latter would put an f64 in src/sequence.rs and break D-21's grep gate
- [Phase 01]: Channel-count helpers return Option, so a layout the spec version does not fix a count for is ErrorKind::UnsupportedLayout rather than a profile selected from nothing
- [Phase 01]: Shared integration-test fixtures live in tests/support/*.rs and are #[path]-included; a subdirectory of tests/ is not compiled as its own test target
- [Phase 01]: Research assumption A1 REFUTED: libiamf@v1.1.0's reads24be (bitstream.c:206-210) uses readu16le where readu16be was meant, so 24-bit big-endian LPCM is misread with its top two bytes transposed
- [Phase 01]: Sample-identity fixture is 24-bit LITTLE-endian; a third stereo 16-bit BIG-endian fixture keeps DESC-03's endianness sense asserted end to end (waiver W-1)
- [Phase 01]: CONF-07 is byte-identical: our .iamf and iamf-tools' encoder_main output are both 7073 bytes with the same sha256; DIFF-LEDGER.md is empty and asserted
- [Phase 01]: Sample identity remains on the 5.1 fixture; repeated-descriptor ordering is proved separately by the two-Audio-Element structure fixture.
- [Phase 01]: iamf-tools v2.1.0 at 848c6ff4968ff8cc6f728259892ab4f90cb83256 is the pinned tool release for IAMF v1.1.0; SPEC_VERSION remains 1.1.0.
- [Phase 01]: The Phase 1 cross-target terminal branch is normal-four-target; no D-17 runner waiver is active.
- [Phase 01]: Windows preserves committed LF bytes before checkout, and x86_64 macOS tests execute under Rosetta 2.
- [Phase 01]: Cross-target evidence is bound to candidate 5fea02a; closure descendants are restricted to the explicit documentation/state allowlist.
- [Phase 02]: Parameter Block readers require an explicit ordered ParamDefinitionRegistry carrying descriptor-owned parse context.
- [Phase 02]: Registry lookup follows emitted wire order and binds duplicate parameter IDs to the first definition.
- [Phase 02]: Reserved demixing modes and high Recon Gain bits are preserved and diagnosed rather than rejected or normalized.
- [Phase 02]: Each reserved group is stored at its nearest exact wire structure and emitted unchanged.
- [Phase 02]: Non-zero reserved syntax remains structurally permissive and is diagnosed only through validate().
- [Phase 02]: Sequence parsing dispatches the real cursor only through central bounded OBU readers and translates payload errors once.
- [Phase 02]: Flat validation and grouping retain exact wire order and never reconstruct or sort a DescriptorSet.
- [Phase 02]: ParsedSequence::from_parts mirrors canonical writer order without serialization and centralizes known trailing bytes for derived equality.
- [Phase 02]: Own-output byte identity covers both writer paths; legal foreign non-minimal obu_size widths canonicalize and are the explicit syntax caveat.
- [Phase 02]: Unknown fidelity is locked by semantic index, raw bytes, and complete absolute OBU boundary vectors.
- [Phase 02]: Bounded ungoverned Parameter Blocks preserve raw bytes and validate missing context; governed malformed syntax remains structural.
- [Phase 02]: Root fuzzing APIs are optional and enabled only for stable replay or the excluded workspace's roundtrip-model target.
- [Phase 02]: One bounded canonical generator supplies both stable smoke replay and the structural libFuzzer oracle.
- [Phase 02]: Permanent parser seeds are byte-identical pinned iamf-tools fixtures; structural seeds are named bounded models, and transient coverage expansion is minimized before retention.
- [Phase 02]: Stable CI exhaustively replays every retained corpus/artifact input on four target paths; scheduled Linux nightly fuzzing remains a separate bounded discovery job.

### Pending Todos

- Plan Phase 3: FLAC and Opus Framing.

### Blockers/Concerns

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

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| *(none)* | | | | |

## Session Continuity

Last session: 2026-09-09T03:53:51Z
Stopped at: Phase 02 verified and complete; ready to plan Phase 03
Resume file: None
