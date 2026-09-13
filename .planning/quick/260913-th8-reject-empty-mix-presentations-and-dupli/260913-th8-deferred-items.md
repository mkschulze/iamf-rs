# Deferred items — quick 260913-th8

None of items 1-6 is implemented in this task. Each entry gives its source, the current state, and why
it is out of scope.

Resolved by this task: 260913-qk3 deferred items 1-3.

- Item 1: `build()` rejects an IA Sequence with no Mix Presentation with `NoMixPresentation` at
  `Field("mix_presentations")`, checked last in `validate_declarations`.
- Item 2: `build()` rejects a Mix Presentation that lists the same Audio Element handle more than once
  with `DuplicateMixPresentationAudioElement` at `Field("mix_presentation.audio_elements")`, after
  `SubMixCountNotOne` and `ReservedHeadphonesRenderingMode`. Presentation findings are now validated on
  a handle-lowered clone, so placeholder template ids cannot cause false rejections.
- Item 3: `MixPresentation::validate()` reports three findings, carried by `DescriptorSet::validate()`
  and `ParsedSequence::validate()`: `num_sub_mixes` (0 or > 1), each later duplicate
  `sub_mix.audio_element_id` across all sub-mixes, and a reserved `headphones_rendering_mode`.
  `build()` checks its dedicated kinds first, so none of them is pre-empted.

The qk3 compliance message for the zero-Mix-Presentation case ("no Mix Presentation complies with
primary_profile N (IAMF v1.1.0 index.bs:1929)") was deliberately left unchanged (research open
question 1, orchestrator decision D-04). No reference fixture has zero Mix Presentations.

## 1. Duplicate `annotations_language` entries

- **Source:** IAMF v1.1.0 `index.bs:1272` ("The same language SHALL NOT be duplicated");
  `iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:528-530` (read) and `:471-473` (write).
- **Current state:** the crate enforces the rule neither on parse nor in `build()`. Whether `build()`
  accepts duplicates was not verified.
- **Why out of scope:** th8 covers qk3 deferred items 1-3 only.

## 2. BCP-47 conformance of `annotations_language`

- **Source:** IAMF v1.1.0 `index.bs:1272` (`annotations_language` SHALL conform to BCP-47).
- **Current state:** not checked; the language tag is carried as bytes.
- **Why out of scope:** outside the qk3 deferred items; needs a BCP-47 validation decision.

## 3. `num_layouts >= num_layers` for a single scalable channel element (parse side)

- **Source:** IAMF v1.1.0 `index.bs:1305-1310`.
- **Current state:** not reported on the parse side. The builder rejects scalable channel layers, so
  `build()` cannot produce the shape.
- **Why out of scope:** parse-side only and not one of the qk3 deferred items.

## 4. `MixPresentationTags` presence rule

- **Source:** IAMF v1.1.0 `index.bs:1313` (`MixPresentationTags` SHALL be present if `obu_size`
  exceeds the sub-mix loop).
- **Current state:** the bytes after the sub-mix loop are modelled as `trailing` and not interpreted.
- **Why out of scope:** needs a model change for the tags; outside this task.

## 5. iamf-tools' unique anchored-loudness elements

- **Source:** `iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:56-66`.
- **Current state:** not checked by the crate.
- **Why out of scope:** a reference-only rule outside the qk3 deferred items.

## 6. Carried from 260913-qk3

- **qk3 item 4, Binaural rendering abort in `decoder_main`:**
  `iamf-tools@848c6ff4 iamf/cli/renderer/audio_element_renderer_channel_to_channel.cc:162` and
  `iamf/cli/decoder_main.cc:245`. Unchanged; `HeadphonesRenderingMode::Binaural` still builds
  (`build_accepts_binaural_headphones_rendering_mode`). Excluded by the th8 scope.
- **qk3 item 5, CONF-05 FLAC/Opus failure with `IAMF_REF_DECODER` set:**
  `tests/conformance.rs` `the_flac_fixture_is_conformant` and
  `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode`. Pre-existing (likely an `iamfdec`
  build without FLAC/Opus); still failing identically in th8's conformance run. Excluded by the th8
  scope.

## Note

`docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` is a foreign, uncommitted file and was deliberately not edited
by this task.
