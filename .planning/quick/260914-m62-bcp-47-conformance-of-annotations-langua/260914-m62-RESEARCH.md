# Quick 260914-m62: BCP-47 conformance of `annotations_language` - Research

**Researched:** 2026-09-14
**Domain:** IAMF v1.1.0 Mix Presentation OBU string semantics; RFC 5646 language-tag syntax
**Confidence:** HIGH (spec, references, fixtures all read or probed this session)
**Scope:** wlv deferred items **1** (BCP-47 conformance) and **4** (language equality parity). No CONTEXT.md exists yet; research runs before the user discussion. Questions the user owns are marked **DECIDE**.

## Summary

The spec requires the tag to conform to BCP 47, and neither pinned reference checks it. iamf-tools reads and writes the bytes as-is, and so does libiamf. Every committed fixture uses `"en-us"`, `"es-mx"`, or zero labels. Test models use `"en"`/`"en-us"`. So a well-formedness check at `validate()` + `build()` changes **no `semantic_sha256`** and breaks no existing test. A registry-validity check would change no hash either, but it brings a time-varying data table into a deterministic crate.

**Primary recommendation:** Add a hand-written RFC 5646 §2.1 ABNF **well-formedness** check with no new dependency. Report it as a `MixPresentation::validate()` finding at `Field("annotations_language")`. In `build()`, reject it with a new dedicated `ErrorKind` placed right after `DuplicateAnnotationsLanguage`. Never reject on parse. Keep ASCII-case-insensitive duplicate equality.

## Project Constraints (from CLAUDE.md)
- Linked graph stays `iamf` + `thiserror` (`cargo tree -e normal,no-proc-macro`). No language-tag crate may be added.
- Clippy denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap/expect/panic`. Use iterators, `.get()`, `checked_*`.
- Never silently normalise, and never reject on read what the references accept (`src/bits/reader.rs:163-166`: "validating them here would make this parser stricter on read than the reference is").
- Conformance rule (qk3 CONTEXT): "satisfy BOTH the spec and the pinned references … Where the IAMF spec is stricter … the spec wins". Record each disagreement in a `// ref:` / `DISAGREEMENT:` comment.
- Changing an existing `ErrorKind` breaks Parallax. Adding one is OK. `const _: () = assert!(size_of::<Error>() <= 32);` [VERIFIED: src/error.rs:313].
- A reference hash may only change for an `is_valid: false` fixture.

## Q1. Spec text

- `index.bs:1273` [VERIFIED: `git -C docs/iamf show v1.1.0:index.bs`]: "annotations_language specifies the language which both localized_presentation_annotations and localized_element_annotations are written in. It SHALL conform to [[!BCP-47]]. The same language SHALL NOT be duplicated in this array."
- `:1274`: "The i-th localized_presentation_annotations and localized_element_annotations SHALL be written in the language indicated by the i-th annotations_language".
- Bibliography `:185-190`: `"BCP-47": { "title": "BCP 47", "status": "Best Practice", "publisher": "IETF", "href": "https://www.rfc-editor.org/info/bcp47" }`. The reference is **undated** and normative (`!`). BCP 47 today = RFC 4647 (Matching) + RFC 5646 (Tags) [CITED: rfc-editor.org/info/bcp47].
- The spec does not say which RFC 5646 conformance class it means: "well-formed" (ABNF) or "valid" (registry). The text is silent. It gives no example, no case rule, and no length rule beyond the generic string type.
- String type `:2881`: "null-terminated … UTF-8 encoded as defined in [[!RFC-3629]] and whose length is limited to 128 bytes". The crate already enforces the 128-byte cap (`StringTooLong`/`StringNotTerminated`) and interior NUL.
- Contrast `:1529`: `content_language` tag value "SHALL conform to [[!ISO-639-2-Codes]]". `:1532` says it is a different concept from `annotations_language`.
- RFC 5646 [CITED: rfc-editor.org/rfc/rfc5646.txt]:
  - §2.2.9: "A tag is considered 'well-formed' if it conforms to the ABNF (Section 2.1)." "Valid" additionally requires every language/extlang/script/region/variant subtag to be in the IANA Language Subtag Registry, with no duplicate variants or singletons. §2.2.9 also says validity "depends on the date of the registry used".
  - Tags are US-ASCII only.
  - §2.1.1: tags "are to be treated as case insensitive"; "All subtags have a maximum length of eight characters."

## Q2. References at the pins

| Reference | Read | Write / encoder | BCP-47 check | Uniqueness |
|---|---|---|---|---|
| iamf-tools@848c6ff4 | `mix_presentation.cc:523-526` `rb.ReadString(annotations_language)` [VERIFIED] | `:478-479` `wb.WriteString(annotations_language)`. The proto is `repeated string annotations_language = 9;` (`cli/proto/mix_presentation.proto:237`) with no validation | **None.** `git grep -i bcp` hits only binary demo files | Exact bytes: `ValidateUnique(... "Annotation languages")` `:528-530`, write `:471-473` |
| libiamf@f06e919e | `IAMF_OBU.c:749` `bs_readString(&b, mixp->annotations_language[i], STRING_SIZE);` (`STRING_SIZE 128`, `IAMF_OBU.h:237`) [VERIFIED] | n/a (decoder) | None | None |

- The closest precedent in iamf-tools is `ValidateCompliesWithIso639_2` (`mix_presentation.cc:327-338`), used only for the `content_language` tag. It is shape-only: "Consider any any three character string valid. A stricter implementation could check it actually is present in the list of valid ISO-639-2 code." So the reference author chose a **syntax check over a registry check** for the analogous rule.
- Testdata [VERIFIED: `git grep` over `iamf/cli/testdata/*.textproto`]: 252× `["en-us"]`, 1× `["en-us", "es-mx"]` (`test_000060`, `is_valid: true`). Unit tests use `{"en-us", "en-gb"}`. `adm_to_user_metadata/.../mix_presentation_handler.cc:223` hardcodes `"en-us"`. All values are lowercase and well-formed. No reference vector exercises a malformed or mixed-case tag.
- Result: the spec is stricter than both references → DISAGREEMENT (spec wins) under the qk3 rule.

## Q3. Committed fixtures and hash impact

A scratch probe parsed every committed `.iamf`/`.bin` and fuzz corpus file with this crate's `parse_sequence` (`scratchpad/langprobe`, toolchain 1.85.0):

| Set | `annotations_language` values |
|---|---|
| `tests/fixtures/reference/test_*.iamf` (35 parsed) | all `["en-us"]`; `test_000060` `["en-us","es-mx"]`; `test_000119/120/122/130` also carry MP 68 with `[]` |
| `tests/fixtures/reference/iamf-tools/*.iamf`, `negative/tones_256samp_5p1_pcm.iamf`, `fuzz/corpus/parse_sequence/*` | `["en-us"]` |
| `tests/fixtures/golden/phase1_sample_identity.iamf` | `["en-us"]` |
| `test_000129.iamf`, codec `.bin` packets, `fuzz/corpus/obu_roundtrip/*` | not IA sequences (the parse error is expected; obu_roundtrip holds generator seeds, and `src/fuzzing.rs:149` hardcodes `b"en"`) |
| Rust-built models (tests/support, encoder/profile/streaming/parallax_contract tests) | `"en"`, `"en-us"`, `"en-US"`, `"es"`, and `"e\0n"` (`tests/encoder_builder.rs:1263`, already rejected; asserts only `is_err()`) |

- **(a) ABNF well-formedness:** nothing is flagged. **Predicted `semantic_sha256` change: none.** The golden bytes are unchanged, because the writer does not change.
- **(b) Registry validity:** nothing is flagged either, since `en`, `es`, `US`, `MX` are registered [ASSUMED]. Predicted hash change: none. The hash constraint therefore does not separate (a) from (b). Determinism does.
- `DuplicateAnnotationsLanguage` tests `["en","en"]` and `["en-US","en-us"]` are well-formed, so they keep their kind if the new check runs **after** the duplicate check.

## Q4. Strictness options

**Level** (DECIDE):

| Level | Effect | Cost | Benefit |
|---|---|---|---|
| none | status quo | spec SHALL unenforced | zero work |
| `validate()` finding only | reported for parsed and built models | **Not actually possible alone.** `build()` already turns any presentation finding into `InvalidDescriptorReference` at `Field("descriptors")` via `validate_findings(lowered.validate())?` [VERIFIED: src/encoder.rs:1256, :1424-1433] | — |
| **finding + dedicated `build()` kind (recommended)** | build rejects with a precise kind/location; validate reports every bad tag | 1 new `ErrorKind` (unit, size-safe), HANDOFF.md line | same pattern as `DuplicateAnnotationsLanguage` (`src/encoder.rs:1209-1224`, `src/obu/mix_presentation.rs:395-417`); Parallax gets a typed error |
| finding + generic build rejection | no new kind | Parallax sees the vague `InvalidDescriptorReference`/`descriptors` | smallest diff |
| parse rejection | reject on read | breaks PARSE-04 byte fidelity and rejects files both references accept | **no** |

**Scope** (DECIDE):

| Scope | Implementation | Determinism / licence | Verdict |
|---|---|---|---|
| minimal: 1-8 ASCII alnum segments joined by `-` | ~5 lines | fine | accepts `1234567-x`, `en-a` (singleton with no subtag), `abcd5` as language; not "conforms to BCP-47" |
| **RFC 5646 §2.1 ABNF well-formed (recommended)** | ~60-line hand-written state machine + 17-entry irregular list; no dependency | fixed forever (RFC is frozen); no data licence | matches the RFC's "well-formed" class and iamf-tools' shape-only precedent |
| + no duplicate variant/singleton (part of "valid", no registry) | +10 lines | fine | optional add-on; low value |
| registry-valid (IANA Language Subtag Registry) | embed ~9k-record table [ASSUMED size], or a crate (forbidden by graph rule) | validity "depends on the date of the registry used". Output of `validate()` would drift with every table refresh, so a pinned table goes stale and a live one is non-deterministic; ISO-derived data provenance needs review | **reject** |

ABNF facts the parser must honour [CITED: RFC 5646 §2.1]:
- Top level: `Language-Tag = langtag / privateuse / grandfathered`.
- **Irregular** grandfathered tags do not match `langtag`, so they need a literal, case-insensitive list: `en-GB-oed, i-ami, i-bnn, i-default, i-enochian, i-hak, i-klingon, i-lux, i-mingo, i-navajo, i-pwn, i-tao, i-tay, i-tsu, sgn-BE-FR, sgn-BE-NL, sgn-CH-DE`.
- **Regular** grandfathered tags (`art-lojban, cel-gaulish, no-bok, no-nyn, zh-guoyu, zh-hakka, zh-min, zh-min-nan, zh-xiang`) already match `langtag`, so no list is needed.
- Private use: `privateuse = "x" 1*("-" (1*8alphanum))`, so `x-private` is a full tag.
- `singleton` excludes `x`/`X`.
- Non-ASCII and non-UTF-8 bytes are rejected automatically (ASCII-only alphabet). Separate validation of the UTF-8 requirement at `:2881` for the *annotation* strings is out of scope (a deferred-item candidate).
- Length: the RFC sets no overall cap; the 127-byte wire cap already applies.

## Q5. Equality (wlv item 4) (DECIDE)

| Option | Behaviour | Hash impact | Verdict |
|---|---|---|---|
| **keep ASCII-case-insensitive (recommended)** | `en-US` == `en-us` | none | RFC 5646 §2.1.1 mandates case-insensitivity; already shipped and documented in HANDOFF.md:42-43 |
| exact bytes (iamf-tools parity) | `en-US` ≠ `en-us` | none (no fixture has duplicates) | weaker than the spec, and it breaks `build_rejects_annotations_languages_differing_only_in_ascii_case` |
| full canonicalization (§4.5: extlang/redundant/grandfathered → preferred) | `zh-yue` == `yue` | none | needs the registry → same rejection as registry scope |

`en` vs `en-US` are distinct tags under any option. Whether they are "the same language" is a judgement the spec does not define. Recommendation: do not treat them as duplicates.

## Q6. Parallax impact

- HANDOFF.md:38-44 lists the `build()` rejections. A new line would say "or whose `annotations_language` is not a well-formed BCP-47 tag (`<NewKind>`)".
- The Parallax contract fixture uses `annotations_language: vec![b"en".to_vec()]` (`tests/support/parallax_contract.rs:236`, `tests/parallax_contract.rs:269`), which is well-formed, so the delivery fixture is unaffected.
- Parallax checkout at `/Volumes/lab/talea/Parallax` (`9685d935`, 2026-09-14): `git ls-files` shows no tracked `.rs`/`.md` file mentioning `annotations_language`, and no `Cargo.toml` naming `iamf`. Parallax does not author the field yet.
- New kind: a unit variant keeps `size_of::<Error>() <= 32`. It needs a naming decision. Suggested: `AnnotationsLanguageNotWellFormed`, which follows `StringNotTerminated` naming and avoids RFC-loaded "Invalid". This is additive, so non-breaking.

## Q7. TDD sketch (recommended option)

**Placement:** `pub(crate) fn is_well_formed_language_tag(tag: &[u8]) -> bool` in a new `src/model/language_tag.rs`. It has no `read_`/`write_` prefix, so `tests/citations.rs` does not require a citation, but add `// ref: RFC 5646 §2.1 (BCP 47)`. It must not import `obu`. The lint-safe shape is `split(|b| *b == b'-')` + `peekable()`, run through greedy phases:
1. irregular list (`eq_ignore_ascii_case`)
2. `x` → private use
3. language `2..=8` alpha
4. up to 3 extlang (3 alpha, only if language len ≤ 3)
5. script (4 alpha)
6. region (2 alpha | 3 digit)
7. variants (5-8 alnum | digit + 3 alnum)
8. extensions (singleton ≠ x, then ≥1 of 2-8 alnum)
9. optional `x` + ≥1 of 1-8 alnum
10. end of input

Every alternative is length- or class-disjoint, so greedy matching is unambiguous. Use `(2..=8).contains(&len)` and `for _ in 0..3`, so no arithmetic is needed.

**Red tests (write first, confirm fail via `tools/red-evidence.sh`):**
1. Unit (module `#[cfg(test)]`) accept: `en`, `en-US`, `EN-us`, `zh-Hant-TW`, `zh-yue-HK`, `sr-Latn-RS`, `es-419`, `de-CH-1901`, `sl-rozaj-biske`, `en-a-bbb-x-ccc`, `x-private`, `X-Priv-1`, `i-klingon`, `I-KLINGON`, `en-GB-oed`, `zh-min-nan`, `qaa` (a well-formed but unregistered language, which proves no registry is used).
2. Unit reject: `""`, `en_US`, `-en`, `en-`, `en--US`, `english-language-too-long-subtag` (a subtag of more than 8 characters), `e`, `x`, `x-`, `en-a` (singleton without a subtag), `en-x` (x without a subtag), `en-US-US` (a second region), `en-Latn-zh` (extlang after the script), `abcdefghi`, `en US`, `"en\u{e9}"` (non-ASCII UTF-8), `[0x65, 0x6e, 0xff]` (non-UTF-8), `e\0n`.
3. `tests/encoder_builder.rs`:
   - `build_rejects_malformed_annotations_language` (`en_US`) → new kind + `Field("annotations_language")`
   - `build_accepts_well_formed_annotations_languages` (`["en","zh-Hant-TW","x-private","i-klingon"]`)
   - ordering: `["en_US","en_US"]` → `DuplicateAnnotationsLanguage` (the duplicate check wins), and the malformed tag beats the `InvalidDescriptorReference` generic finding
4. `tests/sequence_parse.rs`: a parsed MP with `["en_US","en"]` yields exactly one finding at `Field("annotations_language")` with a fixed message, e.g. `mix presentation 42 lists annotations_language "en_US", which is not a well-formed BCP-47 tag (IAMF v1.1.0 index.bs:1273, RFC 5646 2.1)`. It still round-trips byte-identically through `write_parsed_sequence`, which proves there is no parse rejection and no normalisation.
5. Controls: `cargo test --locked --test parse_reference` stays green with **no expectation edits**, and so do `--test golden` and `--test parallax_contract`.
6. Guard-fires proof: temporarily make the predicate return `true`. Tests 2-4 go red; record that with `tools/red-evidence.sh`, then restore.

**Gates:** `cargo clippy --locked --all-targets -- -D warnings`, `bash tools/check-float-escape-census.sh` (still 1), `cargo tree -e normal,no-proc-macro` (iamf + thiserror), `cargo test --locked`.

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|---|---|---|
| A1 | "SHALL conform to BCP-47" is satisfied by RFC 5646 *well-formedness* (the spec names no conformance class) | Q4 | Low. A stricter reading needs a registry, which is rejected for determinism; record it as a DISAGREEMENT note |
| A2 | `en`, `es`, `US`, `MX` are registered subtags | Q3 | None for the recommendation (a well-formed check needs no registry) |
| A3 | IANA registry is ~9k records | Q4 | Only affects the rejected option's cost estimate |
| A4 | `en` vs `en-US` are not "the same language" for the duplicate rule | Q5 | Low; no fixture or test has such a pair |

## Open Questions (for the discussion)
1. **Level:** dedicated `build()` kind + finding (recommended), or finding with generic build rejection?
2. **Scope:** ABNF well-formed (recommended). Should the cheap no-duplicate-variant/singleton rule be added?
3. **Equality:** keep ASCII-case-insensitive (recommended)?
4. **Kind name:** `AnnotationsLanguageNotWellFormed`?
5. Should the predicate be `pub` so Parallax can pre-check user input? Recommendation: `pub(crate)` now.

## Environment Availability
| Dependency | Available | Version |
|---|---|---|
| rustup toolchain 1.85.0 | ✓ (probe built with `cargo +1.85.0`) | 1.85.0 |
| iamf-tools checkout at `848c6ff4` | ✓ `docs/iamf-tools` | commit present |
| libiamf at `f06e919e` | ✓ `.reference/libiamf` | commit present |
| spec `v1.1.0` | ✓ `docs/iamf` | tag present |

## Sources
- `git -C docs/iamf show v1.1.0:index.bs` lines 185-190, 1273-1274, 1529-1532, 2881
- `docs/iamf-tools@848c6ff4`: `iamf/obu/mix_presentation.cc` 327-338, 471-479, 523-530; `iamf/cli/proto/mix_presentation.proto` 225-240; `iamf/common/read_bit_buffer.cc` 186-203; testdata textprotos
- `.reference/libiamf@f06e919e`: `code/src/iamf_dec/IAMF_OBU.c` 723-772, `IAMF_OBU.h` 237-249
- RFC 5646 (§2.1 ABNF, §2.1.1, §2.2.9, §4.5) and rfc-editor.org/info/bcp47
- Repo: `src/obu/mix_presentation.rs:395-417, 686-704`, `src/encoder.rs:1209-1224, 1256, 1424-1433`, `src/error.rs:171-178, 313`, `src/bits/reader.rs:159-166`, wlv deferred items and RESEARCH, qk3 CONTEXT, HANDOFF.md:38-44
- Scratch probe: `/private/tmp/claude-501/-Volumes-lab-talea-iamf-rs/68cc7752-367c-4fd6-a3b5-6ee164fbc0ab/scratchpad/langprobe`
