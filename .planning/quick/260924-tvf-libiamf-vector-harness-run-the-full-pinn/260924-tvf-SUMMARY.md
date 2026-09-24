---
phase: quick-260924-tvf
plan: 01
subsystem: tests
status: complete
tags: [conformance, reference-vectors, test-harness, ci-gate, disposition-ledger]
requirements: [CONF-10, CONF-11, OBU-08, PARSE-04, GUARD-06]

requires:
  - "tests/fixtures/reference (35 top-level .iamf, 34 paired .textproto)"
  - "tools/build-reference.sh (owns .reference/libiamf, the gated corpus root)"
provides:
  - "tests/refvectors.rs — one walker at two roots, four-cell is_valid x parse matrix"
  - "tests/support/vector_ledger.rs — VENDORED_DISPOSITIONS (35 rows), STRICTER_THAN_REFERENCE (empty)"
  - "CONFORMANCE-GATE.md — the reverse-direction section (we are stricter than libiamf)"
  - ".github/workflows/reference.yml — the corpus non-skip gate"
affects:
  - "the Linux reference job now fails when the corpus harness skips there"

tech-stack:
  added: []
  patterns:
    - "printed-SKIP-and-return gating (tests/conformance.rs:179-203), never #[ignore]"
    - "committed-const disposition table as change detector (tests/support/reference_expectations.rs)"
    - "line-scanner over a known file shape instead of a parser dependency (conformance.rs:1251)"
    - "printed markers as a CI contract, grepped by the workflow"

key-files:
  created:
    - tests/refvectors.rs
    - tests/support/vector_ledger.rs
  modified:
    - CONFORMANCE-GATE.md
    - .github/workflows/reference.yml

decisions:
  - "The walker runs at the vendored root too, always on, all four targets — otherwise the whole classifier would be code that executes in one Linux job only, and the committed ledger would have zero rows at execution time"
  - "No libiamf_src manifest key and no change to tools/build-reference.sh: the corpus path is already deterministic and single-owner, and a second resolution path would need a hardcoded fallback anyway"
  - "A false upstream is_valid is recorded, never enforced. Only the RED cell (is_valid: true AND we reject structurally) fails"
  - "Verdict carries an explicit `paired` flag rather than inferring pairing from is_valid.is_some() — deviation from the plan's field list, so 'no paired textproto' and 'paired textproto without the key' stay distinguishable"
  - "The duplicate-parameter_id finding on 31 of 35 vendored files is RECORDED, not fixed — a src/ change is outside this slice's blast radius and touches the Parallax error contract"

metrics:
  duration: ~50min
  completed: 2026-09-25

actuals:
  tokens: 63000
  tasks: 3
  commits: 3
  plan_head_before: 80f5f31
---

# Quick Task 260924-tvf: libiamf Reference-Vector Disposition Harness Summary

Every reachable IAMF reference vector is now joined to the `is_valid` declaration in its own paired
upstream `.textproto`, classified into a four-cell matrix with a boundary and round-trip cross-check,
committed as a drift-detecting ledger, and asserted in CI to have actually run.

## What shipped

| Task | Commit | What |
|---|---|---|
| 1 | `39f00fc` | `tests/refvectors.rs` — one walker at two roots, three enums with total `tag()`s, a three-rule violation set, the printed table and the two CI-contract markers |
| 2 | `ab8fe28` | `tests/support/vector_ledger.rs` (35 committed rows + empty `STRICTER_THAN_REFERENCE`), the bijection test, and the `CONFORMANCE-GATE.md` reverse-direction section |
| 3 | `e9921e2` | `.github/workflows/reference.yml` — the corpus non-skip gate, step 13 of 13, between the manifest assertion and the reference-gated suite |

Base commit: `80f5f31`. Diff: 4 files, +1023 lines, **nothing under `src/`**, nothing in `Cargo.toml`,
nothing in `DIFF-LEDGER.md`, no new vendored byte, no new dependency.

## The measured vendored four-cell counts

```
refvectors[vendored] summary: upstream is_valid x our parse disposition
refvectors[vendored]   is_valid | clean | findings | rejected
refvectors[vendored]   true     | 2     | 20       | 0
refvectors[vendored]   false    | 0     | 11       | 1
refvectors[vendored]   -        | 0     | 1        | 0
refvectors[vendored]: walked 35 .iamf, 34 paired
```

- **The RED cell is empty.** No vendored vector that upstream declares valid is rejected here, so
  `STRICTER_THAN_REFERENCE` is an empty slice — a measured zero, not a placeholder.
- The one structural reject is `test_000129.iamf`, whose own textproto declares `is_valid: false`.
  That is the acceptable cell.
- All 35 walk to their own length; all 34 that parse round-trip **byte-identical**. The PARSE-04
  non-minimal-`obu_size` canonicalisation caveat is not exercised by any vendored vector.
- The unpaired file is `test_000076_aac_lc.iamf` (1 finding).

## The surprise, recorded and not papered over

20 of the 22 `is_valid: true` vectors land in **findings**, not **clean**. The dominant cause is a
single finding present on **31 of the 35** files:

```
parameter_id 100 appears at OBU indices 3 and 3; lookup binds to index 3, the first in wire order
```

`src/sequence.rs:498`. It is **factually correct**: the reference vectors routinely give
`element_mix_gain` and `output_mix_gain` the *same* `parameter_id` inside one Mix Presentation —
`tests/fixtures/reference/test_000003.textproto:104` and `:114` are both `parameter_id: 100` — and
binding lookups to the first in wire order is the documented Phase 02 decision.

What it means is that a `Finding` fires on the overwhelming majority of canonical, upstream-valid
vectors, which devalues it as a signal for Parallax. This is recorded in the new
`CONFORMANCE-GATE.md` section and filed as follow-up (d) below. It was **deliberately not changed**:
the hard gate for this slice is "nothing under `src/`", and that finding's wording reaches the
Parallax-facing error surface.

## Verification — real output, run from the repository root

**1. `cargo test --locked --test refvectors -- --nocapture --test-threads=1`**

```
test every_vendored_pair_is_classified ... ok
test the_flag_scanner_reads_the_paired_metadata_block ... ok
test the_reference_corpus_agrees_with_its_own_textprotos ... SKIP refvectors[corpus]: .reference/libiamf/tests is absent. Run `bash tools/build-reference.sh` to fetch the pinned libiamf tree, whose checkout materialises the vector corpus as a by-product. This is the expected offline state (CONF-10).
test the_vendored_ledger_is_a_bijection_with_what_the_walker_produced ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

Both contract markers present: `refvectors[vendored]: walked 35 .iamf, 34 paired` and
`SKIP refvectors[corpus]`. The harness never passes with nothing walked.

**2. `cargo test --locked`** — 31 test binaries, **617 tests passed, 0 failed**, offline, no reference
binary and no container.

**3. `cargo clippy --locked --all-targets -- -D warnings`**

```
    Checking iamf v0.1.0 (/Users/cell/talea/iamf-rs)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.44s
```

Clean. Every helper in `tests/refvectors.rs` is total (returns `Option`/`Vec`/`Verdict`); there is no
file-level `#[allow]` and no local one was needed.

**4. `bash tools/prove-guards.sh`**

```
guardrail proof: 9 passed, 0 failed (9 expected)
all 9 guardrails fired.
```

> **First run reported 8/9.** Case (a) (GUARD-01 licence allow-list) failed with "cargo deny failed
> but never named LGPL". Diagnosed, **not** worked around: `cargo deny --offline check licenses` could
> not resolve `arbitrary v1.4.2` and `wasip2 v1.0.1+wasi-0.2.4` — the optional `fuzzing`-feature crates
> were absent from the local cargo cache, so deny exited non-zero for an environmental reason before it
> could reach the licence verdict. `cargo fetch --locked` populated the cache and the case passes.
> This slice touches none of prove-guards' inputs (`Cargo.toml`, `Cargo.lock`, `clippy.toml`,
> `deny.toml`, `rust-toolchain.toml`, `src/`), confirmed by `git diff 80f5f31 --name-only`.

**5. `bash tools/check-float-escape-census.sh`**

```
src/model/loudness.rs:94
float escape census: 1 hit(s); exactly 1 expected, in src/model/loudness.rs
```

**6. `cargo tree -e normal,no-proc-macro`**

```
iamf v0.1.0 (/Users/cell/talea/iamf-rs)
└── thiserror v2.0.20
```

**7. Workflow structure** — `awk '/^      - name:/{n++} END{print n+0}'` prints **13** (was 12), and
the new step sits at line 101, after the manifest assertion (98) and before the reference-gated suite
(125).

**8. `git diff 80f5f31 --stat`**

```
 .github/workflows/reference.yml |  24 ++
 CONFORMANCE-GATE.md             |  68 +++++
 tests/refvectors.rs             | 552 ++++++++++++++++++++++++++++++++++++++++
 tests/support/vector_ledger.rs  | 379 +++++++++++++++++++++++++++
 4 files changed, 1023 insertions(+)
```

### The CI gate proven to fire, not asserted from reading

The plan required the non-skip step's failure branch to be demonstrably reachable. The step's
**literal `run:` body was extracted from the workflow and executed** offline:

- **Branch 1 (no walked count)** — exit `1`:
  `CONF-11 FAILED: the corpus layer of tests/refvectors.rs printed no walked count; .reference/libiamf/tests was never read in the one job that has it.`
- **Branch 2 (skip detected)** — its exact grep matches the real offline log
  (`grep -c 'SKIP refvectors\[corpus\]'` → `1`), and run end-to-end against a log carrying both
  markers it exits `1`:
  `CONF-11 FAILED: the corpus layer of tests/refvectors.rs SKIPPED here; re-run tools/build-reference.sh so .reference/libiamf/tests exists before the suite.`
- **Success path** — a log with the walked marker and no skip marker exits `0` and echoes
  `refvectors[corpus]: walked 221 reference vectors`.

### The ledger proven to be a change detector

Mutating one committed cell (`test_000003.iamf` `findings: 1` → `7`) fails with the file and column
named, then passes again on revert:

```
thread 'the_vendored_ledger_is_a_bijection_with_what_the_walker_produced' panicked at tests/refvectors.rs:535:9:
assertion `left == right` failed: test_000003.iamf: findings
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out
```

## Deviations from Plan

### 1. `Verdict` carries an explicit `paired: bool` (not in the plan's field list)

- **Found during:** Task 1, writing `print_table`'s `{p} paired` count.
- **Issue:** the plan's `Verdict` has only `is_valid`/`is_valid_to_decode` as `Option<bool>`, so
  "paired" would have to be inferred from `is_valid.is_some()`. That is true for all 34 vendored
  pairs today but conflates "no sibling textproto" with "sibling textproto that does not carry the
  key" — and the corpus is upstream-inconsistent about pairing (research open question 2).
- **Fix:** one extra `bool` field set from `metadata.is_some()`, documented at the field.
- **Files:** `tests/refvectors.rs`. **Commit:** `39f00fc`.

### 2. An unreadable file is announced rather than silently skipped

- **Found during:** Task 1, writing `walk`.
- **Issue:** a `let Ok(bytes) = ... else { continue }` would let the walk shrink silently, which is the
  exact "green because it sees nothing" defect this repository forbids.
- **Fix:** print `refvectors: UNREADABLE {path}` and drop the row, so the 35-row / `MIN_CORPUS_VECTORS`
  assertions fail loudly. **Commit:** `39f00fc`.

### 3. Two count corrections against the plan's own prose

The plan and research say the two flags disagree on **5** vendored vectors. Measured: **6**
(`test_000017`, `test_000119`, `test_000120`, `test_000122`, `test_000129`, `test_000130`). Corrected
in both the `scan_flag` doc comment and `CONFORMANCE-GATE.md`. **Commit:** `ab8fe28`.

### 4. `cargo fetch --locked` run to repair the local cargo cache

Not a code change and not a plan deviation in substance, but recorded because it changed a verify
outcome: see the prove-guards note above.

## Known Stubs

None. No stub, no `TODO`/`FIXME`, no skipped test was introduced.

The corpus test's offline `SKIP` is **not** a broken window: it is the repository's established
gating convention (`tests/conformance.rs:179-203`), it is the correct CONF-10 offline state, and it is
precisely what the new CI step exists to forbid in the one job where the corpus is present. Nothing
was appended to `.planning/WINDOWS.md`.

## Threat Flags

None. No new network endpoint, auth path or trust boundary. The one new read surface — foreign bytes
from `.reference/libiamf/tests` — was in the plan's threat register (T-tvf-01/02/03) and the
mitigations hold: the debug projection is built only on the unequal round-trip branch, each model is
dropped before the next file is read, and `tests/reference_manifest.rs` still runs before this harness
in the reference job.

## Follow-up work this slice deliberately does NOT do

Carried verbatim from the plan's `<notes>`. The 1-3 task budget was fully spent, so these were not
written into `.planning/todos/pending/` — **file them from this summary.**

1. **Encoder conformance from each `.textproto`** — phase-sized, not a quick task. It needs a
   hand-rolled reader for the OLD `UserMetadata` proto dialect (libiamf's textprotos still carry
   `count_label`, `num_substreams`, `num_layers`, `num_sub_mixes`, `num_audio_elements`, `num_layouts`
   and `param_definition_size`, per `tests/fixtures/MANIFEST.md:65-71`), no textproto crate is on the
   `deny.toml` allow-list, the existing `textproto_for` writer handles exactly one Codec Config, one
   Audio Element and one Mix Presentation, and every non-LPCM vector is permanently unencodeable here
   because this crate frames pre-encoded access units and never encodes them.
2. **The `coverage.csv` per-spec-field report** — a new quick task, and only after this ledger exists,
   so the report has a committed floor to assert against instead of being a printout nobody reads.
3. **Test-scaffolding consolidation** (the originating todo's items 2 and 3) — its own quick task, and
   never in the same commit as this harness: a diff that both ADDS and MOVES test surface makes any
   regression ambiguous between "the new harness found a real bug" and "the refactor dropped an
   assertion". Start with the five true duplicates (`sha256_hex`, `rust_files_under`, `repo_root`,
   `sha_on_line_naming`, `PartialThenFail`), and budget for the clippy test carve-out boundary: a
   panicking assertion moved out of a test body into a shared helper trips denied warnings.

### Discovered during execution — two more to file

4. **The duplicate-`parameter_id` finding fires on 31 of 35 canonical vectors.** Decide whether a
   `Finding` that is present on the overwhelming majority of upstream-valid reference files is the
   right signal, given the reference deliberately reuses one id for `element_mix_gain` and
   `output_mix_gain` within a Mix Presentation. A `src/` change and a Parallax-facing wording change,
   so it needs its own slice. Documented in `CONFORMANCE-GATE.md` under the new section.
5. **Tighten `MIN_CORPUS_VECTORS` from its conservative 200** to the count the first green Linux
   reference run prints, citing that run. The floor exists to catch a half-materialised checkout, not
   to pin the upstream census.

## Self-Check: PASSED

Created files exist:

- `FOUND: tests/refvectors.rs`
- `FOUND: tests/support/vector_ledger.rs`

Modified files carry their content:

- `FOUND: CONFORMANCE-GATE.md` — `260924-tvf` present, exactly one
  `## Reference limits diagnosed, not enforced` heading, new subsection at `:692`
- `FOUND: .github/workflows/reference.yml` — `refvectors` present, 13 named steps

Commits exist (`git log --oneline --all | grep -q`):

- `FOUND: 39f00fc` test(260924-tvf): join every reference vector to its paired textproto disposition
- `FOUND: ab8fe28` test(260924-tvf): commit the vendored vector dispositions and document the reverse direction
- `FOUND: e9921e2` ci(260924-tvf): fail the Linux reference job if the vector corpus harness skips

Measured, not narrated: `git rev-list --count 80f5f31..HEAD` = **3**.
