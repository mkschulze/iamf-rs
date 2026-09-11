# Requirements: iamf-rs

**Defined:** 2026-09-08
**Core Value:** A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM sample-identical — *and* accepted by `iamf-tools`' stricter parser, because `libiamf` alone is a permissive oracle.
**Spec version:** IAMF v1.1.0 (decided 2026-09-08)
**Licence:** `MIT OR Apache-2.0` (decided 2026-09-08)

## v1 Requirements

Requirements for the initial release, covering milestones M1–M4. Each maps to a roadmap phase.

### Bit-level I/O

- [x] **BITS-01**: Hand-rolled `BitCursor<'a>` borrowing `&[u8]` and `BitWriter`, exposing `is_byte_aligned()`, `bits_remaining()` and a bounded `sub_reader(len)`; the cursor owns the byte offset so every failure carries a native located error
- [x] **BITS-02**: Reader and writer method names mirror `iamf-tools`' `read_bit_buffer.h` / `write_bit_buffer.h` one-for-one, with separate signed and unsigned methods
- [x] **BITS-03**: Hand-written uleb128 writer emitting minimal form, enforcing the 8-byte and `u32::MAX` caps
- [x] **BITS-04**: Hand-written uleb128 reader accepting 1–8 bytes, returning a typed error on a 9th continuation byte and on a value exceeding `u32::MAX`
- [x] **BITS-05**: Byte alignment asserted at every OBU boundary — the reference pads nothing and errors instead
- [x] **BITS-06**: Hand-computed unit vectors covering the bit primitives, written before any OBU type exists
- [x] **BITS-07**: Only `src/bits` owns shipping bit offsets and primitives, and no `std::io::Error` escapes that boundary; `bitstream-io` is dev-only and imported solely by the randomized differential oracle, never by shipping code

### OBU header and common structure

- [x] **OBU-01**: Header byte 0 encoded MSB-first as `[obu_type:5][obu_redundant_copy:1][type_specific_flag:1][obu_extension_flag:1]`
- [x] **OBU-02**: `obu_size` written as uleb128 counting every byte after the size field itself — trim fields and extension header included, byte 0 and the size bytes excluded
- [x] **OBU-03**: Bit 6 modelled as an enum carried by the payload variant, not a shared `bool`, with exactly **two** cases — trimming status for Audio Frames (types 5 and 6..=23), and reserved-SHALL-be-0 for every other type

  *Amended 2026-09-08 (plan 01-04).* The original wording gave bit 6 four meanings, adding an inverted key-frame flag for the Temporal Delimiter and an optional-fields flag for Mix Presentation. **That describes `iamf-tools@main`, a draft-v2.0.0 tree — not the v2.1.0 tag this project pins.**

  - *Positive evidence.* At `iamf-tools@v2.1.0` the field is named `obu_trimming_status_flag`, and `IsTrimmingStatusFlagAllowed` (`iamf/obu/obu_header.cc:60-70`) returns `true` **only** for `kObuIaAudioFrame` and `kObuIaAudioFrameId0..=17`. `ObuHeader::Validate` (`obu_header.cc:95-101`) turns any other use into an `InvalidArgumentError`.
  - *Negative evidence.* A repository-wide grep for the four draft-v2.0.0 symbol names over `iamf/obu/` returns **nothing** at v2.1.0 and returns **all of them** at `main`, where the accompanying comment reads "In IAMF v2.0.0, if `optional_fields_flag` is true…".
  - *Decoder-side consequence, and the reason this is dangerous rather than merely wrong.* `libiamf@v1.1.0`'s `IAMF_OBU_split` (`code/src/iamf_dec/IAMF_OBU.c:64-120`) reads the two trim fields whenever bit 6 is set, **for any OBU type**; its own development tip guards that with an audio-frame check, but the tree we pin does not. So the four-variant model shifts a descriptor OBU's entire payload by two bytes on the exact decoder whose acceptance is the Core Value — silently, with no error and no crash, and invisibly to any test that only round-trips against itself.

  **OBU-04 is unaffected.** The `obu_redundant_copy` legality table is correct as written and was verified at the same tag (`obu_header.cc:43-57`). Implemented as `TypeSpecific { Trimming(Option<Trimming>), Reserved }` in `src/obu/header.rs`; a verify gate greps `src/` for the draft-v2.0.0 names and must find none.
- [x] **OBU-04**: `obu_redundant_copy` rejected on audio frames, temporal delimiters and parameter blocks; legal only on descriptors (0, 1, 2, 31)
- [x] **OBU-05**: Trimming fields written END first, then START
- [x] **OBU-06**: Extension header (`extension_header_size` uleb128 plus that many bytes) read and written
- [x] **OBU-07**: Every OBU struct carries `trailing: Vec<u8>` from its first commit, drained centrally by the shared read wrapper
- [x] **OBU-08**: `find_obu_boundaries(&[u8])` helper whose last boundary must land exactly on `bytes.len()`, written before the first encoder test

### Descriptor OBUs

- [x] **DESC-01**: IA Sequence Header (type 31) serialised and parsed
- [x] **DESC-02**: Codec Config (type 0) with an LPCM decoder config and a **derived** `audio_roll_distance`
- [x] **DESC-03**: `sample_format_flags` handled with the correct sense — `0` means **big**-endian, the opposite of a WAV-shaped assumption
- [x] **DESC-04**: Audio Element (type 1), single-layer channel-based, with recon gain and output gain flagged absent
- [x] **DESC-05**: Mix Presentation (type 2) with one sub-mix, including the **mandatory stereo layout** the reference hard-checks
- [x] **DESC-06**: Mandatory element and output Mix Gain param definitions (`param_definition_mode = 1`, `default_mix_gain = 0`) emitted even when the file contains zero Parameter Block OBUs
- [x] **DESC-07**: Layouts modelled as four distinct types — `loudspeaker_layout` (4 bits), `expanded_loudspeaker_layout` (u8, present only when layout == 15, offset by 13 from Eclipsa's index), `AmbisonicsConfig`, and `SoundSystem` — not one flat 31-variant enum
- [x] **DESC-08**: Descriptors written in reference order, honouring forward-only reference resolution (Audio Element references a Codec Config ID; Mix Presentation references Audio Element IDs)
- [x] **DESC-09**: Descriptor collections held as `Vec` in bitstream order with a `by_id()` accessor — never `BTreeMap`, which reorders bitstream-visible order

### Time-varying OBUs

- [x] **TIME-01**: Audio Frame (type 5, and 6–23) with implicit substream IDs (`id <= 17` → type `6 + id`)
- [x] **TIME-02**: BCG channel→substream packing in the correct order — coupled stereo pairs first, then mono; distinct from presentation channel order
- [x] **TIME-03**: Trimming applied to the final frame
- [x] **TIME-04**: Temporal Delimiter (type 4)
- [x] **TIME-05**: Parameter Block (type 3) parsed with its `ParamDefinition` supplied as an explicit context argument, never as hidden parser state

### Sequence and container

- [x] **SEQ-01**: Standalone IA Sequence writer producing `.iamf` — descriptors then data
- [x] **SEQ-02**: Streaming as the primitive: `push_descriptors` → `push_temporal_unit` → `finish`, so an hour of 7.1.4 24-bit never needs to be resident
- [x] **SEQ-03**: Whole-file convenience wrapper over the streaming primitive

### Profile and metadata

- [x] **PROF-01**: Profile enum whose legal range follows the pinned spec version
- [x] **PROF-02**: Minimum-profile selection — pick the smallest profile a given configuration fits
- [x] **PROF-03**: Loudness quantisation helper converting caller-supplied LUFS to Q7.8 using `round_ties_even`, range-checked — not `as i16`, which truncates toward zero and biases every negative value upward by up to 1 LSB

### Conformance verification

- [x] **CONF-01**: A reusable `assert_conformant(config, pcm)` function, so later milestones reuse the harness unchanged
- [x] **CONF-02**: Test signal is per-channel-distinguishable and non-silent
- [x] **CONF-03**: Total sample count is **not** a multiple of the frame size, forcing `trim_at_end > 0`
- [x] **CONF-04**: Fixture has ≥6 OBUs including two Codec Configs or two Audio Elements, so ordering is observable
- [x] **CONF-05**: `libiamf` at a pinned commit decodes the file, **the decoded sample count equals what was encoded**, and the PCM is sample-identical
- [x] **CONF-06**: `iamf-tools`' own parser accepts the file — the only check that catches reserved-bit misuse and leb128 strictness
- [x] **CONF-07**: Byte-diff against an `iamf-tools`-produced file for an identical configuration is either identical or has every difference enumerated in writing
- [x] **CONF-08**: Byte-comparison reproducing the shipped golden `libiamf/tests/test_000003.iamf`
- [x] **CONF-09**: Reference binaries invoked by `Command` in tests, discovered via `IAMF_REF_DECODER`, with `tools/build-reference.sh` at pinned commits — in one Linux CI job, never a `build.rs`
- [x] **CONF-10**: Golden fixtures are the always-on layer, so `cargo test` is green offline on all four targets
- [x] **CONF-11**: Select the committed `libiamf@v1.1.0` `.iamf` fixtures, vendor the D-14 size-capped subset from the pinned tree using `git show`, and record selected files, skipped files and regeneration instructions in `tests/fixtures/MANIFEST.md`; the digest-pinned Bazel container generates CONF-07's matching custom fixture, not the offline reference corpus

### Guardrails

- [x] **GUARD-01**: `deny.toml` copied **verbatim** from Parallax, and proven to fail by deliberately adding an LGPL crate
- [x] **GUARD-02**: `clippy.toml` `disallowed-types` banning `HashMap`/`HashSet` outright, with a `reason` string
- [x] **GUARD-03**: Hardening lints enabled, including `clippy::indexing_slicing` and `clippy::arithmetic_side_effects`
- [x] **GUARD-04**: No `unwrap()` or `expect()` outside tests
- [x] **GUARD-05**: `SPEC_VERSION` constant named in the code
- [x] **GUARD-06**: `REFERENCES.md` pinning exact `iamf-tools` and `libiamf` SHAs, with CI checking out those SHAs and never `main`
- [x] **GUARD-07**: `NOTICE` file carrying attribution for ported BSD-licensed reference code
- [x] **GUARD-08**: `CONTRIBUTING.md` carrying an explicit "did not consult `gpac` or `libspatialaudio`" checkbox
- [x] **GUARD-09**: Cross-target byte-identity as a committed golden fixture reproduced on macOS arm64, macOS x86_64, Windows MSVC and Linux x64
- [x] **GUARD-10**: Same-process double-encode test — the only cheap check that catches per-process hash seeding
- [x] **GUARD-11**: A no-DSP guard (grep or lint) enforcing the scope boundary
- [x] **GUARD-12**: `rust-toolchain.toml` pinning Rust 1.85 / edition 2024, so the byte-identity matrix compiles identically everywhere
- [x] **GUARD-13**: `thiserror` error type, `#[non_exhaustive]`, with a byte `offset` on every variant

### Parser and round-trip

- [x] **PARSE-01**: Sequence-level parser reading a bitstream back into the model
- [x] **PARSE-02**: `ParamDefinitionRegistry` passed as an explicit argument to the parser
- [x] **PARSE-03**: `parse(serialize(model)) == model` holds universally
- [x] **PARSE-04**: `serialize(parse(bytes)) == bytes` holds for our own output, with the non-minimal-leb128 caveat documented for foreign files
- [x] **PARSE-05**: Unknown OBU types preserved verbatim at the correct byte offset
- [x] **PARSE-06**: Unknown parameter data preserved verbatim
- [x] **PARSE-07**: Files produced by `iamf-tools` parse successfully and are asserted understood

### Fuzzing

- [x] **FUZZ-01**: `fuzz/` as an independent workspace with its own lockfile, excluded from the root workspace
- [x] **FUZZ-02**: `parse_sequence` fuzz target
- [x] **FUZZ-03**: `obu_roundtrip` fuzz target
- [x] **FUZZ-04**: Corpus committed and seeded from real `iamf-tools` output
- [x] **FUZZ-05**: Stable-toolchain corpus-regression test on all four targets, so every crash ever found becomes a permanent cross-platform test

### Codec framing

- [x] **CODEC-01**: FLAC STREAMINFO `decoder_config` written as hand-written bit fields — channels pinned to 1, frame sizes 0, MD5 zero, `bits_per_sample` stored as value − 1
- [x] **CODEC-02**: Opus `OpusHead` `decoder_config` — 11 bytes, `output_channel_count` fixed at 2, `output_gain` 0, `mapping_family` 0, output rate always 48 kHz
- [x] **CODEC-03**: `audio_roll_distance` derived for Opus as `-ceil(3840 / num_samples_per_frame)`
- [x] **CODEC-04**: Opus priming produces a non-zero `trim_at_start`, exercising trim ordering a second way
- [x] **CODEC-05**: A sample-rate mismatch returns `Error::SampleRateNotSupportedByCodec` — no resampler, ever
- [x] **CODEC-06**: `cargo deny check licenses` run on a throwaway branch that merely adds the candidate codec crates, before the codec work begins
- [x] **CODEC-07**: Zero codec dependencies in the shipping crate; codec crates appear only as dev-dependencies for test material

### Parallax-facing API

- [ ] **API-01**: A host-independent `EncoderBuilder` is the only high-level entry point for static export configuration. `build()` validates and freezes descriptors, codec/frame plans, cross-references and parameter definitions into an immutable `Encoder`; fallible temporal input and sink I/O remain explicitly outside this static-validation claim
- [ ] **API-02**: The builder accepts caller-local opaque handles in stable input order, assigns every codec-config, Audio Element, Mix Presentation, substream and parameter wire ID deterministically, and returns an immutable handle-to-wire-ID manifest for diagnostics and temporal-data submission. No Parallax ID type enters this crate
- [ ] **API-03**: Minimum-profile selection evaluates every Mix Presentation independently and writes the highest profile required by any one Presentation into the sequence header. It never selects from the union of elements across unrelated Presentations, and it supports the known expanded loudspeaker layouts needed for profile counting while rejecting reserved layouts
- [ ] **API-04**: The high-level configuration supports ordered multiple Mix Presentations, multiple valid sub-mixes at the generic IAMF layer, shared channel-based or scene-based Audio Elements, per-element rendering and mix gain, per-sub-mix output gain, layouts, labels and caller-supplied loudness. Shared Audio Elements and substreams are described and emitted once
- [ ] **API-05**: The initial convenience constructors required by Parallax cover single-layer channel-based elements and Ambisonics-mono scene-based elements. Scalable channel layers, demixing/recon-gain authoring and Ambisonics projection remain accessible only through the existing low-level model until a separately verified high-level contract is approved
- [ ] **API-06**: The frame API accepts LPCM frame payloads or already encoded FLAC/Opus access units together with their typed codec/frame plan. It validates codec identity, sample rate, samples per frame, substream coverage, trim agreement and temporal-unit alignment; it never performs codec encoding, resampling, panning, rendering or loudness measurement
- [ ] **API-07**: Parameter data is accepted as pre-decimated IAMF blocks (DEC-03). The builder validates definitions; each temporal submission validates known IDs, type, rate, duration and exact tiling transactionally before emitting any byte. The crate never owns a timeline, interpolates curves or accepts Parallax Source-position automation
- [ ] **API-08**: A built encoder starts an append-only writer over plain `W: Write`, writes descriptors once, transactionally preflights each temporal unit, becomes observably poisoned after a partial sink failure, and consumes itself on `finish()`. Static build errors, temporal-input errors and sink errors remain distinct typed `#[non_exhaustive]` errors
- [ ] **API-09**: Public-API-only contract fixtures cover ordinary Stereo, mixed channel-based plus HOA scene elements, multiple ordered Presentations sharing an Audio Element, explicit element/output gains and loudness, pre-decimated parameter blocks, and LPCM/FLAC/Opus framing. Each emitted deliverable passes the existing structural, deterministic and external-conformance gates applicable to its codec
- [ ] **API-10**: The high-level API contains no Parallax graph, terminal, lineage, Preview or Include-in-Export state. A Parallax-shaped contract fixture proves that a caller can filter its delivery snapshot first and then map it through public handles alone; the production adapter remains in Parallax
- [ ] **API-11**: Bitstream parsing/serialization and the high-level encoder remain available with `--no-default-features` and no linked codec or integration dependency. The existing proc-macro-only `thiserror` graph is not misreported as literally dependency-free; fuzz and `cargo deny` continue to exercise this minimal production surface

### Decisions to settle

- [x] **DEC-01**: `SPEC_VERSION = "1.1.0"` pinned in code, with the profile enum's legal range and the expanded-layout set following from it *(decided 2026-09-08)*
- [x] **DEC-02**: Crate published under `MIT OR Apache-2.0` — `Cargo.toml` `license` field, `LICENSE-MIT` + `LICENSE-APACHE`, README licence section, and the `NOTICE` file Apache-2.0 §4(d) wants *(decided 2026-09-08; licence files landed, `Cargo.toml` follows in Phase 1)*
- [x] **DEC-03**: IAMF parameter data is taken as **pre-decimated blocks**; no time model or interpolation enters this crate *(decided 2026-09-08; clarified 2026-09-11: in the v1.1 Parallax scope these blocks describe IAMF mix/demixing/recon-gain parameters, not Source positions, whose motion is already baked into upstream Bed/HOA PCM)*
- [x] **DEC-04**: `iamf-tools` tags `v2.0.0` and `v2.1.0` fetched to check whether either is a v1.1.0-exact tree, before the type model is written
- [x] **DEC-05**: `libiamf`'s `codec_config_obu.c` and `audio_frame_obu.c` read for payload-level rejection rules before the LPCM path is written

## v2 Requirements

Deferred to a future release. Tracked but not in the current roadmap.

### Container

- **BMFF-01**: ISO-BMFF encapsulation — IA sample entry, configuration box holding the descriptors, IA samples with trimming metadata
- **BMFF-02**: Muxer written from the IAMF spec's ISO-BMFF binding section and `iamf-tools`' own muxer, never `gpac`
- **BMFF-03**: Confirm `iamf-tools` contains a usable permissively-licensed muxer before committing to this work

### Decoder consumer boundary

- **DECO-01**: Keep the parsed wire model consumable by the sibling `iamf-decode-rs`; native codec decoding and timed Audio Element reconstruction never enter this repository
- **DECO-02**: Streaming/RT allocation and locking constraints belong to `iamf-decode-rs`; this crate exposes bounded parsed data without assuming its consumer thread
- **DECO-03**: Add AAC-LC decoder-config wire syntax here only when required by `iamf-decode-rs`; the AAC codec implementation remains outside this crate

### High-level scalable coding authoring

- **SCAL-01**: Safe builder construction for multi-layer channel configurations (the BCG/DCG ladder)
- **SCAL-02**: Safe builder construction for demixing weights
- **SCAL-03**: Safe builder construction for recon gain
- **SCAL-04**: Safe builder construction for Ambisonics projection mode

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Rendering, panning, or any spatial DSP | Parallax owns Source/AGIO panning and HOA encoding; `iamf-render-rs` owns OAR rendering. A PR adding DSP here is in the wrong repository |
| A resampler for codec sample-rate mismatch | The single most likely first DSP breach. Correct answer is a typed error (CODEC-05) |
| Loudness measurement | Parallax's BS.1770 meter shares code with export normalisation (`MON-05`). This crate carries the numbers |
| UI of any kind | This is a library |
| Object-based audio elements | The target profiles forbid them — `profile_filter.cc` erases Simple, Base and Base-Enhanced the moment an element is object-based — and `libiamf` cannot decode them. Present in `iamf-tools` HEAD only as v2.0-draft machinery |
| Mirroring `iamf-tools` HEAD's type model | HEAD is a draft-v2.0.0 tree. Its Metadata OBU, param types 3–8, profiles 3–5 and re-laid-out `RenderingConfig` produce files `libiamf` rejects |
| `gpac` as a muxer | LGPL-2.1, rejected by Parallax's `deny.toml` |
| `libspatialaudio` as a reference | LGPL-2.1+, may not be read or ported |
| A `LebMode` knob on the public encoder | Non-minimal leb128 is legal on the wire and the parser accepts it, but exposing the choice makes byte-identity caller-dependent |
| `build.rs`-driven reference builds | Would force CMake, C++20, abseil, protobuf and fdk-aac onto every `cargo build`, Parallax's and docs.rs's included |
| Publishing to crates.io | `publish = false` until the library does something real |

## Traceability

Populated during roadmap creation (2026-09-08). Every v1 requirement maps to exactly one phase.

| Requirement | Phase | Status |
|-------------|-------|--------|
| BITS-01 | Phase 1 | Complete |
| BITS-02 | Phase 1 | Complete |
| BITS-03 | Phase 1 | Complete |
| BITS-04 | Phase 1 | Complete |
| BITS-05 | Phase 1 | Complete |
| BITS-06 | Phase 1 | Complete |
| BITS-07 | Phase 1 | Complete |
| OBU-01 | Phase 1 | Complete |
| OBU-02 | Phase 1 | Complete |
| OBU-03 | Phase 1 | Complete |
| OBU-04 | Phase 1 | Complete |
| OBU-05 | Phase 1 | Complete |
| OBU-06 | Phase 1 | Complete |
| OBU-07 | Phase 1 | Complete |
| OBU-08 | Phase 1 | Complete |
| DESC-01 | Phase 1 | Complete |
| DESC-02 | Phase 1 | Complete |
| DESC-03 | Phase 1 | Complete |
| DESC-04 | Phase 1 | Complete |
| DESC-05 | Phase 1 | Complete |
| DESC-06 | Phase 1 | Complete |
| DESC-07 | Phase 1 | Complete |
| DESC-08 | Phase 1 | Complete |
| DESC-09 | Phase 1 | Complete |
| TIME-01 | Phase 1 | Complete |
| TIME-02 | Phase 1 | Complete |
| TIME-03 | Phase 1 | Complete |
| TIME-04 | Phase 1 | Complete |
| TIME-05 | Phase 1 | Complete |
| SEQ-01 | Phase 1 | Complete |
| SEQ-02 | Phase 1 | Complete |
| SEQ-03 | Phase 1 | Complete |
| PROF-01 | Phase 1 | Complete |
| PROF-02 | Phase 1 | Complete |
| PROF-03 | Phase 1 | Complete |
| CONF-01 | Phase 1 | Complete |
| CONF-02 | Phase 1 | Complete |
| CONF-03 | Phase 1 | Complete |
| CONF-04 | Phase 1 | Complete |
| CONF-05 | Phase 1 | Complete |
| CONF-06 | Phase 1 | Complete |
| CONF-07 | Phase 1 | Complete |
| CONF-08 | Phase 1 | Complete |
| CONF-09 | Phase 1 | Complete |
| CONF-10 | Phase 1 | Complete |
| CONF-11 | Phase 1 | Complete |
| GUARD-01 | Phase 1 | Complete |
| GUARD-02 | Phase 1 | Complete |
| GUARD-03 | Phase 1 | Complete |
| GUARD-04 | Phase 1 | Complete |
| GUARD-05 | Phase 1 | Complete |
| GUARD-06 | Phase 1 | Complete |
| GUARD-07 | Phase 1 | Complete |
| GUARD-08 | Phase 1 | Complete |
| GUARD-09 | Phase 1 | Complete |
| GUARD-10 | Phase 1 | Complete |
| GUARD-11 | Phase 1 | Complete |
| GUARD-12 | Phase 1 | Complete |
| GUARD-13 | Phase 1 | Complete |
| PARSE-01 | Phase 2 | Complete |
| PARSE-02 | Phase 2 | Complete |
| PARSE-03 | Phase 2 | Complete |
| PARSE-04 | Phase 2 | Complete |
| PARSE-05 | Phase 2 | Complete |
| PARSE-06 | Phase 2 | Complete |
| PARSE-07 | Phase 2 | Complete |
| FUZZ-01 | Phase 2 | Complete |
| FUZZ-02 | Phase 2 | Complete |
| FUZZ-03 | Phase 2 | Complete |
| FUZZ-04 | Phase 2 | Complete |
| FUZZ-05 | Phase 2 | Complete |
| CODEC-01 | Phase 3 | Complete |
| CODEC-02 | Phase 3 | Complete |
| CODEC-03 | Phase 3 | Complete |
| CODEC-04 | Phase 3 | Complete |
| CODEC-05 | Phase 3 | Complete |
| CODEC-06 | Phase 3 | Complete |
| CODEC-07 | Phase 3 | Complete |
| API-01 | Phase 4 | Pending |
| API-02 | Phase 4 | Pending |
| API-03 | Phase 4 | Pending |
| API-04 | Phase 4 | Pending |
| API-05 | Phase 4 | Pending |
| API-06 | Phase 4 | Pending |
| API-07 | Phase 4 | Pending |
| API-08 | Phase 4 | Pending |
| API-09 | Phase 4 | Pending |
| API-10 | Phase 4 | Pending |
| API-11 | Phase 4 | Pending |
| DEC-01 | Phase 1 | Complete |
| DEC-02 | Phase 1 | Complete |
| DEC-03 | Phase 4 | Complete |
| DEC-04 | Phase 1 | Complete |
| DEC-05 | Phase 1 | Complete |

**Coverage:**

- v1 requirements: 94 total
- Mapped to phases: 94 ✓
- Unmapped: 0 ✓

**Per phase:**

| Phase | Name | Requirements |
|-------|------|--------------|
| Phase 1 | Conformant LPCM Bitstream | 63 |
| Phase 2 | Parser, Round-Trip and Fuzzing | 12 |
| Phase 3 | FLAC and Opus Framing | 7 |
| Phase 4 | Parallax-Facing API | 12 |

---
*Requirements defined: 2026-09-08*
*Last updated: 2026-09-11 after Phase 3 closure and the Parallax integration-contract audit — traceability populated, 94/94 mapped*
