---
phase: quick-260914-kfs
plan: 01
subsystem: obu/audio_element
tags: [validation, iamf-tools, kMaxNumParameters, conformance-docs]
status: complete
requires: [quick-260914-hoa, quick-260914-5c5]
provides:
  - AudioElement::validate Finding at Field("num_parameters") for more than 256 params
  - CONFORMANCE-GATE.md "Reference limits diagnosed, not enforced" section
affects: [DescriptorSet::validate, ParsedSequence::validate, EncoderBuilder::build error kind for >256 params]
tech-stack:
  added: []
  patterns: [stricter reference limit diagnosed by validate(), not refused on read or write]
key-files:
  created: []
  modified:
    - src/obu/audio_element.rs
    - tests/sequence_parse.rs
    - tests/encoder_builder.rs
    - CONFORMANCE-GATE.md
    - HANDOFF.md
    - .planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md (edited, not staged)
decisions:
  - "kMaxNumParameters = 256 is reported by AudioElement::validate (> 256, matching iamf-tools), never enforced on read or write (user decision 2026-09-14)"
  - "Writer does not refuse > 256 params so parse-then-write stays byte-exact (PARSE-04), matching the param_definition_type 0 precedent"
requirements: [PARSE-01, PARSE-04, DESC-04]
metrics:
  duration: ~25 min
  completed: 2026-09-14
actuals:
  tokens: 3619
  tasks: 2
  commits: 2
plan_head_before: 2b4c05835ab33e5067f973df48d91d8240876436
---

# Phase quick-260914-kfs Plan 01: kMaxNumParameters decision Summary

An Audio Element with more than 256 params still parses and round-trips byte-exactly. `AudioElement::validate`, and through it `DescriptorSet::validate` and `ParsedSequence::validate`, now reports one cited `Finding` at `Field("num_parameters")` because iamf-tools@v2.1.0 refuses such an element. The decision is recorded in CONFORMANCE-GATE.md and HANDOFF.md.

## Commits

| Task | Commit | Message | Files |
| ---- | ------ | ------- | ----- |
| 1 | `98c80fe` | feat(quick-260914-kfs): report Audio Elements with more than 256 params in validate() | src/obu/audio_element.rs, tests/sequence_parse.rs, tests/encoder_builder.rs |
| 2 | `36b4289` | docs(quick-260914-kfs): record the kMaxNumParameters decision and the num_parameters finding | CONFORMANCE-GATE.md, HANDOFF.md |

`git rev-list --count 2b4c058..HEAD` = 2.

## TDD: RED evidence (HEAD 2b4c058, tests only; `target/kfs-t1-red.log`)

```
test build_rejects_257_audio_element_params_through_the_element_finding ... FAILED
test build_rejects_audio_element_params_even_when_an_extension_definition_is_opaque ... ok
test audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit ... FAILED
test audio_element_with_256_params_has_no_num_parameters_finding ... ok

---- audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit stdout ----
thread '...' panicked at tests/sequence_parse.rs:1459:5:
assertion `left == right` failed
  left: []
 right: [Finding { at: Field("num_parameters"), message: "audio element 300 has 257 parameters; iamf-tools@v2.1.0 refuses an Audio Element with more than 256 (kMaxNumParameters) on read and write, although IAMF v1.1.0 requires parsers to support any num_parameters" }]

---- build_rejects_257_audio_element_params_through_the_element_finding stdout ----
thread '...' panicked at tests/encoder_builder.rs:185:5:
assertion `left == right` failed
  left: UnsupportedParameterData
 right: InvalidDescriptorReference
```

K1 failed on the finding assertion (a), the element's own `validate()`, not on parse or write. The log has no `UnexpectedEndOfInput` or `ObuTooLarge`. K3 failed on the kind. Controls K2 (256 params) and K4 (1 param, existing test) were green on HEAD. The RED step made no commit; tests and implementation went into one `feat` commit, as the plan specified.

## GREEN / gate outputs

Task 1 verify (after GREEN): sequence_parse 49 passed, encoder_builder 73 passed, descriptors 62 passed, parse_reference 7 passed (no semantic_sha256 changed), golden 11 passed, citations 3 passed, public_api 1 passed, lib 16 passed. `clippy --all-targets -D warnings` clean, and clean again with `--features fuzzing`. `git diff --quiet 2b4c058 -- tests/support/reference_expectations.rs tests/fixtures tests/golden.rs fuzz src/model src/sequence.rs src/encoder.rs src/error.rs DIFF-LEDGER.md` passed.

Task 2 release gates:
- `cargo fmt --check`: OK
- `cargo build --locked --all-targets`: OK
- `cargo clippy --locked --all-targets -- -D warnings`: OK
- `cargo test --locked`: exit 0, 30 result lines, 591 passed, 0 failed, 0 ignored
- `cargo test --locked --features fuzzing --test fuzz_regression`: 5 passed
- `bash tools/prove-guards.sh`: exit 0, 9 distinct PASS cases (a)-(i). The script prints its case list twice, so `grep -c PASS` returns 18.
- `bash tools/check-float-escape-census.sh`: 1 hit, `src/model/loudness.rs:94`
- `cargo tree -e normal,no-proc-macro`: `iamf thiserror`
- No `*.proptest-regressions` files were created.

## Evidence verified during execution

- `docs/iamf/index.bs:754` is "Parsers SHALL support any value of [=audio_element_obu/num_parameters=]." Lines 752-753 hold the 0/1/2 and 0 per-type rules.
- iamf-tools `848c6ff4`: `audio_element.cc:97-113` `ValidateNumParameters` (quoted rationale and `UnimplementedError`). Line `:759` is `ValidateNumParameters(GetNumParameters())` and `:811` is `ValidateNumParameters(num_parameters)`. `audio_element.h:282-287` has `kMaxNumParameters = 256`. Commit `feb873de` is dated 2025-11-03 and is an ancestor of `848c6ff4`.
- libiamf `f06e919e`: `IAMF_OBU.c:455-480` `iamf_element_new` has `IAMF_MALLOCZ(ParameterBase *, val)` with no cap and uses `bs_skipABytes` for unknown types.

## Deviations from Plan

None in the code. Minor placement notes:
- `IAMF_TOOLS_MAX_NUM_PARAMETERS` sits directly after `impl AudioElement`, before `push_reserved_finding`, rather than above the impl block. It is private, carries the planned `// ref:` / `// NOTE:` block and a `///` doc, and the citations test passes.
- The hoa deferred-items edit is left unstaged, as instructed.

## Deferred Items

See `260914-kfs-deferred-items.md`:
- The spec rule at `index.bs:752-753` (channel-based `num_parameters` SHALL be 0, 1 or 2; scene-based SHALL be 0) has no finding today. It is out of scope for this decision.

## Threat Flags

None. No new surface. T-kfs-01 (uncapped read count) is accepted and documented in CONFORMANCE-GATE.md. T-kfs-02 and T-kfs-03 are mitigated by the finding, the K1 test and the "Revisit when" entry.

## Self-Check: PASSED

- FOUND: src/obu/audio_element.rs contains `IAMF_TOOLS_MAX_NUM_PARAMETERS`, `kMaxNumParameters`, `ValidateNumParameters`, `iamf_element_new`
- FOUND: commits `98c80fe`, `36b4289` on main
- FOUND: CONFORMANCE-GATE.md `## Reference limits diagnosed, not enforced` before `## Waivers`; HANDOFF.md `quick 260914-kfs`
