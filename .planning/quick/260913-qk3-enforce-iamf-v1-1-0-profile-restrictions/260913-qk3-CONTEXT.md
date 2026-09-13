# Quick Task 260913-qk3: enforce IAMF v1.1.0 profile restrictions in build and parse-side validation - Context

**Gathered:** 2026-09-13
**Status:** Ready for research

<domain>
## Task Boundary

Resolve the deferred items 1-5 from `.planning/quick/260913-p28-assign-base-enhanced-profile-to-expanded/260913-p28-deferred-items.md`:

1. `build()` does not reject a Mix Presentation with `num_sub_mixes > 1`. This gap is unverified; confirm it with a failing test first.
2. `build()` does not reject headphones rendering modes 2 and 3 (reserved). Also unverified; confirm with a failing test first.
3. The Simple and Base element and channel limits apply across the whole IA sequence, not per Mix Presentation.
4. The Base profile limits: at most one scene-based Audio Element, and at most one Audio Element with `num_layers > 1`.
5. Add a parse-side `DescriptorSet::validate()` finding when no Mix Presentation complies with the header's `primary_profile`.

Item 6 (Binaural rendering abort in `decoder_main`) is NOT part of this task.

</domain>

<decisions>
## Implementation Decisions

### Directive refinement (user, 2026-09-13)
- The user asked for items 1-2 with research, and said: "3 wundert mich, wir sollten uns an die spec
  halten" ("3 surprises me, we should follow the spec"), "4 bitte fixen" ("4 please fix"),
  "5 bitte fixen" ("5 please fix").
- **New conformance rule: satisfy BOTH the spec and the pinned references.**
  - Where the IAMF spec is stricter than `iamf-tools@v2.1.0` or `libiamf@v1.1.0`, the spec wins
    (items 3 and 4).
  - Where a pinned reference is stricter than the spec (e.g. `parameter_rate` must equal the sample
    rate), the reference rule stays, because the Core Value is that `libiamf` accepts the file.
- This supersedes the earlier "references win on disagreement" tie-break for every rule where the
  spec is stricter. Record each case where spec and reference disagree in a `// ref:` comment.
- **Spec version:** `SPEC_VERSION = "1.1.0"`, from `git -C docs/iamf show v1.1.0:index.bs`.
  - The p28 research cited v1.0.0-errata `index.bs:1852-1869` for items 3 and 4. Research must
    establish what **v1.1.0** says for the Simple, Base and Base-Enhanced profiles.
  - That includes whether v1.1.0 incorporates the errata limits by reference, and the exact
    wording: "unique Audio Element OBU" across the IA sequence, channel counts, scene-based and
    scalable limits.
  - v1.1.0 is binding; errata text binds only as far as v1.1.0 adopts it.

### LOCKED after research (orchestrator, 2026-09-13; supersedes defaults below where they conflict)
- **Scopes follow the spec text exactly** (see RESEARCH A):
  - **Simple / Base unique-element limits:** 1 and 2 respectively, counted across the whole sequence
    (errata:1853, :1866).
  - **Simple / Base channel limits:** 16 and 18, counted per Mix Presentation (errata:1879,
    v1.1.0:1927).
  - **Base-Enhanced:** at most 28 elements per Mix Presentation (v1.1.0:1951), and at most 28
    channels in total across the sequence (v1.1.0:1953).
- **A1 accepted:** Base's "at most one scene-based" and "at most one channel-based element with
  `num_layers > 1` at any one time" both apply across the whole sequence.
  - The rule sits under "at most two unique Audio Element OBUs" in the IA Sequence, and the listed
    allowed combinations are pairs.
  - Neither reference enforces it; the spec is stricter and wins.
- **Items 1 and 2:** reject in `build()` with `SubMixCountNotOne` (this also rejects 0 sub-mixes) and
  `ReservedHeadphonesRenderingMode`. Both checks run before `validate_findings(presentation.validate())`,
  as research placed them.
- **Items 3 and 4:** split `select_minimum_profile` into a per-presentation check and a
  sequence-wide check over slices, so the same logic serves the builder and item 5. Invert or rename
  the two tests research identified to the spec reading.
- **Item 5 — IN SCOPE for both validators; do not defer the parsed side:**
  - **`DescriptorSet::validate()`:** add the compliance finding.
  - **`ParsedSequence::validate()`:** add it too, because that is the true parse-side validator.
    Count unique elements after filtering out `obu_redundant_copy` duplicates.
  - **Both validators:** report no Mix Presentation complying with `primary_profile` /
    `additional_profile` (per-presentation rules), and also any sequence-wide limit the header's
    profile is exceeded by (research open question 2: yes).
  - **`test_000124.iamf`:** its `semantic_sha256` in `tests/support/reference_expectations.rs` may
    change. That is allowed and explained: the iamf-tools testdata marks this file
    `is_valid: false` (`test_000124.textproto:111`), and its only Mix Presentation has 2 sub-mixes
    under a Base header. Record the old and new hash and the reason in the commit message. If any
    **other** reference expectation changes, stop and report instead of updating it.
- **Q1 (parse-side sequence-scope finding):** A
  - Both validators report sequence-wide limit violations. The `semantic_sha256` of
    `test_000119`, `test_000120`, `test_000122`, `test_000124` and `test_000130` may change.
  - This relaxes the earlier rule that only `test_000124` may change.
  - Orchestrator verified that all five are `is_valid: false` in `iamf-tools@848c6ff4`
    `iamf/cli/testdata`.
  - Record each old and new hash with its reason in the commit message.
  - Any OTHER reference expectation change: stop and report.
- **Deferred (write `260913-qk3-deferred-items.md`):**
  - `build()` accepts zero Mix Presentations.
  - `build()` accepts the same element twice in one sub-mix.
  - Separate sub-mix and headphones findings in `MixPresentation::validate()`.
  - Item 6, the Binaural rendering abort.

### Items 3 and 4 are profile selection, not rejection (default)
- `select_minimum_profile` must raise the profile when the Simple or Base limits are exceeded across
  the whole sequence, or when the Base scene-based / multi-layer limits are exceeded.
- It raises the profile rather than rejecting, just as the expanded-layout floor does. A declaration
  is rejected only when no profile allows it; for Base-Enhanced that means the existing 28-element
  and 28-channel ceilings, scoped exactly as v1.1.0 states them.
- `tests/encoder_builder.rs::unrelated_presentations_do_not_sum_their_element_or_channel_limits`
  encodes the old per-presentation reading. Invert or rename it to the spec reading.

### Items 1 and 2 are rejections at build()
- They follow the existing `build()` validation and error conventions: payload-free `ErrorKind`
  variants, the 32-byte `Error` assertion, and `Location::Field`.
- If research finds that the spec allows more than one sub-mix in some profile, apply the spec plus
  references rule above.

### Item 5 is a parse-side finding, not a rejection
- Add a finding to `DescriptorSet::validate()`.
- The low-level writers (`SequenceWriter`, `write_sequence`, `write_parsed_sequence`) must keep
  round-tripping foreign files byte-exactly.
- Add no write-time rejection for foreign files.

### Claude's Discretion
- Finding and error names, and test layout.
- Whether items 1 and 2 also get parse-side findings, alongside item 5.

</decisions>

<specifics>
## Specific Ideas

- **No golden changes:** Golden fixtures and `DIFF-LEDGER.md` must stay byte-identical. The golden
  is a single 5.1 Simple element, which should be unaffected.
- **Gates:** CONF-06 (`decoder_main`) and CONF-05 must still pass. For CONF-05, use
  `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec`.
- **Leave alone:** foreign working-tree changes (`.claude/CLAUDE.md`, `.gitignore`,
  `.planning/PROJECT.md`, `.planning/codebase/CONVENTIONS.md`, `README.md`, `REFERENCES.md`,
  `.claude/hooks/`, the untracked `CLAUDE.md` files) and the `docs/` clones.

</specifics>

<canonical_refs>
## Canonical References

- IAMF spec `v1.1.0` `index.bs` "Profiles" (~1896-1990), and `v1.0.0-errata` `index.bs:1852-1869`
  where v1.1.0 refers to it.
- `iamf-tools@848c6ff4 iamf/cli/profile_filter.cc` (sub-mix count `:220-238`, headphones mode
  `:240-271`, element and channel limits `:282-311`).
- `libiamf@f06e919e code/src/iamf_dec/IAMF_decoder.c:1284-1337`, `:1402-1410`.
- `.planning/quick/260913-p28-assign-base-enhanced-profile-to-expanded/260913-p28-RESEARCH.md` and `-deferred-items.md`.

</canonical_refs>
