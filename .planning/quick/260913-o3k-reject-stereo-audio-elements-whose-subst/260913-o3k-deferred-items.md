# Deferred items — quick 260913-o3k

None of items 1-5 is implemented in this task. Each entry gives its source, the current state, and why
it is out of scope.

Resolved by this task: 260913-n56 deferred item 3 ("Builder accepts Stereo with 2 uncoupled
substreams"). `EncoderBuilder::build()` now rejects Stereo 2/0 with
`ErrorKind::CoupledSubstreamCountMismatch` at `Location::Field("coupled_substream_count")`.

## 1. Expanded layouts get `Profile::Simple`, but iamf-tools requires Base-Enhanced

- **Source:** `iamf-tools@848c6ff4 iamf/cli/profile_filter.cc:165-171` (`kLayoutExpanded` erases the
  Simple and Base profiles).
- **Current state:** `src/model/profile.rs select_minimum_profile` ignores the layout type.
  `tests/encoder_builder.rs unrelated_presentations_do_not_sum_their_element_or_channel_limits`
  asserts `Profile::Simple` for two 9.1.6 elements. Pinned `decoder_main` does not apply
  `ProfileFilter` (research 260913-o3k section E, note 3), so the probe files decoded anyway. libiamf
  profile gating was not checked.
- **Why out of scope:** it is a separate profile rule, and fixing it changes a public test expectation.

## 2. Binaural 1/1 aborts `decoder_main` when rendering to Sound System A

- **Source:** research 260913-o3k section E, note 2. `audio_element_renderer_channel_to_channel.cc:162`
  returns `NOT_FOUND: Input key for LoudspeakerLayout= 9 was not found in the map`, followed by the
  `decoder_main.cc:245` CHECK failure ("Mix Presentation ID 0 not found in rendering metadata").
- **Current state:** the builder accepts Binaural 1/1 with a loudspeaker presentation layout, and
  `build_accepts_the_reference_substream_counts_for_every_named_layout` builds it Ok. The coupling
  check itself passes in the reference.
- **Why out of scope:** it is a renderer limitation for Binaural elements with a loudspeaker
  presentation layout (likely needs `Layout::Binaural`), not a coupling rule.

## 3. Per-layer substream counts for scalable multi-layer elements

- **Source:** `iamf-tools@848c6ff4 iamf/cli/obu_with_data_generator.cc CollectDemixedChannelGroupLabels`
  `:269-350` (research 260913-o3k section A3).
- **Current state:** `validate_element_topology` rejects `num_layers != 1` and `recon_gain_is_present`
  before the new table is consulted.
- **Why out of scope:** unreachable through the builder today. The table function is deliberately
  named `single_layer_substream_counts` so a future scalable path cannot misuse it for demixed layers.

## 4. `validate()` finding for coupling on parsed sequences

- **Source:** research 260913-o3k section C; same reasoning as 260913-n56 deferred item 2.
- **Current state:** the rule lives only in `EncoderBuilder::build()`. `parse_sequence(..).validate()`
  reports nothing for Stereo 2/0.
- **Why out of scope:** `tests/support/sequence_cases.rs` and `src/fuzzing.rs` intentionally
  round-trip Stereo 2/0 through the low-level writers, which must keep accepting foreign (permissive)
  files byte-exactly. libiamf accepts the shape.

## 5. Coupled-first substream layout for Ambisonics projection

- **Source:** `iamf-tools@848c6ff4 iamf/cli/obu_with_data_generator.cc:499-507`.
- **Current state:** the builder rejects projection Ambisonics (`validate_element_topology` falls to
  `invalid("audio_element_type")`).
- **Why out of scope:** unreachable while projection is rejected; it only matters once projection is
  authored.
