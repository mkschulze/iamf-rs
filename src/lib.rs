#![forbid(unsafe_code)]
//! A Rust implementation of the **IAMF** (Immersive Audio Model and Formats)
//! bitstream — OBU serialiser and parser, descriptor model, and an encoder
//! producing conformant `.iamf` files.
//!
//! Not a renderer. This crate receives rendered PCM plus metadata and produces
//! bytes; it never pans, places a source, or treats a speaker layout as
//! anything but a label.
//!
//! Targets IAMF [`SPEC_VERSION`].
//!
//! # What "dependency-free" means here (D-02)
//!
//! API-05 calls this crate dependency-free. That means **zero *linked*
//! dependencies — proc-macro derive crates excepted**. `thiserror` is
//! unconditional (PROJECT.md: never `anyhow` in a library) and pulls
//! `thiserror-impl -> syn -> proc-macro2 -> quote -> unicode-ident`; none of
//! those contribute code to the linked artifact. `cargo tree -e normal` is the
//! check, and it lists `iamf` and `thiserror` and nothing else.
//!
//! This definition is written down here, and again in `REFERENCES.md`, so a
//! later milestone does not rediscover it as a contradiction and "fix" it by
//! dropping `thiserror`.
//!
//! # Guardrails
//!
//! `#![forbid(unsafe_code)]` above is restated from `Cargo.toml`'s
//! `[lints.rust]` table for readers who never open the manifest. The hardening
//! set (`clippy::indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`,
//! `expect_used`, `panic`, `disallowed_types`, `disallowed_methods`) lives in
//! `Cargo.toml` so it applies to every target, and is configured by
//! `clippy.toml`. `tools/prove-guards.sh` proves each one fires on a
//! deliberate violation rather than merely being configured.

/// The IAMF specification version this crate implements (GUARD-05, DEC-01).
///
/// **`"1.1.0"`.** The reason is the Core Value: a file this crate writes must
/// be read back by `libiamf`, and `libiamf`'s pinned release
/// (`f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63`, tag `v1.1.0`) implements IAMF
/// v1.1.0. Base-Enhanced profile does not exist in v1.0 at all, so targeting
/// v1.0 would forbid a profile the reference decoder accepts.
///
/// This is the only spec-version literal in `src/`. Anything that needs to
/// name the version reads this constant.
///
/// See `REFERENCES.md` for the pinned reference-implementation SHAs this
/// version claim is measured against (GUARD-06).
pub const SPEC_VERSION: &str = "1.1.0";

pub mod bits;
pub mod dump;
pub mod error;
pub mod model;
pub mod obu;
pub mod packing;
pub mod sequence;

pub use crate::error::{Error, ErrorKind, Finding, Location, Result};
