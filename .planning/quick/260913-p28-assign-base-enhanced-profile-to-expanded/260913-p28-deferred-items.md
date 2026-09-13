# Deferred items — quick 260913-p28

None of items 1-6 is implemented in this task. Each entry gives its source, the current state, and why
it is out of scope.

Resolved by this task: 260913-o3k deferred item 1 ("Expanded layouts get `Profile::Simple`, but
iamf-tools requires Base-Enhanced"). `select_minimum_profile` now applies a Base-Enhanced floor when
any channel-based element's first layer is `LoudspeakerLayout::Expanded(_)`, after both 28-ceilings,
and `EncoderBuilder::build()` writes `primary_profile = additional_profile = 2`. Pinned libiamf
`iamfdec` decodes the crate's expanded LFE and 9.1.6 files (128 frames each) where the Simple-header
control decodes 0 frames.

## 1. `build()` does not reject `num_sub_mixes > 1`

- **Source:** `iamf-tools@848c6ff4 iamf/cli/profile_filter.cc:220-238`
  `FilterProfileForNumSubmixes` erases all three profiles when a Mix Presentation has more than one
  sub-mix. libiamf checks only the first sub-mix (`libiamf@f06e919e code/src/iamf_dec/IAMF_decoder.c:1284-1337`
  `iamf_database_mix_presentation_is_valid`).
- **Current state:** UNVERIFIED gap. Research assumption A1 rests on a grep of `src/encoder.rs` and
  `src/obu/mix_presentation.rs` only. Confirm with a failing test before implementing.
- **Why out of scope:** it is a rejection, not profile selection, and it needs an error kind/location
  decision.

## 2. `build()` does not reject headphones rendering modes 2/3

- **Source:** `iamf-tools@848c6ff4 iamf/cli/profile_filter.cc:240-271`
  `FilterProfileForHeadphonesRenderingMode` erases all profiles for `Reserved2` / `Reserved3`. libiamf
  drops a Mix Presentation with `headphones_rendering_mode > 1`
  (`IAMF_decoder.c:1284-1337`).
- **Current state:** UNVERIFIED gap. `HeadphonesRenderingMode::Reserved(u8)` exists at
  `src/obu/mix_presentation.rs:35-42`; no `build()` check was found by grep. Confirm with a test first.
- **Why out of scope:** same as item 1.

## 3. Spec errata Disagreement 1: Simple and Base element limits across the whole sequence

- **Source:** IAMF v1.0.0-errata `index.bs:1852-1861` ("only one unique" Audio Element OBU for
  Simple, "at most two unique" for Base), and v1.1.0 `index.bs:1953` (28 channels sequence-wide for
  `additional_profile = 2`).
- **Current state:** the crate applies the limits per Mix Presentation, as both references do
  (`profile_filter.cc:282-311`, `IAMF_decoder.c:1284-1337`).
  `unrelated_presentations_do_not_sum_their_element_or_channel_limits` records this.
- **Why out of scope:** the references win (D-LOCKED REF). Recorded only.

## 4. Spec errata Disagreement 2: Base scene-based and scalable-layer limits

- **Source:** IAMF v1.0.0-errata `index.bs:1862-1869`: Base allows at most one scene-based element
  and at most one element with `num_layers > 1`.
- **Current state:** neither reference enforces it; neither does the crate.
- **Why out of scope:** the references win. Recorded only.

## 5. No parse-side finding when no Mix Presentation complies with the header profile

- **Source:** IAMF v1.1.0 `index.bs:1929` ("at least one Mix Presentation OBU that complies with …
  primary_profile").
- **Current state:** `DescriptorSet::validate()` has no profile-vs-content finding. The encoder now
  always writes a sufficient profile; a foreign parsed file is not checked.
- **Why out of scope:** the low-level writers must keep round-tripping foreign files byte-exactly,
  with the same reasoning as 260913-o3k deferred item 4. A finding (not a rejection) could be added
  later without touching the writers.

## 6. Binaural 1/1 aborts `decoder_main` when rendering to Sound System A (carried forward)

- **Source:** 260913-o3k deferred item 2.
  `iamf-tools@848c6ff4 audio_element_renderer_channel_to_channel.cc:162` returns `NOT_FOUND: Input key
  for LoudspeakerLayout= 9 was not found in the map`, followed by the `decoder_main.cc:245` CHECK
  failure.
- **Current state:** unchanged by this task. Binaural alone still selects Simple.
- **Why out of scope:** a renderer limitation for Binaural elements with a loudspeaker presentation
  layout, not a profile rule.

## Note: untracked completeness audit

`docs/IAMF-V1.1-COMPLETENESS-AUDIT.md:121` already claims that the profile filter considers expanded
layouts ("Expanded Layouts"). Before this task that was not true. The untracked doc was deliberately
not edited (D-LOCKED HANDS-OFF); its owner should re-check the whole line. It also claims
headphones-mode filtering (see item 2, unverified) and unique-Codec-Config filtering, although research
260913-p28 A found no codec rules in `profile_filter` at the pin.
