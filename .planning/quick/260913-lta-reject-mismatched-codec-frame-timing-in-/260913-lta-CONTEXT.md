# Quick Task 260913-lta: reject mismatched codec frame timing in the builder and repair the Parallax delivery fixture - Context

**Gathered:** 2026-09-13
**Status:** Ready for planning

<domain>
## Task Boundary

The `EncoderBuilder` accepts audio elements with different frame timing in the same sub-mix. The Parallax
delivery fixture, `tests/support/parallax_contract.rs:98-156`, puts four audio elements in one sub-mix
of the primary mix presentation: LPCM at 128 samples per frame, FLAC at 128, Opus at 960 and
Ambisonics-mono LPCM at 128, all at 48 kHz. The pinned `decoder_main` rejects the file, which makes
`tests/conformance.rs::parallax_delivery_fixture_uses_the_offline_safe_reference_gates` fail:

- It fails with "Audio elements in a submix must have the same number of samples per frame."
- The reference check is `iamf-tools@848c6ff4 (v2.1.0)`
  `iamf/cli/rendering_mix_presentation_finalizer.cc:120`.
- Quick task 260913-js8 logged this failure as a deferred item.
- It was confirmed in the cross-AI review `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md`.

The task has three parts:
1. The builder rejects these configurations with a typed error.
2. The fixture is repaired so the conformance test passes against the pinned reference.
3. Any related cross-Codec-Config rules the references enforce are adopted in the same pass.

</domain>

<decisions>
## Implementation Decisions

### Governing principle (user, verbatim intent)
- The user's words: "wir wollen mit iamf konform sein und haben eine beispiel implementierung in
  eclipsa-audio und iamf-tools, verscueh die standards daraus abzuleiten und alles so dazu passend zu
  implemntieren". In English: we want to be IAMF-conformant; derive the rules from the reference
  implementations (`iamf-tools` and `eclipsa-audio-plugin`) and implement to match them.
- **Rules come from the references, not from guesses.** Every validation rule this task adds must
  cite its source as `// ref: iamf-tools@v2.1.0 <path> <function>`. Eclipsa is used to confirm how a
  real authoring tool keeps its configurations within these rules.
- **Pinned tree only.** Read `iamf-tools` at the pinned commit `848c6ff4968ff8cc6f728259892ab4f90cb83256`
  (tag `v2.1.0`), for example with `git -C docs/iamf-tools show 848c6ff4:<path>`. Do NOT use the
  working-tree HEAD in `docs/iamf-tools`: it is a newer draft, and CLAUDE.md warns that mirroring
  HEAD produces files `libiamf` rejects.
- If `iamf-tools` and Eclipsa disagree, `iamf-tools@v2.1.0` together with the pinned `libiamf` wins.
  Record the disagreement.

### LOCKED after research (user decision 2026-09-13) — supersedes the defaults below where they conflict
- **Strict single Codec Config.** IAMF v1.1.0 `index.bs` (tag `v1.1.0`, line 1912), under "Common
  restrictions on the IA Sequence for all profiles", says: "There SHALL be only one unique Codec
  Config OBU."
  - `EncoderBuilder::build()` rejects any declaration with more than one Codec Config, using a new
    `ErrorKind` variant with no payload, e.g. `MultipleCodecConfigs` at
    `Location::Field("codec_configs")`. A variant carrying two `u32` values would grow `Error` past
    the 32-byte assertion (verified by research).
  - This check makes the per-sub-mix frame-size, sample-rate and bit-depth rules impossible to break
    through the builder. No separate timing check is needed.
  - Cite the spec line and the `iamf-tools@v2.1.0` sites from RESEARCH.md in `// ref:` comments.
- **Fixture repair:** all four Audio Elements share the single LPCM Codec Config (16-bit LE, 48 kHz,
  128 samples per frame).
  - Remove the FLAC and Opus Codec Configs, and the frames and substreams that feed them.
  - Keep the four retained names and the 4 Audio Elements.
  - Update `tests/parallax_contract.rs` so the expected codec ids are `[0]`.
  - Research ran this variant against the pinned `decoder_main` and it passed CONF-06.
- **Follow-ups, not this task:**
  - FLAC and Opus Parallax deliveries, each as its own IA sequence.
  - The spec rule that every Parameter Block duration equals the Audio Frame duration
    (`index.bs:1915-1916`).
  - The spec rule that every Audio Substream has the same trimming information (`index.bs:1913`,
    related to the review's trim-position finding).
  - Record these as deferred items; do not implement them here.
- Existing tests that declare more than one Codec Config (`tests/encoder_builder.rs`,
  `tests/encoder_streaming.rs`) must be migrated to one config, or turned into rejection tests.

### Validation scope (defaults accepted — "All clear"; superseded by the LOCKED block above)
- The check applies per sub-mix, matching the reference decoder's rule.
- It must also enforce any further cross-config rules the pinned references apply to the same
  elements, such as common output sample rate or profile-filter Condition D in
  `iamf/cli/profile_filter.cc` `FilterProfilesForCodecConfigRules`. Research decides exactly which
  rules apply and at what scope (sub-mix, mix presentation or profile).

### Error surface (defaults accepted)
- Fail in `EncoderBuilder::build()`, alongside the existing descriptor validation.
- Use a new typed `ErrorKind` variant that names the conflicting values: both frame sizes and/or
  both sample rates, plus enough identity (mix presentation / sub-mix / element) to locate the
  conflict.
- Follow the existing `error.rs` conventions, including `#[non_exhaustive]` and the size assertion.

### Fixture repair (default, subject to research)
- **Default:** make all four elements share one frame size that every codec in the fixture
  supports. Opus at 48 kHz allows 120/240/480/960, so raise LPCM and FLAC (and the Ambisonics LPCM)
  to 960 and keep Opus at 960.
- The fixture must keep its purpose: the retained names `program stereo`, `flac archive`,
  `opus stream`, `ambisonics bed`, the 4 Audio Elements, the filtering and determinism assertions,
  and the parameter block durations, scaled consistently if they depend on 128.
- If research shows the references require something else (for example Base-Enhanced Condition D's
  limit on Codec Config count or LPCM pairing), adjust the fixture to match and record why.

### Claude's Discretion
- Whether `EncodingWriter` / preflight also needs a runtime guard, beyond the build-time check.
- Test layout, for example a new focused test file versus extending `tests/encoder_builder.rs`.
- Whether to open follow-up items for review findings that are related but out of scope: trimming
  position, and profile selection for expanded layouts.

</decisions>

<specifics>
## Specific Ideas

- **Required tests:**
  - A builder regression test showing that mismatched `num_samples_per_frame` in one sub-mix is
    rejected with the new error.
  - A positive test showing that matching timing still builds.
  - Tests for any adopted sample-rate or profile rule.
- **Required gate:** `tests/conformance.rs::parallax_delivery_fixture_uses_the_offline_safe_reference_gates`
  must pass when the pinned `iamf-tools` container is available. Report honestly if it is not.
- **Must not change:** golden byte-identity fixtures (`tests/golden.rs`) and `DIFF-LEDGER.md` must
  stay green and unchanged unless a change is fully explained.
- **Don't touch:** `README.md` (unrelated user edit) and the untracked `docs/` clones. Only read the
  clones.

</specifics>

<canonical_refs>
## Canonical References

- `iamf-tools@848c6ff4` `iamf/cli/rendering_mix_presentation_finalizer.cc`
  `GetCommonCodecConfigPropertiesFromAudioElementIds`: the sub-mix frame-size rule, plus the common
  sample-rate and bit-depth handling.
- `iamf-tools@848c6ff4` `iamf/cli/profile_filter.cc` `FilterProfilesForCodecConfigRules`: Condition D,
  which requires the same frame sizes and output sample rates across Codec Configs.
- `docs/eclipsa-audio-plugin/`: how a production authoring tool keeps codec timing consistent. Read
  only.
- The pinned `libiamf` SHA from `REFERENCES.md`: the decoder-side acceptance rules.
- `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md`: Consensus concern #2.
- `.planning/quick/260913-js8-fix-the-count-field-preallocation-memory/deferred-items.md`

</canonical_refs>
