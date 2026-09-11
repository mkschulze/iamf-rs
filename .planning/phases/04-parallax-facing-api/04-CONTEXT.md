# Phase 04: Parallax-Facing API — Context

**Defined:** 2026-09-11
**Status:** Ready for planning
**Requirements:** API-01–API-11, DEC-03

## Goal

Expose a host-independent high-level export API through which Parallax can turn an already filtered,
immutable IAMF delivery snapshot into a conformant standalone IA Sequence. The API is shaped by the
Parallax use case but contains no Parallax types and does not become a second delivery model.

The existing low-level OBU model, parser and `SequenceWriter` remain public. Phase 4 adds a safer
authoring and streaming layer above them; it does not replace the exact round-trip surface needed for
foreign files.

## Repository boundary

`iamf-rs` owns:

- IAMF v1.1 wire types, parsing and serialization;
- deterministic wire-ID allocation and minimum-profile selection;
- static export validation and immutable built configuration;
- transactional temporal-unit validation and append-only IA Sequence writing; and
- framing LPCM payloads and already encoded FLAC or Opus access units.

Parallax owns:

- Sources, AGIO terminals, lineage and Audio Element bindings;
- `active_preview_mix_presentation` and `included_in_iamf_export` session state;
- filtering the export snapshot and rejecting included invalid drafts;
- panning, HOA encoding, loudness measurement, codec encoding, resampling and export orchestration;
- persistent Parallax IDs and the adapter from those IDs to caller-local `iamf-rs` handles; and
- carrier/container work outside the standalone IA Sequence.

Consequently `iamf-rs` must not depend on a Parallax crate. The production adapter belongs in
Parallax. Phase 4 proves compatibility using fixtures shaped like Parallax's filtered delivery input.

## Configuration and identity

The builder accepts ordered declarations through opaque caller-local handles. It assigns all wire IDs
deterministically and returns a manifest mapping every accepted handle to its wire ID. That manifest
is the only bridge needed for diagnostics and later parameter/frame submission; a Parallax `Id` type
never crosses the repository boundary.

Mix Presentations retain caller order. Codec Configs and Audio Elements follow their normative wire
ordering. A shared Audio Element or substream is declared once and may be referenced by several Mix
Presentations without duplicated audio emission.

Profile selection is sequence-global but evaluated from Presentation-local limits: compute the
minimum profile required by each Mix Presentation, then select the highest of those results for the
IA Sequence Header. Do not count the union of unrelated Presentations as if they played together.
Known expanded loudspeaker layouts need explicit channel counts for this calculation; reserved
layouts remain a typed error.

## Supported high-level authoring scope

The generic low-level model continues to round-trip every currently modelled valid shape. The initial
high-level convenience API required by Parallax covers:

- single-layer channel-based Audio Elements using supported standard or expanded layouts;
- scene-based Audio Elements using Ambisonics mono mode;
- multiple ordered Mix Presentations and multiple valid sub-mixes at the generic IAMF layer;
- shared Audio Elements across Presentations;
- rendering configuration, element mix gain, output mix gain, labels, layouts and caller-supplied
  loudness; and
- LPCM plus externally encoded FLAC and Opus frames.

Parallax initially authors exactly one sub-mix per Mix Presentation. That is a caller policy rather
than an `iamf-rs` format restriction. High-level scalable-layer authoring, demixing/recon-gain
construction and Ambisonics projection are deferred; their existing low-level wire types remain
available for parsing and exact round trips.

## Build and streaming lifecycle

`EncoderBuilder::build()` is the single static-configuration gate. It validates descriptors,
cross-references, codec/frame plans, layouts, loudness shapes, parameter definitions, profile limits
and ID capacity, then returns an immutable encoder and ID manifest.

That does not make later operations infallible. Future input and I/O cannot be known at build time:

1. starting the encoder writes the frozen descriptors to a plain `W: Write` sink;
2. each submitted temporal unit is fully preflighted before any of its bytes are emitted;
3. temporal validation checks substream coverage, codec/frame identity, sample count, trim agreement,
   parameter identity/type/rate/duration and exact block tiling;
4. a validation failure emits none of that temporal unit;
5. a partial sink failure poisons the writer observably; and
6. `finish(self)` consumes the writer and returns the sink or a typed sink error.

Static build errors, temporal-input errors and sink failures remain distinguishable through a stable,
`#[non_exhaustive]` public error surface.

## Timing and codec boundary

DEC-03 means callers submit already decimated IAMF parameter blocks. It does not transfer a timeline
or interpolation policy into this crate. In Parallax's IAMF-v1.1 scope, Source position drives the
upstream channel panner or HOA encoder and is therefore present in the resulting PCM. The parameter
blocks reaching `iamf-rs` describe supported IAMF mix, demixing or recon-gain parameters—not Source
position—and do not share an ADM position-decimation contract.

Likewise Phase 3 added codec configuration and packet framing, not codec encoders. Phase 4 accepts
LPCM payloads or pre-encoded FLAC/Opus access units. It verifies that the submitted data matches the
frozen codec and frame plan. It never invokes a codec, resamples audio or derives payloads from PCM.

## Parallax contract fixtures

Public-API-only fixtures must cover:

1. an ordinary Stereo Audio Element and Presentation;
2. a Presentation combining a channel-based Bed and Ambisonics-mono scene element;
3. multiple ordered Presentations sharing one Audio Element without duplicate descriptor or frame
   emission;
4. element and output gains, labels, loudness and pre-decimated parameter blocks;
5. LPCM, FLAC and Opus framing through the same high-level lifecycle; and
6. a Parallax-shaped source fixture in which excluded/draft delivery state is filtered before the
   builder, proving the API needs no Include or Preview field.

Each produced file runs through the existing structural, deterministic and external-conformance gates
applicable to its codec. The fixture is a consumer-contract oracle, not a dependency on Parallax.

## Deferred

- a Parallax crate dependency or production adapter inside `iamf-rs`;
- Preview rendering or `iamf-render-rs` integration;
- codec encoding, resampling, DSP or loudness measurement;
- Source-position or AGIO concepts;
- high-level scalable-channel, demixing/recon-gain and Ambisonics-projection authoring;
- ISO-BMFF and other carriers; and
- native decoding.
