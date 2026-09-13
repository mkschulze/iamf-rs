<!-- refreshed: 2026-09-13 -->
# Architecture

**Analysis Date:** 2026-09-13

## System Overview

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         Public entry surface                         │
├───────────────────────┬────────────────────────┬─────────────────────┤
│  Encoder (authoring)  │  Sequence parse/write  │  Diagnostics        │
│  `src/encoder.rs`     │  `src/sequence.rs`     │  `src/dump.rs`      │
│  `src/packing.rs`     │                        │  `src/fuzzing.rs`   │
└──────────┬────────────┴───────────┬────────────┴──────────┬──────────┘
           │                        │                       │
           ▼                        ▼                       ▼
┌──────────────────────────────────────────────────────────────────────┐
│        Descriptor model (DescriptorSet, profile, layout, loudness)   │
│        `src/model/`                                                  │
└──────────────────────────────────┬───────────────────────────────────┘
                                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│   OBU layer: header + one read_/write_ pair per OBU type             │
│   `src/obu/` (single `trailing` drain in `src/obu/mod.rs`)           │
└──────────────────────────────────┬───────────────────────────────────┘
                                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│   Bit I/O: BitCursor / BitWriter / uleb128 (only place bits exist)   │
│   `src/bits/`                                                        │
└──────────────────────────────────┬───────────────────────────────────┘
                                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│   Error surface: Error { kind, at: Location }, Finding               │
│   `src/error.rs`  (used by every layer)                              │
└──────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Crate root | `SPEC_VERSION = "1.1.0"`, module declarations, re-export of error types | `src/lib.rs` |
| Bit cursor | Bounded, offset-carrying bit/byte reads; `sub_reader` for OBU payload bounds | `src/bits/reader.rs` |
| Bit writer | Mirror of the cursor, method-for-method in the same order | `src/bits/writer.rs` |
| uleb128 | Minimal encoder (public), `pub(crate)` fixed-size encoder, 8-byte/`u32` capped decoder | `src/bits/leb128.rs` |
| OBU header | `ObuType`, `ObuHeader`, `Trimming`, `obu_size` computation and 2 MiB ceiling | `src/obu/header.rs` |
| OBU framing | Generic `Obu<T>` + `read_obu_with[_header]` / `write_obu_with[_header]` | `src/obu/mod.rs` |
| Per-OBU codecs | Sequence header, codec config (LPCM/FLAC/Opus/AAC decoder configs), audio element, mix presentation, param definition, parameter block, audio frame, temporal delimiter | `src/obu/*.rs` |
| Boundary scan | Structural OBU offset walk without payload parsing | `src/obu/boundaries.rs` |
| Descriptor model | `DescriptorSet` (Vec in bitstream order + `by_id`), `validate() -> Vec<Finding>`, `write_descriptors` | `src/model/mod.rs` |
| Profile selection | `Profile`, `select_minimum_profile` | `src/model/profile.rs` |
| Layouts / loudness | Loudspeaker/ambisonics layouts; `Q7_8`, `lufs_to_q7_8` | `src/model/layout.rs`, `src/model/loudness.rs` |
| Sequence | `parse_sequence`, `ParsedSequence`, streaming `SequenceWriter`, `write_sequence`, `write_parsed_sequence` | `src/sequence.rs` |
| Encoder | `EncoderBuilder` handles -> `build()` -> `(Encoder, IdManifest)` -> `EncodingWriter` | `src/encoder.rs` |
| Channel packing | `SubstreamPlan::for_layout`, `pack_channels_to_substreams` | `src/packing.rs` |
| Dump | `dump_annotated(bytes) -> String` human-readable annotated OBU dump (golden `.dump.txt`) | `src/dump.rs` |
| Fuzz generator | `sequence_from_fuzz_bytes` bounded `arbitrary` model generator (feature `fuzzing`) | `src/fuzzing.rs` |

## Pattern Overview

**Overall:** Layered, hand-written recursive-descent bitstream codec with symmetric read/write pairs mirroring the C++ reference (`iamf-tools` v2.1.0 / `libiamf` v1.1.0, pinned in `REFERENCES.md`).

**Key Characteristics:**
- Every `fn read_*` / `fn write_*` sits adjacent in the same file (read first) and carries a `// ref: <repo>@<tag> <file> <Symbol>` citation; `tests/citations.rs` enforces it.
- Models are plain owned structs with `pub` fields; ordered collections are `Vec` in wire order. No `HashMap`/`HashSet`/float (enforced via `clippy.toml` disallowed types/methods).
- Unknown/unparsed bytes are preserved (`Obu::trailing`, `UnknownObu`) so parse -> write is byte-identical.
- Zero linked dependencies besides `thiserror`; no `unsafe` (`forbid`); no DSP (no resampling, gain, mixing).
- Offline, streaming-for-memory design: `SequenceWriter` holds no per-unit state.

## Layers

**Bits (`src/bits/`):**
- Purpose: The only module that knows bit offsets; produces `Error` with `Location::InputOffset`/`OutputOffset` natively.
- Contains: `BitCursor<'a>` over `&[u8]`, `BitWriter` over `Vec<u8>`, uleb128.
- Depends on: `src/error.rs`. Used by: `src/obu/`, `src/model/mod.rs`, `src/sequence.rs`, `src/dump.rs`.
- `bitstream-io` is a dev-dependency oracle only (`tests/bits_oracle.rs`); never import it in `src/`.

**OBU (`src/obu/`):**
- Purpose: Serialise/parse individual OBUs. Submodules are private; the public API is flat re-exports from `src/obu/mod.rs`.
- Depends on: bits, error, `model::layout`. Used by: model, sequence, encoder, dump, fuzzing.
- Context-dependent parsing uses explicit context types: `ParamDefinitionRegistry`, `ParameterDataContext` (`src/obu/param_definition.rs`).

**Model (`src/model/`):**
- Purpose: Whole-descriptor-set semantics — cross-OBU validation, id lookup (`Identified` trait, `by_id`), profile, layout, loudness.

**Sequence (`src/sequence.rs`):**
- Purpose: IA Sequence = descriptor prologue + temporal units. Parse to `ParsedSequence { Vec<SequenceObu> }`; write via state machine.

**Encoder (`src/encoder.rs`, `src/packing.rs`):**
- Purpose: High-level authoring API for Parallax; assigns wire ids after validation, selects minimum profile, drives `SequenceWriter`.

## Data Flow

### Parse path (`.iamf` bytes -> model)

1. `parse_sequence(input: &[u8])` (`src/sequence.rs:475`) creates a `BitCursor`.
2. For each OBU, `read_obu_header` peeks type; dispatches to `read_obu_with` / `read_obu_with_header` with the type's `read_*` fn (`src/obu/mod.rs`).
3. `read_obu_with_header` bounds the payload with `r.sub_reader(payload_len)`, runs the parser, drains leftovers into `trailing` (the single drain site, OBU-07).
4. Descriptors populate a `ParamDefinitionRegistry`, used to parse subsequent `ParameterBlock`s; unknown/reserved types become `UnknownObu`; blocks without a definition become `UngovernedParameterBlock`.
5. Result is `ParsedSequence`; `ParsedSequence::validate()` (`src/sequence.rs:195`) returns `Vec<Finding>` rather than failing.

### Write path (model -> bytes)

1. `write_parsed_sequence(sink, &ParsedSequence)` (`src/sequence.rs:594`) or `SequenceWriter::push_descriptors` (`:781`) -> `push_temporal_unit` (`:816`) -> `finish` (`:875`).
2. Each OBU payload is written into a scratch `BitWriter`, `trailing` appended, then `write_obu` (`src/obu/header.rs:370`) emits header with minimal uleb128 `obu_size`.
3. Bytes flushed to the `std::io::Write` sink per temporal unit.

### Encoder authoring flow

1. `EncoderBuilder::new()` (`src/encoder.rs:530`); `add_codec_config`, `add_audio_element[_with_substreams]`, `add_substream`, `add_mix_gain_parameter`, `add_ambisonics_mono`, `add_mix_presentation[_with_parameters]` return generation-tagged handles.
2. `build()` (`src/encoder.rs:688`) validates, assigns ids deterministically, selects profile -> `(Encoder, IdManifest)`.
3. `Encoder::start(sink)` (`:158`) -> `EncodingWriter::push_temporal_unit(TemporalUnitInput)` (`:224`) -> `finish()` (`:234`).
4. `src/packing.rs` maps a `LoudspeakerLayout` to coupled/mono substreams for callers packing PCM channels.

**State Management:**
- No global mutable state except `NEXT_BUILDER_GENERATION: AtomicU64` in `src/encoder.rs` (handle provenance, not output-affecting).
- Writer state lives in `SequenceWriter` (state machine + scratch buffer + param definitions).

## Key Abstractions

**`Obu<T>`:** header + payload + `trailing` (`src/obu/mod.rs`). Generic wrapper for every OBU type.
**`BitCursor` / `BitWriter`:** mirrored primitives (`src/bits/reader.rs`, `src/bits/writer.rs`).
**`DescriptorSet`:** ordered codec configs, audio elements, mix presentations + sequence header (`src/model/mod.rs:73`).
**`SequenceObu` / `ParsedSequence` / `TemporalUnit`:** sequence-level model (`src/sequence.rs`).
**`Error { kind: ErrorKind, at: Location }` / `Finding`:** single error type, `#[non_exhaustive]`, size-budgeted (`src/error.rs`).
**Handles + `IdManifest`:** caller-local references resolved to wire ids (`src/encoder.rs`).

## Entry Points

**Library:** `src/lib.rs` — consumed by Parallax as a path dependency (contract tested in `tests/parallax_contract.rs`, `tests/public_api.rs`).
**Fuzz targets:** `fuzz/fuzz_targets/parse_sequence.rs` (hostile input, must not panic), `fuzz/fuzz_targets/obu_roundtrip.rs` (generate -> write -> parse -> equal -> rewrite byte-identical). Separate workspace.
**Codec fixture generator:** `tools/codec-fixtures/` (separate workspace; FLAC/Opus fixture tests).
**Scripts:** `tools/build-reference.sh`, `tools/prove-guards.sh`, `tools/check-codec-dependency-boundary.sh`, `tools/check-codec-dev-deps.sh`, `tools/red-evidence.sh`, `tools/cargo-test-tap.sh`.
**CI:** `.github/workflows/ci.yml` (4-target matrix, clippy, tests, golden, dep graph, cargo-deny), `fuzz.yml`, `reference.yml` (libiamf/iamf-tools).

## Architectural Constraints

- **Threading:** Single-threaded, synchronous, offline. No async, no real-time constraints.
- **Global state:** Only `NEXT_BUILDER_GENERATION` in `src/encoder.rs`.
- **Circular imports:** `src/model/mod.rs` imports `crate::obu` types while `src/obu/*` imports `crate::model::layout`; acceptable intra-crate cycle, keep `model::layout` free of `obu` imports.
- **Lints:** `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`, `panic`, `disallowed_types`, `disallowed_methods` are `deny` in `Cargo.toml`; test carve-outs in `clippy.toml`.
- **Dependency graph:** `cargo tree -e normal,no-proc-macro` must list only `iamf` and `thiserror` (CI-asserted). Codec crates live only in `tools/codec-fixtures/`.
- **Spec version:** `SPEC_VERSION` is the only version literal in `src/`.

## Anti-Patterns

### Per-type trailing-byte draining
**What happens:** A type-specific parser consumes "the rest" itself.
**Why it's wrong:** Creates a second drain site; under-reads become silent and round-trip identity breaks.
**Do this instead:** Use `read_obu_with_header` in `src/obu/mod.rs` and let it fill `trailing`.

### Wrapping a foreign bit-stream crate in `src/`
**What happens:** Importing `bitstream-io` into `src/bits/`.
**Why it's wrong:** Loses native `Location` offsets and reintroduces `io::Error` mapping.
**Do this instead:** Extend `BitCursor`/`BitWriter` and cross-check in `tests/bits_oracle.rs`.

### Signal processing in the writer
**What happens:** Resampling/padding/gain in `src/sequence.rs` or `src/encoder.rs`.
**Why it's wrong:** Crate is bytes-only; mismatches are typed errors.
**Do this instead:** Return an `ErrorKind` variant.

## Error Handling

**Strategy:** Fallible functions return `crate::Result<T>`; errors carry `Location` exactly once. Validation collects `Vec<Finding>`.

**Patterns:**
- `Error::new(ErrorKind::X, Location::InputOffset(pos))`, with `checked_*` arithmetic and `.ok_or_else(...)`.
- No `#[from]` conversions; convert at call site to keep the offset.

## Cross-Cutting Concerns

**Logging:** None (library emits no logs). Diagnostics via `src/dump.rs`.
**Validation:** `DescriptorSet::validate`, `ParsedSequence::validate`, encoder `build()`.
**Authentication:** Not applicable.

---

*Architecture analysis: 2026-09-13*
