# External Integrations

**Analysis Date:** 2026-09-13

## APIs & External Services

The crate makes no network calls and uses no SaaS APIs. Its external integrations are the reference IAMF implementations, which serve as test oracles, and GitHub Actions.

**Reference decoder, `libiamf` (native build):**
- `AOMediaCodec/libiamf` at tag `v1.1.0`, SHA `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` (pinned in `REFERENCES.md` and `tools/build-reference.sh`)
  - Built by: `bash tools/build-reference.sh`. It clones to `.reference/libiamf`, builds `iamfdec`, runs a smoke decode (8000 frames, 0 differing samples) and writes `.reference-manifest.json`.
  - Client: `std::process::Command` in tests (`tests/conformance.rs`)
  - Discovery: the `IAMF_REF_DECODER` env var holds the path to `iamfdec`
  - Pin enforcement: `tests/reference_manifest.rs` checks `.reference-manifest.json` (both SHAs plus the sha256 of `libiamf.a` and `iamfdec`) against `REFERENCES.md`

**Reference encoder/decoder, `iamf-tools` (container):**
- `AOMediaCodec/iamf-tools` at tag `v2.1.0`, SHA `848c6ff4968ff8cc6f728259892ab4f90cb83256`
  - Built by: `docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/`. The base `ubuntu:24.04` is pinned by digest, Bazelisk v1.29.0 by binary sha256, and Bazel 7.4.1 by the tree's `.bazelversion`.
  - Binaries used: `bazel-bin/iamf/cli/decoder_main` (CONF-06 parse acceptance, which asserts `Decoded <N> temporal units.` on stderr) and `encoder_main` (CONF-07 companion file for the byte-diff against `DIFF-LEDGER.md`). There is no `probe_main` at v2.1.0.
  - Client: `docker run` via `Command` in `tests/conformance.rs`, which first checks for the image with `docker image inspect`
  - Discovery: the `IAMF_TOOLS_IMAGE` env var holds the image tag

**Vendored reference data:**
- `tests/fixtures/reference/`: `.iamf` files committed from the upstream projects (BSD-3-Clause-Clear, provenance in `tests/fixtures/MANIFEST.md`). Parsed offline by `tests/parse_reference.rs` and `tests/refcorpus.rs`.
- Local upstream clones under `docs/` (untracked) are for reading only. No test or build step uses them.

**Downstream consumer:**
- Parallax (a deterministic spatial-audio DAW) uses the crate as a path dependency. The contract is exercised by `tests/parallax_contract.rs` and `tests/support/parallax_contract.rs`.

## Data Storage

**Databases:**
- Not applicable

**File Storage:**
- Local filesystem only
  - Golden fixtures: `tests/fixtures/golden/`. Regenerate them with `IAMF_REGENERATE_GOLDEN`.
  - Codec fixtures: `tests/fixtures/codecs/flac`, `tests/fixtures/codecs/opus`
  - Fuzz corpus and minimized artifacts: `fuzz/corpus/`, `fuzz/artifacts/` (committed, see `fuzz/CORPUS.md`)
  - Reference build output: `.reference/` and `.reference-manifest.json` (gitignored)
  - Conformance output: `target/conformance/`, `target/reference-out/`

**Caching:**
- None

## Authentication & Identity

**Auth Provider:**
- Not applicable. No secrets are needed; reference builds clone public GitHub repositories anonymously.

## Monitoring & Observability

**Error Tracking:**
- None. Errors are typed values (`src/error.rs`, `thiserror`).

**Logs:**
- The CI workflows upload artifacts. `fuzz-output` (fuzz failures and logs, kept 14 days) and `reference-output` (manifest, `.reference/configure.log`, `.reference/smoke.log`, conformance output, kept 7 days).
- `tools/cargo-test-tap.sh` and `tools/red-evidence.sh` turn `cargo test` output into TAP and JSON records for GSD's TDD red-evidence gate.

## CI/CD & Deployment

**Hosting:**
- GitHub repository `https://github.com/mkschulze/iamf-rs`. Nothing is published to crates.io (`publish = false`).

**CI Pipeline (GitHub Actions; actions pinned by commit SHA):**
- `.github/workflows/ci.yml`, on PRs and pushes to main
  - Job `matrix` covers the four targets (`macos-latest` for aarch64, `macos-latest` running x86_64 under Rosetta via `CARGO_TARGET_X86_64_APPLE_DARWIN_RUNNER=arch -x86_64`, `windows-latest` with `core.autocrlf false`, and `ubuntu-latest`). Steps: toolchain pin assertion, build, clippy, test, codec framing regressions, fuzz corpus replay (`--features fuzzing --test fuzz_regression`), golden fixture (`--test golden --test fixture`), and a check that the linked graph is exactly `iamf` and `thiserror`.
  - Job `guardrails` runs on Linux. Steps: `cargo-deny@0.20.2` (installed with `taiki-e/install-action`) for the root, `fuzz/` and `tools/codec-fixtures/`; `tools/check-codec-dependency-boundary.sh`; compiling the codec fixture generator (`--no-run`); verifying the committed codec corpora (`CODEC_FIXTURE_INPUT=... -- --ignored`); a check that `libfuzzer-sys` is absent from the root lock and graph; and `tools/prove-guards.sh`.
- `.github/workflows/fuzz.yml`, nightly cron `41 3 * * *` plus manual dispatch. It runs `parse_sequence` and `obu_roundtrip` (the latter with `--features roundtrip-model`) for 300 s each on `nightly-2026-09-01`.
- `.github/workflows/reference.yml`, on PRs, cron `17 4 * * *` and manual dispatch, with a 90-minute timeout. It builds `libiamf`, exports `IAMF_REF_DECODER` from the manifest, runs `--test reference_manifest`, runs `cargo test -- --include-ignored`, builds the `iamf-tools` image, runs `--test conformance -- --nocapture --test-threads=1`, and runs a separate shell CONF-06 check on `tests/fixtures/reference/test_000003.iamf` (expects 63 temporal units).

## Environment Configuration

**Env vars (all optional, used by tests and tools only):**
- `IAMF_REF_DECODER`: path to the `iamfdec` binary. When unset or empty, the reference-gated tests skip, so the offline four-target matrix stays green.
- `IAMF_TOOLS_IMAGE`: Docker image tag for `iamf-tools` (`tests/conformance.rs`)
- `IAMF_REGENERATE_GOLDEN`: rewrites the golden fixtures instead of comparing against them
- `CODEC_BOUNDARY_ROOT`: root override for the codec dependency boundary check
- `CODEC_FIXTURE_INPUT`: directory of committed corpora that the `tools/codec-fixtures` verifiers read
- `CARGO_TARGET_X86_64_APPLE_DARWIN_RUNNER`: set in CI for Rosetta execution

**Secrets location:**
- None required. No `.env` files are present.

## Webhooks & Callbacks

**Incoming:**
- None. Only GitHub Actions triggers (`pull_request`, `push`, `schedule`, `workflow_dispatch`).

**Outgoing:**
- None

---

*Integration audit: 2026-09-13*
