#!/usr/bin/env bash
# Isolated CODEC-06 preflight. This deliberately never invokes Cargo in the
# repository: codec crates are only candidates until a later task changes the
# dev-dependency graph.
set -euo pipefail

REPO_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_BASE="${TMPDIR:-/tmp}"
if [[ ! -d "${TMP_BASE}" ]]; then
    printf 'temporary base is not a directory: %s\n' "${TMP_BASE}" >&2
    exit 1
fi
TMP_BASE="$(cd -P "${TMP_BASE}" && pwd)"

case "${TMP_BASE}" in
    "${REPO_ROOT}"|"${REPO_ROOT}"/*)
        printf 'refusing temporary base inside repository: %s\n' "${TMP_BASE}" >&2
        exit 1
        ;;
esac

PREFLIGHT_DIR="$(mktemp -d "${TMP_BASE}/iamf-codec-preflight.XXXXXX")"

if [[ ! -d "${PREFLIGHT_DIR}" || -L "${PREFLIGHT_DIR}" ]]; then
    printf 'mktemp did not create a safe directory: %s\n' "${PREFLIGHT_DIR}" >&2
    exit 1
fi

PREFLIGHT_DIR="$(cd -P "${PREFLIGHT_DIR}" && pwd)"

case "${PREFLIGHT_DIR}" in
    "${TMP_BASE}"/iamf-codec-preflight.*) ;;
    *)
        printf 'refusing unexpected temporary path: %s\n' "${PREFLIGHT_DIR}" >&2
        exit 1
        ;;
esac

if [[ ! -d "${PREFLIGHT_DIR}" || -L "${PREFLIGHT_DIR}" ]]; then
    printf 'mktemp did not create a safe directory: %s\n' "${PREFLIGHT_DIR}" >&2
    exit 1
fi

case "${PREFLIGHT_DIR}" in
    "${REPO_ROOT}"|"${REPO_ROOT}"/*)
        printf 'refusing temporary directory inside repository: %s\n' "${PREFLIGHT_DIR}" >&2
        exit 1
        ;;
esac

cleanup() {
    rm -rf -- "${PREFLIGHT_DIR}"
}
trap cleanup EXIT

mkdir -p "${PREFLIGHT_DIR}/src"
cp "${REPO_ROOT}/deny.toml" "${PREFLIGHT_DIR}/deny.toml"

cat >"${PREFLIGHT_DIR}/Cargo.toml" <<'EOF'
[package]
name = "iamf-codec-preflight"
version = "0.0.0"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
publish = false

[dependencies]
claxon = "=0.4.3"
flacenc = { version = "=0.5.1", default-features = false }
opus = "=0.4.0"
EOF

cat >"${PREFLIGHT_DIR}/src/lib.rs" <<'EOF'
use claxon::FlacReader;
use flacenc::config::Encoder as FlacEncoder;
use opus::Encoder as OpusEncoder;

#[allow(dead_code)]
fn candidate_imports(
    _: Option<FlacReader<std::io::Cursor<Vec<u8>>>>,
    _: FlacEncoder,
    _: OpusEncoder,
) {
}
EOF

cd "${PREFLIGHT_DIR}"
cargo +1.85.0 check --all-targets
cargo deny check licenses
cargo deny check advisories bans sources
cargo metadata --locked --format-version 1 >metadata.json
cargo tree --edges normal,build >tree-normal-build.txt

metadata_package() {
    local package_name="$1"

    jq -er --arg package_name "${package_name}" '
        [.packages[]
         | select(.name == $package_name)
         | {version, license}]
        | if length != 1 then
              error("expected exactly one resolved package named " + $package_name)
          elif .[0].license == null then
              error("resolved package has no declared license: " + $package_name)
          else
              .[0] | [.version, .license] | @tsv
          end
    ' metadata.json
}

CLAXON_PACKAGE="$(metadata_package claxon)"
FLACENC_PACKAGE="$(metadata_package flacenc)"
OPUS_PACKAGE="$(metadata_package opus)"
OPUSIC_SYS_PACKAGE="$(metadata_package opusic-sys)"

IFS=$'\t' read -r CLAXON_VERSION CLAXON_LICENSE <<<"${CLAXON_PACKAGE}"
IFS=$'\t' read -r FLACENC_VERSION FLACENC_LICENSE <<<"${FLACENC_PACKAGE}"
IFS=$'\t' read -r OPUS_VERSION OPUS_LICENSE <<<"${OPUS_PACKAGE}"
IFS=$'\t' read -r OPUSIC_SYS_VERSION OPUSIC_SYS_LICENSE <<<"${OPUSIC_SYS_PACKAGE}"

TIMESTAMP="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
CARGO_VERSION="$(cargo +1.85.0 --version)"
RUSTC_VERSION="$(rustc +1.85.0 --version)"
DENY_VERSION="$(cargo deny --version)"
JQ_VERSION="$(jq --version)"

cat >"${REPO_ROOT}/CODEC-DEPENDENCY-PREFLIGHT.md" <<EOF
# Codec dependency preflight

**Status:** PASS
**Executed:** ${TIMESTAMP}

This is CODEC-06 evidence recorded before adding any codec crate to this
repository. The preflight creates a disposable Cargo package outside the
repository, copies this repository's \`deny.toml\`, and removes the package on
exit. It does not edit \`Cargo.toml\` or \`Cargo.lock\`.

## Environment

- \`${CARGO_VERSION}\`
- \`${RUSTC_VERSION}\`
- \`${DENY_VERSION}\`
- \`${JQ_VERSION}\` (extracts resolved package fields from locked metadata)
- Temporary package: \`${PREFLIGHT_DIR}\` (removed after this run)

## Candidate resolution and policy result

| Candidate | Resolved version | Licence | Configuration | Result |
| --- | --- | --- | --- | --- |
| \`claxon\` | ${CLAXON_VERSION} | ${CLAXON_LICENSE} | exact pin | PASS |
| \`flacenc\` | ${FLACENC_VERSION} | ${FLACENC_LICENSE} | exact pin; \`default-features = false\` | PASS |
| \`opus\` | ${OPUS_VERSION} | ${OPUS_LICENSE} | exact pin | PASS |
| \`opusic-sys\` (transitive from \`opus\`) | ${OPUSIC_SYS_VERSION} | ${OPUSIC_SYS_LICENSE} | bundled default build | PASS under copied policy |

Versions and licences in this table are extracted from the generated locked
\`metadata.json\`; extraction requires exactly one resolved package with each
candidate name and a declared licence, otherwise the script stops before it
can write PASS evidence.

The \`flacenc\` default feature set (\`log\`, \`par\`, and \`serde\`) is
disabled. Its pure-Rust graph still has a \`build.rs\`; it is a test-tool
candidate only. \`opus\` reaches \`opusic-sys\`, whose default bundled build
uses CMake and compiles bundled C/libopus. That C toolchain/FFI risk is confined
to this isolated preflight and must be proved on Windows, macOS, and Linux
before the candidate is accepted into an ordinary dev-dependency graph. It
must never enter the shipping normal dependency graph.

## Commands executed

All commands below ran from the generated package after its manifest declared
edition \`2024\`, \`rust-version = "1.85"\`, and
\`license = "MIT OR Apache-2.0"\`; its source imports one item from each
candidate.

\`\`\`text
cargo +1.85.0 check --all-targets
cargo deny check licenses
cargo deny check advisories bans sources
cargo metadata --locked --format-version 1 >metadata.json
cargo tree --edges normal,build >tree-normal-build.txt
\`\`\`

Every command exited 0. The locked metadata and normal/build-edge tree were
generated before this evidence was written; their success proves Cargo locked
the candidate graph and exposes both normal and build dependencies for review.
EOF
