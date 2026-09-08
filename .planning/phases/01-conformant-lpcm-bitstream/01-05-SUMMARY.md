---
phase: 01-conformant-lpcm-bitstream
plan: 05
subsystem: bitstream
tags: [iamf, descriptors, lpcm, obu, byte-identity, tdd]

requires:
  - phase: 01-conformant-lpcm-bitstream (plan 01-03)
    provides: the hand-rolled BitCursor/BitWriter and the uleb128 primitives every descriptor field reads through
  - phase: 01-conformant-lpcm-bitstream (plan 01-04)
    provides: the OBU header, the two-pass obu_size path, read_obu_with/write_obu_with and the single `trailing` drain site
provides:
  - the four descriptor OBUs — IA Sequence Header, Codec Config, Audio Element, Mix Presentation — with read immediately before write in each file
  - DESC-07's four distinct layout types across two OBUs, with the expanded-layout precondition encoded in the type
  - D-04's published AudioElementType, with the Reserved byte-identity round-trip property under test
  - DescriptorSet, by_id() and the reference's descriptor write order (sorted ids, unsorted Mix Presentations)
  - a shared ParamDefinition used by both the Audio Element and the Mix Presentation
affects: [01-06, 01-07, 01-08, phase-02-parser, phase-03-codecs]

actuals:
  tokens: 36702
  tasks: 4
  commits: 7

plan_head_before: 645eef5f89c84e02e66e8258b46aaf106804cb46

tech-stack:
  added: []
  patterns:
    - "Pattern 2 applied at seven sites: every gate flag derived from the data it gates"
    - "Pattern 3 applied per descriptor: the payload-level drain fills the descriptor's own `trailing`, leaving the OBU-level one empty"
    - "One bounds-checking count helper per module, so the check cannot be present at three sites and forgotten at the fourth"

key-files:
  created:
    - src/obu/sequence_header.rs
    - src/obu/codec_config.rs
    - src/obu/audio_element.rs
    - src/obu/mix_presentation.rs
    - src/obu/param_definition.rs
    - src/model/mod.rs
    - src/model/layout.rs
    - tests/descriptors.rs
  modified:
    - src/obu/mod.rs
    - src/lib.rs
    - src/error.rs

key-decisions:
  - "AudioElementType committed in D-04's locked shape (checkpoint auto-selected `as-locked` under auto mode): #[non_exhaustive] ChannelBased / SceneBased / Reserved { value, raw }, only ChannelBased publicly constructible"
  - "param_definition_mode is a DERIVED accessor over Option<DurationFields>, not a stored bool — the plan's literal struct listing would have made the flag/field disagreement constructible"
  - "recon_gain_is_present stays a stored field, documented: it is the one gate flag in the crate whose gated data lives in a different OBU, so there is nothing local to derive it from"
  - "SoundSystem variants are named A0_2_0 … Ss13_6_9_0 rather than the reference's A_0_2_0, because rustc's non_camel_case_types lint rejects a cased character adjacent to an underscore"
  - "ParamDefinition got its own module: it is genuinely shared between the Audio Element and the Mix Presentation, which land in different commits"
  - "DescriptorSet carries the IA Sequence Header, because the write order the reference defines starts with it"

patterns-established:
  - "Descriptor `trailing` is the payload-level remainder and is drained by the descriptor's own reader, so the OBU-level `trailing` is empty for every descriptor; `Reserved.raw` outranks both (D-05 precedence, now asserted)"
  - "A `// NOTE:` rider under the `// ref:` citation wherever a reference comment misleads — used for the inverted LPCM endianness comment and for the deliberate non-sorting of Mix Presentations"

requirements-completed: [DESC-01, DESC-02, DESC-03, DESC-04, DESC-05, DESC-06, DESC-07, DESC-08, DESC-09]

coverage:
  - id: D1
    description: "The complete 120-byte descriptor prologue of test_000003.iamf is reproduced byte-exact from its published textproto configuration"
    requirement: "DESC-08"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#the_120_byte_descriptor_prologue_of_test_000003_is_reproduced_byte_exact"
        status: pass
    human_judgment: false
  - id: D2
    description: "IA Sequence Header, including the libiamf-only additional_profile >= primary_profile rule as a typed write error"
    requirement: "DESC-01"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#ia_sequence_header_reproduces_offsets_0x00_through_0x07"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#writing_additional_profile_below_primary_is_a_typed_error"
        status: pass
    human_judgment: false
  - id: D3
    description: "Codec Config with the LPCM decoder config, the derived audio_roll_distance, and sample_format_flags == 0 meaning BIG-endian"
    requirement: "DESC-02"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#codec_config_reproduces_offsets_0x08_through_0x19"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#sample_format_flags_big_endian_serialises_as_zero"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#a_wire_audio_roll_distance_of_two_is_preserved_and_reported"
        status: pass
    human_judgment: false
  - id: D4
    description: "sample_format_flags is a named enum whose BigEndian variant serialises as 0x00, asserted by name in both directions"
    requirement: "DESC-03"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#sample_format_flags_big_endian_serialises_as_zero"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#sample_format_flags_little_endian_serialises_as_one"
        status: pass
    human_judgment: false
  - id: D5
    description: "Audio Element with D-04's AudioElementType, and the Reserved { value, raw } byte-identity round-trip over element types 2..=7"
    requirement: "DESC-04"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#audio_element_reproduces_offsets_0x1a_through_0x27"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#reserved_element_type_round_trips_byte_identically"
        status: pass
    human_judgment: false
  - id: D6
    description: "Mix Presentation, with a missing Sound System A layout reported by validate() and accepted by the reader"
    requirement: "DESC-05"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#mix_presentation_reproduces_offsets_0x28_through_0x77"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#a_sub_mix_without_a_stereo_layout_is_a_finding_and_not_a_parse_failure"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#a_sub_mix_with_two_layouts_serialises_two_loudness_blocks"
        status: pass
    human_judgment: false
  - id: D7
    description: "Two mandatory Mix Gain param definitions with param_definition_mode = 1 and default_mix_gain = 0, emitted with zero Parameter Block OBUs in the file"
    requirement: "DESC-06"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#param_definition_mode_1_writes_0x80_and_the_gain_follows_immediately"
        status: pass
    human_judgment: false
  - id: D8
    description: "Four distinct layout types across two OBUs, with the expanded-layout precondition unrepresentable when violated"
    requirement: "DESC-07"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#an_expanded_layout_carries_its_expanded_value_inside_the_variant"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#every_four_bit_loudspeaker_layout_value_round_trips_except_the_expanded_one"
        status: pass
    human_judgment: false
  - id: D9
    description: "Descriptor write order — Codec Configs and Audio Elements ascending by id, Mix Presentations in list order"
    requirement: "DESC-08"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#codec_configs_and_audio_elements_sort_by_id_but_mix_presentations_do_not"
        status: pass
    human_judgment: false
  - id: D10
    description: "Vec-in-bitstream-order collections with by_id(), returning None on empty and the first match on a duplicate plus a Finding naming both indices"
    requirement: "DESC-09"
    verification:
      - kind: unit
        ref: "tests/descriptors.rs#by_id_returns_none_on_an_empty_collection_and_the_first_match_on_a_duplicate"
        status: pass
      - kind: unit
        ref: "tests/descriptors.rs#a_sequence_with_zero_mix_presentations_serialises_without_panicking"
        status: pass
    human_judgment: false

duration: 29min
completed: 2026-09-08
status: complete
---

# Phase 01 Plan 05: Descriptor OBUs Summary

**The complete 120-byte descriptor prologue of `test_000003.iamf` reproduced byte-exact from its published textproto, with the three asymmetries this plan owns — big-endian-is-zero, the write-only stereo-layout rule, and the `libiamf`-only profile ordering — each handled, commented and asserted.**

## Performance

- **Duration:** 29 min
- **Started:** 2026-09-08T04:27:31Z
- **Completed:** 2026-09-08T04:56:10Z
- **Tasks:** 4 (one checkpoint, one tracer, two auto)
- **Files modified:** 11 (8 created, 3 modified)

## Accomplishments

- **The prologue reproduces byte-exact.** `write_descriptors` over a `DescriptorSet` built entirely from `test_000003.textproto` emits exactly the 120 bytes at offsets `0x00..0x78` of the vendored `.iamf`. Two independent derivations agree, as D-25 requires: the hand-decode in `01-RESEARCH.md` supplied the expected bytes and the vendored file is the second assert (several tests slice it directly, so a transcription slip in the table cannot pass silently).
- **`sample_format_flags == 0` is big-endian, and the crate says so three ways.** `SampleFormatFlags` is an enum with a named `BigEndian` variant whose discriminant is `0`, never a bare integer and never a `bool` called `little_endian`. Two tests assert the sense by name in both directions, and a `// NOTE:` under the citation records that `libiamf@main`'s comment one line above the code that acts on it is **inverted** and the code is authoritative.
- **`additional_profile < primary_profile` is a typed write error.** `libiamf`'s `_valid_profile` rejects the whole sequence; `iamf-tools` does not check the field at all, so such a file passes CONF-06 and fails CONF-05. The comment beside the check records that this is the reverse of the usual strictness asymmetry.
- **The mandatory stereo layout is handled as three separate things.** The model supports two layouts with two `Loudness` blocks per sub-mix (research correction 9, which plan 01-08's 5.1 fixture needs). `SubMix::validate` reports a missing Sound System A layout as a Finding, quoting the reference's own message. The **reader accepts it** — `iamf-tools` enforces the rule on write only and its read path carries an explicit TODO, so rejecting on read would make this crate stricter than the reference and break Phase 2's foreign-file round-trip. All three facts sit in one comment beside the check.
- **`Reserved { value, raw }` round-trips byte-identically**, table-driven over element types 2 through 7, with both the element-level and the OBU-level `trailing` asserted empty. This is the property D-04 chose its shape to buy, and it is the only assertion in the crate that can prove we preserved something we did not understand.
- **Descriptor ordering matches the reference including its deliberate inconsistency:** Codec Configs and Audio Elements ascending by id, Mix Presentations in **list order**, with the reference's own reason ("the original ordering may be used downstream when selecting the mix presentation") in the comment at the exact place a future reader would "fix" it.
- **Every count is bounds-checked before it reserves** — `num_substreams`, `num_parameters`, `num_layers`, `num_subblocks`, `count_label`, `num_sub_mixes`, `num_audio_elements`, `num_layouts`, `num_anchored_loudness` and the ambisonics demixing matrix (T-01-24, T-01-25, T-01-26).
- The shipping dependency graph is unchanged: `cargo tree -e normal,no-proc-macro` still lists `iamf` and `thiserror` and nothing else.

## Task Commits

1. **Task 1: Confirm the AudioElementType shape (D-04, one-way)** — checkpoint, no commit. Auto mode active and the gate was `blocking` (not `blocking-human`), so option `as-locked` was auto-selected: the D-04 shape as CONTEXT.md locks it.
2. **Task 2: IA Sequence Header and Codec Config (tracer, TDD)** — `256efd3` (test, RED) → `0a3b506` (feat, GREEN)
3. **Task 3: Audio Element, the D-04 type model, four layout types (TDD)** — `b6d03c5` (test, RED) → `328a0e9` (feat, GREEN) → `a0e123a` (fix, citations)
4. **Task 4: Mix Presentation, mandatory structure, descriptor ordering (TDD)** — `0d8c2cc` (test, RED) → `bbeabd2` (feat, GREEN)

## TDD Gate Compliance

`workflow.tdd_mode` is enabled and this is a `type: tdd` plan. Every behaviour-adding task ran RED → GREEN in that order, with a separate commit per gate.

| Task | RED | GREEN | REFACTOR | RED evidence |
|---|---|---|---|---|
| 2 | `256efd3` | `0a3b506` | — | `RED_EVIDENCE_OK` on `ia_sequence_header_reproduces_offsets_0x00_through_0x07` |
| 3 | `b6d03c5` | `328a0e9` | — | `RED_EVIDENCE_OK` on `reserved_element_type_round_trips_byte_identically` |
| 4 | `0d8c2cc` | `bbeabd2` | — | `RED_EVIDENCE_OK` on `the_120_byte_descriptor_prologue_of_test_000003_is_reproduced_byte_exact` |

Each RED was **intentional**, not incidental: the module's types and signatures were committed with the wire behaviour stubbed, so the named target test failed on a byte-comparison assertion rather than on a compile error, a zero-test discovery or a load crash.

**One tooling note for later plans.** `gsd_run check tdd-red-evidence` parses `node --test` TAP, which `cargo test` does not emit. The evidence records were produced by faithfully re-encoding the real `cargo test` run as TAP — every `ok`/`not ok` line comes from a `test <name> ... ok|FAILED` line cargo actually printed, and the counts are recounted from those lines. No result was synthesized. The converter lives in the session scratchpad; if later phases want the gate to keep working, it should become a small committed helper under `tools/`.

## Files Created/Modified

- `src/obu/sequence_header.rs` — DESC-01. `ia_code` stored and reported rather than assumed (D-06); the `libiamf`-only profile ordering rule as a typed write error.
- `src/obu/codec_config.rs` — DESC-02/DESC-03. `SampleFormatFlags`, `LpcmDecoderConfig`, `DecoderConfig::{Lpcm, Raw}`, `required_audio_roll_distance`, and a `validate()` returning Findings for the six conditions `iamf-tools` rejects and neither `libiamf` revision checks.
- `src/obu/audio_element.rs` — DESC-04 and D-04's `AudioElementType`, plus `ChannelBasedConfig`, `ScalableChannelLayoutConfig`, `ChannelAudioLayerConfig`, `OutputGain`, `AudioElementParam` and the ambisonics wire code.
- `src/obu/mix_presentation.rs` — DESC-05/DESC-06. `MixPresentation`, `SubMix`, `SubMixAudioElement`, `MixGainParamDefinition`, `Layout`, `LayoutWithLoudness`, `Loudness` with a computed `info_type`, `AnchoredLoudness`, `RenderingConfig`.
- `src/obu/param_definition.rs` — the shared `ParamDefinition` and `DurationFields`.
- `src/model/layout.rs` — DESC-07's four distinct types, with a module doc stating plainly that they live in two different OBUs and why the flat 0–30 numbering in `PROJECT.md` is a plugin's enum rather than a wire field.
- `src/model/mod.rs` — `DescriptorSet`, `Identified`, `by_id`, `write_descriptors`, the cross-reference and duplicate-id findings.
- `tests/descriptors.rs` — 45 named vectors, each keyed to the offset range it reproduces.
- `src/obu/mod.rs`, `src/lib.rs`, `src/error.rs` — module wiring and one new `ErrorKind` variant.

## Decisions Made

- **Task 1's checkpoint was auto-selected.** `workflow.auto_advance` and the chain flag are both true and the gate was `blocking`, not `blocking-human`, so `as-locked` (the recommended option, and the one CONTEXT.md D-04 records the user choosing) was taken without stopping. Nothing about the decision was re-litigated.
- **`param_definition_mode` is derived, not stored.** The plan's literal struct listing had both a `param_definition_mode: bool` and the `Option<DurationFields>` it gates, which makes exactly the flag/field disagreement Pattern 2 exists to prevent constructible — mode 1 with duration fields present would write a bit saying "no duration fields" and then three of them. It is now `ParamDefinition::param_definition_mode()`, computed from `duration_fields.is_none()`.
- **`recon_gain_is_present` stays stored.** Task 3's acceptance criterion asked for both layer gate flags to be derived accessors. `output_gain_is_present` is (over `Option<OutputGain>`). `recon_gain_is_present` cannot be: it gates data in a **different OBU** — the Recon Gain Parameter Block — so there is nothing local to derive it from. The type carries a paragraph saying so, distinguishing it from the flags Pattern 2 governs.
- **`SoundSystem` variants are renamed.** `A_0_2_0` fails rustc's `non_camel_case_types` lint (a cased character adjacent to an underscore), which `-D warnings` turns into a build failure. The variants are `A0_2_0` … `Ss13_6_9_0` — the reference's own digits and separators with the letter-adjacent underscore dropped. Recorded in the module doc so the divergence from the reference's spelling is not mistaken for a transcription error.
- **`ParamDefinition` got its own module.** It is shared by the Audio Element (demixing, recon gain) and the Mix Presentation (element and output mix gain), which land in different tasks and different commits; putting it in either owner would make the other reach across an OBU boundary for it.
- **`DescriptorSet` carries the IA Sequence Header.** The plan's field list omitted it, but the write order the reference defines begins with it, so a set that cannot hold one cannot express the ordering it exists to express.
- **A reserved `ambisonics_mode` reads nothing further**, matching `iamf-tools`: the rest of the payload becomes the OBU's remainder rather than part of the config.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] `param_definition_mode` made a derived accessor**
- **Found during:** Task 3 (the shared `ParamDefinition` the Audio Element needs)
- **Issue:** The plan's struct listing stored the mode bit *and* the `Option<DurationFields>` it gates. A model holding mode 1 with duration fields present would write a bit claiming the fields are absent and then emit them — a file the reference mis-frames, with nothing on our side able to see it. This is precisely the disagreement Pattern 2 and the `ObuHeader` precedent exist to make unrepresentable.
- **Fix:** Dropped the stored bool; added `ParamDefinition::param_definition_mode()` computed from `duration_fields.is_none()`, and the writer emits that.
- **Files modified:** `src/obu/param_definition.rs`
- **Verification:** `param_definition_mode_1_writes_0x80_and_the_gain_follows_immediately`; the whole prologue still reproduces byte-exact.
- **Committed in:** `b6d03c5` / `328a0e9`

**2. [Rule 3 - Blocking] `SoundSystem` variants renamed for the lint set**
- **Found during:** Task 3 (writing `src/model/layout.rs`)
- **Issue:** The plan named the variants `A_0_2_0` … `Ss13_6_9_0`. `non_camel_case_types` rejects a cased character adjacent to an underscore, and `cargo clippy --all-targets -- -D warnings` is a verify gate, so `A_0_2_0` fails the build. `#[allow]` was rejected because the D-21 escape census counts exactly one `#[allow]` in `src/` and it belongs to PROF-03.
- **Fix:** Renamed to `A0_2_0` … `Ss13_6_9_0`, with the reason recorded in the module doc.
- **Files modified:** `src/model/layout.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` clean; `every_four_bit_loudspeaker_layout_value_round_trips_except_the_expanded_one` covers all sixteen values.
- **Committed in:** `b6d03c5`

**3. [Rule 3 - Blocking] `src/obu/param_definition.rs` added, outside the plan's file list**
- **Found during:** Task 3
- **Issue:** `ParamDefinition` is needed by both `audio_element.rs` (task 3) and `mix_presentation.rs` (task 4). The plan placed it in `mix_presentation.rs`, which would have made task 3 depend on a file task 4 creates.
- **Fix:** New shared module, re-exported from `src/obu/mod.rs`.
- **Files modified:** `src/obu/param_definition.rs`, `src/obu/mod.rs`
- **Verification:** Both owners compile against it; `cargo test --test citations` passes over both `read_param_definition` and `write_param_definition`.
- **Committed in:** `b6d03c5`

**4. [Rule 1 - Bug] `read_counted` and `write_count` had no `// ref:` citation**
- **Found during:** Task 3 verification
- **Issue:** `tests/citations.rs` governs every `fn read_*`/`fn write_*` in `src/`, and these two helpers matched the pattern. The gate went red after the GREEN commit.
- **Fix:** Citations added, pointing at the uleb128 primitive they factor over, with a `// NOTE:` that the reference has no equivalent helper — it repeats the count-then-loop shape at each site — so a reviewer knows the bound is what they must check, not the read.
- **Files modified:** `src/obu/audio_element.rs`
- **Verification:** `cargo test --test citations` — 44 read/write functions checked, 0 missing.
- **Committed in:** `a0e123a`

**5. [Rule 2 - Missing Critical] `recon_gain_is_present` kept as a stored field, documented**
- **Found during:** Task 3
- **Issue:** The acceptance criterion asked for it to be a derived accessor, but nothing in the layer config (or anywhere in the Audio Element) gates on it — the data it describes lives in a Recon Gain Parameter Block in a different OBU. A "derivation" would have had to invent a source.
- **Fix:** Left stored, with a paragraph on the type explaining that Pattern 2 governs a flag and the fields it gates *in the same structure* and that this flag has none. `output_gain_is_present` is genuinely derived.
- **Files modified:** `src/obu/audio_element.rs`
- **Verification:** The published Audio Element vector reproduces `0x10` for the layer byte with both flags clear.
- **Committed in:** `328a0e9`

---

**Total deviations:** 5 auto-fixed (2 missing critical, 2 blocking, 1 bug)
**Impact on plan:** All five are corrections to the plan's letter in service of its stated intent — three of them (1, 2, 5) are places where following the letter would have contradicted an acceptance criterion or a verify gate. No scope creep; no requirement was widened or dropped.

## Issues Encountered

- **The RED-evidence gate is node-TAP-specific.** `gsd_run check tdd-red-evidence` parses `# tests` / `not ok N - name` lines that `cargo test` never emits, so a Rust project's real RED run classifies as `invalid_record`. Resolved by re-encoding the actual cargo output as TAP line-for-line (no result invented) before handing it to the checker. Worth promoting to `tools/` if later phases want the gate to keep working.
- **Nothing else.** No auth gates, no package installs, no architectural questions.

## Known Stubs

None. `grep -rn "STUB(GREEN)\|TODO\|FIXME\|unimplemented\|todo!" src/` returns only two hits, both of which are *quotations* of the reference's own `TODO(b/339855338)` in prose explaining why our reader must not reject a sub-mix without a stereo layout.

`rg 'allow\(' src/` returns nothing — the D-21 escape census is still empty, and PROF-03's single sanctioned `#[allow]` has not landed yet.

## User Setup Required

None — no external service configuration required. Every gate in this plan runs offline: `env -u IAMF_REF_DECODER cargo test --locked` is green with no reference binary present (CONF-10 holds).

## Next Phase Readiness

**Ready.**

- **01-06 (audio frames, BCG packing)** has the Audio Element it needs, including the 5.1 shape (`substream_count = 4`, `coupled_substream_count = 2`) already under test.
- **01-07 (profiles, Q7.8)** has `PROFILE_COUNT`, the profile-ordering rule, and `Loudness` fields typed as `i16` awaiting PROF-03's `lufs_to_q7_8()` helper — which will be the crate's single sanctioned `#[allow(clippy::disallowed_types)]`.
- **01-08 (the conformance fixture)** has everything the amended D-18 design needs: two layouts per sub-mix with two `Loudness` blocks for the single-element 5.1 fixture, and `DescriptorSet` ordering that will push the two-Audio-Element structure-only fixture through the same writer.
- **Phase 2 (parser)** has `AudioElementType::SceneBased` constructible through `scene_based_for_test`, which is what makes PARSE-07's "asserted understood" measurable, and the `Reserved` byte-identity property already asserted for element types 2..=7.

One thing to carry forward: the descriptor `read_*` functions currently assume they are handed a **bounded** payload reader. That is true through `read_obu_with`, and Phase 2's sequence-level parser must keep it true — a descriptor reader given an unbounded cursor would drain the rest of the file into `trailing`.

---
*Phase: 01-conformant-lpcm-bitstream*
*Completed: 2026-09-08*

## Self-Check: PASSED

All 8 created source/test files exist on disk and all 7 task commits resolve in
`git log --all`. `commits: 7` in the frontmatter is **measured** —
`git rev-list --count 645eef5..HEAD` — not narrated.
