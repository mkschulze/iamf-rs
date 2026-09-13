# Deferred items — quick 260913-wlv

None of the numbered items is implemented in this task. Each entry gives its source, the current state,
and why it is out of scope.

Resolved by this task: 260913-th8 deferred items 1, 3 and 5.

- th8 item 1: `build()` rejects a Mix Presentation whose `annotations_language` list holds two entries
  equal under ASCII case folding with `DuplicateAnnotationsLanguage` at `Field("annotations_language")`.
  `MixPresentation::validate()` reports each later duplicate at the same field, right after the
  presentation-annotation count check.
- th8 item 3: `DescriptorSet::validate()` and `ParsedSequence::validate()` report an identical
  `Field("sub_mix.num_layouts")` finding when a sub-mix's only element is channel-based with
  `num_layers > 1` and `num_layouts < num_layers`, through the shared `pub(crate)`
  `scalable_layout_finding`. It is a finding only; `build()` and the writers are unchanged.
- th8 item 5: `build()` rejects a `loudness_info` that repeats a raw `anchor_element` byte (Unknown 0
  included) with `DuplicateAnchorElement` at `Field("anchored_loudness.anchor_element")`.
  `MixPresentation::validate()` reports each later duplicate per layout, after `layout.reserved`.
- Both dedicated `build()` checks run after `DuplicateMixPresentationAudioElement` and before the
  generic presentation findings.
- `test_000063.iamf` `semantic_sha256`: `793dcdaf78e53e80dba54de3ce39ec28e794b9deb587c437c6b5e346e67aba24`
  -> `a2774dd684771a79f997a8b89764eaf61aa5d2271793303568596729b9465409` (its one LoudnessInfo carries
  `anchor_element` 1 twice; iamf-tools@v2.1.0 testdata marks it `is_valid: false`). No other reference
  expectation changed.

## Correction to 260913-th8

- th8 items 1 and 2 cite `index.bs:1272`; the rule is at IAMF v1.1.0 `index.bs:1273`.
- th8 item 5 is not reference-only: it is a spec SHALL at `index.bs:1485` ("There SHALL be no duplicate
  values of anchor_element within one LoudnessInfo()"), matched by iamf-tools.
- th8 item 3's rule text spans `index.bs:1305-1309`.

The th8 files were not edited.

## 1. BCP-47 conformance of `annotations_language`

- **Source:** th8 item 2; IAMF v1.1.0 `index.bs:1273` (`annotations_language` SHALL conform to BCP-47).
- **Current state:** not checked; the tag is carried as bytes. Duplicate detection folds ASCII case only
  (RFC 5646 2.1.1). Canonical equivalence beyond ASCII case (`en` vs `en-US`, grandfathered or
  redundant tag mappings) is not detected.
- **Why out of scope:** excluded by the user-approved bundle; needs a BCP-47 parsing decision.

## 2. `MixPresentationTags` presence rule

- **Source:** th8 item 4; IAMF v1.1.0 `index.bs:1313`.
- **Current state:** the bytes after the sub-mix loop stay in `trailing` and are not interpreted.
- **Why out of scope:** excluded by the user-approved bundle; needs a model change for the tags.

## 3. Rule 3 (`num_layouts >= num_layers`) in `build()` and the writers

- **Source:** IAMF v1.1.0 `index.bs:1305-1309`.
- **Current state:** never enforced in `build()` or on write. The shape is unreachable from the builder
  (`validate_element_topology` rejects `num_layers != 1`), and the authoring-layout exception
  (`index.bs:1308`) is not observable in the bitstream. The nesting of `:1308-1309` under "except in the
  following cases" is ambiguous. Redundant Mix Presentation copies repeat the finding in
  `ParsedSequence::validate()`, as other pass-two findings do.
- **Why out of scope:** locked decision (finding only). Revisit if scalable layers become buildable.

## 4. Assumption A1 parity: language equality

- **Source:** research assumption A1; iamf-tools@v2.1.0 `ValidateUnique` compares exact bytes.
- **Current state:** language equality is ASCII-case-insensitive, stricter than iamf-tools. Switching to
  exact-byte parity means changing `eq_ignore_ascii_case` to `==` in `src/encoder.rs` and
  `src/obu/mix_presentation.rs`; no reference hash would change.
- **Why out of scope:** locked decision (open question 1).

## 5. Carried from 260913-th8 item 6

- **Binaural rendering abort in `decoder_main`:**
  `iamf-tools@848c6ff4 iamf/cli/renderer/audio_element_renderer_channel_to_channel.cc:162` and
  `iamf/cli/decoder_main.cc:245`. Unchanged.
- **CONF-05 FLAC/Opus with `IAMF_REF_DECODER` set:** a host-dependent SKIP per 260913-vcc. Unchanged.
- **Why out of scope:** not part of the wlv bundle.

## Note

`docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` is a foreign, uncommitted file and was not edited by this task.
