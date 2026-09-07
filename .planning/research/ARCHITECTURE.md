# Architecture Research

**Domain:** Bitstream codec library (IAMF OBU serialiser/parser, encoder, later decoder) in Rust
**Researched:** 2026-09-07
**Confidence:** HIGH for everything derived from reading `AOMediaCodec/iamf-tools` and `AOMediaCodec/libiamf` source directly; MEDIUM for the Rust-ecosystem comparisons and tooling conventions.

**Licence hygiene for this document:** the only third-party sources read were `AOMediaCodec/iamf-tools` (BSD-3-Clause-Clear + AOM PL 1.0), `AOMediaCodec/libiamf` (BSD-3-Clause-Clear), `alfg/mp4-rust` (MIT), `pdeljanov/Symphonia` (MPL-2.0, structure only), `jam1garner/binrw` (MIT) and public Rust/Cargo documentation. **No LGPL or GPL source was consulted.** `libspatialaudio` and `gpac` were not opened.

**Provenance note:** all `iamf-tools` and `libiamf` claims below were read from `raw.githubusercontent.com` at named paths on `main`, not from prose about them. Paths are cited so they can be re-checked. Latest tagged release at time of reading: **`iamf-tools` v2.1.0**.

---

## Standard Architecture

A bitstream codec library of this shape has four horizontal layers. Everything above a layer depends on everything below it, and nothing depends upwards. This is the shape `iamf-tools` has (`iamf/common` → `iamf/obu` → `iamf/cli` → `iamf/include`), the shape the `mp4` crate has (`types` → `mp4box/*` → `reader`/`writer`), and the shape recommended here.

### System Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│  L4  PUBLIC API                     (what Parallax calls)            │
│  ┌───────────────────┐  ┌────────────────────┐  ┌─────────────────┐  │
│  │ EncoderBuilder    │  │ Encoder (push/pull)│  │ Reader / ObuIter│  │
│  │  #[cfg(encode)]   │  │   #[cfg(encode)]   │  │  (ungated)      │  │
│  └─────────┬─────────┘  └─────────┬──────────┘  └────────┬────────┘  │
├────────────┼──────────────────────┼──────────────────────┼───────────┤
│  L3  DRIVERS  (ordering, timing, validation — not per-OBU)           │
│  ┌─────────▼──────┐  ┌────────────▼──────┐  ┌────────────▼────────┐  │
│  │ EncodePlan     │  │ sequence::write   │  │ sequence::read      │  │
│  │ profile choice │  │ descriptors→data  │  │ + ParamDefRegistry  │  │
│  └─────────┬──────┘  └────────────┬──────┘  └────────────┬────────┘  │
│            │         ┌────────────▼──────┐                │          │
│            │         │ codec:: framing   │  (LPCM/FLAC/Opus, NO DSP) │
│            │         └────────────┬──────┘                │          │
├────────────┼──────────────────────┼──────────────────────┼───────────┤
│  L2  MODEL + PER-TYPE SERDE       (the shared core — ungated)        │
│  ┌─────────▼──────────────────────▼──────────────────────▼────────┐  │
│  │ obu::{header, ia_sequence_header, codec_config, audio_element, │  │
│  │       mix_presentation, parameter_block, audio_frame,          │  │
│  │       temporal_delimiter, arbitrary}                           │  │
│  │  each: pub struct + impl ReadObu + impl WriteObu, same file    │  │
│  ├────────────────────────────────────────────────────────────────┤  │
│  │ labels (layout/channel LABELS only)  ·  profile (fit rules)    │  │
│  └───────────────────────────┬────────────────────────────────────┘  │
├──────────────────────────────┼───────────────────────────────────────┤
│  L1  PRIMITIVES              │           (ungated, zero deps)        │
│  ┌───────────────────────────▼────────────────────────────────────┐  │
│  │ bits::{BitReader, BitWriter, leb128, LebPolicy}                │  │
│  │ types::{Uleb128, Q7_8, AudioElementId, ParameterId, …}         │  │
│  │ error::{Error, …}  (thiserror)                                 │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Owns | Does not own | Reference counterpart |
|---|---|---|---|
| `bits` | Bit cursor, sub-byte reads/writes, ULEB128/SLEB128, `LebPolicy`, `remaining()` | Any knowledge of OBUs | `iamf/common/{read,write}_bit_buffer.*`, `iamf/common/leb_generator.h` |
| `types` | Newtype IDs, fixed-point (`Q7_8`), `Uleb128` | Semantics of any field | `iamf/obu/types.h` |
| `error` | The one public error enum tree | Recovery policy | `absl::Status` (no Rust analogue needed) |
| `obu::header` | 5-bit type, flag bits, `obu_size` and its ULEB128 width | Payload meaning | `iamf/obu/obu_header.h` |
| `obu::<type>` | One struct per OBU type, all fields public, `read_payload`/`write_payload` beside it | Ordering, timing, IDs assignment | `iamf/obu/*.{h,cc}`, `ObuBase` |
| `obu::arbitrary` | Verbatim passthrough of unrecognised OBU types + an insertion hook | Interpreting them | `iamf/obu/arbitrary_obu.h` |
| `labels` | Layout and channel-label enums, canonical channel order per layout | Geometry, angles, gains | `iamf/cli/channel_label.h` |
| `profile` | Simple/Base/Base-Enhanced limits, "does this fit", "minimum profile" | Building the sequence | `iamf/cli/profile_filter.h` |
| `codec` | Framing only: PCM ↔ substream payload bytes | Any DSP, any resampling | `iamf/cli/codec/` |
| `sequence::write` | Descriptor-then-data ordering, redundant descriptor copies, temporal-delimiter insertion, `LebPolicy` application | Field values | `iamf/cli/obu_sequencer_{base,iamf,streaming_iamf}.h` |
| `sequence::read` | OBU iteration, `ParamDefinitionRegistry` construction, bounds enforcement | Producing PCM | `iamf/cli/{descriptor_obu_parser,obu_processor}.h` |
| `encode` | Builder validation, ID assignment, plan → descriptors, temporal-unit push/pull | Bit layout | `iamf/api/`, `iamf/include/iamf_tools/` |

---

## Recommended Project Structure

```
iamf-rs/
├── Cargo.toml                  # publish = false; features encode/decode/flac/opus/serde
├── deny.toml                   # Parallax allow-list, run in CI here
├── src/
│   ├── lib.rs                  # #![forbid(unsafe_code)]; SPEC_VERSION const; re-exports
│   ├── error.rs                # thiserror: Error { Parse(..), Write(..), Invalid(..) }
│   ├── types.rs                # Uleb128, Sleb128, Q7_8, Q0_8, ID newtypes
│   ├── bits/
│   │   ├── mod.rs
│   │   ├── reader.rs           # BitReader<'a> over &'a [u8]; sub_reader(len); remaining()
│   │   ├── writer.rs           # BitWriter<'w> into &'w mut Vec<u8>
│   │   └── leb128.rs           # LebPolicy::{Minimal, Fixed(1..=8)}; encode/decode
│   ├── obu/
│   │   ├── mod.rs              # ObuType, enum Obu, ReadObu/WriteObu traits, dispatch
│   │   ├── header.rs           # ObuHeader — flags, obu_size, extension header
│   │   ├── ia_sequence_header.rs      # type 31
│   │   ├── codec_config.rs             # type 0
│   │   ├── decoder_config/
│   │   │   ├── mod.rs
│   │   │   ├── lpcm.rs
│   │   │   ├── flac.rs
│   │   │   └── opus.rs
│   │   ├── audio_element.rs            # type 1 (single-layer channel-based + ambisonics mono)
│   │   ├── mix_presentation.rs         # type 2 (carries loudness numbers)
│   │   ├── param_definition/           # lives in descriptors; needed to parse type 3
│   │   │   ├── mod.rs                  # enum ParamDefinition + ParamDefinitionKind
│   │   │   ├── mix_gain.rs
│   │   │   ├── demixing.rs
│   │   │   ├── recon_gain.rs
│   │   │   └── extended.rs             # unknown definition kinds, kept verbatim
│   │   ├── param_data/                 # lives in type 3; mirrors param_definition
│   │   │   ├── mod.rs
│   │   │   ├── mix_gain.rs
│   │   │   ├── demixing.rs
│   │   │   ├── recon_gain.rs
│   │   │   └── extension.rs            # unknown data kinds, kept verbatim
│   │   ├── parameter_block.rs          # type 3 — Ctx = &ParamDefinitionRegistry
│   │   ├── temporal_delimiter.rs       # type 4
│   │   ├── audio_frame.rs              # types 5..=23
│   │   └── arbitrary.rs                # unknown OBU types, verbatim + InsertionHook
│   ├── labels.rs               # LoudspeakerLayout, ExpandedLayout, ChannelLabel — LABELS ONLY
│   ├── profile.rs              # Profile::{Simple, Base, BaseEnhanced}; fits(); minimum_for()
│   ├── codec/
│   │   ├── mod.rs              # trait Framer / Unframer; SampleFormat
│   │   ├── lpcm.rs             # always available
│   │   ├── flac.rs             # #[cfg(feature = "flac")]
│   │   └── opus.rs             # #[cfg(feature = "opus")]
│   ├── sequence/
│   │   ├── mod.rs              # DescriptorSet, TemporalUnit, IaSequence  (model, ungated)
│   │   ├── write.rs            # SequenceWriter — descriptor/data ordering
│   │   └── read.rs             # SequenceReader, ObuIter, ParamDefinitionRegistry
│   ├── encode/                 # #[cfg(feature = "encode")]
│   │   ├── mod.rs              # Encoder, EncoderBuilder
│   │   ├── plan.rs             # EncodePlan: validated config → DescriptorSet
│   │   ├── input.rs            # TemporalUnitInput, ParameterBlockInput, Trim
│   │   └── curve.rs            # #[cfg(feature = "curves")] — OPTIONAL, see §Open fork
│   └── decode/                 # #[cfg(feature = "decode")]
│       └── mod.rs              # Decoder: OBUs → PCM (needs codec decoders)
├── fuzz/                       # independent workspace; NOT a root workspace member
│   ├── Cargo.toml              # has its own empty [workspace]; iamf = { path = "..",
│   │                           #   default-features = false }
│   └── fuzz_targets/
│       ├── obu_header.rs
│       ├── parse_sequence.rs
│       └── round_trip.rs       # arbitrary-derived model → write → read → assert eq
└── tests/
    ├── fixtures/               # .iamf goldens generated once from iamf-tools v2.1.0
    │   ├── README.md           # records the iamf-tools tag + commit SHA + how regenerated
    │   └── *.iamf
    ├── goldens.sha256          # our own output hashes — the cross-target identity test
    ├── golden.rs               # writes canonical files, compares SHA-256
    ├── round_trip.rs
    ├── parse_reference.rs      # parse iamf-tools output
    └── reference_decoder.rs    # shells out to libiamf's iamfdec; skips if env unset
```

### Structure Rationale

- **`bits/` is separate and knows nothing about IAMF.** It is the only place a bit offset exists. That makes it independently unit-testable and it is where the `obu_size`/ULEB128 semantics the handoff refuses to assert get pinned down first. `iamf-tools` does the same in `iamf/common/`.
- **One file per OBU type.** Both references agree: `iamf/obu/<type>.{h,cc}` and `src/mp4box/<box>.rs`. A field added to Mix Presentation touches one file, and its read, its write and its test are all in it.
- **`param_definition/` sits under `obu/`, not under a parser module,** because the definitions are *descriptor content* (they live inside Audio Element and Mix Presentation OBUs) even though they are *parse context* for Parameter Blocks. Putting them anywhere else creates a circular module dependency.
- **`param_data/` mirrors `param_definition/` file for file.** The reference keeps them paired (`iamf/obu/param_definitions/mix_gain_param_definition.h` ↔ `iamf/obu/mix_gain_parameter_data.h`). Keeping the mirror explicit means a new parameter kind is obviously two files, not one.
- **`arbitrary.rs` and `param_data/extension.rs` are not optional extras.** They are the round-trip contract (see §Round-trip fidelity).
- **`sequence/` is the container, and it is a driver, not a model.** The standalone `.iamf` writer and a future ISO-BMFF writer are two backends behind one ordering policy — which is exactly how `iamf-tools` splits `obu_sequencer_base` from `obu_sequencer_iamf`.
- **`labels.rs` is a flat file of enums with no arithmetic in it.** Making it a single file, not a directory, is a deliberate signal that nothing grows there. It carries layout *identity* and canonical *channel order*; it carries no angles, no coordinates, no gains.

---

## How `iamf-tools` Organises the Same Concepts — and Where Rust Should Diverge

Read from `main` (v2.1.0). Confidence HIGH — these are file paths and header contents, not summaries.

| Concept | `iamf-tools` | Recommended Rust | Why diverge |
|---|---|---|---|
| Bit IO | `iamf/common/read_bit_buffer.h`, `write_bit_buffer.h` | `bits::BitReader<'a>` / `BitWriter<'w>` | Same shape. Diverge on ownership: the C++ reader owns a buffer; the Rust reader should **borrow** `&'a [u8]` so parsing allocates nothing and `sub_reader(len)` can enforce payload bounds in the type system. |
| ULEB width policy | `LebGenerator{kMinimum, kFixedSize(1..8)}` passed into `WriteBitBuffer` and into `ObuSequencerBase` | `LebPolicy` enum, a field on `SequenceWriter`, defaulting to `Minimal` | Keep it. This is not incidental — it is what makes byte-for-byte comparison against reference output possible at all. Diverge by making it non-optional and non-defaultable at the *sequence* level, so the choice is recorded in the golden fixtures. |
| OBU base class | `ObuBase` with `virtual ValidateAndWritePayload` + `virtual ReadAndValidatePayloadDerived`, plus a concrete non-virtual `ValidateAndWriteObu` wrapper | Two traits, `WriteObu` and `ReadObu` with an associated context type; a free function for the header wrapper | Rust has no inheritance and does not need it. **The important divergence:** the C++ `ReadAndValidatePayloadDerived(payload_size, rb)` cannot express that Parameter Block needs a `ParamDefinition` — so the reference has to route around it with a *static factory* `ParameterBlockObu::CreateFromBuffer(header, size, param_definition, rb)` that sits outside the virtual dispatch. Rust can express it directly with a GAT: `type Ctx<'a>`, `()` for most types and `&'a ParamDefinitionRegistry` for Parameter Block. That removes an entire class of "you forgot to call the right factory" bug. |
| Unknown trailing bytes | `ObuBase::footer_` — public `std::vector<uint8_t>`, documented against spec §3.2 ("obu_size MAY be greater than the size needed… Parsers SHOULD ignore bytes past the OBU syntax that they recognize") | `trailing: Vec<u8>` on every OBU struct | Keep exactly. Rust should additionally make it *impossible to forget*: the shared header-level `read_obu()` wrapper drains the remainder into `trailing`, so a type-specific `read_payload` that under-reads cannot silently lose data. |
| Unknown OBU types | `ArbitraryObu` with an `InsertionHook` enum (before/after descriptors, after codec configs, after audio frames at tick…) and `invalidates_bitstream: bool` | `obu::arbitrary::ArbitraryObu` with the same hook enum | Keep. The hook enum is what lets an unknown OBU round-trip *to the right byte offset*, not just survive. The `invalidates_bitstream` flag is also a ready-made negative-test generator. |
| Sequencing | `ObuSequencerBase` (abstract) + `ObuSequencerIamf` + `ObuSequencerStreamingIamf`; usage is `PushDescriptorObus` → `PushTemporalUnit`* → `UpdateDescriptorObusAndClose` \| `Close`, with `Abort` for cleanup | `SequenceWriter` trait + `IamfWriter`; `push_descriptors` → `push_temporal_unit` → `finish` | Keep the push model. Diverge on `Abort`: in Rust, make failure consume the writer (`fn push_temporal_unit(self, …) -> Result<Self, Error>` or a poisoned-state flag) and use `Drop` for file cleanup, rather than requiring the caller to remember `Abort()`. |
| Public API | `iamf/include/iamf_tools/` — abstract `IamfEncoderInterface` + a factory, with `iamf/api/` behind it | `encode::Encoder` as a concrete struct + `EncoderBuilder` | **Do not introduce a trait/factory pair.** The C++ split exists to give a stable ABI across a shared-library boundary. Rust has one consumer, by path dependency, with no ABI. A trait here buys nothing and costs dynamic dispatch and a second place to change. |
| Descriptor collections | `DescriptorObus::CodecConfigsById`, `AudioElementsById`; public API uses `absl::flat_hash_map<uint32_t, …>` | `Vec<CodecConfigObu>` + `Vec<AudioElementObu>` in bitstream order, with `fn by_id(&self, id) -> Option<&_>` | **Mandatory divergence** — see §Determinism. `flat_hash_map` is both non-deterministic in iteration and *order-lossy*, and descriptor order is bitstream-visible. |
| PCM at the boundary | `IamfAudioElementData = flat_hash_map<string /*channel label*/, Span<const double>>` | `&[(ChannelLabel, &[f64])]`, or planar `&[&[f64]]` in the layout's canonical channel order | Keep `f64` and keep it planar — it matches Parallax's renderer and avoids an interleave nobody asked for. Drop the hash map: an ordered slice is deterministic, allocation-free and, since the layout already fixes the channel order, strictly more informative. |
| Label ↔ string tables | `BuildStaticMapFromPairs` producing a static `absl` map | `match` arms, or a sorted `&'static [(K, V)]` with binary search | Simpler, `const`-evaluable, no static initialisation, no hashing. |
| Curve → block decimation | `iamf/cli/parameter_block_partitioner.{h,cc}` — **in the CLI layer, above the API** | Above `encode`, behind an optional module | Evidence for the open question; see §The open fork. |
| Profile | `iamf/cli/profile_filter.{h,cc}` — *filters* a set of OBUs down to a profile | `profile::Profile::fits(&DescriptorSet)` + `minimum_for(&DescriptorSet)` | Diverge in direction: the reference removes things to fit a chosen profile; this crate's stated requirement is to *choose* the minimum profile that fits. Make it a pure query over the descriptor set, with no mutation. |

**One thing not to copy:** `iamf-tools` routes its entire configuration through protocol buffers (`iamf/cli/proto/`, `proto_to_obu/`, `obu_to_proto/`, 338 `.textproto` test vectors). That is a test-harness and CLI concern, and importing it would add `prost`/`protobuf` to the dependency graph for no benefit. Rust struct literals are already the configuration language.

---

## The Model/Codec Split: Types Own Their Serialisation

**Recommendation: types own `read_payload` and `write_payload`, in the same file as the struct, behind two crate-level traits. Do not build parallel `model/`, `read/`, `write/` module trees. Drivers — sequence ordering, registry construction, the encoder — do get their own modules, because that logic is not per-type.**

### What comparable projects do

| Project | Pattern | Notes |
|---|---|---|
| `iamf-tools` (C++, BSD) | Type owns it. `ObuBase` virtuals `ValidateAndWritePayload` / `ReadAndValidatePayloadDerived`, implemented in each `iamf/obu/<type>.cc` | The reference for this exact format |
| `mp4` (`alfg/mp4-rust`, MIT) | Type owns it. `src/mp4box/<box>.rs` defines the struct and impls `ReadBox`/`WriteBox` from `mp4box/mod.rs`. Whole-file drive in `src/reader.rs`, `src/writer.rs` | Closest Rust analogue: one file per box, drivers separate |
| `binrw` (MIT) | Type owns it, derived. `#[binrw]` generates `BinRead`+`BinWrite` on one struct; `#[br(import{..})]`/`#[bw(args(..))]` pass parse context to children; `#[br(temp)]` drops redundant length fields from the struct | The context-passing mechanism is exactly the Parameter-Block problem, solved generically |
| `Symphonia` (MPL-2.0, structure only) | Splits by *direction of use*, not by read/write: `symphonia-core` holds shared traits, `symphonia-format-*` are demuxers, `symphonia-codec-*` are decoders, registered into a probe/registry | The crate-per-codec split is the model for the `codec` module and the eventual `flac`/`opus` features |
| `rav1e` | Write-only OBU emission through a bit writer; no parser, so no round-trip invariant to maintain | Instructive as a counter-example: this crate must not end up write-only, because the round-trip test is a stated requirement |

### Why the split hurts here

Round-trip equality is the reason. `parse(serialize(m)) == m` is a per-type invariant. If the struct is in `model/mix_presentation.rs`, its writer in `write/mix_presentation.rs` and its reader in `read/mix_presentation.rs`, then adding a field means editing three files, and forgetting one of the other two **compiles fine** and fails only at runtime as a round-trip mismatch. Co-located, the same omission is either a compile error (unused field warning, non-exhaustive construction) or a diff a reviewer sees in one hunk. Co-location also puts the per-type round-trip unit test in the same file's `#[cfg(test)] mod tests`, which is where it will actually get written.

### The traits

```rust
// src/obu/mod.rs
pub trait WriteObu {
    /// Payload only. The header — including obu_size — is written by `write_obu`.
    fn write_payload(&self, w: &mut BitWriter<'_>) -> Result<(), Error>;
}

pub trait ReadObu: Sized {
    /// Parse context. `()` for everything except Parameter Block.
    type Ctx<'a>;

    /// `r` is already bounded to this OBU's payload; reading past its end is an error,
    /// and reading less than all of it is legal (the remainder becomes `trailing`).
    fn read_payload(r: &mut BitReader<'_>, ctx: Self::Ctx<'_>) -> Result<Self, Error>;
}
```

`Ctx` as a GAT is the single divergence that matters most from the C++ design. In `iamf-tools`, `ParameterBlockObu` cannot use the virtual read path — it needs `ParamDefinition` — so it exposes `PeekParameterId(rb)` and a static `CreateFromBuffer(header, payload_size, param_definition, rb)` and the caller must know to use them (`iamf/obu/parameter_block.h`). In Rust the requirement is in the signature: `impl ReadObu for ParameterBlockObu { type Ctx<'a> = &'a ParamDefinitionRegistry; }`. You cannot call it wrong.

### Should `binrw` or `deku` derive this?

**No, for v1. Hand-write it.** Reasons, in order:

1. **The load-bearing detail is `obu_size` and the flag bits**, which the handoff explicitly declines to assert and demands be verified against source. Hand-written code makes that verification a readable 40-line function. A derive macro makes it an attribute soup that is harder to diff against a C++ header.
2. `binrw` requires `Read + Seek` (`read_options<R: Read + Seek>`). A borrowed `&[u8]` slice parser needs neither, and `Seek` is friction for a streaming push parser.
3. IAMF is bit-packed at the header (5-bit type + single-bit flags). `binrw` has no first-class sub-byte field support; `deku` does but brings `bitvec`.
4. `LebPolicy` — writing the *same value* at a caller-chosen width — is a policy the derive macros have no vocabulary for.
5. Zero optional dependencies in the core keeps `--no-default-features` genuinely dependency-free, which keeps `cargo deny` quiet and keeps the fuzz target's build fast.

Steal `binrw`'s *ideas* — the `args`/`import` context mechanism (→ `Ctx`), and `#[br(temp)]`'s insight that a length field stored in a `Vec` is redundant and should not be a struct field (→ never store a count that a `Vec::len()` already carries; see §Round-trip fidelity).

---

## Feature Gating

### The decision that makes this work

**Put both directions of OBU serialisation in the ungated core. Gate the *drivers* and the *codec libraries*, not the bitstream.**

`encode` and `decode` in the IAMF sense mean "PCM → bytes" and "bytes → PCM". The expensive, optional, licence-sensitive thing is the **codec** (libFLAC, libopus), not the bitstream parser. The parser is a few thousand lines of pure Rust with no dependencies. Gating it:

- breaks the round-trip test, which needs read and write simultaneously and is a stated requirement;
- doubles the `cfg` matrix inside every OBU file, which is where `cfg` sprawl comes from;
- gains nothing — no dependency and negligible code size.

So the seam is:

```toml
[features]
default = ["encode"]

# Produce bytes from PCM: the Encoder driver + the encode side of codec framing.
encode = []

# Produce PCM from bytes: the streaming Decoder driver + codec decoders.
decode = []

# Codec framing. Additive; either direction can use them.
flac = ["dep:<flac-binding>"]     # licence: BSD-3-Clause (verify on the line that adds it)
opus = ["dep:<opus-binding>"]     # licence: BSD-3-Clause (verify on the line that adds it)

# Debug/inspection JSON dump only. NOT the bitstream.
serde = ["dep:serde"]             # licence: MIT OR Apache-2.0

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

This yields a useful third configuration nobody asked for but everybody wants: `--no-default-features` gives a **probe/inspect build** — full model, full parser, full serialiser, zero optional dependencies. That is the ideal fuzz target dependency and the ideal `cargo deny` baseline.

### What must be in the shared core

`error`, `types`, `bits`, all of `obu/` (both directions), `labels`, `profile`, `sequence::{mod, write, read}`, `codec::lpcm` and the `codec::Framer` trait. LPCM framing is core because it has no dependency and because M1 has nothing to encode without it.

### Making `--no-default-features --features encode` compile

Three rules, all mechanical:

1. **`cfg` lives at module declarations in `mod.rs`, never mid-struct and never mid-function.**
   ```rust
   // src/lib.rs
   pub mod bits;
   pub mod error;
   pub mod labels;
   pub mod obu;
   pub mod profile;
   pub mod sequence;
   pub mod types;
   pub mod codec;

   #[cfg(feature = "encode")]
   pub mod encode;

   #[cfg(feature = "decode")]
   pub mod decode;
   ```
   A `#[cfg]` on a struct field is the sprawl that eventually makes a feature non-additive. Ban it by convention and grep for it in review.

2. **Gated items get a doc label.**
   ```rust
   #[cfg(feature = "encode")]
   #[cfg_attr(docsrs, doc(cfg(feature = "encode")))]
   pub mod encode;
   ```
   with `#![cfg_attr(docsrs, feature(doc_cfg))]` at the crate root. Without this, docs.rs shows gated items with no indication they are gated, and readers file bugs about missing types.

3. **A gated module may depend on the core and on other gated modules it names, never the reverse.** `sequence::write` must not reference `encode::Encoder`. This is what actually keeps the graph acyclic under any feature subset.

### CI matrix — the only thing that keeps this true

```yaml
- cargo check --no-default-features
- cargo check --no-default-features --features encode
- cargo check --no-default-features --features decode
- cargo check --no-default-features --features encode,decode
- cargo check --all-features
- cargo test  --all-features
# and periodically:
- cargo hack check --feature-powerset --depth 2
```

### Feature-gating pitfalls for a codec crate

| Pitfall | What goes wrong | Prevention |
|---|---|---|
| **Non-additive features** | `#[cfg(feature = "decode")] pub extra: u8` on an OBU struct changes the type's shape depending on who else is in the dependency graph. Feature unification means you do not control that. | Never `cfg` a field. Cargo's own guidance: enabling a feature must not change or remove functionality. |
| **`cfg` sprawl** | Feature checks scattered through parsing code produce combinations nobody compiles and `E0432 unresolved import` only in the untested one. | `cfg` at module boundaries only + the check matrix above. |
| **`dev-dependencies` mask breakage** | A dev-dependency on the crate itself, or on a test helper that enables `all-features`, silently re-enables everything, so `cargo test --no-default-features` passes while `cargo build --no-default-features` for a real consumer fails. | Use `cargo check` (not `test`) for the minimal-feature rungs of the matrix. |
| **Docs build differs from every real build** | `all-features = true` compiles a combination CI may never test. | Include `--all-features` in the matrix explicitly. |
| **Feature unification with Parallax** | Parallax enabling `decode` anywhere in its graph turns it on for the export path too. If `decode` ever changed encoder behaviour, exports would stop being byte-identical depending on unrelated Parallax features. | Assert in a test that a canonical file's SHA-256 is identical under `--features encode` and `--features encode,decode`. This is cheap and it makes the determinism guarantee robust against the one thing most likely to break it. |
| **Codec bindings drag `-sys` crates** | `libopus`/`libFLAC` bindings pull build scripts, `cc`, possibly vendored C, and licences that must clear `deny.toml`. | Keep them behind `flac`/`opus`, never in `default`. State the licence on the line that adds each. Run `cargo deny check --all-features`, not just default. |
| **`default = ["encode"]`** | A consumer who wants only parsing still compiles the encoder. | Acceptable here — the encoder has no optional dependencies, and Parallax's primary use is export. Revisit only if `encode` ever gains a dependency. |

---

## Round-Trip Fidelity

There are two different assertions and they must not be confused.

| Assertion | Strength | When it must hold |
|---|---|---|
| **A. `parse(serialize(m)) == m`** — model round-trip | The stated M2 requirement | Always, for every model the crate can construct |
| **B. `serialize(parse(b)) == b`** — byte round-trip | Much stronger | For files **this crate wrote**: always. For foreign files: only under a matching `LebPolicy` — do not promise it in v1 |

The distinction matters because ULEB128 is not canonical. `LebGenerator{kMinimum, kFixedSize(1..8)}` in `iamf/common/leb_generator.h` exists because the reference deliberately emits non-minimal encodings in some configurations. A foreign file with a 4-byte encoding of `obu_size = 7` parses to the same model as a 1-byte one, so **B fails on a byte comparison even though nothing is wrong.** Promise A universally, promise B for own-output, and document the `LebPolicy` caveat for B on foreign input.

### The five things that must be modelled to make A true

1. **Trailing bytes.** Spec §3.2, quoted verbatim in `iamf/obu/obu_base.h`: *"The obu_size MAY be greater than the size needed to represent the OBU syntax. Parsers SHOULD ignore bytes past the OBU syntax that they recognize."* The reference keeps them in `ObuBase::footer_`. **Every OBU struct gets `pub trailing: Vec<u8>`**, and the shared `read_obu` wrapper — not the per-type `read_payload` — drains the remainder into it. Dropping them makes A false for any conformant file that uses the allowance.

2. **Unknown OBU types.** Types 25–30 are reserved and type 24 (Metadata) is newer than the handoff's list. `obu::arbitrary::ArbitraryObu { obu_type, header, payload: Vec<u8>, insertion_hook }`, from `iamf/obu/arbitrary_obu.h`. Note the hook enum: `BeforeDescriptors`, `AfterIaSequenceHeader`, `AfterCodecConfigs`, `AfterAudioElements`, `AfterMixPresentations`, `AfterDescriptors`, `Before/AfterParameterBlocksAtTick`, `AfterAudioFramesAtTick`. Preserving the *type* without the *position* still fails A at the sequence level.

3. **Unknown parameter data and unknown param definitions.** `iamf/obu/extension_parameter_data.h` keeps `parameter_data_bytes: Vec<u8>` verbatim; `param_definitions/extended_param_definition.*` does the same for definitions. Mirror both.

4. **Reserved bits as fields, not assertions.** Model `reserved: u8` (or `reserved: bool`) and write back what was read. Do **not** assert reserved bits are zero on parse — the spec says ignore, not reject, and a rejection makes the crate fail on a future-version file that is otherwise perfectly readable. Reserved bits that are always zero on the write path are a `Default` impl, not a parse constraint.

5. **Fields the reference always fixes.** `iamf-tools` writes one channel layer with recon gain and output gain flagged absent. Do **not** encode that as a type-level absence (`struct AudioElement { /* no layers vector */ }`) — model the layer vector and the flags, default them to the reference's values in the builder, and let the parser read any value. A model that cannot represent what a conformant file contains cannot satisfy A for that file.

### Lossy normalisations to refuse

| Tempting normalisation | Why it breaks round-trip |
|---|---|
| Sorting descriptors by ID (a `BTreeMap<AudioElementId, _>` does this silently) | Descriptor order is bitstream-visible. Use `Vec` in bitstream order + a lookup helper. |
| Recomputing `obu_size` on read and discarding the observed encoding | Breaks assertion B against reference output. Keep the observed byte width alongside the value if B is ever wanted for foreign files. |
| Storing counts as struct fields *and* as `Vec::len()` | Two sources of truth that can disagree. Store only the `Vec`; derive the count on write. (`binrw`'s `#[br(temp)]` exists for exactly this.) |
| Collapsing a redundant descriptor copy | `obu_redundant_copy` descriptors are re-emitted deliberately for stream sync (`GetDescriptorObus(redundant_copy, …)`). They are content, not duplication. |
| Canonicalising `constant_subblock_duration` vs an explicit subblock schedule | Both encode the same timing; only one is what the file said. |

### Making the assertion cheap

Derive `PartialEq` on every OBU type and let the derive *be* the comparator — `iamf-tools` does the same with `friend bool operator==(...) = default` on `ObuBase`, `ObuHeader` and `ArbitraryObu`. A hand-written comparator is where "we normalise that field" quietly enters, and the test stops testing anything. If a field genuinely cannot be compared, that is a design smell to fix, not to paper over in `eq`.

---

## The API Surface Parallax Calls

### Shape

Two layers, deliberately.

**Low level — struct literals, everything public.** `iamf::obu::*` and `iamf::sequence::*`. Fields are public, `PartialEq`, `Clone`, `Debug`. This is what the fuzz target, the round-trip test and any future ISO-BMFF work use. No builders here: a builder over a wire-format struct just hides fields the format requires.

**High level — a builder.** `iamf::encode::EncoderBuilder`. A builder rather than a struct literal because the invalid states are numerous and cross-cutting (profile vs element count vs channel count vs layout vs codec vs sample rate vs parameter rate), and a builder gives exactly one place — `build() -> Result<Encoder, Error>` — where all of them are checked and where IDs are assigned and the minimum profile is chosen. A struct literal would push validation to first use, which is the wrong place for a caller who wants to know before rendering an hour of audio.

### Streaming, with a whole-file convenience over it

The reference's public API (`iamf/include/iamf_tools/iamf_encoder_interface.h`) is push/pull:

```
GetDescriptorObus(redundant_copy, &out, &finalized)
while (GeneratingTemporalUnits()) { Encode(temporal_unit_data); OutputTemporalUnit(&out); }
FinalizeEncode();
```

Mirror it, because the reasons are structural, not stylistic:

```rust
let mut enc = EncoderBuilder::new()
    .sample_rate(48_000)
    .bit_depth(BitDepth::S24)
    .codec(Codec::Lpcm)
    .layout(LoudspeakerLayout::Ch7_1_4)   // a label
    .parameter_rate(48_000)               // ← see §The open fork; must be known HERE
    .loudness(loudness)                   // ← see below
    .leb_policy(LebPolicy::Minimal)
    .build()?;                            // ← the single validation point; picks min profile

enc.write_descriptors(&mut out)?;         // bytes now, before any audio
for unit in units {
    enc.push_temporal_unit(&unit)?;       // planar f64 + parameter blocks + trim
    enc.take_output(&mut out)?;           // appends; caller controls the sink
}
enc.finish(&mut out)?;
```

Plus a one-liner for the common case:

```rust
pub fn encode_to_writer<W: Write>(plan: &EncodePlan, units: impl Iterator<Item = TemporalUnitInput>, w: W) -> Result<(), Error>;
```

Streaming is the primitive and whole-file is the wrapper, not the other way round, because: an export of an hour of 7.1.4 at 24-bit is ~4 GB and must not be assembled in memory; Parallax's export already has a progress/cancel loop that wants a push boundary; and IAMF's own redundant-descriptor mechanism only makes sense in a streaming shape.

### Where loudness enters — a real fork

The reference API documents it plainly: *"Mix Presentation OBUs contain loudness information, which is only possible to know after all data OBUs are generated… after encoding is finished, a final call to get non-redundant OBUs with accurate loudness information is encouraged."* Two shapes:

**(i) Loudness supplied up front, at `build()`. Recommended as the primary.** Parallax has a BS.1770 meter that already shares code with export normalisation (`MON-05`), so the numbers exist before the export loop starts. Descriptors are written once, the writer is **append-only**, `W: Write` suffices, and the output is trivially streamable and trivially deterministic.

**(ii) Loudness supplied at `finish()`, writer seeks back.** Requires `W: Write + Seek`, so it cannot write to a pipe, and it makes the descriptor prefix's byte length load-bearing — with a `Minimal` `LebPolicy`, a later loudness value could change a ULEB width and change the prefix length, which is precisely why `LebPolicy::Fixed(n)` exists in the reference. Offer this as `SequenceWriter<W: Write + Seek>` for the case where loudness genuinely is not known in advance, and document the `Fixed` requirement.

Shape (i) is the default; (ii) is available. Do not build (ii) in M1.

### What crosses the boundary

| Direction | Payload | Type |
|---|---|---|
| In, once | Sample rate, bit depth, codec, layout **label**, parameter rate, loudness, optional forced profile | `EncoderBuilder` setters |
| In, per temporal unit | PCM, planar, `f64`, one slice per channel label, per audio element | `&[(ChannelLabel, &[f64])]` |
| In, per temporal unit | Parameter block inputs | `&[ParameterBlockInput]` — see fork |
| In, per temporal unit | `samples_to_trim_at_start` / `_at_end` | `Trim { start: u32, end: u32 }` |
| Out, once | Chosen profile, assigned IDs, `encoder_delay()` in samples | `EncoderInfo` |
| Out, per call | Serialised bytes, appended | `&mut Vec<u8>` or `impl Write` |

`f64` planar matches both the reference (`absl::Span<const double>`) and Parallax's `f64` renderer. The quantisation from `f64` to the LPCM bit depth happens **in this crate**, once, in `codec::lpcm`, deterministically — that is framing, not DSP, and putting it here is what makes the export byte-identical rather than dependent on how the caller rounded.

### The open fork: position curves vs pre-decimated blocks

**Not decided here.** Both options are architecturally coherent; the consequences differ sharply. One fact constrains both: **`parameter_rate` lives in the `param_definition`, which lives inside the Audio Element / Mix Presentation descriptors.** Descriptors are written before any audio. So the tick rate is a `build()`-time input under either option — it is never a per-block choice. That is where the decision bites.

**Option A — the crate takes pre-decimated blocks.**
`ParameterBlockInput { parameter_id, duration, subblocks: Vec<Subblock> }`, near-identical to the OBU.
- The crate acquires **no time model and no floating-point arithmetic on the encode path**. The determinism guarantee is inherited from Parallax rather than re-established here. `libm` is not needed.
- `encode/` stays thin: validate that blocks tile the temporal units without gap or overlap, and serialise.
- Round-trip stays clean — the input type is close enough to the OBU type that `parse(serialize(x)) == x` is nearly an identity test at the API level.
- Matches the reference's own layering: `parameter_block_partitioner.cc` sits in `iamf/cli/`, **above** the public API, not inside it.
- Cost: the decimation policy lives in Parallax and must be shared with ADM BWF there. If it is not, it gets written twice and the two exports disagree — which is exactly the reason the open question says Parallax should answer it once for both.
- Cost: a naive caller can produce a wasteful block schedule (one subblock per sample for a static position) and the crate will faithfully serialise it.

**Option B — the crate takes curves.**
`PositionCurve { keyframes, interpolation }` or a `Fn(tick) -> Value`.
- The crate acquires a **time model and a sampling policy**: tick-rate selection, subblock partitioning, and interpolation kind (IAMF mix gain has animated step/linear/Bézier forms — `iamf/obu/animated_parameter_data.h`). None of that is DSP, but all of it is *lossy decision-making the crate would then own*, which sits uncomfortably beside the no-DSP constraint and would need an explicit carve-out in the scope document.
- It puts `f64` arithmetic on the encode path here, so the byte-identity guarantee now depends on this crate's maths across four targets, not only on Parallax's. `libm` becomes a dependency of the core.
- It weakens the round-trip invariant at the API level: what goes in is not what comes out, so `parse(serialize(x)) == x` no longer type-checks at the public boundary and the invariant has to be asserted one layer down, where it is easier to forget.
- Benefit: one implementation of the decimation, chosen by the party that knows the format's constraints, able to collapse a static curve to a single subblock and to pick a rate that divides the frame size cleanly.
- Benefit: Parallax's export code gets simpler and cannot get the tiling wrong.

**A structure that defers the decision without cost.** Make the core API take blocks (Option A), and put curves in `encode::curve` behind a `curves` feature, layered strictly above the block API and consuming it. Then: the core stays free of time arithmetic; the decision is reversible; and if Parallax later answers "curves", the module is promoted rather than the API rewritten. If Parallax answers "blocks", the module is never written. Adopting this layering is not the same as answering the question — the tick-rate policy still has to be decided before `build()` has a sensible signature.

---

## Data Flow

### Encode path

```
Parallax
  │  ExportConfig: sample_rate, bit_depth, codec, layout LABEL,
  │                parameter_rate, loudness, optional forced profile
  ▼
EncoderBuilder::build()  ── validates ── assigns IDs ── profile::minimum_for()
  │
  │  crossing: EncodePlan  (a fully validated, ID-assigned config; the ONLY place
  │            profile selection and ID assignment happen)
  ▼
plan::to_descriptors()
  │
  │  crossing: DescriptorSet { IaSequenceHeaderObu, Vec<CodecConfigObu>,
  │            Vec<AudioElementObu>, Vec<MixPresentationObu> }  — in bitstream order
  ▼
sequence::write::write_descriptors(&DescriptorSet, LebPolicy, &mut BitWriter)
  │
  │  crossing: bytes                                            ──────────────► out
  ▼
── per temporal unit ───────────────────────────────────────────────────────────
  │
  │  crossing: TemporalUnitInput {
  │              pcm: &[(AudioElementId, &[(ChannelLabel, &[f64])])],
  │              parameters: &[ParameterBlockInput],
  │              trim: Trim }
  ▼
codec::Framer::frame()      LPCM: f64 → quantise → interleave → substream bytes
  │                         FLAC/Opus: hand to the codec, take back a packet
  │  crossing: Vec<u8> per substream + optional trim hint from encoder delay
  ▼
TemporalUnit { Option<TemporalDelimiterObu>, Vec<ParameterBlockObu>, Vec<AudioFrameObu> }
  │
  │  crossing: the OBU model — pure data, no bit offsets exist above this line
  ▼
obu::write_obu()  → ObuHeader::write (obu_size via LebPolicy) → write_payload
  │
  │  crossing: bits — the ONLY place bit offsets exist
  ▼
BitWriter → Vec<u8> → caller's Write                            ──────────────► out
```

### Parse path

```
&[u8]  (borrowed, whole file or a growing buffer)
  ▼
BitReader<'a>::new(&bytes)          — no allocation; tracks remaining()
  ▼
ObuHeader::read(&mut r)
  │  crossing: (ObuType, payload_len, flags) — payload_len is passed DOWN,
  │            never inferred by the payload parser
  ▼
r.sub_reader(payload_len)           — bounds enforced by the borrow, not by discipline
  ▼
dispatch on ObuType
  ├─ 31, 0, 1, 2 → descriptor OBUs
  │      │  SIDE EFFECT: every param_definition found inside an Audio Element or
  │      │  Mix Presentation is inserted into ParamDefinitionRegistry
  │      ▼  crossing: &ParamDefinitionRegistry  (BTreeMap<ParameterId, ParamDefinition>)
  ├─ 3  → ParameterBlockObu::read_payload(r, &registry)    ← context-dependent
  ├─ 4  → TemporalDelimiterObu (empty payload)
  ├─ 5..=23 → AudioFrameObu (substream id implicit in the type for 6..=23)
  └─ 24..=30, unknown → ArbitraryObu { obu_type, payload: verbatim }
  ▼
r.remaining() drained into obu.trailing        ← the §3.2 allowance, done once, centrally
  ▼
IaSequence { descriptors: DescriptorSet, temporal_units: Vec<TemporalUnit> }
   or, streaming: Iterator<Item = Result<Obu, Error>>
```

### Boundary contracts

| Boundary | What crosses | Contract |
|---|---|---|
| Parallax → `EncoderBuilder` | Labels and numbers | No DSP objects, no geometry, no `f64` *time* |
| `EncoderBuilder` → `EncodePlan` | Validated config | Every invalid combination is rejected here or nowhere |
| `EncodePlan` → `DescriptorSet` | OBU structs | Pure function; same plan ⇒ same descriptors, always |
| `Encoder` → `codec::Framer` | `&[&[f64]]` planar + `SampleFormat` | Framer returns bytes + an optional delay/trim hint; it never sees an OBU |
| OBU model → `bits` | Struct + `LebPolicy` | The only place bit offsets exist |
| `bits` → OBU payload parser | `sub_reader(payload_len)` | A payload parser physically cannot read past its OBU |
| descriptors → parameter blocks | `&ParamDefinitionRegistry` | **The one place parsing is not a pure function of the current bytes.** It must be an explicit argument, never a hidden field on a stateful parser — otherwise the fuzz target's entry point silently lies about what the parser needs |

---

## Determinism-Safe Structure

### Where a `HashMap` creeps in — and it will, because the reference uses one

Every item below was read in `iamf-tools`. Each is a place a direct port would introduce a hash map.

| Site | Reference | Replacement | Why |
|---|---|---|---|
| Descriptor collections | `DescriptorObus::CodecConfigsById`, `AudioElementsById` | `Vec<T>` in bitstream order + `fn by_id(&self, id) -> Option<&T>` linear scan | Not just non-deterministic — a `BTreeMap` here is *also wrong*, because it reorders by ID and descriptor order is bitstream-visible. `Vec` is the only round-trip-safe choice. N is ≤ 28 by profile, so a linear scan is free. |
| PCM input | `IamfAudioElementData = flat_hash_map<string, Span<const double>>` | `&[(ChannelLabel, &[f64])]`, or planar `&[&[f64]]` in the layout's canonical order | Ordered, allocation-free, and the layout already determines the order |
| Parameter block input | `flat_hash_map<uint32_t, string>` in `IamfTemporalUnitData` | `&[ParameterBlockInput]` sorted by `parameter_id`, or `BTreeMap<ParameterId, _>` | Parameter block emission order within a temporal unit is bitstream-visible |
| Param definition registry (parser) | implicit in `obu_processor` | `BTreeMap<ParameterId, ParamDefinition>` | Lookup-only, never iterated for output — but `BTreeMap` costs nothing and removes the question |
| Label ↔ string tables | `BuildStaticMapFromPairs` (`iamf/common/utils/map_utils.h`) | `match` arms or a sorted `&'static [(K, V)]` + binary search | `const`-evaluable, no static init, no hashing |
| Layout → channel order | `iamf/cli/channel_label.cc` | `const fn channels(layout) -> &'static [ChannelLabel]` | The canonical order *is* the data; a map would only obscure it |
| Substream ↔ channel mapping | scattered | `Vec<(SubstreamId, Vec<ChannelLabel>)>` in bitstream order | Same reasoning as descriptors |

**Enforce it, don't remember it.** Add to `clippy.toml`:

```toml
disallowed-types = [
  { path = "std::collections::HashMap", reason = "non-deterministic iteration; use BTreeMap or Vec" },
  { path = "std::collections::HashSet", reason = "non-deterministic iteration; use BTreeSet or Vec" },
]
```
and run `cargo clippy -- -D warnings` in CI. This turns a constraint that lives in a document into a build failure.

### Other determinism hazards specific to this crate

- **Fixed-point conversion.** Loudness is Q7.8 in the bitstream and arrives from the caller as a float. Convert with integer arithmetic and one documented rounding step; `f32::round` is IEEE-exact and safe, `format!`/`parse` round-trips are not. Never store a float in a model field that has an integer wire representation — store `Q7_8(i16)` and convert at the API boundary.
- **Overflow behaviour.** Debug builds panic on overflow, release wraps. A ULEB128 accumulator that overflows differently in the two profiles is a determinism bug that only appears in release. Use explicit `checked_*` / `saturating_*` in `bits` — which is also required by the no-`unwrap` rule.
- **Iteration over `Vec` is fine; iteration over anything derived from a pointer is not.** No `Box<dyn Any>` keyed collections, no `TypeId` ordering.
- **Feature-dependent output.** Assert that the same input produces the same bytes under `--features encode` and `--features encode,decode`.

### How byte identity is tested across targets

**Layered, and all of it runs in plain `cargo test` with no external tooling:**

1. **Golden hash manifest.** `tests/golden.rs` builds N canonical models in code (not from fixtures — from code, so the test is self-contained), serialises each, and compares the SHA-256 against a checked-in `tests/goldens.sha256`. Cover: LPCM 16/24/32-bit, mono/stereo/5.1.4/7.1.4, `LebPolicy::Minimal` and `Fixed(4)`, with and without temporal delimiters, with an `ArbitraryObu` at each insertion hook.
2. **Cross-target CI matrix.** The *same* test on `macos-14` (arm64), `macos-13` (x86_64), `windows-latest` (MSVC), `ubuntu-latest` (x64). Identical hashes across four targets **is** the byte-identity test; no artefact shipping, no cross-compilation, no comparison step. This is the cheapest possible way to satisfy the "byte-identity test across targets in CI from the first release" requirement, and it satisfies it on day one because it needs nothing but the serialiser.
3. **Both profiles.** Run `--release` as well as debug, to catch overflow-behaviour divergence.
4. **Byte round-trip on own output.** `serialize(parse(serialize(m))) == serialize(m)` — cheap, and catches parser field-drops that model equality might miss if `PartialEq` were ever hand-written.

Prefer hashes over committed `.iamf` blobs for *our own* output: the diff on a change is one line, and a deliberate format change is an obvious, reviewable manifest update. Keep real `.iamf` blobs only for *foreign* input fixtures, where the bytes are the artefact.

---

## Fuzz Target Placement

**`fuzz/` at the repo root, beside `src/`, as an independent workspace.**

```
iamf-rs/
├── Cargo.toml          # exclude = ["fuzz"]  (belt and braces for cargo package)
├── src/
└── fuzz/
    ├── Cargo.toml      # contains an empty [workspace] table
    ├── .gitignore      # artifacts/, corpus/ (or commit a minimised corpus deliberately)
    └── fuzz_targets/
```

```toml
# fuzz/Cargo.toml
[package]
name = "iamf-fuzz"
publish = false
edition = "2024"

[workspace]                     # ← makes fuzz its own workspace root

[dependencies]
libfuzzer-sys = "0.4"           # licence: (MIT OR Apache-2.0) AND NCSA
arbitrary = { version = "1", features = ["derive"] }  # licence: MIT OR Apache-2.0
iamf = { path = "..", default-features = false }
```

### Why an independent workspace

`cargo fuzz init` defaults to making `fuzz/` a member of the existing workspace. Take the independent option instead, for one concrete reason: **`libfuzzer-sys` is licensed `(MIT OR Apache-2.0) AND NCSA`.** NCSA is permissive and BSD-like, but it is not on Parallax's preferred list, so a workspace-member fuzz crate would force an explicit `deny.toml` allow entry for a dependency that never ships. Keeping it out of the root workspace keeps `cargo deny check` at the repo root clean and honest about what actually reaches a consumer. Secondary benefits: `cargo build` at the root never compiles the fuzz crate, and `Cargo.lock` for the library is not perturbed by fuzzing tooling.

### How it stays out of a published package

Three independent guards, because `publish = false` is temporary:

1. `publish = false` on `iamf-fuzz` itself.
2. `exclude = ["fuzz"]` in the library's `[package]`.
3. It is not a workspace member and not a dependency of anything, so no consumer's resolver ever sees it.

`cargo package` already skips directories with their own `Cargo.toml`, but stating it in `exclude` costs one line and removes the question.

### Targets, and when each can land

| Target | Entry point | Earliest possible | Value |
|---|---|---|---|
| `obu_header` | `ObuHeader::read(&mut BitReader::new(data))` | The moment `ObuHeader::read` exists (M2 day 1) | Highest per line: the header is where `obu_size` and the ULEB128 accumulator live, and both are the classic overflow/OOM sites |
| `parse_sequence` | `IaSequence::parse(data)` — the whole file, registry built from the same input | With the full parser (M2) | The realistic hostile-input surface, and the only one that exercises the descriptor → parameter-block dependency honestly |
| `round_trip` | `#[derive(Arbitrary)]` on the model → `serialize` → `parse` → `assert_eq!` | With the parser (M2) | Structure-aware; finds *encoder* bugs and asymmetries that byte-input fuzzing reaches only by luck |

The `round_trip` target is why `#[derive(Arbitrary)]` should be available on the model behind an `arbitrary` feature — additive, dev-only, and it is the mechanism that turns the round-trip requirement from a handful of hand-written cases into continuous property testing.

### Two architectural rules the parser must satisfy for fuzzing to work

1. **No allocation sized by an unvalidated bitstream value.** `Vec::with_capacity(n)` where `n` is a freshly-decoded ULEB128 is the single most common OOM in bitstream fuzzing, and a ULEB128 can encode a number far larger than the input. Rule: `BitReader` exposes `remaining()`, and every count-prefixed allocation is bounded by it before reserving — or simply pushes without reserving. This is a design constraint on `bits`, decided before the first OBU is written, not a patch applied after the first crash.
2. **The registry is an argument, not hidden state.** The `parse_sequence` target must build `ParamDefinitionRegistry` from the same fuzz input it parses. If the parser held the registry as private state and exposed `parse(&[u8]) -> Obu`, the target would either not reach Parameter Block parsing at all or would need a fabricated registry that does not correspond to the input — in both cases the fuzzer would be testing the wrong thing while looking healthy.

Seed the corpus from `tests/fixtures/*.iamf`. `--cfg fuzzing` is set for the whole tree, so any deliberately expensive validation can be relaxed under it — but resist using it to skip bounds checks, which are the point.

---

## Testing Against the C++ Reference

Four options, weighed. **Recommendation: golden fixtures everywhere plus a `Command`-shelled reference decoder in one pinned CI job. Not a build script. Not FFI, not yet.**

| Option | Fidelity | CI portability | Verdict |
|---|---|---|---|
| **Build script compiles `libiamf`** | High | Very poor | **Reject.** `libiamf` uses git submodules (`.gitmodules`) for its codec dependencies under `code/dep_codecs/`, plus CMake. A `build.rs` that can fail on a network fetch blocks `cargo check` on every developer machine and every CI job, including the ones that only want to run unit tests. Build-script failure is total: it stops the crate compiling at all. |
| **FFI bindings to `code/include/IAMF_decoder.h`** | Highest | Poor now | **Defer.** `bindgen` needs `libclang` on four targets, and `libiamf` is not packaged anywhere — every job would build it from source. Revisit only if the answer to "is a native decoder wanted" is *no* and shelling out proves too coarse. |
| **`Command`-shell `iamfdec`** | High — it is literally the reference decoder | Good, if isolated | **Adopt.** `libiamf` ships `code/test/tools/iamfdec/` with `test_iamfdec.c`, a CLI test decoder. Shell it, decode to WAV, compare PCM. |
| **Golden files captured once** | Medium — proves agreement at a point in time | Excellent | **Adopt, in addition.** This is what makes `cargo test` work offline on all four targets. |

### Concretely

**Layer 1 — always, everywhere, no external tooling.**
`tests/golden.rs` (own-output SHA-256, cross-target), `tests/round_trip.rs`, `tests/parse_reference.rs` reading `tests/fixtures/*.iamf`.

An important practical finding: **`iamf-tools` does not ship a ready-made `.iamf` corpus.** `iamf/cli/testdata/` contains 338 `.textproto` descriptions and 25 `.wav` inputs but only **one** `.iamf` file — the binaries are generated by running the encoder. Neither `iamf-tools` nor `libiamf` attaches a test-vector bundle to its GitHub releases. So the fixture corpus has a real one-time cost: **build `iamf-tools` with Bazel once, run a selected subset of the textprotos through `encoder_main`, and commit the resulting `.iamf` files here** under `tests/fixtures/`, with a `README.md` recording the `iamf-tools` tag (v2.1.0), the commit SHA, and the exact command that regenerates them. Because it is external tooling work with its own setup latency and it is a hard prerequisite for the M2 requirement "parse `iamf-tools`-produced files", **start it early — it is not on the code critical path and it should not be discovered late.**

**Layer 2 — one CI job, pinned.**
`tests/reference_decoder.rs` reads `IAMF_REF_DECODER` from the environment. Unset ⇒ the test prints a skip reason and passes, so `cargo test` is green for anyone without a C toolchain. Set ⇒ it encodes a canonical file, shells `iamfdec` to WAV, and asserts the PCM is sample-identical for LPCM (`libiamf`'s own `tests/run_decode_and_psnr_test.py` compares by PSNR, which is right for lossy codecs; for LPCM, demand exactness). One `ubuntu-latest` job builds `libiamf` from a pinned commit SHA — cached — and sets the variable. **This is the M1 success criterion**, so it is not optional, but it is confined to one job on one platform and cannot break the other three.

**Layer 3 — later, optional.**
If a native decoder is built (open question 3), a `#[cfg(feature = "reference-ffi")]` bindgen path becomes worthwhile for differential fuzzing: feed the same bytes to both decoders and compare. Not before then.

**Why the split earns its keep:** fidelity is needed *once per format change*, portability is needed *on every commit*. Layer 1 gives every commit a fast, hermetic, four-target check. Layer 2 gives the format change the real reference decoder. Neither compromises for the other, and no developer needs Bazel or CMake installed to contribute a parser fix.

---

## Build Order

### Strictly sequential spine

1. **`error` + `types`** — everything depends on them. Small; do them together.
2. **`bits`** — `BitReader`, `BitWriter`, ULEB128/SLEB128, `LebPolicy`. **Nothing above this can be written or tested without it.** This is where the handoff's explicit instruction lands: *verify the exact ULEB128 semantics and whether `obu_size` includes the header, by reading `iamf/common/leb_generator.h` and `iamf/obu/obu_header.cc`, before writing any OBU.* Getting this wrong produces files that almost work, and the error surfaces only at the reference decoder.
3. **`obu::header`** — every OBU depends on it; `obu_size` semantics gate the whole crate. Land `ObuHeader::read` and the `obu_header` fuzz target in the same change.
4. **Descriptor OBUs, in dependency order:** `ia_sequence_header` (trivial, any time after 3) → `codec_config` + `decoder_config::lpcm` → `audio_element` → `mix_presentation`. This order is forced: Audio Element references a Codec Config ID; Mix Presentation references Audio Element IDs; both embed `param_definition`s.
5. **`param_definition`** — must be complete before `parameter_block`, because it is the parse context. It arrives naturally with step 4, since definitions live inside those descriptors.
6. **Time-varying OBUs:** `temporal_delimiter` (trivial), `audio_frame` (needs the type 5..=23 implicit-ID handling), `parameter_block` (requires 5).
7. **`codec::lpcm` framing** — depends on `decoder_config::lpcm`.
8. **`sequence::write` + `encode::{plan, mod}`** — depends on all of 4, 6, 7. **This is the end of M1** and the first point at which the reference decoder can be pointed at anything.
9. **`sequence::read` + `ParamDefinitionRegistry`** — same model, opposite direction. **M2.**
10. **`fuzz/parse_sequence` + `fuzz/round_trip`** — depend on 9. M2, non-negotiably per the constraint.
11. **`codec::flac`, `codec::opus`** — depend on the `codec::Framer` trait from 7, independent of each other. **M3.**
12. **`encode::curve` (if chosen)** — strictly above 8.
13. **ISO-BMFF** — a second `sequence` backend, above 8, independent of 9 and 11.

### What can run in parallel

| Track | Can start | Independent of |
|---|---|---|
| `labels` + `profile` | After `types` (step 1) | `bits`, `obu::header` — pure enums and rules, no serialisation |
| **Fixture generation** (Bazel build of `iamf-tools`, run textprotos, commit `.iamf`) | Day 1 | The entire codebase. External tooling with its own latency; a hard prerequisite for step 9's acceptance test. **Start it first.** |
| **Reference-decoder CI job** (pinned `libiamf` build, cached) | Day 1 | The codebase. Must exist before step 8 can be declared done. |
| **Cross-target CI matrix + `goldens.sha256`** | As soon as step 8 writes one file | Everything else. Satisfies "byte-identity from the first release" immediately. |
| `codec::flac` ∥ `codec::opus` | After step 7 | Each other, entirely |
| `sequence::read` for *descriptors* | After step 4 | Can overlap step 6/8; only `parameter_block::read` is blocked on step 5 being settled |
| `deny.toml` + `clippy.toml` disallowed-types | Day 1 | Everything. Cheapest possible time to install the determinism and licence guardrails — retrofitting them means fixing violations rather than never writing them |

### The two orderings that matter most

- **`bits` before anything, and verified against source before anything.** It is the smallest module and the one whose errors are most expensive and latest-discovered.
- **Both directions of OBU serde in the core, together, per type.** Writing all the writers first and all the readers in M2 sounds like it matches the milestones, but it costs a full re-read of every OBU file and defers every round-trip bug to a single painful milestone. Better: M1 ships writers for all types plus readers wherever they are free (headers, fixed-layout descriptors), M2 completes the readers. The milestone gate is "the reference decodes our file" (M1) and "round trip + fuzz" (M2); neither forbids landing a reader early, and landing readers early makes M1's own testing far better.

---

## Anti-Patterns

### 1. A stateful `Parser` that hides the descriptor dependency

**What people do:** `struct Parser { registry: ParamDefinitionRegistry }` with `fn next(&mut self) -> Option<Result<Obu>>`, so the registry is invisible in the signature.
**Why it's wrong:** it hides that Parameter Block parsing is not a function of the current bytes. The fuzz target then either misses type 3 entirely or fabricates a registry, and looks healthy while testing nothing. It also makes it impossible to parse a single OBU out of context — which is exactly what a test wants to do.
**Instead:** `ReadObu::Ctx<'a>`. `()` for everything except Parameter Block, `&'a ParamDefinitionRegistry` for that one. The requirement is in the type.

### 2. `#[derive(Serialize, Deserialize)]` as the bitstream

**What people do:** reach for serde because the model is already a struct tree.
**Why it's wrong:** serde's data model cannot express sub-byte fields, context-dependent lengths, or a caller-chosen ULEB128 width. A `bincode`-shaped output is not IAMF and never will be.
**Instead:** hand-written `read_payload`/`write_payload`. Serde is still welcome behind a `serde` feature for a **debug JSON dump** — `iamf-tools` has `iamf/cli/probe_json.cc` for exactly that use, and it is genuinely useful for diffing our model against the reference's `probe` output. Name it so nobody confuses the two.

### 3. Rejecting reserved bits

**What people do:** `if reserved != 0 { return Err(...) }`.
**Why it's wrong:** the spec says ignore unrecognised trailing data, not reject it, and a future minor version that defines a reserved bit turns this crate from "reads new files with a shrug" into "reads nothing".
**Instead:** model reserved bits as fields, write back what was read, default them to zero on construction.

### 4. Discarding the observed `obu_size` encoding

**What people do:** parse `obu_size`, use it for bounds, throw the encoding away, re-emit minimal.
**Why it's wrong:** byte round-trip against reference output then fails whenever the reference used `kFixedSize`, and the failure looks like a serialiser bug rather than a policy mismatch.
**Instead:** record the `LebPolicy` at the sequence level, and be explicit in the documentation that byte-identity against foreign files requires a matching policy.

### 5. `Vec::with_capacity(n)` where `n` came from the bitstream

**What people do:** read a count, reserve, then loop.
**Why it's wrong:** a ULEB128 can encode a number vastly larger than the input, so the fuzzer finds an OOM within minutes.
**Instead:** bound every count against `BitReader::remaining()` before reserving, or push without reserving. Decide this in `bits`, before the first OBU.

### 6. Layouts as anything but labels

**What people do:** attach speaker azimuths or downmix coefficients to `LoudspeakerLayout`, "since we have the enum anyway".
**Why it's wrong:** that is Parallax's job under `D-40`, it is explicitly out of scope, and it drags `libm` and a determinism surface into a crate that currently has neither.
**Instead:** `LoudspeakerLayout` carries a discriminant and a canonical channel order. Note that newer IAMF parameter data *does* carry positions (`iamf/obu/{polar,cart8,cart16}_parameter_data.h`) — those are **numbers carried through the bitstream, not numbers computed here**. Carrying is in scope; computing is not.

### 7. `HashMap<Id, Obu>` — and `BTreeMap<Id, Obu>` too

**What people do:** port `DescriptorObus::AudioElementsById` directly, then "fix determinism" by swapping to `BTreeMap`.
**Why it's wrong:** `HashMap` is non-deterministic; `BTreeMap` is deterministic but silently reorders descriptors by ID, and descriptor order is bitstream-visible, so it breaks round-trip instead. The swap looks like a fix and is a different bug.
**Instead:** `Vec` in bitstream order plus a `by_id` accessor. `BTreeMap` is correct only for lookup-only structures that never drive output order, such as `ParamDefinitionRegistry`.

### 8. A `build.rs` that compiles C

**What people do:** vendor `libiamf` and build it from the build script so the conformance test "just works".
**Why it's wrong:** build-script failure blocks `cargo check`, `cargo clippy`, `rust-analyzer` and every unit test, on every machine, including ones that will never run the conformance test. It also drags git submodules and CMake into a Rust crate's dependency story and puts a large C tree under `cargo deny`'s nose.
**Instead:** env-var-gated `Command` shell-out, one CI job, pinned SHA.

### 9. Building the encoder first and the parser second because the milestones say so

**What people do:** read "M1 encoder, M2 parser" as "no reader code before M2".
**Why it's wrong:** it defers every round-trip bug to one milestone and forces a second pass over every OBU file. It also gives M1 weaker tests than it could have had for free.
**Instead:** land the reader for each OBU type alongside its writer wherever it is cheap, and let M2 be about the *sequence-level* parser, the registry, the round-trip property and the fuzz target — which is where the genuinely new work is.

---

## Scaling Considerations

Not user counts — stream size and stream complexity are what break first.

| Scale | What breaks | Architectural response |
|---|---|---|
| A few seconds, one element | Nothing | Whole-file `encode_to_writer` is fine |
| Minutes to hours, 7.1.4 24-bit (~4 GB) | Whole-file assembly in `Vec<u8>` | Already handled: streaming push/pull is the primitive, whole-file is the wrapper. `take_output(&mut Vec<u8>)` reuses one buffer across temporal units |
| 28 elements / 28 channels (Base-Enhanced ceiling) | `by_id` linear scans, per-temporal-unit allocation | N ≤ 28 by profile, so scans stay free. Allocation is the real cost: reuse the substream byte buffers across temporal units, as the reference's API comments explicitly recommend |
| High parameter rate (per-sample position) | Parameter block volume can exceed audio frame volume | This is precisely the tick-rate question. Whatever the answer, the crate should be able to *report* parameter-block byte overhead so Parallax can make an informed choice |
| Parsing a hostile or truncated 4 GB file | Unbounded allocation, quadratic scans | `BitReader` borrows and never copies; every count is bounded by `remaining()`; streaming `ObuIter` so a caller need not materialise the whole `IaSequence` |

**First bottleneck in practice:** per-temporal-unit allocation on the encode path. Fix by making `push_temporal_unit`/`take_output` operate on caller-owned reusable buffers — which the recommended API already does.

**Second:** codec framing throughput for FLAC/Opus, which is not this crate's code and is fixed by choosing the binding, not the architecture.

---

## Integration Points

### External

| Dependency | Pattern | Notes |
|---|---|---|
| **Parallax** | Path dependency; `iamf::encode` public API | The only consumer. Do not build ABI-stability machinery (traits + factories) for it — that is why `iamf-tools` has `iamf/include/` and why this crate should not |
| **`libiamf` (BSD-3-Clause-Clear)** | `Command` shell-out to `code/test/tools/iamfdec`, env-var gated, pinned SHA, one CI job | Never a build-script or FFI dependency in M1. Its codec deps are git submodules |
| **`iamf-tools` (BSD + AOM PL 1.0)** | Read for facts; run once offline to generate `tests/fixtures/*.iamf` | Never a build-time or runtime dependency. Only 1 `.iamf` is checked in upstream and no release ships a corpus, so the fixtures are ours to generate and commit |
| **libFLAC / libopus bindings** | Behind `flac` / `opus` features | Licence stated on the line that adds each; `cargo deny check --all-features` |
| **`cargo-deny`** | CI, with Parallax's allow-list | Runs at the repo root, which the independent fuzz workspace keeps free of `libfuzzer-sys`'s NCSA clause |

### Internal boundaries

| Boundary | Communication | Considerations |
|---|---|---|
| `encode` → `obu` | Direct construction of public structs | One-directional. `obu` must never mention `encode` |
| `encode` → `codec` | `Framer` trait, `&[&[f64]]` planar in, `Vec<u8>` + delay hint out | The only place PCM exists. Framing is not DSP; quantisation lives here so it is deterministic and singular |
| `sequence::read` → `obu` | `ReadObu` with `Ctx` | The registry crosses here and nowhere else |
| `obu` → `bits` | `BitReader`/`BitWriter` + `LebPolicy` | The only place bit offsets exist |
| `obu::parameter_block` → `obu::param_definition` | `&ParamDefinitionRegistry` as an explicit argument | The one non-local coupling in the whole crate; keep it visible in every signature that has it |
| core → `encode`/`decode` | **Forbidden** | Gated modules depend on the core, never the reverse. This is what keeps every feature subset compiling |
| `fuzz` → `iamf` | Path dependency, `default-features = false` | Possible only because the parser is in the ungated core |

---

## Sources

Read directly (paths given so claims can be re-verified):

- `AOMediaCodec/iamf-tools` @ `main` (v2.1.0), BSD-3-Clause-Clear + AOM PL 1.0 — `iamf/obu/obu_base.h` (`ValidateAndWritePayload`, `ReadAndValidatePayloadDerived`, `footer_` and its §3.2 rationale), `iamf/obu/obu_header.h` (`ObuType` 0–31 incl. audio-frame IDs 6–23 and Metadata 24, flag accessors, `HeaderMetadata`), `iamf/obu/parameter_block.h` (`PeekParameterId`, `CreateMode0/1`, `CreateFromBuffer(…, param_definition, …)`), `iamf/obu/arbitrary_obu.h` (`InsertionHook`, `invalidates_bitstream`), `iamf/obu/extension_parameter_data.h`, `iamf/common/leb_generator.h` (`kMinimum` / `kFixedSize(1..8)`), `iamf/common/write_bit_buffer.h`, `iamf/cli/obu_sequencer_base.h` (push/pull lifecycle), `iamf/include/iamf_tools/iamf_encoder_interface.h` (streaming contract, redundant descriptor copies, loudness known only at finalisation, `GetEncoderDelay`), `iamf/include/iamf_tools/iamf_tools_encoder_api_types.h` (`flat_hash_map`, `Span<const double>` per channel label), plus full recursive tree listings of `iamf/{obu,common,cli,api,include}` and `iamf/cli/testdata` (368 entries: 338 `.textproto`, 25 `.wav`, **1 `.iamf`**), and `docs/build_instructions.md` (stated folder structure). Confidence **HIGH**.
- `AOMediaCodec/libiamf` @ `main`, BSD-3-Clause-Clear — repo tree: `code/include/IAMF_decoder.h`, `code/src/iamf_dec/`, `code/dep_codecs/` (git submodules), `code/test/tools/iamfdec/src/test_iamfdec.c`, `tests/run_decode_and_psnr_test.py`, `tests/coverage.csv`, `tests/proto/*.proto`. Confidence **HIGH**.
- `alfg/mp4-rust` (MIT) — `src/` tree: `mp4box/<box>.rs` one file per box, `reader.rs`, `writer.rs`, `types.rs`, `error.rs`. Confidence **HIGH**.
- `pdeljanov/Symphonia` (MPL-2.0, **structure only, nothing ported**) — crate split `symphonia-core` / `symphonia-format-*` / `symphonia-codec-*`, and `symphonia/fuzz`. Confidence **HIGH**.
- `jam1garner/binrw` (MIT) via Context7 — `#[binrw]` derive, `#[br(import{..})]`/`#[bw(args(..))]` context passing, `#[br(temp)]`, `BinRead`/`BinWrite` requiring `Read + Seek`. Confidence **MEDIUM**.
- crates.io API — licences: `bitstream-io` 4.10.0 `MIT/Apache-2.0`, `deku` 0.20.3, `binrw` 0.15.2 `MIT`, `arbitrary` 1.4.2, **`libfuzzer-sys` 0.4.13 `(MIT OR Apache-2.0) AND NCSA`**, `thiserror` 2.0.20. Confidence **HIGH**.
- The Cargo Book (features must be additive), RFC 2957 (resolver v2), `rust-fuzz/cargo-fuzz` README (`fuzz/` placement, independent vs member workspace, `--cfg fuzzing`), docs.rs `doc_cfg` conventions. Confidence **MEDIUM** (web search).

Not consulted, by constraint: `libspatialaudio` (LGPL-2.1+), `gpac` (LGPL-2.1).

---
*Architecture research for: IAMF bitstream codec library in Rust*
*Researched: 2026-09-07*
