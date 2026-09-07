# Stack Research

**Domain:** Bit-level binary format serialiser/parser library (Rust), licence-constrained, deterministic, fuzzed
**Researched:** 2026-09-07
**Confidence:** HIGH

Every version and licence below was read from the crates.io registry API on 2026-09-07, and the
licence allow-list, the dependency graph and the lint configuration were **executed locally against
`cargo-deny 0.20.2` and `clippy 1.92.0`**, not inferred. Where a claim about IAMF itself informs a
stack choice, it was read from `AOMediaCodec/iamf-tools` at `main`, not from the spec prose — per the
handoff's standing instruction.

---

## Executive recommendation

**Take exactly two runtime dependencies: `bitstream-io` and `thiserror`.**

That is not minimalism for its own sake. The reference implementation's entire bitstream primitive
surface is **12 read operations and 10 write operations** (`iamf/common/read_bit_buffer.h`,
`write_bit_buffer.h`). A crate that must be auditable line-by-line against those two files has nothing
to gain from a derive macro and everything to lose from one. `bitstream-io` supplies the bit packing
and the write-length counter; `thiserror` supplies the error enum; the OBU grammar is hand-written
against the C++.

The resulting graph that Parallax inherits is **nine crates**, and `allow = ["MIT", "Unicode-3.0"]`
satisfies all of them:

```
iamf
├── bitstream-io 4.10.0        MIT/Apache-2.0
│   └── no_std_io2 0.9.4       Apache-2.0 OR MIT
│       └── memchr 2.8.3       Unlicense OR MIT
└── thiserror 2.0.20           MIT OR Apache-2.0
    └── thiserror-impl 2.0.20  MIT OR Apache-2.0   (proc-macro)
        ├── proc-macro2 1.0.107  MIT OR Apache-2.0
        │   └── unicode-ident 1.0.24  (MIT OR Apache-2.0) AND Unicode-3.0
        ├── quote 1.0.47       MIT OR Apache-2.0
        └── syn 3.0.5          MIT OR Apache-2.0
```

*(verified: `cargo tree -e normal` + `cargo deny check licenses` on a scratch crate, 2026-09-07)*

---

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

---

## 1. Bit-level I/O — the decision, and the evidence

### What the format actually demands

Read from `iamf-tools` `main` on 2026-09-07:

| Fact | Source | Consequence for the stack |
|---|---|---|
| The complete primitive set is `ReadUnsignedLiteral(n≤64)`, `ReadSigned8/9/16`, `ReadBoolean`, `ReadString`, `ReadULeb128` (with and without a byte-count out-param), `ReadIso14496_1Expanded`, `ReadUint8Span` — and a symmetric write side | `read_bit_buffer.h:51–205`, `write_bit_buffer.h:51–128` | ~22 functions total. Any framework big enough to "help" is bigger than the thing it helps with. |
| **`ReadSigned9` / `WriteSigned9`** — a 9-bit two's-complement field | `read_bit_buffer.h:123`, `write_bit_buffer.h:79` | Rules out byte-oriented crates. `bitstream-io` does this as `read_signed_var::<i16>(9)`. |
| ULEB128 is capped at **8 bytes** (`kMaxLeb128Size = 8`) and decodes to **`uint32_t`** (`DecodedUleb128`) | `iamf/obu/types.h` | A hostile 10-byte continuation chain must be a typed error, not a `u64` overflow. Off-the-shelf LEB readers do not enforce this cap. |
| `LebGenerator` has **two modes: `kMinimum` and `kFixedSize`** (fixed size 1–8 bytes) | `iamf/common/leb_generator.h` | **Decisive.** IAMF permits *non-minimal* ULEB128, and the reference encoder uses it (fixed-size mode lets it write the header before knowing the payload size). The `leb128` crate (gimli, `MIT OR Apache-2.0`) only *emits* minimal form — it cannot reproduce a reference file byte-for-byte. Write your own; it is ~30 lines each way. |
| Whole OBU ≤ 2 MB (`kEntireObuSizeMaxTwoMegabytes = 1 << 21`); strings ≤ 128 bytes incl. NUL | `iamf/obu/types.h` | Gives the fuzz target and the parser hard, cheap allocation ceilings. A parser that allocates on a length field without checking these is the classic OOM fuzz finding. |
| The OBU **header is bit-packed; the payload is byte-aligned** | `obu_header.h`, §4.2 of the handoff | You only need bit reads inside headers and descriptor payloads. Audio-frame payloads are byte slices — take them by offset, never bit-by-bit. |

### Verdict

**Use `bitstream-io` 4.10.0 for both directions.** Wrap it in two thin newtypes —
`BitReader`/`BitWriter` — whose method names mirror `read_bit_buffer.h` exactly
(`read_unsigned_literal`, `read_signed_9`, `read_uleb128`, `write_uleb128_fixed`, …). That wrapper is
the audit seam: a reviewer diffing your `read_uleb128` against `ReadULeb128` should see the same
control flow, not a macro expansion.

Three concrete wins that no alternative gives you:

1. **`BitsWritten` solves the `obu_size` chicken-and-egg.** An OBU header carries a ULEB128 size of a
   payload you have not written yet. `BitsWritten` is a `BitWrite` sink that counts and discards, so
   you serialise the payload once into a counter, emit the header, then serialise for real — or
   serialise into a `Vec` and prepend. Both are one-liners. (`BitsWritten` supersedes the deprecated
   `BitCounter`.)
2. **`byte_align()` is a first-class operation**, which is exactly the OBU header/payload boundary.
3. **`read::<N, T>()` is const-checked**: a bit-width/type mismatch is a compile error, not a runtime
   panic. `read_var(n)` returns `Err` for a runtime-invalid width. Neither panics — which the
   no-`unwrap` constraint requires.

**Caveat to design around:** `bitstream-io` is generic over `io::Read`/`io::Write`, so its errors are
`std::io::Error` — which is not `PartialEq`, allocates on some paths, and is a poor fit in a public
error enum. Do not let `io::Error` escape. In the reader newtype, map every `io::Error` to your own
`Error::Truncated { offset }` at the boundary; you are reading from a `&[u8]` cursor, so the only
possible `io::Error` is `UnexpectedEof` anyway. This costs about ten lines and keeps the public error
type small, `Clone` and `PartialEq` (which round-trip tests want).

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

---

## 2. Parser architecture — hand-written, over `bitstream-io`

**Recommendation: hand-written recursive-descent over the `bitstream-io` wrapper. Not combinators,
not derive macros.**

The deciding criterion is in the brief itself: *"the byte layout must be auditable against a reference
C++ implementation."* That is not a normal parser requirement, and it inverts the usual calculus.

| Architecture | Fit for IAMF | Verdict |
|---|---|---|
| **Hand-written** | The reference is itself hand-written recursive descent (`ObuHeader::ReadAndValidate`, `FlacDecoderConfig::ReadAndValidate`, …). A Rust function that mirrors it 1:1 can be reviewed side-by-side against the C++ by someone who knows neither codebase well. Read and write live adjacent in the same module, which is how you catch asymmetry — the failure mode that produces "files that almost work". | ✅ **Chosen** |
| Combinators (`winnow` 1.0.4 `MIT`, `nom` 8.0.0 `MIT`) | Combinators are for *grammars*. IAMF is a length-prefixed record format with flag-dependent optional fields and forward references (`num_samples_per_frame` from the Codec Config governs how the Opus config validates). Threading that context through combinators is `winnow`'s `Stateful` — more machinery than a plain function. `nom`'s `bits` combinators are notoriously awkward. Neither produces a *writer*, so the serialiser would be hand-written anyway and the two halves would stop resembling each other. | ❌ Reject. Revisit `winnow` only if a genuinely **streaming/incremental** parser is ever needed (partial-input resumption for the in-DAW playback path in Open Question 4) — that is winnow's real edge, and `nom` 8.0 has had no release since Jan 2025 while `winnow` 1.0.4 shipped July 2026. |
| Derive macros (`deku`, `binrw`) | Optimise for *writing* a parser fast. This project optimises for *proving* a parser correct against a reference. A macro moves the byte layout into attribute syntax and the control flow into generated code, which is precisely the thing that must stay reviewable. | ❌ Reject |

### The shape to build

```
src/
  bits/          BitReader / BitWriter newtypes over bitstream-io — the ONLY module that
                 touches bitstream-io. Names mirror read_bit_buffer.h exactly.
                 uleb128.rs holds both LebMode::Minimum and LebMode::Fixed(n).
  obu/
    header.rs    ObuHeader: read + write + size, adjacent in one file
    codec_config.rs / audio_element.rs / mix_presentation.rs / ...
                 each: `fn write(&self, w: &mut BitWriter) -> Result<()>`
                       `fn read(r: &mut BitReader) -> Result<Self>`
                 with a doc comment naming the reference file and the spec section
  error.rs       one thiserror enum
```

Two conventions worth adopting from day one, because retrofitting them is expensive:

- **Every `read`/`write` pair in the same file, in that order.** Asymmetry becomes visually obvious.
- **Every function carries `// ref: iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite`.** This is the
  audit trail, and it is also how you diff against a future upstream revision. (Upstream moves: the
  tree now contains `polar_parameter_data`, `cart16_parameter_data`, `dual_polar_parameter_data` and
  `metadata_obu` — parameter-data types that postdate the handoff's read. Flag for FEATURES/roadmap;
  the handoff's "IAMF as practised is a bed format" conclusion may need re-checking.)

---

## 3. Error handling — `thiserror` 2.0.20, and the lints that enforce it

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unexpected end of bitstream at byte {offset}: needed {needed} more bit(s)")]
    Truncated { offset: usize, needed: u32 },

    #[error("uleb128 at byte {offset} exceeds the IAMF 8-byte limit")]
    Uleb128TooLong { offset: usize },

    #[error("uleb128 at byte {offset} does not fit in u32")]
    Uleb128Overflow { offset: usize },

    #[error("obu_size {size} exceeds the IAMF 2 MiB limit")]
    ObuTooLarge { size: u32 },

    #[error("reserved/unsupported obu_type {ty} at byte {offset}")]
    UnsupportedObuType { ty: u8, offset: usize },
    // ...
}

pub type Result<T> = core::result::Result<T, Error>;
```

Patterns that matter here specifically:

- **`#[non_exhaustive]`** on the enum. You will add variants every milestone; without it each one is a
  breaking change for Parallax.
- **Carry `offset` on every variant.** A fuzz crash report with a byte offset is a bug you fix in ten
  minutes; without one it is an afternoon. Cheap now, impossible to retrofit tidily.
- **Derive `Clone + PartialEq + Eq`.** Round-trip and property tests want to assert on the error, and
  this is only possible because you kept `std::io::Error` out (§1). Non-negotiable design
  consequence of the `bitstream-io` choice.
- **Keep the enum small.** Add `const _: () = assert!(size_of::<Error>() <= 32);` — `Result<T, Error>`
  is on every hot path in the parser.
- **`#[from]` sparingly.** Blanket `From` conversions lose the offset. Convert at the call site.
- **Never `panic!` on malformed input, only on broken *invariants*** — and prefer to have no such
  invariants. Debug assertions are acceptable; `unreachable!()` in a parser is not.

### Enforcement, not convention — verified working

`Cargo.toml`:

```toml
[lints.clippy]
unwrap_used             = "deny"
expect_used             = "deny"
panic                   = "deny"
indexing_slicing        = "deny"   # forces .get(..n).ok_or(Error::Truncated)?
arithmetic_side_effects = "deny"   # forces checked_add/checked_mul on all size arithmetic
disallowed_types        = "deny"
missing_docs_in_private_items = "allow"

[lints.rust]
missing_docs = "warn"
unsafe_code  = "forbid"
```

`clippy.toml`:

```toml
disallowed-types = [
  { path = "std::collections::HashMap", reason = "non-deterministic iteration order; use BTreeMap or Vec" },
  { path = "std::collections::HashSet", reason = "non-deterministic iteration order; use BTreeSet or Vec" },
]
```

**Verified 2026-09-07** against clippy 1.92.0: all six lints fire on a deliberately bad file, with the
`disallowed-types` `reason` string surfaced in the diagnostic. In `#[cfg(test)]` modules add
`#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]`
— the handoff permits `unwrap` in tests, and `indexing_slicing`/`arithmetic_side_effects` are
unbearable in test code.

`arithmetic_side_effects` is the highest-value lint on this list and the one people disable first.
Resist that. Every integer-overflow CVE in a parser is `header_size + obu_size` in release mode.

---

## 4. Fuzzing — `cargo-fuzz`, in an **excluded** `fuzz/` workspace

### The licence problem, and its resolution

`libfuzzer-sys 0.4.13` is `(MIT OR Apache-2.0) AND NCSA`. Run against your allow-list, cargo-deny
0.20.2 rejects it — **verified**:

```
error[rejected]: failed to satisfy license requirements
36 │ license = "(MIT OR Apache-2.0) AND NCSA"
   │                                    rejected: license is not explicitly allowed
   ├ NCSA - University of Illinois/NCSA Open Source License:
   ├   - OSI approved
   ├   - FSF Free/Libre
```

NCSA is the LLVM licence — a permissive BSD+MIT hybrid, no copyleft, and it is `AND`-ed because the
crate vendors LLVM's libFuzzer runtime.

**Do not widen the allow-list to accept it.** Instead:

```sh
cargo fuzz init --fuzzing-workspace=true
```

This gives `fuzz/` its **own workspace and its own `Cargo.lock`**, so `libfuzzer-sys` never appears in
the crate's lockfile, never appears in `cargo deny check` for the library, and can never reach
Parallax's dependency graph. Add `/fuzz` to the root `Cargo.toml`'s `workspace.exclude` and to
`.gitignore`'s exceptions so `fuzz/Cargo.lock` and the seed corpus *are* committed.

Then add a second `fuzz/deny.toml` with `"NCSA"` appended to the same allow-list, and run
`cargo deny check` in `fuzz/` separately. That way the exception is explicit, scoped, documented and
still gated — rather than silently absent.

### Tool choice

| Option | Version / Licence | Verdict |
|---|---|---|
| **`cargo-fuzz` + `libfuzzer-sys`** | 0.13.2 `MIT OR Apache-2.0` / 0.4.13 `(MIT OR Apache-2.0) AND NCSA` | ✅ **Chosen.** The default in the Rust ecosystem. In-process, fast, coverage-guided, integrates with ASan/UBSan. Now supports Windows via MSVC AddressSanitizer, plus x86_64 Linux and both macOS arches — matching your four CI targets except Windows-on-Arm. |
| `afl.rs` | 0.18.2 `Apache-2.0` | ❌ Reject as primary. Licence is clean and it finds different bugs, but it requires building AFL++ from source and is fork-based (slower per exec for a microsecond-scale parser). Consider as a *second* fuzzer in M5+ if the corpus plateaus. |
| `honggfuzz` | 0.5.62 `MIT/Apache-2.0/Unlicense/WTFPL` | ❌ **Reject on licence.** The WTFPL term is not on the allow-list, and cargo-deny will not accept that expression without an exception. Not worth the conversation. |
| `arbitrary` + `proptest` | 1.4.2 / 1.11.0, both `MIT OR Apache-2.0` | ✅ **Complement, not substitute.** `proptest` runs in plain `cargo test` on stable, on all four targets, in the normal PR gate. Coverage-guided fuzzing runs nightly on Linux only. You want both: proptest catches round-trip asymmetry, libFuzzer catches hostile-input panics. |

### The two targets you need in M2

```rust
// fuzz/fuzz_targets/obu_parse.rs — the hostile-input target
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    // must not panic, must not OOM, must not hang, for ANY input
    let _ = iamf::parse_ia_sequence(data);
});
```

```rust
// fuzz/fuzz_targets/obu_roundtrip.rs — the correctness target
#![no_main]
use libfuzzer_sys::fuzz_target;
use iamf::Obu;
fuzz_target!(|obu: Obu| {                       // Obu: Arbitrary
    let Ok(bytes) = obu.to_bytes() else { return };
    let parsed = iamf::Obu::parse(&bytes).expect("our own output must parse");
    assert_eq!(obu, parsed);                    // model equality
    assert_eq!(bytes, parsed.to_bytes().unwrap()); // byte equality
});
```

Gate the `Arbitrary` impl on a `fuzzing` feature so `arbitrary` is not a normal dependency:
`arbitrary = { version = "1.4", features = ["derive"], optional = true }` +
`[features] fuzzing = ["dep:arbitrary"]`.

**Seed the corpus from real bytes.** Drop every `.iamf` produced by `iamf-tools`' `encoder_main` into
`fuzz/corpus/obu_parse/` and commit them. A coverage-guided fuzzer starting from a valid IA Sequence
reaches the interesting code in minutes; starting from `""` it may never get past the sequence header.
This is the single highest-leverage fuzzing decision in M2.

Structure-aware note: an experiment published June 2026 found the `fuzz_mutator!` macro gives better
coverage over time than `Arbitrary`-based generation. Use `Arbitrary` for the round-trip target
(where you want valid models) and consider `fuzz_mutator!` for `obu_parse` in M5+ if coverage stalls.

### CI wiring

Two separate jobs, because they have incompatible requirements:

- **PR gate (all four targets, stable):** `cargo test` including the proptest round-trips. No nightly,
  no sanitizers, no `fuzz/` build.
- **Nightly cron (ubuntu-latest, nightly toolchain):** `cargo fuzz build` then
  `cargo fuzz run <target> -- -max_total_time=300` per target in a matrix, `cargo install cargo-fuzz --locked`
  cached by version, `actions/upload-artifact` on `failure()` for `fuzz/artifacts`.
- **Regression, on every PR (stable):** an ordinary integration test that iterates
  `fuzz/corpus/obu_parse/*` and `fuzz/artifacts/**` and asserts each parses-or-errors without
  panicking. This runs the accumulated fuzz findings on *all four targets* with no nightly and no
  cargo-fuzz. Every crash the fuzzer ever found becomes a permanent, cross-platform regression test.
  This is the piece teams forget, and it is the one that keeps the value.

---

## 5. Determinism

### The finding that makes this cheap: **the IAMF bitstream has no floating-point fields**

Grepping `iamf/obu/**` in `iamf-tools` (excluding tests) for `float`/`double` on 2026-09-07 returns
matches only in:

- `parameter_block.cc` — `GetMixGainAtTime` / `linear_mix_gain_per_tick`, which are *interpolation
  helpers* for a renderer, not serialisation;
- `demixing_info_parameter_data.h/.cc` — demixing coefficients, a constant lookup table used at
  decode time (and deferred by the handoff anyway);
- `types.h` — `InternalSampleType = double`, an internal computation type.

Everything on the wire is an integer: loudness is `int16_t` in Q7.8 (`mix_presentation.h:117–124`),
mix gain is `int16_t` Q7.8, and even the object-position parameter data is `int16_t`/`int8_t`/`uint8_t`
(`polar_parameter_data.h`).

**Consequence: `libm` is not a day-one dependency.** Determinism for the serialiser is achieved
entirely by integer arithmetic and ordered containers. Add `libm` only when you write a caller-facing
`dB → Q7.8` helper, or if you ever implement Bézier parameter animation. When you do: `libm 0.2.16`,
`MIT`, and route *every* transcendental through it — a single stray `f64::powf` reintroduces
target-dependence and there is no compiler diagnostic for it. Consider a
`disallowed-methods` clippy entry for `f64::sin`/`cos`/`powf`/`exp`/`ln` at that point.

### Containers

| Instead of | Use | Why |
|---|---|---|
| `HashMap<K, V>` | `BTreeMap<K, V>` | Deterministic iteration order across runs and targets. `HashMap`'s `RandomState` is seeded per-process. |
| `HashSet<T>` | `BTreeSet<T>` | Same. |
| Anything hashed that reaches the byte stream | **`Vec<T>`** | Strongly preferred here. Almost every IAMF collection — `audio_substream_id`, `channel_audio_layer_configs`, `sub_mixes`, `layouts` — is an *ordered sequence in the bitstream*, not a map. Modelling it as a `Vec` makes the wire order the type's order, which removes a whole class of "we sorted differently than the reference" bugs. Use `BTreeMap` only for genuine lookup indices (e.g. `audio_element_id → &AudioElement`) that never determine output order. |
| `indexmap` | — | Not needed. It is `Apache-2.0 OR MIT` and would be allowed, but a `Vec` plus a `BTreeMap` index is clearer and dependency-free. |

Enforced by the `clippy.toml` `disallowed-types` entry in §3 — verified firing.

Also: **do not iterate a `BTreeMap` to produce output order** unless the spec's order *is* key order.
Prefer `Vec`, and if you must, add a golden test.

### Testing byte-identity across targets in CI

The common mistake is to build a cross-job artifact-comparison pipeline. Don't. **Commit the golden
bytes and let every target prove it reproduces them.**

```rust
#[test]
fn golden_lpcm_stereo_sequence_is_byte_identical() {
    let bytes = fixtures::lpcm_stereo_48k().to_bytes().unwrap();
    assert_eq!(bytes.as_slice(), &include_bytes!("golden/lpcm_stereo_48k.iamf")[..]);
}
```

Run that test in a matrix of `ubuntu-latest`, `macos-14` (arm64), `macos-13` (x86_64),
`windows-latest` (MSVC). Properties this has that a cross-job comparison does not:

- The golden is in git, so a change in output is a **reviewable diff in a PR**, not a red CI job with
  no artefact.
- It works locally with `cargo test`, on one machine, with no CI.
- A new target added later is a one-line matrix change and is immediately covered.

Supplement with:

- **`--release` and debug both.** Overflow behaviour differs, and `arithmetic_side_effects` only
  catches what it can see statically. Set `overflow-checks = true` in `[profile.release]` — for a
  serialiser this crate's performance is dominated by memcpy of PCM, so the cost is nil and a silent
  wraparound in `obu_size` is a corrupt file.
- **`cargo test --target` for a big-endian target** (e.g. `powerpc64-unknown-linux-gnu` under `cross`)
  if you want real confidence. IAMF is big-endian on the wire; `bitstream-io`'s `BigEndian` handles
  this, but a hand-written `u32::from_be_bytes` slip only shows up on a BE host. Optional, M5.
- A `--locked` build in CI. Determinism you did not pin is a coincidence.

---

## 6. Codec framing — **take no FLAC or Opus dependency**

Read from the reference on 2026-09-07, and this is the headline finding of this section:

**IAMF's codec-specific config fields are not codec data. They are fixed bit layouts you write by hand.**

| Codec | What IAMF's `decoder_config` actually is | Source | Dependency needed |
|---|---|---|---|
| **LPCM** | `sample_format_flags` (8-bit bitmask, 0x00 BE / 0x01 LE), `sample_size`, `sample_rate`. `audio_roll_distance` must be 0. | `lpcm_decoder_config.h` | **None** |
| **FLAC** | The FLAC **STREAMINFO metadata block**: an 8-bit block header, then min/max block size (16 bits each), min/max frame size (24 bits each), sample rate (**20 bits**), channels (**3 bits**), bits-per-sample (**5 bits**, stored as value−1), total samples (**36 bits**), 16-byte MD5. IAMF pins channels = 1, roll distance = 0, and recommends frame sizes = 0 and MD5 = 0. | `decoder_config/flac_decoder_config.h` | **None.** This is ~60 lines over `bitstream-io`, whose `read_var`/`write_var` handle 20/24/36-bit fields directly. A FLAC crate would give you a *decoder*, which is not what the field holds. |
| **Opus** | The **OpusHead** identification-header fields: `version`, `output_channel_count` (fixed to 2 and explicitly ignored), `pre_skip` u16, `input_sample_rate` u32, `output_gain` i16, `mapping_family` 0. Output sample rate is **always 48 kHz** (IAMF v1.1.0 §3.11.1). | `decoder_config/opus_decoder_config.h` | **None.** ~11 bytes of fixed fields. |

So the *shipping* crate needs zero codec dependencies. What M3 needs is **test material**, and that is
a different question with a different answer:

| Need | Recommendation | Licence (verified) |
|---|---|---|
| Encode FLAC frames to put in an Audio Frame OBU, for decode-and-compare | `flacenc 0.5.1` as a **dev-dependency** | `Apache-2.0` ✅ — pure Rust, no C toolchain |
| Decode those frames back | `claxon 0.4.3` as a **dev-dependency** | `Apache-2.0` ✅ — pure Rust. Risk: last released 2020, no declared edition/MSRV. Verify it builds on 1.92/edition-2024 before committing to it. |
| Opus packets | **Commit pre-encoded fixtures.** Generate once with `opusenc` or `iamf-tools`' `encoder_main`, check the packets into `tests/fixtures/`, and framing-test against them. | n/a — no dependency at all |

On Opus specifically: **there is no pure-Rust Opus encoder.** Every option is an FFI wrapper —
`opus 0.4.0` (`MIT/Apache-2.0` wrapper), `audiopus 0.2.0` (`ISC`, unmaintained since 2021),
`opusic-sys 0.7.5` (`BSD-3-Clause`, vendors and builds libopus). All of their *licences* are on the
allow-list, and libopus itself is BSD-3-Clause, so none is forbidden. But every one of them puts a C
build into `cargo test` on four platforms including Windows MSVC, for a crate whose entire Opus
responsibility is emitting an 11-byte header and copying opaque packets. Fixtures cost one afternoon
and zero maintenance.

**Rejected on licence:** nothing here, notably. **Rejected on policy:** `symphonia-bundle-flac 0.6.1`
and `symphonia 0.6.1` are **MPL-2.0** — permitted by the handoff *only* as unmodified upstream with a
recorded reason. They are unmodified upstream, so this is legal, but prefer Apache-2.0 `claxon` and
avoid needing the exception at all. **Not applicable:** `gpac` (LGPL-2.1) and `libspatialaudio`
(LGPL-2.1+) remain unreadable and unportable per the handoff — the ISO-BMFF muxer is written by hand
when it is written.

---

## 7. Licence tooling — a `deny.toml` that has been executed

### The schema correction that matters

**`cargo-deny`'s `[licenses]` section is now allow-list-only.** The fields `deny`, `copyleft`,
`allow-osi-fsf-free`, `default` and `unlicensed` **have been removed and now emit errors**, and the
`version` field is no longer used. Anything not in `allow` is denied.

This changes how the handoff's policy is expressed. "Rejected outright: GPL, LGPL, AGPL, SSPL, EUPL,
CDDL, OSL" **cannot be written as a deny list**. It is expressed by *omission* — which is stricter and
better, but means the rejection is invisible in the config file. Put it in a comment so the intent
survives review.

### The config — verified passing on 2026-09-07

```toml
# deny.toml — iamf-rs
# Licence policy inherited from Parallax (HANDOFF.md §3.2).
#
# cargo-deny's [licenses] section is ALLOW-LIST ONLY: anything absent from
# `allow` is denied. GPL, LGPL, AGPL, SSPL, EUPL, CDDL and OSL are therefore
# rejected by omission -- do not add them, and do not add a `deny` key
# (it was removed from cargo-deny and now errors).

[graph]
all-features = true
targets = [
  { triple = "x86_64-unknown-linux-gnu" },
  { triple = "x86_64-apple-darwin" },
  { triple = "aarch64-apple-darwin" },
  { triple = "x86_64-pc-windows-msvc" },
]

[licenses]
# Policy, not description: don't warn about entries we haven't hit yet.
unused-allowed-license = "allow"
confidence-threshold = 0.93
allow = [
  # Parallax preferred list
  "MIT",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSD-3-Clause-Clear",       # libiamf, if ported source is ever vendored
  "ISC",
  "Zlib",
  "Unlicense",
  "CC0-1.0",
  # Required: unicode-ident (via syn, via every proc macro incl. thiserror)
  # is "(MIT OR Apache-2.0) AND Unicode-3.0". Unicode License v3 is
  # OSI-approved and permissive. Without this, NOTHING using a derive
  # macro can build. Verified 2026-09-07.
  "Unicode-3.0",
]

[licenses.private]
ignore = true                  # this crate is publish = false

# MPL-2.0 is permitted ONLY for unmodified upstream crates, with a reason
# (HANDOFF.md §3.2). Add here, per crate, never to the global allow list.
# [[licenses.exceptions]]
# allow = ["MPL-2.0"]
# name = "symphonia-bundle-flac"
# # reason: unmodified upstream, dev-dependency only (M3 FLAC verification)

[bans]
multiple-versions = "warn"
wildcards = "deny"

[advisories]
yanked = "deny"
# ignore = []

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

**Verified 2026-09-07** on a scratch crate carrying `bitstream-io 4.10`, `thiserror 2.0`,
`arbitrary 1.4` (+derive), `proptest 1.11`, `claxon 0.4`, `flacenc 0.5`, `hex-literal 1.1`:

```
$ cargo deny --all-features check
advisories ok, bans ok, licenses ok, sources ok
```

Three empirical findings from that run that you would otherwise hit as CI failures:

1. **`Unicode-3.0` is mandatory.** Without it the check fails on `unicode-ident`, which arrives via
   `syn` via `thiserror-impl`. Every Rust project using any derive macro needs this entry.
2. **The slash form parses.** `bitstream-io`'s `license = "MIT/Apache-2.0"` is not valid SPDX, but
   cargo-deny 0.20.2 accepts it as `OR`. No `[[licenses.clarify]]` needed. (Same applies to
   `num-rational` and `opus` if they ever appear.)
3. **`unused-allowed-license = "allow"`** is worth setting. With the default `"warn"`, every
   allow-list entry you have not yet encountered (ISC, Zlib, CC0, BSD-*) emits a warning, which
   trains people to ignore cargo-deny output.

Add a second `fuzz/deny.toml` — identical plus `"NCSA"` — and run both in CI:

```yaml
- run: cargo deny --all-features check
- run: cargo deny --manifest-path fuzz/Cargo.toml --all-features check
```

---

## 8. Testing against the C++ reference

### What the references actually give you — checked, not assumed

| | `AOMediaCodec/iamf-tools` (encoder) | `AOMediaCodec/libiamf` (decoder) |
|---|---|---|
| Licence | BSD-3-Clause-Clear + AOM Patent License 1.0 | BSD-3-Clause-Clear + AOM Patent License 1.0 |
| Build system | **Bazel is the system of record**, but `CMakeLists.txt` + `build_cmake.sh` exist as a supported-in-tree alternative | CMake ≥ 3.28, plus a git submodule (`AOMediaCodec/oar`) |
| Produces | `encoder_main`, `decoder_main`, `probe_main` CLIs | `libiamf` shared/static lib + **`iamfdec`** CLI (needs `-DIAMF_TEST_TOOL=ON`) |
| C dependencies it drags in | abseil, protobuf, **fdk-aac**, loudness_ebur128, obr, audio-to-tactile, pffft, plus system opus/FLAC/expat/eigen | codecs downloaded and built by default (`-DENABLE_BUILD_CODECS=OFF` to reuse) |
| Version | tracks IAMF spec | decoder for **IAMF v1.1.0** (v1.0.1 previously) |

Note the fdk-aac dependency in `iamf-tools`' build. That is a licence you do not want anywhere near
your dependency graph — which decides question 8 on its own.

### Recommendation: `Command`-in-tests against pre-built binaries, discovered by env var

```rust
// tests/reference_decode.rs
fn reference_decoder() -> Option<PathBuf> {
    std::env::var_os("IAMF_REF_DECODER").map(PathBuf::from)
        .or_else(|| existing("target/reference/bin/iamfdec"))
}

#[test]
fn libiamf_decodes_our_lpcm_sequence_sample_identically() {
    let Some(dec) = reference_decoder() else {
        eprintln!("skipping: set IAMF_REF_DECODER or run tools/build-reference.sh");
        return;                       // skip, do not fail
    };
    let iamf = fixtures::lpcm_stereo_48k().to_bytes().unwrap();
    // write to a tempdir, run `dec -o out.wav in.iamf`, compare PCM to the input
}
```

Plus `tools/build-reference.sh` that clones both repos at **pinned commits**, builds them into
`target/reference/`, and is invoked by one CI job. Pin the commits: an unpinned reference makes
"conformance" a moving target, which is the same failure mode the handoff's spec-version constraint
guards against.

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

---

## Installation

`Cargo.toml`:

```toml
[package]
name = "iamf"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
publish = false
license = "MIT"                        # see Open Question 2: MIT vs MIT OR Apache-2.0

[features]
default  = ["encode"]
encode   = []
decode   = []
fuzzing  = ["dep:arbitrary"]

[dependencies]
bitstream-io = "4.10"                  # MIT/Apache-2.0 — bit-level reader/writer
thiserror    = "2.0"                   # MIT OR Apache-2.0 — typed library errors
arbitrary    = { version = "1.4", features = ["derive"], optional = true }  # MIT OR Apache-2.0

[dev-dependencies]
proptest          = "1.11"             # MIT OR Apache-2.0 — round-trip properties
hex-literal       = "1.1"              # MIT OR Apache-2.0 — readable byte fixtures
pretty_assertions = "1.4"              # MIT OR Apache-2.0 — readable byte diffs
# M3 only:
# flacenc = "0.5"                      # Apache-2.0 — pure-Rust FLAC encoder
# claxon  = "0.4"                      # Apache-2.0 — pure-Rust FLAC decoder

[profile.release]
overflow-checks = true                 # a silent wraparound in obu_size is a corrupt file

[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
indexing_slicing = "deny"
arithmetic_side_effects = "deny"
disallowed_types = "deny"

[lints.rust]
unsafe_code = "forbid"
```

Tooling:

```sh
rustup toolchain install nightly           # cargo-fuzz only
cargo install cargo-deny --locked          # 0.20.2
cargo install cargo-fuzz  --locked         # 0.13.2, M2
cargo fuzz init --fuzzing-workspace=true   # M2 — keeps libfuzzer-sys out of our lockfile
```

---

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

---

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

---

## Stack Patterns by Milestone

**M1 — a file `libiamf` can decode:**
- `bitstream-io` + `thiserror` only. Nothing else.
- Golden-byte tests + the `Command`-based `iamfdec` decode-and-compare.
- `deny.toml` and the clippy `[lints]` block land here — before there is any code to retrofit.
- The byte-identity CI matrix lands here (handoff: "from the first release").

**M2 — parser, round trip, fuzzing:**
- `+ arbitrary` (feature-gated), `+ proptest`.
- `cargo fuzz init --fuzzing-workspace=true`, two targets, corpus seeded from `encoder_main` output.
- Corpus-regression test added to the stable PR gate.

**M3 — FLAC and Opus framing:**
- `+ flacenc` / `claxon` as dev-dependencies (or MPL exception for symphonia).
- Opus: committed packet fixtures, **no dependency**.
- No change to the shipping dependency graph — codec configs are hand-written bit fields.

**M4 — the Parallax-facing API:**
- No new dependencies expected.
- `libm` only if a `dB → Q7.8` conversion helper is exposed.
- Blocked on Open Question 1 (parameter tick rate) — the API takes curves or pre-decimated blocks.

**M5+ — ISO-BMFF, decoder, scalable layers:**
- ISO-BMFF written by hand (`gpac` is unusable). Still no new dependency.
- Revisit `winnow` **only** if Open Question 4 resolves to "the decoder runs near the audio callback"
  and incremental parsing is genuinely required.
- Consider `afl.rs` as a second fuzzer, and a big-endian target in the matrix.

---

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

Notes:
- **`proptest` sets the effective MSRV at 1.85**, which coincides with the edition-2024 floor. Nothing
  costs you anything.
- `deku` is listed as MSRV 1.82 by crates.io metadata but 1.88 in upstream `Cargo.toml` — a further
  reason not to depend on it. Not applicable given the recommendation.
- `cargo-fuzz` needs nightly; keep it out of the stable PR gate.

---

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

**Flagged — licence not verified beyond registry metadata:** every licence above comes from the
crates.io `license` field, which is what `cargo-deny` itself reads and enforces, so it is the
operative source. `bitstream-io` was additionally confirmed by the presence of `LICENSE-MIT` and
`LICENSE-APACHE` in the upstream repo. `libm 0.2.16` reports `MIT` alone in registry metadata while
now being published from `rust-lang/compiler-builtins` (dual-licensed); MIT is on the allow-list
either way, but re-check if it is ever added.

---

## Open Items for the Roadmap

1. **Upstream has moved past the handoff's read.** `iamf-tools@main` now contains
   `polar_parameter_data`, `dual_polar_parameter_data`, `cart8/cart16_parameter_data`,
   `animated_parameter_data` and `metadata_obu` — object-position parameter data with animation types
   and an `int16`/`int8`/`uint8` encoding. This bears on handoff §4.1 correction 1 ("IAMF as practised
   is a bed format") and on Open Question 5. Not a stack decision; flag for FEATURES.md.
2. `libiamf`'s README states it decodes **IAMF v1.1.0**. The handoff's spec-version constraint says
   pin a version and name it in code; v1.1.0 is what the reference decoder will actually accept.
3. Verify `claxon` builds on rustc 1.92 / edition 2024 before M3 commits to it.
4. Pin the `iamf-tools` and `libiamf` commits in `tools/build-reference.sh` from day one.

---

## Sources

- **crates.io registry API** (`https://crates.io/api/v1/crates/<name>`) — all versions, licences,
  MSRVs, editions, download counts, last-updated dates. Queried 2026-09-07. **HIGH**
- **Local execution**, 2026-09-07 — `cargo-deny 0.20.2`, `cargo tree`, `cargo clippy 1.92.0`,
  `cargo generate-lockfile` against scratch crates carrying the proposed dependency sets. **HIGH**
- **`github.com/AOMediaCodec/iamf-tools`** @ `main`, shallow clone — `iamf/common/read_bit_buffer.h`,
  `write_bit_buffer.h`, `leb_generator.h`, `iamf/obu/types.h`, `obu_header.h`,
  `decoder_config/{lpcm,flac,opus}_decoder_config.h`, `mix_presentation.h`, `polar_parameter_data.h`,
  `CMakeLists.txt`, `build_cmake.sh`. BSD-3-Clause-Clear, read under its own terms. **HIGH**
- **`github.com/AOMediaCodec/libiamf`** @ `main` — `README.md`, `code/README.md`, `.gitmodules`,
  directory listing. BSD-3-Clause-Clear. **HIGH**
- **docs.rs/bitstream-io/latest** — current public API (`BitRead`/`BitWrite`, `BitsWritten`,
  `BitRecorder`, endianness types). **HIGH**
- **embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html** — current `[licenses]` schema and
  the removal of `deny`/`copyleft`/`allow-osi-fsf-free`/`default`/`version`. **HIGH**
- **rust-fuzz.github.io/book** (`cargo-fuzz/setup.md`, `tutorial.md`, `ci.md`,
  `structure-aware-fuzzing.md`) and **github.com/rust-fuzz/cargo-fuzz** README — platform support,
  workspace handling, `--fuzzing-workspace`, CI template, `fuzz_mutator!` vs `Arbitrary`. **HIGH**

---
*Stack research for: bit-level binary format serialiser/parser (IAMF), Rust 2024, deterministic, fuzzed, licence-constrained*
*Researched: 2026-09-07*
