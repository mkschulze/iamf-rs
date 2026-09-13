#!/usr/bin/env bash
# portfolio-sync.sh — remind Claude to update the IAMF portfolio contract when
# iamf-rs closes a phase or reaches a milestone.
#
# The portfolio boundary document lives in the Parallax repository and carries
# a "Current maturity" row for iamf-rs. It goes stale silently: nothing in this
# repository fails when a phase closes and the row still describes the last one.
#
# Wired as a SessionStart and PostToolUse hook (.claude/settings.local.json). It
# fingerprints three completion signals and, when the fingerprint differs from
# the last one recorded, injects a one-shot instruction into Claude's context:
#
#   1. completed phases in .planning/ROADMAP.md  (`- [x] **Phase N: ...**`)
#   2. archived milestones under .planning/milestones/
#   3. git tags (releases such as v0.1.0-beta.1)
#
# Event-agnostic on purpose: GSD marks phases complete through `gsd-tools` run
# via Bash, not through Edit/Write, so matching on file-edit events would miss
# it. The first run records a baseline and stays silent.
#
# Override the document location with IAMF_PORTFOLIO_DOC. Missing document =
# silent no-op (e.g. a clone without a sibling Parallax checkout).

set -euo pipefail

repo=$(git -C "${CLAUDE_PROJECT_DIR:-$(pwd)}" rev-parse --show-toplevel 2>/dev/null) || exit 0
doc=${IAMF_PORTFOLIO_DOC:-"$repo/../Parallax/docs/superpowers/specs/2026-09-13-iamf-library-portfolio-boundaries-design.md"}
state="$repo/.claude/portfolio-sync.state"

[ -f "$doc" ] || exit 0

fingerprint() {
    {
        echo "# phases"
        sed -n 's/^- \[x\] \*\*\(Phase [^*]*\)\*\*.*/\1/p' "$repo/.planning/ROADMAP.md" 2>/dev/null || true
        echo "# milestones"
        ls -1 "$repo/.planning/milestones" 2>/dev/null || true
        echo "# tags"
        git -C "$repo" tag --list 2>/dev/null || true
    }
}

current=$(fingerprint)

if [ ! -f "$state" ]; then
    printf '%s\n' "$current" > "$state"
    exit 0
fi

previous=$(cat "$state")
[ "$current" = "$previous" ] && exit 0

added=$(diff <(printf '%s\n' "$previous") <(printf '%s\n' "$current") | sed -n 's/^> //p' | grep -v '^#' || true)
printf '%s\n' "$current" > "$state"

event=$(jq -r '.hook_event_name // "PostToolUse"' 2>/dev/null || echo PostToolUse)
doc_abs=$(cd "$(dirname "$doc")" && pwd)/$(basename "$doc")

context="iamf-rs completion state changed (new: ${added:-reordered/removed entries}). Update the iamf-rs row of the IAMF portfolio contract at ${doc_abs}: refresh its 'Current maturity' cell (and 'Owns' only if ownership actually changed) to match .planning/ROADMAP.md, .planning/STATE.md and the latest tag. Keep the document's Change control rules: an ownership change or new cross-library object also needs HANDOFF.md and the consumer pin updated together. That file is in the Parallax repository — edit it there, show the diff, and do not commit in Parallax without asking."

jq -n --arg event "$event" --arg ctx "$context" --arg msg "iamf-rs phase/milestone change detected — portfolio contract needs an update" \
    '{systemMessage: $msg, hookSpecificOutput: {hookEventName: $event, additionalContext: $ctx}}'
