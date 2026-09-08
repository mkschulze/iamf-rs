---
schema_version: 1
open_count: 1
waived_count: 0
fixed_count: 0
total_count: 1
last_updated: 2026-09-08T02:04:27.373Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 01 | unrun-verify | .github/workflows/ci.yml |  | The four-target CI matrix (GUARD-09, D-16) has never executed — no pushed branch, no CI run. Structurally valid and 4 matrix entries, but 'green on four targets' is unproven. | open |  | 2026-09-08T02:04:27.373Z |  |

````json
[
  {
    "id": 1,
    "kind": "unrun-verify",
    "phase": "01",
    "file": ".github/workflows/ci.yml",
    "line": null,
    "description": "The four-target CI matrix (GUARD-09, D-16) has never executed — no pushed branch, no CI run. Structurally valid and 4 matrix entries, but 'green on four targets' is unproven.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-09-08T02:04:27.373Z",
    "resolved_at": null
  }
]
````
