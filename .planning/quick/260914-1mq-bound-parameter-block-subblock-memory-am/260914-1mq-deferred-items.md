# 260914-1mq deferred items

1. **Linear parse-memory residuals and a per-parse memory budget.** After this task the remaining
   linear classes are Raw extension subblocks (1 wire byte -> about 40× exact / 64× with `Vec` growth
   slack), Mix Gain Step subblocks (3 bytes -> about 13×/16×) and `read_strings`
   (`src/obu/mix_presentation.rs`, about 24-32×). Each is bounded by the 2 MiB OBU cap, but the review's
   38 MB -> 1.94 GB RSS scenario stays reachable via `param_definition_type` >= 3 blocks. Option (d) from
   research Q3 is a per-parse budget such as `parse_sequence_with_limits`. That is an API change for
   Parallax and needs a user decision.

2. **Optional `validate()` finding for the full iamf-tools Demixing/Recon Gain definition rule**
   (`param_definition_mode` = 0, `constant_subblock_duration` = `duration` != 0;
   `iamf/obu/param_definitions.cc:37-60`). Churn: the mode-1 models in `src/fuzzing.rs:103-110`,
   `tests/support/sequence_cases.rs:119-123` and `tests/temporal.rs` (around the Demixing and Recon Gain
   tests). Research question, verbatim:
   - EN: "Should the full iamf-tools rule for Demixing/Recon Gain definitions (`param_definition_mode` = 0, `constant_subblock_duration` = `duration` ≠ 0) become a `validate()` finding? That means test churn in the fuzz model and `sequence_cases`, which use mode 1."
   - DE: "Soll die vollständige iamf-tools-Regel für Demixing-/Recon-Gain-Definitionen (Modus 0, `constant_subblock_duration` = `duration` ≠ 0) zusätzlich als `validate()`-Finding gemeldet werden? Diese Aufgabe erzwingt nur die Folge daraus (genau 1 Subblock). Option A: jetzt nicht (empfohlen, eigener Quick-Task). Option B: als Finding im selben Task."

   260914-1mq took Option A, so Option B now means a follow-up quick task. A parse-side check (rather than
   a finding) would also need the reference-hash constraint checked first.

3. **Assumption A1 not run against a binary.** That iamf-tools and libiamf accept a single all-absent
   Recon Gain block comes from reading the pinned source; no reference binary decoded such a file. Low
   risk: the change only turns an error into Ok for a count of 1.
