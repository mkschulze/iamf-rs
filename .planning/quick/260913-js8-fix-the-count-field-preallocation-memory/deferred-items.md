# Deferred items — quick 260913-js8

## Pre-existing conformance failure (out of scope)

- Test: `tests/conformance.rs` `parallax_delivery_fixture_uses_the_offline_safe_reference_gates` (CONF-06)
- Symptom: `iamf-tools` `decoder_main` aborts with `INVALID_ARGUMENT: Audio elements in a submix must have the same number of samples per frame.` on the Parallax delivery fixture from `parallax_contract::build_delivery()`.
- Reproduced at baseline commit `3db396c` (before any change in this task) in a scratch worktree, so it is not caused by the bounded-reservation change. Only runs when the `iamf-tools` container is available locally; skipped otherwise.
- Suggested follow-up: `/gsd-debug` on the Parallax delivery fixture's frame sizes across its Audio Elements.
