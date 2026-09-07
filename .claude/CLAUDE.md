<!-- GSD:project-start source:PROJECT.md -->

## Project

**iamf-rs**

A Rust implementation of **IAMF** — the Alliance for Open Media's Immersive Audio Model and Formats
bitstream. It is an OBU serialiser and parser, the descriptor model, an encoder that produces
conformant `.iamf` files, and later a decoder. It exists because there is **no Rust IAMF
implementation on crates.io** (verified 2026-09-07) and Parallax — a deterministic spatial-audio DAW —
needs IAMF as both a monitoring/playback output path and an export format.

The crate is `iamf`; the repo is `iamf-rs`. Parallax consumes it as a path dependency during
development.

**Core Value:** **A `.iamf` file this crate writes is read back by the reference decoder `libiamf` with the PCM
sample-identical.** Everything else — API shape, codec coverage, containers, the decoder — is
secondary to producing bytes the reference implementation accepts.

Sharpened by research (2026-09-08): `libiamf` is a *permissive* reader — it ignores reserved-bit
misuse and clamps an overlong leb128 rather than erroring. **Passing the `libiamf` gate is necessary
but not sufficient for conformance.** The real exit criterion is a byte-diff against an
`iamf-tools`-produced file that is either identical or fully explained in writing.

### Constraints

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
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->

## Technology Stack

## Executive recommendation

## Recommended Stack

### Core Technologies

| Technology | Version | Licence (verified) | Purpose | Why Recommended |
|------------|---------|--------------------|---------|-----------------|
| **Rust** | 1.85+ toolchain, **edition 2024** | — | Language | Handoff constraint. Edition 2024 requires 1.85; local toolchain is 1.92.0. Every recommended crate's MSRV is ≤ 1.85 except dev tools noted below. Pin with `rust-toolchain.toml` so the byte-identity matrix compiles identically everywhere. |
| **`bitstream-io`** | **4.10.0** | `MIT/Apache-2.0` — slash form in `Cargo.toml`; cargo-deny 0.20.2 resolves it as `OR` (verified). `LICENSE-MIT` + `LICENSE-APACHE` both present in repo | Bit-level reader/writer | The only mature Rust crate that is a *bit stream codec* rather than a bit *collection*. 50.6M downloads. MSRV 1.83. Exactly one transitive dep. Ships `BitsWritten`, which computes serialised length without emitting bytes — that is the two-pass `obu_size` problem solved for free. Handles the 20/24/36-bit FLAC STREAMINFO fields and the reference's 9-bit signed reads natively. |
| **`thiserror`** | **2.0.20** | `MIT OR Apache-2.0` | Typed library errors | Handoff constraint (never `anyhow` in a library). 1.42B downloads; `dtolnay`-maintained; v2 is the current major. |

### Supporting Libraries

| Library | Version | Licence (verified) | Purpose | When to Use |
|---------|---------|--------------------|---------|-------------|
| `arbitrary` (+`derive`) | **1.4.2** | `MIT OR Apache-2.0` | Structured fuzz input | **M2.** Dev-dependency behind `#[cfg(any(test, fuzzing))]`, plus a dependency of the `fuzz/` crate. Derives `Arbitrary` on the OBU model so the round-trip fuzz target generates valid-ish models rather than noise. |
| `proptest` | **1.11.0** | `MIT OR Apache-2.0` | In-tree property tests | **M2.** Round-trip `model → bytes → model` properties that run under plain `cargo test` on all four CI targets, unlike libFuzzer which needs nightly + sanitizers. MSRV 1.85 — matches ours exactly. |
| `hex-literal` | **1.1.0** | `MIT OR Apache-2.0` | Byte-array literals in tests | **M1.** Golden-vector tests read far better as `hex!("f8 05 ...")` than as `vec![0xf8, 0x05, ...]`. Edition 2024, MSRV 1.85. |
| `pretty_assertions` | **1.4.1** | `MIT OR Apache-2.0` | Readable diffs | Optional. A failing byte-identity assertion on a 2 MB buffer is unreadable without it. Consider `similar-asserts 2.0.0` (Apache-2.0) instead if you want a unified diff. |
| `libm` | **0.2.16** | `MIT` (crates.io metadata for 0.2.16; now published from `rust-lang/compiler-builtins`) | Deterministic transcendentals | **Do not add on day one.** See "Determinism" below — the IAMF bitstream has **no floating-point fields**. Add only when a dB→Q7.8 helper or Bézier parameter interpolation appears. |
| `flacenc` | **0.5.1** | `Apache-2.0` | FLAC encoder for M3 decode-and-compare | **M3, dev-dependency only.** Pure Rust — no C toolchain, no `-sys` crate. |
| `claxon` | **0.4.3** | `Apache-2.0` | FLAC decoder for M3 decode-and-compare | **M3, dev-dependency only.** Pure Rust. Caveat: last release 2020, no declared MSRV/edition. If it fails to build on edition 2024, fall back to `symphonia-bundle-flac 0.6.1` (**MPL-2.0** — permitted only as unmodified upstream, and it *is* unmodified upstream, but record the reason). |

### Development Tools

| Tool | Version | Licence (verified) | Purpose | Notes |
|------|---------|--------------------|---------|-------|
| `cargo-deny` | **0.20.2** | `MIT OR Apache-2.0` | Licence / advisory / source gate | MSRV 1.88 for the *tool*; irrelevant to the crate's MSRV. See the drop-in `deny.toml` below — **already executed and passing** against the full proposed dependency set. |
| `cargo-fuzz` | **0.13.2** | `MIT OR Apache-2.0` | libFuzzer driver | **M2.** Needs nightly, a C++11 compiler, and LLVM sanitizer support: x86_64 Linux, x86_64 + aarch64 macOS, Windows via MSVC ASan. |
| `libfuzzer-sys` | **0.4.13** | ⚠️ `(MIT OR Apache-2.0) AND NCSA` | libFuzzer bindings | **This licence is NOT on your allow-list and cargo-deny rejects it (verified).** NCSA is the LLVM licence — OSI-approved and FSF Free/Libre, permissive, no copyleft. Resolution below; it does not require widening the allow-list. |
| `clippy` | bundled 1.92.0 | — | The no-panic / no-hash enforcement mechanism | The lint set below was executed and every lint fired as intended. |
| CMake ≥ 3.28, C++20 compiler | — | — | Building `libiamf` / `iamf-tools` for reference tests | CI-only, never a build dependency of the crate. |

## 1. Bit-level I/O — the decision, and the evidence

### What the format actually demands

| Fact | Source | Consequence for the stack |
|---|---|---|
| The complete primitive set is `ReadUnsignedLiteral(n≤64)`, `ReadSigned8/9/16`, `ReadBoolean`, `ReadString`, `ReadULeb128` (with and without a byte-count out-param), `ReadIso14496_1Expanded`, `ReadUint8Span` — and a symmetric write side | `read_bit_buffer.h:51–205`, `write_bit_buffer.h:51–128` | ~22 functions total. Any framework big enough to "help" is bigger than the thing it helps with. |
| **`ReadSigned9` / `WriteSigned9`** — a 9-bit two's-complement field | `read_bit_buffer.h:123`, `write_bit_buffer.h:79` | Rules out byte-oriented crates. `bitstream-io` does this as `read_signed_var::<i16>(9)`. |
| ULEB128 is capped at **8 bytes** (`kMaxLeb128Size = 8`) and decodes to **`uint32_t`** (`DecodedUleb128`) | `iamf/obu/types.h` | A hostile 10-byte continuation chain must be a typed error, not a `u64` overflow. Off-the-shelf LEB readers do not enforce this cap. |
| `LebGenerator` has **two modes: `kMinimum` and `kFixedSize`** (fixed size 1–8 bytes) | `iamf/common/leb_generator.h` | **Decisive.** IAMF permits *non-minimal* ULEB128, and the reference encoder uses it (fixed-size mode lets it write the header before knowing the payload size). The `leb128` crate (gimli, `MIT OR Apache-2.0`) only *emits* minimal form — it cannot reproduce a reference file byte-for-byte. Write your own; it is ~30 lines each way. |
| Whole OBU ≤ 2 MB (`kEntireObuSizeMaxTwoMegabytes = 1 << 21`); strings ≤ 128 bytes incl. NUL | `iamf/obu/types.h` | Gives the fuzz target and the parser hard, cheap allocation ceilings. A parser that allocates on a length field without checking these is the classic OOM fuzz finding. |
| The OBU **header is bit-packed; the payload is byte-aligned** | `obu_header.h`, §4.2 of the handoff | You only need bit reads inside headers and descriptor payloads. Audio-frame payloads are byte slices — take them by offset, never bit-by-bit. |

### Verdict

### Alternatives — and why each is rejected

| Candidate | Version / Licence (verified) | Verdict |
|---|---|---|
| **Hand-rolled `BitCursor` over `&[u8]`** | — | **The credible runner-up.** ~150 lines, zero deps, perfect error types, zero-copy. Rejected only because `BitsWritten` and the const-checked reads are real value and `bitstream-io`'s bit-packing has 50M downloads of exercise behind it. If the `io::Error` boundary turns out to be friction in practice, this is the fallback and the migration is local to the wrapper. |
| `bitvec` 1.1.1 | `MIT` | Rejected. It is a bit-*collection* (`BitSlice`, `BitVec`) with heavy generics, not a stream codec. Notorious compile times, and there is no positional-cursor API — you would build the streaming layer yourself on top anyway. |
| `deku` 0.20.3 | `MIT OR Apache-2.0` (+ optional `bitvec` MIT) | Rejected — and this is the closest call, since `deku` is explicitly a *bit-level* derive macro. Three reasons: (a) the byte layout must be **auditable against C++**, and `#[deku(bits = 5)]` attributes scattered over a struct do not read like `write_bit_buffer.cc`; (b) ULEB128 with a fixed-size generation mode and flag-dependent conditional fields both need `ctx`/custom reader-writer escape hatches, and once half the fields are escape hatches the macro has stopped paying rent; (c) MSRV is **1.88** on `master` (crates.io metadata for 0.20.3 says 1.82 — the two disagree; treat 1.88 as binding), which is above our 1.85 floor. |
| `binrw` 0.15.2 | `MIT` | Rejected. Excellent crate, but byte-oriented at heart — bitfields go through `#[br(map)]` and custom parsers. Wrong shape for a 5-bit-type header and a 9-bit signed field. |
| `modular-bitfield` 0.13.1 | `MIT OR Apache-2.0` | Rejected. Fixed-layout structs only; cannot express variable-length or conditional fields. |
| `bitter` 0.9.1 | `MIT` | Rejected. Read-only (no writer) and only 335K downloads. Fast, but you need both directions. |
| `bitreader` 0.3.11 | `MIT OR Apache-2.0` | Rejected. Read-only; last touched 2024. |
| `leb128` 0.2.7 | `MIT OR Apache-2.0` | Rejected as a dependency. Cannot emit non-minimal/fixed-size ULEB128, which the reference requires, and does not enforce IAMF's 8-byte / `u32` caps. Read `leb_generator.cc` and write it. |

## 2. Parser architecture — hand-written, over `bitstream-io`

| Architecture | Fit for IAMF | Verdict |
|---|---|---|
| **Hand-written** | The reference is itself hand-written recursive descent (`ObuHeader::ReadAndValidate`, `FlacDecoderConfig::ReadAndValidate`, …). A Rust function that mirrors it 1:1 can be reviewed side-by-side against the C++ by someone who knows neither codebase well. Read and write live adjacent in the same module, which is how you catch asymmetry — the failure mode that produces "files that almost work". | ✅ **Chosen** |
| Combinators (`winnow` 1.0.4 `MIT`, `nom` 8.0.0 `MIT`) | Combinators are for *grammars*. IAMF is a length-prefixed record format with flag-dependent optional fields and forward references (`num_samples_per_frame` from the Codec Config governs how the Opus config validates). Threading that context through combinators is `winnow`'s `Stateful` — more machinery than a plain function. `nom`'s `bits` combinators are notoriously awkward. Neither produces a *writer*, so the serialiser would be hand-written anyway and the two halves would stop resembling each other. | ❌ Reject. Revisit `winnow` only if a genuinely **streaming/incremental** parser is ever needed (partial-input resumption for the in-DAW playback path in Open Question 4) — that is winnow's real edge, and `nom` 8.0 has had no release since Jan 2025 while `winnow` 1.0.4 shipped July 2026. |
| Derive macros (`deku`, `binrw`) | Optimise for *writing* a parser fast. This project optimises for *proving* a parser correct against a reference. A macro moves the byte layout into attribute syntax and the control flow into generated code, which is precisely the thing that must stay reviewable. | ❌ Reject |

### The shape to build

- **Every `read`/`write` pair in the same file, in that order.** Asymmetry becomes visually obvious.
- **Every function carries `// ref: iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite`.** This is the

## 3. Error handling — `thiserror` 2.0.20, and the lints that enforce it

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]

- **`#[non_exhaustive]`** on the enum. You will add variants every milestone; without it each one is a
- **Carry `offset` on every variant.** A fuzz crash report with a byte offset is a bug you fix in ten
- **Derive `Clone + PartialEq + Eq`.** Round-trip and property tests want to assert on the error, and
- **Keep the enum small.** Add `const _: () = assert!(size_of::<Error>() <= 32);` — `Result<T, Error>`
- **`#[from]` sparingly.** Blanket `From` conversions lose the offset. Convert at the call site.
- **Never `panic!` on malformed input, only on broken *invariants*** — and prefer to have no such

### Enforcement, not convention — verified working

## 4. Fuzzing — `cargo-fuzz`, in an **excluded** `fuzz/` workspace

### The licence problem, and its resolution

### Tool choice

| Option | Version / Licence | Verdict |
|---|---|---|
| **`cargo-fuzz` + `libfuzzer-sys`** | 0.13.2 `MIT OR Apache-2.0` / 0.4.13 `(MIT OR Apache-2.0) AND NCSA` | ✅ **Chosen.** The default in the Rust ecosystem. In-process, fast, coverage-guided, integrates with ASan/UBSan. Now supports Windows via MSVC AddressSanitizer, plus x86_64 Linux and both macOS arches — matching your four CI targets except Windows-on-Arm. |
| `afl.rs` | 0.18.2 `Apache-2.0` | ❌ Reject as primary. Licence is clean and it finds different bugs, but it requires building AFL++ from source and is fork-based (slower per exec for a microsecond-scale parser). Consider as a *second* fuzzer in M5+ if the corpus plateaus. |
| `honggfuzz` | 0.5.62 `MIT/Apache-2.0/Unlicense/WTFPL` | ❌ **Reject on licence.** The WTFPL term is not on the allow-list, and cargo-deny will not accept that expression without an exception. Not worth the conversation. |
| `arbitrary` + `proptest` | 1.4.2 / 1.11.0, both `MIT OR Apache-2.0` | ✅ **Complement, not substitute.** `proptest` runs in plain `cargo test` on stable, on all four targets, in the normal PR gate. Coverage-guided fuzzing runs nightly on Linux only. You want both: proptest catches round-trip asymmetry, libFuzzer catches hostile-input panics. |

### The two targets you need in M2

#![no_main]
#![no_main]

### CI wiring

- **PR gate (all four targets, stable):** `cargo test` including the proptest round-trips. No nightly,
- **Nightly cron (ubuntu-latest, nightly toolchain):** `cargo fuzz build` then
- **Regression, on every PR (stable):** an ordinary integration test that iterates

## 5. Determinism

### The finding that makes this cheap: **the IAMF bitstream has no floating-point fields**

- `parameter_block.cc` — `GetMixGainAtTime` / `linear_mix_gain_per_tick`, which are *interpolation
- `demixing_info_parameter_data.h/.cc` — demixing coefficients, a constant lookup table used at
- `types.h` — `InternalSampleType = double`, an internal computation type.

### Containers

| Instead of | Use | Why |
|---|---|---|
| `HashMap<K, V>` | `BTreeMap<K, V>` | Deterministic iteration order across runs and targets. `HashMap`'s `RandomState` is seeded per-process. |
| `HashSet<T>` | `BTreeSet<T>` | Same. |
| Anything hashed that reaches the byte stream | **`Vec<T>`** | Strongly preferred here. Almost every IAMF collection — `audio_substream_id`, `channel_audio_layer_configs`, `sub_mixes`, `layouts` — is an *ordered sequence in the bitstream*, not a map. Modelling it as a `Vec` makes the wire order the type's order, which removes a whole class of "we sorted differently than the reference" bugs. Use `BTreeMap` only for genuine lookup indices (e.g. `audio_element_id → &AudioElement`) that never determine output order. |
| `indexmap` | — | Not needed. It is `Apache-2.0 OR MIT` and would be allowed, but a `Vec` plus a `BTreeMap` index is clearer and dependency-free. |

### Testing byte-identity across targets in CI

#[test]

- The golden is in git, so a change in output is a **reviewable diff in a PR**, not a red CI job with
- It works locally with `cargo test`, on one machine, with no CI.
- A new target added later is a one-line matrix change and is immediately covered.
- **`--release` and debug both.** Overflow behaviour differs, and `arithmetic_side_effects` only
- **`cargo test --target` for a big-endian target** (e.g. `powerpc64-unknown-linux-gnu` under `cross`)
- A `--locked` build in CI. Determinism you did not pin is a coincidence.

## 6. Codec framing — **take no FLAC or Opus dependency**

| Codec | What IAMF's `decoder_config` actually is | Source | Dependency needed |
|---|---|---|---|
| **LPCM** | `sample_format_flags` (8-bit bitmask, 0x00 BE / 0x01 LE), `sample_size`, `sample_rate`. `audio_roll_distance` must be 0. | `lpcm_decoder_config.h` | **None** |
| **FLAC** | The FLAC **STREAMINFO metadata block**: an 8-bit block header, then min/max block size (16 bits each), min/max frame size (24 bits each), sample rate (**20 bits**), channels (**3 bits**), bits-per-sample (**5 bits**, stored as value−1), total samples (**36 bits**), 16-byte MD5. IAMF pins channels = 1, roll distance = 0, and recommends frame sizes = 0 and MD5 = 0. | `decoder_config/flac_decoder_config.h` | **None.** This is ~60 lines over `bitstream-io`, whose `read_var`/`write_var` handle 20/24/36-bit fields directly. A FLAC crate would give you a *decoder*, which is not what the field holds. |
| **Opus** | The **OpusHead** identification-header fields: `version`, `output_channel_count` (fixed to 2 and explicitly ignored), `pre_skip` u16, `input_sample_rate` u32, `output_gain` i16, `mapping_family` 0. Output sample rate is **always 48 kHz** (IAMF v1.1.0 §3.11.1). | `decoder_config/opus_decoder_config.h` | **None.** ~11 bytes of fixed fields. |
| Need | Recommendation | Licence (verified) |
|---|---|---|
| Encode FLAC frames to put in an Audio Frame OBU, for decode-and-compare | `flacenc 0.5.1` as a **dev-dependency** | `Apache-2.0` ✅ — pure Rust, no C toolchain |
| Decode those frames back | `claxon 0.4.3` as a **dev-dependency** | `Apache-2.0` ✅ — pure Rust. Risk: last released 2020, no declared edition/MSRV. Verify it builds on 1.92/edition-2024 before committing to it. |
| Opus packets | **Commit pre-encoded fixtures.** Generate once with `opusenc` or `iamf-tools`' `encoder_main`, check the packets into `tests/fixtures/`, and framing-test against them. | n/a — no dependency at all |

## 7. Licence tooling — a `deny.toml` that has been executed

### The schema correction that matters

### The config — verified passing on 2026-09-07

# deny.toml — iamf-rs

# Licence policy inherited from Parallax (HANDOFF.md §3.2).

#

# cargo-deny's [licenses] section is ALLOW-LIST ONLY: anything absent from

# `allow` is denied. GPL, LGPL, AGPL, SSPL, EUPL, CDDL and OSL are therefore

# rejected by omission -- do not add them, and do not add a `deny` key

# (it was removed from cargo-deny and now errors).

# Policy, not description: don't warn about entries we haven't hit yet.

# MPL-2.0 is permitted ONLY for unmodified upstream crates, with a reason

# (HANDOFF.md §3.2). Add here, per crate, never to the global allow list.

# [[licenses.exceptions]]

# allow = ["MPL-2.0"]

# name = "symphonia-bundle-flac"

# # reason: unmodified upstream, dev-dependency only (M3 FLAC verification)

# ignore = []

- run: cargo deny --all-features check
- run: cargo deny --manifest-path fuzz/Cargo.toml --all-features check

## 8. Testing against the C++ reference

### What the references actually give you — checked, not assumed

| | `AOMediaCodec/iamf-tools` (encoder) | `AOMediaCodec/libiamf` (decoder) |
|---|---|---|
| Licence | BSD-3-Clause-Clear + AOM Patent License 1.0 | BSD-3-Clause-Clear + AOM Patent License 1.0 |
| Build system | **Bazel is the system of record**, but `CMakeLists.txt` + `build_cmake.sh` exist as a supported-in-tree alternative | CMake ≥ 3.28, plus a git submodule (`AOMediaCodec/oar`) |
| Produces | `encoder_main`, `decoder_main`, `probe_main` CLIs | `libiamf` shared/static lib + **`iamfdec`** CLI (needs `-DIAMF_TEST_TOOL=ON`) |
| C dependencies it drags in | abseil, protobuf, **fdk-aac**, loudness_ebur128, obr, audio-to-tactile, pffft, plus system opus/FLAC/expat/eigen | codecs downloaded and built by default (`-DENABLE_BUILD_CODECS=OFF` to reuse) |
| Version | tracks IAMF spec | decoder for **IAMF v1.1.0** (v1.0.1 previously) |

### Recommendation: `Command`-in-tests against pre-built binaries, discovered by env var

#[test]

### Why not the alternatives

| Approach | Verdict |
|---|---|
| **`Command` in tests** (chosen) | The reference **already ships CLIs** that do exactly what the tests need: `iamfdec` decodes to WAV (M1's decode-and-compare), `encoder_main` produces reference `.iamf` files (M2's "parse iamf-tools output"), `probe_main` dumps structure (debugging a mismatch). Process isolation means a segfault in the reference is a failed test, not a corrupted test binary. Zero effect on `cargo build`. Zero effect on Parallax. Zero C++ licences in your graph. |
| **`build.rs` invoking CMake** (e.g. `cmake` crate) | ❌ **Reject decisively.** It would make *every* `cargo build` of `iamf` — including Parallax's, and including `cargo deny`'s and docs.rs's — require CMake, a C++20 compiler, abseil, protobuf and system opus/FLAC/expat/eigen. It would drag fdk-aac's licence toward your graph. And a build script cannot be conditional on "the developer wants reference tests"; it runs always. This turns a two-dependency library into a build-systems project. |
| **FFI via `bindgen` / a `-sys` crate** | ❌ Reject for this project. `bindgen 0.73.1` is `BSD-3-Clause` (allowed), and this is the right answer when you need *fine-grained* comparison — e.g. calling `ObuHeader::ValidateAndWrite` directly to diff a single field. But `libiamf` has a git submodule and builds its own codecs, and the value over `iamfdec` is small: you want PCM equality, which the CLI already gives you. Revisit only if you find a mismatch you cannot localise from `probe_main` output. |
| **Golden fixtures, no reference at all** | ✅ **Use this *as well*, as the primary layer.** Commit small `.iamf` files produced once by `encoder_main`, plus the expected PCM. Then the *parser* tests and *byte-identity* tests run on all four CI targets with **no C++ toolchain at all**, and the expensive `libiamf` job runs on Linux only. This is the layering that keeps the PR gate fast and cross-platform while still satisfying the Core Value statement. |

### The resulting three-tier test strategy

| Tier | Runs on | Needs C++? | Proves |
|---|---|---|---|
| Unit + golden-byte + proptest round-trip | all 4 targets, every PR, stable | no | Determinism, self-consistency, byte-identity |
| Corpus regression (fuzz findings replayed) | all 4 targets, every PR, stable | no | No known crash ever returns |
| Reference decode-and-compare + `iamf-tools` parse | Linux, PR or nightly | yes | **The Core Value:** `libiamf` reads it, PCM is sample-identical |

## Installation

# M3 only:

# flacenc = "0.5"                      # Apache-2.0 — pure-Rust FLAC encoder

# claxon  = "0.4"                      # Apache-2.0 — pure-Rust FLAC decoder

## Alternatives Considered

| Recommended | Alternative | When to Use the Alternative |
|-------------|-------------|-----------------------------|
| `bitstream-io` | Hand-rolled `BitCursor` over `&[u8]` | If the `io::Error` boundary proves to be real friction, or if a zero-copy borrow of descriptor payloads (not just frame payloads) turns out to matter. ~150 lines; migration is confined to `src/bits/`. |
| `bitstream-io` | `deku` | If the format turns out to be far more regular than the reference suggests *and* the audit-against-C++ requirement is dropped. Neither is likely. |
| Hand-written parser | `winnow 1.0.4` | If Open Question 4 resolves to "yes, the decoder runs near the audio callback" and you need genuine **incremental/streaming** parsing with partial-input resumption. That is winnow's real advantage and it is a legitimate reason to revisit. |
| `proptest` | `quickcheck` | No. `proptest` shrinking is materially better for byte-level failures and `quickcheck` is effectively unmaintained. |
| `cargo-fuzz` | `afl.rs 0.18.2` (`Apache-2.0`) | As a *second* fuzzer in M5+ if libFuzzer's coverage plateaus. Different mutation strategy finds different bugs. Not instead. |
| `claxon` (FLAC decode) | `symphonia-bundle-flac 0.6.1` (**MPL-2.0**) | If `claxon` (2020, no declared edition) fails to build. Requires a `[[licenses.exceptions]]` entry with a recorded reason — permitted by the handoff for unmodified upstream. |
| Fixtures for Opus | `opus 0.4.0` / `opusic-sys 0.7.5` | Only if M3 needs to *generate* many Opus configurations at test time. Licences are fine (`MIT/Apache-2.0`, `BSD-3-Clause`); the cost is a C build on four platforms. |
| `Command`-in-tests | `bindgen 0.73.1` FFI (`BSD-3-Clause`) | If a byte mismatch appears that `probe_main` output cannot localise, and you need to call a single reference function in isolation. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `anyhow` | Handoff constraint. Callers need typed errors; `anyhow` erases them. | `thiserror` |
| `HashMap` / `HashSet` anywhere output order can see them | `RandomState` is per-process seeded — non-deterministic iteration order, target-to-target and run-to-run. Breaks the byte-identity guarantee silently. | `Vec` for wire sequences; `BTreeMap`/`BTreeSet` for lookups. Enforced by `clippy.toml` `disallowed-types`. |
| `honggfuzz 0.5.62` | Licence expression includes **WTFPL**, which is not on the allow-list and would need an exception nobody wants to defend. | `cargo-fuzz` (NCSA, scoped to an excluded `fuzz/` workspace) |
| `gpac` | **LGPL-2.1.** May not be read or ported (handoff §3.1). | Write the ISO-BMFF muxer by hand when M5 arrives |
| `libspatialaudio` | **LGPL-2.1+.** May not be read or ported. | Not applicable — no DSP in this crate |
| `leb128 0.2.7` as a dependency | Cannot emit non-minimal / fixed-size ULEB128, which the reference's `LebGenerator::kFixedSize` mode requires; does not enforce IAMF's 8-byte and `u32` caps. A file that "almost works" is the exact failure mode the handoff warns about. | Hand-written, ported from `leb_generator.cc` under its BSD licence with attribution |
| `build.rs` invoking CMake for the reference | Forces CMake + C++20 + abseil + protobuf + fdk-aac onto every consumer's `cargo build`, including Parallax's and docs.rs's. Unconditional by construction. | Pre-built binaries discovered via `IAMF_REF_DECODER`, built by `tools/build-reference.sh` in CI |
| `[licenses] deny = [...]` / `copyleft` / `allow-osi-fsf-free` / `version = 2` in `deny.toml` | **Removed from cargo-deny; they now error.** | Allow-list only; rejections happen by omission (documented in a comment) |
| Omitting `Unicode-3.0` from the allow-list | `unicode-ident` (via `syn`, via `thiserror-impl`) is `(MIT OR Apache-2.0) AND Unicode-3.0`. Without the entry, `cargo deny check licenses` fails on a clean project. Verified. | Include `"Unicode-3.0"` |
| Adding `NCSA` to the crate's allow-list for `libfuzzer-sys` | Widens the policy permanently for a dev-only tool that never ships. | `cargo fuzz init --fuzzing-workspace=true` + a scoped `fuzz/deny.toml` |
| `f64`/`f32` anywhere in the serialiser | The IAMF wire format has **no floating-point fields** (verified). Any float you introduce is a determinism risk with no format-level justification. | Integer arithmetic. Q7.8 is `i16`. `libm` only if a caller-facing dB helper appears. |
| `unwrap()` / `expect()` / `[]` indexing / bare `+` in `src/` | A parser meets hostile input by definition; a fuzzer finds all three within minutes. | `?`, `.get(..).ok_or(...)?`, `checked_add`/`checked_mul`. Enforced by the clippy `[lints]` block. |

## Stack Patterns by Milestone

- `bitstream-io` + `thiserror` only. Nothing else.
- Golden-byte tests + the `Command`-based `iamfdec` decode-and-compare.
- `deny.toml` and the clippy `[lints]` block land here — before there is any code to retrofit.
- The byte-identity CI matrix lands here (handoff: "from the first release").
- `+ arbitrary` (feature-gated), `+ proptest`.
- `cargo fuzz init --fuzzing-workspace=true`, two targets, corpus seeded from `encoder_main` output.
- Corpus-regression test added to the stable PR gate.
- `+ flacenc` / `claxon` as dev-dependencies (or MPL exception for symphonia).
- Opus: committed packet fixtures, **no dependency**.
- No change to the shipping dependency graph — codec configs are hand-written bit fields.
- No new dependencies expected.
- `libm` only if a `dB → Q7.8` conversion helper is exposed.
- Blocked on Open Question 1 (parameter tick rate) — the API takes curves or pre-decimated blocks.
- ISO-BMFF written by hand (`gpac` is unusable). Still no new dependency.
- Revisit `winnow` **only** if Open Question 4 resolves to "the decoder runs near the audio callback"
- Consider `afl.rs` as a second fuzzer, and a big-endian target in the matrix.

## Version Compatibility

| Package | Version | MSRV | Edition | Compatible with edition 2024 / rustc 1.85+ |
|---|---|---|---|---|
| `bitstream-io` | 4.10.0 | 1.83 | 2018 | ✅ |
| `thiserror` | 2.0.20 | 1.71 | 2021 | ✅ |
| `arbitrary` | 1.4.2 | 1.63 | 2021 | ✅ |
| `proptest` | 1.11.0 | **1.85** | 2021 | ✅ exactly at our floor — pins the MSRV |
| `hex-literal` | 1.1.0 | 1.85 | 2024 | ✅ |
| `flacenc` | 0.5.1 | n/a | 2021 | ⚠️ no declared MSRV — verify at M3 |
| `claxon` | 0.4.3 | n/a | **none declared** | ⚠️ 2015-edition crate, last released 2020 — **verify it builds before committing to it** |
| `cargo-deny` (tool) | 0.20.2 | 1.88 | 2024 | ✅ tool MSRV, not crate MSRV |
| `cargo-fuzz` (tool) | 0.13.2 | n/a | 2021 | ✅ requires **nightly** at run time |
| `libfuzzer-sys` | 0.4.13 | n/a | 2018 | ✅ in the excluded `fuzz/` workspace only |

- **`proptest` sets the effective MSRV at 1.85**, which coincides with the edition-2024 floor. Nothing
- `deku` is listed as MSRV 1.82 by crates.io metadata but 1.88 in upstream `Cargo.toml` — a further
- `cargo-fuzz` needs nightly; keep it out of the stable PR gate.

## Confidence Assessment

| Claim | Confidence | Basis |
|---|---|---|
| All versions and licence strings | **HIGH** | crates.io registry API, 2026-09-07 |
| The runtime graph is 9 crates, all MIT-or-Unicode-3.0 satisfiable | **HIGH** | `cargo tree -e normal` + `cargo deny check licenses` executed locally |
| The `deny.toml` above passes | **HIGH** | Executed: `advisories ok, bans ok, licenses ok, sources ok` |
| `Unicode-3.0` is mandatory | **HIGH** | Reproduced the failure and the fix locally |
| `libfuzzer-sys` NCSA is rejected by the allow-list | **HIGH** | Reproduced the exact cargo-deny error locally |
| The clippy lint set fires as described | **HIGH** | Executed against clippy 1.92.0 on a deliberately bad file |
| IAMF's bit primitives are the 22 listed | **HIGH** | Read from `iamf-tools@main` headers |
| ULEB128 is capped at 8 bytes / `u32`, OBU at 2 MiB | **HIGH** | `iamf/obu/types.h` |
| `LebGenerator` has a fixed-size (non-minimal) mode | **HIGH** | `iamf/common/leb_generator.h` |
| FLAC/Opus decoder configs need no codec crate | **HIGH** | `decoder_config/{flac,opus,lpcm}_decoder_config.h` |
| The IAMF wire format has no float fields | **HIGH** | Exhaustive grep of `iamf/obu/**` excluding tests |
| `bitstream-io`'s `BitsWritten` solves two-pass sizing | **MEDIUM-HIGH** | docs.rs API listing; not yet exercised in code |
| `claxon` builds on edition 2024 | **LOW** | Not attempted — 2020 release, no declared edition. **Verify at M3.** |
| cargo-fuzz works on Windows MSVC for this crate | **MEDIUM** | Documented upstream; not verified here. Nightly fuzz job should be Linux-only regardless. |

## Open Items for the Roadmap

## Sources

- **crates.io registry API** (`https://crates.io/api/v1/crates/<name>`) — all versions, licences,
- **Local execution**, 2026-09-07 — `cargo-deny 0.20.2`, `cargo tree`, `cargo clippy 1.92.0`,
- **`github.com/AOMediaCodec/iamf-tools`** @ `main`, shallow clone — `iamf/common/read_bit_buffer.h`,
- **`github.com/AOMediaCodec/libiamf`** @ `main` — `README.md`, `code/README.md`, `.gitmodules`,
- **docs.rs/bitstream-io/latest** — current public API (`BitRead`/`BitWrite`, `BitsWritten`,
- **embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html** — current `[licenses]` schema and
- **rust-fuzz.github.io/book** (`cargo-fuzz/setup.md`, `tutorial.md`, `ci.md`,

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
