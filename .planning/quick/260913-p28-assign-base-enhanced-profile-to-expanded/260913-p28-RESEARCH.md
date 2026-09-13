# Quick 260913-p28: Base-Enhanced for expanded loudspeaker layouts - Research

**Researched:** 2026-09-13
**Domain:** IAMF profile selection (IA Sequence Header `primary_profile` / `additional_profile`)
**Confidence:** HIGH. Every rule was read at the pinned SHAs, and libiamf's behaviour was confirmed by running it.

## Summary

**This is a Core Value bug, not a spec nicety.** Today the crate writes `primary = additional = Simple (0)` for any lone expanded-layout element (LFE, 9.1.6, Top6, ...). Pinned **libiamf `iamfdec` returns exit 0, decodes 0 frames and writes a bare 44-byte WAV** for those files. Change only bytes 6-7 (the profile bytes) to `02 02` and the same files decode 1 frame / 128 samples (E). The earlier `decoder_main` probe missed this because `decoder_main` filters on caller-requested profiles, not the header's.

Both references and Eclipsa agree: an expanded layout (`loudspeaker_layout == 15`) needs Base-Enhanced. The crate's other profile rules (element and channel limits, per-presentation scope, equal pair) already match. The remaining gaps are *rejection* rules, not profile selection. They are listed in C and deferred in F.

**Primary recommendation:** in `select_minimum_profile`, treat an element whose first layer is `LoudspeakerLayout::Expanded(_)` as a Base-Enhanced floor. Keep the ceiling checks first. Migrate the four test groups listed in D. Golden fixtures are unaffected.

## A. iamf-tools@848c6ff4 rule set (VERIFIED: `git show 848c6ff4…:iamf/cli/profile_filter.cc`, read in full)

Constants `profile_filter.cc:35-41`: `kSimpleProfileMaxAudioElements = 1`, `kBaseProfileMaxAudioElements = 2`, `kBaseEnhancedProfileMaxAudioElements = 28`, `kSimpleProfileMaxChannels = 16`, `kBaseProfileMaxChannels = 18`, `kBaseEnhancedProfileMaxChannels = 28`.

| # | Rule (function, lines) | Scope | Erases |
|---|---|---|---|
| R1 | `FilterAudioElementType` :50-71. Any type other than `kAudioElementChannelBased` / `kAudioElementSceneBased` | per referenced element | all three |
| R2 | `FilterChannelBasedConfig` :119-183, **first layer only** (`channel_audio_layer_configs[0]`, :140-141). Mono..3.1.2 and Binaural: nothing. `kLayoutReserved10..14`: all three (:156-164). **`kLayoutExpanded`: Simple and Base (:165-167)**, then `FilterExpandedLoudspeakerLayout` | per element | see left |
| R3 | `FilterExpandedLoudspeakerLayout` :76-117. Missing value: clear all and error. The 13 named values (LFE .. Top6Ch) keep Base-Enhanced. `Reserved13` / `Reserved255` / other: erases Base-Enhanced | per element | BE |
| R4 | `FilterAmbisonicsConfig` :185-218. Mono and Projection keep all; any other mode erases all | per element | all three |
| R5 | `FilterProfileForNumSubmixes` :220-238. `num_sub_mixes > 1` | per MP | all three |
| R6 | `FilterProfileForHeadphonesRenderingMode` :240-271. Mode `Reserved2` / `Reserved3` on any element | per MP | all three |
| R7 | `FilterProfilesForNumAudioElements` :313-337. Sum of `sub_mix.audio_elements.size()` over sub-mixes (:292). Duplicate references count twice | per MP | above 1: Simple; above 2: Base; above 28: BE |
| R8 | `FilterProfilesForNumChannels` :339-362. Channels = Σ `substream_id_to_labels` label counts (:273-280, :307) | per MP | above 16: Simple; above 18: Base; above 28: BE |

- **Unreferenced elements are never filtered.** Element filtering happens only inside `FilterProfilesForMixPresentation` (:282-311, :397-429).
- **No codec or parameter rules exist at the pin.** The header comment at `profile_filter.h:28-33` says so. The HEAD-only `FilterProfilesForCodecConfigRules` must not be ported.
- Label counts for Ambisonics mono equal the number of `channel_mapping` entries that are not 255 (`obu_with_data_generator.cc:463-484`).

**Encoder use.** `obu_sequencer_base.cc:306-313` checks each Mix Presentation against `{GetPrimaryProfile(), GetAdditionalProfile()}` with `RETURN_IF_NOT_OK`. If the check fails, the sequencer refuses to write. For a Simple/Simple header with an expanded element, the error text is `"… Audio element ID= N has the first loudspeaker_layout= 15. But the requested profiles do support not support this type."` (:176-179, typo verbatim). `MAYBE_RETURN_IF_NOT_OK` in the filter only ignores errors under `IGNORE_ERRORS_USE_ONLY_FOR_IAMF_TEST_SUITE` (`common/utils/macros.h:29-36`). `IASequenceHeaderObu::Validate` checks only `ia_code` and the primary profile's enum range (`ia_sequence_header.cc:29-50`).

**Decoder use.** `ObuProcessor::InitializeForRendering` filters on **caller-supplied** `desired_profile_versions` (`obu_processor.cc:200-219, 527-543`), and fails with `"No supported mix presentation OBUs found."`. The API default is all three profiles (`api/decoder/iamf_decoder.h:58-60`). The sequence header's profiles are **not** used, which explains 260913-o3k note 3.

**Corroboration.** Every pinned testdata textproto that uses `LOUDSPEAKER_LAYOUT_EXPANDED` (test_000600-000633, 000700-000703, 000711, 000712) sets `primary_profile: PROFILE_VERSION_BASE_ENHANCED` [VERIFIED: git grep at pin].

### Spec cross-check (VERIFIED: `git show v1.1.0:index.bs`, `git show v1.0.0-errata:index.bs`)
- v1.1.0 `index.bs:1932-1944`: Simple and Base "comply with" v1.0.0-errata. In errata, the `loudspeaker_layout` table lists `others: Reserved` (errata `index.bs:985`). Value 15 is therefore reserved to Simple/Base parsers, and expanded layouts are Base-Enhanced only. This agrees with R2.
- v1.1.0 `:1920-1921`: all profiles `SHOULD` ignore MPs with `num_sub_mixes > 1` or more than 28 elements (R5, R7).
- v1.1.0 `:1929`: "at least one Mix Presentation OBU that complies with … primary_profile".
- v1.1.0 `:1951-1960`: Base-Enhanced allows 28 channels per MP.
- **Disagreement 1 (scope):** errata Simple says "only one unique Audio Element OBU" and Base "at most two unique" (errata `:1852-1861`). Those limits are **sequence-wide**. v1.1.0 `:1953` puts the 28-channel limit for `additional_profile = 2` on the whole sequence. Both references apply the limits per Mix Presentation. The references decide; the crate already follows them.
- **Disagreement 2:** errata Base allows at most one scene-based element and at most one element with `num_layers > 1` (`:1862-1869`). Neither reference enforces this.

## B. libiamf@f06e919e and Eclipsa

**libiamf enforces profiles, and it is the stricter reference here** (VERIFIED: `git show f06e919e:code/src/iamf_dec/IAMF_decoder.c`):
- `:1371`: `db->profile = IAMF_PROFILE_DEFAULT` (= Base-Enhanced, `IAMF_types.h:143`).
- `:1402-1410`: if `primary_profile > db->profile`, fail with "Unimplemented profile". If `additional_profile < db->profile`, set `db->profile = additional_profile`. **The effective profile is `min(additional, 2)`.**
- `iamf_element_is_valid` `:634-668`:
  - Channel-based with `num_layers == 1 && profile <= IAMF_PROFILE_BASE && !iamf_audio_layer_base_layout_check(layer[0].loudspeaker_layout)` makes the element invalid. `base_layout_check` is `type >= MONO && type < IA_CHANNEL_LAYOUT_COUNT` (`IAMF_utils.c:81-83`), and `COUNT` comes right after `BINAURAL` (`IAMF_defines.h:182-194`). **Layout 15 under Simple or Base drops the element.**
  - Scene-based elements are invalid if `substream_count + coupled_substream_count > max_amb_channels`.
  - `_profile_limit` `:628-632`: `{1, 16, 16}` Simple, `{2, 18, 16}` Base, `{28, 28, 25}` Base-Enhanced.
  - `iamf_database_element_add` `:1258` returns `IAMF_ERR_UNIMPLEMENTED`, so the element is never stored.
- `iamf_database_mix_presentation_is_valid` `:1284-1337` checks only the **first** sub-mix:
  - `nb_elements > max_elements`.
  - A referenced element missing from the database.
  - `headphones_rendering_mode > 1`.
  - Summed channels `> max_channels`. The sum is the last layer's layout channels, or `output_channel_count` for Ambisonics.
  - A failing presentation is dropped (`:1427-1431`).
- **Net effect:** a Simple header plus an expanded element drops the element, then the presentation, and decodes 0 frames (E).

**Eclipsa** (VERIFIED: `git show HEAD:` at c964609):
- `FileExport.h:91-106 minimumProfile` returns `BASE_ENHANCED` if any element `isExpandedLayout()`. Otherwise it applies `<=16 ch && <=1 element` → Simple and `<=18 && <=2` → Base.
- `IAMFFileWriter.cpp:114-117` writes the result as an equal primary/additional pair.
- Tests `IAMFFileWriter_test.cpp:146-160` assert Base-Enhanced for a lone expanded 7.1.4-front element and a lone LFE element.
- Aside: Eclipsa passes `audio_element_metadata_size()` as `numChannels` and applies the counts sequence-wide. That is a known-divergent implementation; do not mirror it.

## C. Crate integration and gaps (VERIFIED: files read this session)

- `src/model/profile.rs:136-170 select_minimum_profile`:
  - Checks the element ceiling (above 28 → `ElementCountExceedsProfile` @ `num_audio_elements`), then the channel ceiling (above 28 → `ChannelCountExceedsProfile` @ `loudspeaker_layout`).
  - Then applies the Simple / Base / BaseEnhanced tiers **by count only**, and returns `(profile, profile)`.
  - `element_channel_count` `:207-229` returns the last layer's `channel_count()` or `output_channel_count()`, and `UnsupportedLayout` for reserved layouts or types.
- `src/encoder.rs:932-936`: `build()` calls `select_sequence_profile` (`:1235-1266`, per MP, highest pair wins), then `IaSequenceHeader::new(primary.to_wire(), additional.to_wire())`, and sets `manifest.sequence_profile`.
- `src/obu/sequence_header.rs:55-87, 119-130`: ia_code / `primary < 3` / `additional >= primary` findings, plus a write-time `AdditionalProfileBelowPrimary` error.
- `src/model/mod.rs:122-…` `DescriptorSet::validate()`: no profile findings.
- `src/error.rs:258` `const _: () = assert!(size_of::<Error>() <= 32);`. Errors are `Error::new(ErrorKind, Location::Field(..))`.

| Ref rule | Crate status | Gap? |
|---|---|---|
| R2 expanded → BE (iamf-tools :165-167, libiamf :643-648) | ignored | **YES: this task** |
| R2 reserved layout / R3 reserved expanded | builder rejects `UnsupportedLayout` (`encoder.rs:1371-1377`); selection errors | no |
| R1 reserved type, R4 reserved or projection mode | builder rejects (`validate_element_topology`) | no |
| R7 / R8 limits and per-MP scope | match. Channel counting uses output channels (= libiamf `:1321-1327`). iamf-tools counts non-255 mapping entries, so the crate is ≥ iamf-tools and equal to libiamf, the stricter one | no |
| libiamf ambisonics `substreams + coupled ≤ 16` for Simple/Base | a scene element with more than 16 output channels is already above 18 → BE | no |
| R5 `num_sub_mixes > 1` rejected by iamf-tools encoder | no `build()` check found (grep of `encoder.rs` / `mix_presentation.rs` validate) [INFERRED] | yes, deferred |
| R6 headphones mode 2/3 (iamf-tools erases all; libiamf drops the MP) | no `build()` check found (`HeadphonesRenderingMode::Reserved(u8)` exists, `mix_presentation.rs:35-42`) [INFERRED] | yes, deferred |
| Parsed-sequence "header profile vs content" finding (spec :1929) | absent | deferred |

## D. Test and fixture impact (VERIFIED by reading the tests)

Tests that must change:
- `tests/profile.rs:203-215` `expanded_9_1_6_has_sixteen_channels_and_selects_simple_when_alone`: rename to `…_selects_base_enhanced_when_alone` and assert `BaseEnhanced`.
- `tests/profile.rs:160-184` `elements_totalling` uses `Expanded(Ch9_1_6)` and `Expanded(Ch3_0)`, which changes the 16/17/18 tests at `:243-270` (Simple/Base → BE).
  - The 16-channel single-element Simple boundary cannot be built from channel-based elements without expanded layouts; the largest is 12 channels.
  - `AudioElementType::scene_based` is `pub(crate)` (`audio_element.rs:216`), so tests must obtain Ambisonics elements through `EncoderBuilder::add_ambisonics_mono` (`encoder.rs:759-771`) and `encoder.descriptors().audio_elements`.
  - Suggested boundaries:
    - 16 = TOA alone → Simple
    - 17 = TOA + Mono → Base
    - 18 = 7.1.4 + 5.1 → Base
    - 19 = SOA(9) + 5.1.4 → BE
    - 28 = TOA + 7.1.4 → BE (the spec's own example)
    - 29 = TOA + 7.1.4 + Mono → `ChannelCountExceedsProfile`
- `tests/profile.rs:330-345` (28 × 9.1.6 → `ChannelCountExceedsProfile`) is unchanged, provided the ceilings stay checked **before** the expanded floor.
- `tests/encoder_builder.rs:184-191` `unrelated_presentations_do_not_sum_their_element_or_channel_limits` uses `two_presentation_builder` (`:1016-1037`, two 9.1.6 elements; its only user). Replace the elements with two TOA `add_ambisonics_mono` elements (16 substreams, mapping 0..16). Each MP then needs Simple, and the union would be 32 channels, which errors, so the test keeps its intent. It remains a Disagreement-1 shape; note that in the test comment.
- No other test asserts a profile for an expanded element. `tests/fixture.rs:295-299, 545-555`, `tests/conformance.rs:684` and `builder_authors_ambisonics_mono…` (`:1004`) are 5.1, stereo or FOA. `tests/parallax_contract.rs` has no profile references (grep).

**Fixtures:**
- The only golden is `tests/fixtures/golden/phase1_sample_identity.iamf`, which is 5.1 → Simple and **byte-identical**.
- `tests/fixtures/reference/*` are parsed rather than produced; MANIFEST lists them as stereo Simple/Base.
- No `Expanded` usage in conformance, golden or refcorpus tests (grep). No DIFF-LEDGER impact.

## E. Probe (executed)

Copies only, in `scratchpad/p28/`. The inputs are the crate-built LPCM files from `scratchpad/probe2/out/`, which carry a Simple/Simple header. Each file was also copied with bytes 6-7 patched to `02 02`. Run with `.reference/libiamf/code/test/tools/iamfdec/iamfdec -i0 -o3 X.wav -r 48000 -s0 -d 16 -disable_limiter X.iamf`:

| file | header 0/0 | header 2/2 |
|---|---|---|
| x_lfe_1_0 | exit 0, "Get 0 frames", WAV 44 B | exit 0, "Get 1 frames / 128 samples", WAV 556 B |
| x_916_9_7 | exit 0, 0 frames, 44 B | 1 frame, 556 B |
| x_top6_3_3 | exit 0, 0 frames, 44 B | 1 frame, 556 B |
| c714_7_5 (control) | 1 frame, 556 B | 1 frame, 556 B |

libiamf logged no warning at the default log level; the failure is silent apart from the frame count. The Docker `iamf-tools` image listing hung, so no new `decoder_main` run was made. A.decoder already explains why `decoder_main` accepts both headers.

## F. Recommendation

**Implement (this task):**
1. `src/model/profile.rs`: after the two ceiling checks, compute `needs_base_enhanced = elements.iter().any(first layer is LoudspeakerLayout::Expanded(_))`. The match is `AudioElementType::ChannelBased(config)` with `config.scalable_channel_layout.layers.first()`, whose `loudspeaker_layout` matches `LoudspeakerLayout::Expanded(_)`. Select `Profile::BaseEnhanced` when it is true, and the existing count tiers otherwise.
   - Cite `// ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:165-171` and `// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:643-648 iamf_element_is_valid`.
   - Update the fn docs, the module docs and the `tests/profile.rs` header table. No new `ErrorKind`, so the 32-byte assert is unaffected.
2. Test migrations exactly as in D.
3. New tests:
   - `tests/profile.rs`, table-driven: each of the 13 named expanded layouts alone → `BaseEnhanced`; 7.1.4 alone stays `Simple`; FOA (via the builder) + expanded LFE → `BaseEnhanced`.
   - `tests/encoder_builder.rs`: a lone expanded LFE → `manifest.sequence_profile() == BaseEnhanced` and header bytes `primary_profile == 2 && additional_profile == 2`.
   - `tests/encoder_builder.rs`: one stereo MP plus one expanded MP in a single sequence → `BaseEnhanced`.
4. Recommended, gated on `IAMF_REF_DECODER`: one `tests/conformance.rs` round trip of a lone expanded LFE or 9.1.6 element that asserts WAV length > 44. That is the check that would have caught this. Skip it if the harness has no generic element path; record it as a deferred item instead.

**Defer (with reasons):**
- R5 `num_sub_mixes > 1` and R6 headphones mode 2/3: both are rejections that need a new error kind/location and public-behaviour decisions. They are not profile selection.
- A parsed-sequence `validate()` finding that no MP complies with the header profile. The low-level writers must keep round-tripping foreign files byte-exactly, as in the o3k deferral 4 reasoning.
- Spec Disagreements 1 and 2 (sequence-wide Simple/Base element counts; the Base scene-based and scalable combination rule). The references do not enforce them; record them only.
- Update `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md:121`. That untracked doc already claims the filter considers expanded layouts; correct it after the fix lands, or leave it to its owner.

## Security Domain

No new input surface. ASVS V5 (input validation) applies only to selection logic over already-validated builder descriptors. The fix uses no indexing or arithmetic (`first()` + `matches!`), which respects `indexing_slicing` and `arithmetic_side_effects`. No packages are added; no legitimacy audit is needed.

## Environment

| Dependency | Available | Note |
|---|---|---|
| libiamf `iamfdec` @ f06e919e | yes: `.reference/libiamf/code/test/tools/iamfdec/iamfdec` (built 2026-09-08) | used for E |
| `iamf-tools:v2.1.0` image | not verified (`docker images` hung) | not needed |

Validation (nyquist_validation = false; commands only): `cargo test --test profile --test encoder_builder`, then the full `cargo test` and `cargo clippy --all-targets`. Optionally `IAMF_REF_DECODER=… cargo test --test conformance`.

## Assumptions Log

| # | Claim | Risk if wrong |
|---|---|---|
| A1 | `build()` accepts `num_sub_mixes > 1` and headphones modes 2/3 (grep found no check) | only affects the deferral list |
| A2 | `tests/conformance.rs` has no generic element-spec path for an expanded-layout decode | item F4 may be cheap rather than deferred |

## Sources
- iamf-tools @848c6ff4: `iamf/cli/profile_filter.{h,cc}`, `obu_sequencer_base.cc:302-318`, `obu_processor.cc:198-219, 527-543`, `api/decoder/iamf_decoder.h:50-60`, `obu/ia_sequence_header.cc:29-91`, `common/utils/macros.h`, `obu_with_data_generator.cc:457-486`, `cli/testdata/test_0006xx/0007xx.textproto`
- libiamf @f06e919e: `code/src/iamf_dec/IAMF_decoder.c:621-668, 1245-1345, 1365-1445`, `IAMF_utils.c:81-95`, `IAMF_OBU.c:250-291, 531-591`, `include/IAMF_defines.h:182-196`, `IAMF_types.h:132-144`
- Spec: `docs/iamf` v1.1.0 `index.bs:1900-1961, 1025, 1082`; v1.0.0-errata `index.bs:970-986, 1848-1878`
- Eclipsa @c964609: `common/data_structures/src/FileExport.h:91-106`, `IAMFFileWriter.cpp:112-117`, `IAMFExportUtil.cpp:28-51`, `tests/IAMFFileWriter_test.cpp:118-160`
- Crate: `src/model/profile.rs`, `src/encoder.rs:759-771, 932-936, 1235-1266, 1367-1427`, `src/obu/sequence_header.rs`, `src/model/mod.rs:122-170`, `src/obu/mix_presentation.rs:35-42, 329-425`, `src/error.rs:258`, `tests/profile.rs`, `tests/encoder_builder.rs`
