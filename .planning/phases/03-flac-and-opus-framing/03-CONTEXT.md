# Phase 3: FLAC and Opus Framing - Context

**Gathered:** 2026-09-09
**Status:** Awaiting design approval before planning

<domain>
## Phase Boundary

Phase 3 adds typed IAMF v1.1.0 FLAC and Opus decoder configurations, frames already encoded FLAC
and Opus packets in Audio Frame OBUs, and proves both paths through the Phase 1 conformance machinery.
FLAC is checked losslessly; Opus is checked against an independent decode of the exact same committed
packets. The phase also derives Opus roll distance and priming trim, rejects codec-incompatible sample
rates with a typed error, and keeps every codec implementation out of the shipping dependency graph.

This phase does **not** add runtime audio compression or decoding, resampling, quality controls,
bitrate selection, AAC-LC, scalable layers, container muxing, or the final Parallax-facing builder.
Phase 4 owns the public exporter surface and feature gating; this phase supplies its codec-framing
primitives and evidence.

</domain>

<decisions>
## Implementation Decisions

No explicit selection was returned by the interactive prompt, so the unresolved item below uses the
recommended default under Claude's discretion. All Phase 1 and Phase 2 decisions remain binding.

### Codec boundary and dependency policy

- The shipping library accepts **already encoded access-unit bytes** as Audio Frame payloads. It does
  not encode PCM to FLAC or Opus and does not decode either codec.
- Codec libraries or command-line tools may be used only to create fixtures and independent expected
  decode output in tests. Any Rust codec crate must remain a dev-dependency and must first pass the
  repository's licence policy in an isolated pre-check.
- The normal linked graph remains unchanged: no FLAC, Opus, resampler, DSP, C FFI, or platform codec
  library may appear in `cargo tree -e normal,no-proc-macro`.
- Research selects exact dev tools and versions. `claxon` is the required first FLAC candidate and
  must be proven compatible with Rust 2024/MSRV constraints; the Opus candidate must be portable and
  licence-clean rather than assumed from popularity.

### Typed decoder configurations

- `DecoderConfig` gains public, non-exhaustive typed `Flac` and `Opus` variants beside `Lpcm` and
  `Raw`. AAC-LC remains `Raw`; unknown codec IDs retain the existing validation behavior.
- Fresh constructors derive codec ID and `audio_roll_distance`; callers cannot supply a contradictory
  derived value through the encoder path. Parsed foreign files preserve their wire value and report a
  mismatch through `validate()` rather than being normalized.
- Known fixed-size codec syntax is parsed into named fields. Extra bounded bytes remain preserved in
  the existing trailing-byte mechanism, and structurally short known configs remain reproducible as
  raw data rather than being reinterpreted or silently padded.
- FLAC STREAMINFO fields are written by the existing hand-rolled bit writer. Channel count is pinned
  to one, minimum and maximum frame sizes are zero, MD5 is all zero, and bits-per-sample is stored as
  value minus one, exactly as CODEC-01 requires.
- Opus configuration uses the required 11-byte IAMF OpusHead layout: version/channel fields are
  explicit, output channels are two, output gain is zero, mapping family is zero, and output sample
  rate is 48 kHz.

### Rate, roll, and trim behavior

- Unsupported codec sample rates fail through the published typed `Error` surface. The library never
  resamples, rounds, substitutes 48 kHz silently, or defers the failure until a reference tool runs.
- Opus `audio_roll_distance` is derived with checked integer arithmetic as
  `-ceil(3840 / num_samples_per_frame)`. Parsed values remain stored and independently validated.
- Opus priming produces a non-zero `trim_at_start`; end padding remains `trim_at_end`. A focused vector
  makes the two values different so the existing END-before-START wire ordering cannot pass by
  symmetry.
- Exact accepted sample-rate and pre-skip constraints are research inputs from the pinned IAMF v1.1.0
  references, not guesses from a generic Opus API.

### Conformance oracle

- FLAC uses deterministic non-silent committed input and must decode through `libiamf` sample-for-
  sample identical to the original PCM.
- Opus is lossy, so comparing its decode to the original PCM is not a meaningful bit-identity test.
  The selected oracle compares `libiamf` output sample-for-sample with an **independent standalone
  decode of the exact same committed Opus packets**. This isolates IAMF framing, priming, trimming,
  packet order, and channel order without inventing an error tolerance.
- The same conformance assertion path retains file-existence, decoded-frame-count, sample-rate,
  channel-count, strict-parser, and limiter-drift checks. Opus does not get a structure-only shortcut.
  Codec-specific fixture preparation may supply the encoded access units and expected decoded PCM;
  the gate semantics remain shared.
- Approximate comparison against source PCM and acceptance-only testing are rejected: both mix codec
  behavior with framing or miss cleanly decoded-but-wrong payload order.

### Fixture and regression policy

- FLAC and Opus packet fixtures are small, deterministic, committed, provenance-labelled, and replayed
  offline. Tests never require a developer machine to have a codec library or encoder installed.
- Hand-derived decoder-config byte vectors land before constructors/readers. Reference-generated bytes
  are a second oracle, preserving Phase 1's D-25 discipline.
- Round-trip tests cover typed own output and foreign/raw preservation. Focused negative tests cover
  unsupported rates, malformed fixed-size configs, incorrect roll distance, zero Opus priming, and
  swapped trim fields.
- Existing strict Clippy, cargo-deny, no-DSP, MSRV, parser, fuzz, and four-target gates remain green.

### Claude's Discretion

- Exact Rust type and constructor names, provided the variants and invariants above are public and
  non-exhaustive where future IAMF revisions may add values.
- Exact fixture duration and frame size, provided they force non-zero Opus start trim, exercise final
  padding where applicable, remain small, and have deterministic provenance.
- Exact dev-only Opus implementation/tool after the licence, MSRV, cross-platform, and cargo-deny
  pre-checks are recorded.
- Whether independent expected decodes are generated by a dev-dependency test helper or by a one-time
  fixture-generation tool, provided normal/offline tests consume only committed artifacts.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets

- `src/obu/codec_config.rs`: already defines codec IDs, `DecoderConfig::{Lpcm, Raw}`, the faithful
  read/write pair, stored `audio_roll_distance`, validation findings, and explicit Phase 3 extension
  points.
- `src/obu/header.rs`: already writes trim-at-end before trim-at-start and preserves both values, so
  Opus priming needs no second header representation.
- `src/obu/audio_frame.rs` and `src/sequence.rs`: already treat frame bodies as opaque bytes and can
  carry pre-encoded packets without linking a codec implementation.
- `tests/conformance.rs`: already checks output existence, frame count, channels, rate, deterministic
  double decode, limiter behavior, strict-parser acceptance, and exact PCM comparison.
- `tests/support/fixture.rs`: supplies deterministic PCM, descriptors, packing plans, and fixture
  encoding; Phase 3 can extend fixture preparation without creating a parallel conformance system.
- Phase 2 parser properties and reference expectations already preserve FLAC/Opus configs as raw
  bytes, providing migration and byte-fidelity regressions when typed variants are introduced.

### Established Patterns

- Encoder constructors derive fields; readers preserve wire values; `validate()` diagnoses semantic
  mismatches without rewriting foreign input.
- Known readers consume only their named syntax and preserve bounded remainder bytes centrally.
- Every read/write pair is adjacent, hand-written, and mechanically cited to pinned reference code.
- Bitstream-visible order stays in `Vec`; all lengths and arithmetic are checked; malformed input
  cannot panic.
- Reference executables are test-time `Command` oracles, never `build.rs` or shipping dependencies.

### Integration Points

- Extend `DecoderConfig`, `CodecConfig` constructors/accessors, validation, and roll derivation in
  `src/obu/codec_config.rs`; update the dumper and public re-exports for the new typed variants.
- Extend fixture description/encoding so Audio Frame payloads may be supplied as pre-encoded access
  units while LPCM behavior remains unchanged.
- Reuse `assert_conformant`'s shared assertions with codec-specific expected PCM rather than forked
  weaker gates.
- Add dev-only dependency and licence assertions to `Cargo.toml`, `Cargo.lock`, `deny.toml`, and CI
  graph guards without altering the normal linked graph.

</code_context>

<specifics>
## Specific Ideas

- For Opus, “sample-identical” means both decoders produce the same PCM from the same lossy packets;
  it does not mean Opus reproduces its pre-encode source samples.
- A non-zero Opus start trim is an independent detector for reversed trim-field order.
- Codec crates are fixture oracles, not part of the product architecture.

</specifics>

<deferred>
## Deferred Ideas

- Runtime FLAC/Opus encoding and decoding, including bitrate, complexity, and quality controls.
- Resampling or automatic sample-rate conversion.
- AAC-LC and IAMF-v2 codec syntax.
- Final public codec selection and feature gates for Parallax — Phase 4.
- Container muxing and streaming/incremental media input.

</deferred>

---

*Phase: 03-flac-and-opus-framing*
*Context gathered: 2026-09-09*
