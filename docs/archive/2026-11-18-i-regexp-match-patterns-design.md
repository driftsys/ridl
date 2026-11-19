# `match` patterns are I-Regexp — one meaning in the checker and every output

Status: design note for driftsys/ridl#597, written 2026-11-18 against `main` at
e012019c, which is after release 0.20.0. The owner accepted the "I-Regexp only"
proposal on 2026-11-18, and decided the same day to refuse `\p{..}` (§3.6).
Nothing here is implemented. This note replaces
[`2026-10-01-portable-match-patterns-design.md`](../archive/2026-10-01-portable-match-patterns-design.md)
(approach A), which was never implemented. §12 records the decisions the owner
took on the questions this note raised.

## 1. The problem

One pattern text is read by several engines, and they do not match the same
strings:

- the checker, `ridl-sem`, with `regress` 0.11.1 and no flags — it decides
  TYPL-106 and tests a declared init value against the pattern (TYPL-109);
- the Rust output and the wasm codec, with `regex` 1.13.1 and its defaults;
- the TypeScript output, with the host's `RegExp` and no `u` flag;
- the Kotlin output (driftsys/ridlc-gen-kotlin), with `java.util.regex`.

The differences are recorded in three places. The issue measures `\d`, `\w`,
`\p{..}`, `\A`, `\z`, `\a`, `\<`, `[a&&b]` and `[[a]]`. The typl reference §2.7
adds `.`, and the archived note adds `\s` and `\b`.
driftsys/ridlc-gen-kotlin#179 adds POSIX classes and `(?i)`. Each backend has to
translate these constructs, and each has to choose a reference. The Kotlin
plugin has merged three pull requests for this (#168, #176 and #185 of that
repository), and its issue #186 waits for this change.

All four engines search for a match anywhere in the value. A pattern with no `^`
and no `$` therefore accepts any value that contains a match.

## 2. The decision in one paragraph

A typl pattern is an I-Regexp
([RFC 9485](https://www.rfc-editor.org/rfc/rfc9485)) without Unicode categories,
and it matches the whole value. `ridl-sem` refuses every other pattern, so
`ridl check`, `ridl build`, the language server and every path that sends a
codegen request refuse the same patterns with the same diagnostic. A string type
may carry several `match` clauses, and a value must match every clause. A
backend does not translate or refuse a construct: it receives a text that every
engine reads the same way, and it adds only the whole-value anchoring of its
engine.

## 3. The pattern language

### 3.1 What a typl pattern has

- literal characters, and the escapes `\(` `\)` `\*` `\+` `\.` `\?` `\[` `\\`
  `\]` `\^` `\n` `\r` `\t` `\{` `\|` `\}`, and `\-` inside a class only (§3.3);
- `.`, which matches any character except `\n` and `\r`;
- character classes with ranges and negation: `[a-z0-9_]`, `[^@]`;
- alternation, groups, and the quantifiers `*`, `+`, `?`, `{n}`, `{n,}`,
  `{n,m}`.

A literal character, a class range and a negated class take any Unicode
character, so a pattern is not limited to ASCII.

### 3.2 What I-Regexp does not have

`^` and `$` anchors, `\d` `\w` `\s` and their negations, `\b`, `\A`, `\z`,
`\uXXXX` and `\xXX`, lookahead and lookbehind, backreferences, non-capturing and
named groups, lazy and possessive quantifiers, inline flags, POSIX classes,
nested classes, and class set operators.

Replacements:

| Today              | Write                                   |
| ------------------ | --------------------------------------- |
| `^…$`              | `…` (the match is whole)                |
| `^…` with no `$`   | `….*`                                   |
| `…$` with no `^`   | `.*…`                                   |
| `…` with no anchor | `.*….*` (§9: no diagnostic asks for it) |
| `\d`               | `[0-9]`                                 |
| `\w`               | `[0-9A-Za-z_]`                          |
| `\s`               | `[ \t\n\r]`                             |
| `[\x20-\x7E]`      | `[ -~]`                                 |
| `(?=.*X)(?=.*Y)`   | two `match` clauses (§4)                |
| `(a)\1`            | the alternatives, written out           |
| `(?i)ab`           | `[Aa][Bb]`                              |

### 3.3 Six spellings typl refuses although RFC 9485 allows them

RFC 9485 gives these spellings a meaning, and the engines read them differently.
A probe on 2026-11-18 measured `regex` 1.13.1, V8 (Node, `u` flag) and
`java.util.regex` (OpenJDK 21), each with whole-value anchoring:

| Spelling                   | I-Regexp meaning      | `regex`              | `RegExp`, `u` flag  | `java.util.regex`   | typl spelling   |
| -------------------------- | --------------------- | -------------------- | ------------------- | ------------------- | --------------- |
| `^` or `$` outside a class | the literal character | anchor               | anchor              | anchor              | `\^`, `[$]`     |
| `&&` inside a class        | the character `&`     | class intersection   | `&`                 | class intersection  | one `&`         |
| `~~` inside a class        | the character `~`     | symmetric difference | `~`                 | `~`                 | one `~`         |
| `--` inside a class        | the character `-`     | class difference     | not probed          | not probed          | one `-`         |
| `\-` outside a class       | the character `-`     | `-`                  | compile error       | `-`                 | a bare `-`      |
| `\p{..}`, `\P{..}`         | a Unicode category    | its Unicode version  | its Unicode version | its Unicode version | explicit ranges |

The probe measured each spelling of the last column in all three engines, for
the rows `^`, `$`, `&&`, `~~` and `\-`. The `regex` reading of `--` is from the
source of `regex-syntax`. The refusal is of an unescaped `-` directly followed
by an unescaped `-`. The grammar of RFC 9485 produces that only when a class
holds nothing but a leading and a trailing `-`: `[--]` and `[^--]`. A class such
as `[\--z]`, the range from `-` to `z`, is accepted, and all three engines read
it the same way. §3.6 covers the last row.

A class never needs a character twice, so the three class rows remove nothing
from the language.

The first row is the one that matters for migration. Most patterns in this
repository use `^` or `$` today. Under plain RFC 9485, `^[A-Z]{2}$` would stay
valid and would change its meaning to "a `^`, two letters, a `$`" with no
diagnostic. Refusing the bare characters turns that change into an error with a
help text.

With these refusals a typl pattern is still an I-Regexp: typl accepts a subset
of RFC 9485, not a variant of it.

### 3.4 The delimiter

A regex literal is still `/…/`, and the lexer still needs `\/` for a `/` inside
it (`[/]` does not work, because the lexer does not parse classes). `\/` is not
an I-Regexp escape. It is the delimiter escape of the literal: the pattern is
the text between the delimiters with every `\/` replaced by `/`, and that text
is what RFC 9485 is applied to.

### 3.5 Whole-value matching

A value matches a pattern only when the pattern matches the value from its first
character to its last. A pattern for a prefix says so with `.*`. `.` does not
match `\n` or `\r`; a prefix followed by any character at all is `(.|\n|\r)*`.

### 3.6 `\p{..}` is refused

The owner decided on 2026-11-18 to refuse `\p{..}` and `\P{..}`.

Each engine carries its own Unicode tables, so a category does not hold the same
code points in every engine. A code point that a later Unicode version assigned
is inside `\p{L}` in one engine and outside it in another, and a Unicode version
can also move an assigned code point to another category. The approach A note
measured 4,644 code points of `\p{L}` on which `regress` 0.11.1 and `regex`
1.13.1 differ. The Rust `regex` crate, the host's `RegExp` and the JVM each have
one Unicode version, and no test in this repository can pin the version of a
host engine. No spelling of the pattern removes this difference.

With the construct refused, every accepted pattern means the same in every
engine. The cost is that a pattern cannot select characters by category. It can
still accept them: `.` and a negated class match any Unicode character, and
explicit ranges such as `[a-zà-ÿ]` select a stated set. No pattern in this
repository uses a category.

The refusal can be reversed. Accepting `\p{..}` later, with a stated rule for
the Unicode version, breaks no source file.

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

- **Syntax.** A recogniser for the ABNF of RFC 9485 §3, written in `ridl-sem`.
  The grammar needs no backtracking, so this is a small hand-written parser. It
  replaces the call to `regress`. Beyond the ABNF it refuses:
  - `[^]`, which RFC 9485 §3 excludes in prose;
  - the spellings of §3.3;
  - a class range whose end is before its start (`[z-a]`), and a quantifier
    whose maximum is below its minimum (`{3,2}`). The grammar produces both, and
    the `regex` crate refuses both (measured).
- **One diagnostic.** A pattern outside the language is TYPL-106, in a `match`
  and in a regex constant. The message names the construct and its replacement
  from §3.2 or §3.3, for example: "`^` is not accepted: a pattern matches the
  whole value, so remove the anchor; a literal `^` is `\^`". The code keeps its
  number and its catalogue text ("invalid regex syntax in `match` or a regex
  `const`"). No new code is added.
- **TYPL-220 stays** for an accepted pattern that the `regex` crate still
  refuses. The known case is the size limit of a compiled pattern:
  `(([a-z]{1,64}){1,64}){1,64}` exceeds it (measured). Its message drops the
  reference to ECMA-262.
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
of clauses, each with its own optional constant name. The compatibility rule of
the IR specification §6 applies to both: a field whose meaning is retired stays
written and read, and the meaning of an existing value does not change in place.

- **The normalised pattern.** One function in `ridl-ir` takes a source literal
  and returns the normalised pattern: the delimiters are removed, `\/` is `/`,
  and every `.` outside a class is written `[^\n\r]`. The normalised pattern is
  itself an I-Regexp with the same meaning. It exists because the engines
  disagree on `.`: `regex` excludes only `\n`, and `RegExp` and Java also
  exclude U+2028 and U+2029 (Java also U+0085). With `.` spelled out, the only
  step left to a backend is the anchoring of §7.
- **New field.** `ridl.ir.v2.Constraint` and `ridl.codegen.v1.Constraint` each
  gain `repeated Match matches`, where `Match` holds `pattern` and an optional
  `pattern_const`. One message per clause keeps a text and its constant
  together, which two parallel lists cannot do. In `ridl.ir.v2` the text is the
  source literal. In `ridl.codegen.v1` the text is the normalised pattern.
- **Old fields.** `pattern` and `pattern_const` stay, are marked deprecated, and
  are still written, for the first clause. A reader of 0.20 searches with that
  text, so the writer gives it a text that a search reads as a whole-value
  match. In `ridl.codegen.v1` the text is `^(?:P)$`, where `P` is the normalised
  pattern. In `ridl.ir.v2` the field keeps its delimiters, as today, and the
  text is `/^(?:S)$/`, where `S` is the text between the delimiters of the
  source literal, with its `\/` escapes unchanged. The meaning of the field ("a
  pattern that a search applies") does not change. A search with `regex` or with
  `RegExp` gives the verdict of the whole-value match. In `java.util.regex`, `$`
  also matches before a final line terminator, as it does for every pattern that
  ends with `$` today. A reader of 0.20 does not see a second clause; §12
  decision 2 accepts that.
- **Readers.** Every reader of `Constraint.pattern` in this workspace moves to
  `matches`: the two code backends, the proto3 and FlatBuffers spellings, the
  descriptor spelling, hover, the derived-init rule in `ridl-sem`,
  `constraint_is_vacuous`, the absent-value rule of the proto3 codec, the
  IR-to-model copy and the model-to-IR copy, and `ridl diff`. Four more sites
  read `pattern_const` and visit the constant of every clause: the reference
  walk of `ridl-ir`, the canonical form behind the catalog hash, the design lint
  that qualifies an inline scalar, and the dependency scan of `ridlc`, which
  matches the JSON key.
- **Input from 0.20.** An IR snapshot or a codegen request that 0.20 wrote has
  no `matches`. Every reader treats an empty `matches` with a present `pattern`
  as one clause with the old meaning (a search), so the old fields are still
  read, as the IR specification §6 requires.
- **The Rust backend** reads the model, so it receives the normalised pattern.
  **The TypeScript backend** reads `ridl.ir.v2` directly and calls the same
  `ridl-ir` function. No backend implements the normalisation itself.
- **Rule text.** The rule list holds one `Rule.pattern` entry per clause. Its
  text stays the author's literal with its delimiters, because generated doc
  comments print it.
- `Constant.regex_body` and `ConstDef.regex` keep their shape and their text.

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
  flag, which it does not pass today. The message of the `ConstraintError`
  already holds the constraint text, so it names the clause that refused.
- **proto3 and FlatBuffers backends, the descriptor spelling, hover.** Each
  spells the constraint as text. Each writes every clause, in order.
- **`ridl-fmt`.** The constraint layout takes more than one `match`.

## 8. `ridl diff`

A constraint is compared as a set of clauses. A clause that the new side adds
narrows the type, and a clause that it removes widens it. The comparison is
still by text. Every pattern is rewritten by the migration, so a diff of a
migrated package against a baseline from 0.20 reports every pattern as changed,
which it classifies as breaking. §9 lists the patterns whose accepted values did
change. For a pattern that was `^…$` with none of those constructs, the report
is a false positive that the changelog entry has to name.

## 9. Compatibility

This is a breaking change to typl, marked `!`, in a declared minor release, with
a hand-written changelog entry.

- **Source, with a diagnostic.** Every pattern that uses a construct of §3.2 or
  §3.3 becomes TYPL-106. Most patterns in this repository use `^` or `$`.
- **Source, with no diagnostic.** A pattern with no anchor and no refused
  construct is a valid I-Regexp, so it compiles as before and changes from a
  search to a whole-value match. `/ABC/` accepted `"xABCx"` and now accepts only
  `"ABC"`. Five patterns in this repository are in this group, all in fixtures
  and examples (`/ABC/`, `/ab/`, `/[a-z]+/` twice, `/a\/b\/c/`). The compiler
  cannot tell an intended search from an intended whole match, so the changelog
  entry states this case first.
- **Accepted values.**
  - A pattern that used `\d`: the Rust output stops accepting non-ASCII digits.
  - A pattern that used `.`: the Rust output stops accepting `\r` in that
    position, and the checker and the TypeScript output start accepting U+2028
    and U+2029 there.
  - A pattern that used `\s` or `\w`: the replacements of §3.2 are ASCII sets.
    `[ \t\n\r]` is smaller than the 25 characters of `\s` in the checker, and
    `[0-9A-Za-z_]` is smaller than the Unicode `\w` of the Rust output.
  - A character outside the Basic Multilingual Plane: with the `u` flag, the
    TypeScript output counts it as one character for `.`, a class and a
    quantifier, as the Rust output does. Today it counts as two.
  - A pattern with no `$`, or no `^`: every output stops accepting text after,
    or before, the match, unless the author adds `.*`.
- **`ridl.std`.** All 13 patterns (8 constants and 5 inline) are rewritten:

  | Declaration             | Today                                 | After                             |
  | ----------------------- | ------------------------------------- | --------------------------------- |
  | 6 constants and 2 types | `^…$` with no other refused construct | the same text without `^` and `$` |
  | `ASCII_PATTERN`         | `^[\x20-\x7E]+$`                      | `[ -~]+`                          |
  | `IPV4_PATTERN`          | `^(\d{1,3}\.){3}\d{1,3}$`             | `([0-9]{1,3}\.){3}[0-9]{1,3}`     |
  | `Date`                  | `^\d{4}-\d{2}-\d{2}$`                 | `[0-9]{4}-[0-9]{2}-[0-9]{2}`      |
  | `TimeOfDay`             | `^\d{2}:\d{2}:\d{2}`                  | `[0-9]{2}:[0-9]{2}:[0-9]{2}.*`    |
  | `Version`               | `^\d+\.\d+\.\d+`                      | `[0-9]+\.[0-9]+\.[0-9]+.*`        |

  `TimeOfDay` and `Version` have no `$` today, so they accept any text after the
  prefix. `.*` keeps that, except that the text can no longer hold `\n` or `\r`.
- **Catalog hash.** The hash input includes the constraint. The new `matches`
  field and the rewritten `ridl.std` patterns change the hash of every unit that
  reaches a type with a pattern, including a unit that reaches only a `ridl.std`
  type. A toolchain change that moves the hash restarts each compatibility chain
  once, as the catalog descriptor design records.
- **Generated code.** The pattern text in every generated crate and module
  changes, and the doc comment of a type prints the new literal.
  `freeze/generated/` is regenerated.
- **Plugins.** The model changes as §6 describes. The Kotlin plugin needs a
  release that reads `matches`, removes its translation code, and calls
  `matches()`.

## 10. Testing

- **Recogniser fixtures.** One accepted and one refused case for each production
  of the ABNF that typl keeps, one refused case for each row of §3.2, one for
  each row of §3.3, and one each for `[^]`, `[z-a]` and `{3,2}`. Each refused
  case asserts TYPL-106 and the construct that the message names.
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
- **Old field.** A search with the text of the deprecated `pattern` field gives
  the verdict of the whole-value match, for every pair of the agreement list.
- **Mutation checks.** Removing one refusal of §3.3 fails a recogniser fixture.
  Removing the `.` rewrite, or the anchoring of one output, fails the agreement
  test.

## 11. Alternatives considered

1. **Approach A: ECMA-262 with the `u` flag, and a rewrite to a portable form**
   (the archived note). It kept `\d`, `\w`, `\s` and the anchors, and refused
   `\p{..}` and `\b`. Rejected on 2026-11-18: the reference stays an
   engine-specific dialect, the rewrite is a second pattern language that the
   compiler has to maintain, and the differences found since (`\<`, POSIX
   classes, `(?i)`) show that listing divergent constructs one by one does not
   end.
2. **Plain RFC 9485, without the refusals of §3.3.** Each backend would then
   escape `^`, `$`, `&` and `~` for its engine, which is the per-backend
   translation this change removes. It would also let `^[A-Z]{2}$` change its
   meaning without a diagnostic.
3. **Accept `^…$` as an optional spelling of a whole-value match.** It would
   make the migration smaller. Rejected: it is a second spelling of the same
   pattern, and the text would no longer be an I-Regexp.
4. **Accept `\p{..}` and state that a category follows the Unicode version of
   each engine.** This is what RFC 9485 requires. Rejected by the owner on
   2026-11-18: it leaves one construct on which two sides of a connection can
   give different verdicts for the same value (§3.6).
5. **Each backend normalises `.` itself.** Rejected: it is one more step that
   each backend can get wrong. One `ridl-ir` function does it (§6).
6. **Stop writing the old `pattern` field.** Rejected: the IR specification §6
   requires a deprecated field to stay written, and a plugin of 0.20 would emit
   no pattern check at all.
7. **An automatic migration in `ridl fmt` or a fix command.** Not built. The
   TYPL-106 help text names the replacement for each construct, and the prefix
   patterns need a decision by the author that a tool cannot make.
8. **A clause index in `ridl_rt::payload::Rule::Pattern`.** It would let the
   Rust violation name the clause. Not built: it changes a frozen runtime
   surface.

## 12. Decisions by the owner

The owner answered these on 2026-11-18.

1. **The refusals of §3.3 are accepted**, all six. typl accepts a subset of
   RFC 9485.
2. **A reader of 0.20 and a type with several clauses.** The old field holds the
   first clause only (§6), so a plugin built against 0.20 checks one clause and
   ignores the others. The IR specification §6 calls a new field that a reader
   cannot ignore a breaking change. The owner accepts this for the one new
   source feature. The Kotlin plugin, the only known plugin outside this
   repository, is released with the compiler. The alternatives were to name the
   packages `ridl.codegen.v2` and `ridl.ir.v3`, or to leave several clauses out
   of this change.
3. **`TimeOfDay` and `Version`** keep accepting any suffix through `.*` (§9).
   Stating the suffix that each standard allows is a separate issue.

## 13. Records to amend when this lands

- typl reference §2.7 (the pattern language, replacing the dialect text and its
  table), §5.3 (several clauses), §6.2, §16 (TYPL-106 and TYPL-220), the grammar
  appendix (`pattern`, the constraint productions), the `ridl.std` appendix, the
  two appendices that map `match` to a JSON Schema `pattern` (the text is
  `^(?:P)$`, and several clauses are an `allOf`), the normative references (RFC
  9485 replaces ECMA-262) and the glossary.
- `family.ungram`.
- IR specification: the `Constraint` fields, and a statement in §6 of the
  exception that §12 decision 2 accepts.
- ADR-0007 decision 10 (pattern validation with `regress`) and ADR-0013 (the
  TypeScript pattern check "follows the checker's ECMA-262 semantics"): read
  each `## Status` first, then record the amendment.
- `docs/book/generated-code.md` (the `validate-pattern` feature and the
  TypeScript `RegExp` section), `docs/book/getting-started.md` (two regex
  constants and the `match` examples), `docs/book/documenting.md` and
  `docs/book/codegen-plugins.md` (the rule text of a pattern), and
  `docs/design/ts-codec-table.md`.
- `docs/design/catalog-descriptor.md`: its two statements that the narrowing of
  a byte capacity from an ASCII-only pattern (driftsys/ridl#665) waits for this
  issue.
- `ridl-core` diagnostic catalogue (`diag.rs`): the TYPL-106 and TYPL-220 doc
  comments.
- The workspace `Cargo.toml` comment on `regress` and `regex`, and the comment
  in `check.rs` that says a typl pattern is anchored with `^` and `$`.
- After merge: a comment on driftsys/ridlc-gen-kotlin#186 that names the
  normalised pattern, the `matches` field, the `matches()` call, and the refusal
  of `\p{..}`.
