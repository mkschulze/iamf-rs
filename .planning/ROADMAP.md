# Roadmap: iamf-rs

## Overview

Four phases, one per milestone M1–M4, in an order that dependency, verification and detection-latency
all force to be the same order. Phase 1 discovers and pins every byte-level fact, installs every
guardrail before there is code to retrofit, and starts the two external-tooling tracks (Bazel fixture
generation, pinned `libiamf` CI job) on day one — it is far larger than "produce one LPCM file", and it
exits only through a seven-clause conformance gate, not through "`libiamf` returned OK". Phase 2 adds
the parser, the round-trip property and the fuzzer, all of which would inherit Phase 1's
misunderstandings silently if written earlier. Phase 3 reuses Phase 1's conformance harness unchanged
for FLAC and Opus framing. Phase 4 shapes the surface Parallax actually calls, and cannot start until
the parameter tick-rate decision is answered. ISO-BMFF, a native decoder and scalable layers are v2 and
deliberately absent below.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Conformant LPCM Bitstream** - A standalone `.iamf` the reference decodes sample-identically, with every guardrail and every byte-level fact pinned
- [ ] **Phase 2: Parser, Round-Trip and Fuzzing** - Read the bitstream back into the model, prove both round-trip directions, and ship a fuzzer with a committed corpus
- [ ] **Phase 3: FLAC and Opus Framing** - Two more codecs through the unchanged conformance harness, with zero codec crates in the shipping graph
- [ ] **Phase 4: Parallax-Facing API** - One validated `build()`, minimum-profile selection, and a dependency-free bitstream core

## Phase Details

### Phase 1: Conformant LPCM Bitstream

**Goal**: A standalone `.iamf` file this crate writes is accepted by both reference implementations, is byte-explicable against reference output, and is reproducible byte-for-byte on every target
**Mode:** mvp
**Depends on**: Nothing (first phase)
**Requirements**: BITS-01, BITS-02, BITS-03, BITS-04, BITS-05, BITS-06, BITS-07, OBU-01, OBU-02, OBU-03, OBU-04, OBU-05, OBU-06, OBU-07, OBU-08, DESC-01, DESC-02, DESC-03, DESC-04, DESC-05, DESC-06, DESC-07, DESC-08, DESC-09, TIME-01, TIME-02, TIME-03, TIME-04, TIME-05, SEQ-01, SEQ-02, SEQ-03, PROF-01, PROF-02, PROF-03, CONF-01, CONF-02, CONF-03, CONF-04, CONF-05, CONF-06, CONF-07, CONF-08, CONF-09, CONF-10, CONF-11, GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-05, GUARD-06, GUARD-07, GUARD-08, GUARD-09, GUARD-10, GUARD-11, GUARD-12, GUARD-13, DEC-01, DEC-02, DEC-04, DEC-05
**Success Criteria** (what must be TRUE):

  1. The crate reproduces the shipped golden `libiamf/tests/test_000003.iamf` byte-for-byte from an equivalent configuration, and `find_obu_boundaries()` walked over our own output lands its final boundary exactly on `bytes.len()`. *(CONF-08, OBU-08)*
  2. `libiamf` at the pinned SHA decodes our fixture, reports a decoded sample count equal to what was encoded, and returns PCM sample-identical to the input — where that fixture is non-silent, per-channel-distinguishable, has a total sample count that is **not** a multiple of the frame size (so `trim_at_end > 0`), and contains at least six OBUs including two Codec Configs. *(CONF-02, CONF-03, CONF-04, CONF-05)*
  3. `iamf-tools`' own stricter parser at the pinned v1.x tag accepts the same file, and a byte-diff against an `iamf-tools`-produced file of identical configuration is either empty or has every differing byte enumerated in writing in the repository — "mostly the same" is not a passing result. *(CONF-06, CONF-07)*
  4. The same fixture hashes identically on macOS arm64, macOS x86_64, Windows MSVC and Linux x64 against a committed golden hash, and encoding twice inside one process produces identical bytes. *(GUARD-09, GUARD-10)*
  5. `cargo deny check licenses` fails when an LGPL crate is deliberately added; `cargo clippy` fails on a `HashMap`, on an `unwrap()`/`expect()` outside tests, on a raw index and on unchecked arithmetic; the no-DSP guard fails on an introduced transcendental — while `cargo test` stays green offline on all four targets with no reference binary present. *(GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-11, CONF-10)*

**Research**: Yes — read `libiamf`'s `codec_config_obu.c` and `audio_frame_obu.c` for payload-level rejection rules before the LPCM path is written; fetch `iamf-tools` tags `v2.0.0`/`v2.1.0` to check whether either is a v1.1.0-exact tree; read AOM Patent License 1.0 §1.2
**Decisions settled here**: DEC-04 (`iamf-tools` tag) and DEC-05 (payload rejection rules), both before the type model is written. DEC-01 and DEC-02 were **answered by the user on 2026-09-08** and are now implementation, not investigation: pin `SPEC_VERSION = "1.1.0"`, and carry `MIT OR Apache-2.0` into `Cargo.toml` plus the `NOTICE` file (the licence files themselves already landed)
**Plans**: 2/8 plans executed

Plans (8 plans, 8 waves — strictly sequential; each layer genuinely needs the one below, and D-24
forces 01-01 → 01-02 before any bitstream code):

- [x] 01-01-PLAN.md — Repo scaffolding and every guardrail, each **proven to bite**: `deny.toml` allow-list-only, `clippy.toml` `disallowed-types`/`disallowed-methods`, hardening lints, `rust-toolchain.toml`, `SPEC_VERSION`, `REFERENCES.md` pinned SHAs, `NOTICE`, `PATENTS`, `CONTRIBUTING.md` checkbox, the D-08 error surface, four-target CI matrix
- [x] 01-02-PLAN.md — External-tooling track on day one: `tools/build-reference.sh` builds `libiamf`+`iamfdec` at the pinned SHA and **self-validates against a file we did not write**, `.reference-manifest.json` asserted at runtime, a vendored reference fixture corpus with a D-14 manifest (no Bazel needed — research correction 6), the digest-pinned `iamf-tools` container, and two recorded experiments
- [ ] 01-03-PLAN.md — `bits` layer: hand-rolled `BitCursor`/`BitWriter` (D-01), hand-written uleb128 both directions with the 8-byte and `u32` caps, byte-alignment assertions, BITS-06 hand-computed vectors, the proptest differential oracle, and D-23's mechanical `// ref:` citation test
- [ ] 01-04-PLAN.md — OBU header and common structure: MSB-first byte 0, the two-pass `obu_size` origin, the **two-variant** bit-6 enum (research correction 1 — OBU-03 is amended here), `obu_redundant_copy` legality, END-before-START trimming, extension header, central `trailing` drain, `find_obu_boundaries()` proven over the vendored corpus offline
- [ ] 01-05-PLAN.md — Descriptor OBUs: the full 120-byte prologue of `test_000003.iamf` reproduced byte-exact — IA Sequence Header with `additional >= primary`, Codec Config with derived `audio_roll_distance` and big-endian `sample_format_flags`, Audio Element with D-04's `AudioElementType`, Mix Presentation with two layouts and the mandatory Mix Gain param definitions, the four-type layout model, `Vec`-in-bitstream-order collections
- [ ] 01-06-PLAN.md — Time-varying OBUs and LPCM framing: Audio Frame with implicit substream IDs, BCG coupled-pairs-then-mono packing proven three independent ways, final-frame trimming, Temporal Delimiter, Parameter Block with an explicit `ParamDefinition` argument
- [ ] 01-07-PLAN.md — Sequence writer, profile and metadata: the streaming `push_descriptors`/`push_temporal_unit`/`finish` primitive reproducing all 32567 bytes of `test_000003.iamf`, the whole-file wrapper, the profile enum, minimum-profile selection, and the `round_ties_even` Q7.8 helper carrying the single documented float escape
- [ ] 01-08-PLAN.md — `assert_conformant(config, pcm)` and the seven-clause exit gate: the **gating 24-bit probe first** (research assumption A1), the D-18/D-19 fixture, D-20's three golden artifacts, D-12's executable diff ledger, both reference oracles, and the always-on offline layer

### Phase 2: Parser, Round-Trip and Fuzzing

**Goal**: A bitstream can be read back into the model and re-emitted unchanged, foreign files are understood rather than merely tolerated, and the parser is continuously fuzzed
**Mode:** mvp
**Depends on**: Phase 1 (the parser mirrors the writer, so it must not be written until the writer is verified — a parser that mirrors a misunderstanding passes its own round-trips forever, and an accumulated fuzz corpus that explored a grammar that does not exist is worthless)
**Requirements**: PARSE-01, PARSE-02, PARSE-03, PARSE-04, PARSE-05, PARSE-06, PARSE-07, FUZZ-01, FUZZ-02, FUZZ-03, FUZZ-04, FUZZ-05
**Success Criteria** (what must be TRUE):

  1. `parse(serialize(model)) == model` holds for every model the encoder can construct, and `serialize(parse(bytes)) == bytes` holds byte-for-byte for every file this crate produced — with the non-minimal-leb128 divergence on foreign files documented rather than silently normalised. *(PARSE-03, PARSE-04)*
  2. Every `.iamf` generated from `iamf-tools` textprotos parses without error and each descriptor is asserted against an expected value, not merely accepted. *(PARSE-07)*
  3. A file containing an unknown OBU type and unknown parameter data re-serialises with those bytes verbatim at their original byte offsets. *(PARSE-05, PARSE-06)*
  4. Parsing a Parameter Block without a `ParamDefinitionRegistry` does not compile — the registry is an argument, never hidden parser state. *(PARSE-01, PARSE-02)*
  5. `cargo fuzz run parse_sequence` and `cargo fuzz run obu_roundtrip` run from a committed corpus seeded with real `iamf-tools` output without a crash, from an independent `fuzz/` workspace whose dependencies never appear in the root lockfile; and the corpus-regression test replays every corpus entry on the stable toolchain on all four targets. *(FUZZ-01, FUZZ-02, FUZZ-03, FUZZ-04, FUZZ-05)*

**Research**: No — the parser mirrors the writer one-for-one and the fuzzing setup is fully specified in STACK.md §4 and ARCHITECTURE.md. Mechanical once Phase 1 is verified; skip `--research-phase`
**Note**: "Phase 1 encoder, Phase 2 parser" does not mean no reader code before Phase 2 — land the reader alongside the writer per OBU type wherever it is cheap in Phase 1. Phase 2 is about the *sequence*-level parser, the registry, the round-trip property and the fuzzer
**Plans**: 4 plans

Plans:

- [ ] 02-01: Sequence-level parser and `ParamDefinitionRegistry` as an explicit argument
- [ ] 02-02: Round-trip properties both directions, plus verbatim preservation of unknown OBU types and unknown parameter data
- [ ] 02-03: Parse the committed `iamf-tools` fixtures and assert the descriptors are understood
- [ ] 02-04: Independent `fuzz/` workspace, both targets, committed seeded corpus, and the stable-toolchain cross-target regression test

### Phase 3: FLAC and Opus Framing

**Goal**: FLAC- and Opus-framed `.iamf` files pass the Phase 1 conformance harness unchanged, without adding a single codec crate to the shipping dependency graph
**Mode:** mvp
**Depends on**: Phase 2 (the LPCM chain must be proven and round-trippable first, and the same `assert_conformant` harness is then reused unchanged)
**Requirements**: CODEC-01, CODEC-02, CODEC-03, CODEC-04, CODEC-05, CODEC-06, CODEC-07
**Success Criteria** (what must be TRUE):

  1. A FLAC-framed file passes `assert_conformant` with the harness unmodified — `libiamf` decodes it and the PCM is sample-identical, with STREAMINFO written as hand-written bit fields (channels pinned to 1, frame sizes 0, MD5 zero, `bits_per_sample` stored as value − 1). *(CODEC-01)*
  2. An Opus-framed file passes the same harness, its `decoder_config` is the 11-byte OpusHead layout, its `audio_roll_distance` equals `-ceil(3840 / num_samples_per_frame)`, and Opus priming makes `trim_at_start` non-zero — catching a swapped trim order a second, independent way. *(CODEC-02, CODEC-03, CODEC-04)*
  3. `cargo tree -e normal` on the shipping crate lists zero codec crates; codec crates appear only as dev-dependencies, and `cargo deny check licenses` is still green. *(CODEC-06, CODEC-07)*
  4. A sample rate the codec cannot carry returns `Error::SampleRateNotSupportedByCodec`, and the no-DSP guard is still green — no resampler appears in the diff. *(CODEC-05)*

**Research**: Yes — run `cargo deny check licenses` on a throwaway branch that merely *adds* the candidate codec crates before the approach is committed (the wrapper crate's own licence, whatever it vendors, and its build-dependencies are three separate questions); verify `claxon` builds on edition 2024
**Scope pressure to resist**: sample-rate mismatch is the single most likely first DSP breach. The answer is a typed error, never a resampler
**Plans**: 3 plans

Plans:

- [ ] 03-01: Throwaway-branch licence pre-check, then FLAC STREAMINFO `decoder_config` and its conformance run
- [ ] 03-02: Opus `OpusHead` `decoder_config`, derived roll distance, and the priming-driven non-zero `trim_at_start`
- [ ] 03-03: Typed sample-rate rejection and dependency hygiene — dev-dependency-only codec crates, committed Opus packet fixtures

### Phase 4: Parallax-Facing API

**Goal**: Parallax can export a conformant `.iamf` through the public surface alone, with one validation point and no way to hold the encoder wrong
**Mode:** mvp
**Depends on**: Phase 3 (all three codecs exist before the surface that selects between them is frozen)
**Entry condition**: ✓ **Satisfied 2026-09-08.** DEC-03 answered: the crate takes **pre-decimated blocks**. No time model, no interpolation, no float arithmetic on the encode path; `parameter_rate` remains a `build()`-time input. The decimation policy now lives in Parallax and must be shared with the ADM BWF exporter, or the two exports will disagree
**Requirements**: API-01, API-02, API-03, API-04, API-05, DEC-03
**Success Criteria** (what must be TRUE):

  1. `EncoderBuilder::build()` is the single place an invalid configuration is rejected — every validation error surfaces there as a typed `#[non_exhaustive]` error, and a successfully built `Encoder` cannot subsequently be misconfigured. *(API-01)*
  2. `build()` assigns IDs and selects the minimum profile the configuration fits, and the selected profile matches `profile_filter.cc`'s verdict for the same configuration. *(API-02)*
  3. An export driven entirely through the public API — loudness supplied up front at `build()`, parameter data supplied in the shape DEC-03 chose, writer append-only over a plain `W: Write` — produces a file that passes `assert_conformant`. *(API-03, API-04, DEC-03)*
  4. `cargo build --no-default-features` produces a build with no dependencies that still parses and serialises the full bitstream, and that build is what the fuzz target and the `cargo deny` baseline compile against. *(API-05)*

**Research**: No — blocked on the tick-rate *decision* rather than on missing research
**Plans**: 3 plans

Plans:

- [ ] 04-01: `EncoderBuilder`, the single `build() -> Result<Encoder>` validation point, ID assignment and minimum-profile selection
- [ ] 04-02: Loudness supplied up front, parameter data in the DEC-03 shape, append-only `W: Write` writer, and the end-to-end Parallax export path
- [ ] 04-03: Feature gating over drivers and codec libraries but never the bitstream — `--no-default-features` probe build as fuzz target and `cargo deny` baseline

## Deferred Beyond v1

Not in this roadmap. Tracked in REQUIREMENTS.md under v2.

- **ISO-BMFF encapsulation** (BMFF-01..03) — the licence contamination milestone. `gpac` is LGPL-2.1 and forbidden, and it is the obvious reference. The ISO-BMFF binding was **not researched at all**; confirm `iamf-tools` contains its own permissively-licensed muxer before committing. Needs deep research when it is scheduled.
- **Native decoder** (DECO-01..03) — a product decision, not a gap. Phase 1 needs `libiamf` present either way.
- **Scalable coding** (SCAL-01..04) — recon gain and demixing weights are DSP-adjacent; this crate serialises values it is given, and nobody supplies them yet.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Conformant LPCM Bitstream | 2/8 | In Progress|  |
| 2. Parser, Round-Trip and Fuzzing | 0/4 | Not started | - |
| 3. FLAC and Opus Framing | 0/3 | Not started | - |
| 4. Parallax-Facing API | 0/3 | Not started | - |

---
*Roadmap created: 2026-09-08*
*Granularity: coarse — 4 phases, one per milestone M1–M4*
*Coverage: 88/88 v1 requirements mapped*
