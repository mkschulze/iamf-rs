---
phase: 01-conformant-lpcm-bitstream
plan: 06
subsystem: bitstream
tags: [iamf, audio-frame, packing, parameter-block, temporal-delimiter, lpcm, tdd]

requires:
  - phase: 01-conformant-lpcm-bitstream (plan 01-04)
    provides: the OBU header, TypeSpecific::Trimming with its END-before-START write order, and the single `trailing` drain site this plan extends rather than duplicates
  - phase: 01-conformant-lpcm-bitstream (plan 01-05)
    provides: ParamDefinition and its derived param_definition_mode, ChannelAudioLayerConfig, LoudspeakerLayout, and the vendored reference fixtures
provides:
  - the Audio Frame OBU with TIME-01's implicit substream ids and a non-canonical explicit id that survives a round trip
  - BCG channel→substream packing for mono, stereo and 5.1, with substream_count and coupled_substream_count derived from the same plan the frames are written from
  - the Temporal Delimiter OBU
  - the Parameter Block OBU whose governing ParamDefinition is an explicit, compiler-enforced argument
  - plan_frames and validate_temporal_unit — TIME-03's final-frame trimming and the two temporal-unit clauses re-verified at the pinned tag
affects: [01-07, 01-08, phase-02-parser, phase-03-codecs]

actuals:
  tokens: 24916
  tasks: 3
  commits: 6

plan_head_before: 90daf38b79797d6c79ac810ef4318779578fa6b8

tech-stack:
  added: []
  patterns:
    - "Pattern 2 extended to cross-structure duplication: a field the OBU header already owns is absent from the payload type rather than mirrored into it"
    - "Header-aware payload callbacks (read_obu_with_header / write_obu_with_header) so a payload parser can see obu_type WITHOUT the crate acquiring a second `trailing` drain site"
    - "Context-as-argument: every fact a parser needs that is not in its own bytes arrives as a function parameter, never as parser state"

key-files:
  created:
    - src/obu/audio_frame.rs
    - src/obu/temporal_delimiter.rs
    - src/obu/parameter_block.rs
    - src/packing.rs
    - tests/temporal.rs
    - tests/packing.rs
  modified:
    - src/obu/mod.rs
    - src/lib.rs
    - src/error.rs
    - CONFORMANCE-GATE.md

key-decisions:
  - "AudioFrame carries neither `trimming` nor `trailing`: the trim counts live in the OBU header and the frame claims the whole payload remainder, so both plan-listed fields would have been a second owner for bytes that already have one"
  - "read_parameter_block takes THREE explicit arguments — the cursor, the ParamDefinition and a ParamDefinitionType — because the shared wire prefix carries no type and the subblock data shape depends on it"
  - "An unknown ANIMATION type is a typed error, not preserved verbatim: the wire carries no length for it, so `verbatim` would mean inventing a subblock boundary. The reference returns UnimplementedError for the same reason"
  - "SubstreamPlan::for_layout implements mono, stereo, binaural and 5.1 and returns UnsupportedLayout for everything else, rather than inventing an ordering the reference never states"
  - "Research assumption A4 was re-verified at the pinned SHA before being enforced, and only the two clauses that are wire facts are enforced — the timestamp clauses are iamf-tools' internal bookkeeping"
  - "BlockDurationFields is a new two-field type rather than a reuse of param_definition::DurationFields, because the block's per-subblock durations live inside each subblock and a second list would give the same wire bytes two owners"

patterns-established:
  - "A payload type omits any field the OBU header already carries; `into_obu` is the one place the two are assembled consistently"
  - "Where a reference does something for a reason we must match, the comment says both the behaviour and the reason — see the unknown-animation-type NOTE, which explains why 'preserve verbatim' is not implementable there"

requirements-completed: [TIME-01, TIME-02, TIME-03, TIME-04, TIME-05]

coverage:
  - id: T1
    description: "An untrimmed Audio Frame for substream 0 emits `30 80 04` followed by its 512-byte payload — offset 0x78 of test_000003.iamf, obu_size 512"
    requirement: "TIME-01"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#an_untrimmed_audio_frame_for_substream_0_reproduces_offset_0x78"
        status: pass
    human_judgment: false
  - id: T2
    description: "The trimmed final Audio Frame emits `32 82 04 40 00` followed by 512 payload bytes — obu_size 514, trim-at-end 64 written before trim-at-start 0"
    requirement: "TIME-03"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#the_trimmed_final_audio_frame_reproduces_offset_0x7d32"
        status: pass
    human_judgment: false
  - id: T3
    description: "A substream id of at most 17 is written as OBU type 6 + id with no explicit field; a larger id is type 5 with an explicit uleb128 id"
    requirement: "TIME-01"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#a_substream_id_of_17_is_implicit_in_obu_type_23_and_18_is_explicit_in_type_5"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#obu_type_for_and_substream_id_for_are_inverses_across_the_boundary"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#a_type_5_frame_carrying_a_small_id_round_trips_as_type_5"
        status: pass
    human_judgment: false
  - id: T4
    description: "5.1 packs as four substreams — [L,R] coupled to type 6, [Ls,Rs] coupled to type 7, C mono to type 8, LFE mono to type 9 — proven against all three independent derivations"
    requirement: "TIME-02"
    verification:
      - kind: unit
        ref: "tests/packing.rs#five_one_packs_as_two_coupled_pairs_then_two_monos"
        status: pass
      - kind: unit
        ref: "tests/packing.rs#five_one_at_256_samples_and_16_bit_reproduces_the_shipped_size_signature"
        status: pass
      - kind: unit
        ref: "tests/packing.rs#the_shipped_five_one_reference_file_reads_out_the_same_answer"
        status: pass
      - kind: unit
        ref: "tests/packing.rs#the_shipped_five_one_audio_element_declares_the_counts_the_plan_derives"
        status: pass
    human_judgment: false
  - id: T5
    description: "Interleaving inside a coupled substream is sample-interleaved (L0 R0 L1 R1 ...), not planar"
    requirement: "TIME-02"
    verification:
      - kind: unit
        ref: "tests/packing.rs#a_coupled_substream_payload_is_sample_interleaved_not_planar"
        status: pass
    human_judgment: false
  - id: T6
    description: "substream_count is 4 and coupled_substream_count is 2 for a 5.1 element, derived from the plan and equal to the shipped reference file's declared values"
    requirement: "TIME-02"
    verification:
      - kind: unit
        ref: "tests/packing.rs#five_one_declares_four_substreams_and_two_coupled"
        status: pass
      - kind: unit
        ref: "tests/packing.rs#stereo_is_one_coupled_substream_and_mono_is_one_mono_substream"
        status: pass
    human_judgment: false
  - id: T7
    description: "Packing performs no arithmetic on sample values — it is a permutation and a regroup, so the no-DSP guard stays green with zero #[allow] in the file"
    requirement: "TIME-02"
    verification:
      - kind: command
        ref: "grep -v '^[[:space:]]*//' src/packing.rs | grep -c 'allow'  ->  0"
        status: pass
      - kind: command
        ref: "cargo clippy --all-targets -- -D warnings"
        status: pass
    human_judgment: false
  - id: T8
    description: "A frame count that does not evenly divide the total produces exactly one trimmed frame, at the end, with 0 < trim_at_end < num_samples_per_frame"
    requirement: "TIME-03"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#a_non_multiple_sample_count_produces_exactly_one_trimmed_frame_at_the_end"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#an_exact_multiple_sample_count_produces_no_trimmed_frame"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#a_zero_num_samples_per_frame_is_a_typed_error_not_a_division_by_zero"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#the_frame_planner_refuses_a_sample_count_that_overflows_its_arithmetic"
        status: pass
    human_judgment: false
  - id: T9
    description: "Every Audio Frame within one temporal unit carries identical trim values, and a unit whose frames disagree is rejected"
    requirement: "TIME-03"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#a_temporal_unit_whose_frames_disagree_on_trim_is_rejected"
        status: pass
      - kind: command
        ref: "CONFORMANCE-GATE.md Experiment B — A4 re-verified at iamf-tools@848c6ff temporal_unit_view.cc:128-164"
        status: pass
    human_judgment: false
  - id: T10
    description: "A Temporal Delimiter writes obu_size 0 as the single byte 0x00 and carries no payload"
    requirement: "TIME-04"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#a_temporal_delimiter_emits_exactly_two_bytes"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#a_temporal_delimiter_round_trips_through_the_reader"
        status: pass
    human_judgment: false
  - id: T11
    description: "obu_redundant_copy on a Temporal Delimiter, a Parameter Block or any Audio Frame is a typed error, and so is the trimming flag on a Temporal Delimiter"
    requirement: "TIME-04"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#obu_redundant_copy_on_an_audio_frame_is_a_typed_error"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#the_trimming_flag_and_redundant_copy_are_both_refused_on_a_temporal_delimiter"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#a_parameter_block_may_not_carry_a_redundant_copy_flag"
        status: pass
    human_judgment: false
  - id: T12
    description: "Parsing a Parameter Block takes its ParamDefinition as an explicit function argument; the same bytes read against a mode-0 and a mode-1 definition produce different blocks"
    requirement: "TIME-05"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#a_mode_1_parameter_block_carries_its_own_duration_fields"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#a_mode_0_parameter_block_takes_its_duration_from_the_definition"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#a_parameter_id_disagreeing_with_the_definition_is_a_typed_error"
        status: pass
    human_judgment: false
  - id: T13
    description: "The three animation shapes carry the reference's field names and widths, and an unmodelled parameter definition type keeps its payload verbatim"
    requirement: "TIME-05"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#the_three_animation_shapes_have_the_reference_field_widths"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#an_unmodelled_parameter_definition_type_preserves_its_payload_verbatim"
        status: pass
      - kind: unit
        ref: "tests/temporal.rs#an_unknown_animation_type_is_the_same_typed_error_the_reference_returns"
        status: pass
    human_judgment: false
  - id: T14
    description: "num_subblocks is bounds-checked against bytes_remaining() before a single element is reserved (T-01-32)"
    requirement: "TIME-05"
    verification:
      - kind: unit
        ref: "tests/temporal.rs#a_subblock_count_larger_than_the_bytes_remaining_is_refused_before_reserving"
        status: pass
    human_judgment: false

duration: 26min
completed: 2026-09-08
status: complete
---

# Phase 01 Plan 06: Time-Varying OBUs and BCG Packing Summary

**The phase's highest silent-failure risk is closed: 5.1 packs as `[L,R]`, `[Ls,Rs]`, `C`, `LFE` — proven against all three independent derivations including a shipped reference file's own OBU size signature — with the channel order cited rather than derived and no permutation constant anywhere in the crate.**

## Performance

- **Duration:** 26 min
- **Started:** 2026-09-08T05:01:19Z
- **Completed:** 2026-09-08T05:27:17Z
- **Tasks:** 3 (one tracer, two auto), all TDD
- **Files modified:** 10 (6 created, 4 modified)

## Accomplishments

- **BCG packing agrees with all three derivations, and each is a separate assertion.** `tests/packing.rs` asserts the plan's shape from `libiamf`'s own ordering comment, the `1024/1024/512/512` byte-size signature the plan implies for 256 samples at 16 bit, and a **structural read of the shipped `tones_256samp_5p1_pcm.iamf`** whose four Audio Frames are types 6, 7, 8, 9 with exactly those `obu_size` values and whose Audio Element declares `substream_count = 4`, `coupled_substream_count = 2`. The last test compares the file's declared counts against the counts our plan derives, so the two numbers are checked against each other rather than each against a literal.
- **The source indices are `(0,1) (4,5) (2) (3)`, and that is the point.** The decoder's documented Sound System B output order is `L, R, C, LFE, Ls, Rs`; the packing order is coupled-front, coupled-surround, centre, LFE. Those are two different orders and TIME-02 exists because they differ. The module doc carries the full sound-system table with three citations — `IAMF_layout.c:72-73`, `iamf-tools` `testdata/README.md`, and `channel_label.cc:47` — so the order is **cited, not derived**.
- **There is no permutation constant, and the file says why.** The 5.1 → Sound System B render matrix in `libiamf` is the 6×6 identity, so the decoder's output *is* the caller's input and comparison is direct equality with nothing to tune. The module doc states the D-19 reasoning at the exact place someone would later add a knob: a permutation constant is a knob, and turning it until a comparison goes green absorbs a real packing bug into the harness and ships it.
- **`src/packing.rs` contains zero `#[allow]` and no arithmetic on any sample value.** Samples are moved as opaque byte slices; the only arithmetic is on indices and byte lengths, all of it `checked_*`. The D-21 escape census across `src/` is still **zero**.
- **Both Audio Frame vectors reproduce real reference bytes exactly** — `30 80 04` at offset `0x78` and `32 82 04 40 00` at `0x7D32`, the second confirming END-before-START trim order with values that actually differ (64 and 0), which is the only configuration in which a swapped order is visible at all.
- **A type-5 frame carrying an id of 17 or less round-trips as type 5.** The reader stores what the wire said (D-06) and `validate()` reports that it could have been implicit, rather than the writer normalising it to `6 + id`. That is what keeps `serialize(parse(bytes)) == bytes` true for foreign files, and `iamf-tools`' own constructor cannot express it — it derives the type from the id.
- **The Parameter Block's descriptor context is enforced by the type system.** `read_parameter_block(&mut BitCursor, &ParamDefinition, ParamDefinitionType)` — omitting either context argument is a compile error. Two tests read *different-shaped blocks from the same argument position* under a mode-0 and a mode-1 definition, which is the observable form of "this cannot be parsed from its own bytes".
- **Research assumption A4 was re-verified at the pinned SHA before anything was enforced.** `CONFORMANCE-GATE.md` Experiment B records the command, the SHA echoed back by `git rev-parse HEAD` inside the pinned container, and a per-clause table with file and line. A4 is **confirmed**, plus one clause research had not recorded (`cumulative trim <= num_samples_per_frame`, `temporal_unit_view.cc:71-79`).
- The shipping dependency graph is unchanged: `cargo tree -e normal,no-proc-macro` lists `iamf` and `thiserror`. The offline suite is **177 tests**, up from 143.

## Task Commits

1. **Task 1: One Audio Frame, trimmed and untrimmed, byte-exact (tracer, TDD)** — `f6d74e5` (test, RED) → `d37a23b` (feat, GREEN)
2. **Task 2: BCG channel-to-substream packing for 5.1 (TDD)** — `ef8fe6c` (test, RED) → `25178a9` (feat, GREEN)
3. **Task 3: Temporal Delimiter, Parameter Block context, temporal-unit trim rule (TDD)** — `164869e` (test, RED) → `5ced301` (feat, GREEN)

## TDD Gate Compliance

`workflow.tdd_mode` is enabled and this is a `type: tdd` plan. Every task ran RED → GREEN in that order, with a separate commit per gate.

| Task | RED | GREEN | REFACTOR | RED evidence |
|---|---|---|---|---|
| 1 | `f6d74e5` | `d37a23b` | — | `RED_EVIDENCE_OK` on `an_untrimmed_audio_frame_for_substream_0_reproduces_offset_0x78` (8 tests, 2 pass, 6 fail) |
| 2 | `ef8fe6c` | `25178a9` | — | `RED_EVIDENCE_OK` on `five_one_packs_as_two_coupled_pairs_then_two_monos` (10 tests, 1 pass, 9 fail) |
| 3 | `164869e` | `5ced301` | — | `RED_EVIDENCE_OK` on `a_non_multiple_sample_count_produces_exactly_one_trimmed_frame_at_the_end` (24 tests, 12 pass, 12 fail) |

Each RED was **intentional**, not incidental: the types, signatures and doc comments were committed with only the wire behaviour stubbed, so the named target test failed on a byte or value comparison rather than on a compile error, a zero-test discovery or a load crash. Every stub carried a `STUB(GREEN)` marker and `grep -rn "STUB(GREEN)" src/` now returns zero.

The evidence records were produced with **`tools/cargo-test-tap.sh`** — the committed helper plan 01-01 landed in `ee2d33c` and plan 01-05 did not find. It is a mechanical reformat of a real `cargo test` run into TAP 13, preserving cargo's exit code; no result was invented. `gsd_run check tdd-red-evidence` needs the TAP wrapped in a small JSON record (`command`, `exitCode`, `targetTest`, `output`); that wrapper lived in the session scratchpad. If later phases want the gate to keep working without rediscovering this, the wrapper belongs next to the TAP helper in `tools/`.

## Task 1's tracer feedback gate

Task 1 was `type="tracer"` with no `gate="blocking-human"`, and auto mode is active (`workflow._auto_chain_active` and `workflow.auto_advance` both `true`), so the gate re-ran the task's `<verify>` block end-to-end before any expansion: `cargo test --test temporal` (8 passed), `cargo clippy --all-targets -- -D warnings` (clean), `cargo test --test citations` (3 passed). All three passed — tracer verified end-to-end, expansion proceeded.

## Files Created/Modified

- `src/obu/audio_frame.rs` — TIME-01 and TIME-03. `AudioFrame`, `obu_type_for`/`substream_id_for` as an adjacent matched pair, both `validate()` rules, `FramePlan`/`plan_frames`, and `validate_temporal_unit`.
- `src/packing.rs` — TIME-02. `SubstreamPlan`, `SubstreamSpec`, `SubstreamChannels`, `pack_channels_to_substreams`, and a module doc that is half the deliverable: the ordering rule quoted, the channel-order table cited, and the no-permutation-constant reasoning written where a future reader would otherwise add one.
- `src/obu/parameter_block.rs` — TIME-05. `ParameterBlock`, `ParameterSubblock`, `ParameterData`, `MixGainParameterData`, `AnimationType`, `ParamDefinitionType`, `BlockDurationFields`, and the two context-taking wire functions.
- `src/obu/temporal_delimiter.rs` — TIME-04. Two bytes, and the comment recording that bit 6 is reserved at v1.1.0 and that `libiamf@v1.1.0`'s splitter would otherwise read two trim bytes out of the **next** OBU.
- `src/obu/mod.rs` — `read_obu_with_header` / `write_obu_with_header` plus module wiring.
- `src/error.rs` — ten new `ErrorKind` variants, all payload-free; `size_of::<Error>()` is unchanged at 32.
- `tests/temporal.rs` — 24 named vectors. `tests/packing.rs` — 10.
- `CONFORMANCE-GATE.md` — Experiment B, the A4 re-verification.

## Decisions Made

- **`AudioFrame` carries neither `trimming` nor `trailing`.** The plan's struct listing had both. The trim counts live in `ObuHeader::type_specific` and a copy in the payload makes a header/payload disagreement constructible — a file the reference mis-frames with nothing on our side able to see it, which is exactly the class Pattern 2 exists to prevent. A payload-level `trailing` would be permanently empty, because the frame claims the whole remainder by construction. `AudioFrame::into_obu` is the one place the header and payload are assembled consistently.
- **`read_parameter_block` takes three arguments, not two.** The plan's signature was `(cursor, &ParamDefinition)`. That cannot decide the *shape* of the subblock data, because the reference's type discriminator lives on its `ParamDefinition` subclass and this crate's `ParamDefinition` is deliberately the shared **wire prefix** only. A `ParamDefinitionType` argument keeps the dependency visible in the signature rather than smuggled into a struct field that has no wire representation — which strengthens TIME-05's claim rather than weakening it.
- **An unknown *animation* type is a typed error; an unknown *parameter definition* type is preserved verbatim.** The plan asked for both to be verbatim. Only the second is implementable: an unmodelled `param_definition_type` carries an explicit `parameter_data_size` on the wire, so "verbatim" has a boundary. An unrecognised `animation_type` carries no length at all, so preserving it verbatim would mean inventing where the next subblock starts. The reference returns `UnimplementedError` for exactly this reason and we match it.
- **Layouts beyond mono, stereo, binaural and 5.1 return `UnsupportedLayout`.** Modelling them would mean inventing an ordering the reference never states for them, and a wrong ordering here is the silent clean-decode-wrong-channels failure this module exists to prevent. A typed error is visible where a guess is not.
- **`validate_temporal_unit` enforces two clauses, not four.** A4 is confirmed at the pinned tag for all four, but the two timestamp clauses are `iamf-tools`' internal bookkeeping — timestamps are not a wire field of the Audio Frame OBU — and this crate has no temporal-unit timeline until Phase 2. Enforcing them would mean inventing a timeline to enforce them against.
- **`BlockDurationFields` is a new type rather than a reuse of `param_definition::DurationFields`.** The block-level fields are `duration` and `constant_subblock_duration` only; the per-subblock durations live inside each `ParameterSubblock`. Reusing the definition's three-field struct would give the same wire bytes two owners.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] `AudioFrame` dropped the plan's `trimming` and `trailing` fields**
- **Found during:** Task 1
- **Issue:** The plan listed `AudioFrame { substream_id, trimming: Option<Trimming>, payload, trailing }`. `trimming` duplicates `ObuHeader::type_specific`, making a header/payload disagreement constructible — the same defect plan 01-05 removed from `ParamDefinition`'s `param_definition_mode`. `trailing` duplicates `Obu::trailing` and would be permanently empty.
- **Fix:** Both dropped, with the reasoning on the type. `AudioFrame::into_obu(trimming)` assembles the pair.
- **Files modified:** `src/obu/audio_frame.rs`
- **Verification:** `reading_a_type_6_frame_derives_its_substream_id_and_consumes_no_id_bytes` asserts `obu.trailing.is_empty()` and a byte-exact re-serialisation.
- **Committed in:** `f6d74e5` / `d37a23b`

**2. [Rule 3 - Blocking] `read_obu_with_header` / `write_obu_with_header` added to `src/obu/mod.rs`**
- **Found during:** Task 1
- **Issue:** An Audio Frame parser must see `obu_type` to know whether an explicit `audio_substream_id` is present, but `read_obu_with`'s callback receives only the payload cursor. The alternative — an audio-frame-specific reader that parses its own header — would give the crate a **second `trailing` drain site**, which the module doc explicitly forbids (OBU-07).
- **Fix:** Header-aware variants added; the existing two became thin wrappers over them, so there is still exactly one drain.
- **Files modified:** `src/obu/mod.rs`
- **Verification:** Whole offline suite green, including the 45 descriptor vectors that go through the wrapper path.
- **Committed in:** `f6d74e5`

**3. [Rule 2 - Missing Critical] `read_parameter_block` takes a `ParamDefinitionType` third argument**
- **Found during:** Task 3
- **Issue:** The plan's two-argument signature cannot decide the subblock data shape, so the acceptance criterion "an unmodelled parameter definition type preserves its payload verbatim" was unimplementable as written.
- **Fix:** Third explicit argument, with the reason (this crate's `ParamDefinition` is the shared wire prefix; the reference's is a class hierarchy) in the module doc.
- **Files modified:** `src/obu/parameter_block.rs`
- **Verification:** `an_unmodelled_parameter_definition_type_preserves_its_payload_verbatim` round-trips `05 01 01 03 de ad be` unchanged.
- **Committed in:** `164869e` / `5ced301`

**4. [Rule 1 - Bug] Unknown animation types are a typed error, not preserved verbatim**
- **Found during:** Task 3
- **Issue:** The plan's behaviour line said "Values 3 and above are reserved and preserved verbatim". The wire carries no length for an unrecognised `animation_type`, so there is no boundary to preserve — any implementation would have to invent where the next subblock begins, and the `iamf-tools` reference returns `absl::UnimplementedError` for that reason.
- **Fix:** `ErrorKind::UnsupportedParameterData`, with a `// NOTE:` recording both the divergence from the plan's letter and why the reference behaves the same way.
- **Files modified:** `src/obu/parameter_block.rs`
- **Verification:** `an_unknown_animation_type_is_the_same_typed_error_the_reference_returns`
- **Committed in:** `164869e` / `5ced301`

**5. [Rule 2 - Missing Critical] `SubstreamPlan::for_layout` refuses layouts research did not close**
- **Found during:** Task 2
- **Issue:** The plan said layouts beyond mono/stereo/5.1 are "modelled but not exercised". Modelling their packing order without a citation would mean inventing an ordering — the exact silent failure mode (clean decode, scrambled channels) this module is built to prevent, and one no test in this phase could catch.
- **Fix:** `ErrorKind::UnsupportedLayout` for those layouts, with the reason on the constructor, plus a named test asserting the refusal.
- **Files modified:** `src/packing.rs`, `src/error.rs`, `tests/packing.rs`
- **Verification:** `a_layout_this_phase_does_not_model_is_a_typed_error_not_a_guess`
- **Committed in:** `ef8fe6c` / `25178a9`

**6. [Rule 2 - Missing Critical] `BlockDurationFields` rather than reusing `DurationFields`**
- **Found during:** Task 3
- **Issue:** The plan's artifact list implied reuse of `param_definition::DurationFields`, whose third field `subblock_durations` has no block-level wire representation — the block's per-subblock durations live inside each subblock. Carrying both would give the same bytes two owners.
- **Fix:** A two-field `BlockDurationFields`, with the distinction documented on the type.
- **Files modified:** `src/obu/parameter_block.rs`
- **Verification:** `a_mode_1_parameter_block_carries_its_own_duration_fields` round-trips byte-exactly.
- **Committed in:** `164869e` / `5ced301`

**7. [Rule 2 - Missing Critical] `validate_temporal_unit` added; the frame planner cannot see a temporal unit**
- **Found during:** Task 3
- **Issue:** The plan asked for the temporal-unit trim rule to be "asserted by the frame planner". A planner takes a sample count and a frame length; it never sees a set of frames, so it cannot assert a property *of* a set.
- **Fix:** A separate `validate_temporal_unit(&[Obu<AudioFrame>])` enforcing the two wire-fact clauses A4 confirmed.
- **Files modified:** `src/obu/audio_frame.rs`
- **Verification:** `a_temporal_unit_whose_frames_disagree_on_trim_is_rejected`
- **Committed in:** `164869e` / `5ced301`

**8. [Rule 2 - Missing Critical] `TemporalDelimiter` is a unit struct**
- **Found during:** Task 3
- **Issue:** The plan listed `TemporalDelimiter { trailing: Vec<u8> }`. `obu_size` is 0, so the bounded payload sub-reader is empty and the OBU-level `trailing` is empty with it; a payload-level remainder field would be permanently empty and would invite a second drain site.
- **Fix:** A unit struct, with the reason on the type.
- **Files modified:** `src/obu/temporal_delimiter.rs`
- **Verification:** `a_temporal_delimiter_round_trips_through_the_reader` asserts `obu.trailing.is_empty()`.
- **Committed in:** `164869e` / `5ced301`

---

**Total deviations:** 8 auto-fixed (6 missing critical, 1 blocking, 1 bug)
**Impact on plan:** Every one is a correction to the plan's letter in service of its stated intent. Six (1, 3, 4, 6, 7, 8) are cases where following the letter would have contradicted an acceptance criterion, a stated constraint, or the wire format itself. No requirement was widened or dropped; no scope was added.

## Issues Encountered

- **The RED-evidence gate wants a JSON record, not raw TAP.** `gsd_run check tdd-red-evidence <record.json>` expects `{command, exitCode, targetTest, output}` with the TAP text in `output`. `tools/cargo-test-tap.sh` produces the TAP faithfully; the JSON wrapper was a four-line scratchpad script. Promoting it to `tools/` alongside the TAP helper would close the loop for later phases.
- **`git.base-branch --is-protected` returned `false` for `gsd/phase-01-conformant-lpcm-bitstream`**, as expected — every commit landed on the phase branch and none on `main`.
- **Nothing else.** No auth gates, no package installs, no architectural questions.

## Known Stubs

None. `grep -rn "STUB(GREEN)" src/` returns zero — every stub committed for a RED gate was replaced by its GREEN commit. `grep -rn "allow(" src/` returns zero, so the D-21 escape census is still empty and PROF-03's single sanctioned `#[allow]` has not landed yet.

## User Setup Required

None. Every gate in this plan runs offline: `env -u IAMF_REF_DECODER cargo test --locked` is green with no reference binary present (CONF-10 holds). The A4 re-verification used the already-built, digest-pinned `iamf-tools:v2.1.0` container, and its result is recorded in `CONFORMANCE-GATE.md` so no later plan has to re-run it.

## Next Phase Readiness

**Ready.**

- **01-07 (profiles, Q7.8)** is untouched by this plan and still has everything plan 01-05 left it.
- **01-08 (the conformance fixture)** now has the complete encode path for the amended D-18 design: `SubstreamPlan::for_layout(Ch5_1)` gives the four substreams and the two counts the Audio Element must declare, `pack_channels_to_substreams` turns 24-bit big-endian interleaved PCM into the four frame payloads, `plan_frames` gives the frame count and the single end trim that a deliberate non-multiple length produces, and `AudioFrame::into_obu` writes each frame with its implicit type. `validate_temporal_unit` is available for the fixture's own self-check.
- **One thing 01-08 must still confront first**, unchanged from research: the 24-bit float→int round trip through `iamfdec` is `[ASSUMED]`, not verified — the empirical proof was at 16 bit. Run the 24-bit round trip before the fixture design is frozen; if it is not exact, that is a `CONFORMANCE-GATE.md` note, not a silent 16-bit downgrade.
- **Phase 2 (parser)** inherits the shape PARSE-02 needs: the Parameter Block's context is already an argument, so promoting it to a sequence-level registry is a change of *what* is passed, not of *whether* something is passed. It also inherits `read_obu_with_header`, which is what a sequence walker needs to dispatch on `obu_type` without acquiring a second drain site.

---
*Phase: 01-conformant-lpcm-bitstream*
*Completed: 2026-09-08*

## Self-Check: PASSED

All 6 created source/test files exist on disk and all 6 task commits resolve in
`git log --all`. `commits: 6` in the frontmatter is **measured** —
`git rev-list --count 90daf38..HEAD` against the ledger recorded before the first
commit — not narrated.
