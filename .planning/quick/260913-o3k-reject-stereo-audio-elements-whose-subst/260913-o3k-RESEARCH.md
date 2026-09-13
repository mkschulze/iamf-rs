# Quick 260913-o3k: reject channel-based audio elements whose substream / coupled counts the reference rejects - Research

**Researched:** 2026-09-13
**Domain:** IAMF v1.1.0 `ChannelAudioLayerConfig` substream topology, encoder `build()` validation
**Confidence:** HIGH. The rule table comes from pinned source and was confirmed by executing 25 probe files through pinned `decoder_main`.

## Project Constraints (from CLAUDE.md)

- The rules must match `iamf-tools@v2.1.0` (`848c6ff4`) and `libiamf@f06e919e`. Read `iamf-tools` only through `git show` at that SHA.
- No `unwrap`/`expect`/indexing/bare arithmetic in `src/` (`clippy::indexing_slicing`, `arithmetic_side_effects`). Errors use `thiserror`, and `size_of::<Error>() <= 32` is enforced at `src/error.rs:256`.
- Don't use `HashMap`. Store lookup tables as `const fn` matches.
- Golden fixtures must stay byte-identical. Low-level writers must keep round-tripping foreign spec-legal (and permissive) files. Validation rules belong in `EncoderBuilder::build()`, not in the OBU writers.

## Summary

For a single-layer channel-based element, pinned `iamf-tools` works out the exact set of coupled and non-coupled channel labels from the layout. It then requires `coupled_substream_count == coupled_labels/2` and `substream_count == coupled_labels/2 + non_coupled_labels`. The crate checks only `substream_count + coupled_substream_count == channel_count`, so it accepts shapes like Stereo 2/0, 5.1 6/0, 5.1 5/1 and 7.1.4 8/4 that `decoder_main` rejects. The probe confirmed that the crate writes all of these and `decoder_main` rejects each one.

The builder already rejects `num_layers != 1` (`src/encoder.rs:1379-1381`). That means only the **base channel group (single layer)** table and the **expanded-layout** table matter now. The incremental multi-layer (demixed channel group) rules are recorded below for when scalable layers are unblocked.

**Primary recommendation:** Add a `const fn` on `LoudspeakerLayout` in `src/model/layout.rs` that returns `(substream_count, coupled_substream_count)` for a single layer. Enforce it in `validate_element_topology` with one new payload-free `ErrorKind::CoupledSubstreamCountMismatch`, checked before the existing `ChannelCountMismatch` / `substream_count` check, which is the reference's order. Then migrate two test helpers: the `encoder_streaming.rs` 2/0 stereo builders and the `encoder_builder.rs` 9.1.6 16/0 helper. No golden, conformance or codec fixture changes.

## A. Exact rule set

### A1. Where the check lives, and whether decoder_main enforces it

- `ValidateSubstreamCounts` is at `iamf-tools@848c6ff4 iamf/cli/obu_with_data_generator.cc:395-430` [VERIFIED: git show]. It is called per layer from `ObuWithDataGenerator::FinalizeScalableChannelLayoutConfig` (`:671-743`, call at `:709-710`). The caller is `GenerateAudioElementsWithData` (`:561`), which `iamf/cli/descriptor_obu_parser.cc:274` invokes. That is the **decoder-side** descriptor parse path. The error surfaces as `obu_processor.cc:484`.
- **decoder_main enforces it** [VERIFIED: executed, section E]. The log reads `obu_processor.cc:484] INVALID_ARGUMENT: Coupled substream count different from the required number. In OBU: 0 vs expected: 1`, followed by `Failed to create OBU processor`.
- Error texts, verbatim (`obu_with_data_generator.cc:412-427`):
  - `"Coupled substream count different from the required number. In OBU: ", <m>, " vs expected: ", <req>` (checked **first**)
  - `"Substream count different from the #non-coupled substreams. In OBU: ", <n>, " vs expected: ", <req>`
- Related checks on the same path:
  - `AddSubstreamLabels` (`:352-393`) assigns **coupled labels first, two per substream id, then non-coupled**. It fails with `"Too few substream IDs are present to assign all labels. ..."`.
  - `ValidateEqual(audio_substream_ids.size(), substream_id_to_labels.size())` (`:737-740`).
  - `ValidateUnique(audio_substream_ids)` (`:676-678`).
  - `"Expanded layout is only permitted when there is a single layer."` (`:189-192`).
  - `ScalableChannelLayoutConfig::Validate` (`iamf/obu/audio_element.cc:470-503`) checks `num_layers` in [1,6], `"Cumulative substream count from all layers is not equal to the `num_substreams` in the OBU."`, and `"There must be exactly 1 layer if there is a binaural layout."`.

### A2. Single layer (base channel group): `CollectBaseChannelGroupLabels` `:102-180`, `LoudspeakerLayoutToChannels` `:57-99`

Channel numbers `{surround, lfe, height}` come from `:62-89`. Surround switch (`:106-143`):
- 1 → non-coupled `kMono`
- 2 → coupled `kL2,kR2`
- 3 → coupled `kL3,kR3` and non-coupled `kCentre`
- 5 → coupled `kL5,kR5,kLs5,kRs5` and non-coupled `kCentre`
- 7 → coupled `kL7,kR7,kLss7,kRss7,kLrs7,kRrs7` and non-coupled `kCentre`

Height switch (`:144-167`):
- 2 → coupled `kLtf3,kRtf3` if surround==3, else `kLtf2,kRtf2`
- 4 → coupled `kLtf4,kRtf4,kLtb4,kRtb4`

LFE (`:168-177`): 1 → non-coupled `kLFE`.

| loudspeaker_layout | channels {S,L,H} | coupled_substream_count | substream_count | Probe (section E) |
|---|---|---|---|---|
| Mono (0) | {1,0,0} | 0 | 1 | 1/0 decoded |
| Stereo (1) | {2,0,0} | 1 | 1 | 1/1 decoded; 2/0 rejected `0 vs expected: 1` |
| 5.1 (2) | {5,1,0} | 2 | 4 | 4/2 decoded; 6/0 rejected `0 vs 2`; 5/1 rejected `1 vs 2` |
| 5.1.2 (3) | {5,1,2} | 3 | 5 | 5/3 decoded |
| 5.1.4 (4) | {5,1,4} | 4 | 6 | 6/4 decoded |
| 7.1 (5) | {7,1,0} | 3 | 5 | 5/3 decoded |
| 7.1.2 (6) | {7,1,2} | 4 | 6 | 6/4 decoded |
| 7.1.4 (7) | {7,1,4} | 5 | 7 | 7/5 decoded; 8/4 rejected `4 vs expected: 5` |
| 3.1.2 (8) | {3,1,2} | 2 | 4 | 4/2 decoded; 6/0 rejected `0 vs 2` |
| Binaural (9) | {2,0,0} | 1 | 1 | 2/0 rejected `0 vs 1`; 1/1 passes the coupling check (see E, note 2) |
| Reserved 10-14 | — | `Unknown loudspeaker_layout=` (`:90-93`) | — | the crate already rejects `UnsupportedLayout` |

Expanded (`loudspeaker_layout` 15), single layer only. `CollectChannelLayersAndLabelsForExpandedLoudspeakerLayout` `:182-266` [VERIFIED: git show]:

| expanded_loudspeaker_layout | coupled labels | non-coupled | coupled | substream | Probe |
|---|---|---|---|---|---|
| LFE (0) | — | LFE | 0 | 1 | 1/0 decoded |
| StereoS (1), StereoSS (2), StereoRS (3), StereoTF (4), StereoTB (5), StereoF (9), StereoSi (10), StereoTpSi (11) | one pair | — | 1 | 1 | StereoS 1/1 decoded; 2/0 rejected `0 vs 1` |
| Top4Ch (6) | Ltf4,Rtf4,Ltb4,Rtb4 | — | 2 | 2 | 2/2 decoded |
| 3.0 (7) | L7,R7 | Centre | 1 | 2 | 2/1 decoded; 3/0 rejected `0 vs 1` |
| 9.1.6 (8) | FLc,FRc,FL,FR,SiL,SiR,BL,BR,TpFL,TpFR,TpSiL,TpSiR,TpBL,TpBR | FC, LFE | 7 | 9 | 9/7 decoded; 16/0 rejected `0 vs expected: 7` |
| Top6Ch (12) | TpFL,TpFR,TpSiL,TpSiR,TpBL,TpBR | — | 3 | 3 | 3/3 decoded |
| Reserved 13-255 | `"Unsupported expanded loudspeaker layout= "` (`:254-258`) | | | | the crate already rejects |

In every row, `substream + coupled == channel_count()` in `src/model/layout.rs:130-145, 201-218` [VERIFIED: Read]. The existing channel-count check is therefore implied once both new equalities hold. A table self-consistency unit test should assert exactly this.

### A3. Scalable layers (demixed channel groups): record only, the builder rejects them

Source: `CollectDemixedChannelGroupLabels` `:269-350`. For layer i>0, the new surround channels from `accumulated+1..=layer` add:
- 2 → non-coupled `kL2`, pushed last (the "Mono→Stereo" special case, spec rule "C, then LFE, then L")
- 3 → non-coupled `kCentre`
- 5 → coupled `kL5,kR5`
- 7 → coupled `kLss7,kRss7`

New height adds:
- from 0 → 4: `Ltf4,Rtf4,Ltb4,Rtb4`
- from 0 → 2: `Ltf3,Rtf3` if surround==3, else `Ltf2,Rtf2`
- from 2 → 4: `Ltf4,Rtf4`

New LFE adds non-coupled `kLFE`. Channel numbers may not decrease (`:531-540`, `"At least one channel number decreased from accumulated_channels to layer_channels"`). Example: Stereo→5.1.2→7.1.4 gives layer 1 {1,1}, layer 2 {3 coupled: L5/R5, Ltf2/Rtf2 + C, LFE → 4 substreams, 2 coupled}, and so on. **Defer:** the crate cannot author scalable layers (`recon_gain_is_present` and `num_layers` are rejected).

### A4. Spec v1.1.0 (`git show v1.1.0:index.bs`)

- `index.bs:1002` defines `coupled_substream_count` as "the number of referenced Audio Substreams, each of which is coded as coupled stereo channels".
- `:1004-1006`: coupled pairs "SHALL be coded in stereo mode". The coupled pairs are `L/R, Ls/Rs, Lss/Rss, Lrs/Rrs, Ltf/Rtf, Ltb/Rtb, FLc/FRc, FL/FR, SiL/SiR, BL/BR, TpFL/TpFR, TpSiL/TpSiR, TpBL/TpBR`. The non-coupled channels are `C, LFE, L, FC, LFE1`.
- `:1162-1166`: coupled substreams first, surround before top, front before side/rear/back, side before rear, then C, LFE, L.
- `:1000` requires the sum of `substream_count` to equal `num_substreams`, and it "SHALL NOT be set to 0". `:929` sets `num_layers` to 1..=6. `:931` says Binaural requires `num_layers` 1. `:1025` says expanded only when `num_layers` = 1.

**No disagreement.** The spec's "SHALL be coded in stereo mode" makes the coupling mandatory, and iamf-tools implements it as exact equality [VERIFIED: both read].

## B. libiamf and Eclipsa

- **libiamf does not check the coupled count** [VERIFIED: read `libiamf@f06e919e code/src/iamf_dec/IAMF_OBU.c:531-551`]. For each layer it accumulates `channels += nb_substreams + nb_coupled_substreams`. It accepts the layer only if `iamf_audio_layer_get_layout_info(layout)->channels == channels` and `nb_substreams > 0`; otherwise it logs `Invalid loudspeaker layout`. `IAMF_decoder.c:1893-1946` then sets `nb_channels = n + m` and uses `nb_coupled_substreams` only to set up the decoders (opus multistream: `opus_multistream2_decoder.c:63-71`).
  - [INFERRED] Stereo 2/0 LPCM would therefore decode in libiamf as two mono substreams mapped L, R. libiamf is the permissive oracle here, and the gate is iamf-tools. No `iamfdec` binary is available locally (`IAMF_REF_DECODER` is unset), so this was not executed.
- **Eclipsa assigns reference coupling** [VERIFIED: read `eclipsa-audio-plugin@c96460925f common/substream_rdr/substream_rdr_utils/Speakers.cpp:169-262`, `common/data_structures/src/AudioElement.cpp:103-146`].
  - `getCoupledChannelCount` / `getUncoupledChannelCount`:
    - Mono 0/1, Stereo 1/0, 3.1.2 2/2, 5.1 2/2, 5.1.2 3/2, 5.1.4 4/2, 7.1 3/2, 7.1.2 4/2, 7.1.4 5/2, Binaural 1/0
    - Expl LFE 0/1, Expl 7.1.4 Top (Top4Ch) 2/0, Expl 9.1.6 7/2, Expl 7.1.4 Front (3.0) 1/1, Expl 9.1.6 Top (Top6Ch) 3/0, all expanded stereo pairs 1/0
  - `substream_count = coupled + uncoupled` and `coupled_substream_count = coupled`, with one layer. This is identical to the A2 table.

## C. Crate integration

- **The API lets callers choose the counts.** `ChannelAudioLayerConfig::new(layout, substream_count, coupled_substream_count)` (`src/obu/audio_element.rs:64-77`) is passed through `add_audio_element` / `add_audio_element_with_substreams` (`src/encoder.rs:689-722`). The counts are not derived. `src/packing.rs SubstreamPlan::for_layout` derives them, but only for Mono/Stereo/Binaural/5.1 and only in test fixtures. Do not reuse it for validation, because it errors on the other layouts.
- **Check placement:** `validate_element_topology` (`src/encoder.rs:1367-1421`), called from `build()` at `:1026`. Put the new check after the single-layer destructure (`:1379`) and the recon-gain gate (`:1382-1387`), in place of or before the existing `:1388-1399` block. Order:
  1. `coupled_substream_count != required.coupled` → `Error::new(ErrorKind::CoupledSubstreamCountMismatch, Location::Field("coupled_substream_count"))`
  2. `substream_count != required.substream` → the existing `ErrorKind::ChannelCountMismatch, Location::Field("substream_count")`

  The old `coupled > substream` and channel-sum clauses become redundant but harmless. Keep the channel-sum check if the planner wants belt and braces. `channel_count().is_none()` for reserved layouts is already rejected at `:1371-1378`, so the new table function can return `Option` and reuse that path.
- **Table location:** add `pub const fn substream_counts(self) -> Option<(u8, u8)>` (or two fns) on `LoudspeakerLayout` in `src/model/layout.rs`, delegating `Expanded(e)` to `ExpandedLoudspeakerLayout`. Cite `// ref: iamf-tools@v2.1.0 iamf/cli/obu_with_data_generator.cc CollectBaseChannelGroupLabels / CollectChannelLayersAndLabelsForExpandedLoudspeakerLayout / ValidateSubstreamCounts`. The name must not start with `read_`/`write_`, which `tests/citations.rs` would police, but cite anyway per the standing directive.
- **Frame-length angle:** `substream_channels` (`src/encoder.rs:1352-1365`) already treats positions `< coupled_substream_count` as 2-channel. That matches reference `AddSubstreamLabels` (coupled first). LPCM `validate_frame` (`:520-551`) uses `num_samples_per_frame × channels × bytes`. FLAC/Opus payload sizes are not validated (`:552-553`). The new rule changes no frame-size logic. It only removes the topologies where a "stereo" element carried two mono frames.
- **Error conventions:** `ErrorKind` is `#[non_exhaustive]`, and the payload-free variants sit near `ChannelCountMismatch` (`src/error.rs:91`) [VERIFIED: Read]. A new unit variant does not affect the 32-byte assertion. Suggested message: `#[error("coupled_substream_count differs from the count the loudspeaker layout requires")]`.
- **Scene-based:** out of scope. Mono ambisonics has no coupled field and is already validated (`:1401-1417`). Projection is rejected by the builder (`:1418`, test `:1458-1496`). Reference projection assumes coupled-first (`obu_with_data_generator.cc:499-507`), which only matters once projection is authored.
- **`validate()` / parse side:** do NOT add a `Finding` in this task. `tests/support/sequence_cases.rs:99` and `src/fuzzing.rs:84` intentionally build Stereo 2/0 for low-level round-trip. The same reasoning as 260913-n56 deferred item 2 applies.

## D. Test and fixture impact

Every `ChannelAudioLayerConfig::new` site was grepped, and each builder-path site was read [VERIFIED: grep + sed of each site]:

| Site | Shape | Builder path? | Impact |
|---|---|---|---|
| `tests/encoder_streaming.rs:822` `stereo_builder`, `:858` `stereo_builder_with_parameter`, `:916` `reverse_substream_order_builder` | Stereo **2/0**, two handles, LPCM 16-bit, 128 samples, `stereo_pcm_frame()` = 256 B each | yes | **Breaks. Migrate.** About 22 call sites destructure `(left, right)`. |
| `tests/encoder_builder.rs:928-940` `element_with_layout`, used only by `two_presentation_builder` (`:865-887`), test `:184-191` | Expanded 9.1.6 **16/0** | yes | **Breaks. Migrate to 9/7** with ids `0..9`. Profile assertion unchanged, since channel count comes from the layout. |
| `tests/encoder_builder.rs:388-406` `build_rejects_incoherent_channel_topology` | Stereo 1/0, 5.1 2/4 | yes | still rejected; extend with new rejections |
| `tests/encoder_builder.rs:31, 350-362 stereo_element, 382, 827`; `tests/parallax_contract.rs:250-262`; `tests/support/parallax_contract.rs:221-233`; `tests/encoder_streaming.rs:780` (Mono 1/0) | Stereo 1/1 / Mono 1/0 | yes | unaffected |
| `tests/conformance.rs:677`, `tests/support/fixture.rs:825-840` (counts from `SubstreamPlan`), `tests/support/test_000003.rs:88` | 1/1, 5.1 4/2 | low-level fixtures | unaffected. The goldens (`tests/golden.rs` → `fixture::sample_identity`) use plan-derived reference coupling, so they stay **byte-identical**. |
| `tests/support/sequence_cases.rs:99`, `src/fuzzing.rs:84`, `tests/sequence_parse.rs:227-231`, `tests/descriptors.rs`, `tests/profile.rs:43` | incl. Stereo 2/0, 5.1 3/1, layout 1/0 | no (writers/parse/profile) | **must not change**. They prove foreign round-trip. |
| `tests/fixtures/codecs/{flac,opus}` | stereo packets (`channels 2`, `force_channels stereo` in `tests/codec_fixtures.rs:156,331-361`) | — | already one coupled stereo substream. **No regeneration, no new deps.** |

**Streaming migration (recommended):** switch the three helpers to `LoudspeakerLayout::Expanded(ExpandedLoudspeakerLayout::Ch3_0)` with `ChannelAudioLayerConfig::new(.., 2, 1)`. The first handle is the L/R pair and the second is Centre, so the two-handle tuple shape survives. Frames become `(first, pair_frame())` = 512 B and `(second, centre_frame())` = 256 B. Replace `stereo_pcm_frame()` with those two helpers, mechanically, by position; `stereo_temporal_unit` likewise. Rejection tests that submit a single or duplicate frame should use the correct size for the handle they submit, so the asserted `ErrorKind` is not masked by an LPCM size error first. `presentation()` stays (Sound System A output). Probe `x_30_2_1` shows `decoder_main` accepts this shape.
- **Alternative:** 5.1 with 4/2 avoids expanded layouts (see F, deferred item 1), but changes four handles across about 22 call sites. Choose this if the planner prefers not to put an expanded layout in a Simple-profile file.

## E. Probe (executed)

The probe crate is a copy at `/private/tmp/claude-501/-Users-cell-local-iamf-rs/ffcacbe6-b061-4445-a28e-fe0219356ad0/scratchpad/probe2/` (`src/main.rs`, `out/*.log`); the repo was not modified. It is a single element with LPCM 16-bit, 48 kHz, 128 samples per frame, 1 TU and a Sound System A presentation. **The current crate `build()` accepted all 25 shapes**, profile `Simple`. Pinned `iamf-tools:v2.1.0` (`4a011a6d2b9e`) `decoder_main` results:

- `Decoded 1 temporal units.` for:
  - mono_1_0, stereo_1_1
  - c51_4_2, c512_5_3, c514_6_4, c71_5_3, c712_6_4, c714_7_5, c312_4_2
  - x_lfe_1_0, x_stereos_1_1, x_top4_2_2, x_30_2_1, x_916_9_7, x_top6_3_3
- `INVALID_ARGUMENT: Coupled substream count different from the required number. In OBU: <m> vs expected: <req>` (exit 139) for:
  - stereo_2_0 (0 vs 1), binaural_2_0 (0 vs 1)
  - c51_6_0 (0 vs 2), c51_5_1 (1 vs 2), c714_8_4 (4 vs 5), c312_6_0 (0 vs 2)
  - x_stereos_2_0 (0 vs 1), x_30_3_0 (0 vs 1), x_916_16_0 (0 vs 7)
- Note 1: none of the shapes reached the second error ("Substream count different…"). Given `sub + coupled == channels`, a correct coupled count forces the substream count to be correct. That error is reachable only if the channel-sum check is removed.
- Note 2: **binaural_1_1 passes the coupling check but aborts later.** The log shows `audio_element_renderer_channel_to_channel.cc:162] NOT_FOUND: Input key for LoudspeakerLayout= 9 was not found in the map`, then `F… decoder_main.cc:245 … Mix Presentation ID 0 not found in rendering metadata` and a CHECK stack trace. This is a renderer limitation for Binaural elements with a loudspeaker presentation layout. It is unrelated to coupling (see F, deferred item 2).
- Note 3: expanded layouts decoded under a **Simple** profile header, so `decoder_main` does not apply `ProfileFilter`.

## F. Recommendation

**Rules** (single layer; ref `obu_with_data_generator.cc:102-180, 182-266, 395-430`; spec `index.bs:1002-1006`):
- R1: `coupled_substream_count == required_coupled(layout)`, else `CoupledSubstreamCountMismatch` @ `coupled_substream_count`.
- R2: `substream_count == required_substreams(layout)`, else the existing `ChannelCountMismatch` @ `substream_count`.
- Both use the A2 tables, checked in that order inside `validate_element_topology`.

**Tests to add** (`tests/encoder_builder.rs`, asserting `kind()` and `at()`):
- Rejections:
  - Stereo 2/0, Binaural 2/0, 5.1 6/0, 5.1 5/1, 7.1.4 8/4, 3.1.2 6/0
  - Expanded StereoS 2/0, 3.0 3/0, 9.1.6 16/0
  - One with correct coupled but wrong substream (e.g. 5.1 3/2 → `ChannelCountMismatch`)
- Positives (`build()` Ok), matching the ones `decoder_main` decoded:
  - Mono 1/0, Stereo 1/1, Binaural 1/1
  - 5.1 4/2, 5.1.2 5/3, 5.1.4 6/4, 7.1 5/3, 7.1.2 6/4, 7.1.4 7/5, 3.1.2 4/2
  - LFE 1/0, all 8 expanded stereo pairs 1/1, Top4Ch 2/2, 3.0 2/1, 9.1.6 9/7, Top6Ch 3/3
- A layout.rs unit test: for every named layout, `substream + coupled == channel_count()`, and reserved values return `None`.
- Optional CONF evidence: none needed in-repo. The probe logs above are the external evidence.

**Migrations:** the three `encoder_streaming.rs` helpers (D), and `encoder_builder.rs element_with_layout` → derive counts from the new table function (or hard-code 9/7). Nothing else.

**Defer (with reasons):**
1. **Expanded layouts get Profile::Simple, but iamf-tools requires Base-Enhanced.** `iamf-tools@848c6ff4 iamf/cli/profile_filter.cc:165-171` (`kLayoutExpanded` erases Simple and Base) [VERIFIED: git show]. `src/model/profile.rs select_minimum_profile` ignores the layout type, and `tests/encoder_builder.rs:184-191` asserts Simple for 9.1.6. `decoder_main` does not enforce it (Note 3). libiamf profile gating was not checked. This is a separate profile rule and changes a public test expectation, so it needs its own task.
2. **Binaural elements abort `decoder_main` rendering to Sound System A** (Note 2). Investigate the presentation layout (`Layout::Binaural`) in a separate task.
3. **Scalable multi-layer incremental counts** (A3). Unreachable while `num_layers != 1` is rejected.
4. **`validate()` Finding for coupling on parsed sequences.** Same reasoning as 260913-n56 item 2, and the low-level 2/0 round-trip cases must stay.
5. **Ambisonics projection coupled-first layout.** Unreachable, because the builder rejects projection.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | libiamf would decode Stereo 2/0 LPCM without error (inferred from source, not executed) | B | Low. The rule is driven by iamf-tools regardless. |
| A2 | Ch3_0 streaming-test migration raises no crate-side profile/builder error beyond what the probe showed (probe used the same builder path at 48 kHz, tests use 16 kHz) | D | Low. Fall back to 5.1 4/2. |

## Sources

- `iamf-tools@848c6ff4`:
  - `iamf/cli/obu_with_data_generator.cc:57-430, 521-557, 561-608, 671-743`
  - `iamf/cli/descriptor_obu_parser.cc:268-282`
  - `iamf/obu/audio_element.cc:470-503`
  - `iamf/cli/profile_filter.cc:73-171`
- IAMF spec `v1.1.0:index.bs:905-1025, 1145-1166`.
- `libiamf@f06e919e`: `code/src/iamf_dec/IAMF_OBU.c:505-580`, `IAMF_decoder.c:1870-1960`.
- `eclipsa-audio-plugin@c96460925f`: `common/substream_rdr/substream_rdr_utils/Speakers.cpp:169-262`, `common/data_structures/src/AudioElement.cpp:95-146`.
- Crate (Read this session):
  - `src/encoder.rs:470-600, 680-724, 990-1060, 1290-1497`
  - `src/model/layout.rs` (whole file), `src/model/profile.rs:178-229`, `src/packing.rs:1-260`, `src/error.rs:1-256`
  - The test sites listed in D
- Executed: probe2 crate + pinned `decoder_main` (25 files), 2026-09-13.
