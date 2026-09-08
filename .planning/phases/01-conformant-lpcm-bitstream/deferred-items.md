# Deferred items — phase 01

Out-of-scope discoveries logged rather than fixed, per the executor's scope
boundary. Each names the plan that found it.

| Found by | Item | Why deferred |
|---|---|---|
| 01-03 | `cargo fmt --check` reports three diffs in `tests/error_shape.rs` (line 180) and `tests/reference_manifest.rs` (lines 101, 176), both authored by plans 01-01 and 01-02. | Pre-existing, in files this plan does not own. `cargo fmt` was run and its edits to those two files were reverted deliberately. CI does not currently run `cargo fmt --check`, so this is not a red gate — but if a later plan adds one, it will need a formatting-only commit over those two files first. |
| 01-07 | `cargo fmt --check` reports diffs in eleven more files this plan does not own: `src/model/mod.rs`, `src/obu/{audio_element,mix_presentation,mod,parameter_block}.rs`, `src/packing.rs`, `tests/{descriptors,packing,temporal}.rs`, on top of 01-03's two. | Pre-existing drift from plans 01-04 through 01-06, which formatted their own files but not the ones they edited in passing. This plan formatted only its own new/changed files (`rustfmt --edition 2024` on each) rather than running repo-wide `cargo fmt`, which would have produced a large unrelated diff inside a feature commit. A single formatting-only commit over the whole tree, before any `cargo fmt --check` CI gate lands, is the clean fix. |
