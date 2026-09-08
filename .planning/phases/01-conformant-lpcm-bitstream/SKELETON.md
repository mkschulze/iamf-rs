# Walking Skeleton — iamf-rs

**Phase:** 1
**Generated:** 2026-09-08

> **Domain translation.** The canonical Walking Skeleton is web-shaped (scaffold → routing → DB
> read/write → UI interaction → dev deploy). This project is a Rust library: no UI (PROJECT.md
> lists "UI of any kind" as out of scope), no database, no deployment. The *concept* — the thinnest
> possible end-to-end working slice — translates to the axis that actually exists here:
>
> **a configuration in memory → bytes on disk → a reference decoder → PCM that matches the input.**
>
> That chain is the Core Value statement, restated as an executable path.

## Capability Proven End-to-End

A caller hands the crate a channel-based LPCM configuration and interleaved PCM; the crate writes a
standalone `.iamf` file; `libiamf`'s `iamfdec` at the pinned SHA decodes it back to PCM that is
**sample-identical** to the input, with the decoded sample count equal to the encoded count.

## The Chain, and Where Each Link Is Proven

| # | Link | Owning plan | The proof that the link exists |
|---|---|---|---|
| 0 | The **far end** — the oracle itself, and the comparison logic | 01-02 | `iamfdec` built at `f06e919e` decodes the shipped `test_000003.iamf` to a WAV sample-identical with `sawtooth_100_stereo.wav`: 8000 frames, 0 differing samples of 16000. **This runs before a single byte of ours exists** — it proves the harness's comparison logic on a file we did not write. |
| 1 | bits → bytes | 01-03 | `BitWriter` emits `0xF8` from `write_unsigned(31, 5)` + three `write_bool(false)`; `BitCursor` reads it back. |
| 2 | bytes → one OBU | 01-04 | The IA Sequence Header round-trips as `f8 06 69 61 6d 66 00 00` — header byte, two-pass `obu_size`, payload. |
| 3 | one OBU → the descriptor set | 01-05 | The complete 120-byte descriptor prologue of `test_000003.iamf` is reproduced byte-exact from its published `test_000003.textproto`. |
| 4 | descriptors → temporal units | 01-06 | An untrimmed Audio Frame (`30 80 04 …`) and the trimmed final frame (`32 82 04 40 00`) are reproduced byte-exact. |
| 5 | temporal units → a whole file | 01-07 | `push_descriptors` → `push_temporal_unit` → `finish` reproduces all 32567 bytes of `test_000003.iamf`, 67 OBUs, final boundary landing exactly on `len()`. |
| 6 | our file → the oracle → PCM | 01-08 | `assert_conformant(config, pcm)` closes the loop on our *own* 5.1 24-bit fixture through both reference implementations. |

Links 0 and 6 are the two ends. Everything between them is a layer that link 0 has already proved
is reachable, which is why the external-tooling track (01-02) runs on day one rather than last.

## Architectural Decisions

These are the decisions later phases inherit. They are locked by `01-CONTEXT.md` and are recorded
here so Phase 2, 3 and 4 build on them rather than re-litigating them.

| Decision | Choice | Rationale |
|---|---|---|
| Language / edition | Rust 2024, toolchain pinned by `rust-toolchain.toml` | GUARD-12. The four-target byte-identity matrix must compile identically everywhere. |
| Shipping dependency graph | `thiserror` **only** | D-02: "dependency-free" means zero *linked* dependencies; proc-macro derive crates excepted. `thiserror`'s whole chain is proc-macro. |
| Bit I/O | Hand-rolled `BitCursor` / `BitWriter` over `&[u8]` in `src/bits/` | D-01. Produces byte offsets natively, which is what every `Error.at` value flows from. `bitstream-io` is a **dev-dependency** used as a proptest differential oracle and never enters the shipping graph. |
| Parser shape | Hand-written recursive descent, `read` and `write` adjacent in the same file, in that order, in the same commit | D-10 + D-23. Asymmetry becomes visually obvious; a mechanical test enforces a `// ref:` citation on every `fn read_*` / `fn write_*`. |
| Error surface | `struct Error { kind: ErrorKind, at: Location }`, `size_of::<Error>() <= 32` | D-08 (**one-way** — Parallax's import adapter matches on it). Position attached once, not repeated into every variant. |
| Validation | `validate() -> Vec<Finding>`, explicit, never implicit; the writer is faithful; `build()` is Phase 4's mandatory point | D-07 + D-09. "Refuse with the reason named, never approximate." |
| Descriptor collections | `Vec` in bitstream order + `by_id()` | DESC-09. `BTreeMap` would reorder Mix Presentations, which `obu_sequencer_base.cc` writes in *list* order. |
| Element type model | `#[non_exhaustive] enum AudioElementType { ChannelBased, SceneBased, Reserved { value, raw } }` | D-04 (**one-way**). Decode is a superset of encode. `Reserved` buys a testable byte-identity property, not just forward compatibility. |
| Cargo features | **None in Phase 1** | D-22. Nothing to gate, and a cfg matrix would multiply the four-target byte-identity gate. |
| Reference oracles | `Command`-in-tests, never `build.rs`. `libiamf` built natively (`tools/build-reference.sh`); `iamf-tools` only in a digest-pinned container | CONF-09 + D-11. A build script would force CMake + abseil + protobuf onto every consumer's `cargo build`, Parallax's included. |
| Pinned references | `iamf-tools` **v2.1.0** `848c6ff4968ff8cc6f728259892ab4f90cb83256`; `libiamf` **v1.1.0** `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` | DEC-04, settled by research. Both are IAMF v1.1.0-exact trees by four discriminating checks; `main` fails all four. |
| Spec version | `SPEC_VERSION = "1.1.0"` | DEC-01/GUARD-05. `libiamf` — whose acceptance *is* the Core Value — is a v1.1.0 decoder. |
| Directory layout | `src/{bits,obu,model}/`, `src/{error,packing,sequence,dump}.rs`, `tests/`, `tools/` | Claude's discretion per CONTEXT.md, constrained by BITS-07 (only `src/bits/` knows bit offsets) and D-10 (read/write pairing). |

## Stack Touched in Phase 1

- [ ] Crate scaffold — `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `deny.toml`, `src/lib.rs`
- [ ] Every guardrail live **and proven to bite** before there is code to retrofit (`tools/prove-guards.sh`)
- [ ] Reference oracle built and self-validated against a file we did not write (`tools/build-reference.sh`)
- [ ] Bit layer — real read/write pair, hand-computed vectors
- [ ] OBU framing — real two-pass `obu_size`, real boundary walk over 200+ vendored reference files
- [ ] Descriptors, temporal units, whole-file sequence writer — each reproducing published reference bytes
- [ ] `assert_conformant(config, pcm)` — the seven-clause exit gate, reusable unchanged by Phase 3
- [ ] Four-target CI matrix (macOS arm64, macOS x86_64, Windows MSVC, Linux x64) with a committed golden

## Out of Scope (Deferred to Later Slices)

Explicit, so a later phase does not re-litigate Phase 1's minimalism:

- **Sequence-level parser, `ParamDefinitionRegistry`, round-trip properties, the fuzzer** — Phase 2.
  Phase 1 *does* land `read` beside `write` per OBU type (D-10), but round-trip tests are
  supplementary evidence only; hand-decoded vectors and the reference oracles are the primary proof.
- **FLAC and Opus framing** — Phase 3, through this same `assert_conformant` harness, unmodified.
- **`EncoderBuilder` / `build()` / the Parallax-facing API surface** — Phase 4.
- **ffmpeg as a third read oracle** — Phase 2. Phase 1 writes down the licence boundary only (D-15).
- **Multi-layer (BCG/DCG ladder), recon gain, demixing weights, object-based elements, ambisonics
  projection** — v2 or out of scope entirely.
- **ISO-BMFF, a native decoder, publishing to crates.io** — v2.
- **Any rendering, panning, mixing, resampling or spatial DSP** — Parallax owns it under `D-40`.
  The Phase 1 fixture is deliberately designed (D-18) so `iamfdec`'s 5.1→Sound System B render
  matrix is the 6×6 identity, which is what makes sample-identity a direct comparison.

## Subsequent Slice Plan

Each later phase adds one vertical slice on top of this skeleton without altering its architectural
decisions:

- **Phase 2** — the bytes come back: sequence-level parser, round-trip both directions, verbatim
  preservation of unknown OBUs and unknown parameter data, and a continuously-run fuzzer.
- **Phase 3** — two more codecs through the unchanged harness: FLAC STREAMINFO and Opus OpusHead
  `decoder_config`, with zero codec crates in the shipping graph.
- **Phase 4** — the surface Parallax calls: one validated `build()`, minimum-profile selection,
  pre-decimated parameter blocks, append-only `W: Write`.
