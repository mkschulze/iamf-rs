# iamf-rs

## What This Is

A Rust implementation of **IAMF** — the Alliance for Open Media's Immersive Audio Model and Formats
bitstream. It is an OBU serialiser and parser, the descriptor model, an encoder that produces
conformant `.iamf` files, and later a decoder. It exists because there is **no Rust IAMF
implementation on crates.io** (verified 2026-09-07) and Parallax — a deterministic spatial-audio DAW —
needs IAMF as both a monitoring/playback output path and an export format.

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

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

- [ ] Bit-level I/O layer (read/write non-byte-aligned fields, uleb128 both minimal and fixed-size)
- [ ] OBU header: `[obu_type:5][obu_redundant_copy:1][type_specific_flag:1][obu_extension_flag:1]` plus uleb128 `obu_size`, conditional trim fields and extension header
- [ ] Descriptor OBUs: IA Sequence Header (31), Codec Config (0), Audio Element (1), Mix Presentation (2)
- [ ] Time-varying OBUs: Parameter Block (3), Temporal Delimiter (4), Audio Frame (5)
- [ ] Serialiser producing a standalone IA Sequence (`.iamf`) — descriptors then data
- [ ] LPCM codec framing (no external codec dependency)
- [ ] Single-layer channel-based Audio Element, with correct channel→substream BCG packing (coupled pairs first, then mono)
- [ ] Profile selection, including picking the minimum profile a project fits
- [ ] Layout modelling as **four distinct types** across two OBUs — not one flat enum (see Context)
- [ ] Loudness metadata carried into the Mix Presentation — values supplied by the caller
- [ ] Mandatory mix-gain param definitions, including the `param_definition_mode = 1` / `default_mix_gain = 0` path that emits zero Parameter Block OBUs
- [ ] Every sub-mix contains a stereo layout (hard encoder check in the reference)
- [ ] Parser: read a bitstream back into the model, with descriptor-context threading for Parameter Blocks
- [ ] Round-trip: `parse(serialize(model)) == model` universally; `serialize(parse(bytes)) == bytes` for our own output only
- [ ] Preserve unknown data: OBU footer bytes, unknown OBU types at correct byte offset, unknown parameter data
- [ ] Parse `iamf-tools`-produced files and assert we understand them
- [ ] Fuzz target on the OBU parser, in an independent `fuzz/` workspace, shipping with the parser
- [ ] FLAC codec framing (STREAMINFO-shaped `decoder_config`), proved by decode-and-compare
- [ ] Opus codec framing (OpusHead-shaped `decoder_config`), proved by decode-and-compare
- [ ] A shaped public API for the Parallax exporter
- [ ] `cargo deny` in CI with Parallax's licence allow-list
- [ ] Pinned `SPEC_VERSION` and pinned reference-implementation SHA, both named in the code
- [ ] Byte-identity across targets, as a committed golden fixture every target must reproduce
- [ ] Clippy hardening: `indexing_slicing`, `arithmetic_side_effects`, no `unwrap`/`expect` outside tests
- [ ] A no-DSP guard (grep/lint) enforcing the scope boundary

### Out of Scope

- **Any rendering, panning or spatial DSP** — Parallax owns that under decision `D-40`; it builds its
  own VBAP/LBAP/HOA/binaural renderer in `f64` over `libm`. This crate receives rendered PCM plus
  metadata and produces bytes. A PR that adds DSP here is in the wrong repository.
- **Loudness measurement** — Parallax has a BS.1770 meter sharing code with export normalisation
  (`MON-05`). This crate carries the numbers; it does not compute them.
- **UI of any kind** — this is a library.
- **AAC-LC** — deferred, not absent from the ecosystem. *Correction 2026-09-08:* `iamf-tools` `main`
  now ships an AAC-LC encoder, so the handoff's "three codecs shipped, not four" describes the v1.x
  tree, not HEAD. Still out of scope for v1 here; revisit if a decoder needs it.
- **Scalable layers** (BCG/DCG ladder, demixing weights, recon gain) — deferred until the single-layer
  path ships and is proven. *Correction 2026-09-08:* `iamf-tools` implements multi-layer and recon gain
  **fully**; the handoff's claim that it was unimplemented came from Eclipsa, not from `iamf-tools`.
  This remains a scope decision, but it is ours, not a limit of the reference.
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
- **Pin the spec version**: a `SPEC_VERSION` constant named in the code. — See Open Questions #2.
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
| One crate, feature-gated `encode` / `decode` | Split later, where the seam actually turns out to be | — Pending |
| **Parser lives in the ungated core**, not behind `decode` | The expensive optional thing is libFLAC/libopus, not the bitstream. Gating the parser breaks the round-trip test and doubles the `cfg` matrix. Side benefit: `--no-default-features` becomes a zero-dependency build, the ideal fuzz-target dependency | — Pending |
| Build a library, not a port of Eclipsa | Plugin architecture works around not being the host; ~1/3 is JUCE UI; encoder/decoder/muxer live elsewhere | ✓ Good |
| Runtime deps: `bitstream-io` + `thiserror` only | 9-crate transitive graph, satisfiable by `allow = ["MIT", "Unicode-3.0"]`. Hand-written recursive descent mirrors the reference's 22-function primitive surface — a derive macro costs auditability and buys nothing | — Pending |
| Hand-write uleb128 | `LebGenerator` has a `kFixedSize` mode; IAMF permits non-minimal encoding and the reference uses it. The `leb128` crate emits minimal form only and cannot reproduce a reference file byte-for-byte | — Pending |
| No FLAC/Opus crate needed | The `decoder_config` payloads are STREAMINFO fields and OpusHead fields — roughly 60 lines each over `bitstream-io` | — Pending |
| Verify against `libiamf` by **`Command`-in-tests**, not `build.rs` | A build script would force CMake + abseil + protobuf + fdk-aac onto every `cargo build`, Parallax's included, and build-script failure blocks even `cargo check`. `libiamf` also uses git submodules for its codecs | — Pending |
| Write our own ISO-BMFF muxer if ISO-BMFF is built | `gpac` is LGPL-2.1 and rejected by Parallax's `deny.toml` | — Pending |
| M1 target is LPCM, single-layer, standalone `.iamf` | Smallest thing that proves the whole chain with no codec dependency | — Pending |
| Accept **NCSA** in Parallax's licence allow-list | `libfuzzer-sys` is `(MIT OR Apache-2.0) AND NCSA`. NCSA is permissive, BSD/MIT-style, OSI-approved, no copyleft. User decision, 2026-09-07. `fuzz/` stays an independent workspace regardless — that is now a design choice, not a licence workaround | — Pending |
| Licence: **unsettled** — currently MIT only | MIT grants no patent licence; Apache-2.0 does. `iamf-tools` ships the AOM Patent License 1.0, subject to §1.2 conditions including defensive termination. For an implementation of a standard with an explicit patent pool, the Rust `MIT OR Apache-2.0` convention is worth following. Nobody has read §1.2 yet. Cheapest to decide now, at one commit | ⚠️ Revisit |

## Open Questions

1. **The parameter tick rate — curves or pre-decimated blocks?** **Still the one open question that
   blocks API design.** New evidence: `parameter_rate` lives in `param_definition` inside the
   *descriptors*, which are written before any audio — so the tick rate is a `build()`-time input
   either way, never per-block. `iamf-tools` has two open TODOs under one bug ID admitting it has never
   implemented `parameter_rate != sample_rate`, and every golden vector uses rate == sample rate.
   Research recommends **taking pre-decimated blocks** (flagged as a recommendation, not a finding).
   Parallax should answer this once for both IAMF and ADM BWF.
2. **Spec version: v1.0 or v1.1.0?** Certification cites v1.0; `libiamf`'s README says it decodes
   v1.1.0 and `iamf-tools` comments cite v1.1.0. Base-Enhanced does not exist in v1.0 at all, so the
   handoff's own profile table is incompatible with a v1.0 pin. **Decide in M1** — it sets
   `SPEC_VERSION` and the reference tag together.
3. **Licence: MIT alone, or `MIT OR Apache-2.0`?** Requires reading AOM Patent License 1.0 §1.2.
4. **Is a native decoder wanted at all**, or is verification better done by shelling out to `libiamf`
   during tests? M1 needs `libiamf` present either way; a native decoder is a product decision.
5. **Does the decoder ever run near the audio callback?** If an `.iamf` file is decoded for in-DAW
   playback on a streaming thread, Parallax's allocation and locking rules apply to that path and the
   API must be shaped for it from the start. Decide before writing the decoder, not after.
6. **What does an export of 64 moving Sources become?** A rendered bed with the motion baked in. That
   is a Parallax UI decision, but this crate's API should not pretend otherwise by accepting
   per-source positions it cannot express.
7. **Does `iamf-tools` contain its own ISO-BMFF muxer?** Needs confirming before committing to M5,
   since `gpac` is unavailable.

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
*Last updated: 2026-09-08 after project research corrected five handoff claims*
