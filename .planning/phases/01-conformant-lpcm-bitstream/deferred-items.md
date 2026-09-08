# Deferred items — phase 01

Out-of-scope discoveries logged rather than fixed, per the executor's scope
boundary. Each names the plan that found it.

| Found by | Item | Why deferred |
|---|---|---|
| 01-03 | `cargo fmt --check` reports three diffs in `tests/error_shape.rs` (line 180) and `tests/reference_manifest.rs` (lines 101, 176), both authored by plans 01-01 and 01-02. | Pre-existing, in files this plan does not own. `cargo fmt` was run and its edits to those two files were reverted deliberately. CI does not currently run `cargo fmt --check`, so this is not a red gate — but if a later plan adds one, it will need a formatting-only commit over those two files first. |
