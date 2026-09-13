# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **Portfolio contract (read first):**
> [`Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md`](../Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md)
> (absolute: `/Users/cell/local/Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md`).
> This is the source of truth for what each of the six IAMF repositories owns. `iamf-rs` owns the
> IAMF v1.1 OBU model, parsing, validation, deterministic IDs and standalone IA Sequence writing. It
> does not own codec encode/decode, rendering, carrier/container handling or DAW state. Check a
> change against the document's "Non-negotiable rules" before building it. If a change moves
> ownership or adds a cross-library object, the document's "Change control" section requires
> updating `HANDOFF.md`, the portfolio document, the fixtures and the consumer's pin together.
>
> **Keep it current:** `.claude/hooks/portfolio-sync.sh` (a SessionStart/PostToolUse hook in
> `.claude/settings.local.json`) detects a newly completed ROADMAP phase, an archived milestone or a
> new git tag and tells you to update the `iamf-rs` row's "Current maturity". Do that in the Parallax
> repository, show the diff, and don't commit there without asking.

Project goals, constraints and the licence policy are in `.claude/CLAUDE.md`. That file is a
research snapshot and parts of it are **superseded**. Where the two disagree, this file and the code
win:

- `bitstream-io` is **not** a runtime dependency. The bit cursor and writer are hand-written in
  `src/bits/`, and `bitstream-io` is a dev-dependency used only as a differential oracle in
  `tests/bits_oracle.rs` (D-01). Never import it from `src/`.
- The reference pins are **`iamf-tools` v2.1.0 `848c6ff`** and **`libiamf` v1.1.0 `f06e919`**
  (`REFERENCES.md`), not an `iamf-tools` v1.x tag. `SPEC_VERSION` is `"1.1.0"` in `src/lib.rs`.
- The toolchain is pinned to **1.85.0** exactly (`rust-toolchain.toml`).
- Decoding is out of scope. The crate parses and writes IAMF bitstreams. It does not render, decode
  codecs, resample, measure loudness or do any other DSP (README "Consumer boundary").

## Commands

```sh
cargo build --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings      # CI sets RUSTFLAGS="-D warnings"
cargo test --locked                                     # offline, no reference binary needed
cargo test --locked --test packing                      # one integration-test binary
cargo test --locked --test packing five_one_packs_as_two_coupled_pairs_then_two_monos   # one test
cargo test --locked --features fuzzing --test fuzz_regression  # replay the committed fuzz corpus

cargo deny --all-features check                          # licence/advisory gate
bash tools/prove-guards.sh                               # proves each lint/licence guard actually fires (expects 6 PASS)
bash tools/check-codec-dependency-boundary.sh            # no codec crate may reach the root crate
cargo tree -e normal,no-proc-macro                        # must list only iamf + thiserror (plain `-e normal` gives a false positive)
```

Golden fixture: if the `tests/golden` test fails, output bytes changed. Regenerate only on purpose,
and read the dump diff before committing:
`IAMF_REGENERATE_GOLDEN=1 cargo test --test golden regenerate_golden_artifacts -- --nocapture`.

Reference tier (Linux, network, C toolchain): `bash tools/build-reference.sh` builds `iamfdec` at the
pinned SHA into `.reference/` and writes `.reference-manifest.json`. Export `IAMF_REF_DECODER=<path>`;
`iamf-tools` runs from `tools/iamf-tools.Dockerfile` via `IAMF_TOOLS_IMAGE`. Then run
`cargo test --locked --test conformance -- --nocapture --test-threads=1`. Without those variables the
reference tests print `SKIP` and pass, so a green offline run is **not** conformance evidence.

`fuzz/` and `tools/codec-fixtures/` are **separate, excluded workspaces** with their own lockfiles and
`deny.toml`. See the `CLAUDE.md` in each.

## Architecture

Layers, from bottom to top:

1. **`src/bits/`**: `BitCursor` (read) and `BitWriter` (write) over byte buffers, plus ULEB128
   (8-byte / `u32` cap, minimal form on write). This is the only module that knows about bit
   positions. Errors are created here with their byte offset already attached. `bounded_vec` limits
   how much a count field read from the input can pre-allocate (64 KiB).
2. **`src/obu/`**: one file per OBU type, each with a `read_*`/`write_*` pair. `obu/mod.rs` holds the
   generic `Obu<T> { header, payload, trailing }` wrapper. `write_obu_with_header` writes the payload
   into a scratch writer first, so `obu_size` is known before the header is written. Trailing bytes
   the parser doesn't understand are kept, so they round-trip byte-for-byte. `boundaries.rs` walks OBU
   boundaries using only byte 0 and `obu_size`.
3. **`src/model/`**: `DescriptorSet`, the ordered set of Codec Configs, Audio Elements and Mix
   Presentations. The collections are `Vec` in bitstream order with `by_id` lookup, and duplicate ids
   are reported as `Finding`s. This layer also holds layout vocabulary, `Q7_8` loudness and minimum
   profile selection.
4. **`src/packing.rs`**: channel-to-substream ordering for channel-based audio elements. The order is
   taken from citations, not derived. A wrong order still decodes cleanly, just with swapped channels,
   so `tests/packing.rs` is the only thing that catches it.
5. **`src/sequence.rs`**: `parse_sequence(&[u8]) -> ParsedSequence` keeps unknown OBUs and parameter
   blocks with no matching definition, so they round-trip. `validate()` returns every `Finding`, not
   just the first. `SequenceWriter` is a streaming writer: descriptors, then each temporal unit
   (nothing is buffered across units), then `finish`.
6. **`src/encoder.rs`**: the high-level API for consumers (Parallax).
   `EncoderBuilder` → `build()` validates all declarations together, then returns an `Encoder` and an
   `IdManifest`. The manifest maps the caller's handles to IAMF wire ids. `Encoder::start(W)` writes
   the descriptors. `push_temporal_unit(TemporalUnitInput)` fully validates each unit before writing
   any of it. `finish()` returns the sink. Inputs are LPCM bytes or already-encoded FLAC/Opus access
   units, plus parameter blocks the caller has already decimated.
7. **`src/dump.rs`**: deterministic annotated text dump. Use it for review diffs only. It reads with
   this crate's own parser, so it can't prove correctness.
8. **`src/error.rs`**: one `Error { kind: ErrorKind, at: Location }`, marked `#[non_exhaustive]` and
   size-budgeted. There are no `#[from]` conversions, because a blanket `From` loses the offset.
   Parallax matches on these types, so changing an existing variant breaks a second repository.

`src/fuzzing.rs` (behind the `fuzzing` feature) builds bounded `ParsedSequence` models for both the
libFuzzer targets and the stable replay test.

## Rules the tooling enforces

- **Clippy denies** `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`,
  `panic`, `HashMap`/`HashSet`, `f32`/`f64` and the transcendental float methods, crate-wide
  (`Cargo.toml` `[lints]` + `clippy.toml`). Tests are exempt through `clippy.toml`, not through
  `#[allow]`. Use `checked_*` and `.get()`, and return a typed `Error`.
- **Float escape census**: `rg 'allow.*disallowed_types' src/` must return exactly one hit,
  `model/loudness.rs::lufs_to_q7_8`. Don't add another.
- **Citations**: every `fn read_*` / `fn write_*` in `src/` needs a `// ref: <project>@<tag> <file>
  <symbol>` comment directly above it. `tests/citations.rs` fails the build without one. If a
  reference comment contradicts the reference code, add a `// NOTE:` saying the code is authoritative.
- **`read` and `write` sit next to each other** in the same file. OBU modules put read first.
- **Never silently normalise**: the reader keeps whatever the wire said for fields the writer derives
  (`obu_size`, `audio_roll_distance`, implicit substream ids). Mismatches are reported by
  `validate()`, and fresh encoding derives the correct value.
- **Licence contamination**: never open `gpac`, `libspatialaudio` or FFmpeg/`libavformat` source.
  Running a compiled binary of them is fine. `iamf-tools`, `libiamf`, `eclipsa-audio-plugin`, `libear`
  and `obr` may be read. The untracked `docs/` checkouts (`iamf-tools`, `libiamf`, `oar`, spec HTML)
  may not be at the pinned SHAs, so check before citing from them.
- **Linked dependency graph** is `iamf` + `thiserror` only (plus optional `arbitrary` behind
  `fuzzing`). Put every dependency's licence in a comment on the line that adds it.

## Evidence documents

- `REFERENCES.md`: pins and the rules on which reference source may be read versus only run.
- `CONFORMANCE-GATE.md`: waiver ledger and experiments. For example, `iamfdec` exits 0 even on corrupt
  input, so tests check what the tool actually produced, never its exit code.
- `DIFF-LEDGER.md`: explains every byte where our output differs from `iamf-tools`.
- `HANDOFF.md`: consumer contract with Parallax.

Planning state is in `.planning/` (GSD). Quick tasks go in `.planning/quick/<id>-<slug>/`.
