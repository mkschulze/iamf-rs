#!/usr/bin/env bash
#
# red-evidence.sh — wrap tools/cargo-test-tap.sh output in the JSON record
# `gsd-tools check tdd-red-evidence <record.json>` expects.
#
# Why this exists: the gate takes a record, not raw TAP —
# {command, exitCode, targetTest, output}. tools/cargo-test-tap.sh produces the
# `output` field; this script produces the envelope. Plans 01-05 and 01-06 each
# rediscovered that and wrote a throwaway; committing it stops the third.
#
# Neither script invents a result. cargo's real exit code is preserved and the
# TAP is a mechanical reformat of a real run.
#
# Usage:
#   tools/red-evidence.sh [--target-test <name>] <record.json> [tap args...]
#   tools/red-evidence.sh --target-test bcg_5_1_packs_pairs_first /tmp/red.json --test packing
#   gsd-tools check tdd-red-evidence /tmp/red.json
#
# --target-test names the TEST. Without it the value is inferred from
# `--test <name>`, which is the integration-test BINARY name -- the gate then
# cannot match it against a `not ok` line and reports INVALID_RED (found by
# plan 01-07).
set -uo pipefail

TARGET_OVERRIDE=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --target-test) TARGET_OVERRIDE="${2:-}"; shift 2;;
    --target-test=*) TARGET_OVERRIDE="${1#*=}"; shift;;
    *) break;;
  esac
done

if [ "$#" -lt 1 ]; then
  echo "usage: tools/red-evidence.sh [--target-test <name>] <record.json> [tap args...]" >&2
  exit 2
fi

RECORD="$1"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAP="$HERE/cargo-test-tap.sh"
[ -x "$TAP" ] || { echo "FATAL: $TAP is missing or not executable." >&2; exit 1; }

OUT="$("$TAP" "$@" 2>&1)"
EXIT=$?

# Name the target under test so the gate can attribute the record. An explicit
# --target-test always wins; otherwise fall back to the first `not ok` test name
# in the TAP (the failing test IS the target during RED), and only then to the
# binary name.
TARGET="$TARGET_OVERRIDE"
if [ -z "$TARGET" ]; then
  TARGET="$(printf '%s\n' "$OUT" | sed -n 's/^not ok [0-9][0-9]* - //p' | head -1)"
fi
if [ -z "$TARGET" ]; then
  prev=""
  for a in "$@"; do
    case "$prev" in --test|--bin|--lib) TARGET="$a";; esac
    prev="$a"
  done
fi
[ -n "$TARGET" ] || TARGET="(whole suite)"

RECORD="$RECORD" OUT="$OUT" EXIT="$EXIT" TARGET="$TARGET" ARGS="$*" node -e '
const fs = require("fs");
fs.writeFileSync(process.env.RECORD, JSON.stringify({
  command: ("tools/cargo-test-tap.sh " + (process.env.ARGS || "")).trim(),
  exitCode: Number(process.env.EXIT),
  targetTest: process.env.TARGET,
  output: process.env.OUT,
}, null, 2) + "\n");
'
echo "$OUT"
echo "red-evidence record: $RECORD (cargo exit $EXIT)" >&2
exit "$EXIT"
