# iamf-rs — handoff

**Written 2026-09-07.** Everything needed to start this crate, and everything that must be observed
while doing it. Read this before the first line of code.

Companion research lives in the Parallax repo at `docs/eclipsa/` — seven files covering the standard,
the ecosystem, Google's reference plugin suite and the integration analysis. This document is the
build brief; that directory is the evidence behind it.

---

## 1. What this is, and what it is not

**A Rust implementation of IAMF — the Immersive Audio Model and Formats bitstream.** An OBU
serialiser and parser, the descriptor model, an encoder producing conformant `.iamf` files, and later
a decoder.

**It is not a renderer.** Parallax builds its own VBAP/LBAP/HOA/binaural renderer under decision
`D-40`, from the papers, in `f64` over `libm`, deterministic and real-time-safe. This crate never
pans, never places a source, never touches a speaker layout as anything but a *label*. It receives
rendered PCM plus metadata and produces bytes. **If a pull request adds DSP here, it is in the wrong
repository.**

**It is not a port of Google's Eclipsa plugins.** That was considered and rejected on 2026-09-07: the
plugin suite's architecture is a workaround for not being the host (two plugins synchronising over
shared memory to reconstruct a scene the DAW will not show them), roughly a third of its ~35 000 lines
is JUCE UI, and the three things actually needed — encoder, decoder, muxer — are not in that
repository at all. They are `iamf-tools`, `libiamf` and `gpac`, separate projects.

**Why it exists.** Parallax needs an IAMF exporter because IAMF is now an Output path for monitoring
and playback *and* an export format (user decision, 2026-09-07). There is **no Rust IAMF
implementation on crates.io** — verified 2026-09-07. A bitstream serialiser has nothing to do with an
audio graph and is separately fuzzable, so it belongs in its own crate whether or not it is ever
published.

---

## 2. Decisions already taken

| Decision | Note |
|---|---|
| Crate name **`iamf`**, repo `iamf-rs` | `iamf` was free on crates.io on 2026-09-07. `-rs` belongs on the repo, not the crate — the Rust API guidelines discourage the suffix |
| **`publish = false`** in `Cargo.toml` | Privacy is that field, not a codename. Publish when the library does something real, never to reserve a name |
| Consumed by Parallax as a **path dependency** during development | Not a git pin. Parallax's own stack policy lists "git-only" as a downside; a sibling checkout avoids the reproducibility and `cargo deny` questions until this stabilises |
| **One crate, feature-gated** | `encode` / `decode` features rather than three crates up front. Split later, where the seam actually turns out to be |

**Still open: the licence.** The repo currently carries **MIT only**. That is on Parallax's preferred
list and works — but see §3.3 before leaving it there. The decision is cheapest now, at one commit;
relicensing later needs every contributor's agreement.

---

## 3. Licence — the part that constrains the work

### 3.1 What may be read and ported

Verified 2026-09-07 by reading the licence files, not the badges.

| Source | Licence | Read? | Port? |
|---|---|---|---|
| **`AOMediaCodec/iamf-tools`** — the reference encoder | BSD (AOM) | **yes** | **yes**, with attribution |
| **`AOMediaCodec/libiamf`** — the reference decoder | BSD-3-Clause-**Clear** | **yes** | **yes**, with attribution |
| `google/eclipsa-audio-plugin` | Apache-2.0 | yes | yes |
| `ebu/libear` | Apache-2.0 | yes | yes |
| `google/obr` | BSD-3-Clause | yes | yes |
| **`libspatialaudio`** | **LGPL-2.1+** | **no** | **no** |
| **`gpac`** | **LGPL-2.1** | **no** | **no** |
| The IAMF specification itself | AOM, royalty-free | yes | it is a spec — implement it |

This is an unusually good position: **the reference encoder and the reference decoder are both
permissively licensed.** Write against `iamf-tools`, test against `libiamf`. Compare that with ADM
BWF, where the profile document is normative prose and there is no reference implementation to check
against.

**BSD-3-Clause-Clear matters.** The "Clear" variant explicitly withholds patent rights. It is a
copyright licence and nothing more — which is why the AOM patent grant is a separate document. See
§3.3.

### 3.2 Dependencies inherit Parallax's policy

If Parallax consumes this crate, **every transitive dependency must satisfy Parallax's `deny.toml`**,
or the crate cannot be used at all.

- **Preferred:** MIT, BSD, Apache-2.0, ISC, Zlib, Unlicense, CC0
- **Allowed with a recorded reason:** MPL-2.0, for an *unmodified upstream* crate
- **Rejected outright:** GPL, LGPL, AGPL, SSPL, EUPL, CDDL, OSL

State each dependency's licence on the line that adds it. Run `cargo deny` here too, with the same
allow-list, so a violation surfaces in this repo rather than at the Parallax integration.

This is why the muxer cannot be `gpac` and the codec bindings need care.

### 3.3 The AOM Patent License 1.0 — read it before settling the licence

`iamf-tools` ships a `PATENTS` file carrying the **Alliance for Open Media Patent License 1.0**: a
royalty-free, worldwide, irrevocable patent grant to Necessary Claims for making, using and
distributing an *Implementation* — **subject to conditions in §1.2**, including the defensive
termination that these licences normally carry.

Two consequences worth deciding deliberately:

1. **MIT grants no patent licence.** Apache-2.0 does. The Rust convention of dual `MIT OR Apache-2.0`
   exists largely for this. For an implementation of a standard with an explicit patent pool, that
   convention is worth following rather than defaulting to MIT alone.
2. **Read AOM's conditions before relying on the grant.** This handoff does not interpret them; it
   only records that they exist and that nobody has read them yet.

---

## 4. What the standard actually is — verified facts

The specification page and Google's product documentation both **overstate what is implemented**.
Five claims written from prose were disproved by reading the reference implementation on 2026-09-07.
Trust this section over any summary, and verify anything not listed here against the code.

### 4.1 The five corrections

1. **There is no object-based element path in practice.** The reference encoder branches on exactly
   two element types — channel-based and scene-based — and nothing constructs an object-based
   element. Its "3D panning" pans a mono source into a channel bed and encodes the bed. Combined with
   the spec's ceilings (1–2 objects per substream, 28 elements maximum, positions living in the Mix
   Presentation), **IAMF as practised is a bed format.**
2. **Scalability is optional and unimplemented.** The reference writes one channel layer with recon
   gain and output gain both flagged absent, under the comment *"Keeping things simple with 1 layer
   for now"*; ambisonics is mono mode only, projection stubbed. **A conformant file is a single-layer
   element.** The BCG/DCG ladder, demixing weights and recon gain are a later quality feature, not an
   entry fee.
3. **Three codecs shipped, not four:** LPCM, FLAC, Opus. **No AAC-LC**, though the spec lists it. A
   decoder needs all four; an encoder evidently does not.
4. **Profiles:** Simple = 1 element / 16 channels · Base = 2 / 18 · Base-Enhanced = 28 / 28.
5. **Layouts** (from the reference's own enumeration): standard 0–12 are Mono, Stereo, 5.1, 5.1.2,
   5.1.4, 7.1, 7.1.2, 7.1.4, 3.1.2, Binaural, **and HOA orders 1–3 as element layouts**. Expanded
   13–25 are mostly *subsets* of a larger layout — LFE-only, 5.1.4-Surround, seven 7.1.4 subsets, 9.1.6
   and four of its subsets. 26–30 are rendering-only: 22.2 and HOA 4–7. **9.1.6 is not a base layout**:
   it is rendered from BS.2051 9+10+3 using a named 16-of-24 speaker subset.

### 4.2 Structure

A bitstream is a sequence of **OBUs**, each a header — 5-bit type, flags, size — plus a byte-aligned
payload. Types 0–2 are static descriptors (Codec Config, Audio Element, Mix Presentation), 3–5 are
time-varying (Parameter Block, Temporal Delimiter, Audio Frame), 31 is the IA Sequence Header.

**Verify before implementing:** the exact encoding of the size field and of the flag bits. This
handoff does not assert it — the summarised spec read did not establish it, and guessing a bitstream
detail is how a serialiser produces files that almost work. Read `iamf-tools`, which is licensed for
exactly this.

Containers: a **standalone IA Sequence** (`.iamf` — descriptors then data) and **ISO-BMFF**
encapsulation (an IA sample entry, a configuration box holding the descriptors, IA samples with
trimming metadata). The ISO-BMFF path is what a YouTube-bound `.mp4` uses.

---

## 5. Scope: in and out

**In**

- OBU model, serialiser and parser
- Descriptors: IA Sequence Header, Codec Config, Audio Element, Mix Presentation
- Time-varying data: Audio Frame, Parameter Block, Temporal Delimiter
- Profile selection, including picking the minimum profile a project fits
- Loudness metadata carried into the Mix Presentation (the *values* come from the caller)
- Standalone `.iamf` first; ISO-BMFF later, **written by us** — `gpac` is LGPL and unusable
- Codec framing for LPCM, FLAC, Opus

**Out**

- **Any rendering, panning or spatial DSP.** Parallax owns that (`D-40`)
- **Loudness measurement.** Parallax has a BS.1770 meter that shares its code with export
  normalisation (`MON-05`); this crate carries the numbers, it does not compute them
- **UI of any kind**
- **AAC-LC**, unless a decoder is built and needs it
- **Scalable layers**, until the single-layer path ships and is proven

---

## 6. Constraints inherited from Parallax

This crate is not in the Parallax workspace, but Parallax consumes it, so some of its rules propagate.

**Determinism applies.** `parallax render` is byte-identical across macOS arm64, macOS x86_64,
Windows MSVC and Linux x64, run to run and target to target. An export must be too. Concretely:

- **No hashed containers** anywhere the output order can see them — no `HashMap`/`HashSet` in
  anything that reaches the byte stream. `BTreeMap`/`BTreeSet` or a `Vec`.
- **No platform transcendentals** in anything affecting output bytes. There is little maths here, but
  if any appears, use the `libm` crate.
- A byte-identity test across targets belongs in CI from the first release.

**Real-time rules mostly do not apply** — encoding is offline. **One thing to decide early:** if the
decoder is ever used for in-DAW playback of an `.iamf` file, does it run on a streaming thread or
anywhere the audio callback can see it? If the latter, the allocation and locking rules apply to that
path and the API must be shaped for it from the start. Decide before writing the decoder, not after.

**Library conventions:** `thiserror` for errors, never `anyhow` in a library. **No `unwrap()` or
`expect()` outside tests** — a parser meets hostile input by definition. Rust 2024.

**Fuzzing is not optional.** Parallax's testing strategy requires a fuzzer on every parser, and the
OBU parser is a parser. **The fuzz target ships in the same milestone as the parser itself — M2 — and
may not slip past it.** It cannot come earlier: M1 is encoder-only and has nothing to fuzz.

---

## 7. Milestones

**M1 — a file `libiamf` can decode.** The smallest thing that proves the whole chain: a single-layer
channel-based Audio Element, **LPCM** (no codec dependency at all), one Mix Presentation, standalone
`.iamf`. Success is not "it compiles" — it is **the reference decoder reads it and the PCM comes back
sample-identical**.

**M2 — the parser and the round trip.** Parse our own output back into the model and assert equality;
parse files produced by `iamf-tools` and assert we understand them. **The fuzz target lands here, with
the parser** (§6) — this is the first milestone that has one.

**M3 — the codecs.** FLAC and Opus framing, each proved by the same decode-and-compare loop as M1.

**M4 — what Parallax actually calls.** A shaped API for the exporter, and the loudness metadata path.

**M5 and beyond, in no fixed order:** ISO-BMFF; the decoder; scalable layers with demixing and recon
gain; ambisonics projection mode.

Do not reorder M1. A serialiser that has never been read by the reference decoder is an untested
guess, however tidy the types are.

---

## 8. Open questions

1. **The parameter tick rate.** IAMF parameter blocks carry their own rate, independent of the audio
   sample rate. Parallax evaluates position per sample in content time, so **every export decimates a
   position curve**, and the rate is audible on fast motion. This is the same question ADM BWF asks;
   Parallax should answer it once for both. **It is the one open question that blocks API design here**
   — it decides whether this crate takes curves or takes pre-decimated blocks.
2. **Licence: MIT alone, or `MIT OR Apache-2.0`?** See §3.3.
3. **Is the decoder wanted at all**, or is verification better done by shelling out to `libiamf`
   during tests? M1 needs `libiamf` present either way; a native decoder is a product decision.
4. **What does an export of 64 moving Sources become?** Per §4.1 it is a rendered 7.1.4 or 9.1.6 bed,
   and the motion is baked. That is a Parallax UI decision, but this crate's API should not pretend
   otherwise by accepting per-source positions it cannot express.

---

## 9. Reference material

| What | Where | Licence |
|---|---|---|
| The research behind this document | Parallax repo, `docs/eclipsa/` (7 files) | ours |
| IAMF specification | `aomediacodec.github.io/iamf/` — was an *AOM Working Group Draft, 21 April 2025*; the certification programme cites **v1.0**. **Pin a version and name it in the code** | AOM |
| Reference encoder | `AOMediaCodec/iamf-tools` | BSD — read and port |
| Reference decoder | `AOMediaCodec/libiamf` | BSD-3-Clause-Clear — read and port |
| Google's plugin suite, for behaviour | `google/eclipsa-audio-plugin` | Apache-2.0 |
| AOM Patent License 1.0 | the `PATENTS` file beside `iamf-tools` | read before settling §3.3 |

**A caution that cost this project real corrections.** The specification page, the product
documentation and the marketing all describe more than the implementation does. When a fact matters,
read the code — it is licensed for that. Mark anything taken from prose as unverified until it is
checked.

---

## 10. Provenance

Compiled from a Parallax session on 2026-09-07 in which the Eclipsa plugin source was cloned and read
under its Apache-2.0 licence, five documented claims were disproved, and the port-versus-library
question was decided. Facts labelled *verified* here were read from that source; everything else is
from the specification page, the product documentation or `eclipsamedia.org`, and is marked as such in
`docs/eclipsa/06-SOURCES.md`.

Nothing in this document was taken from an LGPL or GPL source.
