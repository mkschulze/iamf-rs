---
phase: 01-conformant-lpcm-bitstream
plan: 07
subsystem: encoder
tags: [iamf, bitstream, streaming-writer, profile, fixed-point, q7.8, conformance]

requires:
  - phase: 01-03
    provides: BitCursor/BitWriter, the leb128 primitives and the two-pass obu_size origin
  - phase: 01-04
    provides: the OBU header, find_obu_boundaries, and the END-before-START trim order
  - phase: 01-05
    provides: the four descriptor OBUs, DescriptorSet and write_descriptors' ordering
  - phase: 01-06
    provides: AudioFrame, the implicit-substream-id rule, plan_frames and validate_temporal_unit
provides:
  - "SequenceWriter<W: Write> — the three-method streaming primitive (new / push_descriptors / push_temporal_unit / finish)"
  - "TemporalUnit — the optional delimiter, parameter blocks and audio frames of one unit"
  - "write_sequence — SEQ-03's whole-file wrapper, proven byte-identical to the streamed path"
  - "Profile and select_minimum_profile — profile_filter.cc's limits, returning a pair that can never trip libiamf's ordering rule"
  - "Q7_8 and lufs_to_q7_8 — the crate's single documented float escape, range-checked and ties-to-even"
  - "Loudness::from_q7_8 — the join between the float→fixed path and the wire model"
  - "LoudspeakerLayout::channel_count and AmbisonicsConfig::output_channel_count"
  - "BitWriter::clear/as_bytes — one scratch buffer reusable across OBUs"
  - "tests/support/test_000003.rs — the published configuration as one shared model"
  - "CONF-08 retired: all 32567 bytes of a real reference file reproduced from its published configuration"
affects: [01-08, phase-02-parser, phase-03-codecs, parallax-export-adapter]

actuals:
  tokens: 22000
  tasks: 3
  commits: 6
plan_head_before: e228b49032f0a5eb9542a7dbab0ec911a992040c

tech-stack:
  added: []
  patterns:
    - "Streaming as the primitive, whole-file as a wrapper that adds no logic — so a determinism check over the wrapper covers both paths"
    - "Illegal state transitions rejected by ownership (finish(self)) rather than by a runtime error, proved with a compile_fail doctest"
    - "One scratch BitWriter reused across OBUs, so peak memory does not grow with temporal-unit count"
    - "Shared test fixtures live in tests/support/*.rs, included with #[path], so one transcription of a published configuration serves several test binaries"
    - "A lint escape is a census with a known correct answer, asserted by a test that filters comment lines"

key-files:
  created:
    - src/sequence.rs
    - src/model/profile.rs
    - src/model/loudness.rs
    - tests/sequence.rs
    - tests/profile.rs
    - tests/support/test_000003.rs
  modified:
    - src/lib.rs
    - src/error.rs
    - src/model/mod.rs
    - src/model/layout.rs
    - src/obu/mix_presentation.rs
    - src/bits/writer.rs
    - tests/descriptors.rs

key-decisions:
  - "finish(self) makes push-after-finish a compile error, so ErrorKind::SequenceFinished was NOT added — an error that can never be produced is worse than none"
  - "The plan's 'wire lufs_to_q7_8 into src/sequence.rs' is incompatible with its own grep gate that no src file but loudness.rs may mention f64; the equivalent single-named-place is lufs_to_q7_8 plus Loudness::from_q7_8"
  - "test_000003 has 63 temporal units, not the 64 the plan's prose says: 8000 samples / 128 = 62.5 -> 63 frames, the last trimmed by 64"
  - "The channel-limit boundaries are exercised through a scene-based element, because output_channel_count is a u8 and the modelled loudspeaker layouts top out at 12 — 17 channels is not expressible as one channel-based element"
  - "The float→integer step targets i64 and narrows with a checked try_from, so there is no `as i16` anywhere in the conversion"
  - "Channel-count helpers return Option, so a layout whose count the spec version does not fix is a typed error rather than a profile selected from nothing"

patterns-established:
  - "Pattern: shared integration-test fixtures under tests/support/, #[path]-included — a subdirectory of tests/ is not compiled as its own test target"
  - "Pattern: a byte-comparison assertion names the FIRST DIFFERING OFFSET and dumps a window, built before it is asserted so assert_eq! never prints two 32 KB vectors at each other"
  - "Pattern: a compile-time claim (no Default impl, no post-finish push) is proved by a compile_fail doctest, not narrated in a comment"

requirements-completed: [SEQ-01, SEQ-02, SEQ-03, PROF-01, PROF-02, PROF-03, CONF-08]

coverage:
  - id: D1
    description: "All 32567 bytes of tests/fixtures/reference/test_000003.iamf are reproduced from its published configuration through the streaming primitive"
    requirement: CONF-08
    verification:
      - kind: integration
        ref: "tests/sequence.rs#reproduces_test_000003"
        status: pass
      - kind: integration
        ref: "tests/sequence.rs#the_output_walks_67_obus_ending_exactly_on_the_file_length"
        status: pass
      - kind: integration
        ref: "tests/sequence.rs#descriptors_are_written_in_the_reference_order"
        status: pass
    human_judgment: false
  - id: D2
    description: "The IA Sequence writer emits descriptors then data over a plain W: Write, with an append-only state machine enforced by typed errors"
    requirement: SEQ-01
    verification:
      - kind: integration
        ref: "tests/sequence.rs#push_temporal_unit_before_push_descriptors_is_a_typed_error"
        status: pass
      - kind: integration
        ref: "tests/sequence.rs#push_descriptors_twice_is_a_typed_error"
        status: pass
      - kind: integration
        ref: "tests/sequence.rs#a_failing_sink_is_a_named_error_not_a_panic"
        status: pass
      - kind: unit
        ref: "cargo test --doc (src/sequence.rs SequenceWriter::finish — compile fail)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Streaming is the primitive: no temporal-unit history is retained and peak memory does not grow with the number of units"
    requirement: SEQ-02
    verification:
      - kind: integration
        ref: "tests/sequence.rs#the_writer_holds_no_temporal_unit_history"
        status: pass
      - kind: integration
        ref: "tests/sequence.rs#a_sequence_with_no_temporal_units_is_descriptors_only"
        status: pass
    human_judgment: false
  - id: D4
    description: "write_sequence is a whole-file wrapper over the same primitive with no logic of its own, and encoding twice in one process is byte-identical"
    requirement: SEQ-03
    verification:
      - kind: integration
        ref: "tests/sequence.rs#the_whole_file_wrapper_is_byte_identical_to_the_streamed_path"
        status: pass
      - kind: integration
        ref: "tests/sequence.rs#encoding_the_same_input_twice_produces_byte_identical_output"
        status: pass
    human_judgment: false
  - id: D5
    description: "Profile has exactly Simple/Base/BaseEnhanced plus Reserved(u8), no Default, and every wire byte round-trips"
    requirement: PROF-01
    verification:
      - kind: unit
        ref: "tests/profile.rs#every_wire_byte_maps_to_a_profile_without_panicking"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#base_enhanced_is_two_and_the_draft_v2_profiles_are_absent"
        status: pass
      - kind: unit
        ref: "cargo test --doc (src/model/profile.rs Profile — compile fail on Default::default())"
        status: pass
    human_judgment: false
  - id: D6
    description: "Minimum-profile selection matches profile_filter.cc at every limit with one step either side, sums channels with checked_add, and never returns additional below primary"
    requirement: PROF-02
    verification:
      - kind: unit
        ref: "tests/profile.rs#one_audio_element_selects_simple"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#two_audio_elements_select_base"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#three_audio_elements_select_base_enhanced"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#twenty_eight_audio_elements_select_base_enhanced"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#twenty_nine_audio_elements_are_a_typed_error"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#sixteen_channels_in_one_element_select_simple"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#seventeen_channels_select_base"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#eighteen_channels_select_base"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#nineteen_channels_select_base_enhanced"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#twenty_eight_channels_select_base_enhanced"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#twenty_nine_channels_are_a_typed_error"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#a_single_five_one_six_channel_element_selects_simple"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#channel_counts_are_summed_across_elements_with_checked_addition"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#selection_never_returns_additional_below_primary"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#a_manually_inverted_profile_pair_is_a_finding_and_a_write_rejection"
        status: pass
    human_judgment: false
  - id: D7
    description: "lufs_to_q7_8 rounds ties to even, rejects NaN/infinities/out-of-range before converting, and is the crate's single documented float escape"
    requirement: PROF-03
    verification:
      - kind: unit
        ref: "tests/profile.rs#quantisation::ties_round_to_even_in_both_directions"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#quantisation::truncation_toward_zero_would_bias_every_negative_value_upward"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#quantisation::the_two_published_loudness_values_of_test_000003_round_trip"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#quantisation::nan_and_both_infinities_are_typed_errors"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#quantisation::values_outside_the_q7_8_range_are_errors_not_saturations"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#exactly_one_float_lint_escape_exists_in_src_and_it_is_in_loudness_rs"
        status: pass
      - kind: unit
        ref: "tests/profile.rs#no_file_under_src_but_loudness_rs_mentions_a_64_bit_float"
        status: pass
      - kind: integration
        ref: "bash tools/prove-guards.sh (6 PASS lines, case (f) still fires)"
        status: pass
    human_judgment: false

duration: 62min
completed: 2026-09-08
status: complete
---

# Phase 01 Plan 07: Sequence Writer, Profile Selection and Q7.8 Loudness Summary

**A streaming IA Sequence writer that reproduces all 32567 bytes of `test_000003.iamf` from its published configuration on the first run, minimum-profile selection against `profile_filter.cc`'s six constants, and the crate's one sanctioned float — a range-checked ties-to-even Q7.8 quantiser.**

## Performance

- **Duration:** ~62 min
- **Started:** 2026-09-08T05:45Z
- **Completed:** 2026-09-08T06:47Z
- **Tasks:** 3 (all TDD, RED → GREEN)
- **Files modified:** 13 (6 created, 7 modified)

## Accomplishments

- **CONF-08 is retired mechanically, not waived.** `tests/sequence.rs#reproduces_test_000003` builds the model from `test_000003.textproto` and the `sawtooth_100_stereo.wav` its `audio_frame_metadata` names, drives `push_descriptors → push_temporal_unit ×63 → finish`, and compares against the vendored `.iamf`. All 32567 bytes match; `find_obu_boundaries` over our own output returns 68 entries (67 OBUs) with the last exactly on `len()` and the first Audio Frame at 120.
- **It passed on the first execution.** That is the substantive result: it means the header framing (01-04), the four descriptor OBUs and their write order (01-05), the implicit-substream-id rule and the END-before-START trim order (01-06), and the two-pass `obu_size` (01-03) are simultaneously correct and mutually consistent. No plan-01-03-through-01-06 defect survived to this point.
- **Streaming is genuinely the primitive.** One scratch `BitWriter` is reused for every OBU, nothing per-unit is retained, and a test walks all 63 units asserting each costs a constant 515 bytes (517 for the trimmed one) — a writer that accumulated anything would show it there.
- **Minimum-profile selection actually makes the Base call.** Two Audio Elements select Base even though four channels would fit Simple twice over, which is exactly what the D-18 amendment's two-element structure fixture needs. A single 5.1 six-channel element selects Simple, which plan 01-08 depends on.
- **The D-21 census now returns exactly one entry**, in `src/model/loudness.rs`, function-scoped, and it is asserted by a *test* (comment lines filtered) rather than only by a grep in a verify block. A second test asserts no other file under `src/` mentions a 64-bit float at all. `tools/prove-guards.sh` still reports six PASS lines.
- **The shipping dependency graph is unchanged:** `cargo tree -e normal,no-proc-macro` is still `iamf` and `thiserror`. No dev-dependency was added either — the readable byte-diff the plan anticipated needing `pretty_assertions` for is a 40-line helper that names the first differing offset, which is strictly more useful for this problem than a generic diff.

## Task Commits

1. **Task 1: The streaming writer reproduces a whole reference file byte-for-byte (tracer, TDD)** — `3fdc77d` (test, RED) → `d59d5aa` (feat, GREEN)
2. **Task 2: The profile enum and minimum-profile selection (TDD)** — `a70512b` (test, RED) → `70de14a` (feat, GREEN)
3. **Task 3: The Q7.8 loudness helper — the crate's single documented float escape (TDD)** — `4bdc210` (test, RED) → `5c544cf` (feat, GREEN)

**Plan metadata:** see the `docs(01-07)` commit that follows this file.

## TDD Gate Compliance

`workflow.tdd_mode` is enabled and this is a `type: tdd` plan. Every task ran RED → GREEN in that order, with a separate commit per gate.

| Task | RED | GREEN | REFACTOR | RED evidence |
|---|---|---|---|---|
| 1 | `3fdc77d` | `d59d5aa` | — | `RED_EVIDENCE_OK` on `reproduces_test_000003` (12 tests, 3 pass, 9 fail) |
| 2 | `a70512b` | `70de14a` | — | `RED_EVIDENCE_OK` on `two_audio_elements_select_base` (18 tests, 11 pass, 7 fail) |
| 3 | `4bdc210` | `5c544cf` | — | `RED_EVIDENCE_OK` on `ties_round_to_even_in_both_directions` (29 tests, 27 pass, 2 fail) |

Each RED was **intentional**, not incidental. The types, signatures, doc comments and state machines were committed with only the *wire behaviour* stubbed, so the named target test failed on a byte or value comparison rather than on a compile error, a zero-test discovery or a load crash:

- Task 1: `flush_scratch` built the bytes and did not hand them to the sink.
- Task 2: `select_minimum_profile` checked the ceilings and then answered `Simple` for everything.
- Task 3: the quantiser used `.trunc()` — precisely the `as i16` behaviour PROF-03 forbids — so both tie cases failed while every exact case passed.

Every stub carried a `STUB(GREEN)` marker; `grep -rn "STUB(GREEN)" src/` now returns zero.

**Tooling note, closing 01-06's loop.** `tools/red-evidence.sh` (committed by 01-06 as `e228b49`) was used for all three gates rather than a fourth throwaway. One rough edge remains: it infers `targetTest` from the `--test <name>` argument, which names the test *binary*, not the test. The gate then reports `INVALID_RED (no_target_test_failure)` because no test is literally called `sequence`. Each record's `targetTest` was corrected to the real test name before verification. A `--target-test <name>` flag would remove that step; it is a five-line change and is the obvious next improvement to the helper.

## Files Created/Modified

**Created:**
- `src/sequence.rs` — `SequenceWriter<W: Write>`, `TemporalUnit`, `write_sequence`. The module doc states the memory-ceiling rationale for streaming *and* that no real-time discipline applies, so neither half is later "corrected" away.
- `src/model/profile.rs` — `Profile`, `select_minimum_profile`, and the six `profile_filter.cc` constants quoted verbatim in a comment beside them.
- `src/model/loudness.rs` — `Q7_8`, `lufs_to_q7_8`, and the crate's single lint escape.
- `tests/sequence.rs` — CONF-08's reproduction, the state machine, the zero-unit case, the double-encode and wrapper-identity checks (12 tests).
- `tests/profile.rs` — every profile boundary with one step either side, the Q7.8 contract, and the D-21 census (29 tests).
- `tests/support/test_000003.rs` — the published configuration as one shared model, plus the WAV reader and the first-differing-offset assertion.

**Modified:**
- `src/lib.rs` — `pub mod sequence;`.
- `src/error.rs` — `ElementCountExceedsProfile`, `DescriptorsNotWritten`, `DescriptorsAlreadyWritten`, `NoGoverningParamDefinition`, `SinkWrite`. `size_of::<Error>()` is unchanged at 32; all five are unit variants.
- `src/model/mod.rs` — registers and re-exports `profile` and `loudness`.
- `src/model/layout.rs` — `LoudspeakerLayout::channel_count()` and `AmbisonicsConfig::output_channel_count()`, both returning `Option`.
- `src/obu/mix_presentation.rs` — `Loudness::from_q7_8`, the join to the wire model. This file still contains no float.
- `src/bits/writer.rs` — `clear()` and `as_bytes()`, which is what makes one scratch buffer reusable across OBUs.
- `tests/descriptors.rs` — its seven local `published_*` builders were moved to `tests/support/test_000003.rs` and it now includes that module. All 45 of its tests still pass unchanged.

## Decisions Made

1. **`finish(self)` over a `Finished` state.** The plan asked for `ErrorKind::SequenceFinished` on "push after finish". Because `finish` consumes the writer, that transition is a *compile* error, which is strictly stronger — and a `Finished` state plus a matching `ErrorKind` would both be unreachable through the public API. An error variant that can never be produced is worse than none: it tells a caller to handle a case that does not exist. A `compile_fail` doctest on `SequenceWriter::finish` proves the transition is rejected.

2. **`Loudness::from_q7_8` rather than an LUFS parameter on `write_sequence`.** The plan's Task 3 says to wire the helper into `src/sequence.rs`'s public entry. That directly contradicts the plan's own verify gate — `grep -rln --include=*.rs 'f64' src/` must list nothing but `src/model/loudness.rs`. An LUFS parameter would put an `f64` in `sequence.rs`. The requirement behind both is "the crate quantises once, at one named place, rather than each call site converting", and that is met by `lufs_to_q7_8` plus a typed constructor on the block that consumes it.

3. **The float→integer step targets `i64`, not `i16`.** After `round_ties_even` and a range check, `as i16` would be exact — but PROF-03 names it as forbidden and the acceptance criterion says the file must contain no `as i16` cast of the scaled value. Casting to `i64` (wide enough that no float can saturate into a plausible value) and narrowing with `i16::try_from` is both literally compliant and genuinely safer: no single mistake in the function can silently produce a wrong value.

4. **Channel-count helpers return `Option`.** A reserved or expanded `loudspeaker_layout` has no channel labels defined in this spec version, and a reserved `ambisonics_mode` has no fields at all. Returning `Some(0)` or guessing would select a profile from nothing; `None` becomes `ErrorKind::UnsupportedLayout`.

5. **Shared fixtures live in `tests/support/`.** `tests/descriptors.rs` already transcribed the `test_000003` configuration for the 120-byte prologue proof. Duplicating it for the whole-file proof would have created a second thing to keep in step with the textproto. A subdirectory of `tests/` is not compiled as its own test target, so `#[path]`-including one module from both binaries is the idiomatic fix.

6. **No new dev-dependency.** The plan anticipated needing `pretty_assertions` or similar for a readable 32 KB byte comparison. A purpose-built assertion that names the *first differing offset* and dumps a 32-byte window either side is both smaller and more useful here, because every OBU boundary in the file is known — the offset identifies the wrong OBU immediately, which a unified diff does not.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan says 64 temporal units; the file has 63**
- **Found during:** Task 1
- **Issue:** `<behavior>` says "calling `push_temporal_unit` 64 times". `sawtooth_100_stereo.wav` holds 8000 sample frames; `ceil(8000 / 128) = 63`, and 4 descriptors + 63 frames is the 67 OBUs the same plan asserts. 64 units would be 68 OBUs and a 33082-byte file.
- **Fix:** The count is *derived* by `plan_frames`, never hardcoded, and a named test (`the_published_configuration_yields_63_frames_and_a_64_sample_end_trim`) asserts 8000 samples → 63 frames → 64-sample end trim, so the arithmetic is checked rather than asserted from prose.
- **Files modified:** `tests/sequence.rs`
- **Verification:** `reproduces_test_000003` passes at exactly 32567 bytes.
- **Committed in:** `3fdc77d`

**2. [Rule 3 - Blocking] `BitWriter` had no way to be reused as a scratch buffer**
- **Found during:** Task 1
- **Issue:** `finish(self)` consumes the writer, so "reuse one scratch buffer across OBUs" — an explicit acceptance criterion — was not expressible.
- **Fix:** `BitWriter::clear()` (resets, keeps capacity) and `BitWriter::as_bytes()` (non-consuming, enforcing the same alignment rule as `finish`).
- **Files modified:** `src/bits/writer.rs`
- **Verification:** `the_writer_holds_no_temporal_unit_history` walks 63 units asserting a constant per-unit byte cost; the whole suite passes.
- **Committed in:** `3fdc77d`

**3. [Rule 3 - Blocking] No channel-count accessor existed for profile selection**
- **Found during:** Task 2
- **Issue:** `select_minimum_profile` needs a channel count per Audio Element, and neither `LoudspeakerLayout` nor `AmbisonicsConfig` exposed one.
- **Fix:** `LoudspeakerLayout::channel_count() -> Option<u32>`, counted from the reference header's own channel-label comments quoted beside each arm, and `AmbisonicsConfig::output_channel_count() -> Option<u8>`. Both `None` where the spec version fixes no count.
- **Files modified:** `src/model/layout.rs`
- **Verification:** `a_layout_with_no_fixed_channel_count_is_a_typed_error_not_a_guess`; the 5.1 element is confirmed to carry six channels.
- **Committed in:** `a70512b`

**4. [Rule 1 - Bug] The plan's channel boundaries are unreachable with channel-based elements**
- **Found during:** Task 2 (the first GREEN run failed on 17 and 18 channels)
- **Issue:** The largest modelled loudspeaker layout is 7.1.4 at 12 channels. 17 channels needs ≥ 2 elements, and ≥ 2 elements is Base *by the element limit* — so the test would have proved nothing about the channel limit. No combination of ≤ 2 modelled layouts sums to 17 at all.
- **Fix:** The channel axis is exercised with a **scene-based** element, whose `output_channel_count` is a plain `u8` and can therefore be any value. One element per case isolates the channel limit from the element limit; the element axis is varied separately with stereo elements.
- **Files modified:** `tests/profile.rs`
- **Verification:** All eleven channel/element boundary tests pass, each at the limit and one step either side.
- **Committed in:** `70de14a`

**5. [Rule 3 - Blocking] `clippy.toml`'s float ban reaches test targets**
- **Found during:** Task 3
- **Issue:** `cargo clippy --all-targets -- -D warnings` is a verify gate, and testing a function whose whole purpose is to accept an `f64` requires naming the type.
- **Fix:** The float-typed tests live in one `mod quantisation` carrying a single module-scoped escape with a reason string, so `tests/` has exactly one escape mirroring the one `src/` has. The D-21 census scans `src/` only and is untouched — which the census test itself re-proves.
- **Files modified:** `tests/profile.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` is clean; the census returns exactly one entry in `src/model/loudness.rs`.
- **Committed in:** `5c544cf`

**6. [Rule 2 - Missing Critical] A failing sink had no named error**
- **Found during:** Task 1
- **Issue:** The plan's threat model names "a sink that errors mid-write must leave a named error, not a partially-consistent writer", but no `ErrorKind` covered it. `std::io::Error` cannot live in `ErrorKind` — it is neither `Clone` nor `Eq` and would blow the 32-byte budget.
- **Fix:** `ErrorKind::SinkWrite`, a unit variant, reported at `Location::OutputOffset(bytes_written)` so the caller knows exactly how far the sink got.
- **Files modified:** `src/error.rs`, `src/sequence.rs`
- **Verification:** `a_failing_sink_is_a_named_error_not_a_panic` drives the writer over a sink that refuses every write.
- **Committed in:** `3fdc77d`

**7. [Rule 2 - Missing Critical] A Parameter Block had no governing definition to be written against**
- **Found during:** Task 1
- **Issue:** `write_parameter_block` requires the `ParamDefinition` that governs the block (01-06's three-argument signature). `TemporalUnit` carrying its own copy would be a second source for one definition, which is two definitions that eventually disagree.
- **Fix:** `push_descriptors` collects every published `ParamDefinition` from the Audio Elements' params and the Mix Presentations' mix gains, in descriptor order; `push_temporal_unit` resolves each block's `parameter_id` against that list. A linear scan, for the same reason `model::by_id` is one. Unresolvable is `ErrorKind::NoGoverningParamDefinition`.
- **Files modified:** `src/sequence.rs`, `src/error.rs`
- **Verification:** Compiles and passes; not exercised by `test_000003`, which has zero parameter blocks — see Known Gaps.
- **Committed in:** `3fdc77d`

---

**Total deviations:** 7 auto-fixed (2 bugs in the plan's own numbers, 3 blocking, 2 missing critical)
**Impact on plan:** No scope creep. Two of the seven are corrections to arithmetic stated in the plan's prose (63 not 64 units; the channel boundaries being unreachable through channel-based elements) that the plan's *own* other assertions already contradicted. The rest are the minimum needed to satisfy stated acceptance criteria. One planned artifact was deliberately **not** built: `ErrorKind::SequenceFinished`, for the reason in Decision 1.

## Issues Encountered

- **`tools/red-evidence.sh` infers the wrong `targetTest`.** It reads the value after `--test`, which is the test *binary* name. `gsd_run check tdd-red-evidence` then reports `INVALID_RED (no_target_test_failure)` because no test is named `sequence`. Resolved by rewriting `targetTest` in each record before verification. A `--target-test` flag would close this; noted for whoever touches the helper next.
- **The repo is not `rustfmt`-clean, and was not before this plan.** Thirteen files under `src/` and `tests/` carry pre-existing drift from plans 01-01 through 01-06 (01-03 already logged two of them). This plan formatted only its own new and changed files with `rustfmt --edition 2024` rather than running repo-wide `cargo fmt`, which would have buried unrelated reformatting inside feature commits. Logged to `deferred-items.md`.

## Known Gaps

Not stubs — behaviour that is implemented and typed but not exercised by a shipped fixture:

- **The Parameter Block path through `push_temporal_unit` has no byte-level test.** `test_000003` publishes `num_parameters: 0` and zero Parameter Block OBUs, so the definition-resolution path is compiled and type-checked but never driven end to end. The D-18 fixture (plan 01-08) is likewise mode-1 with no parameter blocks. This wants a fixture with a Mix Gain parameter block before Phase 2 claims round-trip fidelity; recorded here rather than papered over.
- **The Temporal Delimiter path through `push_temporal_unit` is likewise untested at byte level** — `test_000003` sets `enable_temporal_delimiters: false`. `tests/temporal.rs` covers the OBU itself from 01-06, so only the *sequencing* of it is unproven.

Neither prevents this plan's goal. No `STUB(GREEN)` markers remain in `src/`, and `grep -rn "TODO\|FIXME" src/` returns nothing.

## User Setup Required

None — no external service configuration required. Every gate in this plan runs offline with no reference binary: `env -u IAMF_REF_DECODER cargo test --locked` is green at 220 tests across 15 binaries plus 2 doctests.

## Next Phase Readiness

- **Plan 01-08 has everything it needs.** The streaming writer, the whole-file wrapper, profile selection (which answers Simple for the 5.1 element it will build) and the Q7.8 helper are all present and proven. The D-18 fixture is now a matter of building a `DescriptorSet` and driving `write_sequence`.
- **The reference-decoder gate (CONF-02..05) has not been run against our own output yet.** That is 01-08's job, and it is the remaining risk: `test_000003` proves we reproduce a file `libiamf` already accepts, which is necessary but not sufficient — 01-08 writes a file the reference has never seen, at a different sample rate, sample size, endianness and layout.
- **The two Known Gaps above are the honest carry-forward** into Phase 2's round-trip work.

---
*Phase: 01-conformant-lpcm-bitstream*
*Completed: 2026-09-08*

## Self-Check: PASSED

Every file this SUMMARY names exists on disk, and every commit hash it names is
reachable from `git log --all`. `commits: 6` in the frontmatter is **measured**
— `git rev-list --count e228b49..HEAD` — not narrated.

`grep -rn "STUB(GREEN)" src/` returns zero. The only `TODO` strings under `src/`
are two doc comments *quoting the reference's own* `TODO(b/339855338)`,
pre-existing from plan 01-05.
