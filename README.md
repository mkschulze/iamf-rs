# iamf-rs

A Rust implementation of the IAMF (Immersive Audio Model and Formats) bitstream — OBU serialiser and
parser, descriptor model, and an encoder producing conformant `.iamf` files.

Not a renderer. This crate receives rendered PCM plus metadata and produces bytes; it never pans,
places a source, or treats a speaker layout as anything but a label.

Targets **IAMF v1.1.0**.

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
