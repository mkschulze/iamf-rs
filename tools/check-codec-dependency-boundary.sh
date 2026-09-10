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
        function read_string(text, start,    delimiter, multiline, content, i, quotes) {
            delimiter = substr(text, start, 1)
            multiline = substr(text, start, 3) == delimiter delimiter delimiter
            content = start + (multiline ? 3 : 1)
            if (multiline && substr(text, content, 2) == "\r\n") content += 2
            else if (multiline && substr(text, content, 1) == "\n") ++content
            i = content
            while (i <= length(text)) {
                if (delimiter == "\"" && substr(text, i, 1) == "\\") i += 2
                else if (substr(text, i, 1) == delimiter) {
                    if (!multiline) {
                        string_value = substr(text, content, i - content)
                        string_end = i + 1
                        return 1
                    }
                    quotes = 0
                    while (substr(text, i + quotes, 1) == delimiter) ++quotes
                    if (quotes >= 3) {
                        if (quotes > 5) return 0
                        string_value = substr(text, content, i + quotes - 3 - content)
                        string_end = i + quotes
                        return 1
                    }
                    i += quotes
                } else ++i
            }
            return 0
        }
        function strip_comments(text, mask_strings,    i, ch, result, value) {
            i = 1
            while (i <= length(text)) {
                ch = substr(text, i, 1)
                if (ch == "\"" || ch == sprintf("%c", 39)) {
                    if (!read_string(text, i)) string_end = length(text) + 1
                    value = substr(text, i, string_end - i)
                    if (mask_strings) gsub(/[^\n]/, " ", value)
                    result = result value
                    i = string_end
                } else if (ch == "#") {
                    while (i <= length(text) && substr(text, i, 1) != "\n") ++i
                } else { result = result ch; ++i }
            }
            return result
        }
        function consume_array(text,    i, ch) {
            i = 1
            while (i <= length(text)) {
                ch = substr(text, i, 1)
                if (ch == "\"" || ch == sprintf("%c", 39)) {
                    if (!read_string(text, i)) return
                    if (string_value == "tools/codec-fixtures") found = 1
                    i = string_end
                } else if (ch == "]") { collecting = 0; done = 1; return }
                else ++i
            }
        }
        { manifest = manifest $0 "\n" }
        END {
            # Parse complete strings across physical lines in both passes.
            manifest = strip_comments(manifest)
            count = split(strip_comments(manifest, 1), lines, "\n")
            offset = 1
            for (row = 1; row <= count; ++row) {
                line = lines[row]
                line_start = offset
                offset += length(line) + 1
                if (line ~ /^[[:space:]]*\[/) {
                    in_workspace = line ~ /^[[:space:]]*\[workspace\][[:space:]]*$/
                    continue
                }
                if (in_workspace && match(line, /^[[:space:]]*exclude[[:space:]]*=[[:space:]]*\[/)) {
                    collecting = 1
                    consume_array(substr(manifest, line_start + RLENGTH))
                    break
                }
            }
            exit done && found ? 0 : 1
        }
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
        my ($s, $out, $i) = ($_, $_, 0);
        while ($i < length $s) {
            my $start = $i;
            if (substr($s, $i, 2) eq "/*") {
                my $depth = 1; $i += 2;
                while ($i < length($s) && $depth) {
                    if (substr($s, $i, 2) eq "/*") { ++$depth; $i += 2 }
                    elsif (substr($s, $i, 2) eq "*/") { --$depth; $i += 2 }
                    else { ++$i }
                }
            } elsif (substr($s, $i, 2) eq "//") {
                $i = index($s, "\n", $i); $i = length($s) if $i < 0;
            } elsif (($i == 0 || substr($s, $i - 1, 1) !~ /[A-Za-z0-9_\x80-\xff]/)
                && substr($s, $i) =~ /\A b?r(\#*)"/x) {
                my $end = q{"} . $1; $i += length($&);
                my $at = index($s, $end, $i);
                $i = $at < 0 ? length($s) : $at + length($end);
            } elsif (substr($s, $i) =~ /\A b?"/x) {
                $i += length($&);
                while ($i < length $s) {
                    if (substr($s, $i, 1) eq q{\\}) { $i += 2 }
                    elsif (substr($s, $i++, 1) eq q{"}) { last }
                }
                $i = length($s) if $i > length($s);
            } elsif (substr($s, $i) =~ /\A b?\x27
                (?: \\ (?: u\{[0-9A-Fa-f_]+\} | x[0-9A-Fa-f]{2} | [^\r\n] )
                  | [^\\\x27\r\n\x80-\xff] | [\xc2-\xf4][\x80-\xbf]+ ) \x27/x) {
                # Require a closing quote after one character, so lifetimes
                # cannot swallow a subsequent import.
                $i += length($&);
            } else {
                ++$i; next;
            }
            # Blank the entire literal/comment while keeping line positions
            # and token separators identical to the portable Rust scanner.
            substr($out, $start, $i - $start) =~ s/[^\n]/ /g;
        }
        my @forbidden = split /\s+/, $ENV{FORBIDDEN_PACKAGES};
        # Visibility qualifiers naturally precede the globally located use
        # token; a preceding function/module block must not hide it.
        while ($out =~ /(?<![A-Za-z0-9_\x80-\xff])(?<!r\#)
            (?: use(?![A-Za-z0-9_\x80-\xff]) | extern\s+crate(?![A-Za-z0-9_\x80-\xff]) )
            ([^;]*);/gx) {
            my $statement = $1;
            for my $package (@forbidden) {
                my $crate = $package =~ s/-/_/gr;
                for my $branch (split /[{},]/, $statement) {
                    if ($branch =~ /^\s*(?:::)?\s*(?:r\#)?([A-Za-z0-9_\x80-\xff]+)/ && $1 eq $crate) {
                        print $package; exit;
                    }
                }
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
        local description="$1" source="$2" package="${3:-opus}"
        cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"
        printf '\n%s\n' "${source}" >> "${CANARY_DIR}/src/lib.rs"
        if output="$(bash "$0" --root "${CANARY_DIR}" 2>&1)"; then
            printf 'self-test failed: %s canary passed\n' "${description}" >&2
            return 1
        elif [[ "${output}" != *"src import"* || "${output}" != *"${package}"* ]]; then
            printf 'self-test failed: %s error was not specific:\n%s\n' "${description}" "${output}" >&2
            return 1
        fi
    }
    expect_harmless_canary() {
        local description="$1" source="$2"
        printf '%s\n' "${source}" > "${CANARY_DIR}/src/lib.rs"
        if ! output="$(ROOT="${CANARY_DIR}" check_src_imports 2>&1)"; then
            printf 'self-test failed: harmless %s was rejected:\n%s\n' "${description}" "${output}" >&2
            return 1
        fi
    }
    expect_workspace_canary() {
        local description="$1" manifest="$2" accepted="$3" actual=false
        printf '%s\n' "${manifest}" > "${CANARY_DIR}/Cargo.toml"
        if output="$(ROOT="${CANARY_DIR}" check_manifest_and_lock 2>&1)"; then
            actual=true
        elif [[ "${output}" != *"[workspace] exclude"* ]]; then
            printf 'self-test failed: %s error was not specific:\n%s\n' "${description}" "${output}" >&2
            return 1
        fi
        if [[ "${actual}" != "${accepted}" ]]; then
            printf 'self-test failed: %s expected accepted=%s, got %s\n' "${description}" "${accepted}" "${actual}" >&2
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

    local failures=0
    expect_import_canary "raw crate use" 'use r#opus::Encoder;' || failures=$((failures + 1))
    expect_import_canary "raw extern crate" 'extern crate r#opus;' || failures=$((failures + 1))
    expect_import_canary "raw normalized crate use" 'use {std::fmt, r#opusic_sys};' opusic-sys || failures=$((failures + 1))
    expect_import_canary "raw normalized extern crate" 'extern crate r#opusic_sys as codec;' opusic-sys || failures=$((failures + 1))
    expect_import_canary "raw keyword before real import" 'fn main() { let r#use = { use opus::Encoder; }; }' || failures=$((failures + 1))
    expect_harmless_canary "raw use function name" 'fn opus() {} fn r#use() { opus(); }' || failures=$((failures + 1))
    expect_import_canary "compact grouped name" 'use {opus};' || failures=$((failures + 1))
    expect_import_canary "extern crate" 'extern crate opus;' || failures=$((failures + 1))
    expect_import_canary "function then import" $'fn harmless() {}\nuse opus::Encoder;' || failures=$((failures + 1))
    expect_import_canary "restricted visibility" 'pub(crate) use ::opus::Encoder;' || failures=$((failures + 1))
    expect_import_canary "path visibility" 'pub(in crate::private) use {std::fmt, opusic_sys};' opusic-sys || failures=$((failures + 1))
    expect_import_canary "compact use" 'use{claxon, flacenc};' claxon || failures=$((failures + 1))
    expect_import_canary "extern crate alias" 'extern crate audiopus as codec;' audiopus || failures=$((failures + 1))
    expect_import_canary "module then import" 'mod inner { use rubato::Resampler; }' rubato || failures=$((failures + 1))
    expect_import_canary "comment separator" 'use/* comment */opus::Encoder;' || failures=$((failures + 1))
    expect_import_canary "character quote" "const QUOTE: char = '\"'; use opus::Encoder;" || failures=$((failures + 1))
    expect_import_canary "byte character quote" "const QUOTE: u8 = b'\"'; extern crate opus;" || failures=$((failures + 1))
    expect_import_canary "escaped character" "const QUOTE: char = '\\''; use opus;" || failures=$((failures + 1))
    expect_import_canary "unicode character escape" "const SYMBOL: char = '\\u{1f600}'; use opus;" || failures=$((failures + 1))
    expect_import_canary "lifetime" "fn borrow<'a>(s: &'a str) { use opus::Encoder; }" || failures=$((failures + 1))
    expect_import_canary "raw string then import" 'const HELP: &str = r#"use opus;"#; use flacenc::Encoder;' flacenc || failures=$((failures + 1))
    expect_harmless_canary "raw string with semicolon" 'const HELP: &str = r#"; use opus::Encoder;"#;' || failures=$((failures + 1))
    expect_harmless_canary "raw string with two hashes" 'const HELP: &str = r##"use opus::Encoder;"##;' || failures=$((failures + 1))
    expect_harmless_canary "raw string with embedded closing quote" 'const HELP: &str = r##""#; use opus::Encoder;"##;' || failures=$((failures + 1))
    expect_harmless_canary "raw byte string" 'const HELP: &[u8] = br##"; extern crate opus;"##;' || failures=$((failures + 1))
    expect_harmless_canary "byte string" 'const HELP: &[u8] = b"\"; use opus;";' || failures=$((failures + 1))
    expect_harmless_canary "escaped string" 'const HELP: &str = "\"; use opus;";' || failures=$((failures + 1))
    expect_harmless_canary "nested and line comments" $'/* nested /* use opus; */ extern crate opus; */\n// use opus;' || failures=$((failures + 1))
    expect_harmless_canary "identifier boundaries" 'use opus_extra::Encoder; fn reuse() {}' || failures=$((failures + 1))
    expect_workspace_canary "multiline exclusion" $'[workspace]\nexclude = [\n "fuzz",\n "tools/codec-fixtures",\n]' true || failures=$((failures + 1))
    expect_workspace_canary "comment-only exclusion" $'[workspace]\n# exclude = ["tools/codec-fixtures"]' false || failures=$((failures + 1))
    expect_workspace_canary "later members" $'[workspace]\nexclude = ["fuzz"]\nmembers = ["tools/codec-fixtures"]' false || failures=$((failures + 1))
    expect_workspace_canary "multiline then members" $'[workspace]\nexclude = [\n "fuzz", # "tools/codec-fixtures"\n]\nmembers = ["tools/codec-fixtures"]' false || failures=$((failures + 1))
    expect_workspace_canary "later table" $'[workspace]\nexclude = ["fuzz"]\n[package.metadata]\nexclude = ["tools/codec-fixtures"]' false || failures=$((failures + 1))
    expect_workspace_canary "hash and bracket in string" $'[workspace]\nexclude = ["name#with]bracket", "tools/codec-fixtures"] # comment' true || failures=$((failures + 1))
    expect_workspace_canary "literal string and table comment" $'[workspace] # comment\nexclude = [\'tools/codec-fixtures\']' true || failures=$((failures + 1))
    expect_workspace_canary "trailing comment" $'[workspace]\nexclude = ["fuzz"] # "tools/codec-fixtures"' false || failures=$((failures + 1))
    expect_workspace_canary "different key" $'[workspace]\nexclude_more = ["tools/codec-fixtures"]' false || failures=$((failures + 1))
    expect_workspace_canary "multiline basic embedded path" $'[workspace]\nexclude = ["""prefix""tools/codec-fixtures""suffix"""]' false || failures=$((failures + 1))
    expect_workspace_canary "multiline literal embedded path" $'[workspace]\nexclude = [\'\'\'prefix\'\'tools/codec-fixtures\'\'suffix\'\'\']' false || failures=$((failures + 1))
    expect_workspace_canary "multiline path between lines" $'[workspace]\nexclude = ["""prefix\n"tools/codec-fixtures"\nsuffix"""]' false || failures=$((failures + 1))
    expect_workspace_canary "exact multiline basic path" $'[workspace]\nexclude = ["""tools/codec-fixtures"""]' true || failures=$((failures + 1))
    expect_workspace_canary "multiline initial newline" $'[workspace]\nexclude = ["""\ntools/codec-fixtures"""]' true || failures=$((failures + 1))
    expect_workspace_canary "literal multiline initial newline" $'[workspace]\nexclude = [\'\'\'\ntools/codec-fixtures\'\'\']' true || failures=$((failures + 1))
    expect_workspace_canary "four closing quotes" $'[workspace]\nexclude = ["""tools/codec-fixtures""""]' false || failures=$((failures + 1))
    expect_workspace_canary "five closing quotes" $'[workspace]\nexclude = ["""tools/codec-fixtures"""""]' false || failures=$((failures + 1))
    expect_workspace_canary "multiline punctuation then real entry" $'[workspace]\nexclude = ["""prefix\n# ] ""suffix""", "tools/codec-fixtures"]' true || failures=$((failures + 1))
    expect_workspace_canary "multiline embedded path then members" $'[workspace]\nexclude = ["""prefix""tools/codec-fixtures""suffix"""]\nmembers = ["tools/codec-fixtures"]' false || failures=$((failures + 1))
    expect_workspace_canary "workspace inside multiline content" $'[package.metadata]\nnote = """\n[workspace]\nexclude = ["tools/codec-fixtures"]\n"""\n[workspace]\nexclude = ["fuzz"]' false || failures=$((failures + 1))
    expect_workspace_canary "exclude inside multiline content" $'[workspace]\nmetadata.note = \'\'\'\nexclude = ["tools/codec-fixtures"]\n\'\'\'\nexclude = ["fuzz"]' false || failures=$((failures + 1))
    [[ "${failures}" -eq 0 ]] || return 1
    cp "${SCRIPT_ROOT}/Cargo.toml" "${CANARY_DIR}/Cargo.toml"
    cp "${SCRIPT_ROOT}/src/lib.rs" "${CANARY_DIR}/src/lib.rs"

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
    expect_import_canary "hyphen-normalized import" 'use opusic_sys::Encoder;' opusic-sys
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
