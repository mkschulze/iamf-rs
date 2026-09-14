---
phase: quick-260914-hoa
plan: 01
subsystem: sequence parsing
status: complete
tags: [parse, memory, streaming, fuzz, docs]
requires: [PARSE-01, PARSE-02, SEQ-02, FUZZ-02]
provides:
  - "iamf::sequence::SequenceReader (new, byte_position, Iterator<Item = Result<SequenceObu>>, FusedIterator)"
  - "parse_sequence re-implemented as SequenceReader collected (behaviour-identical)"
  - "parse_sequence fuzz target streamed-vs-eager differential"
affects: [HANDOFF.md consumer contract (additive), .planning/codebase/CONCERNS.md residual (1)]
tech-stack:
  added: []
  patterns: ["per-OBU streaming reader with fuse-and-restore on error"]
key-files:
  created: [tests/sequence_reader.rs]
  modified: [src/sequence.rs, fuzz/fuzz_targets/parse_sequence.rs, fuzz/CLAUDE.md, HANDOFF.md, .planning/codebase/CONCERNS.md]
decisions:
  - "Streaming SequenceReader instead of a ParseLimits budget; no new ErrorKind; parse_sequence stays unlimited"
  - "Streaming is per OBU, not per temporal unit, because temporal_unit_ranges decides uses_delimiters over the whole sequence"
  - "After the first Err the reader restores its cursor to the failing OBU start and fuses"
metrics:
  duration: "~18 min (2026-09-14T11:54:31Z to 12:12:55Z)"
  completed: 2026-09-14
actuals:
  tokens: 7595      # chars/4 over git diff f24a095..HEAD
  tasks: 3
  commits: 3
plan_head_before: f24a095213c134bcf94b951139cb47623d1db8d4
---

# Phase quick-260914-hoa Plan 01: Per-parse memory bound via SequenceReader Summary

A streaming `SequenceReader` (one OBU per `next()`, fused with cursor restore on error) now bounds parse memory. `parse_sequence` is that reader collected and behaves exactly as before. With 32 MiB of `20 00`, max RSS drops from 3.25 GB (eager) to 34.3 MB (streamed).

## Commits

| Task | Commit | Message | Files |
|---|---|---|---|
| 1 | c1fb3fa | feat(quick-260914-hoa): stream sequence parsing one OBU at a time with SequenceReader | src/sequence.rs, tests/sequence_reader.rs |
| 2 | 0c34ed4 | test(quick-260914-hoa): fuzz SequenceReader against parse_sequence | fuzz/fuzz_targets/parse_sequence.rs, fuzz/CLAUDE.md |
| 3 | 5fa7718 | docs(quick-260914-hoa): document the parse-side memory contract and SequenceReader | HANDOFF.md, .planning/codebase/CONCERNS.md |

## TDD evidence (Task 1)

- **RED-1** (`target/hoa-t1-red1.log`): `error[E0432]: unresolved import `iamf::sequence::SequenceReader`` / `no `SequenceReader` in `sequence``. This was the only compile error.
- **RED-2, eager stand-in** (`target/hoa-t1-red2.log`, never committed, replaced by GREEN). `test result: FAILED. 2 passed; 4 failed`:
  - FAILED `reader_yields_each_obu_before_a_later_error`: `assertion failed: matches!(reader.next(), Some(Ok(SequenceObu::TemporalDelimiter(_))))`
  - FAILED `reader_reports_the_position_of_the_next_obu`: `assertion failed: matches!(reader.next(), Some(Ok(_)))`
  - FAILED `reader_streams_the_valid_prefix_of_test_000129`: `an Ok item: Error { kind: UnexpectedEndOfInput, at: InputOffset(53) }`
  - FAILED `reader_equals_parse_sequence_on_every_committed_input`: `positions differ on .../golden/phase1_sample_identity.iamf`, left `[0, 7073, 7073, ...]`, right `[0, 8, 26, 43, ...]`
  - ok `reader_carries_parameter_definitions_across_obus`, ok `reader_on_empty_input_yields_nothing` (controls)
- **GREEN:** 6/6 pass.
- **M-fuse** removed `finished = true` on the Err path (`target/hoa-t1-mut-fuse.log`). `3 passed; 3 failed`:
  - FAILED `reader_yields_each_obu_before_a_later_error`: expected None, got `Some(Err(.. InputOffset(6)))`
  - FAILED `reader_streams_the_valid_prefix_of_test_000129`: got `Some(Err(.. InputOffset(53)))` again
  - Also FAILED `reader_reports_the_position_of_the_next_obu` (a second Err instead of None)
  - Restored with `git restore src/sequence.rs`.
- **M-restore** removed the cursor restore (`target/hoa-t1-mut-restore.log`). `3 passed; 3 failed`:
  - FAILED `reader_reports_the_position_of_the_next_obu`: left 6, right 4
  - Also FAILED `reader_streams_the_valid_prefix_of_test_000129` (left 53, right 37)
  - Also FAILED `reader_equals_parse_sequence_on_every_committed_input` (the prefix `..53` does not parse)
  - Restored.
- After both mutations, `git diff --quiet HEAD -- src tests` held (clean=0). No mutation was committed.

## Differential counts

`sequence_reader differential: inputs=44 iamf_files=40 obus=5846 err_inputs=1` (the only Err input is `tests/fixtures/reference/test_000129.iamf`).

5846 is the probe's 5843 OBUs across Ok inputs plus the 3 Ok prefix OBUs the reader yields from `test_000129` before its error. The test counts yielded items, including those.

## RSS evidence (Task 2)

- Machine: `uname -m` = `x86_64`, macOS, release build. The scratch crate `scratchpad/rssprobe` has an empty `[workspace]`, a path dependency on the repo and a copy of the repo's `rust-toolchain.toml`. Input is `[0x20, 0x00].repeat(16_777_216)`.
- `/usr/bin/time -l ./target/release/rssprobe eager`: `eager obus=16777216`, **3,251,961,856 B** maximum resident set size (6.17 s real).
- `/usr/bin/time -l ./target/release/rssprobe stream`: `stream obus=16777216`, **34,299,904 B** maximum resident set size (1.52 s real). That is below the 200 MB stop threshold and about the size of the 32 MiB input buffer.

## Fuzz workspace

- `cargo +nightly-2026-09-01 check --locked --manifest-path fuzz/Cargo.toml --features roundtrip-model --bins`: exit 0 (`target/hoa-fuzz-check.log`).
- Local 60 s run (cargo-fuzz 0.13.2) from a scratchpad copy of the corpus, with `-artifact_prefix` also in the scratchpad: `Done 465862 runs in 61 second(s)`. No crash, no assertion failure, and no artifacts. `git status --porcelain -- fuzz` showed only the two edited files.

## Gates (Task 3)

1. `cargo fmt --all -- --check`: rc 0
2. `cargo build --locked --all-targets`: rc 0
3. `cargo clippy --locked --all-targets -- -D warnings`: rc 0
4. `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`: rc 0
5. `cargo test --locked`: `full_exit=0`. 30 `test result:` lines (test binaries plus doctests), all ok, with 588 passed, 0 failed and 0 ignored.
6. `cargo test --locked --features fuzzing --test fuzz_regression`: 5 passed
7. `bash tools/prove-guards.sh`: `guardrail proof: 9 passed, 0 failed (9 expected)`
8. `bash tools/check-float-escape-census.sh`: `1 hit(s)` at `src/model/loudness.rs:94`, rc 0
9. `cargo tree -e normal,no-proc-macro`: `iamf thiserror`
10. Protected paths unchanged since f24a095: rc 0. No `semantic_sha256`, golden, fixture, corpus, ErrorKind or offset change. `parse_reference` 7/7, `golden` 11/11, `sequence_parse` 47/47, `round_trip` 12/12 and `citations` 3/3 all pass unchanged.
11. The committed scope is exactly the six planned files.

The Task 1, 2 and 3 `<verify>` blocks were run verbatim and printed TASK1_VERIFY_PASS, TASK2_VERIFY_PASS and TASK3_VERIFY_PASS.

## Deviations from Plan

**1. [Rule 3 - Blocking] Redundant borrows removed in the moved loop body.**
- **Found during:** Task 1 GREEN.
- **Issue:** `let Self { reader, registry, .. } = self;` binds `&mut` references, so the verbatim `&mut reader` failed with E0596.
- **Fix:** Inside `next_obu` only, `&mut reader` became `reader` (9 sites) and `&registry` became `registry` (1 site). The logic, the clones, `payload_base` and every `with_input_base` call are unchanged. rustfmt also reflowed the Audio Frame arm.

**2. [Minor] Test helper for the error value.** `Error` is not `Copy`, so `reader_yields_each_obu_before_a_later_error` builds the expected error through a closure `|| Error::new(..)`. This was fixed before RED-2 was recorded. The assertions did not change.

**3. [Note] Branch.** The commits were made on `main`, as the user decided. The executor's default protected-branch halt was overridden by that recorded decision.

The test file doc text uses ASCII `x` for factors in rustdoc and `×` in HANDOFF and CONCERNS. The grep-based verifies (`96`, `192`) are unaffected.

## Known Stubs

None.

## Threat Flags

None. The new public surface is a read-only iterator over a borrowed slice that the plan's threat model covers (T-hoa-01..05).

## Deferred

See `260914-hoa-deferred-items.md`, which has 7 items. Item 7 records that the local fuzz run did run.

## Self-Check: PASSED

- FOUND: src/sequence.rs (`FusedIterator for SequenceReader`), tests/sequence_reader.rs, fuzz/fuzz_targets/parse_sequence.rs, HANDOFF.md `### Parse-side memory`, 260914-hoa-deferred-items.md
- FOUND commits: c1fb3fa, 0c34ed4, 5fa7718 (`git rev-list --count f24a095..HEAD` = 3)
