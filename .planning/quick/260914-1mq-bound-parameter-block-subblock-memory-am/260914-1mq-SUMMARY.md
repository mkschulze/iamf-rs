---
phase: quick-260914-1mq
plan: 01
subsystem: parser-hardening
tags: [parameter-block, recon-gain, demixing, allocation-bounds, GUARD-03]
status: complete
requires: []
provides:
  - "ErrorKind::SubblockCountNotOne (appended unit variant)"
  - "check_single_subblock in read_parameter_block and validate_parameter_block"
affects: [src/obu/parameter_block.rs, src/error.rs]
tech-stack:
  added: []
  patterns: ["spec-derived structural count rule checked before allocation"]
key-files:
  created: []
  modified:
    - src/error.rs
    - src/obu/parameter_block.rs
    - tests/temporal.rs
    - tests/allocation_bounds.rs
    - .planning/codebase/CONCERNS.md
decisions:
  - "Demixing and Recon Gain Parameter Blocks must carry exactly one subblock (spec index.bs:776-792, iamf-tools ValidateSpecificParamDefinition); rejected with SubblockCountNotOne at the payload start on read, Field(\"subblocks\") on write"
  - "The bytes_remaining() subblock rule now exempts only Recon Gain (zero-byte subblocks possible); Demixing, Mix Gain and Raw keep it unchanged"
  - "The full iamf-tools mode-0 / csd == duration rule is deferred (Option A)"
metrics:
  duration: "~15 min"
  completed: 2026-09-14
plan_head_before: 64d2eb361d211f1e75095163db0bf52a389cebdf
actuals:
  tokens: 5200
  tasks: 2
  commits: 2
---

# Phase quick-260914-1mq Plan 01: Bound Parameter Block subblock memory amplification Summary

Demixing and Recon Gain Parameter Blocks now need exactly one subblock (`SubblockCountNotOne`). The check runs before any reservation, which closes the zero-byte Recon Gain subblock amplification (about 204× at 2 MiB). It also fixes the false `UnexpectedEndOfInput` rejection of the legal single all-absent Recon Gain block.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 16eca4a | fix(quick-260914-1mq): require exactly one Demixing or Recon Gain Parameter Block subblock |
| 2 | 5708c33 | docs(quick-260914-1mq): record the zero-byte subblock class as closed and name the linear residuals |

## RED evidence (HEAD 64d2eb3 plus the new variant only, `target/1mq-red.log`)

- H1 `hostile_recon_gain_subblock_count_in_two_mebibyte_parameter_block_errors_at_offset_0`: FAILED. The 2 MiB vector parsed Ok, so `expect_err` panicked.
- H2 `a_recon_gain_count_implied_by_a_hostile_duration_is_refused_before_the_byte_rule`: FAILED. Got `(UnexpectedEndOfInput, InputOffset(0))`.
- H3, H4, H5, H6 (explicit count 2, mode-0 implied 2, two Demixing, zero Demixing): FAILED. Each got `None`, meaning the block parsed Ok.
- P1 `a_single_recon_gain_subblock_with_every_layer_absent_reads_and_round_trips`: FAILED. `expect` panicked on `UnexpectedEndOfInput@0`.
- W1 `the_writer_refuses_two_demixing_subblocks`: FAILED. Got `None`, meaning the write succeeded.
- These controls were ok: C1 (multi-subblock Mix Gain), `a_subblock_count_larger_than_the_bytes_remaining_is_refused_before_reserving`, `demixing_data_uses_exactly_three_mode_bits_and_five_reserved_bits`, `test_000059_two_layer_recon_gain_shape_round_trips_exact_bytes`, `hostile_parameter_count_in_two_mebibyte_audio_element_errors_at_offset_12`.

## Gate results

- `cargo fmt --all -- --check`: clean
- `cargo build --locked --all-targets`: ok
- `cargo clippy --locked --all-targets -- -D warnings`: clean, and also clean with `--features fuzzing`
- `cargo test --locked`: full_exit=0, 556 passed, 0 failed, every binary ok (includes golden, parse_reference, citations, error_shape)
- `cargo test --locked --features fuzzing --test fuzz_regression`: 5 passed
- Float census `rg -U -c 'allow\(\s*clippy::disallowed_types' src/`: `src/model/loudness.rs:1`
- Protected paths unchanged since 64d2eb3: reference_expectations, fixtures, golden, DIFF-LEDGER, HANDOFF, fuzz, tools, sequence/encoder/fuzzing, bits
- Committed scope: exactly `src/error.rs`, `src/obu/parameter_block.rs`, `tests/allocation_bounds.rs`, `tests/temporal.rs`, `.planning/codebase/CONCERNS.md`
- No `semantic_sha256` or golden change

## Deviations from Plan

None. The plan was executed exactly as written.

## Deferred

See `260914-1mq-deferred-items.md`, which is not committed:
1. The linear residuals and the per-parse memory budget decision.
2. The optional full iamf-tools mode/csd rule as a `validate()` finding.
3. Assumption A1 has not been run against a reference binary.

## Known Stubs

None.

## Threat Flags

None. No new surface; T-1mq-01 and T-1mq-03 were mitigated as planned.

## Self-Check: PASSED

- The commits 16eca4a and 5708c33 exist on main.
- The modified files exist. `260914-1mq-deferred-items.md` exists.
