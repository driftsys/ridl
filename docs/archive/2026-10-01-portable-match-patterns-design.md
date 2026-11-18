# Portable `match` patterns — one meaning in the checker and every output

Status: design note for driftsys/ridl#597, written 2026-10-01 against `main` at
50fe073 (release 0.5.0). Sebastien chose approach A (§9) on 2026-10-01. Nothing
here is implemented. It graduates into an amendment of the typl reference §2.7
and §16, and is archived with its plan when the story lands.

## 1. The problem

typl states a `match` pattern in ECMA-262 syntax. Three engines read the same
pattern text:

- the checker, `ridl-sem`, with `regress` 0.11.1 and no flags — it decides
  TYPL-106 and tests a declared init value against the pattern (TYPL-109);
- the Rust output, with `regex` 1.13.1 and its defaults (Unicode mode on);
- the Kotlin output (driftsys/ridlc-gen-kotlin), with `java.util.regex`.

TYPL-220 (#437) guarantees that the Rust output can compile a pattern. It does
not guarantee that the engines match the same strings, and for several
constructs they do not. A probe on 2026-10-01 measured, with the versions in
`Cargo.lock`:

| Construct                       | `regress`, no `u` flag                       | `regex`, defaults                                            |
| ------------------------------- | -------------------------------------------- | ------------------------------------------------------------ |
| `\d`                            | ASCII digits (10)                            | Unicode digits (760)                                         |
| `\w`                            | ASCII word characters (63)                   | Unicode word characters (144,667)                            |
| `\b`                            | ASCII word boundary                          | Unicode word boundary: `^a\b` refuses `"aé"`                 |
| `\s`                            | the ECMA-262 whitespace set (25 scalars)     | Unicode `White_Space`: adds U+0085, drops U+FEFF             |
| `.`                             | any scalar except `\n`, `\r`, U+2028, U+2029 | any scalar except `\n`                                       |
| `\p{L}`                         | the text `p{L}`                              | a Unicode letter                                             |
| `\A`, `\z`, `\a`, `\<`, `[[a]]` | literal characters                           | anchors, the bell character, a word boundary, a nested class |
| `[a&&b]`, `[a~~b]`              | literal characters                           | class intersection, class symmetric difference               |

The consequence that matters most: `ridl.std.Date` is `/^\d{4}-\d{2}-\d{2}$/`,
so the Rust output accepts a date written in Arabic-Indic digits that the
specification refuses.

## 2. The decision in one paragraph

A typl pattern means what ECMA-262 says it means **with the `u` flag**. The
checker refuses the few constructs whose meaning no output can reproduce with
the same text (new error TYPL-221). The lowering then rewrites the remaining
pattern into a **portable form**, in which every construct that the engines read
differently is spelled out as an explicit class. The IR carries the portable
form, so the Rust backend emits it unchanged and the Kotlin plugin receives it
without porting any translation code.

## 3. Semantics: ECMA-262 with the `u` flag

typl gains no flag syntax. The `u` flag is always on, and the reference says so.
`ridl-sem` constructs `regress` with `Flags { unicode: true, .. }`.

Under the `u` flag, ECMA-262 itself refuses `\A`, `\z`, `\a`, `\<`, `\e`, a lone
`]`, `{` or `}`, `[[a]]`, and `\-` outside a class. Each becomes TYPL-106, so
the second half of the divergence list in the #597 comment needs no new rule.

Measured effect on existing text: every one of the 22 valid pattern literals in
`ridl_std.typl`, the test fixtures, the corpus and the book compiles the same
way with and without the flag.

`\d`, `\w` and `\b` stay ASCII under the `u` flag (ECMA-262 widens `\w` only
when `u` and `i` are both set, and typl has no `i`). That is the meaning the
outputs must reproduce.

## 4. The portable subset — TYPL-221

TYPL-221 (error) refuses a pattern, in a `match` or in a regex constant, that
uses one of these constructs:

| Construct                         | Why it is refused                                                                                                                        |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `\p{..}`, `\P{..}`                | each engine carries its own Unicode version: `\p{L}` differs on 4,644 scalars between `regress` and `regex`, and Java is a third version |
| `\b`, `\B`                        | ECMA-262 has no explicit spelling of an ASCII word boundary that `regex` and Java also read as ASCII                                     |
| `&&`, `~~` or `--` inside a class | class set operators in `regex` and Java, literal characters in ECMA-262                                                                  |
| `\D`, `\W`, `\S` inside a class   | the portable form of a negated class escape inside a class needs a nested class, which ECMA-262 `u` mode does not have                   |

Checking order: TYPL-106 (not ECMA-262 `u` syntax), then TYPL-221 (outside the
portable subset), then TYPL-220 (the `regex` crate cannot compile the portable
form). One diagnostic per pattern.

Refusing `\p{..}` is the reversible choice: accepting it later, with a stated
Unicode version, breaks no existing source. No pattern in the corpus uses any of
the four rows.

## 5. The portable form

The lowering rewrites the pattern body (the text between the `/` delimiters)
with this table. Everything not listed is copied unchanged.

| ECMA-262 `u` construct | Portable form, outside a class | Inside a class     |
| ---------------------- | ------------------------------ | ------------------ |
| `\d`                   | `[0-9]`                        | `0-9`              |
| `\D`                   | `[^0-9]`                       | refused (TYPL-221) |
| `\w`                   | `[0-9A-Za-z_]`                 | `0-9A-Za-z_`       |
| `\W`                   | `[^0-9A-Za-z_]`                | refused (TYPL-221) |
| `\s`                   | `[\t\n\u000B\f\r    -     　﻿]` | the same members   |
| `\S`                   | the `\s` class, negated        | refused (TYPL-221) |
| `.`                    | `[^\n\r  ]`                    | not applicable     |

Rules for the spelling:

- A code point is written `\uXXXX` with four hex digits, which ECMA-262, the
  `regex` crate and `java.util.regex` all read the same way. `\v` is not used:
  in Java it is the vertical-whitespace class, not U+000B.
- The probe measured the `\s` class against `regress`'s `\s` over every Unicode
  scalar: 0 differences.
- The portable form is itself a valid ECMA-262 `u` pattern with the same meaning
  as the author's text. The tests assert both (§8).

The rewrite needs a small tokenizer that knows whether it is inside a class and
skips escaped characters. It does not need a full regex parser, because
`regress` has already accepted the text.

## 6. Where the code lives

- **`ridl-sem`** owns the subset check and the rewrite (a `pattern` module next
  to `validate_regex` in `check.rs`). TYPL-109 tests an init value against the
  portable form with `regress`. TYPL-220 compiles the portable form with
  `regex`. As a side effect, the size-limit case in §2.7 (`^\w{1,256}$`)
  disappears, because `[0-9A-Za-z_]` is a small class.
- **The IR** (`Constraint.pattern`, and the text of a regex constant) carries
  the portable form. The field documentation changes from "the pattern without
  its delimiters" to "the portable form of the pattern (typl §2.7)". The IR
  schema does not change: the field is still a string.
- **`ridl-backend-rust`** emits the IR text unchanged, as it does today. Only
  its comments change.
- **The Kotlin plugin** receives the portable form through the IR. One
  divergence remains that the portable form cannot remove: in Java, `$` without
  `MULTILINE` also matches before a final line terminator, so `^abc$` accepts
  `"abc\n"`. The Kotlin plugin must emit `$` as `\z`. This goes in the heads-up
  issue (§10), not in typl.
- **`ridl-fmt`** and the source text are unaffected. The author's spelling stays
  in the `.typl` file. Only the compiled artifacts change.

## 7. Compatibility

This is a breaking change, marked `!`, with its own changelog entry:

- **Rust output.** A type whose pattern uses `\d`, `\w`, `\s` or `.` stops
  accepting the non-ASCII strings it accepted before. In `ridl.std` this affects
  `Date`, `TimeOfDay`, `Version` and `IpV4` (through `\d`) and `Uri` and `Url`
  (through `.`, for `\r`, U+2028 and U+2029). The new behaviour is the specified
  one, and the checker always had it.
- **Source.** A pattern that used a construct refused by §3 or §4 stops
  compiling. The corpus has none.
- **IR artifacts and `ridl diff`.** The `pattern` text in the IR changes for
  every pattern that uses a rewritten construct. A `ridl diff` against a
  baseline built by 0.5.0 reports each one as "a match pattern rewritten", which
  it classifies as breaking. For the Rust output that is accurate. The changelog
  entry says so, so that a consumer who sees the report knows the cause.
- **Kotlin output.** No behaviour change from the portable form for `\d` and
  `\w` (Java's are ASCII already). `\s` and `.` change to the specified meaning.

## 8. Testing

- **Agreement test** in `ridl-sem`. For every pattern in a fixed list (the probe
  patterns, every `ridl_std.typl` pattern, and one pattern per row of the §5
  table), compile the portable form with `regress` (`u` flag) and with `regex`,
  and assert that they agree on every single-scalar input (`^X$` over all
  Unicode scalars, as the probe did) and on a fixed list of multi-character
  inputs. Also assert that `regress` gives the same result on the author's text
  and on the portable form.
- **Diagnostic fixtures**: one TYPL-221 case per row of the §4 table; TYPL-106
  cases for `\A`, `\z`, `[[a]]` and `\-` (now refused by the `u` flag); and the
  existing TYPL-220 cases still firing.
- **Behaviour test** on generated code: the Rust output for `ridl.std.Date`
  refuses `"١٢٣٤-١٢-١٢"` and accepts `"2026-10-01"`. This needs a test that
  compiles and runs the generated crate, of the kind the backend's existing
  validation tests use.
- **Mutation check**: removing any one row of the §5 rewrite must make the
  agreement test fail.

## 9. Alternatives considered

1. **`RegexBuilder::unicode(false)` in the Rust output** (option 1 in #597).
   Rejected on measurement: with Unicode off, `regex` refuses `.` and `[^@]`
   ("pattern can match invalid UTF-8"), which removes the `Uri`, `Url` and
   `Email` types of `ridl.std`.
2. **The Rust output runs `regress` at run time.** The checker and the output
   would match by construction. Rejected: `regress` backtracks, and these
   patterns validate values received from the wire, so a crafted value can take
   exponential time. `regex` guarantees time linear in the input. It would also
   do nothing for the Kotlin output.
3. **Refuse every divergent construct, with no rewrite.** `\d`, `\w`, `\s` and
   `.` would all be TYPL-221, and `ridl.std` would be rewritten with explicit
   classes. Simplest to build. Rejected because it forbids the most common
   constructs authors write, when a mechanical rewrite keeps their meaning.
4. **Rewrite in the Rust backend instead of the lowering.** It keeps the
   author's text in the IR. Rejected because every backend would then port the
   rewrite, and the Kotlin plugin is in another repository and another language.
   The rewrite is part of the meaning the specification gives a pattern, so it
   belongs before the IR.
5. **A `u` flag in typl syntax** (option 2 in #597). Rejected: two meanings of
   one pattern is new language surface. The divergence is between engines, which
   is a backend matter, not a reason for new syntax.
6. **Accept `\p{..}` with a note that the Unicode version follows each engine.**
   Rejected for now, as §4 explains. This is the alternative that can still be
   taken later without a break.

## 10. Records to amend when this lands

- typl reference §2.7: the `u` flag, the portable subset, the portable form;
  replace the divergence table with the §5 table; remove the `\w` size-limit
  example.
- typl reference §16: the TYPL-221 row, and the TYPL-106 and TYPL-220 rows
  reworded for the `u` flag and the portable form.
- `ridl-core` diagnostic catalogue (`diag.rs`): TYPL-221, and the TYPL-106 and
  TYPL-220 doc comments.
- The IR documentation of `Constraint.pattern` (§6).
- ADR-0007 decision 10 (pattern validation with `regress`): read its `## Status`
  first, then record the `u` flag as an amendment if the record allows it.
- After merge: a heads-up issue on driftsys/ridlc-gen-kotlin that names the
  portable form, the `$` to `\z` rule, and the new behaviour of `\s` and `.`.
