# iamf-rs

![Abstract spatial-audio wavefront and IAMF packet structure](docs/assets/iamf-header.png)

[![Beta release](https://img.shields.io/github/v/release/mkschulze/iamf-rs?include_prereleases&display_name=tag&color=0ea5e9)](https://github.com/mkschulze/iamf-rs/releases/tag/v0.1.0-beta.1)
[![CI](https://github.com/mkschulze/iamf-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/mkschulze/iamf-rs/actions/workflows/ci.yml)
[![Rust 1.85](https://img.shields.io/badge/Rust-1.85.0-dea584?logo=rust)](rust-toolchain.toml)
[![IAMF 1.1.0](https://img.shields.io/badge/IAMF-1.1.0-06b6d4)](https://aomediacodec.github.io/iamf/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-22c55e)](LICENSE-APACHE)

A Rust implementation of the IAMF (Immersive Audio Model and Formats) bitstream — OBU serialiser and
parser, descriptor model, and an encoder producing conformant `.iamf` files.

Not a renderer. This crate receives prepared PCM/access units plus metadata and produces bytes; it never pans,
places a source, or treats a speaker layout as anything but a label.

The high-level API is **host-independent**. `iamf-rs` owns validation,
deterministic wire IDs/profile selection and standalone IA Sequence writing.
The calling application owns delivery selection, codec encoding and its
adapter. Rendering and native codec decoding are separate responsibilities;
no renderer or decoder implementation belongs in this crate.

Targets **IAMF v1.1.0** (`iamf::SPEC_VERSION`).

## Release status

**v0.1.0-beta.1** is the first beta release. The v1 encoder API, parser and
standalone IA Sequence writer are complete and covered by the repository's
deterministic, parser and reference-conformance gates. The crate remains
`publish = false`: this beta is a Git release for integration testing, not a
crates.io publication. Downstream users should validate their own delivery
adapter and target playback stack before treating it as a production release.

## Consumer boundary

The public export seam is deliberately host-independent. A consumer first declares an ordered,
static IAMF configuration with `EncoderBuilder`; `build()` validates that complete declaration set
and returns an immutable `Encoder` plus an `IdManifest`. The manifest maps only caller-local opaque
handles to deterministic IAMF wire IDs, so host IDs do not cross this crate boundary.

`Encoder::start(W: Write)` writes the frozen descriptor prologue to a caller-owned sink. Each
`TemporalUnitInput` is fully checked before it is appended, then `finish(self)` consumes the writer
and returns the sink. Frames are either LPCM bytes or pre-encoded FLAC, Opus, or AAC-LC access units.
For AAC-LC, each IAMF Audio Frame carries one pre-encoded `raw_data_block()`; this crate neither
encodes nor decodes AAC. The caller also supplies loudness values and already-decimated IAMF
parameter blocks; this crate carries and validates them but does not derive them.

The production adapter belongs to the calling application, which filters its
immutable delivery snapshot and owns delivery UI state, source identity,
panning/HOA preparation, codec encoding, resampling, loudness measurement,
timeline interpolation, rendering, and carrier/container work. This crate
performs none of those responsibilities.

Public Rust snippets intentionally use only this crate's surface. In
particular, they contain no application-specific type or import,
source-position model, UI-state field, codec-encoder dependency, or renderer
dependency:

```rust
let builder = iamf::encoder::EncoderBuilder::new();
```

The no-default-features contract keeps this builder and the complete production bitstream API
available. `thiserror` is the only normal non-proc-macro dependency; see the reproducible command in
[HANDOFF.md](HANDOFF.md).

## Reference implementations

The correctness claim is not "our tests pass" — it is that a file this crate
writes is read back by the reference decoder `libiamf` with the PCM
sample-identical, and is accepted by `iamf-tools`' stricter parser.

Both references are pinned by commit, never by tag alone and never to `main`.
The SHAs, the four checks that establish they are IAMF v1.1.0-exact trees, and
the read-versus-invoke licence boundary that governs which reference source may
be opened at all are in **[REFERENCES.md](REFERENCES.md)**.

Nothing in a normal `cargo build` or `cargo test` needs a reference binary —
the suite is green offline on all four supported targets, deliberately.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The pull-request checklist is short and
the first item is the one that matters: `gpac`, `libspatialaudio` and FFmpeg /
`libavformat` are LGPL and their source may not be read or ported. Invoking a
compiled binary from a test is fine; opening the source is not, and
contamination is irreversible.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Apache-2.0 is included because it carries a patent grant that MIT does not, which matters for an
implementation of a standard with an explicit patent pool. See `NOTICE` for attribution of code
ported from the permissively-licensed reference implementations.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without any
additional terms or conditions.

### The `PATENTS` file

[`PATENTS`](PATENTS) holds the Alliance for Open Media Patent License 1.0
verbatim. It sits **alongside** the dual `MIT OR Apache-2.0` licence above, not
instead of it, and it is not this project's outbound licence.

It is there because §1.2.1(a) of that licence makes reproducing it "in the root
directory of the source code" a **condition of the inbound grant**: `iamf-rs`
produces an IAMF bitstream, which makes it an Encoder (§2.4) and therefore an
Implementation (§2.6). Omitting the file would cost us the patent licence we
rely on. Apache-2.0 §3's patent grant and AOM §1.1 are independent of each
other.

Keep `PATENTS` in the published archive if `publish = false` is ever lifted —
§1.2.1(a) speaks about the root directory of the source code as distributed.
