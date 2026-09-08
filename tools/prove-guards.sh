#!/usr/bin/env bash
#
# prove-guards.sh — GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-11
#
# A configured guardrail and a working guardrail are different things. This
# script takes the crate as committed, introduces one deliberate violation per
# guardrail, runs the gate, and asserts the gate both exits non-zero and names
# the lint (or the licence) it was supposed to catch. It exits 0 only when
# every case fired.
#
# CI runs this on the Linux job, so a lint that was silently disabled — the
# single most likely way this project's hardening quietly stops existing —
# turns the matrix red and names which guardrail stopped working.
#
# Six cases:
#   (a) GUARD-01  an LGPL-licensed dependency        -> cargo deny check licenses
#   (b) GUARD-02  std::collections::HashMap          -> clippy::disallowed_types
#   (c) GUARD-02  the same type behind an alias      -> clippy::disallowed_types
#   (d) GUARD-03  a bare slice index                 -> clippy::indexing_slicing
#   (e) GUARD-03/04  unchecked `+` and `unwrap()`    -> clippy::arithmetic_side_effects
#                                                       clippy::unwrap_used
#   (f) GUARD-11  an f64 local and a call to `sin`   -> clippy::disallowed_types
#                                                       clippy::disallowed_methods
#
# Case (c) exists on its own because the ban must be on the fully-qualified
# definition path, not on the spelling at the use site. A `disallowed-types`
# entry that only matched the literal text `HashMap` would pass case (b) and
# fail to protect anything.
#
# Case (a) uses a local path-dependency canary rather than a real crates.io
# crate on purpose: the proof must be hermetic and offline (CONF-10 requires
# `cargo test` green offline on all four targets), and it must not depend on
# some third party keeping an LGPL crate published at a resolvable version.
# What is being proven is that the allow-list rejects an LGPL licence
# expression, and a path dependency is a dependency for that purpose —
# cargo-deny evaluates it in the graph exactly like any other crate.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROOF_DIR="${REPO_ROOT}/target/guard-proof"

PASS_COUNT=0
FAIL_COUNT=0
SUMMARY=()

# ---------------------------------------------------------------------------
# Reset the proof crate to a pristine copy of the crate as committed.
#
# `[workspace]` is appended so the copy is its own workspace root. Without it
# cargo walks up to the real Cargo.toml and refuses to build a package that is
# not a member of that workspace.
# ---------------------------------------------------------------------------
reset_crate() {
    rm -rf "${PROOF_DIR}"
    mkdir -p "${PROOF_DIR}"
    cp "${REPO_ROOT}/Cargo.toml" \
       "${REPO_ROOT}/Cargo.lock" \
       "${REPO_ROOT}/clippy.toml" \
       "${REPO_ROOT}/deny.toml" \
       "${REPO_ROOT}/rust-toolchain.toml" \
       "${PROOF_DIR}/"
    cp -R "${REPO_ROOT}/src" "${PROOF_DIR}/src"
    printf '\n[workspace]\n' >> "${PROOF_DIR}/Cargo.toml"
}

record() {
    local verdict="$1" case_id="$2" description="$3"
    if [ "${verdict}" = "PASS" ]; then
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
    SUMMARY+=("${verdict}  case (${case_id})  ${description}")
    printf '%s  case (%s)  %s\n' "${verdict}" "${case_id}" "${description}"
}

# ---------------------------------------------------------------------------
# expect_clippy_fires <case-id> <description> <source> <lint> [<lint>...]
#
# Writes <source> as a new module, runs clippy, and asserts it failed AND that
# every named lint appears in the output. Clippy prints the lint name in its
# `help: for further information visit .../index.html#<lint>` line, which is
# the stable, machine-greppable form.
# ---------------------------------------------------------------------------
expect_clippy_fires() {
    local case_id="$1" description="$2" source="$3"
    shift 3

    reset_crate
    printf '%s\n' "${source}" > "${PROOF_DIR}/src/violation.rs"
    printf '\npub mod violation;\n' >> "${PROOF_DIR}/src/lib.rs"

    local output status
    set +e
    output="$(cd "${PROOF_DIR}" && cargo clippy --all-targets --offline --color never 2>&1)"
    status=$?
    set -e

    if [ "${status}" -eq 0 ]; then
        record FAIL "${case_id}" "${description} — clippy EXITED 0; the guardrail is not firing"
        return
    fi

    local lint
    for lint in "$@"; do
        if ! printf '%s' "${output}" | grep -qE "(#${lint}\b|clippy::${lint//_/-}\b|clippy::${lint}\b)"; then
            record FAIL "${case_id}" "${description} — clippy failed but never named ${lint}"
            return
        fi
    done

    record PASS "${case_id}" "${description} — clippy rejected it and named: $*"
}

# ---------------------------------------------------------------------------
# Case (a) — GUARD-01: the licence allow-list rejects LGPL.
# ---------------------------------------------------------------------------
prove_licence_gate() {
    reset_crate

    mkdir -p "${PROOF_DIR}/lgpl-canary/src"
    cat > "${PROOF_DIR}/lgpl-canary/Cargo.toml" <<'CANARY'
[package]
name = "lgpl-canary"
version = "0.0.0"
edition = "2021"
license = "LGPL-3.0-or-later"
description = "Deliberate GUARD-01 violation. Never depended on outside tools/prove-guards.sh."
CANARY
    printf '// Deliberate GUARD-01 violation canary.\n' \
        > "${PROOF_DIR}/lgpl-canary/src/lib.rs"

    # Insert the offending dependency under `[dependencies]` in the copy.
    # `sed` rather than an append, because appending would land the line under
    # the `[workspace]` table that reset_crate adds at the end of the file.
    sed -i.bak \
        's|^\[dependencies\]$|[dependencies]\nlgpl-canary = { path = "lgpl-canary" } # LGPL-3.0-or-later --- DELIBERATE VIOLATION|' \
        "${PROOF_DIR}/Cargo.toml"
    rm -f "${PROOF_DIR}/Cargo.toml.bak"
    if ! grep -q 'lgpl-canary' "${PROOF_DIR}/Cargo.toml"; then
        record FAIL a "GUARD-01 licence allow-list — could not inject the canary dependency"
        return
    fi

    local output status
    set +e
    output="$(cd "${PROOF_DIR}" && cargo deny --offline check licenses 2>&1)"
    status=$?
    set -e

    if [ "${status}" -eq 0 ]; then
        record FAIL a "GUARD-01 licence allow-list — cargo deny EXITED 0 on an LGPL dependency"
        return
    fi
    if ! printf '%s' "${output}" | grep -q 'LGPL'; then
        record FAIL a "GUARD-01 licence allow-list — cargo deny failed but never named LGPL"
        return
    fi
    record PASS a "GUARD-01 licence allow-list — cargo deny rejected LGPL-3.0-or-later"
}

# ---------------------------------------------------------------------------

echo "prove-guards.sh — proving each guardrail fires on a deliberate violation"
echo "repo: ${REPO_ROOT}"
echo

prove_licence_gate

expect_clippy_fires b \
    "GUARD-02 hashed containers — a direct std::collections::HashMap" \
'pub fn violation() -> std::collections::HashMap<u8, u8> {
    std::collections::HashMap::new()
}' \
    disallowed_types

expect_clippy_fires c \
    "GUARD-02 hashed containers — the same type behind an aliased import" \
'use std::collections::HashMap as Table;

pub fn violation() -> Table<u8, u8> {
    Table::new()
}' \
    disallowed_types

expect_clippy_fires d \
    "GUARD-03 no raw indexing — a bare slice index expression" \
'pub fn violation(bytes: &[u8]) -> u8 {
    bytes[0]
}' \
    indexing_slicing

expect_clippy_fires e \
    "GUARD-03/04 no unchecked arithmetic, no unwrap outside tests" \
'pub fn violation_add(a: u32, b: u32) -> u32 {
    a + b
}

pub fn violation_unwrap(value: Option<u8>) -> u8 {
    value.unwrap()
}' \
    arithmetic_side_effects unwrap_used

expect_clippy_fires f \
    "GUARD-11 no DSP — an f64 local and a platform transcendental" \
'pub fn violation() -> f64 {
    let gain: f64 = 1.0;
    gain.sin()
}' \
    disallowed_types disallowed_methods

echo
echo "-------------------------------------------------------------------"
printf '%s\n' "${SUMMARY[@]}"
echo "-------------------------------------------------------------------"
printf 'guardrail proof: %d passed, %d failed (6 expected)\n' "${PASS_COUNT}" "${FAIL_COUNT}"

if [ "${FAIL_COUNT}" -ne 0 ] || [ "${PASS_COUNT}" -ne 6 ]; then
    echo "GUARDRAIL PROOF FAILED — at least one guardrail is no longer biting." >&2
    exit 1
fi

echo "all six guardrails fired."
