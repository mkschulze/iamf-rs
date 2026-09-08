---
phase: 01-conformant-lpcm-bitstream
plan: 02
subsystem: testing
tags: [libiamf, iamf-tools, cmake, docker, bazel, github-actions, fixtures, conformance, reference-oracle]

requires:
  - phase: 01-01
    provides: "REFERENCES.md's two pinned SHAs, NOTICE's vendored-fixtures heading, CONFORMANCE-GATE.md's empty experiments section, the .gitignore entries for /.reference/ and /.reference-manifest.json, and the lint set every new test file must satisfy"
provides:
  - "tools/build-reference.sh — builds libiamf + iamfdec at f06e919e (v1.1.0), re-runnably, and self-validates by decoding a shipped reference file to PCM sample-identical with its source"
  - ".reference-manifest.json (D-13) — the SHA actually checked out, sha256 of each binary, host triple, CMake version, dep_codecs state"
  - "tests/reference_manifest.rs — the runtime drift assertion that runs before any conformance clause, and skips cleanly offline (CONF-10)"
  - "tests/fixtures/reference/ — 39 .iamf plus configurations and rendered WAVs, 1.17 MiB, curated under a 65 536-byte per-file cap"
  - "tests/fixtures/MANIFEST.md (D-14) — what was vendored, what was skipped with sizes and reasons, and the pinned command to fetch anything else"
  - "tests/fixtures_cap.rs — enforces the cap, the .iamf count, and that the invalid fixture stays flagged invalid"
  - "tools/iamf-tools.Dockerfile — base pinned by @sha256, tree by SHA, Bazelisk by version AND binary sha256, Bazel by the tree's own .bazelversion"
  - ".github/workflows/reference.yml — the single Linux reference job, separate from ci.yml so CONF-10 holds"
  - "CONFORMANCE-GATE.md Experiment A — executed proof that a reference tool fails and returns 0, that libiamf ignores reserved-bit misuse byte-identically, and that a one-bit descriptor change decodes cleanly to the wrong length"
  - "tools/experiments/ — the corruption generator and the prepared two-Codec-Config textproto, so the two blocked experiments are one command away"
affects:
  - "01-03..01-07 — every bitstream plan measures itself against iamfdec built by this script and against the vendored corpus"
  - "01-06 — uses tests/fixtures/reference/negative/tones_256samp_5p1_pcm.iamf as 5.1 BCG packing evidence (structure only)"
  - "01-08 — inherits the harness contract: assert file existence, frame count and sample count, never the exit code; and owns finishing Experiments 1 and 2"
  - "Phase 2 PARSE-04 — tools/experiments and MANIFEST.md name test_000134 as the only fixed-size-leb128 material"
  - "Phase 3 — the manifest's dep_codecs_disabled field makes a codec-enabled iamfdec a visible change rather than a mystery hash"

actuals:
  tokens: 25120
  tasks: 3
  commits: 3

plan_head_before: 1ed736fb865b1c98856d4430e6ebadff32b4dd01

tech-stack:
  added:
    - "No crates. The shipping graph is still iamf + thiserror; both new test files parse text by hand rather than pull in a JSON or TOML dependency."
    - "CMake 4.4.3 + AppleClang 16 (host tooling only, never a build dependency of the crate)"
    - "Bazelisk v1.29.0 and Bazel 7.4.1 — inside the container only, pinned, never on a developer's PATH"
  patterns:
    - "A reference tool's exit code is never the signal. Assert on observables the tool cannot fake: the output file exists and exceeds a bare header, the frame count, the decoded-sample count."
    - "Pins have exactly one owner. Bazel is pinned by iamf-tools' own .bazelversion, not restated in the Dockerfile; the cap is stated in MANIFEST.md and read by the test, not duplicated in Rust."
    - "An unrun experiment is recorded as NOT RUN with its exact commands and its reason, never omitted and never written as though it ran."
    - "Test helpers outside #[test] functions are total (Option/Result). GUARD-04's unwrap/expect carve-out only reaches inside test bodies."

key-files:
  created:
    - tools/build-reference.sh
    - tools/iamf-tools.Dockerfile
    - tools/experiments/corrupt-fixture.py
    - tools/experiments/two-codec-configs.textproto
    - tests/reference_manifest.rs
    - tests/fixtures_cap.rs
    - tests/fixtures/MANIFEST.md
    - tests/fixtures/reference/
    - .github/workflows/reference.yml
  modified:
    - REFERENCES.md
    - CONFORMANCE-GATE.md
    - NOTICE
    - .planning/WINDOWS.md

key-decisions:
  - "The per-file fixture size cap is 65 536 bytes. It admits test_000003.iamf (32 567) with room and excludes the 154 over-cap files, keeping 594 MB out of every Parallax clone."
  - "Vendor a curated subset (77 files, 1.17 MiB), not all 221. Research open question 2 resolved: the deciding constraint is the path dependency, and the asymmetry is that adding a file later is a commit while removing one is a history rewrite."
  - "Bazel is NOT pinned in the Dockerfile. iamf-tools@v2.1.0 commits .bazelversion = 7.4.1 and Bazelisk honours it; a second copy of that pin would drift. This resolves half of research assumption A2 from code rather than by assumption."
  - "Bazelisk is pinned by release v1.29.0 AND by the sha256 of the binary for both linux arches, which is strictly stronger than the versioned URL the plan asked for."
  - "The Dockerfile handles TARGETARCH for amd64 and arm64 rather than requiring --platform=linux/amd64, so an Apple Silicon developer can build it without emulation. CI still builds amd64, which is the digest that gets recorded."
  - "Two new offline test files were added beyond the plan's file list — tests/fixtures_cap.rs (enforces D-14's cap and count) and the corruption/textproto inputs under tools/experiments/. Both make an otherwise-prose guarantee mechanical."
  - "CONFORMANCE-GATE.md's pre-existing claim that 'a non-zero exit or an error on stderr is the CONF-06 signal' was CORRECTED to an open question, because Experiment A refutes the general form of it by execution."

patterns-established:
  - "The tracer proves the far end first: the build script decodes a file this project did not write and asserts sample-identity before any of our own bytes exist, so the oracle and the comparison logic are both known-good before they are ever pointed at our output."
  - "Every load-bearing flag carries its reason in a comment at the call site (-r 16000 or the resampler runs silently; -disable_limiter because the limiter is created unconditionally), so a later maintainer cannot remove one without reading why it is there."
  - "Fixture provenance is directory structure: libiamf copies at the top level, iamf-tools copies under iamf-tools/, invalid files under negative/. The split also keeps the two incompatible proto dialects apart."
  - "The manifest is the single source of the cap and the count; the test reads both out of the Markdown rather than restating them, so the document cannot go stale relative to the check."

requirements-completed:
  - CONF-09
  - CONF-11
  - GUARD-06
  - DEC-04
  - DEC-05

coverage:
  - id: D1
    description: "libiamf and iamfdec build at the pinned SHA from tools/build-reference.sh, re-runnably, with the codec archives moved aside so the LPCM-only configuration links"
    requirement: CONF-09
    verification:
      - kind: integration
        ref: "bash tools/build-reference.sh — configure log carries all three 'the <codec> library was not found' lines; iamfdec and libiamf.a produced"
        status: pass
      - kind: integration
        ref: "two consecutive runs -> iamfdec_sha256 24a80f0c98656650e6abbb9afbdb2db858856368ff10e00388976b37a39a96e6 both times"
        status: pass
    human_judgment: false
  - id: D2
    description: "The oracle and the harness's comparison logic are proven on a file this project did not write — 8000 frames, 0 differing samples of 16000, asserted separately from the exit code"
    requirement: CONF-09
    verification:
      - kind: integration
        ref: "build-reference.sh smoke decode: test_000003.iamf -> WAV vs sawtooth_100_stereo.wav; three assertions (file > 44 bytes, frames == 8000, diff == 0)"
        status: pass
    human_judgment: false
  - id: D3
    description: "D-13's runtime drift assertion: the manifest is checked against REFERENCES.md before any conformance clause, and skips with a printed reason offline"
    requirement: GUARD-06
    verification:
      - kind: integration
        ref: "cargo test --test reference_manifest with IAMF_REF_DECODER set -> 4 passed"
        status: pass
      - kind: integration
        ref: "cargo test --test reference_manifest with IAMF_REF_DECODER unset -> 4 passed, skip reason printed (CONF-10)"
        status: pass
      - kind: integration
        ref: "doctored manifest -> fails with 'manifest disagrees with REFERENCES.md for `libiamf`' naming both values (proven to bite)"
        status: pass
    human_judgment: false
  - id: D4
    description: "A curated, licence-attributed reference corpus with a D-14 manifest: 39 .iamf under a stated 65 536-byte cap, skipped candidates named with sizes and reasons, the invalid fixture flagged negative-only"
    requirement: CONF-11
    verification:
      - kind: integration
        ref: "cargo test --test fixtures_cap -> 3 passed (cap enforced, 39 on disk == 39 claimed, negative fixture documented with its reason)"
        status: pass
      - kind: integration
        ref: "find tests/fixtures/reference -name '*.iamf' | wc -l -> 39"
        status: pass
    human_judgment: false
  - id: D5
    description: "The pinned reference identities and their evidence are recorded: both SHAs, the container base digest, the Bazelisk version and binary hashes, and the Bazel version read from the tree's own .bazelversion"
    requirement: DEC-04
    verification:
      - kind: integration
        ref: "REFERENCES.md container table; registry-1.docker.io returned HTTP 200 echoing sha256:33ceb719... for ubuntu:24.04"
        status: pass
      - kind: integration
        ref: "grep -c '848c6ff4968ff8cc6f728259892ab4f90cb83256' tools/iamf-tools.Dockerfile -> 1; grep -c '@sha256:' -> 1"
        status: pass
    human_judgment: false
  - id: D6
    description: "Executed evidence for how a reference decoder signals failure, and for the two silent-failure classes the gate must catch"
    requirement: DEC-05
    verification:
      - kind: integration
        ref: "CONFORMANCE-GATE.md Experiment A — 5 inputs through iamfdec, all exit 0; reserved-bit flip byte-identical (sha ff8c67981380); sample_rate flip 1570 samples not 8000"
        status: pass
    human_judgment: false
  - id: D7
    description: "The digest-pinned iamf-tools container and the single Linux reference CI job"
    requirement: CONF-09
    verification:
      - kind: integration
        ref: ".github/workflows/reference.yml parses; one job, ubuntu-latest only, 11 steps, exports IAMF_REF_DECODER; ci.yml unchanged"
        status: pass
      - kind: integration
        ref: "docker build -f tools/iamf-tools.Dockerfile"
        status: unknown
    human_judgment: true
    rationale: "The image has never been built. The Docker daemon was not running on the authoring machine and could not be started from this session, and bazel/bazelisk are absent by design. The Dockerfile is structurally complete and every pin in it was verified independently (base digest against the registry, Bazelisk hashes against the release assets, .bazelversion read from the pinned tree), but assumption A2 — that the apt package set suffices — is untested, and the BUILT image's own digest is therefore not recorded in REFERENCES.md as D-13 requires. Recorded in .planning/WINDOWS.md."
  - id: D8
    description: "Research open question 1 — how decoder_main signals a parse failure"
    verification:
      - kind: integration
        ref: "CONFORMANCE-GATE.md Experiment 1 — commands prepared, inputs committed"
        status: unknown
    human_judgment: true
    rationale: "NOT RUN. Requires the container. Until it runs, CONF-06 has a route (decoder_main's parse path, because v2.1.0 ships no probe_main) but no asserted observable — the CI step asserts only a non-empty output WAV, which is deliberately the weakest defensible claim. Recorded in .planning/WINDOWS.md."
  - id: D9
    description: "Research assumption A3 / D-18's research item — does encoder_main accept a Codec Config no Audio Element references"
    verification:
      - kind: integration
        ref: "CONFORMANCE-GATE.md Experiment 2 — tools/experiments/two-codec-configs.textproto prepared and committed"
        status: unknown
    human_judgment: true
    rationale: "NOT RUN. Requires the container. D-18's two-Codec-Config fixture design is confirmed on the parser side from source and unconfirmed on the encoder side; both fallbacks are recorded with their consequences. Recorded in .planning/WINDOWS.md."

duration: 35min
completed: 2026-09-08
status: complete
---

# Phase 1 Plan 02: Reference Oracles and Fixture Supply Summary

**`iamfdec` built at its pinned SHA and proven on a file this project did not write — 8000 frames, 0 differing samples of 16000 — plus a 39-file vendored corpus under an enforced cap, a runtime drift assertion, a digest-pinned `iamf-tools` image, and executed proof that a reference decoder fails while returning 0.**

## Performance

- **Duration:** 35 min
- **Started:** 2026-09-08T02:11:46Z
- **Tasks:** 3 of 3
- **Commits:** 3 (measured: `git rev-list --count 1ed736f..HEAD`)
- **Files changed:** 89 (12 authored, 77 vendored verbatim)

## Accomplishments

- **The far end of the walking skeleton is proven before any of our own bytes exist.** `tools/build-reference.sh` clones `libiamf` at `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63`, builds `libiamf.a` and `iamfdec`, then decodes the shipped `test_000003.iamf` and compares it against the shipped `sawtooth_100_stereo.wav`: **8000 frames, 0 differing samples of 16000**. That single run validates the binary *and* the comparison logic on material this crate did not author, which is exactly what plans 01-03 through 01-08 will be measured with. It is re-runnable — two consecutive invocations produce the identical `iamfdec_sha256`.

- **The exit code is not the signal, and now that is a demonstrated fact in this repository rather than a warning in a research document.** Experiment A ran five inputs through `iamfdec`: the control, a reserved-bit flip, a `sample_rate` bit flip, an oversized `obu_size`, and a mid-OBU truncation. **All five returned exit 0**, including the two that wrote nothing but a 44-byte WAV header. Every harness in this phase asserts on file existence, frame count and decoded-sample count instead, and `CONFORMANCE-GATE.md`'s previous claim that "a non-zero exit or an error on stderr is the CONF-06 signal" has been corrected to an open question, because it was an inference and the sibling tool refutes its general form.

- **"`libiamf` ignores reserved-bit misuse" was reproduced, not cited.** Flipping bit 0 of byte 30 — a reserved bit in the Audio Element's type octet — produced a **byte-identical** output WAV: same size, same sha256, same frame and sample counts. This is the concrete form of the project's own warning that passing the `libiamf` gate is necessary and nowhere near sufficient, and it is why CONF-07's byte diff against `iamf-tools` output is the clause that actually catches a reserved-bit bug.

- **A second silent-failure class was found and named.** Flipping one bit inside the Codec Config's `sample_rate` (16000 → 81536) produced a clean decode of the **wrong length** — 1570 samples instead of 8000, exit 0, no error. Together with `tones_256samp_5p1_pcm.iamf` decoding to zero samples, this is why CONF-05's decoded-sample-count clause is not redundant with PCM equality: a wrong-length decode is never compared sample-for-sample unless the count is checked first.

- **Fixture supply is settled without Bazel, and the scope change is written down where it bites.** CONF-11 was planned around a one-time Bazel build to generate `.iamf` files. At the pinned tags that premise is false: `libiamf@v1.1.0` commits **221 `.iamf` files, 221 matching textprotos and 403 rendered WAVs** under BSD-3-Clause-Clear. Fixture supply is a `git show` from a pinned SHA. 77 files (39 `.iamf`, 1.17 MiB) are vendored under a **65 536-byte per-file cap** that a test enforces, and `tests/fixtures/MANIFEST.md` records every vendored file, named skipped candidates with sizes, and the exact pinned command to fetch any of the rest.

- **Drift is a named error, not a green gate.** `.reference-manifest.json` records the SHA actually checked out, the sha256 of both binaries, the host triple, the CMake version and the `dep_codecs` state; `tests/reference_manifest.rs` asserts it against `REFERENCES.md` before any conformance clause and was **proven to bite** — a doctored manifest fails with a message naming both values. With `IAMF_REF_DECODER` unset it prints a reason and returns, so `cargo test` stays green offline on all four targets (CONF-10).

## Task Commits

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 (tracer) | Build the reference decoder at its pinned SHA and prove the oracle | `3987871` | `tools/build-reference.sh`, `tests/reference_manifest.rs` |
| 2 | Vendor a curated reference fixture corpus with a D-14 manifest | `41217c5` | `tests/fixtures/**`, `tests/fixtures_cap.rs`, `NOTICE` |
| 3 | Digest-pinned container, Linux reference job, recorded experiments | `79a354b` | `tools/iamf-tools.Dockerfile`, `.github/workflows/reference.yml`, `CONFORMANCE-GATE.md`, `REFERENCES.md`, `tools/experiments/**` |

**Tracer feedback gate:** auto mode active and the task carried no `gate="blocking-human"`, so the `<verify>` block was re-run end to end before any expansion task. All four clauses passed — smoke summary, manifest presence, `cargo test --test reference_manifest`, and the two-run `iamfdec_sha256` comparison.

## What was NOT verified, and why

Three items could not execute on the authoring machine and are recorded as unverified in `CONFORMANCE-GATE.md` and appended to `.planning/WINDOWS.md`. None is claimed as passing anywhere.

| Item | Reason |
|---|---|
| The `iamf-tools` image has never been **built or run** | The Docker CLI is present at `/usr/local/bin/docker` but the daemon is not running (`Cannot connect to the Docker daemon at unix:///Users/cell/.docker/run/docker.sock`) and Docker Desktop could not be started from this session. `bazel`/`bazelisk` are absent by design (D-11). Research assumption A2 — that the apt package set suffices — is therefore still untested. |
| The **built image's own digest** is not in `REFERENCES.md` | It cannot be, because no image exists. `REFERENCES.md` says so explicitly and states the consequence: until that row is filled, the `iamf-tools` half of the pin is a source-and-base pin, not a result pin, because `apt-get` and Bazel's dependency fetch both reach the network. |
| **Experiments 1 and 2** did not run | Both need `decoder_main` / `encoder_main`, which exist only inside that image. Both are recorded as **NOT RUN** with their exact commands, and their inputs are prepared and committed (`tools/experiments/corrupt-fixture.py`, `tools/experiments/two-codec-configs.textproto`) so each is a single command once a daemon exists. `.github/workflows/reference.yml` builds the image on `ubuntu-latest`, so its first run is where all three resolve. |

Everything on the plan's critical path proceeded without Docker: the `libiamf` build, the self-validating smoke decode, the fixture corpus, the manifest assertion, and the CI job that will run the container.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical functionality] The D-14 cap and count were prose; added `tests/fixtures_cap.rs` to enforce them**

- **Found during:** Task 2.
- **Issue:** The plan verifies the cap with an ad-hoc `python3` heredoc at execution time. That checks the state of the tree once and then evaporates — the next person to add a 6 MB fixture would not be stopped by anything, and D-14's whole point is that fixtures are irreversible in every Parallax clone.
- **Fix:** Added `tests/fixtures_cap.rs`, which reads the cap and the claimed `.iamf` count **out of `MANIFEST.md`** (so the document is the single source and cannot go stale), walks `tests/fixtures/reference/`, and fails on any over-cap file, on a count mismatch, or on a vendored file the manifest does not name. A third test asserts the invalid fixture stays in `negative/` and keeps its documented reason. Runs offline on all four targets.
- **Files:** `tests/fixtures_cap.rs`, and the `MANIFEST.md` / `NOTICE` sentences that point at it.
- **Committed in:** `41217c5`.

**2. [Rule 3 — Blocking] `test_000134.iamf` does not exist in either reference tree; only its textproto does**

- **Found during:** Task 2. The plan asks to vendor `test_000134.iamf` "because it is the only reference file generated with `GENERATE_LEB_FIXED_SIZE`".
- **Issue:** No `test_000134.iamf` is committed anywhere. `libiamf@v1.1.0 tests/` has no `test_000134.*` at all; the configuration lives only at `iamf-tools@v2.1.0 iamf/cli/testdata/test_000134.textproto`, and it is confirmed to be the sole file of 226 setting `GENERATE_LEB_FIXED_SIZE`.
- **Fix:** Vendored the **textproto**, and recorded in `MANIFEST.md` that the `.iamf` bytes do not exist in either tree and must be produced by `encoder_main` in the pinned container, with the command. The Phase 2 PARSE-04 naming the plan wanted is preserved and is now accurate rather than pointing at a file nobody could find.
- **Committed in:** `41217c5`.

**3. [Rule 3 — Blocking] No LPCM 5.1 or multi-element file exists under the cap in `libiamf@v1.1.0`**

- **Found during:** Task 2. The plan asks for "at least one with more than one Audio Element, at least one with a non-stereo `loudspeaker_layout`".
- **Issue:** Every under-cap LPCM file in `libiamf@v1.1.0 tests/` is single-element stereo (or mono, once). The only under-cap multi-element file is `test_000124` and the only under-cap non-stereo files are `test_000059` / `test_000061` — all three are Opus.
- **Fix:** Vendored `test_000124` (two Audio Elements) and `test_000059` (5.1 plus stereo layouts) anyway. Phase 1 reads descriptors, not audio frames, so the codec of the frame payloads is irrelevant to the structures these files exercise; `MANIFEST.md` says so per file. The genuine LPCM 5.1 structural evidence is `tones_256samp_5p1_pcm.iamf`, which is vendored as a negative fixture and which research already designates as plan 01-06's BCG-packing evidence.
- **Committed in:** `41217c5`.

**4. [Rule 2 — Missing critical functionality] Bazelisk pinned by binary sha256 as well as by version**

- **Found during:** Task 3.
- **Issue:** The plan asks for "a **versioned** release URL, not `latest`". A versioned GitHub release URL still resolves to whatever bytes that release currently serves.
- **Fix:** Pinned `v1.29.0` **and** verified the download against the sha256 published in the release's own `.sha256` assets, for both `linux-amd64` and `linux-arm64`, with `sha256sum -c -` in the build. Both hashes are recorded in `REFERENCES.md`.
- **Committed in:** `79a354b`.

**5. [Rule 3 — Blocking] `.reference/` was not a git worktree hazard, but the first draft of the build script called `git clean` inside the pinned checkout; removed**

- **Found during:** Task 1, self-review before the first run.
- **Issue:** A `git -C .reference/libiamf clean -qfd` would have wiped the CMake outputs on every re-run, defeating the re-runnability requirement, and `git clean` is categorically prohibited by the execution protocol.
- **Fix:** Removed entirely. The checkout is made idempotent instead — the SHA is compared before fetching or checking out, and the `dep_codecs` move loop skips files that are already moved. Two consecutive runs produce an identical `iamfdec_sha256`, which is the property that actually mattered.
- **Committed in:** `3987871`.

**6. [Rule 3 — Blocking] Clippy rejected `expect()` in non-`#[test]` helper functions**

- **Found during:** Tasks 1 and 2.
- **Issue:** GUARD-04's carve-out (`allow-expect-in-tests`) reaches inside `#[test]` bodies only. Helper functions in an integration-test file are ordinary functions and `expect_used = "deny"` applies to them.
- **Fix:** Every helper in both new test files returns `Option`/`Result`; the tests do the asserting. That is the better shape anyway — a helper that panics reports the failure at the wrong place. A comment in `fixtures_cap.rs` records the rule so the next test author does not rediscover it.
- **Committed in:** `3987871`, `41217c5`.

### Scope additions beyond the plan's file list

Two files the plan did not name, both because a blocked experiment recorded as prose is a blocked experiment nobody will ever finish:

- `tools/experiments/corrupt-fixture.py` — generates the control plus four corruptions deterministically, so Experiment A is reproducible and Experiment 1 is one command away in CI.
- `tools/experiments/two-codec-configs.textproto` — the Experiment 2 input, built from the **current**-dialect `iamf-tools` copy, with the second `codec_config_metadata` block differing only by `codec_config_id` and a comment block recording exactly why every other field is held identical.

---

**Total deviations:** 6 auto-fixed (2 Rule 2, 4 Rule 3), plus 2 scope additions. No architectural decision was taken and no crate was added — the shipping graph is still `iamf` + `thiserror`.

## Issues Encountered

**The Docker daemon.** Covered in full above. It cost the two experiments the plan asked for and the built-image digest `REFERENCES.md` wants; it cost nothing on the critical path, because research correction 6 had already removed the container from the fixture-supply route.

**`libiamf@v1.1.0`'s `dep_codecs/lib` is the real build obstacle, exactly as research recorded.** The shipped `.a` files are x86_64-Linux; `find_library(... NO_DEFAULT_PATH)` finds them on macOS, the codec sources get compiled in, and the `iamfdec` link then fails with hundreds of `symbol(s) not found for architecture x86_64`. Moving six archives aside makes the probe fail cleanly. The three `the <codec> library was not found` configure lines are the confirmation that the LPCM-only configuration took, and the script prints them deliberately.

**Formatting was deliberately not normalised.** `cargo fmt --check` reports differences in both new test files and in `tests/error_shape.rs` from plan 01-01. `cargo fmt` is not a CI gate in this project and the existing committed code is not rustfmt-clean, so running it would have produced an out-of-scope diff across a file this plan does not own. Left alone; noted here rather than silently fixed.

## Known Stubs

None in the sense of placeholder code. `grep -rn 'TODO\|FIXME\|unimplemented\|placeholder' src/ tests/*.rs tools/` returns nothing.

Three **unverified claims** exist and are enumerated in "What was NOT verified" above and in `.planning/WINDOWS.md` (entries 2, 3 and 4): the container was never built, the built image digest is absent from `REFERENCES.md`, and Experiments 1 and 2 did not run. Each is written into the artifact where it bites, so a reader of `REFERENCES.md` or `CONFORMANCE-GATE.md` cannot mistake the gap for a completed check.

## Threat Flags

None. The plan's `<threat_model>` register is addressed as written: T-01-07 by checkout-then-verify-by-SHA plus the runtime manifest assertion; T-01-08 by the `@sha256` base and the Bazelisk hash pins (with the built-image digest gap stated, not glossed); T-01-09 by the enforced cap and the curated subset; T-01-10 by Experiment A, which is the direct empirical answer to "a reference tool that fails and returns 0"; T-01-11 by the manifest's binary hashes, host triple and `dep_codecs_disabled` field. T-01-SC holds: no crates were added.

## User Setup Required

To run the reference-gated tests locally:

```sh
bash tools/build-reference.sh
export IAMF_REF_DECODER=$(python3 -c "import json;print(json.load(open('.reference-manifest.json'))['iamfdec_path'])")
cargo test --locked
```

To close the three unverified items, start Docker Desktop (or push the branch and let `.github/workflows/reference.yml` run), then:

```sh
docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/
docker image inspect iamf-tools:v2.1.0 --format '{{.Id}}'   # -> REFERENCES.md
```

and run the two experiment blocks recorded verbatim in `CONFORMANCE-GATE.md`.

The branch question from plan 01-01 is still open: work continues on `gsd/phase-01-conformant-lpcm-bitstream` because `main` is protected and `git.allow_default_branch_commits` is unset.

## Next Phase Readiness

Ready for plan **01-03** (bit primitives) and everything after it:

- `IAMF_REF_DECODER` points at a `iamfdec` whose provenance is asserted at test time, and the harness contract is established and demonstrated: **assert file existence, frame count and decoded-sample count, never the exit code.**
- `tests/fixtures/reference/` gives every later plan real reference bytes to walk with no reference binary present, satisfying CONF-10 on all four targets. `test_000003.iamf` is there with its published configuration, its source PCM and the reference's own rendered output — D-25's two independent derivations, both on disk.
- The two silent-failure classes Phase 1 must defend against are documented with executed evidence: reserved-bit misuse that changes nothing, and a one-bit descriptor change that decodes cleanly to the wrong length.

**Carried concerns:**

1. **CONF-06 has a route but no observable** until Experiment 1 runs. Plan 01-08 should not write a `<fails_when>` for `decoder_main` before that.
2. **D-18's two-Codec-Config fixture is unconfirmed on the encoder side.** Both fallbacks are recorded with their consequences — notably that fallback (a) would stop DESC-08's ascending-ID property being what is observed, which is worse than failing.
3. **The `iamf-tools` pin is a source-and-base pin, not a result pin,** until the built image's digest lands in `REFERENCES.md`.

---
*Phase: 01-conformant-lpcm-bitstream*
*Completed: 2026-09-08*

## Self-Check: PASSED

All 11 claimed artifacts exist on disk. All 3 claimed commits (`3987871`, `41217c5`, `79a354b`) exist in git. Frontmatter parses as valid YAML: `status: complete`, `actuals.commits: 3` measured from `plan_head_before: 1ed736fb865b1c98856d4430e6ebadff32b4dd01` (`git rev-list --count` = 3, not narrated), 5 requirement IDs, 9 coverage entries of which 3 carry `status: unknown` with `human_judgment: true` and a stated reason — the container was never built, and nothing in this summary claims otherwise.
