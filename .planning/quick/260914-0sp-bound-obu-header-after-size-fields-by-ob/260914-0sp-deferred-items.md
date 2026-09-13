# 260914-0sp deferred items

1. **Float-escape census pattern is stale.** `rg 'allow.*disallowed_types' src/` returns 0 hits:
   rustfmt split the single allowed `#[allow(` in `src/model/loudness.rs:94-95` across two lines. The
   invariant still holds (`rg -U -c 'allow\(\s*clippy::disallowed_types' src/` → exactly 1), but the
   documented gate in `CLAUDE.md`, `.claude/CLAUDE.md` and plan templates can no longer detect a second
   escape. Pre-existing (file untouched since `2d61359`). Fix: switch every documented/CI occurrence to the
   multiline pattern, and prove it fires (e.g. in `tools/prove-guards.sh`).
