# Enum variants in PascalCase — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Rust backend spells every enum variant through a pinned
`pascal_case` transform (`CHECK_ENGINE` → `CheckEngine`), and RIDL-149 rejects
two values of one enum whose names collide under it. This closes
driftsys/ridl#506.

**Architecture:** `pascal_case = camel_case ∘ snake_case` is added beside the
other two pinned transforms in `crates/ridl-ir/src/name.rs`. The codegen model's
`Spellings` carries it as a fifth field, `pascal`. The checker in `ridl-sem`
keys RIDL-149 over an enum's values on it. The Rust backend reads it at the four
sites that spell a variant. The TypeScript, proto and FlatBuffers backends do
not change.

**Tech Stack:** Rust (edition 2024, toolchain pinned by `rust-toolchain.toml`),
prost/pbjson for the codegen model, `insta` snapshots, `rustc` compile proofs in
the test suite, `just` for every gate.

**Spec:**
[`2026-09-26-enum-variant-pascal-case-design.md`](2026-09-26-enum-variant-pascal-case-design.md)
("the design"), and ADR-0016's amendment of 2026-09-26
([`../decisions/ADR-0016-schema-projection-and-the-name-transform.md`](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)).
Read both before Task 1.

## Global Constraints

- Work on one branch from `origin/main` in a git worktree under
  `.claude/worktrees/`, and run `./bootstrap` in it first. One pull request for
  the whole plan: the snapshots move in the same pull request as the emit change
  (#506's "Done when").
- `pascal_case(name)` is exactly `camel_case(&snake_case(name))`. Do not write a
  new algorithm.
- RIDL-149 over an enum's values is keyed on `pascal_case` **alone** (design
  §5.2). Do not add a `snake_case` key.
- The message form is `Collision::Pascal(String)`, and its text is fixed in Task
  3. It names `pascal_case` and no other transform.
- A `reserved` enum value, and a value name repeated verbatim, stay out of the
  check (design §5.1, §5.3). The verbatim repeat is driftsys/ridl#554; do not
  fix it here.
- An empty `Spellings.pascal` means `camel_case(snake)` (design §4). The Rust
  backend reads the field only through the helper `pascal_of`.
- The TypeScript, proto and FlatBuffers backends, and enumset bits, keep their
  spelling. No change in `crates/ridl-backend-ts`, `crates/ridl-backend-proto`
  or `crates/ridl-backend-flatbuffers`.
- Commit messages follow Conventional Commits, linted by `just lint-commits`
  against `.git-std.toml`. The commit that renames generated variants is marked
  breaking (`!`).
- Prose in comments, commits and docs is plain and literal English (AGENTS.md).
- Update snapshots with `INSTA_UPDATE=always` and read every moved line before
  committing it. A snapshot change that is not a variant rename, a `pascal`
  line, or a RIDL-149 message is a defect to report, not to accept.
- Run `cargo test` with `--no-fail-fast` whenever a result is used as evidence:
  `cargo test` stops at the first failing test binary, which hides failures in
  later ones (#451 records this).

## Review Focus

1. **A value outside the typl convention.** `checkEngine`, `HTTPServer` or
   `SELF` as an enum value must generate Rust that compiles. Task 1 pins their
   `pascal_case` outputs, and Task 4 runs rustc, with `-D non_camel_case_types`,
   over a package holding all three (`SELF` becomes `Self_` through `ident()`).
2. **Two values that differ only in underscores or case** (`CHECK_ENGINE`,
   `CHECK__ENGINE`) must be refused at check time, not by rustc. Task 3.
3. **A model written by an older toolchain**, with every `pascal` empty, must
   generate the same Rust as a current one. Task 4.
4. **A multi-word variant reaching a compile proof.** The deny flag must bite on
   a fixture that holds one; a deny on a fixture with only single-word variants
   tests nothing. Task 5 proves it by reverting one site.
5. **A variant named `Ok`, `Err`, `None` or `Some`** must not shadow the `core`
   type in generated code. `OK` in `examples/cabin` becomes `Ok`, and
   `just demo` compiles and runs it (Task 4); the Appendix B and corpus proofs
   compile every enum under the deny (Task 5).

---

### Task 1: The pinned `pascal_case` transform

**Files:**

- Modify: `crates/ridl-ir/src/name.rs` (module doc at lines 1-14; add the
  function after `camel_case`, around line 78; tests in the `tests` module)

**Interfaces:**

- Produces: `pub fn pascal_case(name: &str) -> String` in `ridl_ir::name`.

- [ ] **Step 1: Write the failing tests**

Add `pascal_case` to the `use super::{...}` line of the `tests` module, and add
these tests at the end of the module:

```rust
    /// The outputs design §3 names, pinned as values.
    #[test]
    fn pascal_case_pins_the_outputs_the_design_names() {
        for (input, expected) in [
            ("CHECK_ENGINE", "CheckEngine"),
            ("PARK", "Park"),
            ("OK", "Ok"),
            ("A", "A"),
            ("X2", "X2"),
            ("ABS_V2", "AbsV2"),
            ("V2_ABS", "V2Abs"),
            ("LEVEL_10", "Level10"),
            ("HTTP_SERVER", "HttpServer"),
            ("A__B", "AB"),
            ("A_", "A"),
            ("checkEngine", "CheckEngine"),
            ("HTTPServer", "HttpServer"),
            ("SELF", "Self"),
        ] {
            assert_eq!(pascal_case(input), expected, "pascal_case(`{input}`)");
        }
    }

    /// Every name of one to five characters over `a`, `B`, `_` and `2` that
    /// starts with a letter, as the lexer requires.
    fn enumerated_names() -> Vec<String> {
        let alphabet = ['a', 'B', '_', '2'];
        let mut names: Vec<String> = vec!["a".to_string(), "B".to_string()];
        let mut frontier = names.clone();
        for _ in 1..5 {
            let mut next = Vec::new();
            for name in &frontier {
                for c in alphabet {
                    next.push(format!("{name}{c}"));
                }
            }
            names.extend(next.iter().cloned());
            frontier = next;
        }
        names
    }

    /// Design §3 property 4: two names that share a `snake_case` output share
    /// a `pascal_case` output. This is why RIDL-149 keys an enum's values on
    /// `pascal_case` alone.
    #[test]
    fn pascal_case_collides_wherever_snake_case_does() {
        let mut by_snake: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();
        for name in enumerated_names() {
            let pascal = pascal_case(&name);
            let (first, first_pascal) = by_snake
                .entry(snake_case(&name))
                .or_insert_with(|| (name.clone(), pascal.clone()));
            assert_eq!(
                *first_pascal, pascal,
                "`{first}` and `{name}` share a snake_case output but not a pascal_case one"
            );
        }
    }

    /// The containment is strict: this pair differs under `snake_case` and
    /// collides under `pascal_case`.
    #[test]
    fn pascal_case_collides_where_snake_case_does_not() {
        assert_ne!(snake_case("CHECK_ENGINE"), snake_case("CHECK__ENGINE"));
        assert_eq!(pascal_case("CHECK_ENGINE"), pascal_case("CHECK__ENGINE"));
    }

    /// Design §3 property 2: every output is a name rustc's
    /// `non_camel_case_types` accepts — non-empty, no underscore, and an
    /// upper-case first character.
    #[test]
    fn every_pascal_case_output_satisfies_non_camel_case_types() {
        for name in enumerated_names() {
            let pascal = pascal_case(&name);
            assert!(!pascal.is_empty(), "`{name}` gives an empty name");
            assert!(!pascal.contains('_'), "`{name}` gives `{pascal}`");
            assert!(
                pascal.starts_with(|c: char| c.is_ascii_uppercase()),
                "`{name}` gives `{pascal}`"
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-ir --lib name::tests` Expected: FAIL to compile, with
error E0432, "unresolved import `super::pascal_case`".

- [ ] **Step 3: Write the implementation**

After `camel_case`, add:

```rust
/// PascalCase of any name the lexer admits: `CHECK_ENGINE` becomes
/// `CheckEngine`. Used for the Rust backend's enum variant names (ADR-0016,
/// 2026-09-26 amendment).
///
/// It is [`camel_case`] of [`snake_case`]: `snake_case` lower-cases the name
/// and separates its words, and `camel_case` then upper-cases the first
/// character of each word and removes the separators. [`camel_case`] alone
/// does not serve, because it leaves each segment's tail as written and so
/// gives `CHECKENGINE`. Composing the two also defines the result for a name
/// outside the typl convention: `checkEngine` gives `CheckEngine`.
///
/// **The transform is not injective**, and its collision set contains
/// [`snake_case`]'s: two names that share a `snake_case` output share this
/// one, because this is a function of that output. The converse fails —
/// `CHECK_ENGINE` and `CHECK__ENGINE` collide here only. So RIDL-149 checks
/// an enum's values under this transform alone. It is not idempotent either:
/// `A_B` gives `AB`, and `AB` gives `Ab`. Nothing applies it twice.
pub fn pascal_case(name: &str) -> String {
    camel_case(&snake_case(name))
}
```

Rewrite the module doc's first paragraph (lines 3-8) to name the third function:

```rust
//! [`snake_case`] serves every target whose namespace is snake_case, and
//! [`camel_case`] every target whose namespace is CamelCase — the Rust
//! backend's union variants and its induced tuple struct names.
//! [`pascal_case`], the composition of the two, serves the Rust backend's
//! enum variants. They live here rather than in a backend because a
//! projection is a pure function from IR identity to a target's namespace,
//! and because `ridl-ir` is the only crate `ridl-sem` and the backends
//! already depend on.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-ir --lib name::tests` Expected: PASS, all tests in the
module.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/name.rs
git commit -m "feat(ridl-ir): add the pinned pascal_case name transform (#506)"
```

---

### Task 2: `Spellings.pascal` in the codegen model

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto:94-104`
  (`message Spellings`)
- Modify: `crates/ridl-ir/src/codegen/names.rs:1-15` (`spellings()`)
- Test: `crates/ridl-ir/src/codegen/tests.rs:114-135`
- Snapshots: `crates/ridlc/tests/snapshots/corpus__codegen@*.snap` (five files
  gain lines)

**Interfaces:**

- Consumes: `ridl_ir::name::pascal_case` (Task 1).
- Produces: the field `pascal: String` on `v1::Spellings`, filled for every
  declared identifier.

- [ ] **Step 1: Write the failing test**

In `crates/ridl-ir/src/codegen/tests.rs`, rename
`every_identifier_carries_its_four_spellings` to
`every_identifier_carries_its_five_spellings`, change its doc comment to "Every
declared identifier carries the pinned transforms and the compositions the
backends make of them (design note D-2).", add
`assert_eq!(name.pascal, "Dashboard");` after the `Dashboard` screaming
assertion and `assert_eq!(name.pascal, "CurrentSpeed");` after the
`currentSpeed` one, and append:

```rust
let gear = model
    .declarations
    .iter()
    .find(|decl| decl.name.as_ref().is_some_and(|name| name.declared == "GearState"))
    .expect("GearState is lowered");
let Some(v1::declaration::Kind::Enum(def)) = gear.kind.as_ref() else {
    panic!("GearState is an enum");
};
let reverse = def
    .values
    .iter()
    .filter_map(|value| value.name.as_ref())
    .find(|name| name.declared == "REVERSE")
    .expect("REVERSE is lowered");
assert_eq!(reverse.pascal, "Reverse");
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
`cargo test -p ridl-ir --lib codegen::tests::every_identifier_carries_its_five_spellings`
Expected: FAIL to compile, with "no field `pascal` on type `Spellings`".

- [ ] **Step 3: Write the implementation**

In `model.proto`, add the field after `screaming`:

```proto
// `ridl_ir::name::pascal_case`, which is `camel_case` of `snake`
// (ADR-0016, 2026-09-26 amendment): the Rust backend's enum variant names.
// Empty in a model written by a toolchain older than this field; a reader
// that meets an empty value derives it as `camel_case(snake)`, which is
// what makes the field additive (IR stability design D-5).
string pascal = 5;
```

In `names.rs`, import `pascal_case` beside the other two and fill the field:

```rust
use crate::name::{camel_case, pascal_case, snake_case};

/// Every namespace a target can need for one declared identifier.
pub(crate) fn spellings(declared: &str) -> v1::Spellings {
    let snake = snake_case(declared);
    v1::Spellings {
        declared: declared.to_string(),
        camel: camel_case(declared),
        pascal: pascal_case(declared),
        screaming: snake.to_uppercase(),
        snake,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ridl-ir --lib codegen::tests` Expected: PASS.

- [ ] **Step 5: Update the codegen snapshots**

Run: `INSTA_UPDATE=always cargo test -p ridlc --test corpus --no-fail-fast`
Then: `git diff --stat crates/ridlc/tests/snapshots/` Expected: only the five
`corpus__codegen@*.snap` files that carry a model change. Each change is an
added `"pascal": "..."` line, plus the `"screaming"` line above it, which gains
a trailing comma because it is no longer the object's last field. Check with
`git diff crates/ridlc/tests/snapshots/ | grep '^[-+] ' | grep -v '"pascal"\|"screaming"'`,
which must print nothing, and with
`git diff crates/ridlc/tests/snapshots/ | grep '^-.*"screaming"' | sed 's/^-//; s/$/,/' | sort > /tmp/before && git diff crates/ridlc/tests/snapshots/ | grep '^+.*"screaming"' | sed 's/^+//' | sort > /tmp/after && diff /tmp/before /tmp/after`,
which must print nothing: every `"screaming"` line changed only by that comma.
If the field order puts `"pascal"` elsewhere in the object, the line that gains
the comma is a different field; adjust the second check to that field and report
it. Then rerun `cargo test -p ridlc --test corpus --no-fail-fast` and expect
PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-ir/proto/ridl/codegen/v1/model.proto crates/ridl-ir/src/codegen/names.rs crates/ridl-ir/src/codegen/tests.rs crates/ridlc/tests/snapshots/
git commit -m "feat(ridl-ir): carry the pascal_case spelling in the codegen model (#506)"
```

---

### Task 3: RIDL-149 over one enum's values

**Files:**

- Modify: `crates/ridl-sem/src/check.rs` — the import at line 35, `Collision` at
  lines 648-663, `lower_enum` at lines 2629-2716, a new method beside
  `check_arm_projection` (line 3441), `colliding_projected_name` at lines
  3514-3556 and its doc comment above it, and tests after the union-arm RIDL-149
  tests (around line 6520)
- Modify: `crates/ridl-core/src/diag.rs:622-643` (RIDL-149's doc comment)
- Modify: `docs/specification/ridl-language-reference.md` §16.4, the RIDL-149
  row (line 1760)
- Modify: `docs/decisions/ADR-0017-proto3-projection-rules.md` decision 5 (lines
  118-131)

**Interfaces:**

- Consumes: `ridl_ir::name::pascal_case` (Task 1).
- Produces: `Collision::Pascal(String)`; the method
  `fn check_enum_value_projection(&mut self, name: &str, range: TextRange,
pascal_seen: &mut HashMap<String, (String, TextRange)>)`.

- [ ] **Step 1: Write the failing tests**

Add after the last union-arm RIDL-149 test in `check.rs`'s test module:

```rust
    // --- RIDL-149 over an enum's values (ADR-0016, 2026-09-26 amendment) ---

    fn enum_source(first: &str, second: &str) -> String {
        format!("package app\nenum E {{ {first} = 0, {second} = 1 }}\n")
    }

    /// The pair differs under `snake_case` and collides under `pascal_case`:
    /// the Rust backend would emit the variant `CheckEngine` twice.
    #[test]
    fn ridl_149_enum_values_colliding_under_pascal_case_are_refused() {
        let checked = check_source("app", &enum_source("CHECK_ENGINE", "CHECK__ENGINE"));
        assert_eq!(codes(&checked), vec!["RIDL-149"], "got: {:?}", checked.diagnostics);
    }

    /// A pair that collides under `snake_case` collides under `pascal_case`
    /// too, and is refused once.
    #[test]
    fn ridl_149_enum_values_colliding_under_snake_case_are_refused_once() {
        let checked = check_source("app", &enum_source("checkEngine", "CHECK_ENGINE"));
        assert_eq!(codes(&checked), vec!["RIDL-149"], "got: {:?}", checked.diagnostics);
    }

    /// Letter case alone: `PARK` and `Park` both become `Park`.
    #[test]
    fn ridl_149_enum_values_differing_only_in_case_are_refused() {
        let checked = check_source("app", &enum_source("PARK", "Park"));
        assert_eq!(codes(&checked), vec!["RIDL-149"], "got: {:?}", checked.diagnostics);
    }

    #[test]
    fn ridl_149_enum_values_that_do_not_collide_are_accepted() {
        let checked = check_source("app", &enum_source("LOW_FUEL", "CHECK_ENGINE"));
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    /// `A_B` gives `AB` and `AB` gives `Ab`: distinct, so accepted. A check
    /// keyed on `name.to_lowercase().replace('_', "")` refuses this pair.
    #[test]
    fn ridl_149_enum_values_that_collide_only_under_a_merged_transform_are_accepted() {
        let checked = check_source("app", &enum_source("A_B", "AB"));
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    /// A `reserved` value emits no variant, so it is not in the namespace.
    #[test]
    fn ridl_149_a_reserved_enum_value_is_not_in_the_checked_namespace() {
        let checked = check_source(
            "app",
            "package app\nenum E { reserved CHECK__ENGINE, CHECK_ENGINE = 0 }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    /// A value name repeated verbatim is not a transform collision; the
    /// exact-duplicate rule is driftsys/ridl#554. This pins that RIDL-149
    /// does not claim it.
    #[test]
    fn ridl_149_does_not_report_an_enum_value_repeated_verbatim() {
        let checked = check_source("app", &enum_source("A", "A"));
        assert!(
            !codes(&checked).contains(&"RIDL-149"),
            "got: {:?}",
            checked.diagnostics
        );
    }

    #[test]
    fn ridl_149_names_pascal_case_for_an_enum_value() {
        let checked = check_source("app", &enum_source("CHECK_ENGINE", "CHECK__ENGINE"));
        let diagnostic = &checked.diagnostics[0];
        assert_eq!(
            diagnostic.message,
            "`CHECK__ENGINE` and `CHECK_ENGINE` both become `CheckEngine` under the \
             pascal_case name transform, so a target whose namespace is PascalCase would \
             carry one identifier twice. Rename one of them (ridl §11, §16.4; ADR-0016 \
             decision 3)"
        );
        assert_eq!(
            only_label(diagnostic),
            "`CHECK_ENGINE` becomes `CheckEngine` here"
        );
    }
```

Before relying on the `reserved` test, confirm the enum `reserved` syntax the
parser accepts:
`grep -n "reserved" crates/ridl-sem/src/check.rs | grep "enum E"` shows
`enum E { A = 0, reserved 3, B = 3 }` at line 6229, a reserved _value_. If a
reserved _name_ in an enum is written differently, use the form the parser's
`test_data` shows
(`grep -rn "reserved" crates/ridl-syntax/test_data/parser/ok/*.typl`) and keep
the test's intent: a reserved name whose `pascal_case` output equals a live
value's draws no RIDL-149.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-sem --lib ridl_149_ --no-fail-fast` Expected: the new
refusal tests and the message test FAIL (no diagnostic); the four acceptance
tests PASS already.

- [ ] **Step 3: Write the implementation**

Import the transform:

```rust
use ridl_ir::name::{camel_case, pascal_case, snake_case};
```

Add the fourth form at the end of `Collision`:

```rust
/// The two names this report mentions project to this one PascalCase
/// identifier — an enum's values, checked under `pascal_case` alone
/// because its collision set contains `snake_case`'s.
Pascal(String),
```

Add the arm to the `match collision` in `colliding_projected_name`, after
`Collision::Camel`:

```rust
Collision::Pascal(projected) => (
    format!(
        "`{projected}` under the pascal_case name transform, so a target \
         whose namespace is PascalCase would carry one identifier twice"
    ),
    format!("`{first}` becomes `{projected}` here"),
),
```

Add the method after `check_arm_projection`:

```rust
/// RIDL-149 over one enum value name, against the values already seen.
///
/// The Rust backend spells the variant with `pascal_case`; proto's
/// prefixed value is `snake_case` upper-cased. `pascal_case` is a function
/// of `snake_case`'s output, so every pair that collides under
/// `snake_case` collides here too, and one key covers both targets
/// (ADR-0016, 2026-09-26 amendment). First wins, so the secondary label
/// points at the value that keeps the projected name.
fn check_enum_value_projection(
    &mut self,
    name: &str,
    range: TextRange,
    pascal_seen: &mut HashMap<String, (String, TextRange)>,
) {
    let pascal = pascal_case(name);
    if let Some((first, first_range)) = pascal_seen.get(&pascal).cloned() {
        self.colliding_projected_name(
            name,
            &first,
            &Collision::Pascal(pascal.clone()),
            range,
            first_range,
        );
    }
    pascal_seen
        .entry(pascal)
        .or_insert_with(|| (name.to_string(), range));
}
```

In `lower_enum`, beside `let mut seen: HashSet<i64> = HashSet::new();`, add:

```rust
// RIDL-149 over the values' Rust spelling; see
// `check_enum_value_projection`.
let mut declared_values: HashSet<String> = HashSet::new();
let mut pascal_values: HashMap<String, (String, TextRange)> = HashMap::new();
```

and immediately before `values.push(v2::EnumValue {`, add:

```rust
// A value name repeated verbatim is not a collision after a
// transform, so it is held out of the projection map, as a
// union arm's is. The exact-duplicate rule for an enum's values
// is driftsys/ridl#554. A value skipped above for a missing
// integer is not emitted, so it is not checked either.
if declared_values.insert(name.clone()) {
    self.check_enum_value_projection(
        &name,
        member_name_range(value_node.name(), value_node.syntax()),
        &mut pascal_values,
    );
}
```

Update `colliding_projected_name`'s doc comment: "Shared by the
interface-member, parameter, struct-field and union-arm checks — one rule over
four namespaces." becomes "Shared by the interface-member, parameter,
struct-field, union-arm and enum-value checks — one rule over five namespaces."

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-sem --lib --no-fail-fast` Expected: PASS.

- [ ] **Step 5: Run the corpus and the book**

Run: `cargo test -p ridlc --test corpus --no-fail-fast` and
`cargo test -p ridl-cli --test book_examples --no-fail-fast` Expected: PASS with
no snapshot change. A new RIDL-149 anywhere in the corpus or the book
contradicts design §5.5's measurement: stop and report it.

- [ ] **Step 6: Update the records that describe the check**

In `crates/ridl-core/src/diag.rs`, RIDL-149's doc comment: replace the whole
lines from `/// Scoped to the members of one interface, the parameters of one`
through `/// that collided. Emitted per-package by the checker (E9.7).`, keeping
the 8-space indentation of the lines around them, with:

```rust
/// Scoped to the members of one interface, the parameters of one
/// interaction (decision 4), the fields of one struct, which joined
/// in the commit where E9.8 started projecting them onto proto3, the
/// arms of one union, which joined with the ADR-0016 amendment of
/// 2026-09-20, and the values of one enum, which joined with the
/// amendment of 2026-09-26. The first three namespaces are checked
/// under `snake_case` alone; a union's arms are checked under
/// `snake_case` and `camel_case` both, because a union arm reaches
/// both namespaces and the two collision sets are incomparable; an
/// enum's values are checked under `pascal_case` alone, because its
/// collision set contains `snake_case`'s. The message names the
/// transform that collided. Emitted per-package by the checker (E9.7).
```

In `docs/specification/ridl-language-reference.md`, the RIDL-149 row's text
becomes:

> two names in one scope that collide after a pinned name transform — no
> transform is injective and no case-folding transform can be, so two names
> distinct in source can project to one identifier; scoped to the members of one
> interface, the parameters of one interaction, the fields of one struct, the
> arms of one union, and the values of one enum. The first three are checked
> under `snake_case`; a union's arms are checked under `snake_case` and
> `camel_case` both, because an arm reaches both namespaces and the two
> collision sets are incomparable; an enum's values are checked under
> `pascal_case` alone, because its collision set contains `snake_case`'s. The
> message names the transform that collided (§11; ADR-0016 decisions 3 and 4, as
> amended)

In ADR-0017 decision 5, after the 2026-09-20 paragraph, add:

```markdown
**Amended 2026-09-26 by driftsys/ridl#506.** The enum-value half is done
within one enum. RIDL-149 now covers an enum's values, keyed on `pascal_case`
alone — the Rust backend's variant spelling since the same change. Proto's
prefixed value is `snake_case` upper-cased, and every pair that collides under
`snake_case` collides under `pascal_case`, so within one enum the one key covers
this backend as well. Proto's namespace is wider than one enum: `enum A_B { C }`
beside `enum A { B_C }` both give `A_B_C`, and a declared `UNSPECIFIED` value in
an enum with no zero meets the synthesized `<PREFIX>_UNSPECIFIED`. Those stay
with decision 4's backend check, which is unchanged.
```

In ADR-0017's "Alternatives considered", the row "Extending RIDL-149 to enum
values and union arms" changes its verdict cell from "union arms taken
2026-09-20 (driftsys/ridl#451); enum values still deferred" to "union arms taken
2026-09-20 (driftsys/ridl#451); enum values taken 2026-09-26
(driftsys/ridl#506), within one enum".

Then `prim fmt` the two Markdown files and run `just check`.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-sem/src/check.rs crates/ridl-core/src/diag.rs docs/specification/ridl-language-reference.md docs/decisions/ADR-0017-proto3-projection-rules.md
git commit -m "feat(ridl-sem): check an enum's values under pascal_case with RIDL-149 (#506)"
```

---

### Task 4: The Rust backend spells enum variants in PascalCase

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — a helper beside `snake_of`
  (line 764); `emit_enum` at lines 1525-1545
- Modify: `crates/ridl-backend-rust/src/defaults.rs:279-286` (`enum_default`)
- Modify: `crates/ridl-backend-rust/src/codec.rs:957-960`
  (`Repr::Enum { first }`)
- Test: `crates/ridl-backend-rust/src/tests.rs`
- Snapshots: `crates/ridl-backend-rust/src/snapshots/*.snap` and
  `crates/ridlc/tests/snapshots/corpus__rust@*.snap`
- Regenerate: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
- Modify, where they name a generated variant:
  `crates/ridl-backend-rust/tests/interaction_face.rs` (around lines 345-390),
  `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs` (around lines
  59-287), `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs` (around
  lines 162-185), and assertions in `crates/ridl-backend-rust/src/tests.rs`
  (around lines 1405, 2180 — `Mode::SLOW` — 2909 and 2917)
- Modify: `examples/cabin/consumer/src/main.rs:81,92`
- Modify: `docs/technotes/ridl-rt-by-example.md:355`

**Interfaces:**

- Consumes: `v1::Spellings.pascal` (Task 2); `ridl_ir::name::camel_case`.
- Produces: `pub(crate) fn pascal_of(name: Option<&v1::Spellings>) -> String` in
  `lib.rs`.

- [ ] **Step 1: Write the failing tests**

In `crates/ridl-backend-rust/src/tests.rs`, add after `enum_with_discriminants`:

```rust
/// driftsys/ridl#506: a variant is spelled through `pascal_case`, at every
/// site that names one — the declaration, the `TryFrom` arm, and the
/// `Default` value.
#[test]
fn an_enum_variant_is_spelled_in_pascal_case() {
    let source = rust_for(vec![public_decl(
        "Warning",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: warning_bits(),
            reserved: Vec::new(),
        }),
    )]);
    assert!(source.contains("CheckEngine = 1"), "{source}");
    assert!(source.contains("Ok(Self::CheckEngine)"), "{source}");
    assert!(source.contains("Warning::LowFuel"), "{source}");
    assert!(!source.contains("CHECK_ENGINE"), "{source}");
}

/// Design §4: a model written by a toolchain older than `Spellings.pascal`
/// carries it empty, and the backend derives it from `snake`. The output
/// must be the same as from a current model.
#[test]
fn an_empty_pascal_spelling_is_derived_from_snake() {
    let package = package(
        "veh.common",
        vec![public_decl(
            "Warning",
            v2::decl::Kind::EnumDef(v2::EnumDef {
                values: warning_bits(),
                reserved: Vec::new(),
            }),
        )],
    );
    let model = ridl_ir::codegen::lower(&package, &[]);
    let mut older = model.clone();
    for decl in &mut older.declarations {
        if let Some(ridl_ir::codegen::v1::declaration::Kind::Enum(def)) = decl.kind.as_mut() {
            for value in &mut def.values {
                value
                    .name
                    .as_mut()
                    .expect("a lowered value is named")
                    .pascal
                    .clear();
            }
        }
    }
    let current = super::generate_pipeline_over(&model, super::WireEncoding::FlatBuffers)
        .expect("generation succeeds")
        .rust_source;
    let derived = super::generate_pipeline_over(&older, super::WireEncoding::FlatBuffers)
        .expect("generation succeeds")
        .rust_source;
    assert!(current.contains("CheckEngine"), "{current}");
    assert_eq!(derived, current);
}

/// Values outside the typl convention (Review Focus 1). `SELF` becomes
/// `Self`, which cannot be an identifier, and `ident()` escapes it to
/// `Self_`; `checkEngine` and `HTTPServer` become `CheckEngine` and
/// `HttpServer`. The generated enum must compile with
/// `non_camel_case_types` denied.
#[test]
fn an_enum_value_outside_the_typl_convention_compiles() {
    let rust_source = rust_for(vec![public_decl(
        "Direction",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![
                enum_value("SELF", 0),
                enum_value("checkEngine", 1),
                enum_value("HTTPServer", 2),
            ],
            reserved: Vec::new(),
        }),
    )]);
    assert!(rust_source.contains("Self_ = 0"), "{rust_source}");
    assert!(rust_source.contains("CheckEngine = 1"), "{rust_source}");
    assert!(rust_source.contains("HttpServer = 2"), "{rust_source}");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("outside_convention.rs");
    let meta_path = dir.path().join("outside_convention.rmeta");
    std::fs::write(&source_path, &rust_source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-D",
            "non_camel_case_types",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(status.success(), "generated Rust must compile:\n{rust_source}");
}

This follows `a_tuple_under_an_internal_declaration_is_package_private`
(line 1732), which writes the generated source with no prelude and links
`ridl_rt` through `ridl_rt_rlib` (line 2574).
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-backend-rust --lib -- an_enum_variant_is_spelled_in_pascal_case an_empty_pascal_spelling_is_derived_from_snake an_enum_value_outside_the_typl_convention_compiles`
Expected: all three FAIL; the output still spells `CHECK_ENGINE` and `SELF`.

- [ ] **Step 3: Write the implementation**

In `lib.rs`, after `snake_of`:

```rust
/// The pinned `pascal_case` of a declared name (ADR-0016, 2026-09-26
/// amendment), which spells an enum variant. A model written by a toolchain
/// older than `Spellings.pascal` carries it empty, and the field's contract
/// is that an empty value is `camel_case(snake)` (design §4), so that is
/// what this returns for one.
pub(crate) fn pascal_of(name: Option<&v1::Spellings>) -> String {
    match name {
        Some(name) if !name.pascal.is_empty() => name.pascal.clone(),
        Some(name) => ridl_ir::name::camel_case(&name.snake),
        None => String::new(),
    }
}
```

In `emit_enum`, change the doc comment's second line to "Variant names are the
pinned `pascal_case` of the typl name (ADR-0016, 2026-09-26 amendment), so
`CHECK_ENGINE` becomes `CheckEngine`." and both
`let vname = ident(declared(value.name.as_ref()));` lines to:

```rust
let vname = ident(&pascal_of(value.name.as_ref()));
```

In `defaults.rs` `enum_default`:

```rust
let variant = ident(&pascal_of(chosen.name.as_ref()));
```

and add `pascal_of` to that file's `use crate::{...}` import (match the form the
file uses for `declared`).

In `codec.rs`, in the `Repr::Enum` built from `def.values.first()`:

```rust
repr: Repr::Enum {
    name: owner.clone(),
    // The variant `emit_enum` declared, which is the
    // pinned `pascal_case` spelling.
    first: pascal_of(first.name.as_ref()),
},
```

and add `pascal_of` to that file's import of crate helpers.

- [ ] **Step 4: Run the three tests to verify they pass**

Run the command from Step 2. Expected: PASS.

- [ ] **Step 5: Move the snapshots and the checked-in face**

Run:

```bash
INSTA_UPDATE=always cargo test -p ridl-backend-rust --no-fail-fast
INSTA_UPDATE=always cargo test -p ridlc --test corpus --no-fail-fast
RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face
```

Read the diff: `git diff -- '*.snap' crates/ridl-backend-rust/tests/generated/`.
Every changed line must be a variant renamed from its typl spelling to its
`pascal_case` spelling (`LOW_FUEL` → `LowFuel`, `OK` → `Ok`), in a variant
declaration, a `TryFrom` arm, a `Default` value, or a codec's `.unwrap_or(...)`.
An enumset's `pub const LOW_FUEL` must not change. Check that with
`git diff -- '*.snap' | grep '^[-+].*pub const'`, which must print nothing.

Then update the hand-written tests that name a generated variant in its typl
spelling, listed under **Files**. Each such name becomes its `pascal_case`
spelling (`Health::WARN` → `Health::Warn`). An assertion on a string that holds
the ridl source keeps the typl spelling. Run
`cargo test -p ridl-backend-rust --no-fail-fast` and expect PASS.

- [ ] **Step 6: Update the consumers of the generated names**

In `examples/cabin/consumer/src/main.rs`, lines 81 and 92: `api::Health::WARN`
becomes `api::Health::Warn`.

In `docs/technotes/ridl-rt-by-example.md`, line 355: `health: Health::WARN`
becomes `health: Health::Warn`. Leave lines 71-73 as they are: they are the ridl
source, which keeps its `SCREAMING_SNAKE` spelling.

Then search for any other consumer:
`git grep -nE '::[A-Z][A-Z0-9_]*[A-Z0-9]\b' -- '*.rs' '*.md' ':!docs/archive' ':!docs/wip' ':!*.snap'`
(every path segment of two or more capitals, which catches single-word variants
such as `Mode::SLOW` as well as multi-word ones) and check each hit. A hit that
is a generated enum variant is updated. A constant (`MAX_BUFFER_SIZE`), an
enumset bit, or a hand-written Rust item stays.

- [ ] **Step 7: Run the suites and the demo**

Run: `cargo test --workspace --locked --no-fail-fast` and `just demo` Expected:
PASS, and the demo matches every round trip.

- [ ] **Step 8: Commit**

```bash
git add crates/ridl-backend-rust crates/ridlc/tests/snapshots examples/cabin/consumer/src/main.rs docs/technotes/ridl-rt-by-example.md
git commit -F - <<'EOF'
fix(ridl-backend-rust)!: spell enum variants through pascal_case (#506)

A typl enum value was emitted verbatim as a Rust variant, so every
multi-word value drew non_camel_case_types in a consumer's build. The
variant is now the pinned pascal_case of the typl name: CHECK_ENGINE
becomes CheckEngine and PARK becomes Park.

BREAKING CHANGE: every generated enum variant is renamed. Code that
names a variant, as in Health::WARN, must use the new spelling,
Health::Warn.
EOF
```

---

### Task 5: The compile proofs deny `non_camel_case_types`

**Files:**

- Modify: `crates/ridl-backend-rust/src/tests.rs` —
  `a_tuple_under_an_internal_declaration_is_package_private` (line 1732; comment
  at 1877-1889, args at 1902-1907), `constructible_collections_compile` (line
  2977; args at 3036-3037), `appendix_b_compiles_with_rustc` (line 2816; comment
  at 2854-2862, args at 2870-2872) and `appendix_a_compiles_with_rustc` (line
  3427; comment at 3474-3479, args at 3489-3490)
- Modify: `crates/ridlc/tests/corpus.rs` — the doc comment on
  `veh_common_generated_rust_compiles_with_rustc` (lines 1088-1094) and that
  proof's own `rustc` command (the `"metadata",` argument at line 1122),
  `workspace_two_members_composed_compiles_with_rustc`'s own `rustc` command
  (the `"metadata",` argument at line 1241), and `rustc_accepts` (line 1266; its
  doc comment at 1258-1262, the two comments at 1289-1298 and 1302-1308, args at
  1309-1310)
- Modify: `crates/ridl-backend-rust/tests/interaction_face.rs:69-74` (remove the
  `clippy::upper_case_acronyms` allowance)

- [ ] **Step 1: Add the deny flag to the seven proofs**

Seven rustc invocations are the compile proofs this task changes (design §2):
the five that deny a lint by name and the two corpus proofs that build their own
command. Other tests also compile generated Rust — around
`crates/ridl-backend-rust/src/tests.rs:1213` and `:2949`,
`crates/ridl-backend-rust/tests/rust_crate_emit.rs:46` and
`crates/ridl/tests/cabin_example.rs:73` — and deny no lint; they stay as they
are, because the four proofs whose fixtures hold multi-word values guard #506.
In the five `rustc` argument lists that already hold `"non_snake_case",` — the
four in `crates/ridl-backend-rust/src/tests.rs` and `rustc_accepts` — add after
it:

```rust
"-D",
"non_camel_case_types",
```

`veh_common_generated_rust_compiles_with_rustc` and
`workspace_two_members_composed_compiles_with_rustc` build their own command and
deny no lint today. In each, add after `"metadata",`:

```rust
// driftsys/ridl#506: an enum variant is the `pascal_case` of its
// typl name, and this deny keeps it there. `veh-common` holds
// multi-word enum values, so it bites on this fixture.
"-D",
"non_camel_case_types",
```

For the workspace proof, the comment's last sentence becomes "It is inert on
this fixture, which declares no enum
(`crates/ridlc/tests/corpus/workspace-two-members/`); it keeps this a proof if
one is added."

Replace the comments:

- `constructible_collections_compile`: no comment names the variant spelling;
  leave its comments as they are.
- `a_tuple_under_an_internal_declaration_is_package_private`: "the generated
  code carries by-design naming and dead-code lints" becomes "the generated code
  carries dead-code lints", and the last sentence, from "An enum variant keeps
  its typl `SCREAMING_SNAKE` spelling", becomes "`non_camel_case_types` is
  denied too, for driftsys/ridl#506; it is inert on this fixture, which declares
  no enum." (Its fixture, lines 1732-1876, builds no `EnumDef`.)
- `veh_common_generated_rust_compiles_with_rustc`'s doc comment: "(for example
  `non_camel_case_types` on a screaming-case enum variant)" becomes "(for
  example dead code on an unused internal type)".
- `rustc_accepts`'s doc comment: "(`non_camel_case_types` on a screaming-case
  enum variant, dead code on an unused internal type)" becomes "(dead code on an
  unused internal type)".
- `appendix_b_compiles_with_rustc`: the sentences from "An enum variant keeps
  its typl `SCREAMING_SNAKE` spelling" to the end of the comment become: "An
  enum variant is the `pascal_case` of its typl name (driftsys/ridl#506), and
  `non_camel_case_types` is denied too. Appendix B holds multi-word enum values,
  so this deny is the proof that guards #506 on this fixture."
- `appendix_a_compiles_with_rustc`: the sentence "`non_camel_case_types`, which
  a screaming-case enum variant draws by design, stays undenied." becomes
  "`non_camel_case_types` is denied too, for driftsys/ridl#506. Appendix A's
  `DiagError` holds multi-word values (`FILTER_INVALID`, `STORAGE_BUSY`,
  `ACCESS_DENIED`), so the deny bites on this fixture."
- `rustc_accepts`, first comment in the argument list: "(`non_camel_case_types`
  on a screaming-case enum variant, dead code in a crate with no consumers)"
  becomes "(dead code in a crate with no consumers)".
- `rustc_accepts`, second comment in the argument list: "`non_camel_case_types`,
  which a screaming-case enum variant draws by design, stays undenied." becomes
  "The same holds for an enum variant and `non_camel_case_types`
  (driftsys/ridl#506): the variant goes through `ridl_ir::name::pascal_case`.
  `veh-cluster` holds multi-word enum values, so the deny bites on it."
- `rustc_accepts`'s doc comment, and the first comment in its argument list,
  count the lints denied by name: "Three lints are denied by name" becomes "Four
  lints are denied by name", the list after it gains "and `non_camel_case_types`
  (issue #506)", and "The first of the three lints" becomes "The first of the
  four lints".

If a deny fails on a name that is not an enum variant, stop and report it: the
design assumed no other generated type name draws the lint.

- [ ] **Step 2: Remove the lint allowance the old spelling needed**

Delete the `#[allow(clippy::upper_case_acronyms, reason = "...")]` attribute at
`crates/ridl-backend-rust/tests/interaction_face.rs:69-74`. Line 68 closes the
`clippy::derivable_impls` attribute before it, which stays.

- [ ] **Step 3: Run the proofs and the lint**

Run:
`cargo test -p ridl-backend-rust -p ridlc --no-fail-fast -- compiles_with_rustc constructible_collections_compile a_tuple_under_an_internal_declaration_is_package_private`
and `just lint`. Expected: PASS for every proof, and clippy clean.

- [ ] **Step 4: Prove the deny bites**

Revert the spelling at every site at once, so that the generated code stays
consistent and only the lint can fail it: change the body of `pascal_of` to
`name.map(|name| name.declared.clone()).unwrap_or_default()`. Reverting one site
alone is not a test of the deny — the other sites still name the PascalCase
variant, and rustc fails with E0599 whether or not the lint is denied. Run
`cargo test -p ridl-backend-rust -p ridlc --no-fail-fast -- compiles_with_rustc`.
Expected: `appendix_a_compiles_with_rustc`, `appendix_b_compiles_with_rustc`,
`veh_common_generated_rust_compiles_with_rustc` and
`veh_cluster_generated_rust_compiles_with_rustc` FAIL, each reporting
`non_camel_case_types`. Any other failure is recorded with its reason. Then
restore `pascal_of` and rerun: PASS. Record the failing test names for the pull
request.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/src/tests.rs crates/ridlc/tests/corpus.rs crates/ridl-backend-rust/tests/interaction_face.rs
git commit -m "test(ridl-backend-rust): deny non_camel_case_types in the rustc compile proofs (#506)"
```

---

### Task 6: Mutation check, gate, gardening and pull request

**Files:**

- Move: `docs/wip/2026-09-26-enum-variant-pascal-case-{design,plan}.md` to
  `docs/archive/`
- Modify: `docs/wip/README.md`, `docs/archive/README.md`,
  `docs/decisions/ADR-0016-schema-projection-and-the-name-transform.md`,
  `docs/decisions/README.md`

- [ ] **Step 1: Mutation check of the four emit sites and the check**

For each mutation below, apply it alone, run
`cargo test -p ridl-ir -p ridl-sem -p ridl-backend-rust -p ridlc --no-fail-fast 2>&1 | grep -E '^test .* FAILED|panicked' | head`,
record which tests fail, and revert it:

1. `emit_enum` variant declaration back to `declared`.
2. `emit_enum` `TryFrom` arm back to `declared`.
3. `enum_default` back to `declared`.
4. `codec.rs` `first:` back to `declared(...).to_string()`.
5. `pascal_of`'s fallback arm returning `name.snake.clone()`.
6. `check_enum_value_projection` keyed on `snake_case` instead of `pascal_case`.
7. `pascal_case` returning `camel_case(name)`.

Each mutation must fail at least one test. A mutation no test catches is a gap:
add the test to the task that owns the code, then rerun.

- [ ] **Step 2: Run the full gate**

Run: `just verify` Expected: every recipe passes.

- [ ] **Step 3: Garden the design and plan**

Invoke the `sdd-gardening` skill. The expected result: the design and plan move
verbatim to `docs/archive/`; ADR-0016's 2026-09-26 amendment drops "The decision
is taken; the code lands from that design's plan, and until it merges the Rust
backend still emits the typl spelling and RIDL-149 does not cover an enum's
values." and links the archived design instead; the ADR-0016 summary in
`docs/decisions/README.md` drops "Decided; the code change follows from its
plan."; the two rows ADR-0016's "Documents amended" table marks "changes with
the check, not before it" drop that clause; `docs/wip/README.md` loses the entry
and `docs/archive/README.md` gains one. Then grep for dead citations that
link-check cannot see:
`git grep -n "2026-09-26-enum-variant-pascal-case" -- ':!docs/archive'` and fix
every hit in a `.rs`, `.ridl` or Markdown file. Run `just link-check`,
`just doc-path-check` and `just check`, one at a time (`doc-path-check` takes an
optional argument, so a second recipe name on the same line would be read as
that argument).

- [ ] **Step 4: Commit and open the pull request**

```bash
git add -A docs/
git commit -m "docs(docs): archive the enum variant PascalCase design and plan (#506)"
git push -u origin HEAD
```

Open the pull request with a body that says "Closes #506", lists the three
compile proofs Task 5 Step 4 recorded, and the mutation results of Step 1.
Before merging, check that the body names no other issue after a closing
keyword:
`gh pr view --json body -q .body | grep -niE '(close|fix|resolve)[sd]? #'`.
