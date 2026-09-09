---
phase: 02-parser-round-trip-and-fuzzing
verified: 2026-09-09T03:49:07Z
status: passed
score: 5/5 must-haves verified
---

# Phase 2: Parser, Round-Trip and Fuzzing Verification Report

**Phase Goal:** A bitstream can be read back into the model and re-emitted unchanged, foreign files are understood rather than merely tolerated, and the parser is continuously fuzzed.

**Verified:** 2026-09-09T03:49:07Z
**Status:** passed

## Goal Achievement

### Observable Truths

ROADMAP success criteria are the authoritative truths. Plan-level must-haves were checked as supporting contracts and do not weaken those criteria.

| # | Truth | Status | Fresh evidence |
|---|---|---|---|
| 1 | Both round-trip directions hold at their promised strengths, with the foreign non-minimal-ULEB128 caveat explicit | ✓ VERIFIED | `PROPTEST_CASES=256 cargo test --locked --test round_trip -- --nocapture` passed all 8 tests. The three properties use derived `PartialEq`, cover both parsed and streaming writer paths, and the focused foreign-width test proves legal non-minimal `obu_size` canonicalizes instead of overpromising universal foreign-byte identity. |
| 2 | Every valid committed reference file is semantically understood, and invalid files keep exact dispositions | ✓ VERIFIED | Fresh all-target execution passed all 5 `parse_reference` tests. The inventory test enforces exactly 37 positives plus 2 negatives; complete ordered field ledgers, raw codec boundaries/digests, exact negative causes, and the full `test_000059` Demixing/Recon Gain projection are asserted. |
| 3 | Unknown OBU and parameter data bytes remain at their original positions | ✓ VERIFIED | The round-trip sentinel passed with exact variant indices, payload slices, extension bytes, known trailing bytes, and absolute OBU boundaries. `test_000015` passed as a positive with its bounded ungoverned type-3 OBU at index 4 and byte-identical re-emission; governed corrupt syntax remained a structural error. |
| 4 | Parameter Block syntax depends explicitly on descriptor context | ✓ VERIFIED | Source inspection confirms the only lower-level reader signature is `read_parameter_block(r, &ParamDefinitionRegistry)`. The registry retains ordered duplicates, binds lookup first-in-wire-order, and carries Mix Gain, Demixing, Recon Gain layer gates, and bounded Reserved context. Focused parser/temporal tests passed. |
| 5 | Two isolated fuzz targets use committed real/model corpora, stable four-target replay, and separate bounded discovery fuzzing | ✓ VERIFIED | Stable corpus replay passed 4/4 tests; both isolated cargo-deny policies and lock checks passed; the fuzz manifest contains exactly two bins. Fresh runs of both targets completed 10,000 iterations each without a crash. CI inspection confirms replay in the four-target stable matrix and a schedule/manual-only Linux nightly workflow with 300-second target bounds and artifact upload. |

**Score:** 5/5 authoritative truths verified.

## Requirement Coverage

| Requirement | Status | Evidence |
|---|---|---|
| PARSE-01 | ✓ SATISFIED | `parse_sequence(&[u8]) -> Result<ParsedSequence>` consumes the whole input into an owned flat wire-order model. Truncated suffix and absolute-location regressions pass, and no partial public prefix is returned. |
| PARSE-02 | ✓ SATISFIED | `read_parameter_block` has a mandatory `&ParamDefinitionRegistry` argument. Registry order, duplicate first binding, extension prefix, Recon Gain context, and all parameter-data forms have passing focused tests. |
| PARSE-03 | ✓ SATISFIED | The 256-case `model_round_trip` property compares the parsed value to the generated canonical model using derived equality. |
| PARSE-04 | ✓ SATISFIED | Both crate writer paths pass parse/re-serialize byte-identity properties; rustdoc and a dedicated vector document and prove the foreign non-minimal-width boundary. |
| PARSE-05 | ✓ SATISFIED | `UnknownObu` owns the full header and bounded payload, stays in the flat vector, and passes exact position/offset assertions. |
| PARSE-06 | ✓ SATISFIED | Governed Reserved parameter data preserves length-bounded raw subblocks; missing-context type-3 data uses the explicit ungoverned variant. Tests prove neither path becomes a fallback for malformed governed syntax. |
| PARSE-07 | ✓ SATISFIED | The 37-positive/two-negative ledger is a filesystem bijection with ordered semantic assertions. Non-LPCM configs remain raw with exact FourCC, position, length, and SHA-256. |
| FUZZ-01 | ✓ SATISFIED | `fuzz/` is excluded from the root workspace and has independent manifest, lockfile, and deny policy. `libfuzzer-sys` is absent from the root lock and present only in the fuzz lock. |
| FUZZ-02 | ✓ SATISFIED | The hostile-byte `parse_sequence` target invokes the production whole-input parser for every input slice; its fresh 10,000-run session completed without a crash. |
| FUZZ-03 | ✓ SATISFIED | The `obu_roundtrip` target uses the shared bounded canonical generator and asserts model plus byte equality; its fresh 10,000-run session completed without a crash. |
| FUZZ-04 | ✓ SATISFIED | Four parser seeds are byte-identical copies of pinned positive `iamf-tools` fixtures, and eleven named model seeds cover the required canonical and unknown-placement shapes. Stable replay verifies source digests and rejects empty, unreadable, skipped, or symlinked inputs. |
| FUZZ-05 | ✓ SATISFIED | The stable replay test recursively includes committed artifact directories and is wired into all four Phase 1 target paths. Coverage discovery is isolated in a scheduled/manual Linux nightly workflow and uploads artifacts/logs. |

**Coverage:** 12/12 Phase 2 requirements satisfied.

## Adjudicated Corpus Contract

The implementation matches the evidence-backed 37-positive/two-negative ruling:

- `test_000015.iamf` remains positive. Its complete bounded type-3 payload lacks a governing definition, so `parse_sequence` preserves it as `UngovernedParameterBlock`; `validate()` emits the missing-definition finding and `write_parsed_sequence` reproduces its bytes and position.
- This preservation is limited to missing context. A block governed by a known definition still fails structurally when its known syntax is corrupt; there is no raw fallback.
- `test_000129.iamf` remains a structural negative because its pinned paired metadata marks it invalid and its input truncates at offset 53.
- `negative/tones_256samp_5p1_pcm.iamf` remains a semantic negative: it parses, preserves zero frame size, and receives the exact validation finding.

## Artifact and Wiring Verification

| Area | Status | Evidence |
|---|---|---|
| Flat parser and writer | ✓ EXISTS + SUBSTANTIVE + WIRED | `src/sequence.rs` dispatches all known variants through the central bounded OBU readers, preserves unknown and ungoverned variants, validates flat order, and writes directly in vector order. |
| Contextual parameter data | ✓ EXISTS + SUBSTANTIVE + WIRED | `ParamDefinitionRegistry` is ordered and explicit; adjacent read/write implementations cover Mix Gain, Demixing, Recon Gain, and bounded Reserved data. |
| Fidelity and properties | ✓ EXISTS + SUBSTANTIVE + WIRED | Reserved owners, canonical generators, derived equality properties, own-byte properties, fixed-width caveat, unknown sentinels, and structural negatives are all executable. |
| Foreign corpus understanding | ✓ EXISTS + SUBSTANTIVE + WIRED | Provenance, expectation ledger, inventory bijection, semantic projections, raw-codec digests, and exact negative tests are committed and exercised. |
| Fuzz workspace and CI | ✓ EXISTS + SUBSTANTIVE + WIRED | Independent manifest/lock/deny files, exactly two targets, shared model generator, permanent corpora, exhaustive stable replay, four-target CI integration, and separate nightly discovery workflow are connected. |

All 30 plan-level truth statements across plans 02-01 through 02-07 are supported by source inspection or fresh executable evidence. No prohibition was found violated.

## Fresh Verification Metadata

- `cargo test --locked --all-targets` — PASS, 318 tests on the default feature graph.
- `cargo test --locked --features fuzzing --test fuzz_regression -- --nocapture` — PASS, 4/4 tests; combined current suite evidence is 322 passing tests.
- `PROPTEST_CASES=256 cargo test --locked --test round_trip -- --nocapture` — PASS, 8/8 tests with all three 256-case properties.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — PASS.
- `cargo deny --all-features check` — PASS: advisories, bans, licences, and sources.
- `cargo deny --manifest-path fuzz/Cargo.toml --config fuzz/deny.toml --all-features check` — PASS: advisories, bans, licences, and sources.
- Fuzz workspace metadata, two-bin count, root/fuzz lock isolation, and `git diff --check` — PASS.
- `cargo +nightly-2026-09-01 fuzz run parse_sequence <temporary-corpus-copy> -- -runs=10000` — PASS, 10,000 runs without a crash.
- `cargo +nightly-2026-09-01 fuzz run obu_roundtrip --features roundtrip-model <temporary-corpus-copy> -- -runs=10000` — PASS, 10,000 runs without a crash.

The fuzz runs used temporary copies of the committed corpora, so coverage-generated entries did not modify the repository. Four-target replay is verified as CI wiring; this report does not claim fresh local execution on operating systems unavailable to this verifier.

## Human / External Verification

No human-only decision blocks Phase 2. Scheduled discovery and four-target jobs remain ongoing CI operations by design; their deterministic stable replay command and workflow routes are present and locally validated on the available host.

## Anti-Patterns and Residual Risk

| Finding | Severity | Disposition |
|---|---|---|
| Proptest prints a `SourceParallel` persistence warning for integration tests | ℹ️ Info | The properties execute and pass; the warning concerns where future failing seeds are persisted, while the repository has an explicit fuzz-artifact minimization/retention policy. |
| Four-target replay was inspected rather than executed locally on four OS/architecture paths | ℹ️ Info | The existing matrix contains all four inherited targets and invokes the exact stable replay command. Platform execution is the CI system's responsibility, not an implementation gap. |

**Blockers:** none.

## Overall Decision

**passed.** The actual implementation satisfies all five Phase 2 roadmap truths and all twelve PARSE/FUZZ requirements. The parser is transactional and wire-faithful at the promised boundary, the foreign corpus is semantically classified and asserted, unknown syntax preserves position, and permanent stable replay plus bounded coverage fuzzing are wired without contaminating the shipping dependency graph.

---
*Verified: 2026-09-09T03:49:07Z*
*Verifier: Codex (fresh goal-backward Phase 2 verification)*
