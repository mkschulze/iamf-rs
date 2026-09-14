# Quick 260914-hoa: Per-parse memory budget for `parse_sequence` - Research

**Researched:** 2026-09-14 (repo HEAD `f24a095`)
**Domain:** parse-side memory amplification (`src/sequence.rs` `parse_sequence` -> `ParsedSequence`)
**Confidence:** HIGH for the measurements (counting allocator plus `/usr/bin/time -l` on this machine, pinned-source reads). MEDIUM for the recommendation, which is a design judgement on delegated authority.

<user_constraints>
## User Constraints (from orchestrator; no CONTEXT.md)

- The user delegated every design decision to research ("Claude decides after research").
- `parse_sequence` keeps its behaviour for Parallax. Any new API is additive and non-breaking. Existing `ErrorKind` variants may not change. Adding one is fine, within `size_of::<Error>() <= 32`.
- Carried over from 260914-1mq: satisfy both the spec and the pinned references, and keep whichever rule is stricter. `semantic_sha256` may change only for fixtures marked `is_valid: false`. Read pinned trees only through `git show`. Never `git add -A`.
- Toolchain 1.85.0, no let-chains. Clippy denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap`/`expect`/`panic`, and `HashMap`/`f32`/`f64` in `src/`. `unsafe_code = "forbid"` applies to tests as well (`Cargo.toml:74`).
- Size: a quick task, 1-3 plan tasks.
</user_constraints>

## Project Constraints (from CLAUDE.md)

- Linked graph stays `iamf` + `thiserror`. No `HashMap`, no floats, no `BTreeMap` for bitstream order.
- Every `fn read_*`/`fn write_*` needs a `// ref:` line (`tests/citations.rs`). The recommended API adds no `read_*`/`write_*` fn, but cite anyway.
- Never silently normalise. Parse errors carry `Location::InputOffset`. No `#[from]`.
- Golden bytes and reference hashes must not change. Nothing in this task touches bytes or models.
- Licence hygiene: `iamf-tools`, `libiamf` and `eclipsa-audio-plugin` may be read. gpac, libspatialaudio and FFmpeg may not.

## Summary

**The dominant amplification is not a payload class. It is the flat `Vec<SequenceObu>` itself.** `size_of::<SequenceObu>()` is **192 B** because the largest inline variant, `Obu<AudioElement>`, is 192 B. The smallest legal OBU is **2 wire bytes**: a Temporal Delimiter `20 00`, an empty Audio Frame `30 00`, or a reserved type `c0 00`. None of those allocate on the heap, so each costs exactly 192 B, or **96× live**. I measured 32 MiB of `20 00` at **3.27 GB max RSS** (97×), and 3.36-3.66 GB for the other 2-byte classes. The 2 MiB OBU cap does nothing here, because the count of OBUs is bounded only by `input.len() / 2`. The earlier CONCERNS residual list does not include this class. Every in-payload class is smaller: `read_strings` 42× live / ~50× with malloc rounding, extension params 34×, Mix Gain subblocks 17×, substream ids 4×. Real committed files measure **1.2-4.0×**. Tiny files measure up to 17×, and low-bitrate Opus (`noise_3s_stereo_opus`) 11.7×.

**Recommendation: a streaming `SequenceReader` iterator (additive), with `parse_sequence` re-implemented as a `collect` over it (behaviour-identical), plus a measured amplification contract in rustdoc and HANDOFF. Defer a whole-file `ParseLimits` budget.** This matches how the pinned `iamf-tools` decoder and eclipsa avoid the problem. Neither has a memory budget. Both stream and hold only descriptors plus the current temporal unit, and `iamf-tools` caps its stream buffer at 2 × 2 MiB. A per-OBU iterator changes the memory bound from linear in file size to "descriptor registry plus one OBU", where one OBU is at most ~55× its 2 MiB `obu_size`, about 110 MB. It needs no new `ErrorKind` and no platform-dependent accounting, and it fits in one code task plus one docs task. An exact whole-file byte budget would need platform-independent unit accounting at ~20 allocation sites. That is not quick-task sized, and a caller holding a streaming reader can budget for itself.

**Primary recommendation:** add `pub struct SequenceReader<'a>` (`Iterator<Item = Result<SequenceObu>>` + `FusedIterator`) in `src/sequence.rs`. Make `parse_sequence` `SequenceReader::new(input).collect()`. Prove laziness with a prefix-before-error test and equivalence with a fixture/corpus differential test.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| OBU framing and per-OBU parse | `iamf` library (`src/sequence.rs`, `src/obu/`) | — | Wire grammar is owned only by `iamf-rs` (portfolio rule 3) |
| Memory policy (how much to hold, when to stop) | Consumer (`iamf-decode-rs`, `iamf-isobmff-rs`, Parallax import) | `iamf` documents factors | The library provides a streaming primitive and measured factors; the consumer owns resource limits, as with eclipsa and the iamf-tools API |
| Temporal-unit grouping | Consumer / existing `ParsedSequence::temporal_unit_ranges` | — | Grouping needs whole-file lookahead (see Pitfall 2), so it is not streamed here |

## Q1 - Measurements at HEAD `f24a095`

**Method.** Scratch crate `scratchpad/memprobe/` (path dependency on the repo, release build, rust 1.85.0) with a counting `GlobalAlloc`. It records `live` = heap bytes still held by the returned `ParsedSequence`, and `peak` = the modelled worst peak during the parse, counting the new and old blocks both live across a `realloc`. `rounded` applies macOS `malloc_good_size`. RSS is `/usr/bin/time -l` maximum resident set size and includes the input buffer itself (~32 MB). Inputs are 32 MiB. Payload classes use 2,000,000-byte OBUs repeated. [VERIFIED: probe run this session]

`size_of` on aarch64-apple-darwin (the same layout applies on all four 64-bit targets, but that is [ASSUMED], not probed on the others): `SequenceObu` 192, `Obu<AudioElement>` 192, `Obu<MixPresentation>` 168, `Obu<CodecConfig>` 152, `Obu<ParameterBlock>` 104, `Obu<AudioFrame>` 96, `Obu<IaSequenceHeader>` 96, `Obu<TemporalDelimiter>` 64, `UnknownObu` 64, `UngovernedParameterBlock` 64, `ObuHeader` 40, `SubMixAudioElement` 120, `LayoutWithLoudness` 72, `RegisteredParamDefinition` 72, `AudioElementParam` 56, `ParamDefinition` 48, `ParameterSubblock` 40, `Vec<u8>` 24.

| Class (hostile input) | Wire bytes / element | Model bytes / element | live ×in | peak ×in (rounded) | Max RSS @32 MiB |
|---|---|---|---|---|---|
| **Temporal Delimiter `20 00`** | 2 | 192 (enum, no heap) | **96.0** | 144 | **3.27 GB** |
| **Empty Audio Frame `30 00`** | 2 | 192 | **96.0** | 144 | **3.36 GB** |
| **Reserved OBU type `c0 00`** | 2 | 192 | **96.0** | 144 | **3.66 GB** |
| Audio Frame 1-byte payload `30 01 00` | 3 | 192 + 1 (malloc 16) | 96.3 | 148 | 2.41 GB |
| TD with 1 trailing byte `20 01 00` | 3 | 192 + 1 (16) | 96.3 | 148 | 2.64 GB |
| Ungoverned Parameter Block `18 01 00` | 3 | 192 + 1 (16) | 96.3 | 148 | 2.41 GB |
| TD with 1-byte extension header `21 02 01 00` | 4 | 192 + 1 (16) | 48.2 | 74 | 2.14 GB |
| Governed Mix Gain block, 1 Step subblock (8 B OBU) | 8 | 192 + 40 (48) | 29.0 | 39 | 1.38 GB |
| Redundant small Mix Presentation (12 B, registers 1 def) | 12 | 192 + SubMix heap + registry 72 | 32.7 | 52 | 1.33 GB |
| `read_strings` empty NUL strings (MP `count_label`) | 1 | 24 + 8 (malloc 16) | **41.5** | 50 | 1.04 GB |
| AE extension params `03 00` | 2 | 56 | 33.5 | 35 | 1.06 GB |
| AE Recon Gain params (4 B, + registry clone) | 4 | 56 + 72 transient | 16.8 | 38 | 1.45 GB |
| Sub-mix elements (8 B, + registry clone) | 8 | 120 + 72 transient | 16.8 | 27 | 0.96 GB |
| Layouts with loudness | 6 | 72 | 16.8 | 17 | 0.52 GB |
| Mix Gain Step subblocks in one 2 MiB block | 3 | 40 | 16.8 | 17 | 0.54 GB |
| AE substream ids | 1 | 4 (u32) | 4.2 | 4 | 0.16 GB |
| Large Audio Frames (baseline) | 2 MiB | 1× copy | 1.0 | 1.0 | 0.07 GB |
| **All 40 committed `.iamf` fixtures** | — | — | **2.45 aggregate** (1.21-4.02 for files ≥2 KB; 11-17× for files ≤225 B; `noise_3s_stereo_opus` 11.66) | — | — |

Notes:
- `live` is capacity. At 2^24 + 1 OBUs the `Vec` capacity doubles to 6.44 GB (192× allocated), but RSS stayed 3.36 GB, because untouched pages are not resident. An allocator whose `realloc` copies could transiently touch roughly 192× [ASSUMED, not probed on Windows].
- A **single 2 MiB OBU** peaks at: `read_strings` 55.5× (111 MB), AE extension params 50× (101 MB), AE Recon Gain params 45×, sub-mix elements 31×, Mix Gain 26×. That is the per-item ceiling for a streaming reader.
- Registry residual: `ParamDefinitionRegistry::register` pushes duplicates (`src/obu/param_definition.rs:89-94` [VERIFIED: read]). Redundant descriptor copies therefore grow the parse registry linearly, even in a streaming reader: ~72 B per 12 wire bytes, plus growth.
- CPU note (not memory): `registry.get` is a linear `find` (`param_definition.rs:177-181`), so many registered definitions times many blocks is O(n·m). Out of scope.

**Which classes are bounded only per OBU:** every in-payload class is linear and capped per OBU at 2 MiB. The generic per-OBU class is capped by nothing except input length.

## Q2 - Options and recommendation

| Option | Bounds the 96× class? | Bounds in-payload classes? | Deterministic across targets? | API impact | Quick-sized? |
|---|---|---|---|---|---|
| (a) `ParseLimits` byte budget on `ParsedSequence` | Yes | Yes, if every allocation site is charged | Only if charged in a declared unit table, not `size_of` | Additive fn + new `ErrorKind::ParseBudgetExceeded` | **No**: ~20 charge sites across `obu/*`, easy to miss one (a guard that sees nothing) |
| (b) Count limits (`max_obus`, …) | `max_obus` yes | No (strings, params, subblocks stay 17-55× per OBU) | Yes | Additive | Yes, but arbitrary numbers with no reference basis |
| (c) Compaction (box `SequenceObu` variants to 16 B) | Reduces 96× to ~40× | No | Yes | **Breaking**: public tuple-variant field types change | Yes, but breaking |
| **(d′) Streaming `SequenceReader` + contract doc** | **Yes**: nothing accumulates unless the caller keeps it | Per OBU ≤ ~55× of ≤2 MiB | Yes: no size-based behaviour | **Additive only** | **Yes** |
| (e) Document factor only | No | No | — | None | Yes, but leaves the whole-file 96× unaddressed |

**Chosen: (d′) = streaming reader + (e) documentation.** Rationale:

1. **The reference practice is streaming, not a budget.** `iamf-tools@848c6ff4` `iamf/api/decoder/iamf_decoder.h` exposes `Decode(input_buffer, size)`, `IsTemporalUnitAvailable()` and `GetOutputTemporalUnit()`. `iamf_decoder.cc` `DecodeOneTemporalUnit` processes one temporal unit and then calls `read_bit_buffer->Flush(num_bits_read / 8)`. `StreamBasedReadBitBuffer` caps its source at `max_source_size_bits_ = kEntireObuSizeMaxTwoMegabytes * 2 * 8`, i.e. 4 MiB (`iamf/common/read_bit_buffer.cc:509-512`), and `PushBytes` returns `InvalidArgumentError("Cannot push more bytes than the available space in the source.")`. [VERIFIED: pinned source read] `eclipsa-audio-plugin@c964609` `common/processors/file_output/iamf_export_utils/IAMFFileReader.cpp` feeds `Decode` in `kBufferSize = 4096` chunks (`:33-38`) and pulls units via `IsTemporalUnitAvailable` (`:133-141`). Its `indexFile` (`:209`) walks headers only, which is our `find_obu_boundaries`. [VERIFIED: grep of the checkout at c964609; details CITED from coordinator] `libiamf@f06e919e` exposes streaming `IAMF_decoder_decode(handle, data, size, …)` (`code/include/IAMF_decoder.h:82`). A grep for memory or count limits in `code/src/iamf_dec` finds only codec frame sizes, `MAX_STREAMS 255` and `IAMF_MIX_PRESENTATION_MAX_SUBS 1`, and no read budget. [VERIFIED: pinned grep]
2. **It closes the dominant class without touching the model.** Memory becomes registry + one `SequenceObu` + whatever the caller retains.
3. **Determinism is free.** No accept/reject decision depends on `size_of`.
4. **Behaviour preservation is provable.** `parse_sequence` becomes `collect::<Result<Vec<_>>>()` over the same loop body. Errors, offsets, `Vec` growth (size_hint lower bound 0) and the "no public prefix on failure" doc (`src/sequence.rs:513`) are all unchanged.
5. **No new `ErrorKind`.** The Parallax error contract is untouched.

**Why not temporal-unit streaming mirroring `SequenceWriter`:** `ParsedSequence::temporal_unit_ranges` decides `uses_delimiters` over the **whole** sequence (`src/sequence.rs:413-416` [VERIFIED: read]). A TU iterator would need lookahead or a changed rule. Per-OBU streaming is exact and TU grouping stays with the consumer. Deferred.

### API sketch (additive)

```rust
// src/sequence.rs, directly above parse_sequence
// ref: iamf-tools@v2.1.0 iamf/api/decoder/iamf_decoder.cc DecodeOneTemporalUnit
// NOTE: the reference streams and flushes per temporal unit; this reader streams per OBU
// (unit grouping needs whole-sequence lookahead, see ParsedSequence::temporal_unit_ranges).
pub struct SequenceReader<'a> {
    reader: BitCursor<'a>,
    registry: ParamDefinitionRegistry,
    finished: bool,
}

impl<'a> SequenceReader<'a> {
    #[must_use]
    pub fn new(input: &'a [u8]) -> Self;
    /// Absolute input offset of the next OBU (unchanged after an error).
    #[must_use]
    pub fn byte_position(&self) -> u64;
}

impl Iterator for SequenceReader<'_> {
    type Item = Result<SequenceObu>;
    fn next(&mut self) -> Option<Self::Item>; // body = today's loop body; on Err set finished
}
impl core::iter::FusedIterator for SequenceReader<'_> {}

pub fn parse_sequence(input: &[u8]) -> Result<ParsedSequence> {
    SequenceReader::new(input)
        .collect::<Result<Vec<_>>>()
        .map(|obus| ParsedSequence { obus })
}
```

`BitCursor::byte_position(&self) -> u64` exists (`src/bits/reader.rs:257-259` [VERIFIED: read]). `ParamDefinitionRegistry::new()` is `const` (`param_definition.rs:82-86`). The loop body moves verbatim: `src/sequence.rs:525-625`, including the double inspection clones and the `with_input_base(payload_base)` mapping.

## Q3 - Defaults

- **`parse_sequence` stays unlimited.** Behaviour is preserved, and the rustdoc gains a measured contract: "retains one 192-byte enum per OBU on 64-bit targets plus payload copies: typically 1-4× input for real files, up to ~100× RSS for hostile 2-byte OBUs; use `SequenceReader` to bound memory to one OBU (≤ ~55× its `obu_size`, ≤ ~110 MB) plus parameter-definition context."
- **No `ParseLimits` type is added**, so there is no default to choose. If a later task adds one, the reference basis for numbers is: `kEntireObuSizeMaxTwoMegabytes = (1 << 21)` (`iamf/obu/types.h:32`), the 4 MiB stream buffer above, and `AudioElementObu::kMaxNumParameters = 256` (`iamf/obu/audio_element.h:287`), enforced on read by `ValidateNumParameters` (`audio_element.cc:97-113`, called at `:811`) with the comment "To reduce the risk of allocating massive amounts of memory, we limit the number of parameters". [VERIFIED: pinned source read] This crate does **not** apply the 256 cap today (grep of `src/obu/audio_element.rs` finds none). That is a stricter-reference divergence. See Open Questions.

## Q4 - Test design (TDD)

New file **`tests/sequence_reader.rs`** (helpers return `Option`/`Result` with no `expect` outside `#[test]` bodies). RED on HEAD = a compile failure (`SequenceReader` does not exist), plus the behavioural tests below.

| Test | Input (bytes verified on HEAD via probe) | Expect |
|---|---|---|
| `reader_yields_each_obu_before_a_later_error` (the laziness proof) | `20 00 20 00 20 05` | `Ok(TemporalDelimiter)`, `Ok(TemporalDelimiter)`, `Err(UnexpectedEndOfInput @ InputOffset(6))`, then `None`, `None` (fused). `parse_sequence` on the same bytes = the same `Err` (HEAD today: `Err(UnexpectedEndOfInput, InputOffset(6))`). |
| `reader_reports_position_of_next_obu` | same | `byte_position()` is 0 before, 2 after first `next`, 4 after second, and stays 4 after the error |
| `reader_carries_definitions_across_obus` | `10 0a 01 00 01 00 05 01 80 00 00 00` + `18 06 05 01 01 00 00 00` | second item is `SequenceObu::ParameterBlock`, **not** `UngovernedParameterBlock` (HEAD `parse_sequence` gives `[MixPresentation, ParameterBlock]`) |
| `reader_equals_parse_sequence_on_every_committed_file` (differential) | every file under `tests/fixtures/**/*.iamf`, `fuzz/corpus/parse_sequence/*`, `fuzz/artifacts/parse_sequence/*` | `collect::<Result<Vec<_>>>()` equals `parse_sequence(..).map(|s| s.obus)`, comparing `Ok` models or `Err` kind+location. **Anti-vacuity asserts:** ≥ 40 `.iamf` files seen, total OBUs > 0, at least one `Err` case compared (`tests/fixtures/reference/test_000129.iamf` is `Err` on HEAD; `fuzz/corpus/parse_sequence` has 4 files, `fuzz/artifacts/parse_sequence` is empty today, so the loop must tolerate an empty dir) |
| `reader_on_empty_input_yields_nothing` | `[]` | `next()` = `None`; `parse_sequence(&[])` = `Ok` with 0 OBUs |

**Proof the guard fires:** state in the PLAN a mutation check the executor runs once and records in SUMMARY. Implement `next()` eagerly (e.g. call `parse_sequence` on the rest and return its first item) and confirm `reader_yields_each_obu_before_a_later_error` goes red. That test cannot be passed by an eager implementation, because the eager parse returns no prefix. The "budget = exact passes, needed−1 fails" test from the brief is **not applicable**, because no budget is added.

**Regression gates (unchanged expectations):** `cargo test --locked` (includes `parse_reference` semantic hashes, `golden`, `sequence_parse`, `citations`, `allocation_bounds`), `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --features fuzzing --test fuzz_regression`, `bash tools/check-float-escape-census.sh` (exactly 1), `cargo tree -e normal,no-proc-macro` (iamf + thiserror). Optional: extend `fuzz/fuzz_targets/parse_sequence.rs` with a reader-vs-`parse_sequence` differential and check it with `cargo +nightly-2026-09-01 check --locked --manifest-path fuzz/Cargo.toml --bins`.

**Memory evidence:** allocator measurement is impossible in-repo (`unsafe_code = "forbid"`). Record in SUMMARY a before/after `/usr/bin/time -l` run of the scratch probe: 32 MiB of `20 00` through `parse_sequence` (≈3.3 GB) versus through `SequenceReader` with each item dropped (expected ≈ input size only). That second number is a prediction, [ASSUMED] until measured.

## Q5 - Parallax and portfolio impact

- **API additions:** `iamf::sequence::SequenceReader` (struct, `new`, `byte_position`, `Iterator`, `FusedIterator`). No change to `parse_sequence`'s signature or behaviour, and no `ErrorKind` change.
- **Consumers today:** `rg "parse_sequence|ParsedSequence|SequenceObu" /Volumes/lab/talea/{Parallax,iamf-encode-rs,iamf-isobmff-rs,iamf-render-rs} --glob '*.rs'` returned **no hits**. `iamf-decode-rs` is not checked out on this machine. The portfolio names `iamf-rs → iamf-decode-rs` and `iamf-rs → iamf-isobmff-rs` as consumers of the "public model/OBUs" (`Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md:59-61`). [VERIFIED: read]
- **Docs to update:** `HANDOFF.md` (parse-side memory contract, and `SequenceReader` for untrusted or large input), the `README.md` "Consumer boundary" (one paragraph), `.planning/codebase/CONCERNS.md` residual (1) (add the 96× generic per-OBU class, the measured table and the resolution by streaming; keep in-payload residuals), and the `src/sequence.rs` module doc (reader section).
- **Change control:** it does **not** apply. The change is additive, moves no ownership and adds no cross-library object. Per "Change control" (portfolio doc :256-262), new capability "may extend its own local contract without changing this document". The `iamf-rs` row's maturity text only needs a touch if the hook asks after a tag.

## Common Pitfalls

1. **Losing the prefix-free contract of `parse_sequence`.** Keep `collect::<Result<Vec<_>>>()`, which short-circuits and drops the prefix. Do not return a partial `ParsedSequence`.
2. **Trying to stream temporal units.** `uses_delimiters` is whole-sequence (`sequence.rs:413-416`). Stream OBUs only.
3. **Not fusing after an error.** Continuing after `Err` would re-read from a cursor in an undefined position and could emit garbage items. Set `finished = true` on the first `Err` and implement `FusedIterator`.
4. **Offsets.** Keep `payload_base` from the inspection clone and `with_input_base`. Absolute offsets must stay identical; the differential test compares locations.
5. **Clippy in test helpers:** no `expect`/indexing outside `#[test]` bodies (hit in 1mq).
6. **Citations:** no new `read_*` fn, so `tests/citations.rs` is unaffected. Still add the `// ref:` + `NOTE:` above the struct as sketched.

## Deferred (record in deferred-items)

1. Whole-file `ParseLimits` / `parse_sequence_with_limits` with a unit-table budget and `ErrorKind::ParseBudgetExceeded`. Only if a consumer needs a hard cap on a retained whole-file model.
2. Adopt `iamf-tools` `kMaxNumParameters = 256` on Audio Element read. The spec says parsers SHALL support any value, and iamf-tools refuses above 256. Needs a stricter-rule decision plus a reference-hash check. It would cap the AE-param classes (34-50× per OBU).
3. Registry de-duplication for redundant descriptor copies (`register` pushes duplicates), which is the linear residual inside a streaming reader. `entries()` is public and documented as "including duplicates", so this needs care.
4. Compaction by boxing `SequenceObu` variants (192 → 16 B inline). This is a breaking public change.
5. A temporal-unit streaming reader mirroring `SequenceWriter`.
6. O(n·m) `registry.get` linear scan (CPU).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | `size_of::<SequenceObu>() = 192` on the other three 64-bit targets (only aarch64-apple-darwin probed) | Q1 | Low: the factor in the docs shifts slightly; no behaviour depends on it |
| A2 | A copying `realloc` (e.g. Windows) could make whole-file RSS approach ~192× transiently | Q1 notes | Low: strengthens the case for streaming |
| A3 | With `SequenceReader` and items dropped, RSS for 32 MiB of `20 00` ≈ input size | Q4 | Low: the executor measures it |
| A4 | `iamf-decode-rs` (not checked out) does not depend on eager parse internals | Q5 | Low: purely additive API |

## Open Questions

1. **Should the stricter `iamf-tools` `kMaxNumParameters = 256` read rule be adopted?** Known: the pinned source enforces it on read and write with a memory rationale, and the spec requires support for any value. Unclear: whether the "stricter wins" policy applies to a decoder resource limit. Recommendation: a separate quick task with a user decision (deferred item 2).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| Rust toolchain | build/tests | ✓ | 1.85.0 via `rust-toolchain.toml` | — |
| `/usr/bin/time -l` | RSS evidence (SUMMARY) | ✓ | macOS | counting allocator in scratch only |
| nightly-2026-09-01 + cargo-fuzz | optional fuzz differential | not checked | — | skip; fuzz target unchanged |

## Security Domain

V5 Input Validation / resource exhaustion (CWE-400, CWE-770). The threat is amplification of hostile input into model memory: 2-byte OBUs reach 96× live and ~100× RSS. The standard control is streaming consumption with a bounded per-item size (the 2 MiB OBU cap is already enforced), which is the reference decoders' approach. No auth, session or crypto categories apply.

## Sources

### Primary (HIGH)
- Repo HEAD `f24a095`: `src/sequence.rs:57-104,413-416,513-629`; `src/obu/mod.rs:112-166`; `src/obu/param_definition.rs:28-45,82-94,106-155,177-187`; `src/obu/parameter_block.rs:369-449,734-795`; `src/obu/mix_presentation.rs:590-612,667-707,752-795,835-917`; `src/obu/audio_element.rs:471-509,568-630`; `src/obu/header.rs:199-210,439-489`; `src/bits/reader.rs:60-72,167-189,257-259`; `Cargo.toml:70-74`
- `iamf-tools@848c6ff4`: `iamf/api/decoder/iamf_decoder.h` (Decode/IsTemporalUnitAvailable/GetOutputTemporalUnit); `iamf/api/decoder/iamf_decoder.cc` (`DecodeOneTemporalUnit`, Flush; `kInitialBufferSize = 1024`); `iamf/common/read_bit_buffer.cc:470-512`; `iamf/obu/types.h:32`; `iamf/obu/audio_element.h:282-287`; `iamf/obu/audio_element.cc:97-113,811`
- `libiamf@f06e919e` (`.reference/libiamf`): `code/include/IAMF_decoder.h:82`; `code/src/iamf_dec/IAMF_types.h:149-168`; `IAMF_OBU.c:763`
- `eclipsa-audio-plugin@c964609`: `IAMFFileReader.cpp:33-38,133-141,209`
- Scratch probe: `scratchpad/memprobe/` (`src/main.rs`, `results32.txt`, `fixtures.txt`)

### Secondary
- `.planning/quick/260914-1mq-*/260914-1mq-RESEARCH.md` Q3, `-deferred-items.md` item 1; `260914-5c5-deferred-items.md` item 5; `.planning/codebase/CONCERNS.md` §Allocation amplification residuals; `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md:124-131,178`

## Metadata

- Measurements: HIGH (run this session, reproducible via probe). Recommendation: MEDIUM-HIGH (delegated design; reference practice agrees). Pitfalls: HIGH (from the code read).
- Valid until: next change to `SequenceObu` layout or `parse_sequence` (about 30 days).
- Not committed by this agent: the orchestrator asked for no repo changes beyond writing this file.
