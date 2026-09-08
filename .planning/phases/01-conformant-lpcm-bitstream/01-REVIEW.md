---
phase: 01-conformant-lpcm-bitstream
reviewed: 2026-09-08T18:48:20Z
re_reviewed: 2026-09-08T19:32:44Z
resolved_at: 2026-09-08T19:45:46Z
depth: standard
files_reviewed: 68
files_reviewed_list:
  - .github/workflows/ci.yml
  - .github/workflows/reference.yml
  - .gitignore
  - .planning/REQUIREMENTS.md
  - .planning/WINDOWS.md
  - .planning/phases/01-conformant-lpcm-bitstream/01-CONTEXT.md
  - .planning/phases/01-conformant-lpcm-bitstream/deferred-items.md
  - .planning/research/PITFALLS.md
  - CONFORMANCE-GATE.md
  - CONTRIBUTING.md
  - Cargo.toml
  - DIFF-LEDGER.md
  - NOTICE
  - PATENTS
  - README.md
  - REFERENCES.md
  - clippy.toml
  - deny.toml
  - rust-toolchain.toml
  - src/bits/leb128.rs
  - src/bits/mod.rs
  - src/bits/reader.rs
  - src/bits/writer.rs
  - src/dump.rs
  - src/error.rs
  - src/lib.rs
  - src/model/layout.rs
  - src/model/loudness.rs
  - src/model/mod.rs
  - src/model/profile.rs
  - src/obu/audio_element.rs
  - src/obu/audio_frame.rs
  - src/obu/boundaries.rs
  - src/obu/codec_config.rs
  - src/obu/header.rs
  - src/obu/mix_presentation.rs
  - src/obu/mod.rs
  - src/obu/param_definition.rs
  - src/obu/parameter_block.rs
  - src/obu/sequence_header.rs
  - src/obu/temporal_delimiter.rs
  - src/packing.rs
  - src/sequence.rs
  - tests/bits_oracle.rs
  - tests/citations.rs
  - tests/conformance.rs
  - tests/descriptors.rs
  - tests/error_shape.rs
  - tests/fixture.rs
  - tests/fixtures/MANIFEST.md
  - tests/fixtures_cap.rs
  - tests/golden.rs
  - tests/obu_header.rs
  - tests/packing.rs
  - tests/profile.rs
  - tests/refcorpus.rs
  - tests/reference_manifest.rs
  - tests/sequence.rs
  - tests/support/fixture.rs
  - tests/support/test_000003.rs
  - tests/temporal.rs
  - tests/vectors.rs
  - tools/build-reference.sh
  - tools/cargo-test-tap.sh
  - tools/experiments/corrupt-fixture.py
  - tools/experiments/two-codec-configs.textproto
  - tools/iamf-tools.Dockerfile
  - tools/prove-guards.sh
findings:
  critical: 4
  warning: 2
  info: 0
  total: 6
status: resolved
gate_verdict: ready_for_verification
fix_report: .planning/phases/01-conformant-lpcm-bitstream/01-REVIEW-FIX.md
---

# Phase 1: Code Review Report

**Reviewed:** 2026-09-08T18:48:20Z
**Depth:** standard (configured `deep` was auto-downgraded because the computed scope exceeded 50 files)
**Files Reviewed:** 68 authored files
**Status:** resolved after fix re-review
**Gate verdict:** **READY FOR PHASE VERIFICATION**

## Scope

The workflow supplied `DIFF_BASE=a67d64aed6c5dce2d0beee0e6be54640605513bd^` and a computed Phase 1 scope of 145 files. The required reading was narrowed to the 68 authored files listed in frontmatter. The vendored corpus under `tests/fixtures/reference/**`, the generated golden artifacts under `tests/fixtures/golden/**`, and generated `Cargo.lock` were explicitly excluded from content review. Their manifests, consumers, size caps, hashes, and conformance tests remained in scope.

The standing `01-TDD-GATE-WAIVER.md` was honored. Plan 01-05's RED/GREEN commits exist under the unpadded `(1-5)` scope; this review neither re-investigates that known GSD false positive nor recommends rewriting history. The repo-wide rustfmt drift already recorded in `deferred-items.md` is likewise not a new finding.

## Summary

Fix iteration 2 closes the residual CR-01 alias path identified by independent
re-review. All six findings now have focused behavioral coverage and a green
local test/lint baseline.

## Critical Issues

### CR-01: Parameter Block serialization loses the definition type that determines wire syntax

**Classification:** BLOCKER
**File:** `src/obu/parameter_block.rs:400-435`
**Related:** `src/sequence.rs:315-344`

**Issue:** `read_parameter_block` requires both a `ParamDefinition` and `ParamDefinitionType`, because the type determines whether subblock bytes are Mix Gain syntax, extension syntax, or an unsupported Demixing/Recon Gain shape. The writer takes only `ParamDefinition` and serializes whichever `ParameterData` variant the caller supplied. `SequenceWriter` compounds the problem by collecting only bare `ParamDefinition` values and discarding whether each came from Mix Gain, Demixing, or Recon Gain. A Mix Gain definition paired with `ParameterData::Raw` therefore emits a size-prefixed blob where the decoder expects an animation type; the reverse mismatch is equally possible. The resulting stream is not interpreted as the model the caller supplied and can shift all later subblock parsing.

**Fix:** Carry the type with every governing definition (for example, `{ definition, kind }`), pass `kind` into `write_parameter_block`, and preflight every subblock before writing. Reject a data variant that does not match the governing kind; keep Demixing/Recon Gain writes unsupported until their required context is implemented.

**Fix re-review (2026-09-08): unresolved.** Commit `98d04e5` carries the
governing kind through `SequenceWriter` and rejects the ordinary Mix Gain/raw
mismatch, but its public validator accepts `ParameterData::Raw` for every
`ParamDefinitionType::Reserved(_)` (`src/obu/parameter_block.rs:529-534`). The
public enum permits callers to construct `Reserved(0)`, `Reserved(1)`, or
`Reserved(2)` even though those wire values mean Mix Gain, Demixing, and Recon
Gain respectively (`src/obu/parameter_block.rs:65-97`). Consequently,
`Reserved(0)` still writes a size-prefixed raw payload under wire kind 0, while
`Reserved(1)` and `Reserved(2)` bypass the intended unsupported-kind rejection.
The regression test covers only the valid extension value `Reserved(7)` and
does not exercise these aliases (`tests/temporal.rs:536-570`). Validate the
numeric value as well as the enum variant: raw data is legal only when
`kind.value() >= 3`; values 0, 1, and 2 must follow their canonical semantics.

**Fix iteration 2 (2026-09-08): resolved.** Commit `ab9e0e1` canonicalizes the
public kind by its wire value before validating the data shape. The regression
test proves `Reserved(0..=2)` cannot select raw extension syntax, while the
existing `Reserved(7)` byte-identical round-trip remains green.

### CR-02: Parameter Block writer emits impossible duration/subblock combinations

**Classification:** BLOCKER
**File:** `src/obu/parameter_block.rs:411-435`

**Issue:** The read path correctly derives the expected subblock count, requires per-subblock durations only for mode 1 with `constant_subblock_duration == 0`, and checks their sum against `duration` (`src/obu/parameter_block.rs:303-383`). The write path checks only whether the block-level duration structure is present. It then writes every `subblock_duration: Some(_)` regardless of mode, permits `None` where a duration is required, never checks the sum, and never checks `subblocks.len()` when the count is implied by a non-zero constant duration (or supplied by a mode-0 definition). These states produce bytes that the paired reader parses at different field boundaries or with a different number of subblocks.

**Fix:** Before emitting `parameter_id`, derive the expected count and duration policy using the same rules as the reader. Require exactly the implied number of subblocks; require every per-subblock duration when mode 1/constant-zero; forbid it otherwise; and verify the checked sum equals the declared duration. Return typed errors before any bytes are appended.

### CR-03: Annotation-count mismatches are serialized as shifted Mix Presentation payloads

**Classification:** BLOCKER
**File:** `src/obu/mix_presentation.rs:441-453`
**Related:** `src/obu/mix_presentation.rs:565-571`

**Issue:** `count_label` is derived solely from `annotations_language.len()`, but the writer emits every entry of `localized_presentation_annotations` and every sub-mix element's `localized_element_annotations`. If either vector length differs, there is no independent count on the wire: a short vector makes the parser consume following fields as strings, while an extra vector makes the parser interpret an annotation as `num_sub_mixes` or rendering config. `validate()` reports the mismatch at lines 374-399, but the public writer does not reject it. This is not faithful serialization of a non-conformant foreign value—the wire format cannot represent the mismatch, and parsed values always contain exactly `count_label` entries.

**Fix:** Preflight both annotation cardinality rules against `count_label` before writing the first field. Pass `count_label` into `write_sub_mix_audio_element` (or validate at the top level), and return a typed mismatch error rather than emitting an ambiguously shifted payload.

### CR-04: Generic OBU parsing does not enforce the 2 MiB ceiling

**Classification:** BLOCKER
**File:** `src/obu/mod.rs:125-142`
**Related:** `src/obu/header.rs:408-459`, `src/obu/audio_frame.rs:200-205`

**Issue:** The writer and `find_obu_boundaries` enforce `kEntireObuSizeMaxTwoMegabytes`, and `read_audio_frame` explicitly claims that the ceiling was enforced upstream. It was not: `read_obu_header_parts` accepts any `u32` `obu_size`, and `read_obu_with_header` only checks conversion to `usize` and whether the caller's slice contains that many bytes. An input containing a multi-gigabyte OBU is therefore accepted by the actual parser even though the structural walker rejects it; payload readers such as Audio Frame then duplicate the entire remainder with `to_vec()`. This violates the stated parser hardening boundary and removes the intended allocation cap.

**Fix:** Measure the encoded size-field length in `read_obu_header_parts` and apply the same derived `ENTIRE_OBU_SIZE_MAX - 1 - size_of_obu_size` check used by the walker before constructing the payload sub-reader or allocating/copying payload data. Add a test against `read_obu_with_header`, not only `find_obu_boundaries`.

## Warnings

### WR-01: A failed streaming write leaves a reusable writer pointing at partial output

**Classification:** WARNING
**File:** `src/sequence.rs:200-233`
**Related:** `src/sequence.rs:294-311`

**Issue:** `push_temporal_unit` flushes the delimiter and each block/frame independently. If a later block fails validation/serialization, or if `Write::write_all` partially writes before returning an error, earlier bytes remain in the sink while the state remains `DescriptorsWritten`. The API accepts another temporal unit or a retry, which can duplicate a delimiter/earlier blocks and produce a corrupt sequence. `bytes_written` is also updated only after a fully successful `write_all`, so its diagnostic offset can lag the sink after a partial write.

**Fix:** Preflight all fallible model checks before the first flush, and poison the writer after any sink or mid-unit emission failure so subsequent `push_*`/`finish` calls return a dedicated error. Document that the sink may contain a partial terminal sequence. If unit atomicity is required, build one temporal unit in a temporary buffer before flushing it.

### WR-02: CI executes mutable action tags despite the repository's pinning policy

**Classification:** WARNING
**File:** `.github/workflows/ci.yml:63,139,149`
**Related:** `.github/workflows/reference.yml:45,191`

**Issue:** `actions/checkout@v4`, `actions/upload-artifact@v4`, and especially third-party `taiki-e/install-action@v2` are mutable tag references. A moved or compromised tag changes code executed with the workflow token without a repository diff. This conflicts with the project's otherwise explicit rule that moving references are not reproducible pins.

**Fix:** Pin each action to a reviewed full commit SHA and retain the release tag in a comment for readability. Use dependency automation to propose SHA updates as reviewable diffs.

## Verification Performed

- `cargo test --locked --all-targets` — passed: all unit, integration, golden, fixture, and offline conformance targets green.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- Static call-chain review covered OBU framing, descriptor serialization, Parameter Block context propagation, sequence streaming, fixture/conformance harnesses, guard scripts, Dockerfile, and both CI workflows.
- The reference-gated Linux/container workflow was not executed locally. Focused offline tests now exercise the fixed constructed-model, parser-ceiling, streaming failure, and non-canonical `Reserved(0..=2)` cases.

## Resolution

Fix iteration 1 was independently re-reviewed over `98d04e5^..f14bca0`; the
single residual blocker was corrected and verified in iteration 2.

| Finding | Re-review verdict | Evidence |
|---|---|---|
| CR-01 | Resolved | `ab9e0e1` canonicalizes `ParamDefinitionType` by wire value before matching its data shape. A focused red/green regression rejects all three `Reserved(0..=2)` aliases and retains the `Reserved(7)` round-trip. |
| CR-02 | Resolved | `src/obu/parameter_block.rs:437-526` preflights mode, exact count, duration-field presence, and checked sum before line 415 writes the first byte. The focused tests cover implied-count mismatch, missing/wrong explicit durations, and forbidden durations. |
| CR-03 | Resolved | `src/obu/mix_presentation.rs:441-478` checks presentation and every element annotation vector before output; both cardinality sites have focused no-partial-write tests. |
| CR-04 | Resolved | `src/obu/header.rs:349-363,420-436` validates the measured encoded size-field length before any after-size field or payload access. Non-minimal ULEB128 remains accepted. |
| WR-01 | Resolved | `src/sequence.rs:204-240,263-379` preflights the full unit, counts partial writes, poisons after emission failures, and rejects retry and `finish`; focused tests exercise preflight atomicity and poison semantics. |
| WR-02 | Resolved | Every `uses:` entry in both workflows is a full SHA. Upstream tag resolution confirms checkout v4.4.0 = `11d5960...`, upload-artifact v4.6.2 = `ea165f8...`, and install-action v2.87.8 = `d438492...`. |

Fresh iteration-2 verification passed `cargo test --locked --all-targets` and
`cargo clippy --locked --all-targets -- -D warnings`. The standing TDD waiver
and known rustfmt drift remain unchanged. The code-review gate is ready for
phase verification.

---

_Reviewed: 2026-09-08T18:48:20Z_
_Fix re-reviewed: 2026-09-08T19:32:44Z_
_Resolved: 2026-09-08T19:45:46Z_
_Reviewer: Codex (gsd-code-reviewer)_
_Depth: standard_
