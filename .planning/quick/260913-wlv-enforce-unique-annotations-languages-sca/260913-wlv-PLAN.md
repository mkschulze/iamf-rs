---
phase: quick-260913-wlv
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/error.rs
  - src/encoder.rs
  - src/obu/mix_presentation.rs
  - src/obu/mod.rs
  - src/model/mod.rs
  - src/sequence.rs
  - tests/error_shape.rs
  - tests/encoder_builder.rs
  - tests/sequence_parse.rs
  - tests/parse_reference.rs
  - tests/support/reference_expectations.rs
  - HANDOFF.md
  - .planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-deferred-items.md
autonomous: true
requirements: [API-01, API-04, DESC-05, PARSE-07, CONF-06]

estimate:
  tokens: 95000
  raw_tokens: 95000
  tasks: 3
  confidence: low

must_haves:
  truths:
    - "Per the locked orchestrator decision (th8 items 1 and 5): build() rejects a Mix Presentation whose annotations_language list holds two entries equal under ASCII case folding with ErrorKind::DuplicateAnnotationsLanguage at Location::Field(\"annotations_language\"), and one whose LoudnessInfo repeats a raw anchor_element byte (including 0 Unknown) with ErrorKind::DuplicateAnchorElement at Location::Field(\"anchored_loudness.anchor_element\")"
    - "Both dedicated checks run after DuplicateMixPresentationAudioElement and before validate_findings(lowered.validate()): a duplicate handle wins over a duplicate language, a duplicate language wins over a duplicate anchor and an annotation-count finding, and a duplicate anchor wins over a missing stereo layout"
    - "MixPresentation::validate() reports each later case-insensitive duplicate language (after the presentation-annotation count check, before num_sub_mixes) and each later duplicate anchor_element per layout (after layout.reserved), with the exact research C.3 texts"
    - "Per the locked decision (open question 2): DescriptorSet::validate() and ParsedSequence::validate() report an identical Field(\"sub_mix.num_layouts\") finding when a sub-mix's only element is channel-based with num_layers > 1 and num_layouts < num_layers, via one pub(crate) helper; the message names the authoring-layout exception; build() and the writers are unchanged for this rule"
    - "Only test_000063.iamf's semantic_sha256 changes, 793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24 -> a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409, recorded in the commit body; no other reference expectation, golden fixture or DIFF-LEDGER.md byte changes"
    - "size_of::<Error>() <= 32 still holds; fmt, clippy with and without fuzzing, full cargo test --locked, doc tests, fuzz replay and conformance with IAMF_REF_DECODER (test result: ok, CONF-06 lines present, only phase3 FLAC/Opus CONF-05 SKIPs) pass"
  artifacts:
    - path: "src/error.rs"
      provides: "payload-free ErrorKind::DuplicateAnnotationsLanguage and ErrorKind::DuplicateAnchorElement directly after DuplicateMixPresentationAudioElement"
      contains: "DuplicateAnchorElement"
    - path: "src/encoder.rs"
      provides: "two dedicated build() checks between the duplicate-handle block and the lowered clone"
      contains: "DuplicateAnnotationsLanguage"
    - path: "src/obu/mix_presentation.rs"
      provides: "language and anchor findings in MixPresentation::validate(); pub(crate) fn scalable_layout_finding"
      contains: "index.bs:1305-1309"
    - path: "tests/support/reference_expectations.rs"
      provides: "test_000063.iamf new semantic_sha256"
      contains: "a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409"
    - path: ".planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-deferred-items.md"
      provides: "remaining items (BCP-47, MixPresentationTags, rule 3 build() stance, A1 parity note, carried th8 item 6) and the th8 citation correction"
  key_links:
    - from: "src/encoder.rs validate_declarations per-presentation loop"
      to: "ErrorKind::DuplicateAnnotationsLanguage / DuplicateAnchorElement"
      via: "checks placed after the DuplicateMixPresentationAudioElement return and before `let mut lowered`"
      pattern: "DuplicateAnchorElement"
    - from: "src/model/mod.rs DescriptorSet::validate and src/sequence.rs ParsedSequence::validate"
      to: "src/obu/mix_presentation.rs scalable_layout_finding"
      via: "crate::obu::scalable_layout_finding re-exported pub(crate) from src/obu/mod.rs"
      pattern: "scalable_layout_finding"
    - from: "tests/parse_reference.rs semantic_sha256"
      to: "MixPresentation::validate anchor finding text and position"
      via: "findings Debug is hashed; the text must be byte-identical to research C.3.2"
      pattern: "index.bs:1485"
---

<objective>
Resolve th8 deferred items 1, 3 and 5 (user-approved bundle, 2026-09-13; items 2 BCP-47 and 4
MixPresentationTags stay out of scope):

1. Unique `annotations_language` (IAMF v1.1.0 `index.bs:1273`), compared ASCII-case-insensitively (locked:
   open question 1, RFC 5646 §2.1.1; stricter than iamf-tools' exact bytes, so it satisfies both).
2. `num_layouts >= num_layers` for a sub-mix whose only element is scalable channel audio (`index.bs:1305-1309`),
   as a parse-side FINDING ONLY (locked: open question 2), never a build() rejection.
3. Unique `anchor_element` within one LoudnessInfo (`index.bs:1485`; also iamf-tools read/write).

Purpose: build() today accepts files iamf-tools refuses to read (rules 1 and 5), and parse-side validation
is silent on all three rules.

Output: two payload-free error kinds with build() rejections, three kinds of findings, a shared pub(crate)
helper, one explained reference-hash change, tests, HANDOFF.md wording, and a deferred-items record.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@tests/CLAUDE.md
@.planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-RESEARCH.md
@.planning/quick/260913-qk3-enforce-iamf-v1-1-0-profile-restrictions/260913-qk3-CONTEXT.md

Base commit: `3347a7d`. Interfaces verified this session (do not re-explore):

- `src/error.rs:164-169`: `NoMixPresentation`, then `DuplicateMixPresentationAudioElement` with `// ref:` and
  `// DISAGREEMENT:` comment lines above `#[error(..)]`. `const _: () = assert!(size_of::<Error>() <= 32);`.
- `src/encoder.rs` per-presentation loop in `validate_declarations`: the duplicate-handle `if` returns
  `DuplicateMixPresentationAudioElement` at `Field("mix_presentation.audio_elements")` (ends ~line 1208); then a
  comment block and `let mut lowered = declaration.presentation.clone();` (~1215); then
  `validate_findings(lowered.validate())?;` (1224), which maps any finding to `InvalidDescriptorReference` at
  `Field("descriptors")` (1388-1397). The duplicate-handle check uses the `iter().enumerate().any(.. take(position).any(..))` pattern.
- `src/obu/mix_presentation.rs`: `AnchorElement { pub anchor_element: u8, pub anchored_loudness: i16 }` (117),
  `AnchoredLoudness { pub anchor_elements: Vec<AnchorElement> }` (126), `Loudness.anchored: Option<AnchoredLoudness>` (158),
  `SubMix::num_layouts(&self) -> usize` (299), `MixPresentation.annotations_language: Vec<Vec<u8>>`;
  `MixPresentation::validate()` doc comment at 376-380 names the dedicated kinds build() checks first; the
  presentation-annotation count check is at 384-393; inside `for layout in &sub_mix.layouts` the last statement is
  `push_reserved_finding(&mut findings, layout.reserved, "layout.reserved", width);`; `fn push_reserved_finding` follows `impl`.
- `src/obu/audio_element.rs`: `ScalableChannelLayoutConfig { pub layers: Vec<ChannelAudioLayerConfig> }`,
  `num_layers(&self) -> usize` (116); `AudioElementType::ChannelBased(config)` with `config.scalable_channel_layout`.
- `src/obu/mod.rs`: private `mod mix_presentation;` and a `pub use mix_presentation::{AnchorElement, AnchoredLoudness, ..};` block.
- `src/model/mod.rs:182-197`: `for presentation in &self.mix_presentations { for sub_mix in &presentation.sub_mixes { for element in &sub_mix.elements { unresolved audio_element_id finding } } }`;
  `pub fn audio_element_by_id(&self, id: u32) -> Option<&AudioElement>` (104).
- `src/sequence.rs:305-321`: pass-two `SequenceObu::MixPresentation(obu)` arm with the same per-element unresolved loop over
  `audio_elements` (a `Vec<&AudioElement>` in wire order); id is `obu.payload.mix_presentation_id`.
- `tests/encoder_builder.rs`: helpers `lpcm_config()`, `stereo_element()`, `add_fresh_element(&mut builder, codec, element)`,
  `presentation_for_elements(&[ids], element_param_id, output_param_id)` (MP 7, languages `["en"]`, one sub-mix,
  one A0_2_0 layout, element annotations `["bed"]`); th8 order tests at 281-414 show the assertion style
  (`expect_err("..")`, `error.kind()`, `error.at()`); `build_rejects_scalable_channel_layers` at 847. Imports at 6-12
  (add `AnchorElement`, `AnchoredLoudness`, `SoundSystem` if not yet imported).
- `tests/sequence_parse.rs`: `th8_findings` filter at 730 and finding constructors `dup/sub0/subn/hp`; the both-validators
  pattern at 860-882 (`published_descriptor_set()`, `set.mix_presentations = vec![..]`, `ParsedSequence { obus: vec![SequenceObu::IaSequenceHeader(support::published_sequence_header()), SequenceObu::CodecConfig(support::published_codec_config()), SequenceObu::AudioElement(support::published_audio_element()), SequenceObu::MixPresentation(..)] }`).
  Published MP id 42, one sub-mix, one element id 300 (stereo, single layer), one A0_2_0 layout.
- `tests/parse_reference.rs`: `#![allow(clippy::expect_used, clippy::panic)]`, `fixture_bytes("test_000063.iamf")`,
  `parse_sequence`, imports `iamf::error::Location`; `semantic_sha256` hashes `format!("{sequence:#?}\nfindings={:#?}", sequence.validate())`.
- `tests/support/reference_expectations.rs:119-122`: `positive!("test_000063.iamf", "793dcdaf…aba24")`.
- `HANDOFF.md:39-42`: step 2 sentence listing build() rejections ending `(`DuplicateMixPresentationAudioElement`).`
- Scratch diffs `/private/tmp/claude-501/-Users-cell-local-iamf-rs/ffcacbe6-b061-4445-a28e-fe0219356ad0/scratchpad/wlv/wlv-src.diff`
  (usable as a starting point for `src/`) and `wlv-tests.diff` (probe tests with `[0]` indexing, `unwrap` and `println!`: do NOT copy; write the named tests below).

Project constraints: Rust 1.85 (no let-chains; nested `if let`, `let .. else` is fine). No `unwrap`/`expect`/indexing/bare
arithmetic in `src/`; in tests, `clippy::indexing_slicing` and `arithmetic_side_effects` apply too (use `.first_mut()`,
iterators, `vec![x; n]`), and `unwrap`/`expect` only inside `#[test]` fns. No `HashMap`/`HashSet`. Payload-free unit
error variants. Every reference-derived rule carries `// ref:` naming the pinned revision (iamf-tools@v2.1.0,
libiamf@v1.1.0). Commit by explicit path only; never `git add -A`; the untracked `docs/` clones and
`docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` stay untracked and unedited.
</context>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: End-to-end build() rejection of duplicate annotations languages and anchor elements</name>
  <files>src/error.rs, src/encoder.rs, tests/error_shape.rs, tests/encoder_builder.rs, HANDOFF.md</files>
  <behavior>
    - languages ["en","en"] on presentation_for_elements(&[0], 100, 110) (presentation and element annotation counts matched) → DuplicateAnnotationsLanguage at Field("annotations_language")
    - languages ["en-US","en-us"] → DuplicateAnnotationsLanguage at Field("annotations_language") (ASCII case folded, locked open question 1)
    - languages ["en","es"] → build() Ok
    - anchors [1,1] on the layout's loudness → DuplicateAnchorElement at Field("anchored_loudness.anchor_element"); anchors [0,0] → the same (Unknown counts)
    - anchors [1,2] → build() Ok
    - Order: two handles of one element (presentation_for_elements(&[0,0],..), vec![element, element]) plus languages ["en","en"] → DuplicateMixPresentationAudioElement
    - Order: languages ["en","en"] plus anchors [1,1] plus one presentation annotation removed (count mismatch finding) → DuplicateAnnotationsLanguage
    - Order: anchors [2,2] with the only layout changed to Layout::SoundSystem(SoundSystem::B0_5_0) (no stereo layout finding) → DuplicateAnchorElement; control with anchors [1,2] and B0_5_0 → InvalidDescriptorReference at Field("descriptors")
    - Both kinds listed in static_builder_errors_are_compact_typed_kinds; size_of::<Error>() <= 32 still compiles
  </behavior>
  <action>
    RED first: add the tests below and the two kinds to `tests/error_shape.rs`; confirm they fail to compile, then implement.

    `src/error.rs` (locked decision; payload-free): directly after `DuplicateMixPresentationAudioElement`, add
    `DuplicateAnnotationsLanguage` with doc "A Mix Presentation lists the same `annotations_language` more than once.",
    `#[error("a Mix Presentation duplicates an annotations_language")]`, and comment lines:
    `// ref: IAMF v1.1.0 index.bs:1273 ("The same language SHALL NOT be duplicated in this array.")`,
    `// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:471-473 (write), :528-530 (read) ValidateUnique`,
    and a DISAGREEMENT line: libiamf@v1.1.0 `code/src/iamf_dec/IAMF_OBU.c:748-749` reads without checking; iamf-tools
    compares bytes exactly; BCP-47 tags are case-insensitive (RFC 5646 2.1.1), so the stricter ASCII-case-insensitive
    comparison is used. Then `DuplicateAnchorElement` with doc "A `loudness_info` lists the same `anchor_element` more than once.",
    `#[error("a loudness_info duplicates an anchor_element")]`, comment lines
    `// ref: IAMF v1.1.0 index.bs:1485 (no duplicate anchor_element within one LoudnessInfo())`,
    `// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:56-67 ValidateUniqueAnchorElements (write :135-136, read :257-258)`,
    `// DISAGREEMENT: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:957-959 reads without checking`.
    (Research C.1 has this exact shape.)

    `src/encoder.rs` (locked placement): immediately after the `DuplicateMixPresentationAudioElement` return block and
    before the "Templates carry placeholder ids" comment / `let mut lowered`, add two checks on
    `declaration.presentation` (the template; languages and anchors are not lowered):
    1. Duplicate language: the enumerate/take(position)/any scan over `annotations_language` using
       `eq_ignore_ascii_case`; on a hit return `Error::new(ErrorKind::DuplicateAnnotationsLanguage, Location::Field("annotations_language"))`.
       Comment: `// ref: IAMF v1.1.0 index.bs:1273; iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:471-473` plus "ASCII-case-insensitive (RFC 5646 2.1.1), stricter than iamf-tools".
    2. Duplicate anchor: over `sub_mixes` flat_map `layouts`, filter_map `loudness.anchored.as_ref()`, any anchored list
       with the same scan comparing raw `anchor_element` bytes (do not fold reserved values to 0); return
       `Error::new(ErrorKind::DuplicateAnchorElement, Location::Field("anchored_loudness.anchor_element"))`.
       Comment: `// ref: IAMF v1.1.0 index.bs:1485; iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:135-136`.
    Rule 3 needs no build() change: `validate_element_topology` already rejects `num_layers != 1`
    (`build_rejects_scalable_channel_layers` stays unchanged).

    `HANDOFF.md` step 2: extend the rejection sentence so it also names a Presentation that repeats an
    `annotations_language` (compared ASCII-case-insensitively, `DuplicateAnnotationsLanguage`) or repeats an
    `anchor_element` within one loudness info (`DuplicateAnchorElement`). Scoped Edit only.

    `tests/error_shape.rs`: append `ErrorKind::DuplicateAnnotationsLanguage` and `ErrorKind::DuplicateAnchorElement`
    after `ErrorKind::DuplicateMixPresentationAudioElement` in `static_builder_errors_are_compact_typed_kinds`.

    `tests/encoder_builder.rs`: add non-test helpers without indexing/expect:
    `fn presentation_with_languages(languages: &[&[u8]]) -> MixPresentation` (start from
    `presentation_for_elements(&[0], 100, 110)`, set `annotations_language`, set `localized_presentation_annotations`
    to `vec![b"stereo".to_vec(); languages.len()]`, and every sub-mix element's `localized_element_annotations` to
    `vec![b"bed".to_vec(); languages.len()]` via `for` loops over `&mut`), `fn set_anchors(presentation: &mut MixPresentation, anchors: &[u8])`
    (every layout's `loudness.anchored = Some(AnchoredLoudness { anchor_elements: anchors mapped to AnchorElement { anchor_element, anchored_loudness: 0 } })`),
    and `fn build_single(presentation: MixPresentation) -> iamf::Result<(iamf::encoder::Encoder, iamf::encoder::IdManifest)>`
    (fresh builder, `lpcm_config()`, one `stereo_element()` via `add_fresh_element`, `add_mix_presentation(vec![element], presentation)`, `build()`).
    Tests named exactly: `build_rejects_duplicate_annotations_languages`,
    `build_rejects_annotations_languages_differing_only_in_ascii_case`, `distinct_annotations_languages_build`,
    `build_rejects_duplicate_anchor_elements` (covers [1,1] and [0,0]), `distinct_anchor_elements_build`,
    `a_duplicate_handle_wins_over_a_duplicate_annotations_language`,
    `a_duplicate_annotations_language_wins_over_a_duplicate_anchor_and_a_presentation_finding`,
    `a_duplicate_anchor_element_wins_over_a_presentation_finding` (with the [1,2] control). Each rejection asserts
    kind and location; mutate layouts/annotations through `.first_mut()`/`for` loops, never `[0]`; remove one
    presentation annotation with `.pop()`.

    Commit by explicit path: `feat(quick-260913-wlv): reject duplicate annotations languages and anchor elements in build()`.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo test --locked --test encoder_builder && cargo test --locked --test error_shape && for t in build_rejects_duplicate_annotations_languages build_rejects_annotations_languages_differing_only_in_ascii_case distinct_annotations_languages_build build_rejects_duplicate_anchor_elements distinct_anchor_elements_build a_duplicate_handle_wins_over_a_duplicate_annotations_language a_duplicate_annotations_language_wins_over_a_duplicate_anchor_and_a_presentation_finding a_duplicate_anchor_element_wins_over_a_presentation_finding build_rejects_scalable_channel_layers; do grep -q "fn $t" tests/encoder_builder.rs || exit 1; done && grep -q 'ErrorKind::DuplicateAnnotationsLanguage' tests/error_shape.rs && grep -q 'ErrorKind::DuplicateAnchorElement' tests/error_shape.rs && grep -q 'eq_ignore_ascii_case' src/encoder.rs && grep -q 'index.bs:1485' src/error.rs && grep -q 'IAMF_OBU.c:748-749' src/error.rs && grep -q 'DuplicateAnchorElement' HANDOFF.md && cargo test --locked --lib && cargo clippy --locked --all-targets -- -D warnings</automated>
  </verify>
  <done>Both kinds exist, build() rejects duplicate languages (case-folded) and duplicate raw anchor bytes at the locked position, the three order tests and the controls pass, error_shape lists both kinds, HANDOFF.md names them, clippy is clean, and the commit is made by explicit path.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Language and anchor findings in MixPresentation::validate() with the test_000063 hash change</name>
  <files>src/obu/mix_presentation.rs, tests/sequence_parse.rs, tests/parse_reference.rs, tests/support/reference_expectations.rs</files>
  <behavior>
    - Published MP 42 with languages ["en-us","EN-US","en-us"] (annotation counts matched to 3) → wlv findings exactly [lang(42,"EN-US"), lang(42,"en-us")]
    - Published MP 42 with anchors [1,2,1] on its layout → exactly [anchor(1)]; anchors [0,0] → [anchor(0)]; anchors [1,2] and [3,4] → [] (reserved values are not folded)
    - Both duplicates together → [lang(..), anchor(..)] in that order
    - Published MP → no findings (published_mix_presentation_has_no_findings stays green)
    - DescriptorSet::validate() and ParsedSequence::validate() both carry the language and anchor findings unchanged
    - test_000063.iamf validate() contains exactly one Field("anchored_loudness.anchor_element") finding, anchor(1); test_000062.iamf contains none
    - every_positive_matches_its_complete_modeled_field_ledger: the only mismatch before updating is test_000063 with computed value a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409
  </behavior>
  <action>
    RED first: add the tests below, run `cargo test --locked --test sequence_parse` and `--test parse_reference`, and
    confirm the new tests fail while every_positive still passes (it changes only after the implementation).

    `src/obu/mix_presentation.rs` `MixPresentation::validate()` (texts are hash-bearing; they must be byte-identical to research C.3):
    1. Right after the `localized_presentation_annotations` count check and before the th8 `num_sub_mixes` block: for each
       `(position, language)` in `annotations_language`, if any of the first `position` entries is `eq_ignore_ascii_case`,
       push `Finding { at: Location::Field("annotations_language"), message }` with message built by
       `format!` from the text `mix presentation {} lists annotations_language {:?} more than once; the same language SHALL NOT be duplicated (IAMF v1.1.0 index.bs:1273)`
       with arguments `self.mix_presentation_id` and `String::from_utf8_lossy(language)`. Comments: `// ref: IAMF v1.1.0 index.bs:1273`,
       `// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:528-530 (read), :471-473 (write)`, and the DISAGREEMENT line (libiamf does not check; ASCII-case-insensitive per RFC 5646 2.1.1, stricter than iamf-tools' exact bytes).
    2. Inside `for layout in &sub_mix.layouts`, right after the `push_reserved_finding(.., "layout.reserved", width)` call:
       `if let Some(anchored) = layout.loudness.anchored.as_ref()`, then for each later duplicate raw `anchor_element`
       push `Finding { at: Location::Field("anchored_loudness.anchor_element"), message }` with message from the text
       `anchor_element {} appears more than once in one loudness_info; there SHALL be no duplicate anchor_element within one LoudnessInfo() (IAMF v1.1.0 index.bs:1485)`
       and argument `anchor.anchor_element`. Comments: `// ref: IAMF v1.1.0 index.bs:1485`,
       `// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:56-67 (read :257-258, write :135-136)`,
       `// DISAGREEMENT: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:957-959 does not check`.
       When splitting a long string literal across lines use a trailing backslash so the joined text has single spaces exactly as above.
    3. Update the `validate()` doc comment (376-380) to name `DuplicateAnnotationsLanguage` and `DuplicateAnchorElement`
       among the dedicated kinds build() checks first.

    `tests/sequence_parse.rs`: add `fn wlv_findings(findings: Vec<Finding>) -> Vec<Finding>` filtering
    `Location::Field("annotations_language" | "anchored_loudness.anchor_element" | "sub_mix.num_layouts")` (keeps order);
    constructors `fn lang(mix_presentation_id: u32, language: &str) -> Finding` (message via `format!` with
    `{mix_presentation_id}` and `{language:?}`, same text as src) and `fn anchor(value: u8) -> Finding`; a helper
    `fn presentation_with(languages: &[&[u8]], anchors: Option<&[u8]>) -> MixPresentation` over
    `support::published_mix_presentation().payload` using `for` loops (no indexing). Tests named exactly:
    `mix_presentation_validate_reports_each_later_case_insensitive_duplicate_language`,
    `mix_presentation_validate_reports_duplicate_anchor_elements_per_loudness_info`,
    `mix_presentation_language_findings_precede_anchor_findings`,
    `both_validators_carry_the_language_and_anchor_findings` (DescriptorSet via `published_descriptor_set()` with
    `set.mix_presentations = vec![..]`; ParsedSequence with the published header, codec config, audio element and the
    modified presentation in an `Obu`; both equal `vec![lang(42, "EN-US"), anchor(1)]` for languages ["en-us","EN-US"] and anchors [1,1]).

    `tests/parse_reference.rs`: import `iamf::error::Finding`; add
    `test_000063_reports_its_duplicate_anchor_element_and_test_000062_does_not`: parse each fixture with
    `parse_sequence(&fixture_bytes(..))`, filter `validate()` to `Location::Field("anchored_loudness.anchor_element")`,
    assert 063 equals a one-element vec with the exact anchor-1 finding and 062 is empty. Add a comment citing
    `iamf-tools@v2.1.0 iamf/cli/testdata/test_000063.textproto:16-20` (`is_valid: false`, "anchor elements must be unique").

    `tests/support/reference_expectations.rs` (locked): run `cargo test --locked --test parse_reference every_positive_matches_its_complete_modeled_field_ledger`.
    It must fail on `test_000063.iamf` with computed value exactly `a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409`.
    Only then replace `793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24` with it. If the computed value
    differs, or any other fixture mismatches (positive or negative), STOP and report; never adopt a different hash and
    never edit other expectations.

    Commit by explicit path: `feat(quick-260913-wlv): report duplicate annotations languages and anchor elements in MixPresentation validation`,
    body including the exact line
    `test_000063.iamf semantic_sha256: 793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24 -> a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409`
    and a reason paragraph: its one layout's LoudnessInfo carries anchor_element 1 (Dialogue) twice; the new index.bs:1485
    finding is appended in the layout loop; iamf-tools@v2.1.0 iamf/cli/testdata/test_000063.textproto:19-20 marks it
    is_valid: false, is_valid_to_decode: false ("anchor elements must be unique"); control test_000062 (Dialogue + Album,
    is_valid: true) is unchanged; no other reference expectation changed.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && cargo test --locked --test sequence_parse && cargo test --locked --test parse_reference && cargo test --locked --test encoder_builder && for t in mix_presentation_validate_reports_each_later_case_insensitive_duplicate_language mix_presentation_validate_reports_duplicate_anchor_elements_per_loudness_info mix_presentation_language_findings_precede_anchor_findings both_validators_carry_the_language_and_anchor_findings; do grep -q "fn $t" tests/sequence_parse.rs || exit 1; done && grep -q 'fn test_000063_reports_its_duplicate_anchor_element_and_test_000062_does_not' tests/parse_reference.rs && mkdir -p target && git diff -U0 3347a7d -- tests/support/reference_expectations.rs > target/wlv-expectations.diff && D=$(grep -E '^[-+][^-+]' target/wlv-expectations.diff) && [ "$(printf '%s\n' "$D" | wc -l | tr -d ' ')" = 2 ] && printf '%s\n' "$D" | grep -q '^-.*793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24' && printf '%s\n' "$D" | grep -q '^+.*a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409' && grep -q 'index.bs:1273' src/obu/mix_presentation.rs && grep -q 'IAMF_OBU.c:957-959' src/obu/mix_presentation.rs && cargo clippy --locked --all-targets -- -D warnings</automated>
  </verify>
  <done>MixPresentation::validate() reports later case-insensitive duplicate languages and later duplicate anchor bytes at the research positions with the exact texts; both validators carry them; test_000063 has the anchor finding and test_000062 none; the only expectation diff is test_000063's old -> new hash; clippy is clean; committed by explicit path with the hash line and reason in the body.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 3: Scalable num_layouts finding in both cross-OBU validators, deferred items, and all release gates</name>
  <files>src/obu/mix_presentation.rs, src/obu/mod.rs, src/model/mod.rs, src/sequence.rs, tests/sequence_parse.rs, .planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-deferred-items.md</files>
  <behavior>
    - Published DescriptorSet with a second (cloned) layer on element 300 → wlv findings exactly [scalable(42, 300, 2, 1)]
    - ParsedSequence with the same two-layer element and the published MP → the identical vec
    - Published set / published sequence (num_layers 1) → []
    - Two layers and a second (cloned) layout in the sub-mix (num_layouts == num_layers) → []
    - Two layers but the sub-mix references two elements (second element cloned) → []
    - Two layers but the sub-mix element id changed to 999 (unresolved) → no wlv finding, while validate() still contains a Field("audio_element_id") finding
  </behavior>
  <action>
    RED first: add the tests below and confirm the positive cases fail, then implement.

    `src/obu/mix_presentation.rs` (locked: finding only, shared helper): after the `impl MixPresentation` block add
    `pub(crate) fn scalable_layout_finding(mix_presentation_id: u32, sub_mix: &SubMix, element: Option<&crate::obu::AudioElement>) -> Option<Finding>`:
    `let [only] = sub_mix.elements.as_slice() else { return None };`, then `let crate::obu::AudioElementType::ChannelBased(config) = &element?.audio_element_type else { return None };`,
    `num_layers = config.scalable_channel_layout.num_layers()`, return `None` if `num_layers <= 1 || sub_mix.num_layouts() >= num_layers`,
    else `Some(Finding { at: Location::Field("sub_mix.num_layouts"), message })` with message from the text
    `mix presentation {mix_presentation_id} has a sub-mix whose only audio element {} is scalable with num_layers {num_layers} but num_layouts is {}; num_layouts SHALL be >= num_layers unless the highest loudness_layout is the layout the sub-mix was authored on (IAMF v1.1.0 index.bs:1305-1309)`
    with arguments `only.audio_element_id` and `sub_mix.num_layouts()`. Doc: the single-scalable-channel-element rule for one
    sub-mix; needs the referenced Audio Element, so the two cross-OBU validators call it rather than `MixPresentation::validate`.
    Comments: `// ref: IAMF v1.1.0 index.bs:1305-1309 (num_layouts SHALL be >= num_layers, with exceptions)`,
    `// ref: IAMF v1.1.0 index.bs:769-770 (scalable channel audio means num_layers > 1)`, and a NOTE that neither
    iamf-tools@v2.1.0 nor libiamf@v1.1.0 checks it (spec is stricter and wins), and the authoring-layout exception
    (index.bs:1308) is not observable in the bitstream, so it is a finding, never a rejection.

    `src/obu/mod.rs`: after the `pub use mix_presentation::{..};` block add `pub(crate) use mix_presentation::scalable_layout_finding;`.

    `src/model/mod.rs` `DescriptorSet::validate()`: inside `for sub_mix in &presentation.sub_mixes`, after the inner
    per-element unresolved loop, compute `sub_mix.elements.first().and_then(|element| self.audio_element_by_id(element.audio_element_id))`
    and `findings.extend(crate::obu::scalable_layout_finding(presentation.mix_presentation_id, sub_mix, only))`, with `// ref: IAMF v1.1.0 index.bs:1305-1309`.

    `src/sequence.rs` `ParsedSequence::validate()` pass-two `SequenceObu::MixPresentation(obu)` arm: same position, lookup
    `audio_elements.iter().copied().find(|candidate| candidate.audio_element_id == element.audio_element_id)` (first wire
    binding, consistent with qk3), and extend with `obu.payload.mix_presentation_id`. Comment `// ref: IAMF v1.1.0 index.bs:1305-1309 (first wire binding of the element id)`.

    `tests/sequence_parse.rs`: `fn scalable(mix_presentation_id: u32, audio_element_id: u32, num_layers: usize, num_layouts: usize) -> Finding`
    (same text); `fn with_second_layer(element: &mut AudioElement)` using `if let AudioElementType::ChannelBased(config)` and
    cloning `layers.first()` via `if let Some(layer) = .. .cloned()` (no expect in non-test fns). Tests named exactly:
    `both_validators_report_num_layouts_below_num_layers_for_one_scalable_element` (includes the published `[]` controls),
    `num_layouts_equal_to_num_layers_has_no_scalable_finding`, `a_sub_mix_with_two_elements_has_no_scalable_finding`,
    `an_unresolved_scalable_element_reference_has_no_scalable_finding`. Use `.first_mut()` and `for` loops; `unwrap` only inside `#[test]` fns.

    Commit by explicit path: `feat(quick-260913-wlv): report num_layouts below num_layers for a single scalable channel element`.

    Write `260913-wlv-deferred-items.md` in the th8/vvx style (heading, "None of the numbered items is implemented in this
    task", numbered sections with Source / Current state / Why out of scope):
    - Opening "Resolved by this task: 260913-th8 deferred items 1, 3 and 5" with one bullet each (kinds, locations, finding placement, test_000063 hash).
    - "Correction to 260913-th8": item 1 and item 2 cite `index.bs:1272`; the rule is at `index.bs:1273`. Item 5 is not
      reference-only: it is a spec SHALL at `index.bs:1485`, matched by iamf-tools. Item 3's rule text spans `index.bs:1305-1309`. (Do not edit th8 files.)
    - 1. BCP-47 conformance of `annotations_language` (th8 item 2, `index.bs:1273`), including canonical equivalence beyond ASCII case (`en` vs `en-US`, grandfathered/redundant tags).
    - 2. `MixPresentationTags` presence (th8 item 4, `index.bs:1313`); bytes stay in `trailing`.
    - 3. Rule 3 in build() and writers: never enforced; the shape is unreachable (`validate_element_topology` rejects `num_layers != 1`) and the authoring-layout exception is unobservable; the nesting of `:1308-1309` is ambiguous; revisit if scalable layers become buildable. Redundant Mix Presentation copies repeat the finding, as other pass-two findings do.
    - 4. Assumption A1 parity: language equality is ASCII-case-insensitive (stricter than iamf-tools' exact bytes); switching to exact bytes means changing `eq_ignore_ascii_case` to `==` in `src/encoder.rs` and `src/obu/mix_presentation.rs`, no hash change.
    - 5. Carried from th8 item 6: Binaural rendering abort in `decoder_main` unchanged; phase3 FLAC/Opus CONF-05 is a host-dependent SKIP per 260913-vcc.
    Commit by explicit path: `docs(quick-260913-wlv): record deferred mix presentation items`.

    Gates, from /Users/cell/local/iamf-rs, logs under `target/`:
    1. `cargo fmt --all -- --check`
    2. `cargo clippy --locked --all-targets -- -D warnings`
    3. `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`
    4. `cargo test --locked` to `target/wlv-full.log` (stdout+stderr), then append `full_exit=<code>`
    5. `cargo test --locked --doc`
    6. `cargo test --locked --features fuzzing --test fuzz_regression`
    Conformance, bounded (no `timeout` binary; perl at `/usr/bin/perl`). Wrapper W:
    `perl -e 'my $s=shift; my $p=fork; if(!$p){setpgrp(0,0); exec @ARGV; exit 127} $SIG{ALRM}=sub{kill "KILL", -$p; print STDERR "TIMED OUT after $s s\n"; exit 124}; alarm $s; waitpid($p,0); exit($? >> 8)' SECONDS CMD...`
    7. `cargo test --locked --test conformance --no-run`
    8. Probe `docker image inspect iamf-tools:v2.1.0` under W with 60 s. If it does not exit 0, STOP and report: CONF-06 is required and must not be reported as passed when skipped.
    9. Under W with 1200 s: `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec cargo test --locked --test conformance -- --nocapture`, stdout+stderr to `target/wlv-conformance.log`, then append `conf_exit=<code>`.
    If fmt or clippy needs a fix, add a follow-up commit by explicit path `style(quick-260913-wlv): ...`; amend nothing.
    If a gate fails for a reason unrelated to this change, stop and report the log excerpt; do not edit fixtures, goldens,
    DIFF-LEDGER.md, other reference expectations or conformance code.
  </action>
  <verify>
    <automated>cd /Users/cell/local/iamf-rs && for t in both_validators_report_num_layouts_below_num_layers_for_one_scalable_element num_layouts_equal_to_num_layers_has_no_scalable_finding a_sub_mix_with_two_elements_has_no_scalable_finding an_unresolved_scalable_element_reference_has_no_scalable_finding; do grep -q "fn $t" tests/sequence_parse.rs || exit 1; done && grep -q 'pub(crate) fn scalable_layout_finding' src/obu/mix_presentation.rs && grep -q 'scalable_layout_finding' src/model/mod.rs && grep -q 'scalable_layout_finding' src/sequence.rs && grep -q 'index.bs:769-770' src/obu/mix_presentation.rs && F=.planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-deferred-items.md && test -f $F && grep -q 'BCP-47' $F && grep -q 'MixPresentationTags' $F && grep -q 'index.bs:1273' $F && grep -q 'index.bs:1485' $F && grep -q 'Binaural' $F && C=$(git log -n 1 --format=%H -- tests/support/reference_expectations.rs) && git log -1 --format=%B "$C" > target/wlv-hash-commit.txt && grep -qF '793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24 -> a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409' target/wlv-hash-commit.txt && cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && grep -q '^full_exit=0$' target/wlv-full.log && [ -z "$(grep '^test result: ' target/wlv-full.log | grep -v '^test result: ok')" ] && cargo test --locked --doc && cargo test --locked --features fuzzing --test fuzz_regression && grep -q '^conf_exit=0$' target/wlv-conformance.log && grep -q '^test result: ok' target/wlv-conformance.log && ! grep -q 'TIMED OUT' target/wlv-conformance.log && grep -qF '[phase3_flac] CONF-06 (iamf-tools parser): decoder_main reported' target/wlv-conformance.log && grep -qF '[phase3_opus] CONF-06 (iamf-tools parser): decoder_main reported' target/wlv-conformance.log && [ -z "$(grep -F 'SKIP CONF-05' target/wlv-conformance.log | grep -vE 'for phase3_(flac|opus): ')" ] && git diff --quiet 3347a7d -- DIFF-LEDGER.md tests/fixtures tests/golden.rs tests/conformance.rs tests/parallax_contract.rs tests/support/fixture.rs tests/support/parallax_contract.rs tests/support/sequence_cases.rs tests/support/test_000003.rs tools fuzz && git diff --name-only 3347a7d -- src/obu > target/wlv-obu.txt && [ "$(tr '\n' ' ' < target/wlv-obu.txt)" = "src/obu/mix_presentation.rs src/obu/mod.rs " ] && git diff --quiet HEAD -- src tests HANDOFF.md && git diff --name-only 3347a7d HEAD > target/wlv-committed.txt && [ -z "$(grep -vE '^(src/error\.rs|src/encoder\.rs|src/obu/mix_presentation\.rs|src/obu/mod\.rs|src/model/mod\.rs|src/sequence\.rs|tests/error_shape\.rs|tests/encoder_builder\.rs|tests/sequence_parse\.rs|tests/parse_reference\.rs|tests/support/reference_expectations\.rs|HANDOFF\.md|\.planning/STATE\.md|\.planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/.*)$' target/wlv-committed.txt)" ]</automated>
  </verify>
  <done>Both cross-OBU validators emit one identical scalable finding for a single two-layer channel element with one layout and nothing for the negatives; the deferred-items file records the remaining items and the th8 citation correction; fmt, both clippy runs, full cargo test, doc tests and fuzz replay pass; conformance ends test result: ok with conf_exit=0, both CONF-06 lines and only the phase3 FLAC/Opus CONF-05 SKIPs; the hash commit body carries old -> new; protected paths are unchanged since 3347a7d; only allowed paths were committed and src/tests are clean.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| foreign .iamf bytes -> MixPresentation::validate / DescriptorSet::validate / ParsedSequence::validate | Hostile language lists, anchor lists and element references reach the new scans |
| caller configuration -> EncoderBuilder::build | Caller-supplied languages and anchors decide whether iamf-tools can read the output |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-wlv-01 | Denial of Service | O(n^2) duplicate scans in validate() | low | mitigate | Scans use iterators only; languages are bounded by `count_label` read against bytes remaining, anchors by a u8 count (<= 255); the helper allocates nothing |
| T-wlv-02 | Denial of Service (panic) | scalable_layout_finding, new findings | medium | mitigate | No indexing (`let [only] = as_slice() else`, `.first()`), no arithmetic, no unwrap; enforced by clippy `indexing_slicing`/`arithmetic_side_effects` in Tasks 2-3 verify |
| T-wlv-03 | Tampering (output integrity) | EncoderBuilder::build | medium | mitigate | Duplicate languages (case-folded) and duplicate anchor bytes are rejected before descriptors freeze (Task 1), so build() can no longer emit files iamf-tools refuses |
| T-wlv-04 | Repudiation (silent validation drift) | reference semantic_sha256 | low | mitigate | Exactly one pinned expected hash change, stop-and-report on any other; provenance test on test_000063/062; old -> new in commit body |
| T-wlv-05 | Tampering (foreign file fidelity) | low-level writers | low | accept | Writers are deliberately unchanged so foreign files keep round-tripping; rule 3 stays a finding because its spec exception is unobservable |
</threat_model>

<verification>
- `cargo fmt --all -- --check`; `cargo clippy --locked --all-targets -- -D warnings` with and without `--features fuzzing`.
- `cargo test --locked` (every binary `test result: ok`), `cargo test --locked --doc`, `cargo test --locked --features fuzzing --test fuzz_regression`.
- Conformance with `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec` under a perl alarm ends `test result: ok`, with CONF-06 lines for phase3_flac and phase3_opus and only the phase3 FLAC/Opus CONF-05 SKIP lines.
- `git diff -U0 3347a7d -- tests/support/reference_expectations.rs` shows only the test_000063 old/new hash lines.
- `git diff --quiet 3347a7d -- DIFF-LEDGER.md tests/fixtures tests/golden.rs tests/conformance.rs fuzz tools`.
</verification>

<success_criteria>
- build() rejects duplicate `annotations_language` (ASCII-case-insensitive) and duplicate `anchor_element` bytes with the two new payload-free kinds, without pre-empting or being pre-empted wrongly.
- MixPresentation::validate() reports both duplicates with the research texts and positions; both cross-OBU validators report the scalable `num_layouts` finding identically; build() and writers carry no rule-3 check.
- Only test_000063's semantic_sha256 changes, to the pinned new value, explained in the commit body.
- `size_of::<Error>() <= 32`; goldens, fixtures and DIFF-LEDGER.md unchanged; all gates including conformance pass.
- The deferred-items file lists BCP-47, MixPresentationTags, the rule-3 build() stance, A1 parity and carried th8 item 6, and corrects th8's citations.
</success_criteria>

<output>
Create `.planning/quick/260913-wlv-enforce-unique-annotations-languages-sca/260913-wlv-SUMMARY.md` when done.
</output>
</content>
</invoke>
