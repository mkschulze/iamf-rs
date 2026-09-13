# Quick 260913-th8: reject empty Mix Presentation sets and duplicate element references, and add sub-mix / headphones findings - Research

**Researched:** 2026-09-13
**Domain:** IAMF v1.1.0 Mix Presentation OBU semantics; `EncoderBuilder::build()` validation order; parse-side `Finding`s
**Confidence:** HIGH. Every rule was read at a pinned revision. The proposal was implemented on a scratch copy, and the full suite ran green there with exactly one hash update.

<user_constraints>
## User Constraints

There is no th8 CONTEXT.md. The standing directive and the qk3 LOCKED rules apply.

- **Satisfy both the IAMF spec and the pinned references.** v1.1.0 is binding (`git -C docs/iamf show v1.1.0:index.bs`). Where the spec is stricter, the spec wins. Where a reference is stricter, the reference rule stays, because the Core Value is that libiamf accepts the file. Record each disagreement in a `// ref:` comment.
- **Pinned revisions.** Read iamf-tools only at `848c6ff4968ff8cc6f728259892ab4f90cb83256` (= tag `v2.1.0`, verified with `git describe`), and libiamf only at `f06e919e` (= tag `v1.1.0`, verified). Eclipsa is read-only.
- **Scope (qk3 deferred items 1-3):**
  1. `build()` rejects zero Mix Presentations.
  2. `build()` rejects a duplicate `audio_element_id` within one Mix Presentation.
  3. `MixPresentation::validate()` gets dedicated sub-mix-count and reserved-headphones findings, without pre-empting `build()`'s dedicated kinds.
- **Out of scope:** qk3 deferred item 4 (Binaural `decoder_main` abort) and item 5 (CONF-05 FLAC/Opus `iamfdec` failure).
- **Carried from qk3:**
  - Error kinds are payload-free, and the 32-byte `Error` assertion stays.
  - `Location::Field` is the location type.
  - Golden fixtures and `DIFF-LEDGER.md` stay byte-identical.
  - Low-level writers keep round-tripping foreign files. No write-time rejection.
  - A reference `semantic_sha256` change is allowed only with provenance. Any unexpected change means stop and report.
</user_constraints>

## Project Constraints (from CLAUDE.md)
- `thiserror` for errors, never `anyhow` in a library.
- No `unwrap`/`expect` outside tests. `clippy::indexing_slicing` and `clippy::arithmetic_side_effects` are enforced, and foreign input reaches `validate()`.
- No `HashMap`/`HashSet` where output order is visible. Use a `Vec` in bitstream order.
- Rust 2024; toolchain pinned to 1.85.0.
- Every `// ref:` names a pinned revision.

## Summary

**Rules.**
- v1.1.0 requires `num_sub_mixes != 0` (`:1278`) and forbids a duplicate `audio_element_id` "within one Mix Presentation", which means across all its sub-mixes (`:1280`).
- The zero-Mix-Presentation rule is carried only by `:1929`: "at least one Mix Presentation OBU that complies with ... primary_profile".
- For the references:
  - Zero Mix Presentations: libiamf never finishes configuring; iamf-tools' decoder fails with "No mix presentation OBUs found."
  - Duplicate element ids: iamf-tools rejects them on both read and write; libiamf does not check.

**Builder today.** The builder produces both shapes. A probe on a scratch copy of HEAD `36bf30a` gave:
- zero Mix Presentations: `Ok(Simple)`
- the same handle twice in one presentation: `Ok(Base)`, with wire ids `[0, 0]`

**Pre-emption.** Adding findings to `MixPresentation::validate()` does **not** pre-empt the qk3 dedicated kinds: both dedicated checks already run before `validate_findings(presentation.validate())` (`src/encoder.rs:1091-1117`). The real trap is the duplicate-id finding. `build()` validates the caller's **template**, whose `audio_element_id`s are placeholders:
- The Parallax delivery fixture uses `audio_element_id: 0` for every element.
- Validating the template as-is breaks `parallax_delivery_fixture_uses_the_offline_safe_reference_gates` with `InvalidDescriptorReference`. This was reproduced.
- The fix: validate a clone whose element ids are lowered from the handles.

**Primary recommendation:**
- Add `ErrorKind::NoMixPresentation` and `ErrorKind::DuplicateMixPresentationAudioElement`.
- In the per-presentation loop, check duplicate **handles** after the headphones check.
- Run `validate_findings` on a handle-lowered clone.
- Check for zero Mix Presentations last in `validate_declarations`.
- Add three findings to `MixPresentation::validate()`.
- Expect exactly one hash change: `test_000124` → `795ef2885684954d4e5eda8e26c52e0a1328488844d4de54e82eb1e69c8a9e2f`, valid only with the exact messages and order below.

## A. Rules (IAMF v1.1.0 `index.bs`, read this session from `v1.1.0:index.bs`)

| # | Line | Verbatim | Reading |
|---|------|----------|---------|
| R1 | 1278 | "`num_sub_mixes` specifies the number of sub-mixes. It SHALL NOT be set to 0." | [VERIFIED] |
| R2 | 1280 | "`num_audio_elements` ... It SHALL NOT be set to 0. There SHALL be no duplicate values of `audio_element_id` within one Mix Presentation." | Scope is the **whole presentation**, across sub-mixes. `num_audio_elements` is per sub-mix, but the uniqueness sentence says "within one Mix Presentation". iamf-tools reads it the same way (see B). [VERIFIED] |
| R3 | 1920 | "`num_sub_mixes` SHOULD be set to 1. Mix Presentation OBUs with `num_sub_mixes` > 1 SHOULD be ignored." | [VERIFIED] |
| R4 | 1337-1341 | "- 2~3: Reserved for future use." / "Parsers encountering a reserved value of `headphones_rendering_mode` SHALL ignore the Mix Presentation OBU that contains this `rendering_config`." (`:1339`, `:1341`) | [VERIFIED] |
| R5 | 1929 | "There SHALL be at least one Mix Presentation OBU that complies with the conformance points of the primary_profile set in the IA Sequence." | The only IA-Sequence-level SHALL that implies ≥ 1 Mix Presentation. [VERIFIED] |
| R5a | 1233 | "An IA Sequence MAY have one or more Mix Presentations specified." | Informative. [VERIFIED] |
| R5b | 1902 | "A Mix Presentation OBU SHALL be the final OBU of Descriptors." | Sits under the Reserved-OBU placement restriction (`:1900`). It implies presence only indirectly. [VERIFIED] |
| R5c | 2111 | configOBUs (ISO-BMFF): "One or more Mix Presentation OBUs" | Container-specific (M5). [VERIFIED] |

Other SHALLs in the Mix Presentation semantics, and whether the crate enforces them. Listed only; not implemented here.

| Line | Rule | Crate today |
|------|------|-------------|
| 1269 | One unique `mix_presentation_id` per presentation | Enforced (duplicate-id finding in both validators; builder allocates) |
| 1272 | `annotations_language` SHALL conform to BCP-47; "The same language SHALL NOT be duplicated" | **Not enforced** → deferred. iamf-tools rejects duplicates on both read (`mix_presentation.cc:528-530`, "Annotation languages") and write (`:471-473`). |
| 1280 | `num_audio_elements` SHALL NOT be 0 | Enforced (`SubMix::validate()` finding, `src/obu/mix_presentation.rs:340-346`) |
| 1300 | Each sub-mix SHALL include loudness for Stereo | Enforced (finding `sub_mix.layouts`, `:331-339`) |
| 1305-1310 | Single scalable channel audio: `num_layouts ≥ num_layers` with exceptions | Not enforced. The builder rejects scalable layers, so this is parse-side only → deferred |
| 1313 | `MixPresentationTags` SHALL be present if `obu_size` exceeds the sub-mix loop | Modelled as `trailing`; not interpreted → deferred |
| 1282 | Unknown referenced element: parsers SHOULD ignore the presentation | Reported (reference-resolution finding) |

Trivially adjacent: **zero Audio Elements** needs no new check. Once zero Mix Presentations are rejected, every presentation has one sub-mix with ≥ 1 resolved element.

## B. References

| Shape | iamf-tools @848c6ff4 (v2.1.0) | libiamf @f06e919e (v1.1.0) | Classification |
|-------|-------------------------------|----------------------------|----------------|
| **Zero Mix Presentations** | The encoder sequencer has no check: `iamf/cli/obu_sequencer_base.cc:262-320 WriteDescriptorObus` loops over an empty list. The decoder `iamf/cli/obu_processor.cc:531-533 InitializeForRendering` returns `absl::InvalidArgumentError("No mix presentation OBUs found.")`. | `IAMF_decoder.c:3118-3166 iamf_decoder_internal_read_descriptors_OBUs`: `IAMF_FLAG_CONFIG` is set only when every `IAMF_FLAG_DESCRIPTORS` bit, including `IAMF_FLAG_MIX_PRESENTATION` (`IAMF_decoder_private.h:43-45`), is set (`:3159-3160`). Otherwise `iamf_decoder_internal_init` returns `IAMF_ERR_BUFFER_TOO_SMALL` (`:3111`), so decoding never starts. | **Equal in effect.** The spec (R5) and both decoders make it non-conformant. Build-side rejection satisfies all three. |
| **Duplicate element in one presentation** | Rejected on **read** (`iamf/obu/mix_presentation.cc:550`) and **write** (`:496`) by `ValidateUniqueAudioElementIds` (`:41-54`, comment "Audio Element IDs must be unique across all sub-mixes."). Error text from `iamf/common/utils/validation_utils.h:145-146`: `"Audio element IDs must be unique. Found duplicate: <id>"`. | No check found. `iamf_database_mix_presentation_is_valid` (`IAMF_decoder.c:1284-1337`) counts `nb_elements` and sums channels per entry, so a duplicate is counted twice [INFERRED from reading; not run]. | **Spec = iamf-tools, stricter than libiamf.** Reject. |
| **`num_sub_mixes` ≠ 1** | 0: `ValidateNumSubMixes` (`mix_presentation.cc:195-199`) → `"Invalid num_sub_mixes. Expected 0 != 0."` (`validation_utils.h:87-88`). It is wrapped in `MAYBE_RETURN_IF_NOT_OK`, which is ignored only under `IGNORE_ERRORS_USE_ONLY_FOR_IAMF_TEST_SUITE` (`macros.h:24-36`). > 1: `profile_filter.cc:220-238` erases all three profiles → `"<id> has N sub mixes, but the requested profiles do not support this number of sub-mixes."` | `IAMF_OBU.c:760-762` (0: "num_sub_mixes should not be set to 0."), `:763-767` (> `IAMF_MIX_PRESENTATION_MAX_SUBS` = 1, `IAMF_types.h:168`), `:778-782` ("the total of sub mixes should be 1, not support %"). All `goto mix_presentation_fail`. | **References stricter than the spec's SHOULD** (R3). qk3 already rejects in build. This task only adds the finding. |
| **Reserved `headphones_rendering_mode`** | `profile_filter.cc:240-271` erases all profiles → `"<id> has an audio element with headphones rendering mode= N sub mixes, but the requested profiles do support not this mode."` (sic, `:260-265`) | `IAMF_decoder.c:1309-1315`: `"Invalid headphones rendering mode %d in element %" PRId64 " of mix presentation %" PRId64`; `ret = IAMF_ERR_UNIMPLEMENTED` → `return !ret` (`:1331`) → the presentation is not added (`:1426-1431`). | **Equal** (spec SHALL ignore; both drop it). Already rejected in build by qk3. |

Also noted, not in scope: `IAMF_decoder.c:4281-4282` dereferences `mixp` without a null check when no Mix Presentation was stored (`ctx->mix_presentation_id = mixp->mix_presentation_id;`) [VERIFIED: read]. Configuration normally never gets there with zero Mix Presentations (see above).

## C. Crate integration

### Current code (read this session)
- `src/encoder.rs:830-949 build()` calls `resolve_parameter_references()` and then `validate_declarations()`. Handle-to-wire-id lowering happens only after validation (`:908-930`, `element.audio_element_id = u32::try_from(handle.index)`).
- `src/encoder.rs:1068-1140` is the per-presentation loop, in this order:
  1. `expected_references != audio_elements.len()` → `InvalidDescriptorReference` at `mix_presentation.audio_elements`
  2. handle validity
  3. `SubMixCountNotOne` at `Field("num_sub_mixes")` (`:1091-1096`)
  4. `ReservedHeadphonesRenderingMode` at `Field("headphones_rendering_mode")` (`:1100-1116`)
  5. **`validate_findings(declaration.presentation.validate())?` (`:1117`)**
  6. reserved layouts / loudness extension
- `src/encoder.rs:1271-1280 validate_findings` maps any finding to `InvalidDescriptorReference` at `Field("descriptors")`.
- `src/error.rs:1-279`: `Finding { at: Location, message: String }` has **no kind field**. "Finding kinds" are therefore their `Location::Field` path plus message. qk3 kinds are at `:152-159`, and `const _: () = assert!(size_of::<Error>() <= 32);` is present.
- `src/obu/mix_presentation.rs:376-425 MixPresentation::validate()` reports: the annotation count, then per sub-mix `SubMix::validate()` (stereo layout, zero elements), `rendering_config.reserved`, element gain definition, element annotation count, output gain, `layout.reserved`.
- `src/model/mod.rs:122-214 DescriptorSet::validate()` and `src/sequence.rs:195-396 ParsedSequence::validate()` both call `presentation.validate()` (`mod.rs:131`, `sequence.rs:238`), so new findings flow to both validators automatically. Both end with qk3 `compliance_findings` / `sequence_limit_findings`.
- `src/model/profile.rs:211-253 presentation_minimum_profile` returns `SubMixCountNotOne` / `ReservedHeadphonesRenderingMode`. `compliance_findings` (`:447-487`) treats any error as non-compliant, and zero presentations as non-compliant.

### Probe (scratch copy of HEAD 36bf30a, repo untouched)
```
TH8_PROBE zero_mp = Ok(Simple)
TH8_PROBE zero_mp findings = [Finding { at: Field("primary_profile"), message: "no Mix Presentation complies with primary_profile 0 (IAMF v1.1.0 index.bs:1929)" }]
TH8_PROBE dup = Ok(Base)
TH8_PROBE dup findings = []
TH8_PROBE dup ids = [0, 0]
```
Both gaps are confirmed [VERIFIED]. The duplicate also inflates the profile: the element is counted twice per presentation, so Base instead of Simple.

### Design: no pre-emption, and the placeholder-id trap
1. **Sub-mix and headphones findings can go straight into `MixPresentation::validate()`.** The dedicated checks (`:1091`, `:1100`) already return before `:1117`, so `validate_findings` never sees those shapes. The only requirement is to keep that order, and a test should lock it.
2. **The duplicate check must run on handles, not template ids.** Callers pass placeholder ids in templates: `tests/support/parallax_contract.rs:243`, `tests/conformance.rs:1798` and `tests/encoder_streaming.rs:916` all use `audio_element_id: 0`. **Reproduced:** with a duplicate-id finding in `validate()` and `validate_findings(declaration.presentation.validate())` left as-is, `conformance.rs` fails:
   ```
   the filtered public delivery fixture builds: Error { kind: InvalidDescriptorReference, at: Field("descriptors") }
   ```
   **Fix:** after the dedicated duplicate-handle check, clone the template, set each element's `audio_element_id` from the zipped handle (`u32::try_from(handle.index).map_err(allocation_error)?`, the same lowering `build()` does at `:920-921`), and call `validate_findings(lowered.validate())`. Distinct handles give distinct ids, so the duplicate finding can never fire from `build()`, and the template's placeholder ids no longer matter.
3. **The zero-Mix-Presentation check goes last** in `validate_declarations`, after the `write_descriptors` exercise. Many tests build element-only declarations to assert a specific earlier error, for example `build_element()` → `UnsupportedParameterData` and `ScalableChannelLayers`. With the check last, all of them kept their kinds on scratch.
4. **Parse side, zero Mix Presentations: add no new finding.** `compliance_findings` already reports "no Mix Presentation complies with primary_profile N (IAMF v1.1.0 index.bs:1929)" for zero presentations. That is the exact spec rule (R5), so a dedicated finding would duplicate it. No reference fixture has zero Mix Presentations (scratch scan: none printed `mps=0`), so leaving it out costs nothing.

## D. Hash impact (measured on scratch)

A scratch test iterated all 37 `POSITIVE_EXPECTATIONS`. It printed any fixture whose hash changed, that has a new finding, or that has zero Mix Presentations.

- With placeholder messages: only `test_000124.iamf` changed, and nothing else had a new finding or zero Mix Presentations.
- With the **exact messages and order below**:
  ```
  TH8CHANGE test_000124.iamf old=893a6103219259eb5863c01d649f97a8af298f0dcbda3e0514c5236264016ad6 new=795ef2885684954d4e5eda8e26c52e0a1328488844d4de54e82eb1e69c8a9e2f mps=1 th8=[Finding { at: Field("num_sub_mixes"), message: "num_sub_mixes is 2; it SHOULD be 1 and parsers SHOULD ignore a Mix Presentation with more (IAMF v1.1.0 index.bs:1920); libiamf fails its parse" }]
  ```
- With that hash substituted: full `cargo test --locked --no-fail-fast` gave 28 `test result: ok`, 0 non-ok. `cargo clippy --locked --lib -- -D warnings` gave 0 warnings.

| Fixture | Old | New | Reason | Pinned testdata |
|---------|-----|-----|--------|-----------------|
| `test_000124.iamf` | `893a6103219259eb5863c01d649f97a8af298f0dcbda3e0514c5236264016ad6` (qk3 value) | `795ef2885684954d4e5eda8e26c52e0a1328488844d4de54e82eb1e69c8a9e2f` | Its one Mix Presentation has 2 sub-mixes; the new `num_sub_mixes` finding is inserted before the sequence-level findings. | `iamf/cli/testdata/test_000124.textproto:19` `is_valid: false`, `:20` `is_valid_to_decode: false`, description "An invalid IAMF stream because it is base profile and the Mix Presentation contains 2 sub-mixes.", section `3.7/num_sub_mixes` [VERIFIED at 848c6ff4]. (The qk3 CONTEXT's `:111` does not match this file.) |

**The hash is only valid if the messages, `Field` paths and finding order are byte-identical to E.2.** If the executor changes any message, it must recompute the hash with a probe like the one above. If any fixture other than `test_000124` changes, stop and report.

The corpus has no fixture with a reserved headphones mode or a duplicate element id. `test_000125` (reserved headphones) exists upstream but is not committed here.

**Compliance finding overlap (test_000124):** both findings are reported. The new finding names the cause at presentation scope (`num_sub_mixes`, `:1920`). The qk3 `primary_profile` finding states the sequence-level consequence (`:1929`).
- They diverge whenever a second, compliant presentation exists: the compliance finding disappears and the sub-mix finding stays.
- Removing the compliance finding would change qk3 behaviour and the other four qk3 hashes.
- Keep both.

## E. Recommendation

### E.1 Error kinds (`src/error.rs`, after `ReservedHeadphonesRenderingMode`, payload-free)
```rust
/// The builder declares no Mix Presentation.
// ref: IAMF v1.1.0 index.bs:1929 (at least one Mix Presentation SHALL comply with primary_profile)
// ref: iamf-tools@v2.1.0 iamf/cli/obu_processor.cc:531-533 ("No mix presentation OBUs found.")
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:3157-3160 (IAMF_FLAG_CONFIG needs a Mix Presentation)
#[error("an IA sequence must carry at least one Mix Presentation")]
NoMixPresentation,
/// A Mix Presentation references the same Audio Element more than once.
// ref: IAMF v1.1.0 index.bs:1280; iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:41-54 ValidateUniqueAudioElementIds
// DISAGREEMENT: libiamf@v1.1.0 has no check; spec and iamf-tools are stricter and win
#[error("a Mix Presentation references the same Audio Element more than once")]
DuplicateMixPresentationAudioElement,
```
- Locations: `Field("mix_presentations")` and `Field("mix_presentation.audio_elements")`. The second matches the existing handle-count error at `encoder.rs:1078`.
- Add both kinds to `tests/error_shape.rs::static_builder_errors_are_compact_typed_kinds` (`:167-187`).
- Alternative, not chosen: `DuplicateDeclaration`, which is used for a duplicate substream handle in one element. A dedicated kind matches the qk3 precedent of one spec rule per kind.

### E.2 Findings in `MixPresentation::validate()` (exact order; the order is hash-bearing)
The first two go immediately after the `localized_presentation_annotations` check and before the `for sub_mix` loop:
1. `Field("num_sub_mixes")`:
   - `0` → `"num_sub_mixes is 0; it SHALL NOT be 0 (IAMF v1.1.0 index.bs:1278)"`
   - `n > 1` → `"num_sub_mixes is {n}; it SHOULD be 1 and parsers SHOULD ignore a Mix Presentation with more (IAMF v1.1.0 index.bs:1920); libiamf fails its parse"`
2. `Field("sub_mix.audio_element_id")`: collect ids across **all** sub-mixes in wire order. For each later occurrence of an earlier id (`iter().take(position).any(..)`, no indexing), emit `"mix presentation {mix_presentation_id} references audio_element_id {id} more than once; there SHALL be no duplicate audio_element_id within one Mix Presentation (IAMF v1.1.0 index.bs:1280)"`. `Field("audio_element_id")` is avoided because the reference-resolution finding already uses it.

The third goes inside the element loop, right after `push_reserved_finding(.., "rendering_config.reserved", 6)`:

3. `Field("headphones_rendering_mode")`: for `HeadphonesRenderingMode::Reserved(raw)` → `"audio element {audio_element_id} carries reserved headphones_rendering_mode {raw}; parsers SHALL ignore this Mix Presentation (IAMF v1.1.0 index.bs:1341)"`

Each block needs a `// ref:` with the reference lines from B.

### E.3 `build()` order (per-presentation loop)
1. handle count
2. handle validity
3. `SubMixCountNotOne`
4. `ReservedHeadphonesRenderingMode`
5. **new** `DuplicateMixPresentationAudioElement`, on `declaration.audio_elements` handles, via `iter().take(position).any(|other| other == handle)`
6. **changed** `validate_findings(lowered.validate())`, where `lowered` is a clone with handle-derived ids
7. existing layout checks

After the whole of `validate_declarations` (after the `write_descriptors` exercise): **new** `if self.mix_presentations.is_empty()` → `NoMixPresentation`.

A reference scratch diff is at `/private/tmp/claude-501/-Users-cell-local-iamf-rs/ffcacbe6-b061-4445-a28e-fe0219356ad0/scratchpad/th8/th8-proposal.diff`. Its error doc comments are placeholders; use E.1.

### E.4 Tests
Most were exercised on scratch.
- `tests/encoder_builder.rs`:
  - `build_rejects_zero_mix_presentations` → `NoMixPresentation` at `mix_presentations`.
  - `build_rejects_the_same_audio_element_twice_in_one_presentation` → `DuplicateMixPresentationAudioElement` at `mix_presentation.audio_elements`. Update the probe's `Ok(Base)` expectation.
  - Positive: `distinct_elements_with_equal_template_ids_build`, with template ids `[0, 0]` on two handles → `Ok`. This locks the lowering.
  - Order:
    - two sub-mixes + reserved headphones + duplicate handle → `SubMixCountNotOne`
    - reserved headphones + duplicate handle (one sub-mix) → `ReservedHeadphonesRenderingMode`
    - duplicate handle + another presentation finding (e.g. missing stereo layout) → `DuplicateMixPresentationAudioElement`, not `InvalidDescriptorReference`
  - Keep the existing qk3 tests (`build_rejects_two_sub_mixes`, `_zero_sub_mixes`, `_reserved_headphones_rendering_modes`, `build_accepts_binaural_headphones_rendering_mode`).
- `tests/descriptors.rs` or `tests/sequence_parse.rs`: `MixPresentation::validate()` gives the exact findings for 0 sub-mixes, 2 sub-mixes, and duplicate ids split across two sub-mixes. The last one proves presentation scope, which iamf-tools shares. Also cover reserved modes 2 and 3, and a clean presentation → `[]`. For one parsed foreign presentation, assert both `DescriptorSet::validate()` and `ParsedSequence::validate()` carry them.
- **Required fixture update:** `tests/encoder_streaming.rs::mono_builder` (`:735-767`) builds with **no** Mix Presentation and fails with `NoMixPresentation` (seen on scratch). Add a mono presentation from `presentation()` with element/output `parameter_rate` set to the output rate: 16 000 for LPCM/FLAC, 48 000 for Opus (`ParameterRateMismatch` rule). With that change the test passed on scratch.
- `tests/support/reference_expectations.rs`: `test_000124` → the new hash, with provenance in the commit message.

### E.5 Deferred (write `260913-th8-deferred-items.md`)
1. `annotations_language` duplicates (`:1272`). iamf-tools rejects them on read/write; the crate enforces neither. Unverified whether `build()` accepts them.
2. BCP-47 conformance of `annotations_language` (`:1272`).
3. `num_layouts ≥ num_layers` for a single scalable channel element (`:1305-1310`), parse-side.
4. `MixPresentationTags` presence rule (`:1313`).
5. iamf-tools' unique anchored-loudness elements (`mix_presentation.cc:56-66`).
6. Carried: qk3 item 4 (Binaural abort) and item 5 (CONF-05 FLAC/Opus).

## Security Domain
`security_enforcement: true` (ASVS L1). The applicable category is V5 input validation. The new findings run on foreign input, so they must avoid indexing, bare arithmetic and panics, and must not allocate without bound: the id `Vec` is bounded by the elements already parsed. `take(position).any(..)` is O(n²) over `num_audio_elements`, which is already bounded by the OBU's `bytes_remaining()` check at read (`read_bounded_count`, `mix_presentation.rs:516`). This is acceptable. No new I/O and no new dependencies.

## Assumptions Log
| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | libiamf counts a duplicated element twice (channels/elements) rather than misbehaving further | B | Low. Rejection happens regardless. |
| A2 | Choosing a dedicated `DuplicateMixPresentationAudioElement` over reusing `DuplicateDeclaration` | E.1 | API naming only. It is additive under `#[non_exhaustive]`. |

## Open Questions
1. Should the parse-side compliance message say "no Mix Presentation is present" when the list is empty? This is optional and changes no committed hash (no zero-Mix-Presentation fixture). Recommendation: leave it unchanged.

## Sources
- IAMF spec `docs/iamf` @ `v1.1.0` (`e1315500`) `index.bs:1233, 1269-1313, 1337-1343, 1900-1929, 1980-1985, 2107-2111`
- iamf-tools @ `848c6ff4` (v2.1.0):
  - `iamf/obu/mix_presentation.cc:41-66, 180-199, 466-498, 516-550`
  - `iamf/common/utils/validation_utils.h:81-89, 138-150`
  - `iamf/common/utils/macros.h:24-36`
  - `iamf/cli/profile_filter.cc:220-271, 404-408`
  - `iamf/cli/obu_processor.cc:527-542`
  - `iamf/cli/obu_sequencer_base.cc:262-320`
  - `iamf/cli/testdata/test_000124.textproto:15-28`, `test_000125.textproto:15-28`, `test_000502.textproto:17`
- libiamf @ `f06e919e` (v1.1.0):
  - `code/src/iamf_dec/IAMF_OBU.c:753-803`
  - `code/src/iamf_dec/IAMF_decoder.c:1284-1337, 1426-1431, 3085-3166, 3413-3436, 4276-4305`
  - `IAMF_decoder_private.h:43-45`
  - `IAMF_types.h:168`
- Crate (HEAD 36bf30a):
  - `src/encoder.rs:779-949, 1068-1185, 1271-1280`
  - `src/error.rs`
  - `src/obu/mix_presentation.rs:290-425`
  - `src/model/mod.rs:122-214`
  - `src/sequence.rs:195-396`
  - `src/model/profile.rs:200-487`
  - `tests/parse_reference.rs:91-128`
  - `tests/support/parallax_contract.rs:231-266`
  - `tests/encoder_streaming.rs:735-767, 909-930`
- Scratch evidence: `.../scratchpad/th8/after1.log`, `after3.log`, `th8-proposal.diff`

## Metadata
- Rules / references: HIGH (read at pinned revisions)
- Integration design: HIGH (implemented and full suite green on scratch)
- Hash: HIGH, conditional on the exact E.2 text
- Valid until the next change to `MixPresentation::validate()` or the reference corpus.
