//! Bit-level I/O — **the only module in this crate permitted to know what a
//! bit offset is** (BITS-07).
//!
//! Nothing outside `src/bits/` constructs a bit position, and no error type
//! from outside this crate crosses this boundary. Every OBU module above reads
//! through [`BitCursor`] and writes through [`BitWriter`]; when one of them
//! runs out of input it returns an [`crate::Error`] already carrying the byte
//! offset the read began at. That containment is what makes a fuzz report
//! actionable: the offset is produced where the position is known, not
//! reconstructed two layers up.
//!
//! # Why this is hand-rolled (D-01)
//!
//! BITS-01 as originally written said "wrapping `bitstream-io`". D-01 amends
//! that: the cursor and the writer are hand-rolled over `&[u8]`, and
//! `bitstream-io` is demoted to a **dev-dependency** used as a differential
//! oracle in `tests/bits_oracle.rs` — a proptest asserting our primitives agree
//! with it bit-for-bit on random input.
//!
//! The reason is [`crate::Location`]. A wrapped stream crate reports failure as
//! a `std::io::Error`, which carries no byte offset of ours; every primitive
//! would need a mapping layer to invent one, and BITS-07 exists precisely to
//! contain that layer. Hand-rolling means the offset is native — it is the
//! cursor's own field — so `Location::InputOffset` and `Location::OutputOffset`
//! are free rather than reconstructed.
//!
//! **Reversibility: costly.** Adopting a wrapped crate later reintroduces that
//! mapping boundary at every primitive and loses the native offsets that every
//! `Error.at` value in the crate flows from.
//!
//! # The containment proof, and why it is currently free
//!
//! BITS-07 asks that the foreign-error boundary be *contained* to this module.
//! Under D-01 that boundary **does not exist at all**: the crate's public
//! [`crate::ErrorKind`] has no variant wrapping a standard-library I/O error
//! type, because the hand-rolled cursor never produces one. There is nothing to
//! map, nothing to contain, and no `#[from]` to lose an offset through.
//!
//! That is recorded here rather than left implicit because it is exactly what a
//! future maintainer would give up by reaching for a wrapped stream crate: not
//! "some conversion code", but the property that a read failure's byte offset is
//! the cursor's own field. `tests/bits_oracle.rs` is the reason that trade is
//! not worth making — the oracle already buys the confidence a mature crate
//! would, at zero cost to the shipping graph, and it is imported from exactly
//! one file under `tests/`.
//! <!-- the io::Error mention above is deliberate; it names the thing we do not do -->
//!
//! `cargo tree -e normal,no-proc-macro` lists `iamf` and `thiserror` and
//! nothing else. Plain `cargo tree -e normal` is the **wrong** instrument for
//! that claim — it includes proc-macro edges and shows the whole
//! `thiserror-impl → syn → proc-macro2 → quote → unicode-ident` chain, so it
//! reports a failure on a correct crate.
//!
//! # Read and write are adjacent, in that order (D-10, BITS-02)
//!
//! [`BitCursor`] and [`BitWriter`] define their methods in the same order, one
//! for one. `reader.rs` and `writer.rs` are meant to be read side by side; a
//! primitive that exists in one and not the other, or that treats a width
//! differently in the two directions, is supposed to be visually obvious.
//!
//! # The uleb128 fixed-size encoder, and what it is actually for (D-03)
//!
//! The public writer emits **minimal form only**. A `pub(crate)` fixed-size
//! encoder also exists (`leb128::write_uleb128_fixed`), and D-03 records its
//! rationale as "so the conformance harness can reproduce reference files
//! exactly (CONF-08)". **That rationale is wrong and is corrected here.**
//!
//! Research scanned 524 674 `obu_size` fields across all 226 reference `.iamf`
//! files and found **zero** non-minimal encodings; `LebGenerator`'s default
//! mode is `kMinimum` and the CLI proto's default is `GENERATE_LEB_MINIMUM`.
//! Exactly one of the 226 textprotos sets `GENERATE_LEB_FIXED_SIZE`, and it is
//! `test_000134` — not `test_000003`, which is the CONF-08 fixture and whose
//! every size field is minimal. The fixed-size path is therefore **not** a
//! CONF-08 blocker.
//!
//! It is kept because it is fifteen lines, because Phase 2's PARSE-04
//! foreign-file caveat needs to *read* non-minimal encodings (which is the
//! decoder's job regardless), and because `test_000134` exercises it. It stays
//! `pub(crate)`: byte-identity must be caller-independent, so there is no
//! caller-visible `LebMode` on the public encoder.
//!
//! # Citation discipline (D-23)
//!
//! Every `fn read_*` and `fn write_*` below carries a preceding `// ref:` line
//! naming the reference file and symbol at the pinned trees in `REFERENCES.md`
//! (`iamf-tools` v2.1.0 `848c6ff`, `libiamf` v1.1.0 `f06e919`).
//! `tests/citations.rs` enforces this mechanically.
//!
//! The citation points at the **function**, and the reviewer's job is to read
//! the code there, not its docstring: `libiamf@main`'s LPCM endianness comment
//! says `0x01 - big endian` while the line below it does
//! `param->big_endian = !ior_8(r)`. Where a reference comment misleads like
//! that, the citation is followed by a `// NOTE:` line saying the code is
//! authoritative.

mod leb128;
mod reader;
mod writer;

pub use reader::BitCursor;
pub use writer::BitWriter;

/// How many bytes the minimal uleb128 encoding of a value occupies.
///
/// Re-exported `pub(crate)` because `obu_size`'s own ceiling depends on the
/// length of the size field that carries it (`max_obu_size =
/// kEntireObuSizeMaxTwoMegabytes - 1 - size_of_obu_size`), so the OBU layer has
/// to be able to ask.
pub(crate) use leb128::minimal_len as minimal_uleb128_len;
