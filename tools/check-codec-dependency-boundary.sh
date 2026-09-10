#!/usr/bin/env bash
# CODEC-07: codec generation lives only in tools/codec-fixtures. The shipping
# crate must neither resolve nor import a codec or resampler implementation.
set -euo pipefail

SCRIPT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    printf 'usage: %s [--root PATH | --self-test]\n' "${0##*/}" >&2
    exit 2
}

canonical_root() {
    local candidate="$1"
    if [[ ! -d "${candidate}" || -L "${candidate}" ]]; then
        printf 'root must be a non-symlink directory: %s\n' "${candidate}" >&2
        exit 2
    fi
    (cd -P "${candidate}" && pwd)
}

ROOT="${SCRIPT_ROOT}"
SELF_TEST=false
CANARY_DIR=""
CANARY_BASE=""
case "${1:-}" in
    '') ;;
    --root)
        [[ $# -eq 2 ]] || usage
        ROOT="$(canonical_root "$2")"
        ;;
    --self-test)
        [[ $# -eq 1 ]] || usage
        SELF_TEST=true
        ;;
    *) usage ;;
esac

check_normal_graph() {
    local graph expected
    graph="$(cargo tree --locked --manifest-path "${ROOT}/Cargo.toml" -e normal,no-proc-macro --prefix none | sed -E 's/[[:space:]].*$//' | sort -u)"
    expected=$'iamf\nthiserror'
    if [[ "${graph}" != "${expected}" ]]; then
        printf 'locked normal/no-proc graph must be exactly iamf and thiserror; got:\n%s\n' "${graph}" >&2
        return 1
    fi
}

check_root() {
    CODEC_BOUNDARY_ROOT="${ROOT}" cargo +1.85.0 test --locked \
        --manifest-path "${SCRIPT_ROOT}/Cargo.toml" \
        --test codec_dependency_boundary \
        -- --exact codec_dependencies_and_src_imports_remain_outside_the_root_crate
    check_normal_graph
    printf 'CODEC-07 dependency boundary passes for %s\n' "${ROOT}"
}

safe_self_test_dir() {
    local temp_base="${TMPDIR:-/tmp}"
    [[ -d "${temp_base}" && ! -L "${temp_base}" ]] || {
        printf 'temporary base must be a non-symlink directory: %s\n' "${temp_base}" >&2
        exit 1
    }
    temp_base="$(cd -P "${temp_base}" && pwd)"
    case "${temp_base}" in "${SCRIPT_ROOT}"|"${SCRIPT_ROOT}"/*)
        printf 'refusing temporary base inside repository: %s\n' "${temp_base}" >&2
        exit 1 ;;
    esac
    mktemp -d "${temp_base}/iamf-codec-boundary.XXXXXX"
}

self_test() {
    local output
    CANARY_DIR="$(safe_self_test_dir)"
    CANARY_BASE="$(cd -P "$(dirname "${CANARY_DIR}")" && pwd)"
    CANARY_DIR="$(cd -P "${CANARY_DIR}" && pwd)"
    case "${CANARY_DIR}" in "${CANARY_BASE}"/iamf-codec-boundary.*) ;;
        *) printf 'refusing unexpected canary path: %s\n' "${CANARY_DIR}" >&2; exit 1 ;;
    esac
    [[ -d "${CANARY_DIR}" && ! -L "${CANARY_DIR}" ]] || {
        printf 'unsafe canary directory: %s\n' "${CANARY_DIR}" >&2
        exit 1
    }
    cleanup() {
        [[ -d "${CANARY_DIR}" && ! -L "${CANARY_DIR}" ]] || return
        case "${CANARY_DIR}" in "${CANARY_BASE}"/iamf-codec-boundary.*) rm -rf -- "${CANARY_DIR}" ;;
            *) printf 'refusing unsafe canary cleanup: %s\n' "${CANARY_DIR}" >&2; return 1 ;;
        esac
    }
    trap cleanup EXIT

    cp "${SCRIPT_ROOT}/Cargo.toml" "${SCRIPT_ROOT}/Cargo.lock" "${CANARY_DIR}/"
    cp -R "${SCRIPT_ROOT}/src" "${CANARY_DIR}/src"

    printf '%s\n' \
        '[package]' \
        'name = "boundary-canary"' \
        'version = "0.1.0"' \
        'edition = "2024"' \
        '[workspace]' \
        'exclude = ["tools/codec-fixtures"]' \
        '[dependencies]' \
        '"opus" = "0.4"' \
        > "${CANARY_DIR}/Cargo.toml"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: quoted [dependencies] opus canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"[dependencies]"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: quoted dependency canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/Cargo.toml" "${CANARY_DIR}/Cargo.toml"
    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\nextern crate opus;\n' >> "${CANARY_DIR}/src/lib.rs"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: extern crate opus canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"src import"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: extern crate opus canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    printf 'CODEC-07 boundary self-test passed\n'
}

if "${SELF_TEST}"; then
    self_test
else
    check_root
fi
