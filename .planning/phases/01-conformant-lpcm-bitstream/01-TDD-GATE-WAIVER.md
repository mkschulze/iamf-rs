---
phase: 01-conformant-lpcm-bitstream
gate: tdd.review-checkpoint
verdict: waived
waived_by: user
waived_on: 2026-09-08
---

# TDD gate waiver — plan 01-05

## What the gate reported

`gsd-tools check tdd.review-checkpoint 01` returned `block: true`, `violations: 1`:

| Plan | RED | GREEN | Status |
|------|-----|-------|--------|
| 01-03 | ✓ | ✓ | Pass |
| 01-04 | ✓ | ✓ | Pass |
| 01-05 | ✗ | ✗ | FAIL |
| 01-06 | ✓ | ✓ | Pass |
| 01-07 | ✓ | ✓ | Pass |

Under `workflow.tdd_mode = true` this escalates from advisory to blocking, and
`execute-phase.md`'s proceed rule refuses to mark the phase complete.

## Why it is a false positive

Plan 01-05's RED→GREEN commits exist, in the correct order, as three complete
cycles. They are scoped `(1-5)` rather than `(01-05)`:

| RED | GREEN |
|---|---|
| `256efd3 test(1-5): add failing descriptor vectors for the IA Sequence Header and Codec Config` | `0a3b506 feat(1-5): implement the IA Sequence Header and Codec Config OBUs` |
| `b6d03c5 test(1-5): add failing Audio Element vectors and the reserved round-trip property` | `328a0e9 feat(1-5): implement the Audio Element OBU and D-04's AudioElementType` |
| `0d8c2cc test(1-5): add failing Mix Presentation vectors and the whole-prologue claim` | `bbeabd2 feat(1-5): implement the Mix Presentation OBU and descriptor ordering` |

(plus `a0e123a fix(1-5): cite the two count helpers so D-23's walker passes`)

Every other plan in the phase used the zero-padded `(01-0N)` form. The
`tdd.review-checkpoint` checker matches a zero-padded literal, so the unpadded
scopes are invisible to it.

This is GSD defect class #4003. The same workflow spec fixes it in two other
places — `safe_resume_gate` and the in-plan TDD gate both zero-strip *both* scope
components and match anchored (`^[a-z]+\((0*N)-(0*M)\):`), with the comment
"no padding rule in the commit protocol, so a padded literal grep hard-halts on
a correct unpadded RED commit". The end-of-phase checker did not receive that fix.

## Disposition

Waived by the user on 2026-09-08. The TDD discipline was followed; only the
commit-scope spelling deviated. No history was rewritten — the commits above are
the evidence, and they are reachable from `main`.

**Residual risk:** the gate will keep reporting this violation for phase 01 on
every future run until either `tdd.review-checkpoint` adopts the #4003 anchored
zero-stripped match, or 01-05's commit scopes are rewritten. Re-reading this file
is the intended response, not a second investigation.
