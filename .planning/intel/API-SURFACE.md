# API Surface

> The automated `api-map.json` extraction has no entries. This Phase 4 supplement is a manually
> reviewed public-contract record; absence from the extractor remains "unknown", not "does not
> exist".

## Phase 4 export surface

| Public item | Contract |
|---|---|
| `encoder::EncoderBuilder` | Ordered static declaration builder. `build()` validates the complete configuration and returns `(Encoder, IdManifest)`. |
| Caller-local handle types | `CodecConfigHandle`, `AudioElementHandle`, `MixPresentationHandle`, `SubstreamHandle`, and `ParameterHandle` are opaque references owned by the caller. |
| `IdManifest` | Deterministic caller-handle-to-wire-ID mapping, plus ordered substream/parameter lookup and the sequence profile. |
| `Encoder` | Immutable validated descriptors. `start(W: Write)` writes their standalone IA Sequence prologue. |
| `EncodingWriter<W>` | Append-only temporal-unit writer. `push_temporal_unit` preflights a whole unit; `finish(self)` returns the flushed sink. |
| `FrameInput` | `Lpcm`, `Flac`, or `Opus` payload bytes. FLAC and Opus are externally encoded access units. |
| `TemporalUnitInput` | Frozen-order frames, externally supplied parameter blocks, and one shared trimming plan. |
| `SubmittedParameterBlock` | A caller-local `ParameterHandle` paired with already-decimated IAMF parameter data. |
| `Error` / `ErrorKind` / `Location` | Stable typed error surface; `ErrorKind` is non-exhaustive and location is carried once. |

## Static build and manifest

`EncoderBuilder::build()` is the static gate: it resolves and validates handles, descriptors,
codec configuration/frame-plan shape, parameter definitions, layouts, profile limits, wire-ID
capacity, and exact descriptor serializability. Its output is immutable. IDs are assigned
deterministically in declaration order, and `IdManifest` is the sole bridge from caller-local
handles to IAMF wire IDs.

The high-level scope admits single-layer channel elements and Ambisonics-mono scene elements,
ordered Mix Presentations, and shared elements/substreams where their codec and channel plan agree.
The sequence profile is the highest Presentation-local minimum, including known expanded-layout
counts; unrelated Presentations are never unioned for profile selection.

## Streaming and data ownership

`start` writes descriptors to a plain `W: Write`. `push_temporal_unit` accepts only one complete
transactional unit: every declared substream must appear once in frozen order; frame kind must match
the frozen LPCM/FLAC/Opus configuration; LPCM bytes must match the sample plan; trimming and
parameter blocks must validate before any unit bytes are emitted. `finish(self)` consumes the
writer. A partial sink write is reported and poisons subsequent operations.

The caller supplies loudness values, pre-encoded FLAC/Opus access units, IAMF-format LPCM bytes, and
pre-decimated IAMF mix/demixing/recon-gain parameter blocks. This crate neither encodes codecs nor
measures loudness, resamples, interpolates a timeline, handles source positions, or renders audio.

## Exact error boundary

`build()` can report the direct declaration kinds `UnknownCodecConfigHandle`,
`UnknownAudioElementHandle`, `UnknownSubstreamHandle`, `UnknownParameterHandle`,
`InvalidDescriptorReference`, `DuplicateDeclaration`, and `WireIdAllocationExhausted`, alongside
descriptor validation kinds including `UnsupportedLayout` and `ProfileNotFound`.

Temporal submission can report `UnknownTemporalSubstreamHandle`, `UnknownTemporalParameterHandle`,
`MissingTemporalSubstream`, `DuplicateTemporalSubstream`, `TemporalSubstreamOrderMismatch`,
`FrameCodecMismatch`, `LpcmFrameByteAlignment`, `LpcmFrameSampleCountMismatch`,
`TemporalUnitTrimMismatch`, `ParameterIdMismatch`, and `SubblockDurationMismatch`, as well as the
underlying parameter-writer validation kind. Sink failure is `SinkWrite`; a writer after a partial
sink failure reports `SequenceWriterPoisoned`.

## Adapter boundary and public-example audit

The production adapter belongs to Parallax. It owns filtering an immutable delivery snapshot,
host IDs and delivery UI state, panning/HOA preparation, codec encoding, loudness measurement,
parameter scheduling, preview rendering, and carrier/container work. `iamf-rs` has no Parallax,
codec, renderer, decoder, or DSP dependency.

The sole public Rust snippet is in `README.md` and refers only to
`iamf::encoder::EncoderBuilder`; it carries no Parallax import/name, source-position input,
preview/include field, codec-encoder dependency, or renderer dependency. This prose names ownership
explicitly, but it is not an API snippet.

## Feature/dependency proof

`cargo build --locked --no-default-features` retains the public bitstream and encoder surface.
`cargo tree --locked -e normal,no-proc-macro --prefix none` must list only `iamf` and `thiserror`;
the latter's proc-macro implementation is excluded from the linked production graph.
