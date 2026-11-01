# Open Enums and Enum Sets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** typl gains the contextual keyword `open` before `enum` and `enumset`;
an open declaration accepts undeclared values, exposes them as an unknown case,
and writes them back unchanged.

**Architecture:** One boolean `open` per enum and enum-set declaration, carried
from the parser through the IR (`v2`) and the codegen `Model` (`v1`). Each
backend branches on it. `ridl diff` reads it to classify appends and strict/open
changes.

**Tech Stack:** Rust workspace (`ridl-syntax`, `ridl-fmt`, `ridl-sem`,
`ridl-ir`, `ridl-backend-rust`, `ridl-backend-ts`, `ridl-backend-proto`,
`ridl-backend-flatbuffers`, `ridl-diff`, `ridlc`, `ridl-mcp`), prost-generated
protobuf schemas, Deno for the TypeScript checks.

**Spec:** `docs/wip/2026-11-01-open-enums-design.md` (read it before any task;
its section numbers are cited below as "design §n").

## Global Constraints

- Strict is the default; a declaration without `open` generates byte-identical
  code to `main` at `e0d8abb9` in every backend (design X-2). The only behaviour
  change for strict declarations is the `ridl diff` verdict of Task 5.
- `open` is never reserved: not in `TYPL_KEYWORDS`, not in `FAMILY_RESERVED`
  (design X-3); the `reserved-set` line of `freeze/keywords.txt` does not
  change.
- Modifier order is `internal`, `error`, `open`, then `enum` / `enumset`.
- IR and `Model` fields are append-only and default to `false` (strict).
- Rust: no value has two representations (design X-1); `Unknown<Name>` has a
  private field and is built only by `From<i64>`.
- Prose (comments, docs, commit messages) is plain and literal English; no story
  ids or plan names in `crates/`, `docs/book/`, `docs/design/`
  (`just
  story-id-check`).
- Conventional Commits with the scopes in `.git-std.toml`. Every commit ends
  with the attribution line the session gives.
- Implementers run the focused crate tests named in each task; CI runs the full
  gate.
- The Task 13 issue is driftsys/ridl#1168; the podbuf refusal of Task 3 cites
  it.

## Review Focus

1. **`open` as an ordinary identifier.** A field, a value name or a package
   segment spelled `open` (`struct S { open: bool }`, `type open = ...` where
   the naming rules allow it) must still parse and format. Pinned in Task 1.
2. **Pass-through through a transcode.** An `Unknown` decoded from FlatBuffers
   and encoded as proto3 with a raw value outside int32 must be refused, not
   truncated. Pinned in Task 3.
3. **A declared value given as raw.** `DtcStatus::from(1)` must be
   `DtcStatus::Active`, never `Unknown`; equality and hashing then follow.
   Pinned in Task 3.
4. **A strict enum's verdict for an existing workspace.** An append to a strict
   enum without `--encoding` must be breaking and must explain why, naming the
   `open` keyword. Pinned in Task 5.
5. **`open` before a non-enum declaration.** `open struct S {}` must be a parse
   error with a message, not a silent identifier. Pinned in Task 1.

---

### Task 1: Syntax — parser, AST, formatter, keywords dump, grammar text

**Files:**

- Modify: `crates/ridl-syntax/src/syntax_kind.rs` (add `OpenKw`),
  `crates/ridl-syntax/src/keywords.rs` (add
  `pub const TYPL_CONTEXTUAL: &[&str]
  = &["open"];`, not in any reserved
  list), `crates/ridl-syntax/src/parser.rs` (`definition`, `modifiers`,
  `is_top_level_start`, `is_typl_definition_start`, the rsdl foreign-declaration
  path), `crates/ridl-syntax/family.ungram` (`'open'?` before `'enum'` and
  `'enumset'`), `crates/ridl-syntax/src/ast.rs` (`HasModifiers::is_open`),
  regenerated `crates/ridl-syntax/src/ast/generated.rs`
  (`cargo xtask
  codegen`), `crates/ridl-fmt/src/lib.rs` (`modifiers_prefix`),
  `xtask/src/freeze/keywords.rs` and `freeze/keywords.txt`,
  `docs/specification/typl-language-reference.md` (§1.4 a sentence that `open`
  is contextual; Appendix E as design §3.1)
- Test: `crates/ridl-syntax/test_data/parser/ok/open_enums.typl` (+ snapshot),
  `crates/ridl-syntax/test_data/parser/err/open_before_struct.typl` (+
  snapshot), unit tests in `crates/ridl-fmt/src/lib.rs`, the tests in
  `xtask/src/freeze/keywords.rs`

**Interfaces:**

- Produces: `SyntaxKind::OpenKw`; `HasModifiers::is_open(&self) -> bool` on the
  `EnumDef` and `EnumSetDef` AST nodes; `keywords::TYPL_CONTEXTUAL`.

- [ ] **Step 1: Write the failing tests**

`open_enums.typl` holds: `open enum DtcStatus { INACTIVE = 0 ACTIVE = 1 }`,
`internal error open enum Fault { NONE = 0 }`, `open enumset Flags { A = 0 }`,
`open enumset Warnings : Fault`, and `struct S { open: bool }`. Its snapshot
shows `OpenKw` tokens in the first four and an `Ident` `open` in the struct.
`open_before_struct.typl` holds `open struct S {}`; its snapshot has one error
whose message names `enum` and `enumset` as the tokens expected after `open`.

```rust
#[test]
fn open_round_trips_in_modifier_order() {
    assert_eq!(fmt("internal   error open   enum E { A = 0 }"),
               "internal error open enum E {\n    A = 0\n}\n");
}
#[test]
fn open_is_kept_on_a_derived_enumset() {
    assert_eq!(fmt("open enumset W : E"), "open enumset W : E\n");
}
// xtask/src/freeze/keywords.rs
#[test]
fn dump_names_open_as_contextual_and_leaves_reserved_set_unchanged() {
    let lines = dump(root()).unwrap();
    assert!(lines.contains(&"contextual open typl".to_string()));
    assert!(!lines.iter().find(|l| l.starts_with("reserved-set ")).unwrap()
        .split(' ').any(|w| w == "open"));
}
```

- [ ] **Step 2: Run them** — `cargo test -p ridl-syntax -p ridl-fmt` and
      `cargo test -p xtask keywords`. Expected: FAIL (no `OpenKw`, `open`
      dropped by the formatter, no `contextual` line).

- [ ] **Step 3: Implement.** The lexer keeps lexing `open` as `Ident`. In
      `modifiers()` and every lookahead that skips `InternalKw | ErrorKw`, an
      `Ident` whose text is `open` followed (after trivia) by `EnumKw` or
      `EnumsetKw` is remapped to `OpenKw` (`bump_remap`); followed by anything
      else at a definition start, report the error of Step 1. `modifiers_prefix`
      emits `OpenKw` like the other two. The dumper emits one
      `contextual <word> typl` line per `TYPL_CONTEXTUAL` entry.

- [ ] **Step 4: Run them** — the commands of Step 2, plus
      `cargo xtask freeze
  check` after `cargo xtask freeze update`. Expected:
      PASS; the keywords dump diff is one added line.

- [ ] **Step 5: Commit** —
      `feat(ridl-syntax): parse the contextual keyword open
  on enum and enumset`

---

### Task 2: Semantics, IR and `Model`

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` (`EnumDef`:
  `bool open =
  3;`, `EnumSetDef`: `bool open = 4;`),
  `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (`Enum`: `bool open = 5;`,
  `EnumSet`: `bool open = 5;`), `crates/ridl-sem/src/check.rs` (`lower_enum`,
  `lower_enum_set` set `open` from `is_open()`; the warning below),
  `crates/ridl-core/src/diag.rs` (`TYPL_413 = "TYPL-413", Warning`, lint name
  `open-enum-without-values`; the builder takes the next free code if 413 is
  taken), `crates/ridl-ir/src/codegen/lower.rs` (copy `open`),
  `docs/book/lints.md`, `docs/specification/typl-language-reference.md` (§8 and
  §9 semantics and evolution rules as design §3.2, with Q-1's decision), the
  regenerated freeze dumps whose schema gains a field
- Test: in-file tests of `crates/ridl-sem/src/check.rs`,
  `crates/ridl-ir/src/catalog_hash.rs` tests,
  `crates/ridlc-gen-model/tests/parity.rs` (a fixture with an open enum)

**Interfaces:**

- Consumes: `HasModifiers::is_open` (Task 1).
- Produces: `v2::EnumDef.open: bool`, `v2::EnumSetDef.open: bool`,
  `v1::Enum.open: bool`, `v1::EnumSet.open: bool`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn open_is_lowered_on_enum_and_enumset() {
    let p = lower("package p\nopen enum E { A = 0 }\nopen enumset F : E\nenum G { A = 0 }");
    assert!(enum_def(&p, "E").open && enumset_def(&p, "F").open);
    assert!(!enum_def(&p, "G").open);
}
#[test]
fn an_open_enum_without_values_is_warned() {
    assert_codes("package p\nopen enum E {}", &["TYPL-413"]);
    assert_codes("package p\nopen enum E { A = 0 }", &[]);
}
// catalog_hash.rs
#[test]
fn the_open_flag_changes_the_hash_and_a_strict_enum_keeps_its_hash() {
    // `hash_of` over the existing Mode fixture: strict equals the value
    // recorded before this change; open differs from strict.
}
```

The parity fixture adds `open enum` and `open enumset`; the test asserts the
`Model` JSON carries `"open": true` on both.

- [ ] **Step 2: Run them** —
      `cargo test -p ridl-sem -p ridl-ir -p
  ridlc-gen-model`. Expected: FAIL
      (no `open` field).

- [ ] **Step 3: Implement** as listed in Files. The catalog hash needs no code:
      its input is the protobuf binary, and a `false` bool is not serialized, so
      a strict declaration hashes as before (the test proves it).

- [ ] **Step 4: Run them** — Step 2's command, `cargo test -p ridlc`, then
      `cargo xtask freeze update && cargo xtask freeze check`. Expected: PASS;
      freeze dumps gain lines only.

- [ ] **Step 5: Commit** —
      `feat(ridl-ir): carry the open flag of enums and enum
  sets`

---

### Task 3: Rust domain types and the proto3 and FlatBuffers codecs; podbuf refusal

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` (`emit_enum`, `emit_enum_set`,
  and the comment above `emit_enum_set` that cites `podbuf_strict_enum_append`),
  `crates/ridl-backend-rust/src/derives.rs`,
  `crates/ridl-backend-rust/src/defaults.rs`,
  `crates/ridl-backend-rust/src/codec.rs` (FlatBuffers verify/decode/encode),
  `crates/ridl-backend-rust/src/codec_proto3.rs`, the docs emitter for an open
  declaration (ADR-0028), `crates/ridl-sem/src/check.rs` (the RIDL-149 Rust
  spelling check covers `Unknown` and `Unknown<Name>`),
  `crates/ridlc/src/lib.rs` (refusal of open + podbuf),
  `crates/ridl-core/src/diag.rs` (a new RIDL error code for the refusal, next
  free in the podbuf check range), `docs/book/generated-code.md` (the open
  shapes; the `TryFrom` / `verify` table)
- Test: `crates/ridl-backend-rust/src/tests.rs` (+ snapshots),
  `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`,
  `crates/ridl-backend-rust/tests/proto3_conformance.rs`,
  `crates/ridl-backend-rust/tests/transcode_equivalence.rs`,
  `crates/ridlc/tests/podbuf_checks.rs`

**Interfaces:**

- Consumes: `v1::Enum.open`, `v1::EnumSet.open` (Task 2).
- Produces (generated Rust, for `open enum DtcStatus` and
  `open enumset
  Flags`):
  - `#[non_exhaustive] pub enum DtcStatus { ..., Unknown(UnknownDtcStatus) }`,
    no `#[repr]`, no `TryFrom<i64>`;
  - `pub struct UnknownDtcStatus(i64)` (private field), derives
    `Clone, Copy,
    PartialEq, Eq, Hash, PartialOrd, Ord, Debug`,
    `pub const fn raw(self) ->
    i64`;
  - `impl From<i64> for DtcStatus`, `impl From<DtcStatus> for i64`;
  - `impl From<i64> for Flags` (keeps every bit) in place of `TryFrom<i64>`,
    `pub const fn unknown_bits(self) -> i64`.

- [ ] **Step 1: Write the failing tests**

```rust
// tests.rs — generated-source assertions
#[test]
fn an_open_enum_has_an_unknown_variant_and_is_non_exhaustive() { /* snapshot;
    assert contains "#[non_exhaustive]", "Unknown(UnknownDtcStatus)", no "repr(i64)",
    no "TryFrom<i64> for DtcStatus" */ }
#[test]
fn a_strict_enum_is_unchanged() { /* the existing enum snapshots pass unmodified */ }
#[test]
fn an_open_enumset_converts_from_any_integer() { /* snapshot; "From<i64> for Flags",
    "fn unknown_bits" */ }
#[test]
fn a_value_named_unknown_collides_in_an_open_enum() {
    assert_codes("package p\nopen enum E { UNKNOWN = 0 }", &["RIDL-149"]);
    assert_codes("package p\nopen enum E { A = 0 }\nstruct UnknownE {}", &["RIDL-149"]);
}
// flatbuffers_conformance.rs and proto3_conformance.rs, each:
#[test]
fn an_open_enum_round_trips_an_undeclared_value() {
    let bytes = encode_with_raw_status(7);                // DtcStatus declares 0 and 1
    let msg = verify_and_decode(&bytes).unwrap();
    assert!(matches!(msg.status, DtcStatus::Unknown(u) if u.raw() == 7));
    assert_eq!(encode(&msg), bytes);                       // pass-through
    assert_eq!(DtcStatus::from(1), DtcStatus::Active);     // one representation
}
#[test]
fn an_open_enumset_keeps_an_undeclared_bit() { /* raw 0b1000_0001, A = bit 0:
    unknown_bits() == 0b1000_0000, re-encode equals input */ }
#[test]
fn a_strict_enum_still_refuses_an_undeclared_value() { /* Rule::Variant */ }
// transcode_equivalence.rs
#[test]
fn an_unknown_outside_int32_is_refused_by_the_proto3_encoder() {
    // decode from FlatBuffers with raw 1 << 40, encode as proto3:
    // Err(EncodeError::Contract(Violation { rule: Rule::Range, .. }))
}
// podbuf_checks.rs
#[test]
fn an_open_enum_in_a_podbuf_package_is_refused() {
    // manifest [backend.rust] encodings = ["podbuf"], `open enum E { A = 0 }`
    // `ridl build` exits 1; the diagnostic names E and #1168.
}
```

- [ ] **Step 2: Run them** —
      `cargo test -p ridl-backend-rust -p ridlc -p
  ridl-sem`. Expected: FAIL.

- [ ] **Step 3: Implement.** Every codec site that checks or converts an enum
      through `TryFrom` branches on `open`: open uses `From<i64>` and has no
      verify check. Encoding already goes through `i64::from` (`codec.rs`),
      which the open `From<DtcStatus> for i64` serves. The proto3 encoder checks
      that an open enum's integer fits int32 and refuses with `Rule::Range`
      otherwise. The podbuf codec is not changed; `ridlc` refuses before it is
      reached, and if the Rust backend is handed an open declaration with
      `podbuf` among its encodings (plugin path), it returns its existing
      generation error rather than emitting strict code.

- [ ] **Step 4: Run them** — Step 2's command, then
      `cargo clippy -p
  ridl-backend-rust --all-targets -- -D warnings`.
      Expected: PASS.

- [ ] **Step 5: Commit** —
      `feat(ridl-backend-rust): emit an unknown variant for
  an open enum`

---

### Task 4: TypeScript face and codec; schema comments

**Files:**

- Modify: `crates/ridl-backend-ts/src/lib.rs` (`emit_enum`, `emit_enum_set`),
  `crates/ridl-backend-ts/src/codec.rs` (decode and encode of an open enum),
  `crates/ridl-backend-proto/src/lib.rs` and
  `crates/ridl-backend-flatbuffers/src/lib.rs` (`emit_enum`: one comment line,
  `// strict: a value outside this list is refused by RIDL codecs` or
  `// open:
  a value outside this list is kept`), `docs/book/generated-code.md`
- Test: `crates/ridl-backend-ts/src/tests.rs` (+ snapshots),
  `crates/ridl-backend-ts/tests/codec.rs`, the proto and FlatBuffers
  `src/tests.rs` and their stability snapshots

**Interfaces:**

- Consumes: `v1::Enum.open`, `v1::EnumSet.open` (Task 2).
- Produces (generated TypeScript):
  `export type DtcStatusValue = DtcStatus | {
  readonly unknown: number };`
  fields of an open enum type are typed `DtcStatusValue`;
  `FlagsBits.unknownBits(value: Flags): Flags`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_open_enum_emits_a_value_type_with_an_unknown_case() { /* snapshot */ }
// tests/codec.rs (runs Deno over the generated module)
// decode raw 7 -> { unknown: 7 }; encode it -> bytes equal the input;
// decode raw 1 -> DtcStatus.ACTIVE; decode raw 2**53 -> throws, rule "variant";
// open enumset raw 0b1000_0001 -> unknownBits(...) === 0b1000_0000
#[test]
fn proto_and_flatbuffers_state_strict_or_open() {
    // the emitted enum carries the comment of Files for each kind;
    // the strict comment is the only change to the existing stability snapshots
}
```

- [ ] **Step 2: Run them** —
      `cargo test -p ridl-backend-ts -p
  ridl-backend-proto -p ridl-backend-flatbuffers`.
      Expected: FAIL.

- [ ] **Step 3: Implement.** The strict-enum comment changes strict output; this
      is the one accepted exception to "byte-identical" for strict declarations
      (comment only, no wire or API change) — record it in the commit body.

- [ ] **Step 4: Run them** — Step 2's command and `just ts-check`. Expected:
      PASS.

- [ ] **Step 5: Commit** —
      `feat(ridl-backend-ts): expose the unknown value of an
  open enum`

---

### Task 5: `ridl diff` and the records

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs` (`Category::StrictEnumAppend` →
  `"strict_enum_append"`, `Category::EnumOpened` → `"enum_opened"`,
  `Category::EnumClosed` → `"enum_closed"`; `PodbufStrictEnumAppend` stays in
  the enum, is no longer emitted, and its doc says it is superseded by
  `strict_enum_append`), `crates/ridl-diff/src/walk.rs` (emit the three; stop
  emitting `PodbufStrictEnumAppend`; `PodbufWidthChange` is unchanged),
  `crates/ridl-diff/src/classify.rs` (verdicts and explain texts),
  `crates/ridl-mcp/src/diff.rs` and `crates/ridl-mcp/tests/tools.json`,
  `docs/book/evolving-an-interface.md`, `docs/design/podbuf-codec.md`,
  `docs/design/mcp-workspace-tools.md`,
  `docs/decisions/ADR-0031-podbuf-projection-rules.md` (a dated note on decision
  12 and the `## Status` line: built, Q-1 and Q-2 decisions, the Rust shape)
- Test: `crates/ridl-diff/src/tests.rs`,
  `crates/ridl-diff/src/classify/classify_tests.rs`,
  `crates/ridl/tests/diff_cli.rs`, `crates/ridl/tests/diff_gate.rs`

**Interfaces:**

- Consumes: `v2::EnumDef.open`, `v2::EnumSetDef.open` (Task 2).
- Produces: the three categories and their verdicts — `StrictEnumAppend`
  Breaking for every encoding; `EnumOpened` Compatible; `EnumClosed` Breaking.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_append_to_a_strict_enum_is_breaking_without_an_encoding() {
    let d = diff(STRICT_V1, STRICT_V2_APPEND, &[]);
    assert!(has(&d, Category::StrictEnumAppend));
    assert_eq!(d.verdict, Verdict::Breaking);
}
#[test]
fn an_append_to_an_open_enum_or_enumset_is_compatible() {
    assert_eq!(diff(OPEN_V1, OPEN_V2_APPEND, &[]).verdict, Verdict::Compatible);
    assert_eq!(diff(OPEN_V1, OPEN_V2_APPEND, &["podbuf"]).verdict, Verdict::Compatible); // fits width
}
#[test]
fn podbuf_strict_enum_append_is_no_longer_reported() {
    assert!(!has(&diff(STRICT_V1, STRICT_V2_APPEND, &["podbuf"]), Category::PodbufStrictEnumAppend));
}
#[test]
fn opening_is_compatible_and_closing_is_breaking() {
    assert_eq!(verdict_of(diff(STRICT_V1, OPEN_V1, &[]), Category::EnumOpened), Verdict::Compatible);
    assert_eq!(verdict_of(diff(OPEN_V1, STRICT_V1, &[]), Category::EnumClosed), Verdict::Breaking);
}
#[test]
fn the_strict_append_explanation_names_the_open_keyword() {
    assert!(explain("strict_enum_append").contains("open enum"));
}
```

The same four cases are repeated for `enumset`. `diff_cli.rs` asserts the text
report and exit code for the strict append without `--encoding`.

- [ ] **Step 2: Run them** — `cargo test -p ridl-diff -p ridl -p ridl-mcp`.
      Expected: FAIL.

- [ ] **Step 3: Implement.** In `classify.rs`, the `EnumDef` and `EnumSetDef`
      arms of the appended-slot table accept an append only when `new_def.open`;
      the walk reports a strict append as `StrictEnumAppend` (one line per
      declaration, detail lists the appended members, as
      `PodbufStrictEnumAppend` did) instead of `DeclAdded`. A change of `open`
      alone is reported once per declaration as `EnumOpened` or `EnumClosed`,
      alongside any value changes.

- [ ] **Step 4: Run them** — Step 2's command, `just link-check`,
      `just
  doc-path-check`, `just story-id-check`. Expected: PASS.

- [ ] **Step 5: Commit** —
      `feat(ridl-diff)!: classify enum appends by strict or
  open` with a
      `BREAKING CHANGE:` footer: an append to a strict enum or enum set is now
      breaking for every encoding.

---

## After the last task (controller)

- File the issue for N-4 (no strict-enum verify in the TypeScript codec).
- Branch-level `/review` passes 1 and 2, then garden this plan and the design
  (`sdd-gardening`) before the PR merges.
