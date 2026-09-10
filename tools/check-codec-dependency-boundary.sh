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
FOUND_PACKAGE=""
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
        in_workspace {
            line = $0; sub(/#.*/, "", line)
            if (collecting || line ~ /^[[:space:]]*exclude[[:space:]]*=/) {
                collecting = 1; exclude = exclude line
                if (line ~ /\]/) done = 1
            }
        }
        END { exit done && exclude ~ /"tools\/codec-fixtures"/ ? 0 : 1 }
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
    local file
    while IFS= read -r -d '' file; do
        if file_has_codec_import "${file}"; then
            printf 'src import must not name codec package %s: %s\n' "${FOUND_PACKAGE}" "${file}" >&2
            return 1
        fi
    done < <(find "${ROOT}/src" -type f -name '*.rs' -print0)
}

file_has_codec_import() {
    local file="$1"
    FOUND_PACKAGE="$(LC_ALL=C FORBIDDEN_PACKAGES="${FORBIDDEN[*]}" perl -0ne '
        my ($s, $out, $i, $depth) = ($_, q{}, 0, 0);
        while ($i < length $s) {
            if ($depth) {
                if (substr($s, $i, 2) eq "/*") { ++$depth; $i += 2; next }
                if (substr($s, $i, 2) eq "*/") { --$depth; $i += 2; next }
                $out .= "\n" if substr($s, $i, 1) eq "\n"; ++$i; next;
            }
            if (substr($s, $i, 2) eq "/*") { $depth = 1; $i += 2; next }
            if (substr($s, $i, 2) eq "//") { $i = index($s, "\n", $i); $i = length($s) if $i < 0; next }
            if (substr($s, $i) =~ /\A r(\#*)"/x) {
                my $end = q{"} . $1; $i += length($&); my $at = index($s, $end, $i);
                $at = length($s) if $at < 0; $out .= (substr($s, $i, $at - $i) =~ tr/\n/\n/r); $i = $at + length($end); next;
            }
            if (substr($s, $i, 1) eq q{"}) {
                ++$i; while ($i < length $s && substr($s, $i, 1) ne q{"}) { $out .= "\n" if substr($s,$i,1) eq "\n"; $i += substr($s,$i,1) eq q{\\} ? 2 : 1 } ++$i; next;
            }
            $out .= substr($s, $i++, 1);
        }
        my @forbidden = split /\s+/, $ENV{FORBIDDEN_PACKAGES};
        for my $statement (split /;/, $out) {
            $statement =~ s/^\s*pub(?:\([^)]*\))?\s+//; next unless $statement =~ s/^\s*use\s+//;
            for my $package (@forbidden) { my $crate = $package =~ s/-/_/gr;
                if ($statement =~ /(?:^|[,{])\s*(?:::)?\Q$crate\E(?=\s|:|$)/) { print $package; exit }
            }
        }
    ' "${file}")"
    [[ -n "${FOUND_PACKAGE}" ]]
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
    expect_import_canary() {
        local description="$1" source="$2"
        cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
        printf '\n%s\n' "${source}" >> "${CANARY_DIR}/src/lib.rs"
        if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
            printf 'self-test failed: %s canary passed\n' "${description}" >&2
            return 1
        elif [[ "${output}" != *"src import"* ]]; then
            printf 'self-test failed: %s error was not specific:\n%s\n' "${description}" "${output}" >&2
            return 1
        fi
    }
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

    awk '
        /^\[dependencies\][[:space:]]*$/ { print; print "opus = \"0.4\""; next }
        { print }
    ' "${CANARY_DIR}/Cargo.toml" > "${CANARY_DIR}/Cargo.toml.with-opus"
    mv "${CANARY_DIR}/Cargo.toml.with-opus" "${CANARY_DIR}/Cargo.toml"
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

    expect_import_canary "grouped branch" 'use { std::fmt, opus::Encoder };'
    expect_import_canary "grouped name" 'use { opus };'
    expect_import_canary "multiline group" $'use {\n    std::fmt,\n    opus::Encoder,\n};'
    expect_import_canary "visible import" 'pub use opus::Encoder;'
    expect_import_canary "hyphen-normalized import" 'use opusic_sys::Encoder;'
    expect_import_canary "string-followed import" 'const TOKEN: &str = "/*"; use opus::Encoder;'
    expect_import_canary "two comments followed import" '/* one */ /* two */ use opus;'

    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\nconst TOKEN: &str = r#"use opus::Encoder;"#;\n' >> "${CANARY_DIR}/src/lib.rs"
    if ! output="$(ROOT="${CANARY_DIR}" check_src_imports 2>&1)"; then
        printf 'self-test failed: raw-string canary was treated as an import:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\nuse opus;\n' >> "${CANARY_DIR}/src/lib.rs"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: simple src import canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"src import"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: simple import canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\nuse opus as codec;\n' >> "${CANARY_DIR}/src/lib.rs"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: aliased src import canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"src import"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: aliased import canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\nuse { opus::Encoder };\n' >> "${CANARY_DIR}/src/lib.rs"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: grouped src import canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"src import"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: grouped import canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\n/* comment */ use opus;\n' >> "${CANARY_DIR}/src/lib.rs"
    if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
        printf 'self-test failed: post-comment src import canary passed\n' >&2
        return 1
    elif [[ "${output}" != *"src import"* || "${output}" != *"opus"* ]]; then
        printf 'self-test failed: post-comment import canary error was not specific:\n%s\n' "${output}" >&2
        return 1
    fi

    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
    printf '\n/* use opus::Encoder; */\n' >> "${CANARY_DIR}/src/lib.rs"
    if ! output="$(ROOT="${CANARY_DIR}" check_src_imports 2>&1)"; then
        printf 'self-test failed: block-comment canary was treated as an import:\n%s\n' "${output}" >&2
        return 1
    fi

    printf 'CODEC-07 boundary self-test passed\n'
}

if "${SELF_TEST}"; then
    self_test
else
    check_root
fi
