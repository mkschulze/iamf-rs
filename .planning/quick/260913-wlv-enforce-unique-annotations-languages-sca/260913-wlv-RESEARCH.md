# Quick 260913-wlv: unique `annotations_language`, `num_layouts >= num_layers` for a single scalable channel element (parse side), unique anchored-loudness elements - Research

**Researched:** 2026-09-13
**Domain:** IAMF v1.1.0 Mix Presentation / LoudnessInfo semantics; `EncoderBuilder::build()` validation order; parse-side `Finding`s
**Confidence:** HIGH. Every rule was read at a pinned revision. The proposal was implemented on a scratch copy of HEAD `3347a7d`. On that copy, `cargo test --locked --no-fail-fast` gave 28/28 `test result: ok` and `cargo clippy --locked --lib -- -D warnings` was clean. Exactly one reference hash changed.

<user_constraints>
## User Constraints

There is no wlv CONTEXT.md. These rules apply: the standing directive (user, 2026-09-13), the qk3 LOCKED rules, and th8's conventions.

- **Scope:** th8 deferred items **1** (duplicate `annotations_language`), **3** (`num_layouts >= num_layers`, parse side) and **5** (unique anchored-loudness elements). Items **2** (BCP-47 conformance) and **4** (`MixPresentationTags` presence) are OUT of scope.
- **Conformance:** satisfy BOTH the IAMF spec (v1.1.0 binding) and the pinned references.
  - Where the spec is stricter, the spec wins.
  - Where a reference is stricter, the reference rule stays.
  - Record every disagreement in a `// ref:` comment.
- **Pinned revisions:**
  - iamf-tools only at `848c6ff4968ff8cc6f728259892ab4f90cb83256` (`git describe` → `v2.1.0`, verified this session).
  - libiamf only at `f06e919e` (`v1.1.0`, verified).
  - Eclipsa read-only (not needed).
- **Carried from qk3/th8:**
  - Error kinds are payload-free; the `size_of::<Error>() <= 32` const assertion stays.
  - The location type is `Location::Field`.
  - Golden fixtures and `DIFF-LEDGER.md` stay byte-identical.
  - Low-level writers keep round-tripping foreign files; no write-time rejection.
  - A reference `semantic_sha256` may change only for fixtures that pinned iamf-tools testdata marks `is_valid: false`, or with a full written explanation. Any other change: stop and report.
</user_constraints>

## Project Constraints (from CLAUDE.md)
- `thiserror` errors; no `anyhow`.
- No `unwrap`/`expect` outside tests. `clippy::indexing_slicing` and `clippy::arithmetic_side_effects` are enforced **in tests too** (`tests/CLAUDE.md`: use `.get()`/`.first_mut()`, never `[0]`).
- No `HashMap`/`HashSet` where output order is visible. The duplicate scans below use `iter().take(position).any(..)`, the th8 pattern.
- Rust 2024, toolchain 1.85.0. Every `// ref:` names a pinned revision.

## Summary

| # | Rule | Spec v1.1.0 | iamf-tools @848c6ff4 | libiamf @f06e919e | Classification | Implement |
|---|------|-------------|----------------------|-------------------|----------------|-----------|
| 1 | Unique `annotations_language` | SHALL (`:1273`) | Rejects on read and write, **exact bytes** | No check | Spec = iamf-tools on the rule. The spec is stricter on equality, because BCP-47 is case-insensitive. | `build()` kind + `MixPresentation::validate()` finding. ASCII-case-insensitive compare. |
| 3 | `num_layouts >= num_layers`, single scalable channel element | SHALL with an unobservable exception (`:1305-1309`) | No check | No check | Spec only | Parse-side finding in `DescriptorSet::validate()` and `ParsedSequence::validate()`. The builder cannot produce the shape. |
| 5 | Unique `anchor_element` within one LoudnessInfo | SHALL (`:1485`) | Rejects on read and write, raw `u8` | No check | **Spec = iamf-tools.** th8 called this "reference-only"; that was wrong. | `build()` kind + `MixPresentation::validate()` finding. |

**Builder today** (scratch probe on HEAD 3347a7d):
```
WLVB dup_lang   = Ok((Simple, []))
WLVB dup_anchor = Ok((Simple, []))
WLVB scalable   = Err(Error { kind: InvalidDescriptorReference, at: Field("num_layers") })
```
- Rules 1 and 5: `build()` accepts files that iamf-tools refuses to read, and `validate()` stays silent. [VERIFIED: scratch]
- Rule 3: `validate_element_topology` rejects `num_layers != 1` (`src/encoder.rs:1491-1493`, `let [layer] = layers.as_slice() else { return Err(invalid("num_layers")); }`), so rule 3 is parse-side only. [VERIFIED: read + scratch]

**Hash impact:** exactly one fixture changes.
- `test_000063.iamf`: `793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24` → `a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409`.
- Pinned testdata marks it `is_valid: false`: "invalid because anchor elements must be unique".

## A. Rule text

### Rule 1: `annotations_language` (spec)
- `index.bs:1273` [VERIFIED: `git show v1.1.0:index.bs`, line confirmed with `grep -n`]: "`annotations_language` specifies the language which both `localized_presentation_annotations` and `localized_element_annotations` are written in. It SHALL conform to [[!BCP-47]]. The same language SHALL NOT be duplicated in this array."
  - th8 cited this as `:1272`; the correct line is **1273**.
- `:1274`: "The i-th `localized_presentation_annotations` and `localized_element_annotations` SHALL be written in the language indicated by the i-th `annotations_language`, where i = 0, 1, ..., `count_label` -1."
  - This language-count consistency rule is already enforced: the `AnnotationCountMismatch` writer error (`src/obu/mix_presentation.rs:554-573`) and the two count findings in `validate()` (`:384-393`, `:464-475`).
  - `count_label` has no further rule (`:1271`: "indicates the number of labels in different languages").
- **Equality.** RFC 5646 §2.1.1 [CITED: rfc-editor.org/rfc/rfc5646.html]: "language tags and their subtags ... are to be treated as case insensitive", so "mn-Cyrl-MN" is not distinct from "MN-cYRL-mn". "The same language" therefore covers `en-US` and `en-us`. Wider canonical equivalence (e.g. `en` vs `en-US`, or grandfathered tags) needs BCP-47 parsing, which belongs to out-of-scope item 2.
- **iamf-tools** [VERIFIED: `iamf/obu/mix_presentation.cc` @848c6ff4]:
  - Write `:471-473`: `RETURN_IF_NOT_OK(ValidateUnique(annotations_language_.begin(), annotations_language_.end(), absl::StrCat("annotations_language", with_mix_presentation_id)));`
  - Read `:528-530`: `RETURN_IF_NOT_OK(ValidateUnique(annotations_language_.begin(), annotations_language_.end(), "Annotation languages"));`
  - Both use `RETURN_IF_NOT_OK`, not `MAYBE_`, so they are strict in every build.
  - `ValidateUnique` (`iamf/common/utils/validation_utils.h:137-150`) inserts into `absl::flat_hash_set<std::string>`. That is exact byte equality. Its error is `"<context> must be unique. Found duplicate: <value>"`.
- **libiamf** [VERIFIED: `code/src/iamf_dec/IAMF_OBU.c:748-749` @f06e919e]: `for (int i = 0; i < mixp->num_labels; ++i) bs_readString(&b, mixp->annotations_language[i], STRING_SIZE);`. No comparison anywhere. `git grep annotations_language` hits only `IAMF_OBU.c`, `IAMF_OBU.h:249` and `vlogging_tool_sr.c`.
- **Rule to implement:** reject or report when two entries are equal under ASCII case folding (`<[u8]>::eq_ignore_ascii_case`). This is a strict superset of iamf-tools' exact-byte check. Record it as DISAGREEMENT (spec stricter).

### Rule 3: single scalable channel element (spec)
`index.bs:1305-1309` [VERIFIED; tabs shown as →]:
```
1305 If a sub-mix in a [=Mix Presentation OBU=] includes only one single scalable channel audio, it SHALL comply with the following:
1306 - [=num_layouts=] SHALL be greater than or equal to the [=num_layers=] field specified in its [=scalable_channel_layout_config=], except in the following cases:
1307
1308 →- The highest [=loudness_layout=] specified in one sub-mix is the layout that was used for authoring the sub-mix. The exception is when the [=Audio Element=] is a zero-order Ambisonics or Mono channel.
1309 →- The highest [=loudness_layout=] for a zero-order Ambisonics or Mono channel [=Audio Element=] is Stereo.
```
- The text is identical in `v1.0.0:1205-1209` and `v1.0.0-errata:1210-1214` [VERIFIED].
- "Scalable channel audio" means `num_layers > 1`:
  - `:770` "the scalable channel audio (i.e., [=num_layers=] > 1)"
  - `:769` "(non-)scalable channel audio (i.e., [=num_layers=] = 1)" [VERIFIED]
- **Exact condition (INFERRED from the text):** the sub-mix has `num_audio_elements == 1`, **and** that element resolves to `audio_element_type == CHANNEL_BASED` with `num_layers > 1`, **and** `num_layouts < num_layers`.
  - `num_layouts` is the raw wire count (`:1292`), so every layout counts: sound-system, binaural and reserved.
  - With `num_layers == 1`, the stereo-layout SHALL (`:1300`) already gives `num_layouts >= 1`.
- **The exception cannot be evaluated.** "the layout that was used for authoring the sub-mix" is not carried in the bitstream. Annex A3 (informative, `:3286-3289`) shows the intended shape: `num_layouts = N` for N input layouts, plus Stereo if absent.
  - The nesting of `:1308-1309` under "except in the following cases" is also ambiguous. They read as requirements more than exceptions.
  - Consequence: this must be a **finding** (a diagnostic), never a rejection, and the message must name the exception.
  - The mono / zero-order-Ambisonics carve-out cannot apply: a `num_layers > 1` element is neither.
- **iamf-tools:** no check. `git grep -n "num_layouts\|layouts.size()"` over non-test `iamf/**/*.cc|h` finds only the read/write loops (`mix_presentation.cc:179-180, 317-319`), printing, and the CLI (`obu_processor_utils.cc:91-126`, `rendering_mix_presentation_finalizer.cc:469-470`, `mix_presentation_generator.cc:252-258` "Ignoring deprecated `num_layouts` field"). None compares against `num_layers` [VERIFIED].
- **libiamf:** no check. `num_layouts` is used only for parsing (`IAMF_OBU.c:871-890`), layout validity (`IAMF_decoder.c:1340-1350`), and loudness/layout matching (`:3359-3405`) [VERIFIED].
- **Rule to implement:** spec-only finding. The references are more permissive, so the rule is never a rejection and never a writer check.

### Rule 5: anchored loudness (spec AND iamf-tools)
- **Spec** `index.bs:1485` [VERIFIED]: "There SHALL be no duplicate values of [=anchor_element=] within one [=LoudnessInfo()=]. When an unsupported value of [=anchor_element=] is set, parsers MAY treat it as Unknown."
  - Syntax `:1442-1447`: `if (info_type & 2) { unsigned int (8) num_anchored_loudness; for (...) { unsigned int (8) anchor_element; signed int (16) anchored_loudness; } }`
  - Values `:1478-1483`: `0 : Unknown`, `1 : Dialogue`, `2 : Album`, `3~255 : Reserved for future use`.
- **iamf-tools** [VERIFIED @848c6ff4]:
  - `mix_presentation.cc:56-67 ValidateUniqueAnchorElements` casts each `anchor_element` to `uint8_t` and calls `ValidateUnique(..., "Anchored loudness types")`.
  - Read `:257-258`: `RETURN_IF_NOT_OK(ValidateUniqueAnchorElements(loudness.anchored_loudness.anchor_elements));`
  - Write `:135-136`: `MAYBE_RETURN_IF_NOT_OK(...)`. That is strict unless built with `IGNORE_ERRORS_USE_ONLY_FOR_IAMF_TEST_SUITE` (`iamf/common/utils/macros.h:29-36`), which is how `test_000063` was produced.
  - **Scope:** one LoudnessInfo, **all** anchor values including Unknown (0) and reserved (3-255), compared as raw bytes.
- **libiamf** [VERIFIED: `IAMF_OBU.c:957-959`]: `loudness[i].anchor_loudness[k].anchor_element = bs_getA8b(&b);` in a plain loop, no comparison.
- **Rule to implement:** equal (spec = iamf-tools; libiamf permissive). Compare raw `u8` within one `Loudness`. The spec's "MAY treat unsupported as Unknown" is a reader permission, not a duplicate trigger. Do not fold reserved values to 0.

## B. Crate integration (HEAD 3347a7d, read this session)

- **Model** (`src/obu/mix_presentation.rs`):
  - `MixPresentation { mix_presentation_id, annotations_language: Vec<Vec<u8>>, localized_presentation_annotations, sub_mixes, trailing }` (`:355-367`)
  - `Loudness { .., anchored: Option<AnchoredLoudness> }` (`:150-161`)
  - `AnchoredLoudness { anchor_elements: Vec<AnchorElement> }` (`:126-129`)
  - `AnchorElement { anchor_element: u8, anchored_loudness: i16 }` (`:117-122`)
  - `SubMix::num_layouts()` (`:299-301`)
  - `ScalableChannelLayoutConfig::num_layers()` (`src/obu/audio_element.rs:116-118`)
- **`MixPresentation::validate()`** (`:382-488`) reports, in this order:
  1. presentation annotation count
  2. th8 `num_sub_mixes`
  3. th8 duplicate `sub_mix.audio_element_id`
  4. per sub-mix: `SubMix::validate()`; then per element: `rendering_config.reserved`, th8 headphones, element gain, element annotation count; then output gain; then per layout: `layout.reserved`

  It has **no access to Audio Elements**, so rule 3 cannot live there.
- **Rule 3 access:**
  - `DescriptorSet::validate()` (`src/model/mod.rs:122-219`) has `audio_element_by_id` (first binding). Its reference-resolution loop is at `:182-197`.
  - `ParsedSequence::validate()` (`src/sequence.rs:195-396`) builds `audio_elements: Vec<&AudioElement>` in wire order at `:278-285` and resolves Mix Presentation references in its pass-two arm at `:305-321`.
  - Both validators can evaluate rule 3.
- **`build()`** (`src/encoder.rs:902`, `validate_declarations` `:1023-1302`). Per-presentation loop order:
  1. handle count (`:1147`)
  2. handle validity (`:1153`)
  3. `SubMixCountNotOne` (`:1163`)
  4. `ReservedHeadphonesRenderingMode` (`:1172`)
  5. `DuplicateMixPresentationAudioElement` (`:1192-1208`)
  6. handle-lowered clone (`:1215-1223`)
  7. **`validate_findings(lowered.validate())?` (`:1224`)**, which maps any finding to `InvalidDescriptorReference` at `Field("descriptors")` (`:1388-1397`)
  8. layout / extension checks (`:1225-1246`)

  New dedicated checks must sit between steps 5 and 7, or the new findings pre-empt them.
- The th8 `test_000124` hash (`795ef288…`) is already committed and is unaffected.

## C. Design (implemented on scratch; diffs at `.../scratchpad/wlv/wlv-src.diff`, `wlv-tests.diff`)

### C.1 Error kinds (`src/error.rs`, directly after `DuplicateMixPresentationAudioElement`, payload-free)
```rust
/// A Mix Presentation lists the same `annotations_language` more than once.
// ref: IAMF v1.1.0 index.bs:1273 ("The same language SHALL NOT be duplicated in this array.")
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:471-473 (write), :528-530 (read) ValidateUnique
// DISAGREEMENT: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:748-749 reads without checking; iamf-tools
// compares bytes exactly, the spec's BCP-47 tags are case-insensitive (RFC 5646 2.1.1), so the stricter
// ASCII-case-insensitive comparison is used
#[error("a Mix Presentation duplicates an annotations_language")]
DuplicateAnnotationsLanguage,
/// A `loudness_info` lists the same `anchor_element` more than once.
// ref: IAMF v1.1.0 index.bs:1485 (no duplicate anchor_element within one LoudnessInfo())
// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:56-67 ValidateUniqueAnchorElements (write :135-136, read :257-258)
// DISAGREEMENT: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:957-959 reads without checking
#[error("a loudness_info duplicates an anchor_element")]
DuplicateAnchorElement,
```
- Locations: `Field("annotations_language")` and `Field("anchored_loudness.anchor_element")`.
- Add both kinds to `tests/error_shape.rs::static_builder_errors_are_compact_typed_kinds`.
- The 32-byte assertion still compiles (payload-free) [VERIFIED: scratch build].

### C.2 `build()` (per-presentation loop, immediately after the `DuplicateMixPresentationAudioElement` block at `:1208`, before the lowered clone)
1. **Duplicate language:**
   ```rust
   let languages = &declaration.presentation.annotations_language;
   if languages.iter().enumerate().any(|(position, language)| {
       languages.iter().take(position).any(|earlier| earlier.eq_ignore_ascii_case(language))
   }) → Err(DuplicateAnnotationsLanguage, Field("annotations_language"))
   ```
2. **Duplicate anchor:** over `sub_mixes → layouts → loudness.anchored`, the same take/any scan on `anchor_element` → `Err(DuplicateAnchorElement, Field("anchored_loudness.anchor_element"))`.

Verified on scratch:
- `["en","en"]` and `["en-US","en-us"]` → `DuplicateAnnotationsLanguage`
- `["en","es"]` → Ok
- anchors `[1,1]` and `[0,0]` → `DuplicateAnchorElement`
- anchors `[1,2]` → Ok

Order, verified on scratch:
- duplicate handle + duplicate language → `DuplicateMixPresentationAudioElement`
- duplicate language + duplicate anchor + presentation-annotation count mismatch → `DuplicateAnnotationsLanguage`
- duplicate anchor + no stereo layout → `DuplicateAnchorElement` (not `InvalidDescriptorReference`)

**Rule 3: no `build()` change.** The shape is unreachable. `build_rejects_scalable_channel_layers` (`tests/encoder_builder.rs:847-858`) already locks that.

### C.3 Findings in `MixPresentation::validate()` (the text is hash-bearing for rule 5)
1. **Language:** placed right after the `localized_presentation_annotations` count check (`:384-393`) and before the th8 `num_sub_mixes` block. For each later case-insensitive duplicate, one finding:
   - `at: Field("annotations_language")`
   - `format!("mix presentation {} lists annotations_language {:?} more than once; the same language SHALL NOT be duplicated (IAMF v1.1.0 index.bs:1273)", self.mix_presentation_id, String::from_utf8_lossy(language))`
   - Rendered: `mix presentation 42 lists annotations_language "EN-US" more than once; ...`
2. **Anchor:** inside `for layout in &sub_mix.layouts`, right after `push_reserved_finding(.., "layout.reserved", width)`. For each later duplicate within that layout's `Loudness`:
   - `at: Field("anchored_loudness.anchor_element")`
   - `format!("anchor_element {} appears more than once in one loudness_info; there SHALL be no duplicate anchor_element within one LoudnessInfo() (IAMF v1.1.0 index.bs:1485)", anchor.anchor_element)`

### C.4 Rule 3 finding (shared helper; both cross-OBU validators)
- In `src/obu/mix_presentation.rs`, add `pub(crate) fn scalable_layout_finding(mix_presentation_id: u32, sub_mix: &SubMix, element: Option<&AudioElement>) -> Option<Finding>`.
  - Re-export it with `pub(crate) use mix_presentation::scalable_layout_finding;` in `src/obu/mod.rs`. The module is private.
  - `let [only] = sub_mix.elements.as_slice() else { return None };`
  - `ChannelBased(config)` else `None`; `n = num_layers()`; return `None` if `n <= 1 || num_layouts() >= n`.
  - Emit:
    - `at: Field("sub_mix.num_layouts")`
    - `format!("mix presentation {mix_presentation_id} has a sub-mix whose only audio element {} is scalable with num_layers {num_layers} but num_layouts is {}; num_layouts SHALL be >= num_layers unless the highest loudness_layout is the layout the sub-mix was authored on (IAMF v1.1.0 index.bs:1305-1309)", only.audio_element_id, sub_mix.num_layouts())`
  - Add a `// ref:` for `:1305-1309` and `:769-770`, plus a NOTE that neither reference checks it and the exception is unobservable.
- **`DescriptorSet::validate()`:** inside the `for sub_mix` loop at `src/model/mod.rs:183`, after the per-element unresolved-reference findings. Call it with `sub_mix.elements.first().and_then(|e| self.audio_element_by_id(e.audio_element_id))`.
- **`ParsedSequence::validate()`:** pass two, `SequenceObu::MixPresentation` arm (`src/sequence.rs:305`), in the same position. Look the element up with `audio_elements.iter().copied().find(|c| c.audio_element_id == e.audio_element_id)`, the first wire binding, consistent with qk3.
  - An unresolved element returns `None`; the reference finding already covers it.
- Both validators yield identical findings [VERIFIED: scratch test adding a second layer to the published 5.1 element → one finding from each, equal].
- Redundant Mix Presentation copies repeat the finding. That matches existing pass-one/pass-two behaviour.

## D. Hash impact (measured on scratch)

A scratch fixture scan over all 37 `POSITIVE_EXPECTATIONS` (`WLV` probe) found:
- **Languages:** no fixture has a duplicate, even case-insensitively. `test_000060` has `["en-us","es-mx"]`; the rest have one or zero.
- **Anchors:** `test_000062` `[[1, 2]]` (distinct) and **`test_000063` `[[1, 1]]`**. No other fixture has anchored loudness.
- **Scalable:** only `test_000059` has `num_layers = 2`, with `num_layouts = 2` (`A0_2_0`, `B0_5_0`), so no finding. Every other single-element sub-mix has `num_layers` ∈ {0 (scene/unresolved), 1}.

With the exact C.3/C.4 text, `every_positive_matches_its_complete_modeled_field_ledger` reported only:
```
WLVCHANGE test_000063.iamf old=793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24 new=a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409 findings=[Finding { at: Field("anchored_loudness.anchor_element"), message: "anchor_element 1 appears more than once in one loudness_info; there SHALL be no duplicate anchor_element within one LoudnessInfo() (IAMF v1.1.0 index.bs:1485)" }]
```

| Fixture | Old (`tests/support/reference_expectations.rs:119-122`) | New | Reason | Pinned testdata |
|---------|------|-----|--------|-----------------|
| `test_000063.iamf` | `793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24` | `a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409` | Its one layout's LoudnessInfo carries `anchor_element` 1 (Dialogue) twice. The new rule-5 finding is appended in `MixPresentation::validate()`'s layout loop. | `iamf-tools@848c6ff4 iamf/cli/testdata/test_000063.textproto:19` `is_valid: false`, `:20` `is_valid_to_decode: false`, `:16-17` "It is invalid because anchor elements must be unique.", `:126-127` two `ANCHOR_TYPE_DIALOGUE` entries [VERIFIED]. Control `test_000062.textproto:19` `is_valid: true` (DIALOGUE + ALBUM) is unchanged. |

With that hash substituted, the full `cargo test --locked --no-fail-fast` gave **28 `test result: ok`, 0 failed** (including `golden`, `parse_reference`, `conformance` SKIP paths, `error_shape`, `parallax_contract` and `fuzz_regression`). `cargo clippy --locked --lib -- -D warnings` was clean.

**The hash is valid only if the anchor finding's `Field` path, message and position are byte-identical to C.3.2.** If the executor changes them, recompute with the probe pattern: temporarily print `semantic_sha256` on mismatch. If any fixture other than `test_000063` changes, stop and report. The language and scalable message texts are not hash-bearing today, because no fixture triggers them.

## E. Recommendation

### E.1 Implementation (one plan, TDD order)
1. `src/error.rs`: two kinds (C.1), plus `tests/error_shape.rs`.
2. `src/encoder.rs`: two dedicated checks (C.2) between `:1208` and `:1209`. Rule 3 needs nothing.
3. `src/obu/mix_presentation.rs`: two findings (C.3) plus `scalable_layout_finding` (C.4). Update the `validate()` doc comment (`:376-380`) to name the two new dedicated kinds as also checked first.
4. `src/obu/mod.rs`: `pub(crate) use`.
5. `src/model/mod.rs` and `src/sequence.rs`: call the helper (C.4).
6. `tests/support/reference_expectations.rs`: `test_000063` → new hash. Put old/new and the provenance from D in the commit message.

### E.2 Tests (no indexing or bare arithmetic in tests)
- `tests/encoder_builder.rs`:
  - `build_rejects_duplicate_annotations_languages` → kind + `Field("annotations_language")`
  - `build_rejects_annotations_languages_differing_only_in_ascii_case` (`en-US`/`en-us`)
  - `distinct_annotations_languages_build` (`en`/`es`)
  - `build_rejects_duplicate_anchor_elements` (`[1,1]`, plus `[0,0]` to lock "Unknown counts too")
  - `distinct_anchor_elements_build` (`[1,2]`)
  - Order:
    - duplicate handle + duplicate language → `DuplicateMixPresentationAudioElement`
    - duplicate language + duplicate anchor + annotation-count finding → `DuplicateAnnotationsLanguage`
    - duplicate anchor + missing stereo layout → `DuplicateAnchorElement`
  - Keep `build_rejects_scalable_channel_layers`.
- `tests/sequence_parse.rs`:
  - Add a `wlv_findings` filter on the three `Field`s, like `th8_findings` at `:730`.
  - `MixPresentation::validate()` exact vectors:
    - `["en-us","EN-US","en-us"]` → two language findings, both for later entries (`"EN-US"`, then `"en-us"`)
    - anchors `[1,2,1]` → one finding
    - published presentation → `[]`
  - Rule 3 in both validators: add a second layer to `published_descriptor_set()`'s element and to `support::published_audio_element()` in a `ParsedSequence`. Expect exactly one equal `sub_mix.num_layouts` finding from each; published → `[]`.
  - Negatives:
    - two elements in the sub-mix → none
    - `num_layouts == num_layers` → none
    - unresolved element id → no scalable finding (only the reference finding)
- `tests/parse_reference.rs`: optionally assert that `test_000063`'s findings contain the anchor finding and `test_000062`'s do not. That locks the provenance independently of the hash.

### E.3 Deferred / not included
- th8 item 2 (BCP-47 conformance) and item 4 (`MixPresentationTags` presence): out of scope, unchanged. BCP-47 canonical equivalence beyond ASCII case (e.g. `en` vs `en-US`, grandfathered/redundant tag mappings) belongs with item 2.
- Rule 3 is never enforced in `build()` or on write (the shape is unreachable, and the spec exception is unobservable). If scalable layers ever become buildable, `build()` must decide on this rule then.
- th8 item 6 carry-over (Binaural `decoder_main` abort; CONF-05 FLAC/Opus) is unchanged.

## Security Domain
`security_enforcement` is enabled; V5 input validation applies. The new findings run on foreign input:
- no indexing (`as_slice()` pattern match, `.first()`, `.get` via iterators)
- no arithmetic
- no panics

The O(n²) scans are bounded:
- languages ≤ `count_label`, already bounded by `bytes_remaining()` (`read_strings`, `:605-611`)
- anchors ≤ 255 (`u8` count, `:803-813`)
- the helper allocates nothing

No new dependencies. No I/O.

## Assumptions Log
| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | "The same language" (`:1273`) includes ASCII-case variants, so the check is stricter than iamf-tools' exact bytes | A.1, C | Low. It only rejects `en-US`/`en-us` pairs, which no fixture has. If the user prefers exact bytes (= iamf-tools), change `eq_ignore_ascii_case` to `==` in both places; no hash changes. |
| A2 | Rule 3's condition is "exactly one element, channel-based, `num_layers > 1`", and all layouts count toward `num_layouts` | A.3 | Low. Diagnostic only; no fixture triggers it. |
| A3 | Rule 3's authoring-layout exception cannot be observed, so the finding may fire on a file its author considers conformant; the message names the exception | A.3, C.4 | Low to medium. It is a finding, not a rejection. Alternative: defer rule 3 entirely (no hash impact either way). |

## Open Questions
1. **Case-insensitive language equality (A1).** Recommendation: keep it (spec + RFC 5646). Confirm at plan review if the user prefers exact iamf-tools parity.

## Sources
- IAMF spec `docs/iamf` @ `v1.1.0` `index.bs:769-770, 1239-1313, 1426-1487, 3280-3290`. Also `v1.0.0:1205-1209` and `v1.0.0-errata:1210-1214` (identical rule-3 text).
- iamf-tools @ `848c6ff4968ff8cc6f728259892ab4f90cb83256` (v2.1.0):
  - `iamf/obu/mix_presentation.cc:41-67, 125-159, 228-270, 462-575`
  - `iamf/common/utils/validation_utils.h:128-150`
  - `iamf/common/utils/macros.h:20-36`
  - `iamf/cli/testdata/test_000063.textproto:14-34, 123-128`
  - `test_000062.textproto:15-20, 123-128`
  - `test_000059.textproto:15-20`
  - `git grep num_layouts|num_layers` over `iamf/**`
- libiamf @ `f06e919e` (v1.1.0):
  - `code/src/iamf_dec/IAMF_OBU.c:715-760, 871-890, 940-965`
  - `IAMF_decoder.c:1336-1350, 3350-3411`
  - `git grep anchor|language|num_layouts`
- RFC 5646 §2.1.1 (rfc-editor.org) — case insensitivity.
- Crate (HEAD 3347a7d):
  - `src/obu/mix_presentation.rs:115-161, 290-498, 507-620, 787-843`
  - `src/obu/audio_element.rs:92-118`
  - `src/encoder.rs:851-1302, 1388-1397, 1479-1545`
  - `src/error.rs:150-204, 282`
  - `src/model/mod.rs:122-219`
  - `src/sequence.rs:195-396`
  - `tests/parse_reference.rs:91-128`
  - `tests/support/reference_expectations.rs:107-122`
  - `tests/error_shape.rs:166-190`
  - `tests/encoder_builder.rs:281-414, 847-858, 1534-1653`
  - `tests/sequence_parse.rs:730-985`
- Scratch evidence (`/private/tmp/claude-501/-Users-cell-local-iamf-rs/ffcacbe6-b061-4445-a28e-fe0219356ad0/scratchpad/wlv/`): `wlv-src.diff`, `wlv-tests.diff` (probe tests use indexing and must be rewritten), `full.log`.

## Metadata
- Rules and references: HIGH (read at pinned revisions).
- Integration design: HIGH (implemented; full suite green on scratch).
- Hash: HIGH, conditional on the exact C.3.2 text and position.
- Valid until the next change to `MixPresentation::validate()`, the two cross-OBU validators, or the reference corpus.
