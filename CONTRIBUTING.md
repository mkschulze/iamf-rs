# Contributing to iamf-rs

The Core Value is narrow and everything here serves it: **a `.iamf` file this
crate writes is read back by the reference decoder `libiamf` with the PCM
sample-identical, and is accepted by `iamf-tools`' stricter parser.** API shape,
codec coverage and ergonomics are all secondary to producing bytes the reference
implementations accept.

## Pull request checklist

Copy this into the PR description and tick every box. An unticked box is a
question for review, not a blocker — an untouched checklist is a blocker.

- [ ] **Licence contamination.** I did not read or port source from **`gpac`**
      (LGPL-2.1), **`libspatialaudio`** (LGPL-2.1+), or **FFmpeg /
      `libavformat`** (LGPL-2.1+) — including "just to check" while debugging a
      byte diff. *Invoking* a compiled binary from a test is permitted (separate
      process, nothing links, nothing distributed together); *opening the
      source* is not. Contamination is irreversible and relicensing afterwards
      needs every contributor's agreement. See `REFERENCES.md` for the full
      read-versus-invoke table.
- [ ] **Reference citation.** Every `fn read_*` and `fn write_*` I added or
      moved carries a preceding `// ref:` line naming the reference file and
      symbol it mirrors, e.g.
      `// ref: iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite`. This is
      mechanically enforced by a test that walks `src/` (D-23, arriving in plan
      01-03), because citations rot silently otherwise.
- [ ] **Read and write are adjacent.** Every OBU type's `read` and `write` live
      in the same file, in that order. Asymmetry between the two is the failure
      mode that produces files which almost work, and adjacency is what makes it
      visually obvious.
- [ ] **Dependency licences.** Every dependency I added states its licence on
      the line that adds it, and `cargo deny check` reports advisories, bans,
      licenses and sources all ok. New licences go through `deny.toml`'s
      allow-list, never through a `deny` key — see the header comment in that
      file before editing it.
- [ ] **Guardrails still bite.** `bash tools/prove-guards.sh` reports six PASS
      lines. If I changed `clippy.toml` or `deny.toml`, I added or updated the
      matching proof case.
- [ ] **Determinism.** No `HashMap`/`HashSet` anywhere output byte order can
      see them, and no `f32`/`f64` outside PROF-03's single documented
      exception. `rg 'allow.*disallowed_types' src/` returns at most that one
      entry.
- [ ] **The gates pass locally.** `cargo build --locked --all-targets`,
      `cargo clippy --all-targets -- -D warnings` and `cargo test --locked` are
      all green.
- [ ] **Reference pins untouched, or deliberately bumped.** If I changed a SHA
      in `REFERENCES.md`, I re-ran the four discriminating checks and the whole
      conformance suite in this same commit.

## House rules

**Refuse with the reason named, never approximate.** This is inherited from
Parallax and it drives the design. An unknown audio element type round-trips
byte-identically and reports "this file carries element type 3, which we do not
model" rather than being silently dropped or approximated. `validate()` returns
*all* findings, each naming its field path, because one run should tell an
import adapter everything wrong with a foreign file.

**Never silently normalise.** For fields the writer derives
(`audio_roll_distance`, `obu_size`, implicit substream IDs), the reader
preserves whatever the wire said. `validate()` reports the mismatch by name.
Fresh construction through the encoder derives the value.

**Hand-written, auditable, mirroring the reference.** No derive macros for the
byte layout. A reviewer who knows neither codebase well must be able to read a
Rust function side by side with the C++ it cites.

**Errors carry position once.** One `Error { kind, at }` type, `Location`
distinguishing read, write, field and "nowhere". No `#[from]` conversions — a
blanket `From` loses the offset. Convert at the call site.

**No `unwrap()`, `expect()`, `panic!`, raw indexing or unchecked arithmetic in
`src/`.** A parser meets hostile input by definition. These are denied in
`Cargo.toml`'s `[lints.clippy]` table and the "outside tests" carve-out lives in
`clippy.toml` configuration, not in `#[allow]` attributes.

## Getting the reference implementations

`REFERENCES.md` pins both. Nothing in a normal `cargo build` needs them —
`cargo test` is green offline on all four targets with no reference binary
present, deliberately. The conformance tiers that do need them discover them by
environment variable.

## Licence of contributions

Unless you state otherwise, any contribution you intentionally submit for
inclusion in this work, as defined in Apache-2.0, is dual licensed as
`MIT OR Apache-2.0` with no additional terms. See `PATENTS` for the Alliance for
Open Media Patent License 1.0 that sits alongside it.
