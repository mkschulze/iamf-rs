# Roadmap: iamf-rs

## Overview

Four phases, one per milestone M1–M4, in an order that dependency, verification and detection-latency
all force to be the same order. Phase 1 discovers and pins every byte-level fact, installs every
guardrail before there is code to retrofit, and starts the two external-tooling tracks (Bazel fixture
generation, pinned `libiamf` CI job) on day one — it is far larger than "produce one LPCM file", and it
exits only through a seven-clause conformance gate, not through "`libiamf` returned OK". Phase 2 adds
the parser, the round-trip property and the fuzzer, all of which would inherit Phase 1's
misunderstandings silently if written earlier. Phase 3 reuses Phase 1's conformance harness unchanged
for FLAC and Opus framing. Phase 4 shapes the host-independent surface Parallax actually calls and
proves that boundary with Parallax-shaped contract fixtures. ISO-BMFF, decoder-consumer wire additions and
high-level scalable-layer authoring are v2 and
deliberately absent below.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Conformant LPCM Bitstream** - A standalone `.iamf` the reference decodes sample-identically, with every guardrail and every byte-level fact pinned
- [x] **Phase 2: Parser, Round-Trip and Fuzzing** - Read the bitstream back into the model, prove both round-trip directions, and ship a fuzzer with a committed corpus (completed 2026-09-09)
- [x] **Phase 3: FLAC and Opus Framing** - Two more codecs through the unchanged conformance harness, with zero codec crates in the shipping graph (completed 2026-09-11)
- [x] **Phase 4: Parallax-Facing API** - A validated host-independent builder, deterministic ID/profile mapping, transactional streaming and Parallax-shaped contract fixtures

## Phase Details

### Phase 1: Conformant LPCM Bitstream

**Goal**: A standalone `.iamf` file this crate writes is accepted by both reference implementations, is byte-explicable against reference output, and is reproducible byte-for-byte on every target
**Mode:** mvp
**Depends on**: Nothing (first phase)
**Requirements**: BITS-01, BITS-02, BITS-03, BITS-04, BITS-05, BITS-06, BITS-07, OBU-01, OBU-02, OBU-03, OBU-04, OBU-05, OBU-06, OBU-07, OBU-08, DESC-01, DESC-02, DESC-03, DESC-04, DESC-05, DESC-06, DESC-07, DESC-08, DESC-09, TIME-01, TIME-02, TIME-03, TIME-04, TIME-05, SEQ-01, SEQ-02, SEQ-03, PROF-01, PROF-02, PROF-03, CONF-01, CONF-02, CONF-03, CONF-04, CONF-05, CONF-06, CONF-07, CONF-08, CONF-09, CONF-10, CONF-11, GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-05, GUARD-06, GUARD-07, GUARD-08, GUARD-09, GUARD-10, GUARD-11, GUARD-12, GUARD-13, DEC-01, DEC-02, DEC-04, DEC-05
**Success Criteria** (what must be TRUE):

  1. The crate reproduces the shipped golden `libiamf/tests/test_000003.iamf` byte-for-byte from an equivalent configuration, and `find_obu_boundaries()` walked over our own output lands its final boundary exactly on `bytes.len()`. *(CONF-08, OBU-08)*
  2. `libiamf@v1.1.0`, pinned at `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63`, decodes our sample-identity fixture, reports a decoded sample count equal to what was encoded, and returns PCM sample-identical to the input — where that fixture is non-silent, per-channel-distinguishable, has a total sample count that is **not** a multiple of the frame size (so `trim_at_end > 0`), while a separate structure fixture contains at least six OBUs and two Audio Elements, satisfying CONF-04's two Codec Configs **or** two Audio Elements requirement so repeated-descriptor ordering is observable. *(CONF-02, CONF-03, CONF-04, CONF-05)*
  3. `iamf-tools@v2.1.0`, pinned at `848c6ff4968ff8cc6f728259892ab4f90cb83256`, is the tool release whose model passes the four IAMF-v1.1.0 discriminators; its own stricter parser accepts the sample-identity file, and a byte-diff against an `iamf-tools`-produced file of identical configuration is either empty or has every differing byte enumerated in writing in the repository — "mostly the same" is not a passing result. The tool's version number does not change `SPEC_VERSION = "1.1.0"`. *(CONF-06, CONF-07)*
  4. The same fixture hashes identically on macOS arm64, macOS x86_64, Windows MSVC and Linux x64 against a committed golden hash, and encoding twice inside one process produces identical bytes. *(GUARD-09, GUARD-10)*
  5. `cargo deny check licenses` fails when an LGPL crate is deliberately added; `cargo clippy` fails on a `HashMap`, on an `unwrap()`/`expect()` outside tests, on a raw index and on unchecked arithmetic; the no-DSP guard fails on an introduced transcendental — while `cargo test` stays green offline on all four targets with no reference binary present. *(GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-11, CONF-10)*

**Research**: Yes — read `libiamf`'s `codec_config_obu.c` and `audio_frame_obu.c` for payload-level rejection rules before the LPCM path is written; fetch `iamf-tools` tags `v2.0.0`/`v2.1.0` to check whether either is a v1.1.0-exact tree; read AOM Patent License 1.0 §1.2
**Decisions settled here**: DEC-04 (`iamf-tools` tag) and DEC-05 (payload rejection rules), both before the type model is written. DEC-01 and DEC-02 were **answered by the user on 2026-09-08** and are now implementation, not investigation: pin `SPEC_VERSION = "1.1.0"`, and carry `MIT OR Apache-2.0` into `Cargo.toml` plus the `NOTICE` file (the licence files themselves already landed)
**Plans**: 10/10 plans executed

Plans (10 plans, 10 waves — the eight implementation waves are complete, followed by two
strictly ordered verification-gap closure waves):

- [x] 01-01-PLAN.md — Repo scaffolding and every guardrail, each **proven to bite**: `deny.toml` allow-list-only, `clippy.toml` `disallowed-types`/`disallowed-methods`, hardening lints, `rust-toolchain.toml`, `SPEC_VERSION`, `REFERENCES.md` pinned SHAs, `NOTICE`, `PATENTS`, `CONTRIBUTING.md` checkbox, the D-08 error surface, four-target CI matrix
- [x] 01-02-PLAN.md — External-tooling track on day one: `tools/build-reference.sh` builds `libiamf`+`iamfdec` at the pinned SHA and **self-validates against a file we did not write**, `.reference-manifest.json` asserted at runtime, a vendored reference fixture corpus with a D-14 manifest (no Bazel needed — research correction 6), the digest-pinned `iamf-tools` container, and two recorded experiments
- [x] 01-03-PLAN.md — `bits` layer: hand-rolled `BitCursor`/`BitWriter` (D-01), hand-written uleb128 both directions with the 8-byte and `u32` caps, byte-alignment assertions, BITS-06 hand-computed vectors, the proptest differential oracle, and D-23's mechanical `// ref:` citation test
- [x] 01-04-PLAN.md — OBU header and common structure: MSB-first byte 0, the two-pass `obu_size` origin, the **two-variant** bit-6 enum (research correction 1 — OBU-03 is amended here), `obu_redundant_copy` legality, END-before-START trimming, extension header, central `trailing` drain, `find_obu_boundaries()` proven over the vendored corpus offline
- [x] 01-05-PLAN.md — Descriptor OBUs: the full 120-byte prologue of `test_000003.iamf` reproduced byte-exact — IA Sequence Header with `additional >= primary`, Codec Config with derived `audio_roll_distance` and big-endian `sample_format_flags`, Audio Element with D-04's `AudioElementType`, Mix Presentation with two layouts and the mandatory Mix Gain param definitions, the four-type layout model, `Vec`-in-bitstream-order collections
- [x] 01-06-PLAN.md — Time-varying OBUs and LPCM framing: Audio Frame with implicit substream IDs, BCG coupled-pairs-then-mono packing proven three independent ways, final-frame trimming, Temporal Delimiter, Parameter Block with an explicit `ParamDefinition` argument
- [x] 01-07-PLAN.md — Sequence writer, profile and metadata: the streaming `push_descriptors`/`push_temporal_unit`/`finish` primitive reproducing all 32567 bytes of `test_000003.iamf`, the whole-file wrapper, the profile enum, minimum-profile selection, and the `round_ties_even` Q7.8 helper carrying the single documented float escape
- [x] 01-08-PLAN.md — `assert_conformant(config, pcm)` and the seven-clause exit gate: the **gating 24-bit probe first** (research assumption A1), the D-18/D-19 fixture, D-20's three golden artifacts, D-12's executable diff ledger, both reference oracles, and the always-on offline layer
- [x] 01-09-PLAN.md — Reconcile ROADMAP criteria 2/3 and BITS-01, BITS-07 and CONF-11 with the evidence-backed decisions already implemented, preserving all requirement IDs and traceability
- [x] 01-10-PLAN.md — Prove byte identity for an exact CI-tested candidate across the four target paths, with explicit publication authorization, native→Rosetta→D-17 handling for macOS x86_64, durable evidence and fresh phase verification

### Phase 2: Parser, Round-Trip and Fuzzing

**Goal**: A bitstream can be read back into the model and re-emitted unchanged, foreign files are understood rather than merely tolerated, and the parser is continuously fuzzed
**Mode:** mvp
**Depends on**: Phase 1 (the parser mirrors the writer, so it must not be written until the writer is verified — a parser that mirrors a misunderstanding passes its own round-trips forever, and an accumulated fuzz corpus that explored a grammar that does not exist is worthless)
**Requirements**: PARSE-01, PARSE-02, PARSE-03, PARSE-04, PARSE-05, PARSE-06, PARSE-07, FUZZ-01, FUZZ-02, FUZZ-03, FUZZ-04, FUZZ-05
**Success Criteria** (what must be TRUE):

  1. `parse(serialize(model)) == model` holds for every model the encoder can construct, and `serialize(parse(bytes)) == bytes` holds byte-for-byte for every file this crate produced — with the non-minimal-leb128 divergence on foreign files documented rather than silently normalised. *(PARSE-03, PARSE-04)*
  2. Every valid `.iamf` generated from `iamf-tools` textprotos parses without error and each descriptor is asserted against an expected value, not merely accepted; fixtures whose pinned metadata explicitly marks them invalid retain an exact structural-error or validation-finding disposition. *(PARSE-07)*
  3. A file containing an unknown OBU type and unknown parameter data re-serialises with those bytes verbatim at their original byte offsets. *(PARSE-05, PARSE-06)*
  4. Parsing a Parameter Block without a `ParamDefinitionRegistry` does not compile — the registry is an argument, never hidden parser state. *(PARSE-01, PARSE-02)*
  5. `cargo fuzz run parse_sequence` and `cargo fuzz run obu_roundtrip` run from a committed corpus seeded with real `iamf-tools` output without a crash, from an independent `fuzz/` workspace whose dependencies never appear in the root lockfile; and the corpus-regression test replays every corpus entry on the stable toolchain on all four targets. *(FUZZ-01, FUZZ-02, FUZZ-03, FUZZ-04, FUZZ-05)*

**Research**: No — the parser mirrors the writer one-for-one and the fuzzing setup is fully specified in STACK.md §4 and ARCHITECTURE.md. Mechanical once Phase 1 is verified; skip `--research-phase`
**Note**: "Phase 1 encoder, Phase 2 parser" does not mean no reader code before Phase 2 — land the reader alongside the writer per OBU type wherever it is cheap in Phase 1. Phase 2 is about the *sequence*-level parser, the registry, the round-trip property and the fuzzer
**Plans**: 7/7 plans executed

Plans:

- [x] 02-01: Context-rich `ParamDefinitionRegistry` and complete Phase 2 Parameter Data parsing
- [x] 02-02: Preserve and migrate every reserved field currently discarded by known readers
- [x] 02-03: Transactional sequence parser, flat-order validation, and canonical temporal grouping
- [x] 02-04: Round-trip properties in both directions with exact unknown-data position fidelity
- [x] 02-05: Independently derive and assert field-level expectations for the committed reference corpus
- [x] 02-06: Independent `fuzz/` workspace, shared bounded model generator, and exactly two fuzz targets
- [x] 02-07: Committed seeded corpora, four-target stable replay, and bounded nightly fuzzing

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
**Plans**: 8 plans

Plans:

- [x] 03-01: Isolated codec-tool preflight and typed faithful FLAC STREAMINFO configuration
- [x] 03-02: Codec-neutral fixture adapter, excluded FLAC fixture tooling, fixed-block corpus, and FLAC conformance
- [x] 03-03: Typed faithful Opus configuration, sample-rate rejection, and checked roll derivation
- [x] 03-04: Excluded Opus fixture generation with measured lookahead, exact packet count, and standalone decode oracle
- [x] 03-05: Opus packet/trim integration through the shared conformance gate
- [x] 03-06: Shipping dependency-boundary canaries and scoped codec-tool licence policy
- [x] 03-07: Offline parser/property/fuzz regression closure and immutable artifact proof
- [x] 03-08: Four-target CI and pinned reference-evidence closure

### Phase 4: Parallax-Facing API

**Goal**: Parallax can map a filtered immutable delivery snapshot into a conformant `.iamf` through a host-independent public surface alone, without `iamf-rs` depending on Parallax or taking ownership of rendering, codec encoding, timeline policy or delivery UI state
**Mode:** mvp
**Depends on**: Phase 3 (all three codec configuration/framing paths exist before the external-frame surface that selects between them is frozen)
**Entry condition**: ✓ **Satisfied and clarified 2026-09-11.** DEC-03 requires **pre-decimated IAMF parameter blocks**. `iamf-rs` owns no time model or interpolation. In the approved IAMF-v1.1 Parallax scope Source motion is already represented in upstream Bed/HOA PCM; these blocks therefore carry IAMF mix/demixing/recon-gain data rather than Source-position curves and do not share an ADM position-decimation policy
**Requirements**: API-01, API-02, API-03, API-04, API-05, API-06, API-07, API-08, API-09, API-10, API-11, DEC-03
**Success Criteria** (what must be TRUE):

  1. `EncoderBuilder::build()` rejects every static descriptor, reference, codec/frame-plan and parameter-definition error and returns an immutable encoder plus deterministic caller-handle→wire-ID manifest. Subsequent temporal-input and sink failures remain separately typed rather than being falsely promised away at build time. *(API-01, API-02, API-08)*
  2. The sequence profile is the highest minimum required by any individual ordered Mix Presentation, including known expanded layouts; it is not computed from the unrelated-Presentation element union and matches the pinned `profile_filter.cc` verdict. *(API-03)*
  3. The generic surface can describe multiple Presentations and shared channel-/scene-based elements, while its Parallax convenience path initially admits single-layer channel elements and Ambisonics-mono scene elements. No Preview, Export-Include, terminal, lineage or Parallax ID enters the crate. *(API-04, API-05, API-10)*
  4. A public-API-only streaming export accepts LPCM or externally encoded FLAC/Opus frames, validates each complete temporal unit transactionally, consumes pre-decimated IAMF parameter blocks, and remains append-only over plain `W: Write`. It performs no codec encoding, DSP, resampling, loudness measurement or timeline interpolation. *(API-06, API-07, API-08, DEC-03)*
  5. Contract fixtures cover Stereo, mixed channel+HOA, multiple Presentations sharing an element, gains/loudness/parameters and all three framing paths through the existing deterministic and external conformance gates. *(API-09)*
  6. `cargo build --no-default-features` retains the complete production bitstream/builder surface with no linked codec or integration dependency; the documented proc-macro-only `thiserror` graph remains the sole qualification to “dependency-free”. *(API-11)*

**Research**: No — Phase 3 and the 2026-09-11 Parallax integration-contract audit settle the ownership, codec, timing and Presentation questions
**Plans**: 4/4 plans executed

Plans:

- [x] 04-01: Host-independent `EncoderBuilder`, immutable built configuration, deterministic handle→wire-ID manifest and per-Presentation global minimum-profile selection including known expanded layouts
- [x] 04-02: Typed LPCM/external-FLAC/external-Opus frame submission, pre-decimated parameter blocks, transactional temporal-unit validation and append-only `W: Write` lifecycle
- [x] 04-03: Multiple-Presentation/shared-element authoring surface plus Parallax-shaped public-contract fixtures for Stereo, mixed channel+HOA, gains, loudness and filtered export selection
- [x] 04-04: Minimal production feature/dependency surface, consumer handoff and synchronization of Project/README/API documentation without introducing a Parallax dependency

## Deferred Beyond v1

Not in this roadmap. Tracked in REQUIREMENTS.md under v2.

- **ISO-BMFF encapsulation** (BMFF-01..03) — the licence contamination milestone. `gpac` is LGPL-2.1 and forbidden, and it is the obvious reference. The ISO-BMFF binding was **not researched at all**; confirm `iamf-tools` contains its own permissively-licensed muxer before committing. Needs deep research when it is scheduled.
- **Decoder-consumer wire additions** (DECO-01..03) — native codec decoding and timed reconstruction
  belong to `iamf-decode-rs`; add syntax/model support here only when that consumer requires it.
- **High-level scalable coding authoring** (SCAL-01..04) — recon gain and demixing weights are DSP-adjacent; the low-level model serialises values it is given, but the safe builder does not construct them until a caller and oracle are specified.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Conformant LPCM Bitstream | 10/10 | Complete | 2026-09-09 |
| 2. Parser, Round-Trip and Fuzzing | 7/7 | Complete | 2026-09-09 |
| 3. FLAC and Opus Framing | 8/8 | Complete | 2026-09-11 |
| 4. Parallax-Facing API | 4/4 | Complete | 2026-09-12 |

---
*Roadmap created: 2026-09-08*
*Granularity: coarse — 4 phases, one per milestone M1–M4*
*Coverage: 94/94 v1 requirements mapped*
