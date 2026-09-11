# iamf-rs

## What This Is

A Rust implementation of **IAMF** — the Alliance for Open Media's Immersive Audio Model and Formats
bitstream. It is an OBU serialiser and parser, the descriptor model, and an encoder that produces
conformant `.iamf` files. Native codec decoding and timed Audio Element reconstruction live in the
sibling `iamf-decode-rs` repository. It exists because there is **no Rust IAMF
implementation on crates.io** (verified 2026-09-07) and Parallax — a deterministic spatial-audio DAW —
needs IAMF metadata/bitstream support for its `iamf-render-rs` preview path and native export format.

The crate is `iamf`; the repo is `iamf-rs`. Parallax consumes it as a path dependency during
development.

## Core Value

**A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM
sample-identical.** Everything else — API shape, codec coverage, containers, the decoder — is
secondary to producing bytes the reference implementation accepts.

Sharpened by research (2026-09-08): `libiamf` is a *permissive* reader — it ignores reserved-bit
misuse and clamps an overlong leb128 rather than erroring. **Passing the `libiamf` gate is necessary
but not sufficient for conformance.** The real exit criterion is a byte-diff against an
`iamf-tools`-produced file that is either identical or fully explained in writing.

## Requirements

### Validated

- [x] IAMF v1.1 bit-level primitives, OBU model, descriptor and temporal-unit serialization
- [x] Standalone IA Sequence writing accepted by the pinned reference implementations
- [x] Context-aware parser, exact known/unknown round trips and bounded fuzzing corpus
- [x] Deterministic profile/layout/loudness primitives and four-target byte-identity gates
- [x] LPCM plus dependency-free FLAC/Opus configuration and framing of externally generated packets

### Active

- [ ] Host-independent `EncoderBuilder` with immutable static configuration and deterministic caller-handle→wire-ID manifest
- [ ] Presentation-local global minimum-profile selection including known expanded layouts
- [ ] High-level multiple-Presentation/shared-element construction for single-layer channel and Ambisonics-mono scene elements
- [ ] Typed LPCM/external-FLAC/external-Opus temporal input with transactional validation and append-only `W: Write`
- [ ] Pre-decimated IAMF parameter blocks with no caller timeline or Source-position model
- [ ] Parallax-shaped public-contract fixtures without a Parallax dependency
- [ ] Minimal no-default-features production surface with no linked codec/integration dependency

### Out of Scope

- **Any rendering, panning or spatial DSP** — Parallax owns Source/AGIO panning and HOA encoding;
  `iamf-render-rs` owns OAR loudspeaker/binaural rendering. This crate receives prepared PCM/access
  units plus metadata and produces bytes. A PR that adds DSP here is in the wrong repository.
- **Loudness measurement** — Parallax has a BS.1770 meter sharing code with export normalisation
  (`MON-05`). This crate carries the numbers; it does not compute them.
- **UI of any kind** — this is a library.
- **AAC-LC codec implementation** — belongs to `iamf-decode-rs`, not this wire crate. Its decoder
  configuration syntax may be added here when that consumer requires it. *Correction 2026-09-08:* `iamf-tools` `main`
  now ships an AAC-LC encoder, so the handoff's "three codecs shipped, not four" describes the v1.x
  tree, not HEAD. Still out of scope for v1 here; revisit if a decoder needs it.
- **High-level scalable-layer authoring** (BCG/DCG ladder, demixing weights, recon gain) — the
  low-level wire model already represents these structures, but the safe Phase 4 builder does not
  construct them until a caller and conformance oracle are specified. *Correction 2026-09-08:*
  `iamf-tools` implements multi-layer and recon gain fully; this is our scope decision, not a limit of
  the reference.
- **An object-based element path** — out of scope because the target profiles forbid it.
  *Correction 2026-09-08:* `iamf-tools` HEAD (a draft-v2.0.0 tree) *does* have an object-based element
  path. The exclusion stands on profile grounds, not on absence from the reference.
- **Ambisonics projection mode** — deferred.
- **A port of Google's Eclipsa plugin suite** — considered and rejected 2026-09-07.
- **`gpac` as the ISO-BMFF muxer** — LGPL-2.1, rejected by Parallax's `deny.toml`. If ISO-BMFF is
  built, we write it ourselves. First confirm whether `iamf-tools` has its own muxer.
- **`libspatialaudio`** — LGPL-2.1+, may not be read or ported.
- **Publishing to crates.io** — `publish = false` until the library does something real.

## Context

### The reference implementations are moving targets

**`iamf-tools` `main` is a draft-v2.0.0 tree** — by its own TODO. It carries an object-based element
path, position parameter types 3–8, a Metadata OBU, six profiles (0–5, adding Base-Advanced,
Advanced1, Advanced2), an AAC-LC encoder, and a re-laid-out `RenderingConfig`. **Mirroring HEAD's type
model produces files `libiamf` rejects** — precisely the "almost works" failure the handoff warns
about, arriving from a direction nobody was watching.

**Therefore: read `iamf-tools` at a v1.x tag, and pin the SHA in the code.** Same for `libiamf`.

### Corrections to the handoff's corrections

The handoff listed five facts as verified-from-code. Research on 2026-09-08 re-checked them:

| # | Handoff claim | Status |
|---|---|---|
| 1 | No object-based element path in practice | **Provenance wrong.** Came from Eclipsa, not `iamf-tools`. HEAD has one. The scope exclusion still holds on *profile* grounds |
| 2 | Scalability unimplemented; *"Keeping things simple with 1 layer for now"* | **Quote does not exist in `iamf-tools`.** It implements multi-layer and recon gain fully. The quote came from Eclipsa. Single-layer remains our scope choice |
| 3 | Three codecs shipped, not four (no AAC-LC) | **True of v1.x, false of HEAD** — HEAD ships an AAC-LC encoder |
| 4 | Profiles: Simple 1/16 · Base 2/18 · Base-Enhanced 28/28 | **Version-dependent.** Base-Enhanced does not exist in v1.0 at all; HEAD has six profiles |
| 5 | Layouts numbered 0–30 in three bands | **Not an IAMF field.** This is Eclipsa's plugin enum (`Speakers.h:133–186`), conflating three wire fields across two OBUs. The Rust model needs four separate types |

The handoff's *method* was right — "read the code, not the prose" — and it still produced five claims
with weaker provenance than stated. Provenance labelling is what caught it. Keep labelling.

### The OBU header — resolved

Verified three independent ways: the encoder write path (`iamf-tools/iamf/obu/obu_header.cc`), the
decoder read path (`libiamf/code/src/iamf_dec/obu/iamf_obu.c:56–95`), and by hand-decoding a golden
conformance file.

- **Byte 0, MSB-first:** `[obu_type:5][obu_redundant_copy:1][type_specific_flag:1][obu_extension_flag:1]`
- **Then `obu_size`** as uleb128, 1–8 bytes, value must fit `u32`. **Not required to be minimal** —
  `LebGenerator` has a deliberate `kFixedSize` mode, so an off-the-shelf minimal-only leb128 crate
  cannot reproduce a reference file byte-for-byte. Write it by hand.
- **`obu_size` counts everything after itself**: conditional trim fields, conditional extension header,
  and payload. It excludes byte 0 and the size bytes. `libiamf` reconstructs total as
  `obu_size + size_of(obu_size) + 1`.
- **Bit 6 is `type_specific_flag`, not universally a trimming flag.** Its meaning depends on
  `obu_type`: trimming status for audio frames (5–23); `is_not_key_frame` **inverted** for temporal
  delimiter (4); `optional_fields_flag` for mix presentation (2); reserved-SHALL-be-0 otherwise.
- **`obu_redundant_copy` is forbidden** on audio frames, temporal delimiter and parameter block.
- **Trimming fields are written END first, then START** — the opposite of intuition, and byte-identical
  whenever the two values are equal, which is almost always. A test where they differ is mandatory.
- **There is no padding mechanism.** Payloads must be byte-aligned by construction or the reference
  errors out.

Worked example, offset `0x7D32` of `test_000003.iamf`: `32 82 04 40 00` → type 6, trimming flag set,
size 514 = 2 trim bytes + 512 PCM bytes.

### `libiamf` is a permissive reader

A dangerous asymmetry: `libiamf` ignores reserved-bit misuse and *clamps* an overlong leb128 to
`UINT32_MAX`, where `iamf-tools` errors on both. `libiamf`'s splitter returns `0` when
`obu_size > remaining`, which callers read as end-of-stream — so an oversized size field on the final
OBU manifests as a **shorter file, not a decode failure**. Build the byte-diff-against-`iamf-tools`
check into M1, not later.

### The minimum viable file is not a guess

`libiamf/tests/test_000003.iamf` is a shipped conformance vector matching M1's target exactly, with a
118-byte descriptor prologue. Two non-obvious requirements it reveals: every sub-mix **must** contain a
stereo layout (hard encoder check), and mix-gain param definitions are mandatory structure even when
the file contains zero Parameter Block OBUs (`param_definition_mode = 1`, `default_mix_gain = 0`).

**The M1 fixture's own properties are a design decision.** It must be non-silent and
per-channel-distinguishable, its sample count must **not** be a multiple of the frame size, and it
needs ≥6 OBUs including two Codec Configs. Without those four properties the fixture cannot detect four
of the six most likely bugs.

### Highest silent-failure risk in M1

Channel→substream BCG packing order — coupled pairs first, then mono. Get it wrong and you get a
**clean decode with scrambled channels**. No error, no crash, correct byte count.

### Determinism is cheaper than expected, in one place and dearer in another

**Cheaper:** the IAMF wire format contains **no floating-point fields**. Loudness and mix gain are
`int16` Q7.8; object positions are `int16`/`int8`/`uint8`. Floats appear only in renderer-side
interpolation, which this crate does not have. **`libm` is not a day-one dependency.**

**Dearer:** `BTreeMap` is the *wrong* fix for the `HashMap` ban. The reference keys descriptors by ID
in `absl::flat_hash_map` and calls `SortedKeys()` before writing — but sorting by ID is not the same as
preserving bitstream order, and **descriptor order is bitstream-visible**. Swapping to `BTreeMap`
trades a determinism bug for a round-trip bug. Use a `Vec` in bitstream order plus a `by_id` lookup.

### Parameter Block parsing is context-dependent

`ParameterBlockObu::CreateFromBuffer` requires the `ParamDefinition` that lives inside a *descriptor*
OBU. C++ cannot express this through its virtual read path and routes around it with a
`PeekParameterId` + static factory. Rust can express it with a GAT (`type Ctx<'a>`), removing a whole
class of "called the wrong factory" bug. This single fact shapes the parser, the public API, and the
fuzz target.

### Round-trip preservation is already solved by the reference — three ways

Copy all three: `ObuBase::footer_` (trailing bytes; `obu_size` may legally exceed known syntax),
`ArbitraryObu` with an `InsertionHook` enum (unknown OBU types preserved *at the right byte offset*),
and `ExtensionParameterData` (unknown parameter data verbatim).

### Fixture generation is a hidden long-pole

`iamf-tools/iamf/cli/testdata/` holds 338 `.textproto` files and **one** `.iamf`; no release ships a
test-vector bundle. M2's "parse `iamf-tools`-produced files" therefore needs a one-time Bazel build to
generate and commit fixtures. It is off the code critical path — start it on day one rather than
discover it late.

### Companion research

`.planning/research/` (this repo) and the Parallax repo's `docs/eclipsa/` — seven files whose
`06-SOURCES.md` marks which facts are verified-from-code and which are from prose.

## Constraints

- **Determinism**: No `HashMap`/`HashSet` anywhere the output byte order can see them. Prefer a `Vec`
  in bitstream order plus a `by_id` lookup — **not** `BTreeMap`, which reorders bitstream-visible
  descriptor order. — `parallax render` is byte-identical across macOS arm64, macOS x86_64, Windows
  MSVC and Linux x64, run to run and target to target. An export must be too.
- **Determinism (CI)**: Byte-identity as a **committed golden fixture** every target must reproduce —
  not a cross-job artifact comparison. — Makes an output change a reviewable PR diff rather than a red
  job with no explanation.
- **No platform transcendentals** in anything affecting output bytes; use `libm` if any appear. — Not
  currently binding: the wire format has no floating-point fields. Keep the rule; expect it to cost
  nothing.
- **Licence allow-list**: Every transitive dependency must satisfy Parallax's `deny.toml`. Preferred:
  MIT, BSD, Apache-2.0, ISC, Zlib, Unlicense, CC0. Allowed with a recorded reason: MPL-2.0 for an
  *unmodified upstream* crate; **NCSA**, added to Parallax's allow-list by user decision 2026-09-07
  (permissive, BSD/MIT-style, no copyleft) to admit `libfuzzer-sys`. **`Unicode-3.0` is mandatory** or
  a clean build fails (`unicode-ident` ← `syn` ← `thiserror-impl`). Rejected outright: GPL, LGPL, AGPL,
  SSPL, EUPL, CDDL, OSL. State each dependency's licence on the line that adds it. — If Parallax cannot
  consume this crate, the crate has no purpose.
- **`cargo-deny` config shape**: The `deny`, `copyleft`, `allow-osi-fsf-free`, `default` and `version`
  keys have been removed from cargo-deny and now error. Rejection is expressed **by omission** from
  `allow`, not by a deny list — record that in a comment so the intent survives review. — Verified
  against cargo-deny 0.20.2.
- **Source licence hygiene**: `libspatialaudio` (LGPL-2.1+) and `gpac` (LGPL-2.1) may not be read or
  ported. `iamf-tools`, `libiamf`, `eclipsa-audio-plugin`, `libear` and `obr` may. — Contamination is
  irreversible; relicensing later needs every contributor's agreement. M5 (ISO-BMFF) is the
  contamination milestone, because `gpac` is the obvious and forbidden reference there.
- **Pin the references**: pin an `iamf-tools` **v1.x tag and SHA** and a `libiamf` SHA, named in the
  code. — HEAD is a draft-v2.0.0 tree; mirroring it produces files `libiamf` rejects.
- **Pin the spec version**: `SPEC_VERSION = "1.1.0"`, named in the code. — **Decided 2026-09-08.**
  `libiamf` is a v1.1.0 decoder and the Core Value is that it accepts the file; Base-Enhanced does not
  exist in v1.0 at all. The profile enum's legal range and the expanded-layout set follow from this.
- **Error handling**: `thiserror` for errors, never `anyhow` in a library.
- **Parser hardening**: no `unwrap()`/`expect()` outside tests, **plus** `clippy::indexing_slicing` and
  `clippy::arithmetic_side_effects`. — The `unwrap` ban is necessary and nowhere near sufficient; the
  other two matter more and are the two people disable first.
- **Rust edition**: Rust 2024.
- **Fuzzing**: A fuzz target on the OBU parser ships **in M2, alongside the parser** — it may not slip
  past that milestone. It lives in an independent `fuzz/` workspace with its own lockfile, so its
  dependencies never reach Parallax's graph. — Parallax's testing strategy requires a fuzzer on every
  parser.
- **Milestone ordering**: M1 may not be reordered. — A serialiser never read by the reference decoder
  is an untested guess, however tidy the types are.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Crate named `iamf`, repo `iamf-rs` | `iamf` was free on crates.io 2026-09-07; Rust API guidelines discourage the `-rs` suffix on crate names | — Pending |
| `publish = false` in `Cargo.toml` | Publish when the library does something real, never to reserve a name | — Pending |
| Consumed by Parallax as a **path dependency** | Not a git pin. Avoids reproducibility and `cargo deny` questions until this stabilises | — Pending |
| One wire crate; native decoding in sibling `iamf-decode-rs` | The seam is now explicit: this crate owns bitstream/model/parser/encoder mechanics, while codec decoding and timed reconstruction have their own dependency and runtime boundary | ✓ Good |
| **Parser lives in the ungated core**, not behind `decode` | The expensive optional thing is codec or host integration, not the bitstream. Gating the parser breaks the round-trip test and doubles the `cfg` matrix. Side benefit: `--no-default-features` remains the minimal production and fuzz-target surface, with no linked codec or integration dependency | ✓ Good |
| Build a library, not a port of Eclipsa | Plugin architecture works around not being the host; ~1/3 is JUCE UI; encoder/decoder/muxer live elsewhere | ✓ Good |
| Runtime dependency boundary: `thiserror` only; `bitstream-io` dev-only | The linked production graph contains no bitstream helper or codec. Hand-written primitives preserve native error locations and exact reference behavior; `bitstream-io` remains only a differential test oracle | ✓ Good |
| Hand-write uleb128 | `LebGenerator` has a `kFixedSize` mode; IAMF permits non-minimal encoding and the reference uses it. The `leb128` crate emits minimal form only and cannot reproduce a reference file byte-for-byte | — Pending |
| No shipping FLAC/Opus crate needed | This crate frames caller-supplied access units and writes the small STREAMINFO/OpusHead-shaped decoder configurations with hand-written bit primitives; codec crates remain test-material tools only | ✓ Good |
| Verify against `libiamf` by **`Command`-in-tests**, not `build.rs` | A build script would force CMake + abseil + protobuf + fdk-aac onto every `cargo build`, Parallax's included, and build-script failure blocks even `cargo check`. `libiamf` also uses git submodules for its codecs | — Pending |
| Write our own ISO-BMFF muxer if ISO-BMFF is built | `gpac` is LGPL-2.1 and rejected by Parallax's `deny.toml` | — Pending |
| M1 target is LPCM, single-layer, standalone `.iamf` | Smallest thing that proves the whole chain with no codec dependency | — Pending |
| Accept **NCSA** in Parallax's licence allow-list | `libfuzzer-sys` is `(MIT OR Apache-2.0) AND NCSA`. NCSA is permissive, BSD/MIT-style, OSI-approved, no copyleft. User decision, 2026-09-07. `fuzz/` stays an independent workspace regardless — that is now a design choice, not a licence workaround | — Pending |
| Licence: **`MIT OR Apache-2.0`** (dual) | User decision 2026-09-08. MIT grants no patent licence; Apache-2.0 §3 does, which is why the Rust convention exists and why it matters for an implementation of a standard with an explicit patent pool. Both are already on Parallax's allow-list, so the consumer is unaffected either way. The marginal cost is near zero: Apache-2.0 §4(d) wants a `NOTICE` file and GUARD-07 was already adding one for BSD attribution. Dual rather than Apache-alone because it is the ecosystem default and strictly more permissive for consumers. `LICENSE-MIT` + `LICENSE-APACHE` landed 2026-09-08 | ✓ Good |
| Pin **IAMF v1.1.0** | User decision 2026-09-08. `libiamf` — whose acceptance *is* the Core Value — implements v1.1.0. Base-Enhanced, already in the handoff's own profile table, does not exist in v1.0. Building only what is scoped stays v1.1.0-clean automatically | ✓ Good |
| Parameter data taken as **pre-decimated IAMF blocks**, not curves | User decision 2026-09-08, clarified 2026-09-11. Keeps the crate mechanical: no time model or interpolation. In Parallax's IAMF-v1.1 scope Source motion is already baked into upstream Bed/HOA PCM; the submitted blocks are supported IAMF mix/demixing/recon-gain data and do not share ADM position decimation | ✓ Good |

## Open Questions

1. **What does an export of 64 moving Sources become?** A rendered bed or HOA scene with the motion baked in. That
   is a Parallax UI decision, but this crate's API should not pretend otherwise by accepting
   per-source positions it cannot express.
2. **Does `iamf-tools` contain its own ISO-BMFF muxer?** Needs confirming before committing to
   ISO-BMFF, since `gpac` is unavailable.
3. **AOM Patent License 1.0 §1.2 still unread.** No longer blocking — it was the input to the licence
   decision, which is now settled. Remains worth reading as due diligence on the inbound grant from
   AOM, which is separate from this crate's outbound licence.

### Settled

- ~~Spec version v1.0 vs v1.1.0~~ → **v1.1.0** (2026-09-08)
- ~~Licence: MIT alone or `MIT OR Apache-2.0`~~ → **`MIT OR Apache-2.0`** (2026-09-08)
- ~~Parameter tick rate: curves or pre-decimated blocks~~ → **pre-decimated blocks** (2026-09-08)

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-09-08 after settling spec version, licence and parameter tick rate*
