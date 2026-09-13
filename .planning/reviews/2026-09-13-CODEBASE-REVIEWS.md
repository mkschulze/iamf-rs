---
scope: full-codebase
reviewers: [codex, claude, opencode, antigravity]
reviewed_at: 2026-09-13T15:08:00+02:00
reviewed_commit: 77cc284
models:
  codex: "gpt-5.6-terra (reasoning=high)"
  claude: "unknown"
  opencode: "unknown"
  antigravity: "unknown"
model_sources:
  codex: "banner"
  claude: "unknown"
  opencode: "unknown"
  antigravity: "unknown"
lane_status:
  codex: complete
  claude: complete
  opencode: "partial — spawn ETIMEDOUT after 11 min; progress notes only, no file:line findings"
  antigravity: "failed — headless read_file permission auto-denied; no review"
---

# Cross-AI Code Review — iamf-rs full codebase

Adapted `/gsd-review` run: the reviewers got a **code review** prompt for the landed crate at
`77cc284` (all phases complete, so there were no pending plans). Claude was added on purpose, because
most of the code was written with Codex. Exception: the `bounded_vec` change (quick task 260913-js8)
was written by Claude agents, so for that change Codex is the independent reviewer.

---

## Codex Review

### Summary

One HIGH hostile-input flaw in OBU header parsing: the fields after `obu_size` can be read, and
allocated, outside the declared `obu_size`. The high-level builder is well protected, but the public
generic sequence writer can still emit descriptor sets that decoders reject, without returning an
error.

### Strengths

- Primitive reads bounds-check spans before slicing or copying (`src/bits/reader.rs:193`). Parser
  reservations use `bounded_vec`.
- OBU writing measures the after-size fields and the payload before emitting `obu_size`
  (`src/obu/header.rs:370`).
- Descriptor order is deterministic: codec configs and audio elements are sorted by ID, and mix
  presentations keep input order (`src/model/mod.rs:232`).
- High-level encoder submission validates a complete temporal unit before writing
  (`src/encoder.rs:224`).
- `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings` and the fuzz-regression
  replay passed.

### Findings

- **[HIGH] Header after-size fields are parsed outside the declared OBU boundary** —
  `src/obu/header.rs:452`.
  - `read_obu_header_parts` checks only the numeric `obu_size`. It then reads the trim fields and
    `extension_header_size` from the parent cursor.
  - `read_extension_header` copies `len` bytes, bounded only by the rest of the input
    (`src/obu/header.rs:487`). Only afterwards does `read_obu_with_header` subtract the consumed
    after-size bytes and reject an underflow (`src/obu/mod.rs:139`).
  - Example: `01 00 01 aa` declares a type-0 OBU with `obu_size = 0`, yet `read_obu_header` accepts
    it and consumes `01 aa`.
  - **Fix:** parse the trim and extension fields through a cursor limited to `obu_size`.
- **[MEDIUM] Generic sequence writing can emit a header that libiamf rejects** —
  `src/sequence.rs:793`.
  - `SequenceWriter::push_descriptors` never acts on `DescriptorSet::validate()` findings.
  - `IaSequenceHeader::validate()` flags `primary_profile >= PROFILE_COUNT`
    (`src/obu/sequence_header.rs:66`), but `write_ia_sequence_header` only rejects inverted profile
    order (`:119`).
  - **Fix:** validate on the authoring path, or expose an explicit unchecked writer.

### Test gaps

- No test for `obu_size = 0` (or too small) with trim or extension bytes after it.
- No allocation-bound test for header extension lengths that exceed the OBU but fit the input.
- No test decides whether `write_sequence` / `push_descriptors` must reject invalid profiles and
  unresolved references.

### Risk Assessment

**HIGH.** Reads and allocations can go outside a declared OBU.

### Coverage

All of `src/bits/*`, `src/obu/*`, `src/sequence.rs`, `encoder.rs`, `packing.rs`, `dump.rs`,
`error.rs` and `src/model/{mod,layout,loudness,profile}.rs`, plus the relevant tests and both fuzz
targets.

---

## Claude Review

### Summary

A well-built crate. The bit I/O, uleb128, OBU framing and descriptor readers match IAMF v1.1.0 field
for field. Hostile input ends in typed errors, and nothing makes output differ between platforms.
**No HIGH findings.** The defects fall into three groups:

- **Asymmetric writer states:** low-level writers accept models whose gate bits disagree with the
  data they gate.
- **Over-strict parsing:** an opaque extension parameter definition aborts the whole parse.
- **Memory:** zero-byte Parameter Block subblocks amplify memory about 50×.

Eight findings were confirmed with a scratch program in `/tmp/iamf-probe`; the repo was not modified.
Claude did not run `cargo test` or clippy.

### Strengths

- A single bit primitive per direction (`src/bits/reader.rs:309`, `src/bits/writer.rs:178`).
- uleb128 stops at 8 bytes and rejects values above `u32` (`src/bits/leb128.rs:97-115`).
- `obu_size` is measured, not backfilled. Non-minimal sizes still give correct offsets
  (`src/obu/header.rs:380-391`, `:420-431`).
- A single drain for unread payload bytes, through a bounded sub-reader (`src/obu/mod.rs:128-159`).
- Gate flags and counts are derived, not stored.
- Parameter Block writes are checked against the governing definition
  (`src/obu/parameter_block.rs:529-620`).
- Deterministic: no hashed containers, stable sorts, and the only float is the IEEE-exact
  `lufs_to_q7_8`.

### Findings

- **[MEDIUM] Zero-byte Parameter Block subblocks amplify memory about 50× (CONFIRMED)** —
  `src/obu/parameter_block.rs:445-473`.
  - `num_subblocks` is checked only against `bytes_remaining()`. A Recon Gain subblock with no
    present layers consumes 0 bytes.
  - Measured: 38 MB of input reached **1.94 GB max RSS**. NUL strings in `read_strings`
    (`src/obu/mix_presentation.rs:541-556`) cost about 24 bytes per input byte.
  - **Fix:** cap or reject when the minimum subblock size on the wire is 0, and consider a
    per-parse memory budget.
- **[MEDIUM] `LoudnessExtension.info_type_bits` without a `0xFC` bit writes bytes the reader never
  reads (CONFIRMED)** — `src/obu/mix_presentation.rs:201-203`, `:804-807`.
  - The builder guards this (`src/encoder.rs:899-905`); the low-level writers do not.
- **[MEDIUM] Other writer states that break round-trip or framing without error** — all CONFIRMED
  except the ambisonics case:
  - `AudioElementParam::Extension` with type 0, 1 or 2 (`src/obu/audio_element.rs:632-635`).
  - `AudioElementType::Reserved { value: 0 | 1 }` (`audio_element.rs:239`, `:533`).
  - `DurationFields` whose non-empty `subblock_durations` are dropped when a constant duration is
    set (`src/obu/param_definition.rs:333-344`).
  - Ambisonics length mismatches (`audio_element.rs:811-823`) — found by reading, not run.
  - Enum aliases: `Layout::Reserved(2|3)` and `ObuType::Reserved(0..=23 | 31)`
    (`src/obu/header.rs:89`).
  - **Fix:** reject these states on write, and add a proptest that a successful write always
    re-reads equal.
- **[MEDIUM] An opaque extension parameter definition aborts the entire parse (CONFIRMED)** —
  `src/obu/param_definition.rs:119-129`, called from `src/sequence.rs:510-512`.
  - A spec-legal `Extension { type: 7, bytes: [] }` makes `parse_sequence` fail with
    `UnexpectedEndOfInput @ 26`, and that offset is wrong as well.
  - **Fix:** try to decode the prefix; if that fails, don't register the definition.
- **[MEDIUM] `EncoderBuilder` accepts Codec Configs whose frame timing disagrees** —
  `src/encoder.rs:817-828`, `:258-271`. Acceptance CONFIRMED; the spec rule is PLAUSIBLE.
  - Nothing compares `num_samples_per_frame` or sample rate across configs in a presentation.
  - **Fix:** require these to match when the Codec Configs share a presentation.
- **[MEDIUM] Trimming is accepted on any temporal unit** — `src/encoder.rs:453-466`.
  Acceptance CONFIRMED; the reference rule is PLAUSIBLE.
  - `[None, end-trim 100, None, start-trim 480]` was accepted, and the Opus `pre_skip` coverage
    isn't checked.
  - **Fix:** track the unit index; allow start trims only at the beginning and an end trim only at
    the end.
- **[MEDIUM] Expanded loudspeaker layouts get the Simple profile (PLAUSIBLE)** —
  `src/model/profile.rs:157-166`.
  - **Fix:** confirm against `iamf-tools@v2.1.0 ProfileFilter`, and if confirmed force
    Base-Enhanced.
- **[LOW] Trim and extension fields aren't bounded by `obu_size`** — `src/obu/header.rs:442-456`,
  `src/obu/mod.rs:139-145`.
  - The error names the wrong defect (`TruncatedObu`).
  - `parse_sequence` can repeat the copy up to 3 times (`src/sequence.rs:484-486`, `:525-530`).
- **[LOW] `validate()` expects roll distance 0 for AAC; AAC-LC requires −1** —
  `src/obu/codec_config.rs:595-600`.
- **[LOW] A duplicate `parameter_id` binds to the first definition regardless of type** —
  `src/obu/param_definition.rs:171-175`.

### Test gaps

- Arbitrary models never reach the writer. `fuzz/fuzz_targets/obu_roundtrip.rs` feeds only
  writer-valid models.
- No test of memory retained per consumed byte. The `allocation_bounds` tests cover reservations
  only.
- No test of opaque extension definitions in `parse_sequence`.
- No builder tests for cross-config timing or trim position.

### Risk Assessment

**MEDIUM.** The builder → `SequenceWriter` → LPCM path is solid. The risk sits in the low-level
writers, in over-strict extension parsing, in linear memory growth, and in two builder gaps.

### Coverage

- **Read in full:** `src/bits/*`, `src/obu/*`, `src/sequence.rs`, `encoder.rs`, `packing.rs`,
  `error.rs`, `src/model/*`, `lib.rs`, `Cargo.toml`, `clippy.toml`, both fuzz targets.
- **Partly read:** `src/dump.rs`, `src/fuzzing.rs`.
- **Not reached:** most test files and the root ledgers. The upstream clones were off-limits, which
  is why the spec-rule findings are marked PLAUSIBLE.

---

## OpenCode Review

> [reviewed-without-source-citations] The lane timed out (`spawn error: ETIMEDOUT`, ~11 min) and left
> only progress notes, with no `file:line` findings. Down-weighted in the consensus.

Salvaged observations:

- "`parallax_delivery_fixture_uses_the_offline_safe_reference_gates` reproducibly fails because the
  public builder emitted a sub-mix containing 128- and 960-sample elements; `decoder_main` rejects
  it."
- "A parser boundary check performed after extension allocation." Same issue as Codex HIGH /
  Claude LOW.
- "Encoder states that can emit structurally invalid temporal units."
- "No output-visible hash iteration or non-exact floating-point path."

---

## Antigravity Review

Failed; no review was produced. The headless `agy` run was auto-denied the `read_file` permission.
To enable it, add a `read_file(...)` allow-rule to its `settings.json`.

---

## Consensus Summary

Codex and Claude give fully grounded reviews; OpenCode counts at reduced weight. Orchestrator
verification is marked ✔.

### Agreed Strengths

- Every hostile-input path traced ends in a typed error; no panic or overflow paths were found
  (Codex, Claude).
- `obu_size` is measured before it is emitted, and parser reservations are bounded (Codex, Claude).
- Output is deterministic, with no hashed-order or float risk (Codex, Claude, OpenCode).
- The high-level `EncoderBuilder` path is well protected compared with the low-level writers
  (Codex, Claude).

### Agreed Concerns (highest priority)

1. **After-size OBU header fields are parsed outside `obu_size`** — Codex HIGH, Claude LOW,
   OpenCode noted it.
   - ✔ Verified: `read_extension_header` reads from the parent cursor (`src/obu/header.rs:487`),
     and the check comes only afterwards (`src/obu/mod.rs:139`).
   - The copy is bounded by the remaining input, so the memory cost is ~1× input, not amplified.
     The real defect is broken framing: `read_obu_header` alone accepts `obu_size = 0` followed by
     extension bytes, and the error kind is wrong.
   - **Orchestrator severity: MEDIUM.**
   - **Fix (both reviewers):** take `sub_reader(obu_size)` right after the size field.
2. **The builder accepts mismatched frame timing across Codec Configs, and this is the root cause of
   the known failing conformance test** — Claude MEDIUM, OpenCode observed it.
   - ✔ Verified: the Parallax fixture builds FLAC with 128 samples per frame
     (`tests/support/parallax_contract.rs:108`) and Opus with 960 (`:109`).
   - The pinned `decoder_main` rejects the result: "Audio elements in a submix must have the same
     number of samples per frame."
   - That resolves quick task 260913-js8's deferred item: the fixture is invalid, and the builder
     should have rejected it.
   - Two decisions are needed: validate in the builder, and fix or split the fixture.
3. **Public low-level writers emit non-conformant or mis-framed bytes without error** — Codex
   MEDIUM (invalid `primary_profile` via `SequenceWriter`), Claude MEDIUM (loudness extension,
   reserved enum aliases, extension param types, duration fields, ambisonics lengths).
   - Common fix: validate on write, or split into checked and explicitly unchecked writers.
   - Add a proptest that a successful write always re-reads equal.

### Divergent Views

- **Severity of the header-boundary issue:** Codex HIGH vs Claude LOW. See the orchestrator's call
  above (MEDIUM).
- **Found by Claude only:** zero-byte-subblock memory amplification (~50×, measured), opaque
  extension definitions aborting parse (a conformance risk: rejecting spec-legal files), trim
  position, expanded-layout profile (PLAUSIBLE), AAC roll distance, and duplicate `parameter_id`.
  None were contradicted; Codex didn't mention them.
- **Test suite status:** Codex reported `cargo test --all-targets` passing, while OpenCode and quick
  task 260913-js8 reproduced the conformance failure. Most likely Codex's environment had no
  reference binary, so that test's reference gate didn't run.
