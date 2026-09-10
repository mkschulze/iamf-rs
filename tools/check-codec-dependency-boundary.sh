#!/usr/bin/env bash
# CODEC-07: codec generation lives only in tools/codec-fixtures. The shipping
# crate must neither resolve nor import a codec or resampler implementation.
set -euo pipefail

SCRIPT_ROOT="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORBIDDEN=(claxon flacenc opus opusic-sys audiopus rubato)

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

require_file() {
    local file="$1"
    [[ -f "${file}" && ! -L "${file}" ]] || {
        printf 'required regular file is missing: %s\n' "${file}" >&2
        exit 1
    }
}

manifest_has_forbidden_dependency() {
    local package="$1"
    awk -v package="${package}" '
        /^\[[^]]+\][[:space:]]*$/ { section = $0; next }
        section == "[dependencies]" || section == "[dev-dependencies]" {
            if ($0 ~ "^[[:space:]]*" package "[[:space:]]*=") found = 1
            if ($0 ~ "package[[:space:]]*=[[:space:]]*\\\"" package "\\\"") found = 1
        }
        END { exit found ? 0 : 1 }
    ' "${ROOT}/Cargo.toml"
}

lock_has_forbidden_package() {
    local package="$1"
    awk -v package="${package}" '
        /^\[\[package\]\][[:space:]]*$/ { in_package = 1; next }
        in_package && $0 ~ "^name[[:space:]]*=[[:space:]]*\\\"" package "\\\"[[:space:]]*$" { found = 1 }
        END { exit found ? 0 : 1 }
    ' "${ROOT}/Cargo.lock"
}

check_manifest_and_lock() {
    require_file "${ROOT}/Cargo.toml"
    require_file "${ROOT}/Cargo.lock"

    if ! awk '
        /^\[workspace\][[:space:]]*$/ { in_workspace = 1; next }
        /^\[/ { in_workspace = 0 }
        in_workspace && /exclude[[:space:]]*=/ && /"tools\/codec-fixtures"/ { found = 1 }
        END { exit found ? 0 : 1 }
    ' "${ROOT}/Cargo.toml"; then
        printf 'root [workspace] exclude must contain tools/codec-fixtures\n' >&2
        return 1
    fi

    local package
    for package in "${FORBIDDEN[@]}"; do
        if manifest_has_forbidden_dependency "${package}"; then
            printf 'root [dependencies] or [dev-dependencies] must not contain codec package %s\n' "${package}" >&2
            return 1
        fi
        if lock_has_forbidden_package "${package}"; then
            printf 'root Cargo.lock must not contain codec package %s\n' "${package}" >&2
            return 1
        fi
    done
}

check_src_imports() {
    [[ -d "${ROOT}/src" && ! -L "${ROOT}/src" ]] || {
        printf 'root src must be a non-symlink directory: %s\n' "${ROOT}/src" >&2
        return 1
    }
    local package pattern match
    for package in "${FORBIDDEN[@]}"; do
        pattern="^[[:space:]]*(pub[[:space:]]+)?use[[:space:]]+(\\{[[:space:]]*)?(::)?${package}::|^[[:space:]]*extern[[:space:]]+crate[[:space:]]+${package}([[:space:];]|$)"
        match="$(grep -rEn --include='*.rs' "${pattern}" "${ROOT}/src" || true)"
        if [[ -n "${match}" ]]; then
            printf 'src import must not name codec package %s:\n%s\n' "${package}" "${match}" >&2
            return 1
        fi
    done
}

check_normal_graph() {
    local graph expected
    graph="$(cd "${ROOT}" && cargo tree --locked -e normal,no-proc-macro --prefix none | sed -E 's/[[:space:]].*$//' | sort -u)"
    expected=$'iamf\nthiserror'
    if [[ "${graph}" != "${expected}" ]]; then
        printf 'locked normal/no-proc graph must be exactly iamf and thiserror; got:\n%s\n' "${graph}" >&2
        return 1
    fi
}

check_root() {
    check_manifest_and_lock
    check_src_imports
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

    printf '\n[dependencies]\nopus = "0.4"\n' >> "${CANARY_DIR}/Cargo.toml"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: [dependencies] opus canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"[dependencies]"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: dependency canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/Cargo.toml" "${CANARY_DIR}/Cargo.toml"
    printf '\nuse opus::Encoder;\n' >> "${CANARY_DIR}/src/lib.rs"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: src import canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"src import"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: src import canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    printf 'CODEC-07 boundary self-test passed\n'
}

if "${SELF_TEST}"; then
    self_test
else
    check_root
fi
