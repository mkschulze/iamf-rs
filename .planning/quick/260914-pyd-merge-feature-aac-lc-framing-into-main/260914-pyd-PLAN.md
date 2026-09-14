---
phase: quick-260914-pyd
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - .github/workflows/reference.yml
  - README.md
  - REFERENCES.md
  - docs/superpowers/plans/2026-09-12-aac-lc-framing.md
  - docs/superpowers/specs/2026-09-12-aac-lc-framing-design.md
  - src/dump.rs
  - src/encoder.rs
  - src/obu/codec_config.rs
  - src/obu/mod.rs
  - tests/conformance.rs
  - tests/descriptors.rs
  - tests/encoder_builder.rs
  - tests/encoder_streaming.rs
  - tests/fixtures/MANIFEST.md
  - tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md
  - tests/fixtures/reference/test_000076_aac_lc.iamf
  - tests/parse_reference.rs
  - tests/public_api.rs
  - tests/reference_manifest.rs
  - tests/round_trip.rs
  - tests/support/reference_expectations.rs
  - tools/build-reference.sh
  - .planning/PROJECT.md
  - HANDOFF.md
  - .planning/STATE.md
autonomous: true
requirements: [API-06, API-09, CODEC-07]

estimate:
  tokens: 90000
  raw_tokens: 90000
  tasks: 3
  confidence: low

must_haves:
  truths:
    - "main's tip is a two-parent merge commit combining 44221fe (main) and 2674f1e (origin/feature/aac-lc-framing), created with git merge --no-ff (not squashed, not rebased)"
    - "Exactly the 4 files CONTEXT.md predicted needed hand-resolution — src/encoder.rs, tests/encoder_builder.rs, tests/encoder_streaming.rs, tests/round_trip.rs — carry a real conflict resolution; no other file needed manual intervention beyond git's own auto-merge, and no <<<<<<< / >>>>>>> marker remains anywhere in the tree"
    - "src/encoder.rs's obu import list carries both HeadphonesRenderingMode (main-only) and CODEC_ID_AAC (branch-only); the FrameCodecMismatch match in TemporalUnitInput validation is exhaustive over all 4x4 DecoderConfig/FrameInput pairings including AacLc"
    - "tests/encoder_streaming.rs's renamed lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind test keeps main's (result, trimming) tuple shape and Opus ref: comment, adds AAC-LC as a 5th case with trimming = None, and its expected_payload match is exhaustive over FrameInput::AacLc too; the mono_builder (input, rate) match adds DecoderConfig::AacLc(_) => (FrameInput::AacLc(vec![0x21, 0x10, 0x04]), 48_000), verified against the only two AAC-LC construction call sites in that file, both CodecConfig::aac_lc(0, 48_000)"
    - "cargo build --locked --all-targets, cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings and the same clippy command with --features fuzzing are all green on the merge commit itself"
    - "cargo test --locked, cargo test --locked --features fuzzing --test fuzz_regression, bash tools/prove-guards.sh (9 PASS), bash tools/check-float-escape-census.sh (exactly 1 hit), cargo tree -e normal,no-proc-macro (iamf + thiserror only), tests/fixtures_cap.rs and tests/citations.rs are all green on the final tree"
    - "No semantic_sha256 for any fixture that existed before the merge changed in tests/support/reference_expectations.rs; git diff --numstat 44221fe..HEAD for that file shows 4 insertions and 0 deletions; the golden test is unchanged and green"
    - "src/error.rs is byte-identical to its pre-merge main content, and no reference to origin/wip/delivery-validator-conformance was added anywhere"
    - "REFERENCES.md's contamination-boundary section documents that the reference tier optionally links libiamf's own bundled FDK-AAC archive (IAMF_REFERENCE_ENABLE_AAC=1) to run the AAC-LC conformance gate, without adding FDK-AAC as a licence-table row or a project dependency"
    - ".planning/PROJECT.md's Out-of-Scope AAC-LC entry and HANDOFF.md's 'three codecs' + out-of-scope lines both describe framing (implemented, matching the FLAC/Opus precedent) versus decode (still out of scope, iamf-decode-rs's job), worded consistently with each other"
    - ".planning/STATE.md's Quick Tasks Completed table gained a 260914-pyd row and Session Continuity was updated, but STATE.md itself was never staged or committed by the executor"
  artifacts:
    - path: "src/encoder.rs"
      provides: "merged obu import list (HeadphonesRenderingMode + CODEC_ID_AAC) and the exhaustive DecoderConfig/FrameInput FrameCodecMismatch match including AacLc"
      contains: "CODEC_ID_AAC"
    - path: "tests/encoder_streaming.rs"
      provides: "lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind and mono_builder's AacLc rate-48_000 arm"
      contains: "DecoderConfig::AacLc(_) => (FrameInput::AacLc(vec![0x21, 0x10, 0x04]), 48_000)"
    - path: "tests/encoder_builder.rs"
      provides: "union import list (main's full set plus DecoderConfig)"
      contains: "DecoderConfig"
    - path: "tests/round_trip.rs"
      provides: "branch's superset import list (CodecConfig, DecoderConfig, ParameterData added) plus typed_aac_lc_codec_configs_round_trip"
      contains: "typed_aac_lc_codec_configs_round_trip"
    - path: "REFERENCES.md"
      provides: "one-line note on the opt-in bundled FDK-AAC reference-tier archive"
      contains: "IAMF_REFERENCE_ENABLE_AAC"
    - path: ".planning/PROJECT.md"
      provides: "reworded AAC-LC Out-of-Scope entry: framing implemented, decode still out of scope"
      contains: "AAC-LC"
    - path: "HANDOFF.md"
      provides: "four-codec 'shipped' line and matching out-of-scope line, both framing-only"
      contains: "AAC-LC"
  key_links:
    - from: "tests/encoder_streaming.rs mono_builder"
      to: "iamf::obu::DecoderConfig::AacLc"
      via: "the (input, rate) match arm pairing AacLc with FrameInput::AacLc and rate 48_000"
      pattern: "DecoderConfig::AacLc\\(_\\) => \\(FrameInput::AacLc"
    - from: "src/encoder.rs FrameCodecMismatch validation"
      to: "iamf::obu::DecoderConfig::AacLc / encoder::FrameInput::AacLc"
      via: "the exhaustive match arm pairing them as Ok(()), and every cross-codec pairing as FrameCodecMismatch"
      pattern: "DecoderConfig::AacLc\\(_\\), FrameInput::AacLc\\(_\\)"
    - from: "REFERENCES.md contamination-boundary section"
      to: "tools/build-reference.sh IAMF_REFERENCE_ENABLE_AAC toggle"
      via: "a one-line note, not a new licence-table row"
      pattern: "IAMF_REFERENCE_ENABLE_AAC"
    - from: "HANDOFF.md 'three codecs shipped' line"
      to: ".planning/PROJECT.md Out-of-Scope AAC-LC entry"
      via: "consistent framing-implemented / decode-out-of-scope wording"
      pattern: "AAC-LC"
---

<objective>
Merge `origin/feature/aac-lc-framing` (tip `2674f1e`, forked from `a9cf271`) into `main` (`44221fe`) with
`git merge --no-ff`, producing one real merge commit — not a rebase, not a squash. The branch adds AAC-LC
Audio Frame framing as a fourth pre-encoded codec alongside LPCM/FLAC/Opus: this crate frames pre-encoded
`raw_data_block()` access units, it never encodes or decodes AAC. This is the user's own framing (2026-09-14):
"ja mergen würde ich sagen, ist doch ein normales feature" — ordinary feature work, already built and tested
on the branch; this task's job is a clean, verified integration, not new feature design.

`260914-pyd-CONTEXT.md` already contains the orchestrator's own conflict-by-conflict investigation from a
disposable trial merge. This plan independently re-verified every one of its claims with a read-only
`git merge-tree` (no working-tree mutation) before writing this plan: **all four predicted conflicts are
exactly right, nothing else conflicts, and the AAC-LC sample rate CONTEXT.md flagged as "find it, likely
48 kHz" is confirmed 48_000 with certainty** — it is the only rate `CodecConfig::aac_lc(...)` is ever
constructed with anywhere in `tests/encoder_streaming.rs`, on both sides of the merge. Nothing in the
conflict guidance turned out to be wrong; this plan simply removes remaining hedges where verification made
them unnecessary.

Explicitly out of scope: `origin/wip/delivery-validator-conformance` (`79dafb1`) — do not touch, merge,
cherry-pick or reference it. `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` describes that separate, unreviewed work,
not anything on `main` or `feature/aac-lc-framing`.

Output: one merge commit (Task 1), verification only with no new commit unless Task 2 finds something Task 1
should have caught — in which case that fix is folded into Task 1's commit via `git commit --amend`, never a
separate fix-up commit — and one `docs(...)` commit (Task 3).
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@.claude/CLAUDE.md
@.planning/quick/260914-pyd-merge-feature-aac-lc-framing-into-main/260914-pyd-CONTEXT.md

Working directory: `/Volumes/lab/talea/iamf-rs`. Base commit: `44221fe` (main). Merge target:
`origin/feature/aac-lc-framing` at `2674f1e`. Work sequentially on `main` in the primary checkout — no
worktree, no isolation. macOS has no `timeout` binary; toolchain is 1.85.0 exactly, no let-chains.

Never stage any of these: untracked `.DS_Store`, `docs/.DS_Store`, `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md`,
`docs/eclipsa-audio-plugin/`, `docs/iamf-tests/`, `docs/iamf-tools/`, `docs/iamf/`, `docs/oar/`,
`docs/superpowers/` (the pre-existing untracked scratch content, as opposed to the two files the branch
itself tracks — see Task 1), `iamf-rs.code-workspace`, `.planning/STATE.md`, and anything in this quick-task
directory (`260914-pyd-CONTEXT.md`/`-PLAN.md`/`-SUMMARY.md`). The branch's own tracked additions
`docs/superpowers/plans/2026-09-12-aac-lc-framing.md` and `docs/superpowers/specs/2026-09-12-aac-lc-framing-design.md`
arrive as part of the merge itself and are fine — they are not "hand-staged scratch," they are tracked files
the merge naturally stages.

Never use `git add -A`, `git add .` or `git commit -a`. Stage explicit paths only. Run
`git diff --cached --name-only` before every commit and confirm it matches intent exactly. End every commit
message with:
Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SzEAjcfJGG7KBZ1WhoN3ue

Do not push. Do not touch `.planning/ROADMAP.md`. Do not touch `src/error.rs` — nothing in this merge needs a
new `ErrorKind` variant (verified: the branch's diff against `src/error.rs` is empty). Do not resolve,
reference or build on `origin/wip/delivery-validator-conformance`.

Independently re-verified facts this session (do not re-derive, trust these):
- `git diff --stat main...origin/feature/aac-lc-framing -- Cargo.toml Cargo.lock fuzz/ tools/codec-fixtures/`
  is empty — the branch adds no dependency anywhere, in any workspace. No package-legitimacy checkpoint is
  needed.
- `git diff --stat main...origin/feature/aac-lc-framing -- .planning/ HANDOFF.md REFERENCES.md CONFORMANCE-GATE.md`
  is empty — the branch never touched any of those, confirming Task 3's docs work is pure addition, not a
  merge of competing edits.
- A read-only `git merge-tree` (base `a9cf271`, ours `main`, theirs `origin/feature/aac-lc-framing`) shows
  `CONFLICT (content)` in exactly `src/encoder.rs`, `tests/encoder_builder.rs`, `tests/encoder_streaming.rs`,
  `tests/round_trip.rs`, and clean auto-merges everywhere else the two sides both touched (`.github/workflows/reference.yml`,
  `README.md`, `src/obu/mod.rs`, `tests/conformance.rs`, `tests/parse_reference.rs`, `tests/public_api.rs`,
  `tests/reference_manifest.rs`, `tests/fixtures/MANIFEST.md`, `tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md`).
- `tests/fixtures_cap.rs`'s cap and count assertions read dynamically from `tests/fixtures/MANIFEST.md`'s own
  stated numbers (`stated_cap`, `stated_iamf_count` helpers) rather than hardcoding them — the branch's
  MANIFEST.md update (77→78 files, 39→40 `.iamf`) is self-consistent, so no manual cap edit is expected;
  Task 2 still runs the test to confirm rather than assuming.
- `git diff main...origin/feature/aac-lc-framing -- src/obu/codec_config.rs` shows `read_aac_lc_decoder_config`
  and `write_aac_lc_decoder_config` both already carry a `// ref:` citation immediately above them, so
  `tests/citations.rs` is expected to pass unmodified.

Project constraints: `unsafe_code = "forbid"`; in `src/`, no `unwrap`/`expect`/`panic`, no `[]` indexing, no
bare arithmetic, no `HashMap`/floats, no new `#[allow]` without `reason = "..."`. The linked graph stays
`iamf` + `thiserror` (`cargo tree -e normal,no-proc-macro`).
</context>

<tasks>

<task type="auto">
  <name>Task 1: Merge origin/feature/aac-lc-framing into main, resolve the 4 conflicts, document the reference-tier AAC toggle</name>
  <files>src/encoder.rs, tests/encoder_builder.rs, tests/encoder_streaming.rs, tests/round_trip.rs, REFERENCES.md</files>
  <precondition>`git rev-parse --short HEAD` prints `44221fe` and `git status --porcelain -- src tests REFERENCES.md` is empty</precondition>
  <action>
    Step 1 — fetch fresh, do not trust any existing local ref. Run `git fetch origin feature/aac-lc-framing`.
    Confirm `git rev-parse --short origin/feature/aac-lc-framing` prints `2674f1e`. If it prints anything else,
    stop and report — the branch moved since this plan was written and the conflict guidance below may no
    longer be exhaustive.

    Step 2 — start the merge. Run `git merge --no-ff origin/feature/aac-lc-framing`. Git will stop with
    exactly 4 conflicting files (everything else auto-merges): `src/encoder.rs`, `tests/encoder_builder.rs`,
    `tests/encoder_streaming.rs`, `tests/round_trip.rs`. If a 5th file, or a different file, shows a conflict,
    stop and report the actual conflict rather than improvising a resolution — this plan's guidance below is
    keyed to exactly these 4.

    Step 3 — resolve `src/encoder.rs`. The only conflict is the `use crate::obu::{...}` brace list near the
    top of the file. Union both sides: the merged brace list must contain every identifier from main's side
    (`AudioElement, AudioElementType, AudioFrame, CODEC_ID_FLAC, CODEC_ID_LPCM, CODEC_ID_OPUS, CodecConfig,
    DecoderConfig, HeadphonesRenderingMode, IaSequenceHeader, MixGainParamDefinition, MixPresentation, Obu,
    ObuHeader, ObuType, ParamDefinitionRegistry, ParameterBlock, Trimming`) plus the one identifier only the
    branch's side has, `CODEC_ID_AAC`. Remove the conflict markers, leave the items in any order inside the
    braces — `cargo fmt --all` in Step 7 re-sorts them. Nothing else in this file conflicts: main's
    `TemporalProgress`/cross-unit-trim work and the branch's `FrameInput::AacLc` / `DecoderConfig::AacLc`
    additions (including the exhaustive `FrameCodecMismatch` match pairing every `DecoderConfig`/`FrameInput`
    combination, with `(DecoderConfig::AacLc(_), FrameInput::AacLc(_)) => Ok(())` and every AAC-LC-vs-other-codec
    pairing routed to `FrameCodecMismatch`) auto-merge cleanly — verify this by reading the file after
    resolving rather than assuming, since it decides whether the match stays exhaustive.

    Step 4 — resolve `tests/round_trip.rs`. Same shape: a `use iamf::obu::{...}` brace list conflict. The
    branch's side (`AudioElementParam, CodecConfig, DecoderConfig, Obu, ObuHeader, ObuType, ParameterData,
    find_obu_boundaries, read_codec_config, read_obu_with, write_codec_config, write_obu, write_obu_with`) is
    a strict superset of main's side — take it as-is (or union manually; the result is identical either way).
    Remove the conflict markers. Nothing else in this file conflicts; the new
    `typed_aac_lc_codec_configs_round_trip` test that follows arrived from the branch's side with no overlap.

    Step 5 — resolve `tests/encoder_builder.rs`. Same shape again: a `use iamf::obu::{...}` brace list
    conflict. Union both sides: main's side has `AnchorElement, AnchoredLoudness, AudioElement,
    AudioElementParam, ChannelAudioLayerConfig, CodecConfig, HeadphonesRenderingMode, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixPresentation, RenderingConfig,
    SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement` (today's kfs/m62 test helper
    additions); the branch's side adds only `DecoderConfig` beyond a subset of that same list. The merged
    list is main's full list plus `DecoderConfig`. Remove the conflict markers; `cargo fmt --all` normalizes
    order. The new test below the import (`build_rejects_aac_buffer_size_db_that_exceeds_its_24_bit_wire_field`)
    is a clean, non-conflicting branch addition.

    Step 6 — resolve `tests/encoder_streaming.rs`. Two real body conflicts, not import-only:
    - First hunk: main's `lpcm_flac_and_opus_inputs_follow_the_frozen_codec_kind` (today's Opus `pre_skip`
      start-trim work) iterates `for (result, trimming) in [...]` over 3 cases — LPCM (`None`), FLAC (`None`),
      Opus (`Some(start_trim(312))`) — with the ref comment `// ref: IAMF v1.1.0 index.bs:1819 (an Opus stream
      trims exactly pre_skip samples at its start)` directly above the loop. The branch's version, renamed
      `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`, iterates a flat `for result in [...]`
      with no `trimming` tuple, adding `mono_builder(CodecConfig::aac_lc(0, 48_000)?)` as a 4th case.
      Resolution: keep main's `(result, trimming)` tuple shape, the ref comment above the loop, and the
      function body below (which destructures `trimming` and passes it into `TemporalUnitInput`). Rename the
      function to `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`. Add a 5th tuple entry after
      the Opus one: `(mono_builder(CodecConfig::aac_lc(0, 48_000)?), None)` — AAC-LC has no encoder-derived
      start trim in this crate, so `trimming` is `None` here exactly like LPCM/FLAC.
      Immediately below the closing `]` of that array, the `let expected_payload = match &frame.1 { ... }`
      arm needs a 4th pattern for `FrameInput::AacLc(payload)`. Verified this session: in the real 3-way diff
      this specific hunk (the match arm, not the array) carries no conflict markers at all — it auto-merges
      cleanly to `FrameInput::Lpcm(payload) | FrameInput::Flac(payload) | FrameInput::Opus(payload) |
      FrameInput::AacLc(payload) => payload.clone(),`. Confirm this by reading the file after resolving the
      array conflict; do not hand-write it unless it is genuinely still missing.
    - Second hunk, inside `mono_builder`: main computes `let (input, rate) = match config.decoder_config {
      ... }`, where `rate` feeds `sub_mix.output_mix_gain.definition.parameter_rate` (today's
      `ParameterRateMismatch` enforcement) — `iamf::obu::DecoderConfig::Lpcm(_) => (FrameInput::Lpcm(vec![0;
      256]), 16_000)`, `Flac(_) => (FrameInput::Flac(vec![0]), 16_000)`, `Opus(_) => (FrameInput::Opus(vec![0xf8]),
      48_000)`, then `Raw { .. } | _ => unreachable!()`. The branch's side drops the `rate` half of the tuple
      and adds `DecoderConfig::AacLc(_) => FrameInput::AacLc(vec![0x21, 0x10, 0x04])`. Resolution: keep main's
      `(input, rate)` tuple shape for every arm, and add
      `iamf::obu::DecoderConfig::AacLc(_) => (FrameInput::AacLc(vec![0x21, 0x10, 0x04]), 48_000)` as a new arm
      before the `Raw { .. } | _ => unreachable!()` arm, which stays last. The rate is confirmed 48_000, not a
      guess: it is the only sample rate `CodecConfig::aac_lc(...)` is constructed with anywhere in this file,
      on both sides of the merge (both call sites read `CodecConfig::aac_lc(0, 48_000)`) — grep the resolved
      file for `aac_lc(` to confirm exactly 2 call sites and both pass `48_000` before finalizing this arm.

    Step 7 — run `cargo fmt --all`, then `cargo build --locked --all-targets`, then
    `cargo clippy --locked --all-targets -- -D warnings`, then
    `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`. Fix anything that does not
    compile before proceeding — a merge commit must build and lint clean on its own.

    Step 8 — reread the actual `git diff main -- tools/build-reference.sh` (already staged clean by the
    merge). Confirm it only toggles a prebuilt FDK-AAC archive that ships inside the pinned `libiamf@v1.1.0`
    checkout, gated by the opt-in env var `IAMF_REFERENCE_ENABLE_AAC=1` (now set unconditionally in
    `.github/workflows/reference.yml`'s "Build the pinned reference decoder" step); it adds no build
    dependency to this repo and touches neither `Cargo.toml` nor `Cargo.lock`. This matches the existing
    FLAC/Opus reference pattern (prebuilt archives already vendored inside `libiamf`, run only). Then edit
    `REFERENCES.md`'s `## Reading versus invoking — the contamination boundary (D-15, GUARD-08)` section
    (currently ends with the FFmpeg-trap paragraph and the `NOTICE`/`PATENTS` attribution line): add one short
    paragraph noting that the reference tier optionally links `libiamf`'s own bundled FDK-AAC archive
    (`IAMF_REFERENCE_ENABLE_AAC=1`) to run the AAC-LC conformance gate — this is not a new project dependency
    and does not get a licence-table row, matching how the existing Opus/FLAC archives are treated. This
    section does not currently mention FDK-AAC at all (verified this session), so this is a pure addition, not
    a rewrite.

    Step 9 — stage and commit. `git add src/encoder.rs tests/encoder_builder.rs tests/encoder_streaming.rs
    tests/round_trip.rs REFERENCES.md`. Everything else the merge touched (the ~16 cleanly auto-merged files
    plus the new fixture) is already staged by `git merge` itself. Run `git status --porcelain` and confirm
    every entry is either one of these 5 explicitly-added files or a file from the known diffstat
    (`.github/workflows/reference.yml`, `README.md`, `docs/superpowers/plans/2026-09-12-aac-lc-framing.md`,
    `docs/superpowers/specs/2026-09-12-aac-lc-framing-design.md`, `src/dump.rs`, `src/obu/codec_config.rs`,
    `src/obu/mod.rs`, `tests/conformance.rs`, `tests/descriptors.rs`, `tests/fixtures/MANIFEST.md`,
    `tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md`, `tests/fixtures/reference/test_000076_aac_lc.iamf`,
    `tests/parse_reference.rs`, `tests/public_api.rs`, `tests/reference_manifest.rs`,
    `tests/support/reference_expectations.rs`, `tools/build-reference.sh`) — nothing extraneous like
    `.DS_Store` or `docs/superpowers/specs/2026-09-13-...`. Commit with a message whose first line is
    `Merge remote-tracking branch 'origin/feature/aac-lc-framing' into main`, a second paragraph summarising
    what the branch adds (AAC-LC Audio Frame framing as a fourth pre-encoded codec, framing-only, matching the
    existing FLAC/Opus boundary; conflicts resolved in `src/encoder.rs`, `tests/encoder_builder.rs`,
    `tests/encoder_streaming.rs`, `tests/round_trip.rs`), then the two attribution lines from the context
    block. Do not use `git commit -a` or stage with a wildcard — the paths above are exhaustive and explicit.
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && [ "$(git rev-parse --short origin/feature/aac-lc-framing)" = "2674f1e" ] && git rev-parse -q --verify HEAD^2 >/dev/null && git diff --quiet 44221fe HEAD -- src/error.rs && MARKERS=$(git grep -n -e '^<<<<<<< ' -e '^>>>>>>> ' -- '*.rs' '*.md' '*.sh' '*.yml' 2>/dev/null); MARKERS_STATUS=$?; [ "$MARKERS_STATUS" -le 1 ] && [ -z "$MARKERS" ] && grep -q 'CODEC_ID_AAC' src/encoder.rs && grep -q 'HeadphonesRenderingMode' src/encoder.rs && grep -c 'DecoderConfig::AacLc(_)' src/encoder.rs | grep -qv '^0$' && grep -q 'lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind' tests/encoder_streaming.rs && grep -q 'DecoderConfig::AacLc(_) => (FrameInput::AacLc(vec!\[0x21, 0x10, 0x04\]), 48_000)' tests/encoder_streaming.rs && grep -q 'IAMF_REFERENCE_ENABLE_AAC' REFERENCES.md && cargo build --locked --all-targets && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && test -z "$(git status --porcelain)"</automated>
  </verify>
  <done>A single two-parent merge commit is HEAD on main. The 4 predicted files are hand-resolved exactly as specified, no conflict markers remain anywhere, src/error.rs is untouched, REFERENCES.md documents the opt-in FDK-AAC reference-tier link, and the tree builds/fmt-checks/clippy-lints clean both with and without the fuzzing feature. The working tree is clean.</done>
</task>

<task type="auto">
  <name>Task 2: Full verification gate — tests, fuzz replay, guards, float census, dependency graph, fixture cap, citations</name>
  <files>(none — verification only; amend Task 1's commit only if something it should have caught fails here)</files>
  <precondition>Task 1's merge commit is HEAD (`git rev-parse -q --verify HEAD^2` succeeds, proving a second parent exists) and `git status --porcelain` is empty</precondition>
  <action>
    Run, in order, from the repository root:
    1. `cargo test --locked` — the full offline suite, including `tests/round_trip.rs`,
       `tests/encoder_builder.rs`, `tests/encoder_streaming.rs`, `tests/conformance.rs`,
       `tests/parse_reference.rs`, `tests/public_api.rs`, `tests/reference_manifest.rs`,
       `tests/fixtures_cap.rs`, `tests/citations.rs` and `tests/golden.rs`.
    2. `cargo test --locked --features fuzzing --test fuzz_regression` — replays the committed fuzz corpus;
       this crate's own fuzz seeds are untouched by the merge, so this should be a no-op pass.
    3. `bash tools/prove-guards.sh` — expect 9 distinct PASS cases (the script prints its list twice).
    4. `bash tools/check-float-escape-census.sh` — expect exactly 1 hit (`lufs_to_q7_8` in
       `src/model/loudness.rs`); the merge touches no float code, so this must be unchanged from pre-merge
       main.
    5. `cargo tree -e normal,no-proc-macro` — expect only `iamf` and `thiserror`; confirmed pre-merge that the
       branch adds no dependency in any workspace, so this is a pure regression check.
    6. `git diff --numstat 44221fe HEAD -- tests/support/reference_expectations.rs` — expect exactly `4    0`
       (4 insertions, 0 deletions): the branch's new AAC-LC fixture entries are additions, no pre-existing
       `semantic_sha256` line changed. If any deletion count is non-zero, stop and report — that means an
       existing fixture's expected hash moved, which this task must never paper over.
    7. `cargo test --locked --test golden` — must pass unmodified; the merge changes no writer code path this
       fixture exercises.

    If any of these fails in a way that the merge commit itself should have caught — a missing match arm, a
    stale import, a formatting drift, a genuinely wrong conflict resolution from Task 1 — fix it directly in
    the working tree, stage the same explicit paths Task 1 used (plus whatever new path the fix touches), and
    fold the fix into Task 1's commit with `git commit --amend`. Do not create a separate fix-up commit for a
    conflict-resolution mistake. Only a genuinely new issue this merge did not create (unrelated flake, a
    pre-existing environmental gap) gets its own commit — and only after confirming it is not, in fact, this
    merge's fault.

    If everything passes on the first run, this task produces no commit at all — it is pure verification.
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && cargo test --locked && cargo test --locked --features fuzzing --test fuzz_regression && [ "$(bash tools/prove-guards.sh 2>&1 | grep -c PASS)" -ge 9 ] && bash tools/check-float-escape-census.sh && [ "$(cargo tree -e normal,no-proc-macro --prefix none | sed 's/ .*//' | sort -u | tr '\n' ' ')" = "iamf thiserror " ] && NUMSTAT=$(git diff --numstat 44221fe HEAD -- tests/support/reference_expectations.rs); NUMSTAT_STATUS=$?; [ "$NUMSTAT_STATUS" -eq 0 ] && printf '%s\n' "$NUMSTAT" | awk '{ if ($1 != 4 || $2 != 0) { exit 1 } n++ } END { if (n != 1) exit 1 }' && cargo test --locked --test golden && cargo test --locked --test fixtures_cap && cargo test --locked --test citations && test -z "$(git status --porcelain)"</automated>
  </verify>
  <done>Every gate in the constraints list is green on the merged tree: fmt/build/clippy (already proven in Task 1), full offline `cargo test`, fuzz replay, 9 prove-guards PASS, exactly 1 float-census hit, `cargo tree` is exactly `iamf` + `thiserror`, `reference_expectations.rs` gained exactly 4 lines and lost none, golden/fixtures_cap/citations all pass. Either no amendment was needed, or a genuine Task-1-scope mistake was folded into Task 1's commit via `--amend` and re-verified green. The working tree is clean.</done>
</task>

<task type="auto">
  <name>Task 3: Update scope-boundary docs, record the quick task in STATE.md (not committed), re-run the full gate list</name>
  <files>.planning/PROJECT.md, HANDOFF.md, .planning/STATE.md</files>
  <precondition>Task 2's verification passed (with or without a Task-1 amendment already landed) and `git status --porcelain` is empty</precondition>
  <action>
    (1) `.planning/PROJECT.md`, `### Out of Scope` section, the AAC-LC bullet (currently: "**AAC-LC codec
    implementation** — belongs to `iamf-decode-rs`, not this wire crate. Its decoder configuration syntax may
    be added here when that consumer requires it. *Correction 2026-09-08:* `iamf-tools` `main` now ships an
    AAC-LC encoder, so the handoff's 'three codecs shipped, not four' describes the v1.x tree, not HEAD. Still
    out of scope for v1 here; revisit if a decoder needs it."). Reword it to state what is now true after this
    merge: AAC-LC Audio Frame *framing* — the typed decoder-config wire syntax and accepting pre-encoded
    `raw_data_block()` access units — is implemented, exactly matching the pattern already carved out for
    FLAC/Opus (this crate frames, it never encodes or decodes). AAC-LC *decode* remains out of scope,
    `iamf-decode-rs`'s job. Do not claim more than the branch actually did — no encoder, no decoder, no
    resampler was added. Keep the existing correction/history sentences if they remain accurate, drop or
    amend only what the merge supersedes.
    Check `## Key Decisions`' own convention (a table of foundational scope/licence/architecture decisions,
    all dated 2026-09-07/08, each with a Rationale and an Outcome column) before deciding whether this routine
    feature merge warrants a new row — if it does not fit that table's granularity, say so in the SUMMARY
    rather than forcing a row in.

    (2) `HANDOFF.md`. Two places currently say "three codecs, not four":
    - Line ~270, `### 4.1` numbered list item 3: "**Three codecs shipped, not four:** LPCM, FLAC, Opus. **No
      AAC-LC**, though the spec lists it. A decoder needs all four; an encoder evidently does not." Update to
      state four codecs are now framed by this crate (LPCM, FLAC, Opus, AAC-LC), preserving the same
      framing-only boundary already documented nearby for FLAC/Opus — this crate frames pre-encoded
      `raw_data_block()` access units, it never encodes or decodes AAC-LC.
    - Line ~306, `## 5. Scope: in and out`, the **In** list: "Codec framing for LPCM, FLAC, Opus" — extend to
      include AAC-LC, for consistency with the item above (otherwise this list still describes three codecs
      while the paragraph above it describes four).
    - Line ~314, the **Out** list: "**AAC-LC codec implementation** — belongs to `iamf-decode-rs`; only
      required wire syntax may be added here when that consumer needs it." Reword the same way as PROJECT.md's
      entry: framing wire syntax is now implemented; decode remains out of scope for `iamf-decode-rs`.
    Follow the existing wording pattern used nearby for FLAC/Opus rather than inventing new phrasing. Do not
    touch `CONFORMANCE-GATE.md`, `DIFF-LEDGER.md`, `REQUIREMENTS.md`, or the portfolio boundaries document at
    `/Users/cell/local/Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md`
    — this is a framing-syntax addition within the crate's existing boundary, the same shape as FLAC/Opus, not
    an ownership move or a new cross-library object, so the portfolio document's "Change control" trigger does
    not apply.

    Commit by explicit path: `git add .planning/PROJECT.md HANDOFF.md`. Confirm `git diff --cached --name-only`
    lists exactly those two files. Message: `docs(quick-260914-pyd): describe AAC-LC framing in the project
    and consumer scope boundaries`, body noting the framing-implemented/decode-still-out-of-scope distinction
    and that CONFORMANCE-GATE.md/DIFF-LEDGER.md/REQUIREMENTS.md/the portfolio document are untouched, ending
    with the session's attribution lines.

    (3) `.planning/STATE.md` — edit but do NOT stage or commit it; the orchestrator commits it separately, per
    this project's day-long pattern (every quick task today left STATE.md for the orchestrator). Add a row to
    the `### Quick Tasks Completed` table: `| 260914-pyd | merge feature/aac-lc-framing into main | 2026-09-14
    | <merge-commit-sha> | [260914-pyd-merge-feature-aac-lc-framing-into-main](./quick/260914-pyd-merge-feature-aac-lc-framing-into-main/) |`
    using the real short SHA of Task 1's merge commit (amended, if it was amended). Update `## Session
    Continuity`: `Last session` stays `2026-09-14`; `Stopped at` should read something like "Completed quick
    task 260914-pyd (merge commit `<sha>`, docs commit `<sha>`; not pushed)."; adjust `Next:` to whatever the
    prior session's next-step note plus this task's completion implies (do not delete the pre-existing
    260914-m62 follow-up notes if they are still open — append, don't overwrite, unless this task resolved
    them, which it does not).

    (4) Re-run the full gate list once more on the final tree, to confirm the docs commit did not break
    anything (it should not — docs only — but confirm rather than assume): `cargo fmt --check`,
    `cargo build --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`,
    `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`, `cargo test --locked`,
    `cargo test --locked --features fuzzing --test fuzz_regression`, `bash tools/prove-guards.sh` (9 PASS),
    `bash tools/check-float-escape-census.sh` (1 hit), `cargo tree -e normal,no-proc-macro` (iamf + thiserror
    only).
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && grep -q 'AAC-LC' .planning/PROJECT.md && grep -q 'AAC-LC' HANDOFF.md && grep -q '260914-pyd' .planning/STATE.md && git diff --quiet 44221fe HEAD -- CONFORMANCE-GATE.md DIFF-LEDGER.md .planning/REQUIREMENTS.md .planning/ROADMAP.md && STAGED=$(git diff --cached --name-only); STAGED_STATUS=$?; [ "$STAGED_STATUS" -eq 0 ] && [ "$(printf '%s\n' "$STAGED" | sort)" = "$(printf '.planning/PROJECT.md\nHANDOFF.md\n' | sort)" ] && test -z "$(git status --porcelain -- src tests REFERENCES.md .planning/PROJECT.md HANDOFF.md)" && cargo fmt --check && cargo build --locked --all-targets && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && cargo test --locked && cargo test --locked --features fuzzing --test fuzz_regression && [ "$(bash tools/prove-guards.sh 2>&1 | grep -c PASS)" -ge 9 ] && bash tools/check-float-escape-census.sh && [ "$(cargo tree -e normal,no-proc-macro --prefix none | sed 's/ .*//' | sort -u | tr '\n' ' ')" = "iamf thiserror " ]</automated>
  </verify>
  <done>PROJECT.md and HANDOFF.md both describe AAC-LC framing as implemented and AAC-LC decode as still out of scope, in wording consistent with each other and with the existing FLAC/Opus precedent. CONFORMANCE-GATE.md, DIFF-LEDGER.md, REQUIREMENTS.md, ROADMAP.md and the portfolio document are untouched. Exactly one docs commit exists for this step. STATE.md carries the new row and updated Session Continuity but was never staged. The full gate list is green one more time on the final tree.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| untrusted `.iamf` bytes → `read_aac_lc_decoder_config` / Audio Frame reader | attacker-controlled AAC-LC `DecoderConfigDescriptor` and `raw_data_block()` bytes reach the typed parser this merge brings in |
| Parallax/caller input → `EncoderBuilder::build` + `FrameInput::AacLc` | a caller-supplied AAC-LC access unit and its declared codec config decide whether `push_temporal_unit` accepts or rejects it |
| merge integration boundary → shared test helpers (`mono_builder`, the `FrameCodecMismatch` match) | a wrong rate/codec pairing introduced during conflict resolution would silently mask a real `FrameCodecMismatch` regression rather than catching one |
| reference-tier build script → CI | `IAMF_REFERENCE_ENABLE_AAC=1` changes what the Linux reference job links, but only inside the excluded reference tier, never the shipping crate |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-pyd-01 | Tampering | `read_aac_lc_decoder_config` / `write_aac_lc_decoder_config` (`src/obu/codec_config.rs`) | medium | mitigate | Already implemented and tested on the source branch with checked arithmetic and no indexing/unwrap/panic, per the project-wide clippy denies that apply uniformly to both sides of the merge; `tests/citations.rs` (verified passing, `// ref:` already present above both functions) and Task 2's full offline suite re-run them unchanged |
| T-pyd-02 | Denial of Service | untrusted AAC-LC `buffer_size_db` / bitrate 24-bit wire fields | low | mitigate | `tests/encoder_builder.rs::build_rejects_aac_buffer_size_db_that_exceeds_its_24_bit_wire_field` (a clean, non-conflicting branch addition) proves the 24-bit ceiling is enforced by `build()` before any byte is written; Task 2 runs it |
| T-pyd-03 | Tampering | merge conflict resolution silently dropping a `DecoderConfig`/`FrameInput` match arm | high | mitigate | Task 1's `<verify>` greps for the exhaustive AAC-LC pairing in `src/encoder.rs` and for zero leftover `<<<<<<<`/`>>>>>>>` markers repo-wide; `-D warnings` makes a non-exhaustive match a hard compile error, not a silent gap |
| T-pyd-04 | Tampering | reference-tier FDK-AAC toggle silently becoming a shipped dependency | low | mitigate | Task 2 asserts `cargo tree -e normal,no-proc-macro` is exactly `iamf` + `thiserror` and that `Cargo.toml`/`Cargo.lock` carry no diff from pre-merge main (confirmed empty this session); `IAMF_REFERENCE_ENABLE_AAC` only affects the excluded Linux reference-tier build script |
| T-pyd-05 | Repudiation | scope-boundary docs (`PROJECT.md`/`HANDOFF.md`) silently drifting from what the merged code now does | medium | mitigate | Task 3 updates both with matching framing-implemented/decode-out-of-scope wording; `CONFORMANCE-GATE.md`, `DIFF-LEDGER.md`, `REQUIREMENTS.md` and `ROADMAP.md` are asserted untouched since the branch and this task both leave them alone |
| T-pyd-06 | Tampering | supply chain via the merge itself | low | accept | The branch adds zero dependencies in any workspace (`Cargo.toml`/`Cargo.lock`/`fuzz/`/`tools/codec-fixtures/` all diff-empty against main, verified this session); no package-legitimacy checkpoint applies |
</threat_model>

<verification>
- `git merge-tree` (read-only, pre-verified this session) confirms exactly 4 conflicting files and clean auto-merge everywhere else the two sides both touched.
- Task 1: merge commit exists, 2 parents, the 4 conflicts resolved exactly as specified, no markers remain, `src/error.rs` untouched, build/fmt/clippy (both feature sets) green.
- Task 2: full offline test suite, fuzz replay, 9 prove-guards PASS, 1 float-census hit, `cargo tree` exactly `iamf thiserror`, `reference_expectations.rs` gained exactly 4 lines with 0 deletions, golden/fixtures_cap/citations all green.
- Task 3: `PROJECT.md`/`HANDOFF.md` updated consistently; `CONFORMANCE-GATE.md`/`DIFF-LEDGER.md`/`REQUIREMENTS.md`/`ROADMAP.md` untouched; STATE.md updated but not staged; the full gate list re-run green one final time.
</verification>

<success_criteria>
- `main` HEAD is a real `git merge --no-ff` merge commit of `origin/feature/aac-lc-framing` (`2674f1e`) into `44221fe`, not a rebase or squash.
- Exactly the 4 predicted files were hand-resolved; every other file auto-merged cleanly; no conflict markers remain.
- The merged tree builds, formats, lints (with and without `fuzzing`), and passes the entire test/fuzz/guard/census/dependency-graph gate list.
- No pre-existing fixture's `semantic_sha256` changed; golden output is unchanged.
- `PROJECT.md` and `HANDOFF.md` both describe AAC-LC framing as implemented and AAC-LC decode as still out of scope, consistently worded.
- `REFERENCES.md` documents the opt-in bundled FDK-AAC reference-tier archive without adding it as a project dependency.
- `src/error.rs` and `origin/wip/delivery-validator-conformance` are untouched throughout.
- Two commits result: the merge commit (Task 1, possibly amended by Task 2), and one `docs(quick-260914-pyd)` commit (Task 3). `.planning/STATE.md` is updated but not committed by the executor.
</success_criteria>

<output>
Create `.planning/quick/260914-pyd-merge-feature-aac-lc-framing-into-main/260914-pyd-SUMMARY.md` when done (not
staged; the orchestrator commits it). Record: the final merge-commit SHA and whether it needed amending in
Task 2; the docs-commit SHA; the full gate-list pass/fail outcome; whether the PROJECT.md Key Decisions table
got a new row or not (and why); any deviation from this plan's conflict-resolution guidance, however small.
</output>
