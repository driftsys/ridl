# `match` patterns are I-Regexp — one meaning in the checker and every output

Status: design note for driftsys/ridl#597, written 2026-11-18 against `main` at
e012019c (release 0.20.0). The owner accepted the "I-Regexp only" proposal on
2026-11-18. Nothing here is implemented. This note replaces
[`2026-10-01-portable-match-patterns-design.md`](../archive/2026-10-01-portable-match-patterns-design.md)
(approach A), which was never implemented. §12 lists the questions that the
owner still has to answer.

## 1. The problem

One pattern text is read by several engines, and they do not match the same
strings:

- the checker, `ridl-sem`, with `regress` 0.11.1 and no flags — it decides
  TYPL-106 and tests a declared init value against the pattern (TYPL-109);
- the Rust output and the wasm codec, with `regex` 1.13.1 and its defaults;
- the TypeScript output, with the host's `RegExp` and no `u` flag;
- the Kotlin output (driftsys/ridlc-gen-kotlin), with `java.util.regex`.

The measured differences are in the issue: `\d`, `\w`, `\s`, `\b`, `.`,
`\p{..}`, `\A`, `\z`, `\<`, `[a&&b]`, `[[a]]`, POSIX classes and `(?i)`. Each
backend has to translate these constructs, and each has to choose a reference.
The Kotlin plugin has merged three pull requests for this and has one issue
still open.

All four engines search for a match anywhere in the value. A pattern with no `^`
and no `$` therefore accepts any value that contains a match.

## 2. The decision in one paragraph

A typl pattern is an I-Regexp
([RFC 9485](https://www.rfc-editor.org/rfc/rfc9485)) and it matches the whole
value. `ridl-sem` refuses every other pattern, so `ridl check`, `ridl build`,
the language server and every path that sends a codegen request refuse the same
patterns with the same diagnostic. A string type may carry several `match`
clauses, and a value must match every clause. A backend does not translate or
refuse a construct: it receives a text that every engine reads the same way, and
it adds only the whole-value anchoring of its engine.

## 3. The pattern language

### 3.1 What I-Regexp has

- literal characters, and the escapes `\(` `\)` `\*` `\+` `\-` `\.` `\?` `\[`
  `\\` `\]` `\^` `\n` `\r` `\t` `\{` `\|` `\}`;
- `.`, which matches any character except `\n` and `\r`;
- character classes with ranges and negation: `[a-z0-9_]`, `[^@]`;
- Unicode general categories: `\p{L}`, `\p{Lu}`, `\P{Nd}`, alone or in a class;
- alternation, groups, and the quantifiers `*`, `+`, `?`, `{n}`, `{n,}`,
  `{n,m}`.

### 3.2 What I-Regexp does not have

`^` and `$` anchors, `\d` `\w` `\s` and their negations, `\b`, `\A`, `\z`,
`\uXXXX` and `\xXX`, lookahead and lookbehind, backreferences, non-capturing and
named groups, lazy and possessive quantifiers, inline flags, POSIX classes,
nested classes, and class set operators.

Replacements:

| Not accepted     | Write                         |
| ---------------- | ----------------------------- |
| `^…$`            | `…` (the match is whole)      |
| `^…` alone       | `….*`                         |
| `\d`             | `[0-9]`                       |
| `\w`             | `[0-9A-Za-z_]`                |
| `\s`             | `[ \t\n\r]`                   |
| `[\x20-\x7E]`    | `[ -~]`                       |
| `(?=.*X)(?=.*Y)` | two `match` clauses (§4)      |
| `(a)\1`          | the alternatives, written out |
| `(?i)ab`         | `[Aa][Bb]`                    |

### 3.3 Four spellings typl refuses although RFC 9485 allows them

RFC 9485 gives these four spellings a meaning, and the engines read them
differently. A probe on 2026-11-18 measured `regex` 1.13.1, V8 (Node, `u` flag)
and `java.util.regex` (OpenJDK 21), each with whole-value anchoring:

| Spelling                   | I-Regexp meaning      | `regex`              | `RegExp`, `u` flag | `java.util.regex`  |
| -------------------------- | --------------------- | -------------------- | ------------------ | ------------------ |
| `^` or `$` outside a class | the literal character | anchor               | anchor             | anchor             |
| `&&` inside a class        | the character `&`     | class intersection   | `&`                | class intersection |
| `~~` inside a class        | the character `~`     | symmetric difference | `~`                | `~`                |
| `\-` outside a class       | the character `-`     | `-`                  | compile error      | `-`                |

typl refuses each one, and each has a spelling that all three engines read the
same way (measured in the same probe):

- a literal `^` is `\^`, and a literal `$` is `[$]`;
- a class never needs a character twice, so `[a&&b]` is `[a&b]`;
- a `-` outside a class is written bare.

`--` inside a class is refused with `&&` and `~~`: `regex` reads it as class
difference. The grammar of RFC 9485 produces it only as `[--]`.

The first row is the one that matters for migration. Every pattern in this
repository uses `^` or `$` today. Under plain RFC 9485, `^[A-Z]{2}$` would stay
valid and would silently change its meaning to "a `^`, two letters, a `$`".
Refusing the bare characters turns that silent change into an error with a help
text.

With these four refusals a typl pattern is still an I-Regexp: typl accepts a
subset of RFC 9485, not a variant of it.

### 3.4 The delimiter

A regex literal is still `/…/`, and the lexer still needs `\/` for a `/` inside
it (`[/]` does not work, because the lexer does not parse classes). `\/` is not
an I-Regexp escape. It is the delimiter escape of the literal: the pattern is
the text between the delimiters with every `\/` replaced by `/`, and that text
is what RFC 9485 is applied to.

### 3.5 Whole-value matching

A value matches a pattern only when the pattern matches the value from its first
character to its last. A pattern for a prefix says so with `.*`. Note that `.`
does not match `\n` or `\r`; a prefix followed by any character at all is
`(.|\n|\r)*`.

### 3.6 `\p{..}` and the Unicode version

Each engine carries its own Unicode tables. The members of `\p{L}` are the same
in every engine for a code point that the oldest of the engines knows. A code
point that a later Unicode version assigned can be inside the category in one
engine and outside it in another. The specification states this as the one
remaining difference. A pattern that needs an exact set uses explicit ranges.

## 4. Several `match` clauses

```ridl
type Token : string [8..64 match /.*[0-9].*/ match /.*[A-Z].*/]
```

- The constraint grammar takes `match` zero or more times after a length bound,
  and one or more times with no bound.
- A value must match every clause. The order of the clauses has no meaning.
- Each clause is a regex literal or the name of a regex constant, as today.
- `bytes` still takes no `match`.
- A type with one or more clauses has no derivable init value (TYPL-115), as a
  type with one clause has today.

This replaces lookahead for the one constraint that I-Regexp cannot express in a
single pattern: "holds at least one X and at least one Y".

## 5. The checker (`ridl-sem`)

- **Syntax.** A recogniser for the ABNF of RFC 9485 §3, plus the refusals of
  §3.3, written in `ridl-sem`. The grammar has 20 productions and no
  backtracking, so this is a small hand-written parser. It replaces the call to
  `regress`.
- **One diagnostic.** A pattern outside the language is TYPL-106, in a `match`
  and in a regex constant. The message names the construct and its replacement
  from §3.2 or §3.3, for example: "`^` is not accepted: a pattern matches the
  whole value, so remove the anchor; a literal `^` is `\^`". The code keeps its
  number and its meaning ("not a valid pattern"). No new code is added.
- **TYPL-220 stays** for the one refusal that is left: the `regex` crate limits
  the size of a compiled pattern, and a pattern such as `(\p{L}{1,256}){1,256}`
  exceeds it (measured). Its message drops the reference to ECMA-262.
- **TYPL-109 and RIDL-110.** The init value of a type is tested against every
  clause, with the `regex` crate and the engine text of §7. This is the engine
  and the text of the Rust output, so the checker and the Rust output agree by
  construction. Today this test is a search with `regress`.
- **`regress` leaves the workspace.** No other code uses it.
- **Placement.** `validate_regex` already runs in `ridl-sem` for a constant and
  for an inline `match`, and both `ridl check` and `ridl build` refuse a pattern
  it refuses (measured on 0.20.0 with a TYPL-220 pattern). The new check takes
  its place, so no command needs a change.

## 6. The IR and the codegen model

Two changes meet here: the text has a new meaning, and a constraint holds a list
of clauses, each with its own optional constant name.

- `ridl.ir.v2.Constraint` and `ridl.codegen.v1.Constraint` each gain
  `repeated Match matches`, where `Match` holds `pattern` and an optional
  `pattern_const`. One message per clause keeps a text and its constant
  together, which two parallel lists cannot do.
- The writer fills `matches` only. The old `pattern` and `pattern_const` fields
  stay declared and are marked deprecated, so no line of
  `freeze/codegen-protocol.txt` is removed. `ridl diff` reads them from a
  snapshot that an older release wrote, as a list of one clause.
- In `ridl.ir.v2` the text is the source literal, as today.
- In `ridl.codegen.v1` the text is the **normalised pattern**: the delimiters
  are removed, `\/` is `/`, and every `.` outside a class is written `[^\n\r]`.
  The normalised pattern is itself an I-Regexp with the same meaning. It exists
  because the engines disagree on `.`: `regex` excludes only `\n`, and `RegExp`
  and Java also exclude U+2028 and U+2029 (Java also U+0085). With `.` spelled
  out, the only step left to a backend is the anchoring of §7.
- The rule list of the model holds one `Rule.pattern` entry per clause. Its text
  is the normalised pattern.
- `Constant.regex_body` and `ConstDef.regex` keep their shape. The model's
  `regex_body` is normalised in the same way.

A plugin built against the model of 0.20 reads `pattern`, which is now absent,
and emits no pattern check. §12 question 2 asks whether that is acceptable.

## 7. The outputs

Each output wraps the normalised pattern `P` for a whole-value match, and
changes nothing else:

| Output                    | Engine text and call                           |
| ------------------------- | ---------------------------------------------- |
| Rust, wasm codec, checker | `Regex::new(r"\A(?:P)\z")`, `is_match`         |
| TypeScript                | `new RegExp("^(?:P)$", "u")`, `test`           |
| Kotlin plugin             | `Pattern.compile("P")`, `matcher(v).matches()` |

- **Rust backend.** One `static` per clause, checked in declaration order. The
  first clause that refuses the value returns the violation. `Rule::Pattern` in
  `ridl-rt` keeps its shape, so a violation does not say which clause refused.
  `ridl-rt` is a frozen surface, and a clause index is not needed to fix this
  issue.
- **TypeScript backend.** One `RegExp` constant per clause, built with the `u`
  flag, which it does not pass today. The `ConstraintError` already carries the
  constraint text, so it names the clause that refused.
- **proto3 and FlatBuffers backends, the descriptor spelling, hover.** Each
  spells the constraint as text. Each writes every clause, in order.
- **`ridl-fmt`.** The constraint layout takes more than one `match`.

## 8. `ridl diff`

A constraint is compared as a set of clauses. A clause that the new side adds
narrows the type, and a clause that it removes widens it. The comparison is
still by text. Every pattern is rewritten by the migration, so a diff of a
migrated package against a baseline from 0.20 reports every pattern as changed,
which it classifies as breaking. For a pattern that held `\d` or had no `$`, the
set of accepted values did change. For the others the report is a false positive
that the changelog entry has to name.

## 9. Compatibility

This is a breaking change to typl, marked `!`, in a declared minor release, with
a hand-written changelog entry.

- **Source.** Every pattern that uses a construct of §3.2 or §3.3 becomes
  TYPL-106. Every pattern in this repository does: all use `^` or `$`.
- **Accepted values, Rust output.** A type whose pattern used `\d` stops
  accepting non-ASCII digits. A type whose pattern used `.` stops accepting `\r`
  in that position.
- **`ridl.std`.** All 15 patterns are rewritten:

  | Declaration             | Today                                 | After                             |
  | ----------------------- | ------------------------------------- | --------------------------------- |
  | 8 constants and 2 types | `^…$` with no other refused construct | the same text without `^` and `$` |
  | `ASCII_PATTERN`         | `^[\x20-\x7E]+$`                      | `[ -~]+`                          |
  | `IPV4_PATTERN`          | `^(\d{1,3}\.){3}\d{1,3}$`             | `([0-9]{1,3}\.){3}[0-9]{1,3}`     |
  | `Date`                  | `^\d{4}-\d{2}-\d{2}$`                 | `[0-9]{4}-[0-9]{2}-[0-9]{2}`      |
  | `TimeOfDay`             | `^\d{2}:\d{2}:\d{2}`                  | `[0-9]{2}:[0-9]{2}:[0-9]{2}.*`    |
  | `Version`               | `^\d+\.\d+\.\d+`                      | `[0-9]+\.[0-9]+\.[0-9]+.*`        |

  `TimeOfDay` and `Version` have no `$` today, so they accept any text after the
  prefix. `.*` keeps that, except that the text can no longer hold `\n` or `\r`.
- **Generated code.** The pattern text in every generated crate and module
  changes. `freeze/generated/` is regenerated.
- **Plugins.** The model changes as §6 describes. The Kotlin plugin needs a
  release that reads `matches`, removes its translation code and its `(?dU)`
  prefix, and calls `matches()`.

## 10. Testing

- **Recogniser fixtures.** One accepted and one refused case for each production
  of the ABNF, one refused case for each row of §3.2, and one for each row of
  §3.3. Each refused case asserts TYPL-106 and the construct that the message
  names.
- **Agreement test.** A fixed list of patterns and values: the probe cases of
  §3.3, every `ridl.std` pattern, and one pattern for each construct of §3.1.
  The test asserts that the Rust engine text and the TypeScript engine text give
  the same verdict for every pair. `examples/cabin/web/check_test.ts` already
  compares the two engines and is the place for it. The Kotlin plugin runs the
  same list in its own repository.
- **Several clauses.** A value that fails only the second clause is refused, by
  the checker for an init value, by the generated Rust code, and by the
  generated TypeScript code.
- **Whole-value test** on generated code: `ridl.std.Date` refuses
  `"x2026-11-18"`, `"2026-11-18\n"` and `"١٢٣٤-١٢-١٢"`, and accepts
  `"2026-11-18"`.
- **Mutation checks.** Removing one refusal of §3.3 fails a recogniser fixture.
  Removing the `.` rewrite, or the anchoring of one output, fails the agreement
  test.

## 11. Alternatives considered

1. **Approach A: ECMA-262 with the `u` flag, and a rewrite to a portable form**
   (the archived note). It kept `\d`, `\w`, `\s` and the anchors, and refused
   `\p{..}` and `\b`. Rejected on 2026-11-18: the reference stays an
   engine-specific dialect, the rewrite is a second pattern language that the
   compiler has to maintain, and the class of differences found since (`\<`,
   POSIX classes, `(?i)`) shows that listing divergent constructs one by one
   does not end.
2. **Plain RFC 9485, without the refusals of §3.3.** Each backend would then
   escape `^`, `$`, `&` and `~` for its engine, which is the per-backend
   translation this change removes. It would also let `^[A-Z]{2}$` change its
   meaning without a diagnostic.
3. **Accept `^…$` as an optional spelling of a whole-value match.** It would
   make the migration smaller. Rejected: it is a second spelling of the same
   pattern, and the text would no longer be an I-Regexp.
4. **Refuse `\p{..}`**, as approach A did, because of the Unicode version. The
   accepted proposal includes it. RFC 9485 requires it, and a subset without it
   cannot express a non-ASCII letter except by listing ranges.
5. **Each backend normalises `.` itself.** Rejected: it is one more step that
   each backend can get wrong, and the normalised text is still an I-Regexp.
6. **An automatic migration in `ridl fmt` or a fix command.** Not built. The
   TYPL-106 help text names the replacement for each construct, and the prefix
   patterns need a decision by the author that a tool cannot make.
7. **A clause index in `ridl_rt::payload::Rule::Pattern`.** It would let the
   Rust violation name the clause. Not built: it changes a frozen runtime
   surface.

## 12. Questions for the owner

1. **The four refusals of §3.3.** They go beyond "I-Regexp only": typl accepts a
   subset of RFC 9485. Recommended, because without them each backend escapes
   characters again and `^…$` changes meaning silently.
2. **A plugin of 0.20 with a compiler of the next release** emits no pattern
   check, because `pattern` is absent (§6). The alternatives are to keep filling
   the old field with a search-compatible text (`^(?:P)$`, first clause only),
   or to name the model `ridl.codegen.v2`. Recommended: leave the old field
   absent and release the Kotlin plugin with the compiler. The plugin in
   question is the only known one outside this repository.
3. **`TimeOfDay` and `Version`** keep accepting any suffix through `.*` (§9).
   The alternative is to state the suffix each standard allows. Recommended:
   `.*` in this change, and the exact suffix as a separate issue.

## 13. Records to amend when this lands

- typl reference §2.7 (the pattern language, replacing the dialect text and both
  tables), §5.3 (several clauses), §6.2, §16 (TYPL-106 and TYPL-220), the
  grammar appendix (`pattern`, the constraint productions), the `ridl.std`
  appendix, the JSON Schema mapping (a `pattern` is `^(?:P)$`, and several
  clauses are an `allOf`), the normative references (RFC 9485 replaces ECMA-262)
  and the glossary.
- `family.ungram` and the constraint section of the family general form.
- ADR-0007 decision 10 (pattern validation with `regress`) and ADR-0013 (the
  TypeScript pattern check "follows the checker's ECMA-262 semantics"): read
  each `## Status` first, then record the amendment.
- `docs/book/generated-code.md` (the `validate-pattern` feature and the
  TypeScript `RegExp` section), `docs/book/getting-started.md` (two regex
  constants and the `match` examples), `docs/design/ts-codec-table.md`, and
  `docs/design/codegen-plugins.md` (the `matches` field).
- `ridl-core` diagnostic catalogue (`diag.rs`) and `freeze/diagnostics.txt`.
- The workspace `Cargo.toml` comment on `regress` and `regex`.
- After merge: an issue on driftsys/ridlc-gen-kotlin that names the normalised
  pattern, the `matches` field, and the `matches()` call.
