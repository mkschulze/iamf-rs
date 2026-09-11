# iamf-rs

A Rust implementation of the IAMF (Immersive Audio Model and Formats) bitstream — OBU serialiser and
parser, descriptor model, and an encoder producing conformant `.iamf` files.

Not a renderer. This crate receives prepared PCM/access units plus metadata and produces bytes; it never pans,
places a source, or treats a speaker layout as anything but a label.

The Phase 4 API is **Parallax-facing but host-independent**. `iamf-rs` owns validation, deterministic
wire IDs/profile selection and standalone IA Sequence writing. Parallax owns its delivery projection,
Preview/Export-Include state, codec encoding and the production adapter; `iamf-render-rs` owns OAR
loudspeaker/binaural rendering, and `iamf-decode-rs` owns native codec decoding plus timed Audio
Element reconstruction. No Parallax type, renderer or decoder implementation belongs in this crate.

Targets **IAMF v1.1.0** (`iamf::SPEC_VERSION`).

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
