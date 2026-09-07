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

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

- [ ] OBU model: 5-bit type, flags, size header plus byte-aligned payload, for all shipped OBU types
- [ ] Descriptor OBUs: IA Sequence Header (31), Codec Config (0), Audio Element (1), Mix Presentation (2)
- [ ] Time-varying OBUs: Parameter Block (3), Temporal Delimiter (4), Audio Frame (5)
- [ ] Serialiser producing a standalone IA Sequence (`.iamf`) — descriptors then data
- [ ] LPCM codec framing (no external codec dependency)
- [ ] Single-layer channel-based Audio Element (recon gain and output gain absent — matches the reference)
- [ ] Profile selection, including picking the minimum profile a project fits (Simple 1/16, Base 2/18, Base-Enhanced 28/28)
- [ ] Standard and expanded layout enumeration as *labels* only (0–12 standard, 13–25 expanded, 26–30 render-only)
- [ ] Loudness metadata carried into the Mix Presentation — values supplied by the caller
- [ ] Parser: read a bitstream back into the model
- [ ] Round-trip equality: parse our own output and assert it matches the model that produced it
- [ ] Parse `iamf-tools`-produced files and assert we understand them
- [ ] Fuzz target on the OBU parser, shipping in the same milestone as the parser
- [ ] FLAC codec framing, proved by decode-and-compare
- [ ] Opus codec framing, proved by decode-and-compare
- [ ] A shaped public API for the Parallax exporter
- [ ] `cargo deny` in CI with Parallax's licence allow-list
- [ ] Byte-identity test across targets in CI from the first release

### Out of Scope

- **Any rendering, panning or spatial DSP** — Parallax owns that under decision `D-40`; it builds its
  own VBAP/LBAP/HOA/binaural renderer in `f64` over `libm`. This crate receives rendered PCM plus
  metadata and produces bytes. A PR that adds DSP here is in the wrong repository.
- **Loudness measurement** — Parallax has a BS.1770 meter sharing code with export normalisation
  (`MON-05`). This crate carries the numbers; it does not compute them.
- **UI of any kind** — this is a library.
- **AAC-LC** — the spec lists it but the reference encoder ships three codecs (LPCM, FLAC, Opus).
  Revisit only if a decoder is built and needs it.
- **Scalable layers** (BCG/DCG ladder, demixing weights, recon gain) — deferred until the single-layer
  path ships and is proven. The reference itself writes one layer.
- **An object-based element path** — the reference encoder branches on channel-based and scene-based
  only; nothing constructs an object-based element. IAMF as practised is a bed format.
- **Ambisonics projection mode** — reference has it stubbed; mono mode only. Deferred.
- **A port of Google's Eclipsa plugin suite** — considered and rejected 2026-09-07. Its architecture
  works around not being the host, ~1/3 of its ~35k lines is JUCE UI, and the encoder/decoder/muxer
  are not in that repo at all.
- **`gpac` as the ISO-BMFF muxer** — LGPL-2.1, rejected by Parallax's `deny.toml`. If ISO-BMFF is
  built, we write it ourselves.
- **`libspatialaudio`** — LGPL-2.1+, may not be read or ported.
- **Publishing to crates.io** — `publish = false` until the library does something real. Never to
  reserve a name.

## Context

**The specification overstates the implementation.** The spec page, Google's product documentation and
the marketing all describe more than the reference code does. Five claims written from prose were
disproved on 2026-09-07 by reading `iamf-tools`:

1. **No object-based element path in practice.** Two element types are constructed: channel-based and
   scene-based. "3D panning" pans a mono source into a channel bed and encodes the bed. With the
   spec's ceilings (1–2 objects per substream, 28 elements max, positions in the Mix Presentation),
   IAMF as practised is a bed format.
2. **Scalability is optional and unimplemented.** One channel layer, recon gain and output gain both
   flagged absent, under the comment *"Keeping things simple with 1 layer for now"*. A conformant file
   is a single-layer element.
3. **Three codecs shipped, not four:** LPCM, FLAC, Opus. No AAC-LC.
4. **Profiles:** Simple = 1 element / 16 channels · Base = 2 / 18 · Base-Enhanced = 28 / 28.
5. **Layouts:** standard 0–12 include HOA orders 1–3 as element layouts. Expanded 13–25 are mostly
   *subsets* of a larger layout. 26–30 are rendering-only (22.2, HOA 4–7). **9.1.6 is not a base
   layout** — it is rendered from BS.2051 9+10+3 using a named 16-of-24 speaker subset.

**Read the code, not the prose.** When a fact matters, read `iamf-tools` — it is permissively licensed
for exactly this. Mark anything taken from prose as unverified until checked. This applies especially
to the exact encoding of the OBU size field and the flag bits; the handoff explicitly declines to
assert those, because guessing a bitstream detail is how a serialiser produces files that almost work.

**The licence position is unusually good.** Both the reference encoder (`iamf-tools`, BSD/AOM) and the
reference decoder (`libiamf`, BSD-3-Clause-Clear) are permissively licensed and may be read *and*
ported with attribution. Write against the encoder, test against the decoder. Compare ADM BWF, where
the profile document is normative prose with no reference implementation to check against.

**Companion research** lives in the Parallax repo at `docs/eclipsa/` — seven files covering the
standard, the ecosystem, Google's plugin suite and the integration analysis. `06-SOURCES.md` marks
which facts are verified-from-code and which are from prose.

**Structure recap.** A bitstream is a sequence of OBUs. Types 0–2 are static descriptors, 3–5 are
time-varying, 31 is the IA Sequence Header. Two containers: a standalone IA Sequence (`.iamf`,
descriptors then data) and ISO-BMFF encapsulation (IA sample entry, configuration box holding the
descriptors, IA samples with trimming metadata). The ISO-BMFF path is what a YouTube-bound `.mp4` uses.

## Constraints

- **Determinism**: No `HashMap`/`HashSet` anywhere the output byte order can see them — use
  `BTreeMap`/`BTreeSet` or a `Vec`. No platform transcendentals in anything affecting output bytes; use
  the `libm` crate if any maths appears. — `parallax render` is byte-identical across macOS arm64,
  macOS x86_64, Windows MSVC and Linux x64, run to run and target to target. An export must be too.
- **Determinism (CI)**: A byte-identity test across targets belongs in CI from the first release. —
  Determinism that is not tested is a claim, not a property.
- **Licence allow-list**: Every transitive dependency must satisfy Parallax's `deny.toml`. Preferred:
  MIT, BSD, Apache-2.0, ISC, Zlib, Unlicense, CC0. Allowed with a recorded reason: MPL-2.0 for an
  *unmodified upstream* crate; **NCSA**, added to Parallax's allow-list by user decision 2026-09-07
  (permissive, BSD/MIT-style, no copyleft) to admit `libfuzzer-sys`. Rejected outright: GPL, LGPL,
  AGPL, SSPL, EUPL, CDDL, OSL. State each dependency's licence on the line that adds it, and run
  `cargo deny` here. — If Parallax cannot consume this crate, the crate has no purpose. A violation
  must surface here, not at integration.
- **`cargo-deny` config shape**: The `deny`, `copyleft`, `allow-osi-fsf-free`, `default` and `version`
  keys were removed from cargo-deny and now error. Rejection is expressed *by omission* from `allow`,
  not by a deny list — record that in a comment so the intent survives review. `Unicode-3.0` must be
  in the allow-list or a clean build fails (`unicode-ident`, via `syn`, via `thiserror-impl`). —
  Verified against cargo-deny 0.20.2 during research.
- **Source licence hygiene**: `libspatialaudio` (LGPL-2.1+) and `gpac` (LGPL-2.1) may not be read or
  ported. `iamf-tools`, `libiamf`, `eclipsa-audio-plugin`, `libear` and `obr` may. — Contamination is
  irreversible; relicensing later needs every contributor's agreement.
- **Error handling**: `thiserror` for errors, never `anyhow` in a library. — Library convention
  inherited from Parallax; callers need typed errors.
- **No `unwrap()`/`expect()` outside tests** — A parser meets hostile input by definition.
- **Rust edition**: Rust 2024. — Parallax convention.
- **Fuzzing**: A fuzz target on the OBU parser ships **in M2, alongside the parser** — it may not slip
  past that milestone. It cannot land earlier: M1 is encoder-only and has nothing to fuzz. — Parallax's
  testing strategy requires a fuzzer on every parser.
- **Milestone ordering**: M1 may not be reordered. — A serialiser that has never been read by the
  reference decoder is an untested guess, however tidy the types are.
- **Spec version**: Pin an IAMF specification version and name it in the code. The page was an *AOM
  Working Group Draft, 21 April 2025*; the certification programme cites **v1.0**. — Unpinned, a
  conformance claim is meaningless.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Crate named `iamf`, repo `iamf-rs` | `iamf` was free on crates.io 2026-09-07; Rust API guidelines discourage the `-rs` suffix on crate names | — Pending |
| `publish = false` in `Cargo.toml` | Privacy is that field, not a codename. Publish when the library does something real, never to reserve a name | — Pending |
| Consumed by Parallax as a **path dependency** | Not a git pin. Parallax's stack policy lists "git-only" as a downside; a sibling checkout avoids reproducibility and `cargo deny` questions until this stabilises | — Pending |
| One crate, feature-gated `encode` / `decode` | Rather than three crates up front. Split later, where the seam actually turns out to be | — Pending |
| Build a library, not a port of Eclipsa | The plugin suite's architecture works around not being the host; ~1/3 is JUCE UI; encoder/decoder/muxer live in other repos entirely | ✓ Good |
| Write our own ISO-BMFF muxer if ISO-BMFF is built | `gpac` is LGPL-2.1 and rejected by Parallax's `deny.toml` | — Pending |
| M1 target is LPCM, single-layer, standalone `.iamf` | Smallest thing that proves the whole chain with no codec dependency | — Pending |
| Accept **NCSA** in Parallax's licence allow-list | `libfuzzer-sys` is `(MIT OR Apache-2.0) AND NCSA`, which the original allow-list rejected. NCSA is permissive, BSD/MIT-style, OSI-approved, no copyleft — it fits the preferred list's spirit rather than stretching it. User decision, 2026-09-07. The `fuzz/` directory is still built as an independent workspace with its own lockfile, so the dependency never reaches Parallax's graph in any case — this makes that isolation a design choice rather than a licence workaround | — Pending |
| Licence: **unsettled** — currently MIT only | MIT grants no patent licence; Apache-2.0 does. `iamf-tools` ships the AOM Patent License 1.0 (royalty-free grant to Necessary Claims, subject to §1.2 conditions including defensive termination). For an implementation of a standard with an explicit patent pool, the Rust `MIT OR Apache-2.0` convention is worth following. Nobody has read AOM's §1.2 conditions yet. Cheapest to decide now, at one commit | ⚠️ Revisit |

## Open Questions

1. **The parameter tick rate.** IAMF parameter blocks carry their own rate, independent of the audio
   sample rate. Parallax evaluates position per sample in content time, so every export decimates a
   position curve, and the rate is audible on fast motion. **This is the one open question that blocks
   API design here** — it decides whether this crate takes curves or takes pre-decimated blocks.
   Parallax should answer it once for both IAMF and ADM BWF.
2. **Licence: MIT alone, or `MIT OR Apache-2.0`?** Requires reading the AOM Patent License 1.0 §1.2.
3. **Is a native decoder wanted at all**, or is verification better done by shelling out to `libiamf`
   during tests? M1 needs `libiamf` present either way; a native decoder is a product decision.
4. **Does the decoder ever run near the audio callback?** If an `.iamf` file is decoded for in-DAW
   playback on a streaming thread or anywhere the audio callback can see it, Parallax's allocation and
   locking rules apply to that path and the API must be shaped for it from the start. Decide before
   writing the decoder, not after. Encoding is offline, so real-time rules otherwise do not apply.
5. **What does an export of 64 moving Sources become?** Per the corrections above it is a rendered
   7.1.4 or 9.1.6 bed with the motion baked in. That is a Parallax UI decision, but this crate's API
   should not pretend otherwise by accepting per-source positions it cannot express.

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
*Last updated: 2026-09-07 after initialization*
