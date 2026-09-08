---
phase: 01-conformant-lpcm-bitstream
plan: 08
subsystem: conformance
tags: [iamf, conformance, golden, byte-identity, reference-oracle, diff-ledger, waiver]

requires:
  - phase: 01-05
    provides: the four descriptor OBUs, DescriptorSet and write_descriptors' ordering
  - phase: 01-06
    provides: SubstreamPlan, pack_channels_to_substreams and plan_frames
  - phase: 01-07
    provides: SequenceWriter, write_sequence, select_minimum_profile and the test_000003 reproduction
provides:
  - "iamf::dump::dump_annotated — D-20's deterministic annotated structural dumper"
  - "assert_conformant(&DescriptorSet, &[i32]) — CONF-01's reusable harness, reused unchanged by Phase 3"
  - "tests/support/fixture.rs — the three Phase 1 fixtures, one encode path, and D-19's channel diagnostic"
  - "tests/fixtures/golden/ — GUARD-09's three committed artifacts"
  - "DIFF-LEDGER.md — D-12's executable byte-diff ledger, asserted in both directions"
  - "CONFORMANCE-GATE.md § Experiment 3, waiver W-1 and the Phase 1 exit table"
affects: [phase-02-parser, phase-03-codecs, parallax-export-adapter]

actuals:
  tokens: 59000
  tasks: 3
  commits: 5
plan_head_before: cf170c72be1cf639b2b2f06573aeab14c081462d

tech-stack:
  added:
    - "sha2 0.10.9 (MIT OR Apache-2.0) — DEV-dependency only, GUARD-09's golden digest"
  patterns:
    - "The reference's verdict is never its exit code: assert file-exists, then sample-count, then sample-identity, each with its own message"
    - "A committed document that a test asserts, in both directions, so it cannot drift from what it describes"
    - "Diagnose an upstream defect precisely enough to assert the diagnosis, rather than recording that something differs"
    - "Generate the reference tool's input description from our own model, so the two configurations cannot drift apart"

key-files:
  created:
    - src/dump.rs
    - tests/conformance.rs
    - tests/fixture.rs
    - tests/golden.rs
    - tests/support/fixture.rs
    - tests/fixtures/golden/phase1_sample_identity.iamf
    - tests/fixtures/golden/phase1_sample_identity.iamf.sha256
    - tests/fixtures/golden/phase1_sample_identity.dump.txt
    - DIFF-LEDGER.md
  modified:
    - src/lib.rs
    - Cargo.toml
    - CONFORMANCE-GATE.md
    - .github/workflows/ci.yml
    - .github/workflows/reference.yml
    - tests/fixtures/MANIFEST.md
    - .planning/phases/01-conformant-lpcm-bitstream/01-CONTEXT.md

decisions:
  - "Research assumption A1 is REFUTED, and the cause is upstream: libiamf@v1.1.0's reads24be uses readu16le where readu16be was meant, so 24-bit big-endian LPCM is misread with its top two bytes transposed"
  - "The sample-identity fixture is 24-bit LITTLE-endian; a third stereo 16-bit BIG-endian fixture keeps DESC-03's endianness sense asserted end to end"
  - "Waiver W-1 covers CONF-05 for the 24-bit big-endian sample format only, with an executable restoration condition"
  - "CONF-07's byte diff is EMPTY: our output is byte-identical to iamf-tools' encoder_main for the same configuration"
  - "The CONF-07 textproto is generated from our own DescriptorSet rather than committed, so two descriptions of one configuration cannot drift"

metrics:
  duration: ~2h
  completed: 2026-09-08

status: complete
---

# Phase 1 Plan 08: Conformance Harness, Fixtures and the Exit Gate Summary

The seven-clause gate is built, executed, and passed — and CONF-07 came back
**byte-identical to `iamf-tools`' own encoder**, which is the strongest form of
the Core Value's real exit criterion rather than the weakest.

## What was built

`assert_conformant(&DescriptorSet, &[i32])` is a **function, not a test body**,
and carries no codec-specific constant: the sample rate, sample size, frame size
and `-s` output layout it hands the reference are all read back out of the Codec
Config it just wrote. That is CONF-01's whole point — Phase 3 reuses it unchanged
for FLAC and Opus, and a single hard-coded `48000` would have made it an
LPCM-shaped function pretending to be a general one, with the failure surfacing
only in Phase 3.

Three fixtures, a dumper, three committed golden artifacts, an executable diff
ledger, and CI wiring on both workflows.

## The finding that reshaped the plan

**Task 1 was gating, ran first, and refuted the assumption everything rested on.**

Research recorded A1 — "the reference's 24-bit float→int conversion is exact" —
as its highest-risk `[ASSUMED]` claim, and D-18 chose 24-bit **big-endian**
precisely to cover DESC-03's endianness sense. The probe ran before the fixture
was frozen, as the plan required, because the golden hash, the annotated dump and
the diff ledger are all derived from the fixture.

A1 is **refuted — but not where it was expected**. The output conversion is fine.
The input read is not:

```c
/* libiamf@v1.1.0 code/src/iamf_dec/bitstream.c:206-210 */
int reads24be(uint8_t *data, int offset) {
  uint32_t ret = readu16le(data, offset) << 8 | data[offset + 2];
  /*             ^^^^^^^^^ readu16be was meant */
```

Its unsigned sibling `readu24be`, two lines above, uses `readu16be` and is
correct. So **24-bit big-endian LPCM is decoded by the pinned reference with its
top two bytes transposed**, and nothing else is affected — `reads16be` uses
`readu16be`, `reads24le`/`reads16le` use `readu16le`, all correct.

What matters is what the probe asserts. It does not record that the PCM differs;
it asserts that **all 600 decoded samples equal what that defective code computes
from the bytes we wrote**. That is a much stronger claim, and it is what proves
our encoder wrote correct big-endian bytes and the reference misread them —
rather than the reverse, which "the PCM differs" would have left open.

Three probes, all executed and committed as tests:

| probe | format | frames | differing | verdict |
|---|---|---|---:|---|
| `probe_24bit_little_endian_round_trips_exactly` | 24-bit LE | 300/300 | **0 of 600** | the depth is exact |
| `probe_16bit_big_endian_round_trips_exactly` | 16-bit BE | 300/300 | **0 of 600** | the endianness sense is exact |
| `probe_24bit_big_endian_hits_the_upstream_reads24be_defect` | 24-bit BE | 300/300 | 597 of 600 | fully explained by the defect |

**The 24-bit big-endian combination was not silently dropped.** Waiver W-1 in
`CONFORMANCE-GATE.md` carries all five D-17 fields, and its restoration condition
is *executable*: the defect probe asserts `differing > 0`, so the day `reads24be`
is fixed upstream that test fails with a message pointing back at the waiver.

## The result CONF-07 produced

**Zero differing offsets.** Our `.iamf` for the sample-identity fixture is 7073
bytes; `iamf-tools@v2.1.0`'s `encoder_main`, given the same configuration and the
same PCM, produced 7073 bytes with the same SHA-256
`3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c`.

PROJECT.md's exit criterion is "either identical or fully explained in writing".
This is met by identity — and it is materially stronger than passing `libiamf`,
which Experiment A showed accepts a file with a flipped reserved bit without a
murmur.

It was checked for the obvious false pass. `run_encoder_main` refuses to run if
the companion path already exists, and requires `encoder_main`'s per-layout
rendered WAVs to be present afterwards, so a stale or planted `.iamf` cannot pass
as a fresh reference encode. The scratch directory holds two rendered WAVs — one
per sub-mix layout, which is the 5.1 fixture's Sound System B and its mandatory
Sound System A.

The CONF-07 textproto is **generated from the same `DescriptorSet` our encoder
wrote**, not committed. A committed copy would be a second description of one
configuration; the two would drift, and CONF-07 would then compare two files
built from *different* configurations and report the difference as an encoder
defect.

## The Phase 1 exit table

| clause | verdict | evidence |
|---|---|---|
| clause 0 (D-13) | PASS | reference pin confirmed before any other clause |
| CONF-02 | PASS | 6 channels, each distinguishable at every index, peak ≤ −6 dBFS |
| CONF-03 | PASS | `trim_at_end = 84`, `trim_at_start = 0`, and the two differ |
| CONF-04 | PASS | 16 OBUs on the sample-identity file; 11 OBUs with 2 Audio Elements on the structure file; both boundary walks land on `len()` |
| CONF-05 | PASS | **300 sample frames, 0 of 1800 samples differ, limiter delta 0** |
| CONF-06 | PASS | `decoder_main reported "Decoded 3 temporal units."` on all three fixtures |
| CONF-07 | PASS, diff EMPTY | 7073 = 7073 bytes, 0 differing offsets |
| CONF-08 | PASS | 32567 bytes, 68 boundaries, first Audio Frame at 120 |
| CONF-09 | PASS | no `build.rs`, no `[build-dependencies]`, no `bindgen`/`cmake`/`cc`/`pkg-config` |
| CONF-10 | PASS | `env -u IAMF_REF_DECODER cargo test --locked` green; every reference clause prints a skip |

CONF-05 passing on the **5.1** fixture is the one worth pausing on: it is
simultaneously a proof that the BCG channel→substream packing is right, which the
phase named as its highest silent-failure risk precisely because it produces a
clean decode with scrambled channels — no error, no crash, correct byte count.
The 6×6 identity render matrix means the decoder's output *is* our input, so the
comparison is direct equality with nothing to tune.

## No knob (D-19)

There is no permutation constant, no tolerance window and no gain adjustment
anywhere in the harness. `describe_channel_mismatch` reports
`ch4 (Ls) arrived as ch5 (Rs)` and points at `src/packing.rs`; it never applies a
permutation. Two tests prove it names a swap and two more prove it reports a
plain difference when no permutation explains one.

A source-level assertion backs this up:
`the_harness_source_contains_no_permutation_constant_or_tolerance` scans all
three harness files (comments stripped) for `CHANNEL_PERMUTATION`, `TOLERANCE`,
`EPSILON`, `abs_diff() <=` and friends. That check exists because a knob added
later would not fail any behavioural test — it would make one *pass*.

## Deviations from plan

### Auto-fixed issues

**1. [Rule 1 — Bug] The fixture signal never crossed zero**

- **Found during:** Task 2, by the plan's own property test.
- **Issue:** with a ramp step of 7919, 300 frames advance only 2.4 M of an
  8.4 M-sample span, so the 24-bit signal sat entirely negative and exercised
  about a quarter of its range. A wrong high byte could have hidden in the part
  it never visited.
- **Fix:** the step is now a prime that sweeps the whole fold span several times
  over `SAMPLE_FRAMES`, still coprime with the span at both 16 and 24 bits so no
  two channels ever coincide.
- **Files:** `tests/support/fixture.rs`
- **Commit:** `d33b6a7`

**2. [Rule 2 — Missing critical check] An empty `IAMF_REF_DECODER` was treated as a path**

- **Found during:** Task 2, while wiring `ci.yml`.
- **Issue:** `IAMF_REF_DECODER=` is what a CI step meaning "unset" usually writes.
  `env::var_os` returns `Some("")` for it, so the harness would have spawned the
  empty program and reported the failure as a *decode* failure — a misdiagnosis,
  in the one file that exists to prevent them.
- **Fix:** an empty value counts as unset; the workflow comment says so too.
- **Files:** `tests/conformance.rs`, `.github/workflows/ci.yml`
- **Commit:** `d33b6a7`

**3. [Rule 2 — Missing critical check] CONF-07 could have degraded into a self-comparison**

- **Found during:** Task 3, verifying the byte-identical result was genuine.
- **Issue:** nothing structurally prevented a future caller from writing our own
  output where the companion belongs. CONF-07 would then report perfect agreement
  while measuring nothing — worse than failing, because nothing downstream would
  learn the difference (T-01-48).
- **Fix:** `run_encoder_main` refuses to run if the output path already exists,
  and requires `encoder_main`'s per-layout rendered WAVs afterwards.
- **Files:** `tests/conformance.rs`
- **Commit:** `f4fcabe`

**4. [Rule 3 — Blocking] Clause 0 matched the wrong key**

- **Found during:** Task 3. `REFERENCES.md` names the repository (`iamf-tools`);
  `.reference-manifest.json` uses a JSON key (`iamf_tools_sha`). One needle for
  both made clause 0 fail always — a different way of not running the gate.
- **Fix:** each side gets its own needle, as `tests/reference_manifest.rs`
  already did.
- **Commit:** `f4fcabe`

### Deliberate design deviations

**D-18's "24-bit big-endian" is not met.** Recorded above, in
`CONFORMANCE-GATE.md` Experiment 3 + waiver W-1, in `01-CONTEXT.md` as a second
amendment to D-18, and in `WINDOWS.md`. What is retained: 24-bit depth end to end
through the reference, big-endian sense end to end through the reference at 16
bits, and 24-bit **big-endian storage** by hand-computed byte vectors — evidence
that involves no permissive reader at all and is therefore stronger than the round
trip it replaces. What is lost is exactly one thing: the *combination*, verified
end to end.

**Three fixtures, not one.** Experiment 2 forced the structure-only split
(`decoder_main` aborts on an unreferenced Codec Config); Experiment 3 forced the
endianness split. The coupling that made "one fixture, seven clauses" valuable is
genuinely weakened. What preserves the gate's meaning is that the sample-identity
half — the half `libiamf`'s acceptance defines — stays whole and un-permuted.

**The generator lives in `tests/support/fixture.rs`, not `tests/fixture.rs`.**
A top-level `tests/*.rs` is its own test binary, so a `#[test]` there would be
collected into all three including binaries and run three times. This follows the
`tests/support/test_000003.rs` precedent from 01-07. `tests/fixture.rs` is the
property-test target, with 23 assertions.

**`sha2` added as a dev-dependency**, which the plan explicitly authorised.
`cargo tree -e normal,no-proc-macro` still lists exactly `iamf` and `thiserror`;
`cargo deny` reports `advisories ok, bans ok, licenses ok, sources ok`. It is
there rather than `shasum` because GUARD-09 runs the golden test on Windows MSVC
too, where no such tool exists — a `Command`-based digest would have silently
reduced the four-target matrix to three.

## A note on the dumper's limits

`dump_annotated` renders field lines from the parsed **model**, carrying the
containing OBU's offset rather than each field's own. A per-field offset would
need a second walker over the wire layout — a duplicate parser to keep in step
with `iamf-tools`, which is the failure this crate spends most of its comments
avoiding. The 16-per-row hex block carrying absolute offsets is what locates a
wrong byte, and it is exact.

Its module doc states the caveat in the imperative, and the golden test's module
doc repeats it: **the dump aids review, never correctness.** It is
self-consistent with the writer by construction. Conformance evidence comes from
the two reference oracles and from hand-decoded vectors.

## Known Stubs

None. No hardcoded empty value, placeholder string or unwired data source was
introduced.

## Carried forward to Phase 2

`WINDOWS.md` entries 9 and 10 stay **open**, as the plan directed: the Parameter
Block and Temporal Delimiter paths through `push_temporal_unit` are typed and
compiled but still have no byte-level test. `test_000003` publishes neither, and
none of the three fixtures here carries either — a Parameter Block would need a
governing `ParamDefinition` threaded through the dumper too. Left visible rather
than allowed to disappear.

## Threat Flags

None. This plan adds no network endpoint, no auth path and no schema at a trust
boundary. The three mitigations it was assigned (T-01-46 no knob, T-01-48 the
self-generated golden's caveat, T-01-49 unique scratch paths) are implemented and
asserted, and T-01-48 gained a structural guard beyond the documentation the
threat register asked for.

## Verification

- `cargo test --test reference_manifest` — passes; clause 0 confirms both pins.
- `env -u IAMF_REF_DECODER cargo test --locked` — green, 18 test targets, every
  reference-gated clause printing a skip reason (CONF-10).
- `cargo test --test conformance` with both oracles — 22 passed, 0 failed.
- `cargo test --test golden` — 11 passed; the golden reproduces byte-for-byte and
  the same-process double-encode is byte-identical.
- `cargo test --test fixture` — 23 passed.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo tree -e normal,no-proc-macro` — `iamf` and `thiserror`, nothing else.
- D-21 float-escape census — exactly one, in `src/model/loudness.rs`.
- `cargo deny --all-features check` — `advisories ok, bans ok, licenses ok,
  sources ok`.

## Self-Check: PASSED

All nine created files exist on disk. All four commit hashes
(`67476cb`, `16d0f25`, `d33b6a7`, `f4fcabe`) resolve in `git log`. The SHA-256
quoted throughout matches
`tests/fixtures/golden/phase1_sample_identity.iamf.sha256` verbatim.
