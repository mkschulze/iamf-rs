# Phase 2: Parser, Round-Trip and Fuzzing - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 adds the sequence-level parser over the Phase 1 OBU readers, threads descriptor context
explicitly into Parameter Block parsing, proves model and byte round trips at their promised
strengths, preserves unknown OBUs and parameter data at their wire positions, semantically checks
the committed reference corpus, and ships the two required fuzz targets in an independent
workspace with stable cross-target corpus replay.

This phase does **not** add incremental/streaming input, decode audio, interpret FLAC or Opus
decoder-config payloads, add a public Parallax exporter API, or add new IAMF-v2 syntax. Phase 3 owns
FLAC/Opus framing, and Phase 4 owns the shaped public encoder API.

</domain>

<decisions>
## Implementation Decisions

No additional selections were supplied during the interactive area prompt, so the undecided items
below use the recommended defaults under Claude's discretion. Phase 1 decisions remain binding.

### Parser contract and failure behavior

- The Phase 2 entry point is a one-shot, whole-input parser over `&[u8]` that returns an owned
  sequence model. Incremental input, resumable parsing, and an iterator that yields a parsed prefix
  are deferred; they would be a distinct streaming capability.
- Structural failures are transactional: truncated fields, impossible lengths, a missing governing
  parameter definition, or an OBU payload that cannot be decoded return one located `Error` and no
  partial public sequence. The error uses `Location::InputOffset`; malformed input never panics.
- Semantic invalidity remains separate from syntax. Reserved values, duplicate IDs, inconsistent
  derived fields, and unresolved descriptor references are preserved when structurally readable and
  reported by the existing explicit `validate() -> Vec<Finding>` path. Parsing never silently
  normalizes them.
- The sequence parser builds the registry from descriptor OBUs in the same input, but Parameter
  Block decoding receives an explicit `&ParamDefinitionRegistry`. The dependency must be visible in
  the lower-level function signature; it may not hide inside a stateful parser object.
- Registry lookup is not an output-order source. Duplicate parameter IDs bind to the first
  definition in bitstream order, matching the existing `by_id()` rule, while validation reports all
  duplicates. A Parameter Block with no governing definition is a located structural error because
  its payload shape cannot be known safely.
- The parsed sequence must represent every sequence the Phase 1 writer can emit, including an empty
  file, a descriptors-only sequence, temporal units without a delimiter, and redundant descriptor
  copies. It must not synthesize a mandatory header or delimiter that was absent on the wire.
- Temporal grouping is wire-first. Where omitted delimiters make two caller-side `TemporalUnit`
  groupings observationally identical, the parsed form uses the canonical grouping implied by the
  OBU stream rather than inventing a boundary. Property generators must generate canonical,
  wire-distinguishable sequence models; they must not weaken equality with a handwritten comparator.

### Wire fidelity and unknown data

- Phase 1's roles remain unchanged: the reader preserves, `validate()` diagnoses, and the writer
  faithfully serializes what the model holds.
- Known OBU payload trailing bytes stay in the single central `Obu<T>::trailing` drain. Unknown OBU
  types retain header flags, payload bytes, and their exact semantic insertion position. Preserving
  an unknown type but moving it to the end is a failure.
- Unknown parameter definitions and parameter data retain their type value and raw bytes. Raw data
  is accepted only when a governing definition supplies a structurally bounded payload; it is not a
  fallback for corrupted known syntax.
- `parse(serialize(model)) == model` is structural equality through derived `PartialEq`, not a
  semantic or normalized comparator.
- `serialize(parse(bytes)) == bytes` is mandatory for bytes produced by this crate. For foreign
  input, non-minimal ULEB128 widths may canonicalize; tests and docs must state that caveat rather
  than claiming universal foreign-byte identity. Observed widths are not silently presented as a
  stronger guarantee than ROADMAP criterion 1 makes.

### What “reference fixture understood” means

- Every committed positive `.iamf` fixture is inventoried and has an explicit expected disposition;
  a loop that merely asserts `parse(...).is_ok()` is insufficient.
- For Phase 2-owned syntax, tests assert typed OBU counts/order, IDs, cross-references, layouts,
  parameter definitions/data, temporal structure, trailing bytes, and validation findings against
  committed expectations. The purpose is to prove the parser understood the file, not only that it
  found its end.
- FLAC, Opus, and AAC decoder-config payloads remain `DecoderConfig::Raw` until their owning phase.
  Phase 2 still asserts codec ID, raw length or digest, position, and byte preservation, but does not
  parse codec-specific fields early or describe opaque codec bytes as semantically decoded.
- The deliberately invalid fixture under `tests/fixtures/reference/negative/` is a rejection or
  finding test with its expected cause named. It is never counted as a positive corpus parse.
- Fixture provenance, the 65,536-byte per-file cap, and pinned-SHA manifest rules from Phase 1 stay
  in force. New fixtures must add unique syntax coverage and a manifest expectation; corpus volume
  alone is not evidence.

### Fuzzing and permanent regressions

- Use the already-selected `cargo-fuzz`/`libfuzzer-sys` stack. `fuzz/` is an independent workspace
  with its own lockfile and `deny.toml`; NCSA is allowed only there and never enters the root graph.
- Ship exactly the required `parse_sequence` hostile-byte target and `obu_roundtrip`
  structure-aware target. An additional fuzzer or custom mutator is deferred until coverage data
  proves a need.
- Seed `parse_sequence` from the real, pinned positive corpus. Seed `obu_roundtrip` with canonical
  models spanning every Phase 2 sequence/unknown-placement shape rather than arbitrary invalid
  structs that the writer correctly refuses.
- A discovered crash, panic, hang, excessive allocation, or round-trip mismatch is minimized before
  commit. Commit the minimized reproducer; retain the original as a CI artifact when useful. Add a
  focused named regression test whenever the root cause maps to a specific invariant, so the fix is
  legible without rerunning libFuzzer.
- Stable CI replays every committed seed and minimized artifact on all four Phase 1 target paths.
  Coverage-guided fuzzing runs separately on Linux/nightly with bounded time; it is not placed in
  every PR's four-target critical path.

### Resource ceilings

- Keep Phase 1's 2 MiB whole-OBU ceiling, 128-byte string ceiling, ULEB128 caps, and checked offset
  arithmetic. Every count is bounded against remaining input before allocation; no
  `Vec::with_capacity(bitstream_count)` is allowed.
- Do not invent a smaller whole-file limit. The caller already owns the input slice, and the parser
  may allocate only linearly with successfully bounded input. The fuzz targets enforce no panic,
  unbounded allocation, or non-termination for arbitrary bytes.

### Claude's Discretion

- Exact public type and method names for the owned sequence, registry, unknown-OBU insertion model,
  and serialization convenience methods.
- Whether the lookup-only registry uses a `BTreeMap` or an ordered `Vec` plus linear lookup, provided
  it never becomes a bitstream-order source and duplicate binding remains first-in-wire-order.
- The fixture expectation file format and the nightly fuzz workflow layout.
- Property-test strategy, case counts, and shrink configuration, provided all required shapes are
  covered and the stable suite stays practical on the four target paths.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets

- `src/bits/reader.rs`: bounded `BitCursor<'a>`, located errors, remaining-length checks, and bounded
  sub-readers form the sequence parser's input primitive.
- `src/obu/mod.rs`: `read_obu_with_header` is the one central payload-boundary and trailing-byte
  drain; Phase 2 dispatch should reuse it rather than create a second drain site.
- `src/obu/header.rs`: preserves `ObuType::Reserved`, header flags, extension bytes, trimming, and
  the observed payload boundary needed for unknown OBU handling.
- Existing `read_*` functions cover every Phase 1 descriptor and temporal payload. Phase 2 composes
  them; it does not rewrite them into a parser framework.
- `DescriptorSet`, `TemporalUnit`, `Obu<T>`, `DecoderConfig::Raw`, `ParameterData::Raw`, and the
  `Finding`/`Error` surfaces already carry most of the round-trip representation.
- The 39 committed `.iamf` reference fixtures, golden fixture, conformance harness, boundary walker,
  and pinned manifest provide parser seeds and independent expectations.

### Established Patterns

- Read/write pairs remain adjacent, hand-written, and mechanically cited to pinned reference code.
- Bitstream-visible collections use `Vec` order; `HashMap`/`HashSet` remain forbidden, and
  `BTreeMap` is allowed only for lookup state that is never emitted.
- Readers are structurally permissive and writers are faithful; semantic enforcement stays in
  explicit validation.
- All lengths and arithmetic are checked before allocation or cursor movement. Error locations are
  attached once and the public error stays within its 32-byte build-time budget.
- Four-target stable CI, strict Clippy, cargo-deny, dependency-graph checks, and the no-DSP guard
  remain inherited gates.

### Integration Points

- `src/sequence.rs` supplies the writer-side ordering and `TemporalUnit` shape the new parser must
  round-trip without changing output bytes.
- Descriptor readers populate the parameter-definition registry consumed by
  `read_parameter_block` in `src/obu/parameter_block.rs`.
- `src/model/mod.rs` supplies bitstream-order lookup and validation semantics; sequence parsing must
  not bypass them with a second model.
- `tests/fixtures/reference/` and its `MANIFEST.md` are the authoritative foreign corpus boundary.
- `.github/workflows/ci.yml` already contains the four executable target paths that stable corpus
  regression must join without introducing nightly fuzz tooling into the normal matrix.

</code_context>

<specifics>
## Specific Ideas

- “Understood rather than merely tolerated” means named field-level expectations for modeled
  syntax, plus explicit raw-byte expectations for codec payloads intentionally owned by Phase 3.
- An unknown OBU's position is part of its data. A correct payload at the wrong insertion point is a
  failed round trip.
- Round-trip properties are supplementary evidence. Hand-computed vectors, committed foreign
  expectations, and the two reference implementations remain the independent correctness oracles.

</specifics>

<deferred>
## Deferred Ideas

- Incremental/resumable parsing or a streaming `Iterator<Item = Result<...>>` API — revisit only if
  the future in-DAW playback path requires partial-input resumption.
- Semantic FLAC and Opus decoder-config parsing — Phase 3.
- A third ffmpeg read oracle — useful independent lineage, but not part of the Phase 2 requirement
  set unless separately promoted to the roadmap.
- Structure-aware custom fuzz mutators and additional fuzz engines — consider after baseline
  coverage plateaus.
- IAMF-v2 Metadata and newer parameter-data syntax — outside the pinned IAMF v1.1.0 scope.

</deferred>

---

*Phase: 02-parser-round-trip-and-fuzzing*
*Context gathered: 2026-09-09*
