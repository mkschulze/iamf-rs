#!/usr/bin/env bash
#
# cargo-test-tap.sh — run `cargo test` and re-emit libtest's results as TAP 13.
#
# Why this exists: GSD's TDD RED gate (`gsd-tools check tdd-red-evidence`)
# classifies a RED-phase test run from TAP output — `# tests`/`# pass`/`# fail`
# counters and `not ok N - <name>` lines. Rust's libtest emits its own format
# and has no stable TAP or JSON reporter (`--format json` is nightly-only, and
# GUARD-12 pins a stable channel). This script is a faithful, mechanical
# translation of a real run: it invents nothing, it only reformats. The exit
# status of `cargo test` is preserved verbatim so the gate sees the true code.
#
# Usage:
#   tools/cargo-test-tap.sh                     # whole suite
#   tools/cargo-test-tap.sh --test error_shape  # one integration test target
#
# `--test-threads=1` is appended so libtest prints one result per line. With
# the default thread pool, concurrent tests interleave their `test <name> ...`
# prefixes with other tests' results on a single line and the translation
# becomes ambiguous.

set -uo pipefail

set +e
RAW="$(cargo test "$@" -- --test-threads=1 2>&1)"
STATUS=$?
set -e

printf '%s\n' "${RAW}" | awk '
BEGIN { n = 0; pass = 0; fail = 0; print "TAP version 13" }
{
    # libtest result lines look like:  test <name> ... ok
    #                                  test <name> ... FAILED
    #                                  test <name> ... ignored, <reason>
    if ($0 ~ /^test .* \.\.\. /) {
        line = $0
        sub(/^test /, "", line)
        idx = index(line, " ... ")
        name = substr(line, 1, idx - 1)
        verdict = substr(line, idx + 5)

        n = n + 1
        if (verdict ~ /^ok/) {
            pass = pass + 1
            printf "ok %d - %s\n", n, name
        } else if (verdict ~ /^ignored/) {
            # A skipped test did not run; TAP SKIP keeps it out of pass/fail.
            printf "ok %d - %s # SKIP %s\n", n, name, verdict
        } else {
            fail = fail + 1
            printf "not ok %d - %s\n", n, name
        }
    } else {
        # Everything else (compiler output, panic messages, the libtest
        # `failures:` block) is preserved as a TAP comment so the record keeps
        # the diagnostic detail a human needs.
        printf "# %s\n", $0
    }
}
END {
    printf "1..%d\n", n
    printf "# tests %d\n", n
    printf "# pass %d\n", pass
    printf "# fail %d\n", fail
}
'

exit "${STATUS}"
