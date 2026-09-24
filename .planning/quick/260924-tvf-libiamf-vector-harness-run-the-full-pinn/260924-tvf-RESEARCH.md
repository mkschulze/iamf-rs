# Quick task 260924-tvf — libiamf vector harness + test consolidation — Research

**Researched:** 2026-09-24
**Domain:** test harness design over a foreign, non-vendored fixture corpus; test-scaffolding consolidation
**Confidence:** HIGH for everything about the current tree (every claim below cites a file read this session).
LOW for anything about the contents of `.reference/libiamf` — that directory **does not exist in this working tree**
(`ls .reference` → `No such file or directory`), so the 221-vector shape is `[ASSUMED]` from the todo, not verified.

---

## Summary

The todo is 10 days old and its first premise is wrong in the most useful direction: the suite does **not**
exercise "essentially `test_000003` plus its own generated fixtures". It already runs a 40-file foreign corpus
with a committed field-level expectation ledger, and — crucially — **34 of the paired upstream `.textproto`
files are already vendored alongside them**, including 12 that declare `is_valid: false`. Those 12 are the
single most valuable finding in this research, because 11 of them are in `POSITIVE_EXPECTATIONS` (we parse
them cleanly) and only 1 is a hard reject. A harness that naively asserted "accept ⟺ `is_valid`" would fail on
11 of the 40 files we already have, on day one, before ever touching the 221-file corpus.

The four sub-goals are not remotely equal in cost. (b) round-trip is nearly free — the machinery exists.
(a) accept/reject needs a **disposition matrix and a documented policy**, not a boolean, and it needs one
grep-width field extractor, not a textproto parser. (d) coverage report is cheap but asserts nothing, so it
rots. (c) encoder conformance is a research project in its own right: it needs a reader for the full
`UserMetadata` textproto dialect over arbitrary multi-element/HOA/Opus/AAC configurations, it can only ever
cover the LPCM subset (this crate cannot encode FLAC/Opus/AAC), and the existing *writer* it would mirror
(`tests/conformance.rs::textproto_for`, ~180 lines) handles exactly one Codec Config, one Audio Element and
one Mix Presentation.

**Primary recommendation:** ship sub-goals (a)+(b) as one new gated test binary over the fetched corpus, with a
committed disposition ledger and a `CONFORMANCE-GATE.md` section for stricter-than-libiamf cells. Split (c),
(d) and the whole of consolidation into follow-ups. See "Recommended slice" at the end.

---

## 1. What already exists (the todo's premise, re-measured)

### The vendored corpus is 40 files, not "test_000003 plus generated fixtures"

| Fact | Evidence |
|---|---|
| 38 positive + 2 negative `.iamf`, asserted as an exact bijection against what is on disk | `tests/parse_reference.rs:45-80`; `assert_eq!(POSITIVE_EXPECTATIONS.len(), 38)` at `:65`, `NEGATIVE_EXPECTATIONS.len(), 2` at `:66` |
| Each positive is pinned by a `semantic_sha256` over `Debug` of the whole `ParsedSequence` **plus** `validate()` findings | `tests/parse_reference.rs:92-100`; table in `tests/support/reference_expectations.rs:54-207` |
| **34 paired upstream `.textproto` files are already vendored** | `ls tests/fixtures/reference/*.textproto` → 34 files; provenance table `tests/fixtures/MANIFEST.md:99-198` |
| Total vendored footprint is 1.5 MB / 78 files | `du -sh tests/fixtures` → `1.5M`; `tests/fixtures/MANIFEST.md:94` ("78 files, 40 of them `.iamf`, 1,243,481 bytes") |

`tests/refcorpus.rs` is **not** a vector harness. It walks `tests/fixtures/reference` (the *vendored* 40) and
asserts only OBU framing: `find_obu_boundaries(...).last() == Some(bytes.len())` and `.first() == Some(0)`
(`tests/refcorpus.rs:78-114`, corpus root constant at `:27`). It is ungated, offline, all four targets, and it
cross-checks the file count against `MANIFEST.md` (`:88-95`).

`tests/vectors.rs` is **not** about the 221-vector corpus at all. Its module doc is explicit: "BITS-06 / D-25 —
hand-computed bit-primitive vectors" (`tests/vectors.rs:1-15`). It is byte-level `BitCursor`/`BitWriter` tests
named after offsets in `test_000003.iamf`. The name collision is a trap for the planner — **do not add corpus
tests to this file.**

### How much of the todo's item 1 is already done

| Sub-goal | Status on the vendored 40 | Status on the 221 |
|---|---|---|
| (a) accept/reject vs `is_valid` | **Partly, and by hand** — dispositions are transcribed per file into `POSITIVE_EXPECTATIONS` / `NEGATIVE_EXPECTATIONS`; the `is_valid` field itself is never read by any test | Not started |
| (b) parse→serialise byte-identical | **Done for the 40** via the semantic-sha ledger + `write_parsed_sequence` assertions (`tests/parse_reference.rs:108-130`) | Not started |
| (c) encoder conformance from textproto | Only the **reverse** direction exists, for one hand-built fixture: `textproto_for` (`tests/conformance.rs:2698-2873`) + `run_encoder_main` (`:2877`) + byte-diff vs `DIFF-LEDGER.md` (`:2963`) | Not started |
| (d) `coverage.csv` report | Not started; `coverage.csv` is not in the tree (`find . -name coverage.csv` → nothing) | Not started |

Gating: there is **no `#[ignore]` anywhere in `tests/*.rs`** (`grep -rn '#\[ignore' tests/*.rs` → empty). The
established convention is *print a SKIP line and return* — see `skip_no_reference` / `skip_no_container` /
`skip_no_aac_reference` (`tests/conformance.rs:146-203`) and `tests/reference_manifest.rs:72-86`. Match that.

---

## 2. Corpus availability and shape

- `.reference/` and `.reference-manifest.json` are **gitignored** (`.gitignore` lines: `/.reference/`,
  `/.reference-manifest.json`) and neither exists locally right now.
- `tools/build-reference.sh` clones libiamf into `$REF_DIR/libiamf` = `.reference/libiamf`
  (`tools/build-reference.sh:39-41`) at `LIBIAMF_SHA=f06e919e…` (`:33`), with
  `git clone --filter=blob:none --no-checkout` followed by `git checkout -q "$LIBIAMF_SHA"` (`:68-76`) — the
  checkout materialises the whole `tests/` tree, which is why the corpus arrives as a by-product of the build.
- **The corpus path is `.reference/libiamf/tests/`** — proven by the smoke step, which reads
  `SMOKE_IAMF="$SRC/tests/test_000003.iamf"` and `SMOKE_REF_WAV="$SRC/tests/sawtooth_100_stereo.wav"`
  (`tools/build-reference.sh:267-268`). `[VERIFIED: tools/build-reference.sh:39-41,267-268]`
- `.reference-manifest.json` has **no corpus key**. Its emitted keys are exactly: `libiamf_sha`,
  `iamf_tools_sha`, `libiamf_a_sha256`, `iamfdec_sha256`, `iamfdec_path`, `host_triple`, `cmake_version`,
  `dep_codecs_disabled`, `aac_reference_enabled`, `smoke_frames`, `smoke_differing_samples`, `built_at`
  (`tools/build-reference.sh:339-353`). `[VERIFIED: tools/build-reference.sh:339-353]`
  → The harness should either derive the corpus root from `iamfdec_path`, or (cleaner) the plan adds a
  `libiamf_src` key. Adding a key is additive; `tests/reference_manifest.rs` asserts on SHAs and
  `iamfdec_path` only (`tests/reference_manifest.rs:73-113`), so it will not break.

**Can the harness be written and committed while the corpus is absent?** Yes — and this is the same shape as
every other reference-gated test in the repo. The harness must print `SKIP <name>: …` and return when the
corpus directory is absent, exactly as `tests/conformance.rs:179-186` does for `IAMF_REF_DECODER`.

**The offline default:** `cargo test --locked` with no reference present must stay green on all four targets
(CONF-10, stated at `tests/conformance.rs:57-70`). A green offline run is therefore **not** conformance
evidence — which makes the SKIP path itself the highest-risk part of this task (see §3, policy note).

**Vendoring:** the harness must vendor **nothing new**. `tests/fixtures_cap.rs:83-106` walks
`tests/fixtures/reference/` and fails any file over the cap; `tests/fixtures_cap.rs:108-140` asserts the
`.iamf` count matches `MANIFEST.md` **and** that every on-disk filename is named in the manifest. The cap is
65 536 bytes (`tests/fixtures/MANIFEST.md:45`) and the manifest records that skipping the 154 over-cap files
keeps 594 MB out of every clone (`:50`). Compatibility rule for the plan: the harness reads only from
`.reference/…`, writes nothing under `tests/fixtures/`, and therefore is invisible to `fixtures_cap.rs`.
Any expectation data it needs goes in a **Rust table** (`tests/support/…`), not a fixture file.

**`IAMF_REFERENCE_ENABLE_AAC=1`:** it controls whether the bundled FDK-AAC archive is moved into place and
linked into `iamfdec` (`tools/build-reference.sh:100-126`, `:205`), and it sets the manifest's
`aac_reference_enabled` (`:348`). CI sets it (`.github/workflows/reference.yml:66`). It affects **decode
capability only** — the git checkout that supplies the corpus is identical either way. A parse/round-trip
harness sees the same 221 files with the flag on or off. `[VERIFIED: tools/build-reference.sh:100-126,348]`

---

## 3. Harness design — cost and risk of the four sub-goals

### (b) parse → serialise byte-identical — **CHEAP. Do it.**
Everything needed is already public and already used this way: `parse_sequence` + `write_parsed_sequence`
(imported at `tests/parse_reference.rs:18`, exercised at `:108-130`). Over 221 files this is ~40 lines. It is
also the highest-value-per-line item in the whole todo: it multiplies round-trip coverage from 40 real foreign
files to ~221, across codecs and layouts the vendored subset deliberately excludes (the vendored set is
LPCM-heavy by design — `tests/fixtures/MANIFEST.md:99-101`, "Phase 1 is LPCM-only, so this is the working
corpus").

Known caveat to encode in the plan: byte-identity has one documented exception —
"legal foreign non-minimal `obu_size` widths canonicalize" (STATE.md Phase 02 decision, `.planning/STATE.md:117`).
`test_000134` is the only reference configuration setting `GENERATE_LEB_FIXED_SIZE`
(`tests/fixtures/MANIFEST.md:~194`), so at most a handful of corpus files can hit it — but the harness must
classify that outcome rather than fail on it.

### (a) accept/reject vs `is_valid` — **CHEAP TO READ, EXPENSIVE TO GET RIGHT.**

The field extraction is trivial. `is_valid` and `is_valid_to_decode` are flat scalars inside
`test_vector_metadata { … }` at the top of every vector textproto
(`tests/fixtures/reference/test_000003.textproto:14-21`). A 10-line line-scanner is sufficient; **no textproto
parser is needed for this sub-goal.**

The policy is the hard part, and the evidence is already on disk:

```
$ for f in tests/fixtures/reference/*.textproto; do ...; done | grep -v 'true decode=true'
test_000000_3 is_valid=false decode=false     test_000119 is_valid=false decode=true
test_000007   is_valid=false decode=false     test_000120 is_valid=false decode=true
test_000016   is_valid=false decode=false     test_000122 is_valid=false decode=true
test_000017   is_valid=false decode=true      test_000124 is_valid=false decode=false
test_000063   is_valid=false decode=false     test_000129 is_valid=false decode=true
test_000085   is_valid=false decode=false     test_000130 is_valid=false decode=true
```

Cross-referencing `tests/support/reference_expectations.rs:54-207` and `:209-222`: **11 of those 12 are in
`POSITIVE_EXPECTATIONS`** (we parse them, produce a semantic sha, and in `test_000063`'s case emit a `Finding` —
`tests/parse_reference.rs:140-157`). Only `test_000129` is a structural reject, and its disposition is
`UnexpectedEndOfInput` at offset 53 (`tests/support/reference_expectations.rs:210-217`) — *despite* its
textproto declaring `is_valid_to_decode: true`.

So the correct model is a **four-cell disposition matrix**, not an equality:

| | `is_valid: true` | `is_valid: false` |
|---|---|---|
| we **parse** cleanly, no findings | expected (majority) | expected — the defect is semantic, not structural; the vector is generated deliberately |
| we **parse** and emit `Finding`s | **investigate** — either our finding is wrong or the vector is |
| we **reject** structurally | **red** — a real bug, this is the failure mode the harness exists to catch | acceptable — record which field, at which offset |

Two policy points the plan must lock:

1. **The DIFF-LEDGER.md is the wrong home** for stricter-than-libiamf rejections. It is a byte-offset table
   scoped to exactly one comparison — our encoder's output vs `encoder_main`'s for the sample-identity
   fixture — and it is asserted as an exact set in both directions (`DIFF-LEDGER.md:1-25`; assertion at
   `tests/conformance.rs:2963`). Adding a strictness row would break its schema and its test.
   The **right** home already exists: `CONFORMANCE-GATE.md` § "Reference limits diagnosed, not enforced"
   (`CONFORMANCE-GATE.md:646-650`), whose opening line is literally "An entry here is not a waiver, because no
   CONF clause is downgraded, and it is not a `DIFF-LEDGER.md` entry, because no byte differs." The
   `kMaxNumParameters = 256` entry (`:652-690`) is a complete, citable template: Spec → iamf-tools →
   libiamf → Decision → Consequence for consumers → What asserts it → Revisit when.
2. **The ledger must be committed, not computed.** Following the `reference_expectations.rs` precedent, the
   per-vector disposition belongs in a Rust table so a drift is a red diff rather than a silently-changed
   printout. That table is ~221 rows; it is the bulk of the task's line count.

### (c) encoder conformance from each `.textproto` — **A RESEARCH PROJECT. Do not attempt here.**

| Obstacle | Grounding |
|---|---|
| Needs a **reader** for the full `UserMetadata` dialect | The only existing textproto code is a *writer* handling exactly one Codec Config / one Audio Element / one Mix Presentation / one sub-mix / one sub-element, LPCM only, and it errors on any other codec (`tests/conformance.rs:2698-2735`) |
| No textproto crate is on the allow-list | Dev-deps today are `hex-literal`, `proptest`, `bitstream-io`, `sha2`, `syn =2.0.119`, `toml =0.8.23` (`Cargo.toml` `[dev-dependencies]`), each with its licence on the line that adds it. A textproto parser would have to be **hand-rolled in `tests/support/`** — the allow-list in `deny.toml` is allow-only, and adding a proto crate pulls a dependency tree |
| **Two incompatible proto dialects** | `tests/fixtures/MANIFEST.md:65-71`: libiamf's textprotos are the **old** dialect and still carry `count_label`, `num_substreams`, `num_layers`, `num_sub_mixes`, `num_audio_elements`, `num_layouts`, `param_definition_size`. "Do not feed them to `encoder_main`." A reader must therefore handle the old dialect, while `textproto_for` emits the new one |
| Coverage ceiling is the LPCM subset | This crate frames pre-encoded FLAC/Opus/AAC-LC access units and never encodes them (root `CLAUDE.md`, "Decoding is out of scope"; STATE.md Phase 03 decision, `.planning/STATE.md:124`). Every non-LPCM vector is permanently on the skip list |
| Source PCM resolution | `encoder_main` consumes `audio_frame_metadata.wav_filename`; our encoder needs the same PCM. Resolving that per vector requires the full parse, not a field scan |

Honest estimate: this is comparable in size to a planned phase plan, not a quick-task item.

### (d) `coverage.csv` per-spec-field report — **CHEAP BUT INERT. Defer.**
`primary_tested_spec_sections` is already a flat list in every textproto
(`tests/fixtures/reference/test_000003.textproto:26-39`), and `coverage.csv` is a plain CSV in the corpus. The
join is ~80 lines. But it produces a **printout nobody asserts**, and this project's own doctrine is hostile to
that: "A guard that is green because it sees nothing is a defect." Build it only once there is a committed
floor to assert against — otherwise it is documentation that rots.

### The SKIP-path defect risk (applies to the whole harness)
A corpus harness that skips is indistinguishable from one that passes. The existing precedent for closing that
hole is a **CI step that asserts the skip did not happen** — see `.github/workflows/reference.yml:84`,
"Assert the reference decoder has FLAC and Opus (CONF-05 may not skip here)". Mirror it: a step in the Linux
reference job that greps the harness output for a walked-file count and fails below a floor. That is cheaper
and better-precedented than a `tools/prove-guards.sh` case, and prove-guards' 9 cases are all `src/`-side
clippy/licence proofs (`tools/prove-guards.sh:15-29`) — a corpus harness does not fit that harness's shape.

---

## 4. Scope split — this does NOT fit one quick task

A quick task is one plan with 1–3 focused tasks. The todo, as written, is:

| Item | Verdict |
|---|---|
| 1(a) accept/reject + 1(b) round-trip, over the fetched corpus | **Fits** — 2–3 tasks, one new test binary, one ledger, one CONFORMANCE-GATE section, one CI assertion |
| 1(c) encoder conformance from textproto | **Own todo, phase-sized.** Needs a hand-rolled old-dialect textproto reader, a documented skip list, and source-WAV resolution |
| 1(d) coverage.csv report | **Own quick task, and only after (a)/(b) land** — it needs the disposition ledger to have something to report against |
| 2 `tests/support/builders.rs` consolidation | **Own quick task.** Must not share a commit with the harness |
| 3 table-driving profile tiers / presentation rules | **Own quick task**, after item 2 |

Reason item 2/3 must be separate: the harness **adds** test surface while consolidation **moves** it. In one
diff, a regression is ambiguous between "the new harness found a real bug" and "the refactor dropped an
assertion" — which is the exact failure mode a consolidation of 22 000 lines of tests is most likely to
produce.

---

## 5. File-overlap risk (the AAC-LC merge, `474004a`)

The merge commit's own message names the conflicted files: `src/encoder.rs`, `tests/encoder_builder.rs`,
`tests/encoder_streaming.rs`, `tests/round_trip.rs` (`git show 474004a`). It also touched
`.github/workflows/reference.yml` (+4) and `REFERENCES.md` (+9). Current shape of the three files this task
wants:

| File | Current shape | Overlap risk |
|---|---|---|
| `tests/conformance.rs` | **3 178 lines.** Owns `assert_conformant` (`:1070`), reference discovery (`:106-203`), WAV I/O (`:367-500`), the AAC-LC round-trip gate (`:1909`), expanded-layout decode (`:2205`), and the whole CONF-07 textproto/encoder_main/byte-diff machinery (`:2643-3180`) | **High if the harness is added here.** Recommendation: **new binary `tests/refvectors.rs`**, not an addition to `conformance.rs`. That file is already the largest in the repo and its concerns are decode-oracle, not corpus-walk |
| `tests/support/` | 5 files: `fixture.rs` (1 290), `sequence_cases.rs` (390), `parallax_contract.rs` (293), `reference_expectations.rs` (286), `test_000003.rs` (235). **No `builders.rs` exists yet** | Low. A new `tests/support/vector_ledger.rs` is additive |
| `tools/build-reference.sh` | 368 lines. Emits the 12-key manifest at `:339-353`. AAC plumbing at `:100-126`, `:205` | Low — the only change needed is one additive manifest key (`libiamf_src`) |

Two concrete traps:

- **Do not add a `[[test]]` entry to `Cargo.toml`.** The only one is `fuzz_regression`, gated by
  `required-features = ["fuzzing"]` (`Cargo.toml` `[[test]]`). `tools/prove-guards.sh:67-77` copies only
  `Cargo.toml`, `Cargo.lock`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` and `src/` into the proof dir
  and then runs `cargo clippy --all-targets` (`:109`). An **unconditional** `[[test]]` entry pointing at a
  `tests/` file that the proof dir does not have would break all 9 cases. Rely on Cargo auto-discovery.
- `tests/fixture.rs` (947 lines) and `tests/support/fixture.rs` (1 290 lines) are **different files**. Do not
  conflate them when planning consolidation.

---

## 6. Consolidation (todo items 2 and 3) — re-measured

The tree grew ~9 % since the todo was written (2026-09-14 → 2026-09-24):

| Metric | Todo (2026-09-14) | Today | Command |
|---|---:|---:|---|
| `tests/**/*.rs` lines | 20 187 | **22 017** | `cat tests/*.rs tests/support/*.rs \| wc -l` |
| `#[test]` attributes | 469 | **609** | `grep -rc '#\[test\]'` over the same set |
| `.kind()` asserts | 113 | **182** | `grep -rc '\.kind()' tests/*.rs` |
| `err.at()` asserts | 84 | **20** *(literal `err.at()` only; the todo's 84 likely counted other receivers)* | `grep -rho 'err\.at()' tests/*.rs \| wc -l` |
| `EncoderBuilder::new()` in `encoder_builder.rs` | 52 | **54** | `grep -c` |
| `add_codec_config(lpcm_config())` in `encoder_builder.rs` | 49 | **50** | `grep -c` |
| `bytes_written()` in `encoder_streaming.rs` | 22 | **47** | `grep -c` |

Helper duplication, re-measured (`grep -rn "fn <name>" tests/`):

| Helper | Todo | Today | Locations |
|---|---:|---:|---|
| `lpcm_config` | ×4 | **×4** | `encoder_streaming.rs`, `support/fixture.rs` *(different signature — takes `id, sample_size, flags`)*, `parallax_contract.rs`, `encoder_builder.rs` |
| `stereo_element` | ×3 | **×3** + `stereo_elements(n)` in `profile.rs` | `parallax_contract.rs`, `encoder_builder.rs`, `support/parallax_contract.rs` *(takes `substream_label`)* |
| `presentation*` | ×3 | **×8 distinct signatures** | `sequence_parse.rs` ×4, `profile.rs`, `encoder_streaming.rs`, `parallax_contract.rs`, `encoder_builder.rs` ×2, `support/parallax_contract.rs` |
| `sha256_hex` | ×3 | **×3, identical bodies** | `golden.rs:82`, `fuzz_regression.rs:176`, `parse_reference.rs:102` |
| `rust_files_under` | ×3 | **×3** | `profile.rs`, `citations.rs:78`, `allocation_bounds.rs` |
| `repo_root` | ×2 | **×2** | `reference_manifest.rs:22`, `fixtures_cap.rs:14` |
| `sha_on_line_naming` | ×2 | **×2** | `reference_manifest.rs:32`, `conformance.rs:1227` |
| `PartialThenFail` | ×2 | **×2** | `sequence.rs:261`, `encoder_streaming.rs:242` |
| `assert_error` / `assert_rejected` | proposed | **do not exist** | — |

Note that `lpcm_config`, `stereo_element` and `presentation` are **not** true duplicates — the signatures
differ across files. Consolidation is a genuine design exercise (pick a parameterised form, migrate ~100 call
sites), not a mechanical dedup. `sha256_hex`, `rust_files_under`, `repo_root`, `sha_on_line_naming` and
`PartialThenFail` *are* true duplicates and are the safe, mechanical subset (~10 call sites total).

### Do the guards get disturbed by test-side refactoring? **No.**

- `tests/citations.rs` walks **`src/` only**: `for file in rust_files_under(Path::new("src"))`
  (`tests/citations.rs:37`; module doc at `:10` says the same). Test refactoring cannot touch it.
  `[VERIFIED: tests/citations.rs:33-63]`
- `tools/prove-guards.sh` copies **`src/` only** into the proof crate (`tools/prove-guards.sh:67-77`) and all
  nine cases inject violations into that copy (`:15-29`). Test refactoring cannot touch the 9 PASS count —
  **except** via the `[[test]]`-entry trap described in §5. `[VERIFIED: tools/prove-guards.sh:67-77,109]`
- `tools/check-float-escape-census.sh` must report exactly one hit in `src/model/loudness.rs` (root
  `CLAUDE.md`). Also `src/`-scoped.

One real consolidation risk that is **not** a guard: `clippy.toml`'s test carve-out
(`allow-unwrap-in-tests` / `allow-expect-in-tests` / `allow-panic-in-tests`) applies inside `#[test]` functions
**only**. Several files already note this and keep helpers total — `tests/fixtures_cap.rs:18-21`
("every helper below is total: it returns `Option`/`Result` and the tests do the asserting") and
`tests/refcorpus.rs:53-57`. Moving a panicking assertion out of a `#[test]` body into a shared
`tests/support/builders.rs` helper will trip `clippy::panic`/`unwrap_used` under `-D warnings`. The
consolidation plan must budget for that: shared helpers return `Result`, or the assertion macro stays a macro
that expands **inside** the `#[test]` body.

---

## Don't hand-roll / don't re-derive

| Problem | Don't build | Use instead |
|---|---|---|
| Locating the corpus | A new env var | Derive from `.reference-manifest.json`'s `iamfdec_path`, or add a `libiamf_src` key (`tools/build-reference.sh:339-353`) |
| Reporting "no reference present" | `#[ignore]` | The repo's `println!("SKIP …")`-and-return convention (`tests/conformance.rs:179-203`) |
| Reading `is_valid` from a textproto | A textproto parser | A line scanner over `test_vector_metadata` (same shape as `manifest_raw_value`, `tests/conformance.rs:1251`) |
| Recording a stricter-than-libiamf rejection | A `DIFF-LEDGER.md` row | A `CONFORMANCE-GATE.md` § "Reference limits diagnosed, not enforced" entry, templated on `:652-690` |
| A drift-proof expectation table | A generated snapshot | The committed-`const`-table pattern of `tests/support/reference_expectations.rs` |
| A SHA helper | A fourth `sha256_hex` | There are already three identical ones; pick one home first |

---

## Common pitfalls (specific to this task)

1. **`tests/vectors.rs` is a name trap.** It is hand-computed bit primitives (`tests/vectors.rs:1-15`), not the
   vector corpus. Adding corpus tests there would bury D-25's "primary derivation" doctrine.
2. **`is_valid: false` ≠ "must be rejected".** 11 of 12 such vendored vectors parse cleanly today. Asserting
   equality would produce 11 immediate false failures and pressure toward loosening a correct parser.
3. **A skipping harness is a green harness.** Without a CI non-skip assertion the whole thing measures nothing.
4. **`DIFF-LEDGER.md` is asserted as an exact set in both directions** (`DIFF-LEDGER.md:9-18`) — an extra row
   fails `tests/conformance.rs:2963`, not just the documentation review.
5. **Never `git add -A`.** `docs/` holds untracked upstream clones (`ls docs/` → `assets`,
   `IAMF-V1.1-COMPLETENESS-AUDIT.md`, `superpowers` today, but the CLAUDE.md warns clones live there).
6. **Licence:** libiamf is BSD-3-Clause-Clear + AOM Patent 1.0 (`tests/fixtures/MANIFEST.md:13-16`) and is on
   the **readable** list. Reading its `tests/*.textproto` and `coverage.csv` is permitted at the pinned SHA.
   `gpac` / `libspatialaudio` / FFmpeg remain unopenable.

---

## Open questions

1. **Is the corpus really 221 `.iamf`?** Unverifiable offline — `.reference/` is absent. `[ASSUMED]` from the
   todo. The harness must therefore assert a **floor** it discovers on first run and commits, not a number
   copied from the todo.
2. **Do all 221 have a paired `.textproto`?** `test_000076_aac_lc.iamf` is vendored *without* one, and
   `test_000134` has a textproto with *no* `.iamf` (`tests/fixtures/MANIFEST.md:~194`). So the pairing is not
   a bijection upstream either. The harness needs an explicit "unpaired" disposition cell.
3. **How many vectors hit the non-minimal-`obu_size` canonicalisation caveat?** Unknown until first run.
   Plan for it as a disposition, not a failure.

---

## Recommended slice for this quick task

**Ship (a) + (b) as one new gated test binary. Nothing else.**

Three tasks:

1. **`tests/refvectors.rs`** — a new test binary (no `[[test]]` manifest entry; Cargo auto-discovery).
   Locates the corpus at `.reference/libiamf/tests` (via `.reference-manifest.json`, with an additive
   `libiamf_src` key added in `tools/build-reference.sh`); prints `SKIP` and returns when absent. For every
   `test_*.iamf` in sorted order: `find_obu_boundaries` end-equals-length, `parse_sequence` →
   `write_parsed_sequence` byte-identity, `validate()` finding count, and the `is_valid` /
   `is_valid_to_decode` pair read by a ~10-line field scanner. Classifies each file into the four-cell
   disposition matrix from §3 and prints a summary with the walked count.

2. **`tests/support/vector_ledger.rs`** — the committed per-vector disposition table, in the
   `reference_expectations.rs` style, asserted as a bijection against what is on disk. Populated from the
   first real run against the built corpus; the "we reject a vector libiamf accepts" cells get one
   `CONFORMANCE-GATE.md` § "Reference limits diagnosed, not enforced" entry each, on the
   `kMaxNumParameters` template (`CONFORMANCE-GATE.md:652-690`).

3. **CI non-skip assertion** — one step in `.github/workflows/reference.yml`, modelled on the existing
   "CONF-05 may not skip here" step at `:84`, that fails the Linux job if the harness reports fewer than the
   committed floor of walked vectors.

**Why this slice.** It is the only part that is cheap *and* delivers conformance value standing alone: it takes
real foreign round-trip coverage from 40 files to ~221, across codecs and layouts the vendored subset
deliberately excludes, using only machinery that already exists and adding **zero dependencies, zero vendored
bytes and zero new textproto code**. It also produces the artifact that every deferred item needs — the
disposition ledger — so (c) and (d) become tractable follow-ups rather than open-ended ones.

**Becomes follow-up work:**

| Item | Destination |
|---|---|
| (c) encoder conformance from `.textproto` | New todo. Phase-sized: hand-rolled old-dialect textproto reader, LPCM-only skip list, source-WAV resolution |
| (d) `coverage.csv` per-spec-field report | New quick task, after the ledger exists so the report has a floor to assert |
| Item 2 — `tests/support/builders.rs` | New quick task. Start with the 5 true duplicates (`sha256_hex`, `rust_files_under`, `repo_root`, `sha_on_line_naming`, `PartialThenFail`); budget for the `clippy.toml` test-carve-out boundary |
| Item 3 — table-driving profile tiers / presentation rules | New quick task, after item 2 |
