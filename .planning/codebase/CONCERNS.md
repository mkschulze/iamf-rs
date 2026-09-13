# Codebase Concerns

**Analysis Date:** 2026-09-13

Context: v1 (Phases 01-04) is recorded complete in `.planning/STATE.md`. The parser hardening lints are in place: `Cargo.toml` `[lints.clippy]` denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`, `panic`, `disallowed_types` and `disallowed_methods`, and `unsafe_code = "forbid"`. `DIFF-LEDGER.md` is empty and enforced by a test. What remains is mostly scope limits, drift between documents, and a few structural risks. It is not a list of open defects.

## Tech Debt

**Reference pin contradicts the project constraint:**
- Issue: `.claude/CLAUDE.md` / `PROJECT.md` say to pin an `iamf-tools` **v1.x** tag and warn that HEAD is a draft-v2.0.0 tree. `REFERENCES.md` actually pins `iamf-tools` `v2.1.0` (`848c6ff…`), and every `// ref:` comment in `src/` cites `iamf-tools@v2.1.0`, for example `src/obu/mix_presentation.rs` and `src/bits/leb128.rs`.
- Files: `REFERENCES.md`, `.claude/CLAUDE.md`, `.planning/PROJECT.md`, `tools/iamf-tools.Dockerfile`
- Impact: Future contributors can't tell which rule is authoritative. Someone following CLAUDE.md might "fix" the pin and break the ref comments and the conformance fixtures.
- Fix approach: Update the constraint text in `PROJECT.md`/`CLAUDE.md` to name v2.1.0 and state why it is v1.1-spec-conformant, or record the waiver in `CONFORMANCE-GATE.md`.

**Stale planning docs in CLAUDE.md:**
- Issue: The `## Conventions` and `## Architecture` sections in `.claude/CLAUDE.md` still say "not yet established". The stack section recommends `bitstream-io` as a runtime dependency, but `Cargo.toml` demotes it to a dev-only oracle (D-01) and uses a hand-rolled `src/bits/` instead. The stack section also recommends `BTreeMap`, while the constraints and `clippy.toml` require `Vec` plus `by_id`.
- Files: `.claude/CLAUDE.md`, `Cargo.toml`, `src/bits/`
- Impact: Agents that load CLAUDE.md get contradictory guidance.
- Fix approach: Regenerate the CLAUDE.md sections from `.planning/codebase/*.md`.

**Untracked upstream reference clones in the working tree:**
- Issue: `docs/libiamf` (about 7.9 GB), `docs/iamf-tools`, `docs/eclipsa-audio-plugin`, `docs/iamf` and `docs/oar` are untracked and not gitignored. `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` notes that these clones sit on later HEADs (`d13b8dd…`, `e55e183…`) than the pins.
- Files: `docs/`, `.gitignore`
- Impact: A `git add -A` could commit gigabytes. Grep-based guard scripts and tooling could pick up draft-v2 code and treat it as reference. Reading the unpinned HEAD risks mirroring draft behaviour.
- Fix approach: Add these clones to `.gitignore` or move them outside the repo. Check out the pinned SHAs if they are kept for reference reading.

**Audit document is untracked and in German:**
- Issue: `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` is uncommitted (`git status`). It audits the `feature/aac-lc-framing` branch, not `main`.
- Files: `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md`
- Impact: The P1/P2 findings may not describe `main`. They are also not discoverable by English-language planning.
- Fix approach: Commit it on the branch it audits, or restate its P1-P3 findings in `.planning/` against `main`.

**Long source files concentrate logic:**
- Files: `src/encoder.rs` (1284 lines), `src/sequence.rs` (1019), `src/dump.rs` (910), `src/obu/parameter_block.rs` (865), `src/obu/audio_element.rs` (827), `src/obu/mix_presentation.rs` (809), `src/obu/codec_config.rs` (808); `tests/conformance.rs` (2603), `tests/descriptors.rs` (1380), `tests/support/fixture.rs` (1290)
- Impact: Review of read/write symmetry gets harder as files grow. The project relies on keeping each read/write pair next to each other in one file.
- Fix approach: Split by sub-structure only when a file gains a new OBU variant. Keep each read/write pair in the same file.

## Known Bugs

**No confirmed open bugs detected.**
- `DIFF-LEDGER.md` has an empty table, and `tests/conformance.rs::conf_07_byte_diff_matches_the_committed_ledger` asserts that it matches in both directions.
- No `TODO`/`FIXME`/`HACK` markers exist in `src/`. The only `TODO`s are cited upstream TODOs:
  - `src/obu/mix_presentation.rs:10` and `:324` cite `TODO(b/339855338)`, because the iamf-tools read path does not validate sub-mix constraints.
  - `tests/descriptors.rs:1194` cites the same issue.
  - `tests/fixtures/reference/test_000017.textproto:19` cites `TODO(b/356905554)`: fully trimmed frames can't be encoded upstream.
- Implication: this crate's read-side validation in `SubMix::validate` is stricter than the reference. A file that `iamf-tools` accepts may be rejected here, which is a possible interoperability complaint and not a conformance bug.

## Security Considerations

**Allocation amplification from count fields:**
- Risk: Parsers pre-allocate `Vec::with_capacity(count)` after checking only `count <= bytes_remaining()`. The call sites are `src/obu/mix_presentation.rs:450,548,563,569,750` and `src/obu/audio_element.rs:563,656`. With an OBU capped at `ENTIRE_OBU_SIZE_MAX = 1 << 21` (`src/obu/header.rs:13`), a count near 2 M multiplied by `size_of::<SubMix>()` or a layer struct can reserve hundreds of MB per nested level before the first element read fails.
- Files: `src/obu/mix_presentation.rs`, `src/obu/audio_element.rs`, `src/obu/parameter_block.rs`
- Current mitigation: Byte-remaining bound, the 2 MiB OBU cap, the fuzz targets `fuzz/fuzz_targets/parse_sequence.rs` and `obu_roundtrip.rs`, and `tests/fuzz_regression.rs`.
- Recommendations: Cap the capacity with `min(count, bytes_remaining / min_encoded_item_size, SMALL_CONST)`, or push without pre-reserving. Add a fuzz regression that asserts peak allocation (for example under `-rss_limit_mb`).

**Saturating conversions in boundary offsets:**
- Risk: `src/obu/boundaries.rs:75` (`unwrap_or(usize::MAX)`) and `:103` (`unwrap_or(u64::MAX)`) turn a conversion failure into a sentinel value instead of an error.
- Files: `src/obu/boundaries.rs`
- Current mitigation: Appears to be used only for reporting positions, and only on 64-bit hosts.
- Recommendations: Confirm the sentinels can never feed a length or comparison that decides acceptance. Otherwise return `ErrorKind::ObuTooLarge`.

**Single sanctioned float escape:**
- Risk: `src/model/loudness.rs:94` `#[allow(clippy::disallowed_types)]` on `lufs_to_q7_8`. This is the only `#[allow]` in `src/`, and it is justified as IEEE-exact (`* 256.0` plus `round_ties_even`).
- Current mitigation: The `clippy.toml` census rule and `tools/prove-guards.sh`.
- Recommendations: Keep the census check (`rg 'allow.*disallowed_types' src/` must return exactly one hit) in CI. Any second `#[allow]` should be treated as a scope breach. `tools/check-codec-dev-deps.sh:79` also emits `#[allow(dead_code)]` into generated preflight code; this is harmless but falls outside the census.

## Performance Bottlenecks

**No measured bottlenecks.** The crate is offline by design (`src/sequence.rs` module docs).
- `SequenceWriter::push_temporal_unit` streams to the sink and reuses a scratch buffer, so memory stays flat per unit.
- Two-pass `obu_size` computation in `src/obu/header.rs` (`minimal_uleb128_len`) serialises payloads before writing. That costs a copy per OBU, which is acceptable offline.
- About 30 `.clone()` calls in `src/`. Not a concern at current scale, but worth watching in `src/encoder.rs` (whole-input `Vec::with_capacity(input.frames.len())` at lines 239-274 holds all submitted frames in memory, unlike the streaming writer).

## Fragile Areas

**Byte-identity golden fixtures:**
- Files: `tests/golden.rs`, `tests/fixtures/golden/`, `tests/fixtures/MANIFEST.md`, `tests/fixtures/PHASE2-EXPECTATION-PROVENANCE.md`, `DIFF-LEDGER.md`
- Why fragile: Any change to descriptor order, trim order, uleb128 form or substream packing changes the SHA-256 digests and the ledger. The ledger test fails if a difference appears or disappears.
- Safe modification: Regenerate the golden files deliberately. Update `DIFF-LEDGER.md` rows with a non-empty `why`. Re-run the pinned Docker `iamf-tools:v2.1.0` oracle (`tools/build-reference.sh`, `.github/workflows/reference.yml`).
- Test coverage: Strong for LPCM 5.1. Narrower for other codecs and layouts, where CONF-07 compares only the one `sample_identity` fixture.

**Minimal vs fixed-size uleb128:**
- Files: `src/bits/leb128.rs` (`write_uleb128_fixed`), `src/obu/header.rs:273-389` (all header writes use `write_uleb128_minimal`)
- Why fragile: Byte identity with `iamf-tools` depends on the reference encoder using minimal form for the configurations tested. Reference output built with a different `LebGenerator` setting would diverge at every size field.
- Safe modification: Add a ledger fixture per generator mode before exposing a mode switch.

**Profile selection and delivery validation:**
- Files: `src/model/profile.rs`, `src/encoder.rs`, `tests/profile.rs`, `tests/encoder_builder.rs`
- Why fragile: The audit (P1/P2) relies on callers invoking `validate_delivery_conformance()` and `validate_delivery_timeline()` before muxing. `start()` still emits descriptor fragments, and nothing forces a complete sequence. Multiple sub-mixes are conservatively rejected.
- Safe modification: Keep the Simple/Base/Base-Enhanced regressions pinned to `iamf-tools@v2.1.0`. Never emit draft-v2 profiles.

**CI matrix x86_64 macOS:**
- Files: `.github/workflows/ci.yml:14-35`, `CONFORMANCE-GATE.md`
- Why fragile: `macos-13` is retired. x86_64-apple-darwin now runs under Rosetta 2 on arm64 runners. If hosted Rosetta disappears, the four-target byte-identity claim drops to three and needs a recorded waiver.

## Scaling Limits

**OBU size:**
- Current capacity: 2 MiB per OBU (`src/obu/header.rs:13`). uleb128 decodes to `u32`, with at most 8 bytes.
- Limit: Very large audio frames (high channel count LPCM at long frame sizes) can exceed 2 MiB and are rejected.
- Scaling path: Spec-bound. Use smaller `num_samples_per_frame`.

**Encoder whole-input API:**
- Current capacity: `src/encoder.rs` materialises `input.frames` and `input.parameter_blocks`.
- Limit: Host memory, for example about 4 GB for an hour of 7.1.4 24-bit.
- Scaling path: Use `SequenceWriter` streaming (`src/sequence.rs`) for long exports.

## Dependencies at Risk

**`syn = "=2.0.119"` and `toml = "=0.8.23"` exact pins (dev):**
- Risk: Exact pins block security updates and conflict with other dev-dependency upgrades.
- Impact: Only the CODEC-07 AST/manifest oracle (`tests/codec_dependency_boundary.rs`). Nothing is shipped.
- Migration plan: Bump deliberately with the oracle test in the same PR.

**Excluded workspaces with their own lockfiles:**
- Risk: `fuzz/` and `tools/codec-fixtures/` are excluded from the root workspace, so `cargo update` and root `cargo deny` do not cover them. `libfuzzer-sys` needs NCSA, scoped to `fuzz/deny.toml`.
- Impact: Advisories there go unnoticed unless their own CI steps run.
- Migration plan: Keep `cargo deny --manifest-path fuzz/Cargo.toml check` in `.github/workflows/fuzz.yml`. Add the equivalent check for `tools/codec-fixtures`.

**Docker reference image:**
- Risk: `tools/iamf-tools.Dockerfile` builds a 700 MB image with Bazel 7.4.1 and Bazelisk v1.29.0, pinned by digest in `REFERENCES.md`. Upstream base-image or Bazel registry rot breaks rebuilds.
- Impact: The CONF-05/06/07 reference gates (`.github/workflows/reference.yml`, nightly cron) can't run.
- Migration plan: Push the built image to a registry by digest, not just record it.

## Missing Critical Features

Per `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md`, these gaps are intentional scope limits:

**No decoder, renderer or §7 processing:** Parallax must still use `libiamf` for playback or decode. The audit proposes separate `iamf-decode-rs` and `iamf-render-rs` crates.

**No ISO-BMFF (§6):** Only standalone `.iamf` is supported, so MP4 delivery is blocked. When this is built (M5), `gpac` (LGPL) may not be read.

**Partial high-level authoring:** Scalable multi-layer channel layouts, projection ambisonics, and demixing/recon-gain parameter authoring are only available through low-level model types (`src/obu/parameter_block.rs`, `src/obu/audio_element.rs`). `src/encoder.rs` returns `ErrorKind::UnsupportedLayout` / `UnsupportedParameterData` at lines 895, 945, 1161 and 1171.

**Single freeze-once descriptor section:** Descriptor updates mid-sequence aren't supported by the high-level API (§5 "Teilweise", i.e. partial).

**No codec encoders:** FLAC, Opus and AAC-LC frames must be supplied pre-encoded. Codec configs are framing-only (`src/obu/codec_config.rs`, `CODEC-DEPENDENCY-PREFLIGHT.md`).

## Test Coverage Gaps

**Native FLAC/Opus reference decode on macOS:**
- What's not tested: `libiamf` codec paths on Darwin. The bundled codec archives are x86_64-Linux only, according to the audit.
- Files: `tools/build-reference.sh`, `.github/workflows/reference.yml`
- Risk: A platform-specific codec framing issue would only show up in Linux CI.
- Priority: Low

**Exhaustive profile/authoring differential matrix:**
- What's not tested: A differential comparison against `iamf-tools` for every complex authoring path. CONF-07 covers one fixture.
- Files: `tests/conformance.rs`, `tests/profile.rs`
- Risk: Unexplained byte differences for non-5.1-LPCM configurations would not be caught by the ledger.
- Priority: Medium

**Unit tests inside `src/`:**
- What's not tested: Only about 11 `#[test]` functions live in `src/`. Coverage comes mainly from the integration tests in `tests/`. `src/dump.rs` (910 lines) is a debugging aid with little direct assertion.
- Risk: Regressions in dump formatting or private helpers.
- Priority: Low

**Global presentation timeline:**
- What's not tested: Whether presentation timelines are equal across the whole sequence (audit §3.9-3.10, partial). Only per-unit and trim invariants are validated.
- Files: `src/sequence.rs`, `src/encoder.rs`
- Risk: Sequences that pass validation but whose timelines diverge at decode time.
- Priority: Medium, once ISO-BMFF muxing is built

---

*Concerns audit: 2026-09-13*
