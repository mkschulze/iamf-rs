---
schema_version: 1
open_count: 5
waived_count: 0
fixed_count: 0
total_count: 5
last_updated: 2026-09-08T03:37:28.013Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 01 | unrun-verify | .github/workflows/ci.yml |  | The four-target CI matrix (GUARD-09, D-16) has never executed — no pushed branch, no CI run. Structurally valid and 4 matrix entries, but 'green on four targets' is unproven. | open |  | 2026-09-08T02:04:27.373Z |  |
| 2 | 01 | unrun-verify | tools/iamf-tools.Dockerfile |  | The digest-pinned iamf-tools image has never been built or run: the Docker daemon was not running on the authoring machine (Cannot connect to unix:///Users/cell/.docker/run/docker.sock) and bazel/bazelisk are absent by design. Assumption A2 (the apt package set) is therefore still unverified, and the built image's own digest is NOT recorded in REFERENCES.md as D-13 requires. First run of .github/workflows/reference.yml is where this resolves. | open |  | 2026-09-08T02:43:42.662Z |  |
| 3 | 01 | unrun-verify | CONFORMANCE-GATE.md |  | Experiment 1 (how iamf-tools' decoder_main signals a parse failure, research open question 1) did NOT run — no container runtime. CONF-06 therefore has a route (decoder_main's parse path) but no asserted observable; the reference.yml CONF-06 step asserts only that an output WAV is non-empty, which is the weakest defensible claim. | open |  | 2026-09-08T02:43:42.835Z |  |
| 4 | 01 | unrun-verify | CONFORMANCE-GATE.md |  | Experiment 2 (does encoder_main accept a Codec Config no Audio Element references, research assumption A3 / D-18's research item) did NOT run — no container runtime. D-18's two-Codec-Config Phase 1 fixture design is unconfirmed on the encoder side; the input textproto is prepared at tools/experiments/two-codec-configs.textproto and the fallbacks are recorded. | open |  | 2026-09-08T02:43:43.010Z |  |
| 5 | 01 | deviation | src/bits/mod.rs |  | BITS-01's REQUIREMENTS.md text still says BitReader<'a>/BitWriter 'wrapping bitstream-io'. D-01 amended that to a hand-rolled BitCursor with bitstream-io demoted to a dev-only differential oracle, and 01-03 implemented the amendment. The requirement text was NOT amended; the divergence is recorded in the module doc comment only. | open |  | 2026-09-08T03:37:28.013Z |  |

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
  },
  {
    "id": 2,
    "kind": "unrun-verify",
    "phase": "01",
    "file": "tools/iamf-tools.Dockerfile",
    "line": null,
    "description": "The digest-pinned iamf-tools image has never been built or run: the Docker daemon was not running on the authoring machine (Cannot connect to unix:///Users/cell/.docker/run/docker.sock) and bazel/bazelisk are absent by design. Assumption A2 (the apt package set) is therefore still unverified, and the built image's own digest is NOT recorded in REFERENCES.md as D-13 requires. First run of .github/workflows/reference.yml is where this resolves.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-09-08T02:43:42.662Z",
    "resolved_at": null
  },
  {
    "id": 3,
    "kind": "unrun-verify",
    "phase": "01",
    "file": "CONFORMANCE-GATE.md",
    "line": null,
    "description": "Experiment 1 (how iamf-tools' decoder_main signals a parse failure, research open question 1) did NOT run — no container runtime. CONF-06 therefore has a route (decoder_main's parse path) but no asserted observable; the reference.yml CONF-06 step asserts only that an output WAV is non-empty, which is the weakest defensible claim.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-09-08T02:43:42.835Z",
    "resolved_at": null
  },
  {
    "id": 4,
    "kind": "unrun-verify",
    "phase": "01",
    "file": "CONFORMANCE-GATE.md",
    "line": null,
    "description": "Experiment 2 (does encoder_main accept a Codec Config no Audio Element references, research assumption A3 / D-18's research item) did NOT run — no container runtime. D-18's two-Codec-Config Phase 1 fixture design is unconfirmed on the encoder side; the input textproto is prepared at tools/experiments/two-codec-configs.textproto and the fallbacks are recorded.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-09-08T02:43:43.010Z",
    "resolved_at": null
  },
  {
    "id": 5,
    "kind": "deviation",
    "phase": "01",
    "file": "src/bits/mod.rs",
    "line": null,
    "description": "BITS-01's REQUIREMENTS.md text still says BitReader<'a>/BitWriter 'wrapping bitstream-io'. D-01 amended that to a hand-rolled BitCursor with bitstream-io demoted to a dev-only differential oracle, and 01-03 implemented the amendment. The requirement text was NOT amended; the divergence is recorded in the module doc comment only.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-09-08T03:37:28.013Z",
    "resolved_at": null
  }
]
````
