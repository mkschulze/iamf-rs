# Deferred items — quick 260913-qk3

None of items 1-4 is implemented in this task. Each entry gives its source, the current state, and why
it is out of scope.

Resolved by this task: 260913-p28 deferred items 1-5.

- Items 1 and 2: `build()` now rejects `num_sub_mixes != 1` with `SubMixCountNotOne` and a reserved
  `headphones_rendering_mode` with `ReservedHeadphonesRenderingMode`.
- Items 3 and 4: profile selection now applies the IAMF v1.1.0 scopes (v1.0.0-errata Simple/Base
  sections as adopted). The Simple/Base unique-element limits, Base's scene-based and multi-layer
  limits, and the Base-Enhanced 28-channel total span the IA Sequence. p28's "references win"
  disposition for items 3 and 4 is superseded by the qk3 user directive: satisfy both the spec and
  the pinned references.
- Item 5: `DescriptorSet::validate()` and `ParsedSequence::validate()` report a header profile that
  no Mix Presentation complies with, and (Q1 = A) sequence-wide limits the header profile is exceeded
  by.

## 1. `build()` accepts zero Mix Presentations

- **Source:** IAMF v1.1.0 `index.bs:1902` (an IA Sequence carries Mix Presentation OBUs) and `:1929`
  ("There SHALL be at least one Mix Presentation OBU that complies with the conformance points of the
  primary_profile").
- **Current state:** `EncoderBuilder::build()` with no Mix Presentation returns `Ok` with a Simple
  header. `encoder.descriptors().validate()` now truthfully reports
  "no Mix Presentation complies with primary_profile 0 (IAMF v1.1.0 index.bs:1929)" for such output.
- **Why out of scope:** it is a separate `build()` rejection rule with its own error-kind decision, not
  a profile restriction.

## 2. `build()` accepts the same Audio Element twice in one sub-mix

- **Source:** IAMF v1.1.0 `index.bs:1280` ("no duplicate values of audio_element_id within one Mix
  Presentation").
- **Current state:** a probe on 6622a09 returned `Ok(Base)`; profile selection counts the duplicated
  reference per presentation (as iamf-tools does) and once sequence-wide.
- **Why out of scope:** it is a descriptor-validity rule, outside this task's profile scope.

## 3. Separate sub-mix and headphones findings in `MixPresentation::validate()`

- **Source:** IAMF v1.1.0 `index.bs:1278`, `:1920` (`num_sub_mixes`) and `:1337-1341` (reserved
  `headphones_rendering_mode`).
- **Current state:** `MixPresentation::validate()` reports neither shape. On the parse side both are
  covered by the item-5 compliance finding: a presentation with either shape never complies with any
  profile.
- **Why out of scope:** `build()` maps every `MixPresentation::validate()` finding to
  `InvalidDescriptorReference` at `Field("descriptors")` through `validate_findings`, so a finding
  there would pre-empt the dedicated `build()` kinds unless the check order is reworked. It would also
  change more reference `semantic_sha256` values.

## 4. Binaural rendering abort in `decoder_main` (carried forward from 260913-p28 item 6)

- **Source:** `iamf-tools@848c6ff4 iamf/cli/renderer/audio_element_renderer_channel_to_channel.cc:162`
  and `iamf/cli/decoder_main.cc:245`.
- **Current state:** unchanged; `HeadphonesRenderingMode::Binaural` still builds (Simple), as the
  qk3 test `build_accepts_binaural_headphones_rendering_mode` records.
- **Why out of scope:** excluded by the qk3 task boundary (CONTEXT.md).

## 5. Out-of-scope discovery: CONF-05 fails for the phase3 FLAC and Opus fixtures with `IAMF_REF_DECODER` set

- **Source:** `tests/conformance.rs:535` (`the_flac_fixture_is_conformant`,
  `the_opus_fixture_is_conformant_to_the_pinned_libiamf_decode`).
- **Current state:** with
  `IAMF_REF_DECODER=.reference/libiamf/code/test/tools/iamfdec/iamfdec`, `iamfdec` prints
  `errno: -6, fail to configure decoder.` and writes a bare 44-byte WAV for `phase3_flac.iamf` and
  `phase3_opus.iamf`. The same two tests fail identically on an unmodified `git archive 6622a09`
  copy (with `.reference-manifest.json` and `.reference/` supplied), and the generated `.iamf` bytes
  are identical before and after this task (`cmp`). So the failure predates qk3. A likely cause is an
  `iamfdec` build without its FLAC/Opus codecs, but that was not investigated. CONF-06
  (`decoder_main`) decodes `phase3_flac` (3 temporal units), and the LPCM CONF-05 clauses pass.
- **Why out of scope:** it is not caused by this task's changes (scope boundary rule). It needs a
  separate look at the local `iamfdec` build or the phase3 codec fixtures.

## Note

`docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` is a foreign, uncommitted file and was deliberately not edited
by this task.
