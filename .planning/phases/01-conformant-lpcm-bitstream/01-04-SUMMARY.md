---
phase: 01-conformant-lpcm-bitstream
plan: 04
subsystem: obu-framing
tags: [rust, obu, header, uleb128, boundaries, tdd, conformance, iamf]

requires:
  - "01-01 — the lint set, the Error/ErrorKind/Location surface, tools/cargo-test-tap.sh, tests/citations.rs"
  - "01-02 — the vendored reference corpus (39 `.iamf` files) and tests/fixtures/MANIFEST.md"
  - "01-03 — BitCursor/BitWriter/leb128, and sub_reader as Pattern 3's bounded payload view"
provides:
  - "`iamf::obu::ObuType` — all 32 five-bit values, with `is_audio_frame()` and `is_redundant_copy_allowed()` as the reference's two legality tables"
  - "`iamf::obu::TypeSpecific` — the TWO-variant v1.1.0 bit-6 model; `Trimming(Option<Trimming>)` and `Reserved`"
  - "`iamf::obu::ObuHeader` with derived `trimming_status_flag()` / `extension_flag()`, and `Trimming { at_end, at_start }`"
  - "`iamf::obu::write_obu` / `read_obu_header` — byte 0, the two-pass measured obu_size, the after-size fields in END-then-START-then-extension order"
  - "`iamf::obu::Obu<T>` / `read_obu_with` / `write_obu_with` — the single central `trailing` drain site (OBU-07) and D-05's precedence rule"
  - "`iamf::obu::find_obu_boundaries` — OBU-08, proven over the whole vendored corpus offline"
  - "tests/refcorpus.rs — CONF-10's always-on offline layer, 39 files walked in sorted path order"
  - "`bits::minimal_uleb128_len` as a pub(crate) re-export, so the OBU layer can derive obu_size's own ceiling"
  - "REQUIREMENTS.md OBU-03 amended to the two-variant model with its evidence; PITFALLS.md §2 annotated with its provenance"
affects:
  - "01-05 — every descriptor OBU is written through write_obu_with and read through read_obu_with; D-05's precedence rule is now stated where Reserved.raw will land"
  - "01-06/01-07 — Audio Frame trimming is expressible only on Audio Frames, by construction"
  - "01-08 — find_obu_boundaries is the first structural check on our own encoder output"
  - "Phase 2 — read_obu_header_parts measures size_of_obu_size rather than deriving it, so a non-minimal foreign obu_size (PARSE-04) does not shift every later offset"

actuals:
  tokens: 15967
  tasks: 4
  commits: 7

plan_head_before: 38004482519fd484c9233defcb19564021f03eb0

tech-stack:
  added: []
  patterns:
    - "Pattern 1 two-pass sizing, factored so its guard is testable: `obu_size_for(after, payload_len)` owns the alignment check, the checked addition and the derived ceiling, and `write_obu` is then linear"
    - "Pattern 2 applied twice: both gate flags in byte 0 are methods over the data they gate, so a flag/field disagreement is unconstructible rather than validated"
    - "The reader is deliberately less strict than the writer. `write_obu` refuses an illegal flag; `read_obu_header` accepts it, because rejecting a file the reference accepts is as much a conformance failure as accepting one it rejects"
    - "Derived lengths are measured from the cursor, never recomputed from the value — a non-minimal uleb128 is legal on the wire, so `minimal_len` is the wrong instrument on the read side"
    - "Negative fixtures are built by in-memory mutation of a vendored file; a corrupted file committed to git is a file someone eventually treats as a golden"

key-files:
  created:
    - src/obu/mod.rs
    - src/obu/header.rs
    - src/obu/boundaries.rs
    - tests/obu_header.rs
    - tests/refcorpus.rs
  modified:
    - src/lib.rs
    - src/bits/mod.rs
    - .planning/REQUIREMENTS.md
    - .planning/research/PITFALLS.md

decisions:
  - "The planned Task 1 / Task 2 boundary was moved: trim landed in Task 1, not Task 2. Task 1's own central claim — that obu_size is measured across the after-size fields — has exactly one vector that can prove it (0x7D32, obu_size 514 = 2 + 512), and that vector needs trim. Leaving trim to Task 2 would have left the tracer proving only `obu_size == payload.len()`, and would have made Task 2's RED gate invalid for the differing-trim vector."
  - "`ObuType` spells all eighteen Audio Frame variants rather than carrying a `u8`. The low bits are an implicit `audio_substream_id`, so they are eighteen meanings; a newtype with a private field would have worked too, but only by adding a public type the plan's export list does not name."
  - "`read_obu_header` keeps the planned `(ObuHeader, u32)` signature; a `pub(crate) read_obu_header_parts` alongside it also reports the after-size byte count. The payload length is then measured rather than derived through `minimal_len`, which would be wrong for a legal non-minimal `obu_size`."
  - "The boundary walker reads byte 0 as a single 8-bit read and interprets none of its flags. That is correct precisely because `obu_size` counts the after-size fields; a walker that interpreted bit 6 would be wrong on exactly the descriptors whose bit 6 is set illegally."
  - "`find_obu_boundaries` checks the 2 MiB ceiling before the bounds check, so a 2 MiB claim inside a small buffer reports the real defect rather than its consequence."
  - "An empty input yields `[0]`, so OBU-08's property holds degenerately rather than being a special case every caller has to know about."

patterns-established:
  - "In an integration test file, helper functions outside a `#[test]` body are NOT covered by GUARD-04's `allow-expect-in-tests` carve-out. Helpers therefore return `Option`/`Vec` and assert in the caller, with no panic path at all."
  - "A verification instrument that counts unchecked requirement boxes breaks the moment the requirement is marked complete. Count `- [[ x]]`, not `- [ ]`."

requirements-completed:
  - OBU-01
  - OBU-02
  - OBU-03
  - OBU-04
  - OBU-05
  - OBU-06
  - OBU-07
  - OBU-08
  - CONF-10

coverage:
  - id: D1
    description: "Byte 0 is MSB-first `[obu_type:5][redundant:1][trimming:1][extension:1]`"
    requirement: OBU-01
    verification:
      - kind: unit
        ref: "tests/obu_header.rs — 0xF8 (type 31), 0x20 (type 4), 0x30/0x32 (type 6 untrimmed/trimmed), 0x04 (type 0 redundant), 0xF9/0x33 (extension)"
        status: pass
      - kind: integration
        ref: "xxd tests/fixtures/reference/test_000003.iamf at 0x00, 0x78, 0x7D32 — f8, 30, 32"
        status: pass
    human_judgment: false
  - id: D2
    description: "obu_size counts every byte after itself and excludes byte 0 and the size bytes"
    requirement: OBU-02
    verification:
      - kind: unit
        ref: "tests/obu_header.rs#offset_0x7d32_a_trimmed_audio_frame_writes_as_32_82_04_40_00 — 514 = 2 trim + 512 payload"
        status: pass
      - kind: unit
        ref: "src/obu/header.rs tests#obu_size_counts_the_after_size_bytes_as_well_as_the_payload, #the_ceiling_is_derived_from_the_size_fields_own_length"
        status: pass
    human_judgment: false
  - id: D3
    description: "Bit 6 has exactly two variants, and the draft-v2.0.0 ones are provably absent from src/"
    requirement: OBU-03
    verification:
      - kind: integration
        ref: "grep -rnE 'IsNotKeyFrame|OptionalFields|is_not_key_frame|optional_fields_flag' src/ | wc -l -> 0"
        status: pass
      - kind: unit
        ref: "tests/obu_header.rs#the_trimming_flag_is_refused_on_every_non_audio_frame_type (7 types), #the_trimming_flag_is_accepted_on_every_audio_frame_type (19 types)"
        status: pass
      - kind: integration
        ref: ".planning/REQUIREMENTS.md OBU-03 amended with IsTrimmingStatusFlagAllowed and IAMF_OBU_split cited; .planning/research/PITFALLS.md §2 carries the provenance note"
        status: pass
    human_judgment: false
  - id: D4
    description: "obu_redundant_copy refused on 3..=23 at write time and accepted on descriptors"
    requirement: OBU-04
    verification:
      - kind: unit
        ref: "tests/obu_header.rs#a_redundant_copy_is_refused_on_parameter_blocks_delimiters_and_audio_frames (21 types), #a_redundant_copy_round_trips_on_a_descriptor"
        status: pass
    human_judgment: false
  - id: D5
    description: "Trim fields written END first, then START — proven by a vector whose two values differ"
    requirement: OBU-05
    verification:
      - kind: unit
        ref: "tests/obu_header.rs#the_trim_fields_are_written_end_first_then_start — 40 07, not 07 40"
        status: pass
      - kind: unit
        ref: "tests/obu_header.rs#a_trimmed_audio_frame_reads_its_trim_values_back_in_the_same_order"
        status: pass
    human_judgment: false
  - id: D6
    description: "Extension header read and written verbatim, after both trim fields, with its length bounds-checked before allocation"
    requirement: OBU-06
    verification:
      - kind: unit
        ref: "tests/obu_header.rs#an_extension_header_writes_its_size_then_its_bytes_and_obu_size_counts_both, #the_extension_header_is_written_after_both_trim_fields, #an_extension_length_past_the_end_of_the_input_is_refused_without_allocating"
        status: pass
    human_judgment: false
  - id: D7
    description: "trailing is drained at exactly one central site, empty on fresh construction, and appended last on write"
    requirement: OBU-07
    verification:
      - kind: unit
        ref: "tests/obu_header.rs#a_payload_parser_that_under_reads_leaves_the_remainder_in_trailing, #a_payload_parser_that_consumes_everything_leaves_trailing_empty, #a_freshly_constructed_obu_has_empty_trailing_and_serialises_without_it, #an_under_read_obu_re_serialises_to_the_bytes_it_was_read_from"
        status: pass
      - kind: integration
        ref: "src/obu/mod.rs — one `read_uint8_span(remaining)` drain; D-05's precedence rule stated in the module doc comment"
        status: pass
    human_judgment: false
  - id: D8
    description: "find_obu_boundaries lands its final boundary exactly on bytes.len(), for every vendored file, offline"
    requirement: OBU-08
    verification:
      - kind: integration
        ref: "tests/refcorpus.rs#every_vendored_iamf_walks_to_its_own_length — 39 files, sorted order, count asserted against MANIFEST.md"
        status: pass
      - kind: unit
        ref: "tests/refcorpus.rs#test_000003_walks_67_obus_ending_exactly_on_32567 — 68 entries, 0/8/26/40, first frame at 120, trimmed frame at 32050"
        status: pass
      - kind: unit
        ref: "tests/refcorpus.rs — three negative cases: truncated, obu_size past the buffer, obu_size above the 2 MiB ceiling"
        status: pass
    human_judgment: false
  - id: D9
    description: "The whole offline suite is green with IAMF_REF_DECODER unset and no reference binary present"
    requirement: CONF-10
    verification:
      - kind: integration
        ref: "env -u IAMF_REF_DECODER cargo test --locked -> 10 targets, 98 tests, 0 failed"
        status: pass
      - kind: integration
        ref: "env -u IAMF_REF_DECODER cargo test --release -> 10 targets green, so overflow behaviour matches in both profiles"
        status: pass
    human_judgment: false
  - id: D10
    description: "The new module carries no panic path, no raw index, no unchecked arithmetic and no lint escape"
    requirement: OBU-08
    verification:
      - kind: integration
        ref: "production code under src/obu/ grepped for unwrap()/expect(/..] -> 0; grep -rn 'allow(' src/ -> 0"
        status: pass
      - kind: integration
        ref: "cargo clippy --all-targets -- -D warnings -> clean; 6 checked_* steps in boundaries.rs"
        status: pass
      - kind: integration
        ref: "cargo tree -e normal,no-proc-macro --prefix none | sort -u -> exactly `iamf` and `thiserror`"
        status: pass
    human_judgment: false

duration: 42min
completed: 2026-09-08
status: complete
---

# Phase 1 Plan 04: OBU Header and Boundaries Summary

**The OBU header, the two-pass `obu_size` origin and `find_obu_boundaries()` — with the phase's one factually wrong requirement corrected in code *and* in the document that carried it: bit 6 has TWO meanings at the pinned v1.1.0 tree, not four, and the four-variant model would have shifted every descriptor payload two bytes on the exact decoder whose acceptance is the Core Value.**

## Performance

- **Duration:** 42 min
- **Tasks:** 4 of 4
- **Commits:** 7 (measured — see below)
- **Files created/modified:** 9

## Accomplishments

- **The correction landed in three places, not one.** `TypeSpecific` has exactly two variants and the type carries the reason on its own doc comment; `write_obu` refuses the illegal state before a bit is emitted; and `REQUIREMENTS.md` OBU-03 now reads as the two-variant model with its evidence spelled out — `IsTrimmingStatusFlagAllowed` at `obu_header.cc:60-70`, the grep that returns nothing at v2.1.0 and everything at `main`, and `IAMF_OBU_split`'s unguarded trim read at `IAMF_OBU.c:64-120`. `PITFALLS.md` §2 keeps its original text and gains a provenance note above it. Code that quietly does the right thing while the requirement still says the wrong thing is how the next phase re-derives the bug; that is the failure mode this plan existed to close, and `.planning/WINDOWS.md` entry 5 is the standing example of what happens when it is not closed.

- **`obu_size` is measured, and the vector that proves it is the trimmed frame.** `obu_size_for(after, payload_len)` takes an already-serialised after-size buffer, refuses it if it is not byte-aligned, adds the payload length with `checked_add`, and checks the result against `kEntireObuSizeMaxTwoMegabytes - 1 - size_of_obu_size` — the reference's own derived bound, computed from the candidate value because the size field's length depends on the value it carries. The vector at `0x7D32` (`32 82 04 40 00`, `obu_size` 514 = 2 trim bytes + 512 payload bytes) is the only one in the file that can distinguish "counts the after-size fields" from "equals the payload length", which is why it moved into the tracer task.

- **The differing-trim vector exists, and it is the only thing that could ever catch a swap.** `40 07`, not `07 40`. Every Audio Frame in every one of the 39 vendored files has `at_start == at_end == 0` or equal values, so the whole reference corpus is byte-identical under both orders. A `// NOTE:` sits beside the write pair saying exactly that, because the next reader will assume END-before-START is a typo.

- **`find_obu_boundaries` walks all 39 vendored files to their exact length, offline.** 226 real reference files were walked during research; 39 of them are in this repo and every one lands its final boundary on `bytes.len()`, including the deliberately-invalid `negative/tones_256samp_5p1_pcm.iamf` — whose `num_samples_per_frame = 0` is a payload defect, not a framing one. `test_000003` gives 68 entries: 67 OBUs (4 descriptors, 63 audio frames), the first four at `0/8/26/40`, the first Audio Frame at **120** — confirming research's correction of `PROJECT.md`'s 118 — and the last at 32567. The walked file count is asserted equal to `MANIFEST.md`'s declared 39, so a fixture silently dropped fails the test rather than shrinking it.

- **The reader is deliberately less strict than the writer, and that is written down where it happens.** `write_obu` refuses a trimming flag on a non-Audio-Frame and a redundant copy on types 3..=23. `read_obu_header` applies neither rule, and reads the trim fields for *any* type whose bit 6 is set — which is precisely what `libiamf@v1.1.0` does, so what we parse is what the pinned decoder sees. Being stricter on read would reject files the reference accepts and break Phase 2's foreign-file round-trip; being looser on write would emit the silent two-byte shift. The asymmetry is the point.

- **Nothing under `src/obu/` can panic, and it is measured.** Zero `unwrap()`/`expect()`/raw index in production code, zero `#[allow]` anywhere in `src/`, six `checked_*` steps in the boundary walker, and `clippy --all-targets -- -D warnings` clean with `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used` and `panic` all at deny. `cargo tree -e normal,no-proc-macro` is still exactly `iamf` + `thiserror`.

## Task Commits

| Task | Phase | Commit | What |
|---|---|---|---|
| 1 (tracer) | RED | `0178354` | 11 hand-decoded vectors + the type shape. Gate: exit 101, 11 tests, 1 pass, 10 fail, `RED_EVIDENCE_OK`. |
| 1 (tracer) | GREEN | `e08b8ca` | Byte 0, the two-pass measured `obu_size`, the trim pair, all 32 `ObuType` values. 11/11 + 5 in-crate. |
| 1 (tracer) | gate | — | Tracer feedback gate re-ran all three `<verify>` commands end-to-end under auto mode; all passed, so expansion proceeded. |
| 2 | RED | `adef270` | 12 more vectors + `Obu<T>`/`read_obu_with`/`write_obu_with` shape. Gate: exit 101, 23 tests, 13 pass, 10 fail, `RED_EVIDENCE_OK`. |
| 2 | GREEN | `b2a0d6e` | Extension header, both legality rules, the central `trailing` drain. 23/23. |
| 3 | RED | `e921d6e` | `tests/refcorpus.rs` + the walker skeleton. Gate: exit 101, 6 tests, 0 pass, 6 fail, `RED_EVIDENCE_OK`. |
| 3 | GREEN | `ac05505` | `find_obu_boundaries`, all 39 files walking to length. 6/6. |
| 4 | — | `7c4b6ab` | OBU-03 amended with its evidence; PITFALLS.md §2 annotated with its provenance. |

**Measured:** `git rev-list --count 3800448..HEAD` = **7** commits (`plan_head_before: 38004482519fd484c9233defcb19564021f03eb0`). No REFACTOR commits: the only cleanup that arose — factoring the after-size byte count out of the header reader so the payload length is measured rather than derived — was folded into Task 2's GREEN, so every commit on the branch is green under `cargo clippy -D warnings`.

## TDD Gate Compliance

| Task | RED | GREEN | REFACTOR | Status |
|---|:--:|:--:|:--:|---|
| 1 (tracer) | ✓ `0178354` | ✓ `e08b8ca` | — | Pass |
| 2 | ✓ `adef270` | ✓ `b2a0d6e` | — | Pass |
| 3 | ✓ `e921d6e` | ✓ `ac05505` | — | Pass |
| 4 | n/a (`type="auto"`, documentation) | — | — | Pass |

Three `test(01-04)` commits and three `feat(01-04)` commits, each RED preceding its GREEN. All three RED runs were verified with `gsd-tools check tdd-red-evidence` and returned `RED_EVIDENCE_OK`; the evidence records name the target test, its expected result and its actual result, and each target test failed on an assertion for the planned behaviour rather than on a build error, a zero-test discovery or an unrelated failure.

**Task 2's target test was `the_trimming_flag_is_refused_on_every_non_audio_frame_type`,** and its actual result is the whole reason this plan exists: `emit_err()` returned `None` — the writer emitted the exact state `libiamf@v1.1.0` mis-frames by two bytes, with no complaint from anything.

## Files Created/Modified

| File | What it does |
|---|---|
| `src/obu/header.rs` | `ObuType` (32 values, both legality tables as `const fn`), the two-variant `TypeSpecific` with the correction recorded on it, `Trimming`, `ObuHeader` with derived flags, `write_fields_after_obu_size` + its END-then-START `NOTE:`, `obu_size_for`, `validate_header`, `write_obu`, `read_obu_header` / `read_obu_header_parts`, `read_extension_header`, and five in-crate tests for the `pub(crate)` surface. |
| `src/obu/mod.rs` | The module boundary, `Obu<T>`, the single `trailing` drain in `read_obu_with`, `write_obu_with` appending `trailing` last, and D-05's precedence rule in the module doc comment. |
| `src/obu/boundaries.rs` | `find_obu_boundaries`. Reads byte 0 and `obu_size` only, measures `size_of_obu_size` rather than deriving it, checks the ceiling before the bounds, six `checked_*` steps, and a module comment on why interpreting bit 6 here would be wrong. |
| `tests/obu_header.rs` | 23 hand-decoded vectors, each keyed to the `test_000003.iamf` offset it reproduces where one exists. |
| `tests/refcorpus.rs` | CONF-10's offline layer: the sorted corpus walk with the MANIFEST count assertion, the `test_000003` pin, and three in-memory negative cases. |
| `src/lib.rs` | `pub mod obu;`. |
| `src/bits/mod.rs` | `pub(crate) use leb128::minimal_len as minimal_uleb128_len` — the OBU layer needs it to derive `obu_size`'s own ceiling. |
| `.planning/REQUIREMENTS.md` | OBU-03 rewritten to the two-variant model with a dated amendment note carrying all three pieces of evidence and an explicit "OBU-04 is unaffected". |
| `.planning/research/PITFALLS.md` | §2 keeps its original four-variant text and gains a provenance note above it naming `iamf-tools@main` as the source. |

## Decisions Made

**The Task 1 / Task 2 boundary moved: trim landed in the tracer.** The plan scoped Task 1 to "the reserved `TypeSpecific` path and the no-trim, no-extension case". Executed as written, the tracer's only evidence for its own central claim — that `obu_size` is measured across the after-size fields — would have been a case where the after-size buffer is empty and `obu_size` therefore *equals* `payload.len()`. That distinguishes nothing. The one vector in `test_000003.iamf` that can distinguish it is the trimmed frame at `0x7D32`, and it needs trim. Leaving trim to Task 2 would also have made Task 2's RED gate invalid for the differing-trim vector, since Task 1 would have had to implement the write order anyway to satisfy anything. Task 2 kept the extension header, both legality rules and the whole `trailing` mechanism — ten genuinely failing tests, and its RED gate is sound.

**`read_obu_header` keeps its planned signature; a sibling reports the after-size byte count.** The obvious implementation of "payload length = `obu_size` − after-size bytes" is to recompute the after-size length from the header, or to derive `size_of_obu_size` from `minimal_len(obu_size)`. Both are wrong for a foreign file: a legal `obu_size` may be a **non-minimal, fixed-size** uleb128 (`LebGenerator::kFixedSize`, the `test_000134` configuration, Phase 2's PARSE-04), and deriving its length from its value puts every subsequent byte offset in the file wrong — for exactly the files that are hardest to debug. `read_obu_header_parts` measures it from the cursor instead. The same reasoning applies in `find_obu_boundaries`, which measures rather than derives for the same reason.

**All eighteen Audio Frame variants are spelled out.** `AudioFrameId0` through `AudioFrameId17` are the reference's own shape, and the low bits of the type carry an implicit `audio_substream_id` — eighteen meanings, not one type with a parameter. A `u8` payload would have made `AudioFrameId(200)` constructible; a newtype with a private field would have fixed that but only by adding a public type the plan's export list does not name. `every_five_bit_value_round_trips_through_the_type_enum` covers all 32 values, so the two long matches cannot drift apart.

**The 2 MiB ceiling is checked before the bounds check in the walker.** A file claiming a 2 MiB OBU inside a 5-byte buffer is over the ceiling *and* past the end. Reporting `ObuTooLarge` names the defect; reporting `TruncatedObu` names its consequence and sends the reader looking at the wrong thing.

**An empty input yields `[0]`.** The alternative — an empty vector, or an error — makes OBU-08's "last boundary equals `bytes.len()`" a rule with an exception, and an exception in a one-line property is an exception every caller has to remember.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] The planned Task 1 scope would have left the tracer's central claim unproven and Task 2's RED gate invalid**

- **Found during:** Task 1 planning, before the RED commit.
- **Issue:** Task 1 was scoped to exclude trim. Its stated behaviour — "`obu_size` is computed by serialising the after-size fields into a scratch buffer and adding the payload length" — has no observable consequence when the after-size buffer is always empty, and the one vector that makes it observable (`0x7D32`) requires trim. Deferring trim would also have forced Task 2's differing-trim vector to pass on arrival, which is `unexpected_green` in the fail-fast rules.
- **Fix:** Trim moved into Task 1 (three vectors: `30 80 04`, `32 82 04 40 00`, and the differing `40 07`). Task 2 kept the extension header, both legality rules and the `trailing` mechanism. Both RED gates verified `RED_EVIDENCE_OK`.
- **Files modified:** none beyond the planned set.
- **Verification:** Task 2's RED ran 23 tests, 13 pass, 10 fail — all ten of Task 2's own behaviours.

**2. [Rule 1 — Bug] The plan's boundary count for `test_000003.iamf` is off by one against its own definition**

- **Found during:** Task 3.
- **Issue:** The plan says `find_obu_boundaries` "returns 67 boundaries whose last element is exactly 32567" *and* that the vector "includes the starting offset of every OBU **and** the final end offset". Both cannot hold: the file has 67 OBUs, so start offsets plus the final end is **68** entries.
- **Fix:** Implemented the definition (68 entries) and asserted both numbers explicitly — `boundaries.len() == 68` and `boundaries.len() - 1 == 67` OBUs — so the file's structure is stated rather than inferred from a length.
- **Files modified:** `tests/refcorpus.rs`.
- **Verification:** `4 descriptors + 63 frames = 67`; `120 + 62 × 515 + 517 = 32567` reproduces the file length arithmetically from the frame sizes.

**3. [Rule 1 — Bug] Task 4's third verify instrument breaks the moment a requirement is marked complete**

- **Found during:** Task 4 verification.
- **Issue:** The gate is `grep -cE '^- \[ \] \*\*[A-Z]+-[0-9]+\*\*|^\| [A-Z]+-[0-9]+ \|'` and must not drop below its pre-task value. It counts only **unchecked** boxes, so `requirements mark-complete` — which this plan runs for nine IDs immediately afterwards — turns `- [ ]` into `- [x]` and drops the count by nine on a correct file.
- **Fix:** Ran the corrected instrument `'^- \[[ x]\] \*\*[A-Z]+-[0-9]+\*\*|^\| [A-Z]+-[0-9]+ \|'`, which counts checked and unchecked alike. **176 before the amendment and 176 after** — no requirement or traceability row was added, removed or renumbered. The plan's own instrument also reads 151 before and 151 after, because the amendment itself checks nothing off.
- **Files modified:** none.

**4. [Rule 3 — Blocking] GUARD-04's `allow-*-in-tests` carve-out does not reach helper functions in an integration test file**

- **Found during:** Task 1 and Task 3 GREEN.
- **Issue:** `clippy::expect_used` and `clippy::panic` fire inside plain `fn` helpers in `tests/*.rs`, because the carve-out keys on an enclosing `#[test]` attribute or `#[cfg(test)] mod`, not on the target being a test binary. `-D warnings` made that a build failure.
- **Fix:** Rewrote the helpers to have no panic path at all — `emit_err` returns `Option<ErrorKind>`, `manifest_iamf_count` returns `Option<usize>`, and the assertions moved into the `#[test]` bodies where the carve-out does apply. Better tests: a missing manifest count now fails with a message about the manifest rather than a bare panic.
- **Committed in:** `e08b8ca` and `ac05505`.

**5. [Rule 3 — Blocking] `cargo fmt` reformatted two files this plan does not own, again**

- **Found during:** Task 1.
- **Issue:** `cargo fmt` has no path filter that respects task scope; it touched `tests/error_shape.rs` and `tests/reference_manifest.rs`, authored by plans 01-01 and 01-02. This is the identical finding plan 01-03 recorded as its deviation 6.
- **Fix:** Reverted both with `git checkout --` before staging, and switched to invoking `rustfmt --edition 2024` on the specific files this plan owns for the rest of the run. The pre-existing drift stays logged in `deferred-items.md`; CI does not run `cargo fmt --check`, so it is not a red gate today.
- **Files modified:** none (reverted).

---

**Total deviations:** 5 auto-fixed (2 Rule 1 corrections to plan-supplied numbers and instruments, 3 Rule 3 blocking). **No scope creep.** Two items beyond the plan's file list were produced: the `#[cfg(test)]` module in `src/obu/header.rs` (forced — `tests/` cannot reach `obu_size_for`, and the plan's own behaviour list requires the unaligned-after-size case) and the `pub(crate)` re-export in `src/bits/mod.rs` (forced — `leb128` is a private module and `obu_size`'s ceiling depends on `minimal_len`).

## Issues Encountered

**No new `ErrorKind` variants were needed.** All six the plan names — `ObuSizeOverflow`, `ObuTooLarge`, `TruncatedObu`, `UnalignedAfterSizeFields`, `RedundantCopyNotAllowed`, `TrimmingFlagNotAllowed` — already existed from 01-01, and the 32-byte `const` budget is untouched.

**`UnalignedAfterSizeFields` is unreachable through the public API today.** Every after-size field is a whole number of bytes, so the scratch writer is always aligned. It is checked anyway, because `GetObuSizeAndValidate` checks it and because the field set grows; it is exercised from the in-crate test module, which is the only place that can construct an unaligned scratch buffer. This is a guard, not dead code, and it is documented as such where it lives.

**The corpus walk passed on its first run, on all 39 files.** That is the desired outcome and not a sign the test is weak — the RED for it was earned (6 tests, 0 pass), and the three negative cases prove the walker can distinguish a defect from a clean file. Its value is as a standing gate over every later change to the framing layer, and from 01-08 onward over our own encoder output.

**BITS-01's requirement text is still superseded**, unchanged from 01-03. `.planning/WINDOWS.md` entry 5 remains open. This plan did not touch it, deliberately — it is a different requirement and amending it belongs to whoever owns BITS-01, not to a plan passing through.

## Known Stubs

None. `src/` and `tests/*.rs` scanned for `TODO`, `FIXME`, `unimplemented`, `placeholder`, "coming soon" and "not available" — no matches. `grep -rn 'allow(' src/` returns nothing, so D-21's escape census is still zero entries.

The RED commits `0178354`, `adef270` and `e921d6e` each passed through a deliberately incomplete state; none of those states survives into `HEAD`.

## Threat Flags

None. Every `mitigate` disposition in the plan's `<threat_model>` landed:

| Threat | Where it landed |
|---|---|
| T-01-18 `obu_size`-driven offset arithmetic | `find_obu_boundaries` uses `checked_add`/`checked_sub` at every step (6 occurrences), allocates one entry per already-validated OBU and never from an `obu_size` value; a boundary past `bytes.len()` is `TruncatedObu` |
| T-01-19 `extension_header_size`-driven allocation | `read_extension_header` goes through `read_uint8_span`, which caps against `bytes_remaining()` before slicing; `an_extension_length_past_the_end_of_the_input_is_refused_without_allocating` proves it |
| T-01-20 an `obu_size` larger than the buffer on the final OBU | `a_file_truncated_mid_obu_is_an_error_not_a_short_boundary_list` — a typed error, not the short list `libiamf`'s splitter's zero return would produce |
| T-01-21 bit 6 on a non-Audio-Frame | two-variant `TypeSpecific`, `validate_header` returns `TrimmingFlagNotAllowed`, and the grep gate proves the draft-v2.0.0 names are absent from `src/` |
| T-01-22 integer overflow in `obu_size` computation | `obu_size_for` uses `checked_add` and the reference's derived `1 << 21` ceiling, both covered by in-crate vectors at the exact boundary |
| T-01-23 `trailing` echoing unparsed bytes (accepted) | unchanged — the bytes originate in the caller's own file and preserving them verbatim is OBU-07's point |
| T-01-SC package-manager installs | this plan added no crates; `cargo tree -e normal,no-proc-macro` is still exactly `iamf` + `thiserror` |

## User Setup Required

None. Everything in this plan runs offline with `cargo test`, with `IAMF_REF_DECODER` unset and no reference binary present.

The two items plan 01-01 raised are still open and still the user's call: the work remains on `gsd/phase-01-conformant-lpcm-bitstream` rather than `main`, and `.gsd/` plus `.planning/milestone.lock` are still untracked.

## Next Phase Readiness

Ready for plan **01-05** (the descriptor OBUs):

- `write_obu_with` / `read_obu_with` are the wrapper every descriptor type plugs into; `trailing` is drained for them, once, centrally.
- **D-05's precedence rule is now stated in `src/obu/mod.rs`** where 01-05's `Reserved.raw` will land: the payload-level remainder consumes to the end of its own payload and leaves `trailing` empty. That was the wiring constraint D-05 flagged, and it is written where the next author will hit it rather than in a planning document.
- `ObuType` is the enum every descriptor names itself with; Pitfall 6's "never a bare integer" is enforced by the type.
- `find_obu_boundaries` is available as a structural pre-check on any bytes 01-05 produces, before a single field is inspected.

**Carried concern:** the hand-decoded table in `01-RESEARCH.md` covers `test_000003.iamf`'s descriptors field by field, and 01-05 should use it exactly as this plan used the header rows — expected bytes written by hand *before* the code that produces them. There is no oracle for descriptor payloads: `bitstream-io` has no opinion about them, `find_obu_boundaries` only proves the framing, and the reference binaries are a second derivation, not a first. D-25's two-independent-derivations rule is the only thing standing between a plausible descriptor and a wrong one.

---
*Phase: 01-conformant-lpcm-bitstream*
*Completed: 2026-09-08*

## Self-Check: PASSED

All 9 claimed files exist on disk. All 7 claimed commits (`0178354`, `e08b8ca`, `adef270`, `b2a0d6e`, `e921d6e`, `ac05505`, `7c4b6ab`) exist in git. `actuals.commits: 7` is measured with `git rev-list --count 3800448..HEAD`, not narrated. Frontmatter is `status: complete` with 9 requirement IDs and 10 coverage entries.
