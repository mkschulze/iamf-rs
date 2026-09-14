# Quick Task 260914-pyd: merge feature/aac-lc-framing into main - Context

**Gathered:** 2026-09-14
**Status:** Ready for planning

<domain>
## Task Boundary

Merge `origin/feature/aac-lc-framing` (8 commits, tip `2674f1e`, forked from `a9cf271` on
2026-09-12, predates the day's five quick tasks) into `main` (currently `44221fe`). Adds AAC-LC
Audio Frame framing (a fourth pre-encoded codec alongside LPCM/FLAC/Opus), matching the existing
FLAC/Opus pattern: this crate frames pre-encoded `raw_data_block()` access units, never encodes or
decodes AAC.

User decision (2026-09-14): "ja mergen würde ich sagen, ist doch ein normales feature" — merge it,
ordinary feature work, no further discussion needed.

**Explicitly out of scope for this task:** a separate uncommitted "delivery validator"
(`validate_delivery_conformance`/`validate_delivery_timeline`, new `ErrorKind` variants
`MissingDeliveryMixPresentation`/`InvalidDeliveryDescriptorSet`/`DeliveryStartTrimNotFirst`/
`DeliveryEndTrimNotFinal`) was found uncommitted in a stale worktree during reconnaissance for
*this* task and has been preserved separately on `origin/wip/delivery-validator-conformance`
(commit `79dafb1`). Do NOT touch, merge, cherry-pick, or reference that branch/commit in this task.
It is untested and will be reviewed as its own later task. `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md`
was written against that uncommitted state, not against any commit — treat its P1 findings
(`validate_delivery_conformance`/`validate_delivery_timeline`) as describing THAT separate,
unreviewed work, not something already on `main` or on `feature/aac-lc-framing`.

</domain>

<decisions>
## Implementation Decisions

### Merge strategy
- A real `git merge --no-ff origin/feature/aac-lc-framing` into `main` (a merge commit, not a
  rebase or squash) — preserves the branch's own history, matches "ordinary feature" framing.
- The orchestrator already did a disposable trial merge in a scratch worktree (since removed) and
  found exactly 4 conflicting files, all small. Use this as the authoritative resolution guide —
  the executor should still resolve for real and verify by reading the actual conflict markers, not
  blindly trust this summary, but should expect exactly these and nothing more:

  1. **`src/encoder.rs`** — one hunk, the `use crate::obu::{...}` import list. HEAD (main) added
     `HeadphonesRenderingMode`; the branch added `CODEC_ID_AAC` (and reordered
     `CODEC_ID_FLAC, CODEC_ID_LPCM, CODEC_ID_OPUS`). Union both: keep `HeadphonesRenderingMode` AND
     `CODEC_ID_AAC` in the import list. No other part of `encoder.rs` conflicts — main's
     `TemporalProgress`/cross-unit-trim additions and the branch's `FrameInput::AacLc` /
     `DecoderConfig::AacLc` additions are in different, non-overlapping regions and should
     auto-merge; verify with `rustfmt`/`clippy` after resolving the import line.

  2. **`tests/round_trip.rs`** — one hunk, `use iamf::obu::{...}` import list. HEAD added nothing
     notable here; the branch added `CodecConfig, DecoderConfig, ParameterData`. Take the branch's
     superset (it's a strict addition over HEAD's list) — verify by compiling.

  3. **`tests/encoder_builder.rs`** — one hunk, `use iamf::obu::{...}` import list. HEAD (main)
     added `AnchorElement, AnchoredLoudness, ..., HeadphonesRenderingMode, ..., RenderingConfig,
     SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement` (today's kfs/m62
     work added test helpers). The branch's side is a smaller subset plus `DecoderConfig`. Union:
     keep everything HEAD has, add `DecoderConfig` from the branch. Verify by compiling — an unused
     import is a clippy/rustc warning, not silently wrong, so trust the compiler.

  4. **`tests/encoder_streaming.rs`** — two hunks, real test-body conflicts, need actual merging of
     logic, not just import unions:
     - **First hunk** (~line 157): HEAD's test `lpcm_flac_and_opus_inputs_follow_the_frozen_codec_kind`
       iterates `(result, trimming)` pairs (LPCM/FLAC → `None`, Opus → `Some(start_trim(312))`) —
       this is today's Opus `pre_skip` start-trim enforcement (quick 260913-vvx / related trimming
       work). The branch's version, renamed
       `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`, iterates a flat `result`
       list adding `mono_builder(CodecConfig::aac_lc(0, 48_000)?)` as a fourth case, with no
       `trimming` tuple. **Resolution:** keep HEAD's `(result, trimming)` tuple shape and the Opus
       `ref:` comment, keep HEAD's function body below (which reads `trimming` per case), add the
       branch's AAC-LC codec config as a fifth tuple entry with `trimming = None` (AAC-LC has no
       encoder-derived start trim in this crate — it behaves like LPCM/FLAC for this test). Rename
       the function to include `_and_aac_lc` per the branch's naming, e.g.
       `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind`. Also check whether the
       function body (below the conflict, both sides likely auto-merged) needs an `AacLc` arm added
       to whatever `match`/pattern reads `frame.1`/`expected_payload` — grep the full function after
       resolving, since `FrameInput`'s variants are matched exhaustively elsewhere and a missing arm
       is a compile error, not a silent gap.
     - **Second hunk** (~line 1026): HEAD computes `(input, rate)` per `DecoderConfig` variant,
       where `rate` feeds `sub_mix.output_mix_gain.definition.parameter_rate` (today's
       ParameterRateMismatch enforcement — LPCM/FLAC use 16 kHz, Opus uses 48 kHz, matching each
       codec config's own sample rate in this test). The branch computes `input` only (no rate),
       adding `DecoderConfig::AacLc(_) => FrameInput::AacLc(vec![0x21, 0x10, 0x04])`. **Resolution:**
       keep HEAD's `(input, rate)` tuple shape, add the branch's AAC-LC arm with its payload bytes,
       and derive `rate` for AAC-LC from whatever sample rate the test's AAC-LC `CodecConfig` was
       constructed with earlier in this function (find it — likely 48 kHz, matching
       `CodecConfig::aac_lc(0, 48_000)?` used elsewhere in this same file). The `Raw { .. } | _ =>
       unreachable!()` arm stays last. If the exact rate can't be determined from local context,
       reject the merge for this hunk and stop — do not guess a rate for a real test assertion.

  5. Everything else (`.github/workflows/reference.yml`, `README.md`, `src/obu/mod.rs`,
     `tests/conformance.rs`, `tests/descriptors.rs`, `tests/parse_reference.rs`,
     `tests/support/reference_expectations.rs`, `tests/fixtures/*`, `tools/build-reference.sh`,
     `src/dump.rs`, `src/obu/codec_config.rs`) auto-merged cleanly in the trial run (git reported no
     conflicts) because main never touched those files since the branch's fork point. Trust `git
     merge`'s auto-merge for these; do not hand-edit them unless the real merge produces an
     unexpected conflict, in which case stop and report rather than improvising.

- `tools/build-reference.sh`'s AAC change only toggles a prebuilt FDK-AAC archive that already
  ships inside the pinned `libiamf@v1.1.0` checkout, via an opt-in env var
  (`IAMF_REFERENCE_ENABLE_AAC=1`, now set unconditionally in `.github/workflows/reference.yml`'s
  "Build the pinned reference decoder" step). It does not add FDK-AAC as a build dependency of this
  repo, does not touch `Cargo.toml`/`cargo tree`, and matches the existing FLAC/Opus reference
  pattern (prebuilt archives already vendored inside `libiamf`, run only, never read as source
  beyond what the project already treats `libiamf` itself as "may be read"). Confirm this
  understanding by rereading the actual `tools/build-reference.sh` diff during the merge, and add a
  one-line note to `REFERENCES.md`'s licence/build section if it doesn't already cover "the
  reference tier optionally links libiamf's own bundled FDK-AAC archive to run the AAC-LC
  conformance gate" — do not add a new licence table row for FDK-AAC as a project dependency, since
  it is not one.

### Scope-boundary docs that MUST be updated (the branch never touched these — verified by the
orchestrator: `git diff --stat main...origin/feature/aac-lc-framing -- .planning/ HANDOFF.md
REFERENCES.md CONFORMANCE-GATE.md` is empty)
- **`.planning/PROJECT.md`**: line ~56 currently lists "AAC-LC codec implementation" under "Out of
  Scope" ("belongs to `iamf-decode-rs`... Its decoder configuration syntax may be added here when
  that consumer requires it."). After this merge, AAC-LC Audio Frame *framing* (not decode) exists
  in this crate, exactly matching the pattern already carved out for FLAC/Opus (this crate frames,
  never encodes/decodes). Move/reword this entry: framing syntax is now implemented (matches the
  FLAC/Opus precedent already Validated), while AAC-LC *decode* remains out of scope
  (`iamf-decode-rs`'s job) — do not claim more than the branch actually did. Add a Key Decisions row
  if PROJECT.md's decision log convention calls for one (check the file's own structure).
- **`HANDOFF.md`**: line ~270 says "Three codecs shipped, not four: LPCM, FLAC, Opus. **No AAC-LC**,
  though the spec lists it." and line ~314 repeats the AAC-LC out-of-scope line. Update both to
  reflect four codecs, describing the same framing-only boundary (this crate never encodes/decodes
  AAC-LC, only frames pre-encoded `raw_data_block()` units) that FLAC/Opus already have documented
  nearby — follow that existing wording pattern rather than inventing new phrasing.
- Do NOT touch the portfolio boundaries document
  (`/Users/cell/local/Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md`)
  or `.planning/ROADMAP.md`. This is a framing-syntax addition within the crate's existing
  boundary (same shape as FLAC/Opus), not an ownership move or a new cross-library object — the
  top-of-CLAUDE.md "Change control" trigger does not apply. If the executor's own reading of that
  file suggests otherwise, stop and report rather than editing a document outside this repository or
  a repository the user hasn't been asked about.

### What NOT to change
- Do not touch `src/error.rs`. Nothing in this merge needs a new `ErrorKind` variant — the branch's
  own commits (`fix: harden AAC-LC descriptor validation`, `fix: derive AAC-LC roll distance`) reuse
  existing error kinds; verify this holds after the merge (grep the branch's diff for `ErrorKind::`
  if unsure) rather than assuming.
- Do not resolve, reference, or build on `origin/wip/delivery-validator-conformance` — separate,
  unreviewed, out of scope (see Task Boundary above).

### Claude's Discretion
- Exact wording of the PROJECT.md/HANDOFF.md updates, as long as they state the framing-only
  boundary accurately and don't overclaim decode/encode support.
- Whether to keep the merge commit's default merge message or write a fuller one; either way it
  must end with the session's attribution lines (Claude Sonnet 5).
- Commit granularity beyond "the merge itself is one commit" — docs updates may be a separate
  commit or folded into a docs-only follow-up commit, whichever fits this project's day's pattern
  (each of today's five quick tasks used a separate `docs(...)` commit after the code commit(s)).

</decisions>

<specifics>
## Specific Ideas

- The branch's own commit sequence for reference: `6df58e5` docs (spec) → `58de386` docs (plan) →
  `3b0e30d` feat (typed AAC-LC codec framing) → `daa1b58` fix (derive AAC-LC roll distance) →
  `d1007f3` feat (accept framed AAC-LC access units) → `9adcf10` docs (README boundary) → `be7724a`
  fix (harden AAC-LC descriptor validation) → `2674f1e` test (verify against pinned references).
  `docs/superpowers/specs/2026-09-12-aac-lc-framing-design.md` and
  `docs/superpowers/plans/2026-09-12-aac-lc-framing.md` come along with the merge (they're under the
  user's own untracked-ish `docs/` convention elsewhere in this repo, but these two ARE tracked by
  the branch — keep them, they're the design record for this feature).
- `tests/fixtures/reference/test_000076_aac_lc.iamf` (40415 bytes) is a new committed fixture; check
  it doesn't trip `tests/fixtures_cap.rs`'s 65 536-byte-per-file cap or the 39-file curated-subset
  count (D-14) — if it does, stop and report rather than silently raising a cap.
- Reference-hash constraint still applies: `tests/support/reference_expectations.rs` gets 4 lines
  added by the branch (new AAC-LC fixture entries) — these are expected additions, not violations;
  the constraint is that no *existing* fixture's hash changes.

</specifics>

<canonical_refs>
## Canonical References

- `git diff --stat main...origin/feature/aac-lc-framing` (orchestrator's own investigation, this
  session)
- IAMF v1.1.0 AAC-LC decoder config / Audio Frame syntax (see the branch's own
  `docs/superpowers/specs/2026-09-12-aac-lc-framing-design.md` for its citations)
- `.planning/PROJECT.md` "Out of Scope" section, `HANDOFF.md` "Three codecs shipped" section

</canonical_refs>
