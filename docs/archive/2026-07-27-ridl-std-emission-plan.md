# `ridl.std` Emission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the raw output of `ridlc build` compile, by emitting the
`ridl.std` artifact when the workspace references it.

**Architecture:** Four independent tasks. Task 1 adds a recursive, fail-closed
reference walk to `ridl-ir`. Task 2 uses it in `run_build` to emit the standard
package. Task 3 deletes the hand-written stand-ins so the corpus proofs compile
what the command actually emits — this is what verifies task 2. Task 4 pins the
embedded asset to the normative Appendix A. Tasks 1 and 4 are independent of
each other; task 2 needs task 1; task 3 needs task 2.

**Tech Stack:** Rust workspace (`cargo`, `insta` snapshots, `prost`/`protox` for
the IR), `just` for every gate, `prim` and `markdownlint` for Markdown.

**Design:** `docs/wip/2026-07-27-ridl-std-emission-design.md`

## Global Constraints

- Commit scopes are an explicit list in `.git-std.toml`, not path-derived. The
  scopes used by this plan are `ridl-ir`, `ridlc`, `ridl-core`, and `typl`.
  `git std lint` rejects anything else, in a pre-commit hook.
- Never push to `main`; this work lands via a PR from `fix/emit-ridl-std`.
- Prose — comments, commit messages, docs — is plain and literal: no idioms, no
  figures of speech. Technical terms stay as they are.
- `just build` is the full gate and must pass before the final commit.
- **The exhaustive-match rule is normative** (design §3.3): every `oneof` match
  in the walk of task 1 lists every variant and has **no wildcard (`_`) arm**. A
  variant carrying no reference gets an explicit empty arm with a comment saying
  so. A wildcard would silently disarm the fail-closed property that the design
  relies on.

---

### Task 1: The reference walk in `ridl-ir`

**Files:**

- Modify: `crates/ridl-ir/src/lib.rs` — add `referenced_packages` inside
  `pub mod v2`, and its tests in the existing `mod tests`.

**Interfaces:**

- Consumes: nothing from other tasks.
- Produces: `ridl_ir::v2::referenced_packages(&Package) -> BTreeSet<String>`.
  Returns the qualifier of every dotted type reference in the package. A bare
  reference (same-package) contributes nothing.

**The reference graph** — the walk must reach all of it:

```text
Package
  decls[]                  -> Decl
  interfaces[].interactions[] -> Decl
  services[].shape         -> interface_ref(String) | inline(Interface)

Decl.kind
  type_def      -> TypeDef
  const_def     -> ConstDef.type_ref (Option<String>)
  struct_def    -> members[].member -> field(Field.type: FieldType) | reserved
  enum_def      -> no reference
  enum_set_def  -> backing_enum (Option<String>)
  union_def     -> arms[].type_ref (String)
  signal_def    -> payload (String)
  event_def     -> payload (String)
  command_def   -> params[].type (FieldType)
  query_def     -> params[].type, return_type
  fixed_def     -> payload (FieldType)
  reserved_slot -> no reference

TypeDef.constraint.pattern_const (Option<String>)   // names a const, may be qualified
FieldType.kind
  named(String) | primitive | inline_scalar(TypeDef)
  tuple(TupleType.fields[].type) | array(ArrayType.element)
  map(MapType.key, MapType.value) | stream(StreamType)
StreamType.element -> named(String) | primitive
ReturnType.kind   -> value(FieldType) | fallible(FallibleType.ok, .err)
```

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `crates/ridl-ir/src/lib.rs`:

```rust
/// A dotted reference contributes its qualifier; a bare one contributes
/// nothing. The nested case is the one that matters: a reference reachable
/// only through an array element is still a reference.
#[test]
fn referenced_packages_finds_qualifiers_at_depth() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        decls: vec![
            v2::Decl {
                name: "Local".to_string(),
                kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "Speed".to_string(),
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Stamped".to_string(),
                kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef {
                    payload: "ridl.std.Timestamp".to_string(),
                    ..Default::default()
                })),
                ..Default::default()
            },
            v2::Decl {
                name: "Labels".to_string(),
                kind: Some(v2::decl::Kind::FixedDef(v2::FixedDef {
                    payload: Some(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                            element: Some(Box::new(v2::FieldType {
                                kind: Some(v2::field_type::Kind::Named(
                                    "ridl.std.Label".to_string(),
                                )),
                                ..Default::default()
                            })),
                            min: 0,
                            max: 32,
                        }))),
                        ..Default::default()
                    }),
                })),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let found = v2::referenced_packages(&package);
    assert!(found.contains("ridl.std"), "got {found:?}");
    assert!(
        !found.contains("Speed") && !found.contains("veh.cluster"),
        "a bare reference contributes no package: {found:?}",
    );
    assert_eq!(found.len(), 1, "only the qualifier is reported: {found:?}");
}

/// An empty package references nothing — the negative case the emit rule in
/// `ridlc` depends on.
#[test]
fn referenced_packages_is_empty_without_references() {
    let package = v2::Package {
        name: "veh.solo".to_string(),
        ..Default::default()
    };
    assert!(v2::referenced_packages(&package).is_empty());
}
```

> **Note on shapes:** `ArrayType.element` and `FieldType.array` are boxed by
> `prost` because the type is recursive. If the compiler disagrees with a
> `Box::new` above, follow the compiler — the generated types are the authority,
> and the assertions are what matter.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --locked -p ridl-ir referenced_packages` Expected: FAIL to
compile — `cannot find function referenced_packages`.

- [ ] **Step 3: Write the walk**

Add inside `pub mod v2` in `crates/ridl-ir/src/lib.rs`. Write every `match`
without a wildcard arm; the compiler will name any variant left out, which is
the point. For each variant, recurse into whatever it carries and give a
reference-free variant an explicit empty arm with a comment.

```rust
    /// Every package named by a type reference in `package`.
    ///
    /// A resolved type-reference string is the fully qualified `pkg.Name` for
    /// a cross-package reference and the bare `Name` for a same-package one,
    /// never an import alias — the canonical form stated in
    /// `proto/ridl/ir/v2/ir.proto`, which also enumerates the fields carrying
    /// one. **That enumeration and this walk are edited together.** A
    /// reference-bearing field added there and not read here makes the package
    /// it names invisible to every caller asking what a package depends on.
    ///
    /// Every `oneof` below is matched exhaustively with no wildcard arm, so a
    /// variant added later fails to compile here rather than going unread.
    pub fn referenced_packages(package: &Package) -> std::collections::BTreeSet<String> {
        let mut found = std::collections::BTreeSet::new();
        for decl in &package.decls {
            walk_decl(decl, &mut found);
        }
        for interface in &package.interfaces {
            for interaction in &interface.interactions {
                walk_decl(interaction, &mut found);
            }
        }
        for service in &package.services {
            match &service.shape {
                Some(service::Shape::InterfaceRef(reference)) => qualifier(reference, &mut found),
                Some(service::Shape::Inline(interface)) => {
                    for interaction in &interface.interactions {
                        walk_decl(interaction, &mut found);
                    }
                }
                None => {}
            }
        }
        found
    }

    /// Records the package qualifier of a dotted reference. A bare reference
    /// is same-package and contributes nothing.
    fn qualifier(reference: &str, found: &mut std::collections::BTreeSet<String>) {
        if let Some((package, _)) = reference.rsplit_once('.') {
            found.insert(package.to_string());
        }
    }
```

Then the four helpers. Note `r#type`: the proto field is `type`, which `prost`
escapes.

```rust
    /// Records every reference in one declaration — a package-level one or an
    /// interaction inside an interface, which share the `Decl` envelope.
    fn walk_decl(decl: &Decl, found: &mut std::collections::BTreeSet<String>) {
        match &decl.kind {
            Some(decl::Kind::TypeDef(type_def)) => walk_type_def(type_def, found),
            Some(decl::Kind::ConstDef(const_def)) => {
                if let Some(reference) = &const_def.type_ref {
                    qualifier(reference, found);
                }
            }
            Some(decl::Kind::StructDef(struct_def)) => {
                for member in &struct_def.members {
                    match &member.member {
                        Some(struct_member::Member::Field(field)) => {
                            if let Some(field_type) = &field.r#type {
                                walk_field_type(field_type, found);
                            }
                        }
                        // A tombstone occupies an ordinal and names no type.
                        Some(struct_member::Member::Reserved(_)) | None => {}
                    }
                }
            }
            // An enum's variants are integers; it names no type.
            Some(decl::Kind::EnumDef(_)) => {}
            Some(decl::Kind::EnumSetDef(enum_set)) => {
                if let Some(reference) = &enum_set.backing_enum {
                    qualifier(reference, found);
                }
            }
            Some(decl::Kind::UnionDef(union_def)) => {
                for arm in &union_def.arms {
                    qualifier(&arm.type_ref, found);
                }
            }
            Some(decl::Kind::SignalDef(signal)) => qualifier(&signal.payload, found),
            Some(decl::Kind::EventDef(event)) => qualifier(&event.payload, found),
            Some(decl::Kind::CommandDef(command)) => {
                for param in &command.params {
                    if let Some(field_type) = &param.r#type {
                        walk_field_type(field_type, found);
                    }
                }
            }
            Some(decl::Kind::QueryDef(query)) => {
                for param in &query.params {
                    if let Some(field_type) = &param.r#type {
                        walk_field_type(field_type, found);
                    }
                }
                if let Some(return_type) = &query.return_type {
                    walk_return_type(return_type, found);
                }
            }
            Some(decl::Kind::FixedDef(fixed)) => {
                if let Some(field_type) = &fixed.payload {
                    walk_field_type(field_type, found);
                }
            }
            // A tombstone occupies an ordinal and names no type.
            Some(decl::Kind::ReservedSlot(_)) | None => {}
        }
    }

    /// The recursive half: a reference is reachable at arbitrary depth through
    /// tuples, arrays, maps, inline scalars, and streams.
    fn walk_field_type(field_type: &FieldType, found: &mut std::collections::BTreeSet<String>) {
        match &field_type.kind {
            Some(field_type::Kind::Named(reference)) => qualifier(reference, found),
            // A primitive names no package.
            Some(field_type::Kind::Primitive(_)) => {}
            Some(field_type::Kind::InlineScalar(type_def)) => walk_type_def(type_def, found),
            Some(field_type::Kind::Tuple(tuple)) => {
                for field in &tuple.fields {
                    if let Some(inner) = &field.r#type {
                        walk_field_type(inner, found);
                    }
                }
            }
            Some(field_type::Kind::Array(array)) => {
                if let Some(element) = &array.element {
                    walk_field_type(element, found);
                }
            }
            Some(field_type::Kind::Map(map)) => {
                if let Some(key) = &map.key {
                    walk_field_type(key, found);
                }
                if let Some(value) = &map.value {
                    walk_field_type(value, found);
                }
            }
            Some(field_type::Kind::Stream(stream)) => match &stream.element {
                Some(stream_type::Element::Named(reference)) => qualifier(reference, found),
                // STRING or BYTES only; names no package.
                Some(stream_type::Element::Primitive(_)) | None => {}
            },
            None => {}
        }
    }

    /// A `TypeDef`'s only reference is the constant a `match` bound names.
    fn walk_type_def(type_def: &TypeDef, found: &mut std::collections::BTreeSet<String>) {
        if let Some(constraint) = &type_def.constraint
            && let Some(reference) = &constraint.pattern_const
        {
            qualifier(reference, found);
        }
    }

    fn walk_return_type(return_type: &ReturnType, found: &mut std::collections::BTreeSet<String>) {
        match &return_type.kind {
            Some(return_type::Kind::Value(field_type)) => walk_field_type(field_type, found),
            Some(return_type::Kind::Fallible(fallible)) => {
                qualifier(&fallible.ok, found);
                qualifier(&fallible.err, found);
            }
            None => {}
        }
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --locked -p ridl-ir` Expected: PASS.

- [ ] **Step 5: Prove the fail-closed property by hand**

Temporarily add a `_ => {}` arm to the `Decl::kind` match and confirm the code
still compiles — that is what the rule forbids. Then remove it, delete one
variant arm instead, and confirm the compiler reports the missing variant.
Restore the correct code. Record what you saw in the task's commit message.

- [ ] **Step 6: Lint and commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/ridl-ir/src/lib.rs
git commit -m "feat(ridl-ir): add a fail-closed reference walk over a package"
```

---

### Task 2: Emit `ridl.std` from `run_build`

**Files:**

- Modify: `crates/ridlc/src/lib.rs` — the emit block inside `run_build`.
- Test: `crates/ridlc/tests/cli.rs` (add to the existing CLI test file; if the
  golden CLI tests live elsewhere, add beside them rather than creating a file).

**Interfaces:**

- Consumes: `ridl_ir::v2::referenced_packages` from task 1.
- Produces: `ridlc build` writes `ridl.std.<ext>` into `--out-dir` for each
  selected emit kind, when and only when some checked package references
  `ridl.std`.

- [ ] **Step 1: Write the failing test**

Two cases, positive and negative. The negative one is the only guard against a
detection rule that reports every package.

`crates/ridlc/tests/cli.rs` already provides everything these need: `TempDir`
with `new(label)`, `path()` and `write(relative, text)`, the
`ridlc(args) ->
(i32, String)` runner, and the `PACKAGE_MANIFEST` constant.
Follow the shape of `build_package_writes_generated_rust`. Add no dependency.

```rust
/// A workspace that names a standard type gets the standard artifact beside
/// its packages, for each selected emit kind. The corpus entry is used rather
/// than a fixture because it is the same input the compile proofs use.
#[test]
fn build_emits_the_standard_package_when_referenced() {
    let out = TempDir::new("emits-std-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        "tests/corpus/veh-cluster".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "typescript,rust".as_ref(),
    ]);
    assert_eq!(code, 0, "the corpus entry must exit 0, stderr:\n{stderr}");
    assert!(
        out.path().join("ridl.std.rs").is_file(),
        "the Rust standard artifact must be written beside the packages",
    );
    assert!(
        out.path().join("ridl.std.ts").is_file(),
        "the TypeScript standard artifact must be written beside the packages",
    );
}

/// A workspace naming no standard type gets no standard artifact. This is the
/// only guard against a detection rule that reports every package: without it,
/// "always emit" would pass the test above.
#[test]
fn build_omits_the_standard_package_when_unreferenced() {
    let dir = TempDir::new("omits-std");
    dir.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    dir.write(
        "pkg/counter.typl",
        "package veh.common\ntype Counter : integer [0..65535]\n",
    );
    let out = TempDir::new("omits-std-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "rust".as_ref(),
    ]);
    assert_eq!(code, 0, "a clean package must exit 0, stderr:\n{stderr}");
    assert!(
        out.path().join("veh.common.rs").is_file(),
        "the package itself is still emitted",
    );
    assert!(
        !out.path().join("ridl.std.rs").exists(),
        "no standard artifact for a workspace that references none",
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --locked -p ridlc standard_package` Expected: the positive test
FAILS (no `ridl.std.rs`); the negative test passes already, and must keep
passing.

- [ ] **Step 3: Emit the standard package**

In `run_build`, inside the existing `if succeeded { … }` block, after the
per-package loop:

```rust
// `ridl.std` is deliberately absent from `checked` (it is not a
// workspace member), so the loop above never reaches it. A
// consumer's generated code still references it, so the build
// writes it whenever the workspace names something from it —
// otherwise the raw output does not compile (issue #190).
let references_std = checked
    .iter()
    .any(|package| ridl_ir::v2::referenced_packages(&package.ir).contains("ridl.std"));
if references_std {
    let std_ir = check_package(&db, workspace, std, std).ir;
    write_emits(out_dir, "ridl.std", &std_ir, emits, &mut diagnostics)?;
}
```

`Compiled` already destructures `workspace` and `checked`; add `std` to that
destructuring so the standard package is in scope.

- [ ] **Step 4: Run the tests**

Run: `cargo test --locked -p ridlc` Expected: PASS, both new tests and the
existing suite.

- [ ] **Step 5: Verify by hand what a user gets**

```bash
cargo run --quiet --bin ridlc -- build crates/ridlc/tests/corpus/veh-cluster \
  --out-dir /tmp/ridl190 --emit typescript,rust
ls /tmp/ridl190
```

Expected: `ridl.std.rs` and `ridl.std.ts` beside the four package files.

- [ ] **Step 6: Commit**

```bash
git add crates/ridlc/src/lib.rs crates/ridlc/tests/
git commit -m "fix(ridlc): emit ridl.std when the workspace references it"
```

---

### Task 3: The corpus proofs consume the emitted standard package

This task is what verifies task 2. Until the stand-ins are gone, nothing proves
the emitted artifact is sufficient.

**Files:**

- Modify: `crates/ridlc/tests/corpus.rs` — `generated_packages`,
  `generated_typescript_packages`, and the three `PRELUDE` constants at roughly
  lines 626, 745, and 859.
- Delete: `crates/ridlc/tests/tsc/ridl.std.ts`.

**Interfaces:**

- Consumes: the emit behaviour from task 2;
  `check_package(&db, workspace, std, std)` for the in-process lowering, which
  `compile_entry` and `generated_packages` already have `std` and `workspace` in
  scope for.
- Produces: no new API.

- [ ] **Step 1: Include the standard package in the generated Rust set**

In `generated_packages`, generate `ridl.std` alongside the workspace members:

```rust
let std_checked = check_package(&db, workspace, std, std);
let std_generated = ridl_backend_rust::generate(&std_checked.ir)
    .expect("the standard package generates Rust");
let mut generated = vec![("ridl.std".to_string(), std_generated.rust_source)];
```

then extend `generated` with the per-package results and return it. Because
`ModuleTree` nests by dotted name, `ridl.std` composes as
`pub mod ridl { pub mod std { … } }` — exactly what `PRELUDE` declared by hand.

- [ ] **Step 2: Delete the three `PRELUDE` constants**

Remove each constant and the `format!("{PRELUDE}\n{…}")` that used it, leaving
the composed source alone. For `veh_common_generated_rust_compiles_with_rustc`,
which compiles one package rather than a composition, compose that package with
the generated `ridl.std` instead of prepending the constant.

- [ ] **Step 3: Run the rustc proofs**

Run: `cargo test --locked -p ridlc --test corpus rustc` Expected: PASS, with no
prelude anywhere in the file.

- [ ] **Step 4: Include the standard package in the generated TypeScript set**

In `generated_typescript_packages`, prepend `("ridl.std", …)` the same way,
using `ridl_backend_ts::generate(&std_checked.ir).expect(…).source`.

- [ ] **Step 5: Stop reading the stand-in**

In the tsc proof, delete the `std::fs::read_to_string("tests/tsc/ridl.std.ts")`
block and its `std::fs::write`. The generated `ridl.std.ts` is now written by
the same loop that writes every other module.

- [ ] **Step 6: Delete the stand-in file**

```bash
git rm crates/ridlc/tests/tsc/ridl.std.ts
```

- [ ] **Step 7: Run the tsc proofs**

Run: `cargo test --locked -p ridlc --test corpus typescript` Expected: PASS. If
`tsc` is not installed the proofs skip; install it or note the skip, because
this is the task's main evidence.

- [ ] **Step 8: Confirm nothing hand-written remains**

```bash
grep -rn "PRELUDE\|stand-in" crates/ridlc/tests/ || echo "clean"
```

Expected: `clean`.

- [ ] **Step 9: Commit**

```bash
git add -A crates/ridlc/tests/
git commit -m "test(ridlc): compile the emitted ridl.std instead of a stand-in"
```

---

### Task 4: Pin the embedded asset to Appendix A

Independent of tasks 1 to 3; may be done first.

**Files:**

- Modify: `crates/ridl-core/src/std_lib.rs` — add the test to its `mod tests`.

**Interfaces:**

- Consumes: `RIDL_STD_SOURCE`, already public in that module.
- Produces: no new API.

- [ ] **Step 1: Write the failing test**

The appendix block is the fenced `ridl` block whose body begins
`package ridl.std`. Locate it by that marker rather than by line number.

````rust
    /// The asset is the normative Appendix A, committed verbatim. Nothing
    /// enforced that until this test: both were edited by hand in #198, and
    /// every gate would have passed had only one been.
    ///
    /// This matters more since the standard package became a shipped artifact
    /// (issue #190). An asset that has drifted from the appendix now generates
    /// code that disagrees with the specification.
    #[test]
    fn the_asset_is_appendix_a_verbatim() {
        let reference = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/specification/typl-language-reference.md"),
        )
        .expect("the typl reference is readable");

        let block = reference
            .split("```ridl\n")
            .find(|block| block.starts_with("package ridl.std\n"))
            .and_then(|block| block.split("\n```").next())
            .expect("Appendix A carries a `ridl.std` fenced block");

        assert_eq!(
            format!("{block}\n"),
            RIDL_STD_SOURCE,
            "`crates/ridl-core/assets/ridl_std.typl` and Appendix A of \
             `docs/specification/typl-language-reference.md` have drifted apart. \
             They are the same normative text and are edited together.",
        );
    }
````

- [ ] **Step 2: Run it**

Run: `cargo test --locked -p ridl-core the_asset_is_appendix_a` Expected: PASS —
the two agree today, so this step confirms the extraction finds the right block
rather than comparing two empty strings.

- [ ] **Step 3: Prove the test discriminates**

Temporarily add a line to `crates/ridl-core/assets/ridl_std.typl`, re-run, and
confirm it FAILS naming both files. Revert. A test that cannot fail is worse
than none.

- [ ] **Step 4: Commit**

```bash
git add crates/ridl-core/src/std_lib.rs
git commit -m "test(ridl-core): pin the embedded ridl.std asset to Appendix A"
```

---

### Task 5: Gate, document, and open the PR

- [ ] **Step 1: Run the whole gate**

Run: `just build` Expected: exit 0.

- [ ] **Step 2: Check the success criteria in the design**

Work `docs/wip/2026-07-27-ridl-std-emission-design.md` §6 item by item and
confirm each. Criterion 1 is the user-facing one: build the corpus into a temp
directory and compile the raw output with `tsc --strict` and with `rustc`,
adding nothing.

- [ ] **Step 3: Run `just verify` and open the PR**

```bash
just verify
git push -u origin fix/emit-ridl-std
```

Open the PR against `main`, closing #190. Note in the body that CI is blocked on
the GitHub billing gate, so the evidence is the local gate plus the raw output
compiling.

- [ ] **Step 4: Garden the working memory**

This design and plan are working memory. When the PR lands, move both from
`docs/wip/` to `docs/archive/` verbatim, as the `fixed` rename did.

---

## Execution outcome

The plan was executed task by task, each with its own review. Four things it did
not anticipate, recorded because they are what the next plan should expect:

- **Task 1's test, as written in this plan, could not fail.** Both fixture
  declarations resolved to the same qualifier `ridl.std`, so the length
  assertion held even with the array-recursion arm gutted. The fix gave every
  recursive path its own distinct qualifier and asserted the exact set. A test
  written in a plan gets no compiler to check it; state the discrimination
  requirement in the step, as Task 1 step 5 did for the exhaustive match.
- **A repo guard fired that no per-crate test could see.** `xtask`'s
  `every_direct_interfaces_read_is_justified` counts direct `.interfaces` reads
  and requires each to be justified in a table. The walk added one. Per-task
  subagents ran only their own crate's tests, so this surfaced at the first full
  `just build`. Run the whole gate earlier when a task adds a read of a guarded
  shape.
- **The emit re-opened a defect in a different command.** Passing the caller's
  whole `emits` slice meant `ridl baseline` published `ridl.std.ir.json`, which
  `ridl diff`'s compiled side excludes, so diffing an unedited workspace
  reported `breaking`. `ridl check --baseline` masked it. The standard package
  belongs in the code emits, not in a contract snapshot.
- **The design overstated its own defence.** It claimed the corpus proofs turn a
  detection miss into a compile failure. They did not: they generated the
  standard package unconditionally in process and never exercised the emit
  decision. The guard the design described now exists, driven through the real
  `run_build`.
