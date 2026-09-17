# typl Value Objects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every typl named scalar a value object whose constraint is
enforced at construction, and give every generated type the derives that are
sound for it.

**Architecture:** The IR already carries the constraints; no IR schema change is
needed except one shared classifier. The Rust backend gains a private inner
field, `new`/`new_unchecked`, `TryFrom`/`From`, and a derive eligibility pass
reusing the recursion in `defaults.rs`. The constraint error is not generated:
the constructors return `ridl_rt::payload::Violation`, which `ridl-rt` 0.1
already defines and already carries as ridl §10.2's `INVALID_VALUE`. `ridlc`
starts emitting a `lib.rs` and `Cargo.toml` so the Rust output compiles
standalone. One checker change is needed after all: Task 11 extends RIDL-149 to
a union's arms. The TypeScript backend keeps its compile-time brand and gains
factory functions in step 2, not here (Task 9).

**Tech Stack:** Rust 2024, `proc-macro2`/`quote`/`prettyplease` (Rust emitter),
`insta` snapshots, `protox`-generated IR types.

**Spec:** `docs/wip/typl-value-objects-design.md`. Where this plan and the spec
disagree, the spec is authoritative — with one recorded exception. The spec
places the constraint error in a per-package vocabulary emitted by a module that
driftsys/ridl#241 has since retracted, and `ridl-rt` now defines that type; Task
2 states the reasoning, and Task 10 corrects the spec before it is archived.
Where the spec is stale about the tree rather than wrong about the design, this
plan says so at the point of use rather than silently diverging.

## Currency

Written 2026-08-03. Refreshed 2026-09-16 against `origin/main` at 86e10d7,
because three changes landed after it was written and moved what it points at:

- **driftsys/ridl#241 retracted the interaction layer.** Commit `7d539bc`
  deleted `crates/ridl-backend-rust/src/interact.rs`,
  `crates/ridl-backend-ts/src/interact*`, `c_header.rs` and
  `templates/c_header.j2` outright. Nothing replaced them
  ([ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 15 records this as a retraction, not a relocation). Task 2 and Task 7
  named those files and are rewritten below.
- **driftsys/ridl#242 and driftsys/ridl#303 added two wire backends**,
  `crates/ridl-backend-proto` and `crates/ridl-backend-flatbuffers`. Neither is
  touched by this plan; see the wire-backend constraint below.
- **driftsys/ridl#238 pinned the name transform**
  ([ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
  decisions 1 to 5), which is what Task 11 is written against.

Two structural changes came with the refresh: Task 9 (TypeScript) moved to step
2, and Task 11 was added for the two Rust naming defects. Both are recorded
where they happen.

**A third structural change followed on the same day.** Task 2 was rewritten
again: the generated code defines no constraint error, and the constructors
return `ridl_rt::payload::Violation`, which `ridl-rt` 0.1 already carries. The
crate did not exist when the design spec was written, so the spec places that
type in a per-package vocabulary that driftsys/ridl#241 has since retracted. The
reasoning is in Task 2 under "Why not a generated type", and it closes Open item
3 rather than answering it.

**Line references were replaced by symbol names** wherever a symbol exists. The
line numbers this plan carried had drifted by eleven lines in
`crates/ridl-backend-rust/src/lib.rs` alone, and a symbol name does not drift.
Where a line number remains it is the one at 86e10d7 and is given as an aid, not
as the identifier.

## Global Constraints

- **The generated Rust default build takes one dependency, `ridl-rt`**, which is
  `no_std` and has no dependency of its own in any feature combination, plus the
  optional `regex` behind `validate-pattern`. Emitted code names only `core`,
  `alloc`, `std` and `ridl_rt` paths. The rule this replaces said
  "dependency-free"; it was written on 2026-08-03, before `ridl-rt` existed, and
  its purpose was to keep `regex` off a constrained target, which still holds.
  ADR-0020 decision 6 states that generated Rust links `ridl-rt`.
- **Codegen never panics.** Every failure is a `GenerateError` value. This cites
  ADR-0004 section 5, which is about the diagnostic model and states neither
  half; the citation predates this plan's refresh and is left for whoever
  executes Task 10 to correct or drop.
- **Conventional Commits**, linted by git-std against `.git-std.toml`. Scopes
  used here: `ridl-ir`, `ridl-backend-rust`, `ridl-sem`, `ridlc`, `adr`, `typl`.
- **Never push to `main`.** Each task lands as its own pull request, one per
  Epic 10 story, per `docs/wip/2026-09-13-step1-lanes-plan.md` §4 (stage C4).
  The single `feat/typl-value-objects` branch this plan first named is
  superseded by that.
- **The two wire backends are out of scope.** `crates/ridl-backend-proto` and
  `crates/ridl-backend-flatbuffers` emit a schema, not a constructor
  ([ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) decision 2), so no
  task touches them. Their snapshots do change when a name transform changes;
  Task 11 is the only task where that happens.
- **Run `just verify` before opening the PR** (`lint-commits`, then the full
  `build` gate). Individual tasks run `just test` and `just lint`.
- **Snapshots are `insta`.** Review changes with `cargo insta review`; never
  hand-edit a `.snap` file.
- **Prose is plain and literal** — comments, commit messages, docs. No idioms.
- **`Default` is never derived** — it comes from the typl init value (§5.8).
- **`Send`/`Sync` are never emitted** — they are auto traits.

---

### Task 1: The shared vacuous-constraint classifier

**Model:** Opus (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

Both backends must agree on when a constraint has nothing to check. A duplicated
predicate would drift, so it lives in `ridl-ir` beside the generated types.

**Files:**

- Modify: `crates/ridl-ir/src/lib.rs`
- Test: `crates/ridl-ir/src/lib.rs`. The crate has no general
  `#[cfg(test)] mod
  tests`; its one top-level test module is
  `mod v2_round_trip` (`lib.rs:662`), which already does `use crate::v2;`. Add a
  test module of its own beside it rather than putting a classifier test in a
  round-trip module.

**Interfaces:**

- Consumes: nothing.
- Produces:
  `pub fn ridl_ir::v2::constraint_is_vacuous(c: Option<&v2::Constraint>) -> bool`.
  Returns `true` when `c` is `None`, or when `min`, `max`, `len_min`, `len_max`,
  and `pattern` are all absent. `step` is deliberately ignored — step is not
  validated (spec, "Not validated").

- [ ] **Step 1: Write the failing test**

Add to the existing test module in `crates/ridl-ir/src/lib.rs`:

```rust
#[test]
fn vacuous_constraint_ignores_step() {
    // A declared step alone leaves nothing for a constructor to check:
    // quantization is normalized, not validated (design spec, Deferred).
    let stepped = v2::Constraint {
        min: None,
        max: None,
        step: Some("0.5".to_string()),
        len_min: None,
        len_max: None,
        pattern: None,
        pattern_const: None,
    };
    assert!(v2::constraint_is_vacuous(Some(&stepped)));
    assert!(v2::constraint_is_vacuous(None));

    let ranged = v2::Constraint {
        min: Some("0.0".to_string()),
        max: Some("250.0".to_string()),
        step: None,
        len_min: None,
        len_max: None,
        pattern: None,
        pattern_const: None,
    };
    assert!(!v2::constraint_is_vacuous(Some(&ranged)));

    let bounded = v2::Constraint {
        min: None,
        max: None,
        step: None,
        len_min: Some(0),
        len_max: Some(256),
        pattern: None,
        pattern_const: None,
    };
    assert!(!v2::constraint_is_vacuous(Some(&bounded)));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridl-ir --locked vacuous_constraint_ignores_step` Expected:
FAIL — `cannot find function 'constraint_is_vacuous' in module 'v2'`.

- [ ] **Step 3: Write the implementation**

In `crates/ridl-ir/src/lib.rs`, inside the `v2` module's hand-written section:

```rust
/// Whether a constraint leaves a generated constructor nothing to check.
///
/// True when no bound and no pattern is present. `step` is excluded on
/// purpose: quantization is normalized rather than validated, so a step-only
/// constraint still admits an infallible constructor (design spec, Deferred).
///
/// Because the checker materializes the typl §4.4 default `[0..256]` into
/// `len_min`/`len_max`, every string and bytes type is non-vacuous. In
/// practice this reduces to `boolean`, and `integer`/`float` with no declared
/// range.
pub fn constraint_is_vacuous(constraint: Option<&Constraint>) -> bool {
    let Some(c) = constraint else { return true };
    c.min.is_none()
        && c.max.is_none()
        && c.len_min.is_none()
        && c.len_max.is_none()
        && c.pattern.is_none()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ridl-ir --locked vacuous_constraint_ignores_step` Expected:
PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/lib.rs
git commit -m "feat(ridl-ir): add the vacuous-constraint classifier

Both language backends must agree on when a constraint leaves a
constructor nothing to check. step is excluded: quantization is
normalized rather than validated."
```

---

### Task 2: Depend on `ridl-rt` for the constraint error

**Model:** Sonnet (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**The generated code defines no error type. It uses
`ridl_rt::payload::Violation`.** This reverses what this plan said before
2026-09-16, and it closes Open item 3 rather than proposing an answer to it —
see "Why not a generated type" below.

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — the emitted `use` line, so
  each package module names the type once and the constructors spell it short.
- Modify: `crates/ridlc/src/lib.rs` — Task 7's generated manifest gains
  `ridl-rt` as a dependency.
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `ridl_rt::payload::Violation` and `ridl_rt::payload::Rule`, which
  exist today in `crates/ridl-rt/src/payload.rs:177` and `:186`.
- Produces: no type. Tasks 3, 4, 5 and 8 construct `Violation` values.

**What `ridl-rt` already carries**, verified at `origin/main`:

```rust
// crates/ridl-rt/src/payload.rs
/// A value that breaks a typl constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The name of the typl type whose constraint failed.
    pub type_name: &'static str,
    /// The kind of constraint that failed.
    pub rule: Rule,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule { Range, Step, Length, Pattern, Variant }
```

That is the design spec's `ConstraintError` under another name: the same
`type_name: &'static str`, and `Rule` covering all four kinds the spec named
plus `Step`. `ridl-rt` also carries it upward already —
`error::Contract::InvalidValue(Violation)` is ridl §10.2's `INVALID_VALUE`
(`crates/ridl-rt/src/error.rs:12`) — so a constructor's rejection and a
runtime's contract error are one type end to end, with no conversion written
anywhere.

`Rule::Step` is a kind no generated constructor produces: the design defers step
to rounding rather than checking (design spec, Deferred). `Rule` is
`#[non_exhaustive]`, so a consumer must have a wildcard arm regardless.

#### Why not a generated type

The plan carried a generated `pub enum ConstraintError` from 2026-08-03 until
2026-09-16, and proposed under Open item 3 to hoist it to the generated crate
root. Both are wrong, for a reason that outlives either:

1. **Its shape does not depend on the contract.** Everything else the backends
   emit is derived from the `.ridl` source — this type is fixed for every
   package that will ever exist. A type that is the same in every generated
   crate is a library's job, not a generator's.
2. **`ridl-rt` is the crate for exactly this.** Its own description is "the
   vocabulary that code generated from ridl and a runtime agree on: … and
   contract and transport errors", and ADR-0020 decision 6 states plainly that
   "generated Rust links `ridl-rt`"
   (`docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md:199`).
   A package's generated Rust is compiled to `wasm32` against it (decision 7),
   and `just wasm-check` already covers `-p ridl-rt` for that reason
   (`docs/design/ridl-rt.md:253`).
3. **It costs a constrained target nothing.** `ridl-rt` is `#![no_std]`,
   `#![forbid(unsafe_code)]`, has no `extern crate alloc`, and **has no
   dependency in any feature combination** (`docs/design/ridl-rt.md:249`,
   ADR-0021 decision 8). The plan's dependency-free rule was written against
   `regex`, which stays behind `validate-pattern`, and does not reach here.
4. **It settles driftsys/ridl#252 by construction.** That issue records the
   retracted interaction layer emitting its vocabulary once per package, so two
   packages in one crate collided, and requires that whatever replaces it "must
   be nameable across every package in a workspace at once". One type in one
   library is nameable from everywhere, with no crate root to hoist into and no
   single-file special case. A consumer that constructs a `veh.common` type and
   a `veh.adas` type writes one `match`.
5. **The design spec's placement is stale, not overruled.** It put
   `ConstraintError` in "the package vocabulary the `interact` module already
   emits beside `Provenance` and `SignalHandle`". That module was retracted by
   driftsys/ridl#241, and `Provenance` now lives in `ridl-rt`. The spec named
   the company the type should keep; that company moved, and this follows it.

Why the plan did not say this from the start: the design spec is dated
2026-08-03 and `ridl-rt` did not exist. Stage C2's refresh caught the deleted
`interact.rs` but kept the generated type, which is why Open item 3 was filed
rather than answered.

#### What this costs, named plainly

**A pure typl package's generated Rust now has a dependency.** A package
declaring only types, with no interaction anywhere, links `ridl-rt` to name one
struct. That is a real change and it is the one judgement in this task. It is
taken because ADR-0020 decision 6 already makes `ridl-rt` the dependency of
generated Rust without qualifying it by what the package declares, and because a
zero-dependency `no_std` crate is the cheapest dependency available. If that is
ever unwanted, the escape is a `ridl-rt` cargo feature that the generated
manifest sets, not a second copy of the type.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_pure_typl_package_names_the_runtime_violation() {
    // A package with no interface still reaches ridl-rt, because a named
    // scalar's constructor returns its Violation.
    let source = rust_for(vec![speed_decl()]);
    assert!(source.contains("use ridl_rt::payload::{Rule, Violation};"), "got:\n{source}");
    // No generated error type: the library owns this one.
    assert!(!source.contains("enum ConstraintError"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
`cargo test -p ridl-backend-rust --locked a_pure_typl_package_names_the_runtime`
Expected: FAIL — nothing emits a `use` of `ridl-rt` today.

- [ ] **Step 3: Write the implementation**

Emit the `use` at the top of each package module, from `generate` (`lib.rs:54`),
when the package declares a non-vacuous named scalar, an `enum` or an `enumset`
— that is, when Tasks 3, 5 or 8 will construct a `Violation`. Emitting it
unconditionally would draw `unused_imports` on a package that declares only
vacuous types, and the compile proofs deny lints by name, so nothing would catch
it.

```rust
if package_constructs_a_violation(ir) {
    items.push(quote! { use ridl_rt::payload::{Rule, Violation}; });
}
```

`ridl-rt` is a path dependency inside this workspace and a version dependency in
the generated manifest. Task 7 writes the latter; add it there rather than here,
and take the version from `crates/ridl-rt/Cargo.toml` rather than writing a
literal that will drift.

- [ ] **Step 4: Run the test and accept the snapshot**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS. Every snapshot
for a package with a constrained type gains the `use` line.

**The compile proofs need the dependency.** `appendix_b_compiles_with_rustc` and
the others drive `rustc` directly on a single file with no `--extern`, so
generated code naming `ridl_rt` will not compile there. Each proof needs
`ridl-rt` built and passed as `--extern ridl_rt=<path>`, or the proof moves to a
`trybuild`-style fixture crate. Settle this in this task rather than in Task 3,
which is the first task whose output actually names the type.

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "feat(ridl-backend-rust): return the runtime Violation from constructors

ridl-rt already defines the constraint violation - payload::Violation, with
the same type_name and a Rule covering every kind - and error.rs already
carries it as Contract::InvalidValue, ridl 10.2's INVALID_VALUE. Generating
a second type per package would give one crate N incompatible error types,
which is the defect #252 records, and its shape does not depend on the
contract, so it belongs in the library rather than the generator.

ADR-0020 decision 6 already makes ridl-rt the dependency of generated Rust.
The crate is no_std with no dependency in any feature combination, so a
constrained target pays nothing for it."
```

---

### Task 3: Constrained named scalars — private inner, `new`, `TryFrom`

**Model:** Fable (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

The core change, and the breaking one.

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_type_def`
  (`lib.rs:255`), `emit_const` (`lib.rs:272`)
- Modify: `crates/ridl-backend-rust/src/defaults.rs` — the two tuple-struct
  construction sites, `Some(quote! { #name_id(#inner) })` (`defaults.rs:55`) and
  `Some(quote! { #path(#inner) })` (`defaults.rs:183`)
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `ridl_ir::v2::constraint_is_vacuous` (Task 1),
  `ridl_rt::payload::{Violation, Rule}` (Task 2).
- Produces: for a constrained named scalar `Speed` over `f64`, an emitted
  `Speed::new(f64) -> Result<Speed, Violation>`,
  `Speed::new_unchecked(f64) -> Speed` (`pub const`), `Speed::get(self) -> f64`,
  `impl TryFrom<f64> for Speed`, `impl From<Speed> for f64`. Task 4 emits the
  vacuous counterpart; Task 8 adds the pattern branch inside `new`.

Both call sites that construct a newtype by tuple syntax must move to
`new_unchecked`, which is why it is `const`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn constrained_scalar_is_a_value_object() {
    let source = rust_for(vec![speed_decl()]);
    // The field is private: no `pub` inside the tuple struct.
    assert!(
        source.contains("pub struct Speed(f64)"),
        "inner field must be private, got:\n{source}"
    );
    assert!(source.contains("pub fn new(value: f64) -> Result<Self, Violation>"));
    assert!(source.contains("pub const fn new_unchecked(value: f64) -> Self"));
    assert!(source.contains("pub const fn get(self) -> f64"));
    assert!(source.contains("impl TryFrom<f64> for Speed"));
    assert!(source.contains("impl From<Speed> for f64"));
    // The infallible inbound conversion must never appear on a constrained type.
    assert!(
        !source.contains("impl From<f64> for Speed"),
        "From<Inner> reintroduces unchecked construction"
    );
}

#[test]
fn constant_of_a_constrained_type_uses_new_unchecked() {
    let decls = vec![
        speed_decl(),
        public_decl(
            "MAX_SPEED",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Speed".to_string()),
                value: "250.0".to_string(),
                regex: None,
            }),
        ),
    ];
    let source = rust_for(decls);
    assert!(source.contains("Speed::new_unchecked(250.0)"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-backend-rust --locked constrained_scalar_is_a_value_object constant_of_a_constrained_type`
Expected: FAIL — the first on `inner field must be private`, the second on the
missing `new_unchecked` call.

- [ ] **Step 3: Write the implementation**

In `emit_type_def`, replace the body so the field is private and the impl block
is emitted. `newtype_inner(td)` already yields the backing type.

```rust
/// A named scalar becomes a `#[repr(transparent)]` newtype with a private
/// inner value (typl §5.7). Construction goes through `new`, which enforces
/// the typl constraints, or `new_unchecked`, which does not.
fn emit_type_def(decl: &v2::Decl, td: &v2::TypeDef) -> TokenStream {
    let name = ident(&decl.name);
    let inner = newtype_inner(td);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);
    let type_name = decl.name.as_str();

    if ridl_ir::v2::constraint_is_vacuous(td.constraint.as_ref()) {
        return emit_vacuous_type_def(decl, td); // Task 4
    }

    let checks = constraint_checks(td, type_name, quote! { value });
    let getter = scalar_getter(td, vis.clone(), inner.clone());

    quote! {
        #attrs
        #[repr(transparent)]
        #vis struct #name(#inner);

        impl #name {
            /// Constructs the value, enforcing its typl constraints.
            #vis fn new(value: #inner) -> Result<Self, Violation> {
                #checks
                Ok(Self(value))
            }

            /// Constructs the value without checking its constraints.
            ///
            /// Safe: nothing here relies on the invariant for memory
            /// soundness. Use it only for a value already known to satisfy
            /// the contract.
            #vis const fn new_unchecked(value: #inner) -> Self {
                Self(value)
            }

            #getter
        }

        impl TryFrom<#inner> for #name {
            type Error = Violation;
            fn try_from(value: #inner) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<#name> for #inner {
            fn from(value: #name) -> Self {
                value.0
            }
        }
    }
}
```

Add the two helpers. `constraint_checks` emits only the branches the constraint
carries, so an unbounded-but-length-bounded string gets only the length check:

```rust
/// The range and length checks for one constraint. The pattern check is added
/// by Task 8 behind the `validate-pattern` feature.
fn constraint_checks(td: &v2::TypeDef, type_name: &str, value: TokenStream) -> TokenStream {
    let Some(c) = td.constraint.as_ref() else {
        return quote! {};
    };
    let mut checks = Vec::new();

    if let Some(min) = c.min.as_deref() {
        let lit = scalar_literal(td, min);
        checks.push(quote! {
            if #value < #lit {
                return Err(Violation { type_name: #type_name, rule: Rule::Range });
            }
        });
    }
    if let Some(max) = c.max.as_deref() {
        let lit = scalar_literal(td, max);
        checks.push(quote! {
            if #value > #lit {
                return Err(Violation { type_name: #type_name, rule: Rule::Range });
            }
        });
    }
    // Length is in characters for string (typl §5.3) and bytes for bytes
    // (§5.4), which is why the two use different expressions.
    if c.len_min.is_some() || c.len_max.is_some() {
        let len = match backing_scalar(td) {
            ScalarBacking::String => quote! { #value.chars().count() as u64 },
            _ => quote! { #value.len() as u64 },
        };
        if let Some(min) = c.len_min {
            let lit = proc_macro2::Literal::u64_unsuffixed(min);
            checks.push(quote! {
                if #len < #lit {
                    return Err(Violation { type_name: #type_name, rule: Rule::Length });
                }
            });
        }
        if let Some(max) = c.len_max {
            let lit = proc_macro2::Literal::u64_unsuffixed(max);
            checks.push(quote! {
                if #len > #lit {
                    return Err(Violation { type_name: #type_name, rule: Rule::Length });
                }
            });
        }
    }
    quote! { #(#checks)* }
}

/// The accessor. A `Copy` backing returns by value from a `const fn`; `String`
/// and `Vec<u8>` borrow, and gain `into_inner` for the owned form.
///
/// `backing_scalar` is total — it returns `ScalarBacking`, not an `Option`
/// (`lib.rs:645`), and maps a unit backing and an absent backing to `Float`.
fn scalar_getter(td: &v2::TypeDef, vis: TokenStream, inner: TokenStream) -> TokenStream {
    match backing_scalar(td) {
        ScalarBacking::String => quote! {
            #vis fn get(&self) -> &str { &self.0 }
            #vis fn into_inner(self) -> String { self.0 }
        },
        ScalarBacking::Bytes => quote! {
            #vis fn get(&self) -> &[u8] { &self.0 }
            #vis fn into_inner(self) -> Vec<u8> { self.0 }
        },
        _ => quote! {
            #vis const fn get(self) -> #inner { self.0 }
        },
    }
}
```

Extend `decl_attrs` so a named scalar whose constraint carries a `step` gains a
doc line naming what `new` does not check, rather than staying silent (spec,
"Not validated"):

```rust
/// The gaps a generated constructor does not close, named on the type itself.
fn unchecked_doc(td: &v2::TypeDef) -> TokenStream {
    let Some(c) = td.constraint.as_ref() else {
        return quote! {};
    };
    if c.step.is_none() {
        return quote! {};
    }
    let line = " Quantization (`step`) is not checked by `new`.";
    quote! { #[doc = #line] }
}
```

In `defaults.rs`, change both construction sites from tuple syntax to
`new_unchecked`:

```rust
// defaults.rs:55, inside the named-scalar default
Some(quote! { #name_id::new_unchecked(#inner) })
// defaults.rs:183, inside the same-package reference default
Some(quote! { #path::new_unchecked(#inner) })
```

In `emit_const`, change the three `#type_name(#value)` forms to
`#type_name::new_unchecked(#value)`.

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked && cargo test -p ridl --locked`
Expected: PASS, including the three `rustc` compile proofs in `tests.rs` —
`appendix_b_compiles_with_rustc`, `constructible_collections_compile`, and
`a_tuple_under_an_internal_declaration_is_package_private`. Those are the real
check that the emitted code is valid Rust. None of them fails on a warning: the
first two pass no `-D` flag at all and assert only `status.success()`, and the
third denies two lints by name. So a naming lint does not fail any of them; Task
11 is where that matters.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/
git commit -m "feat(ridl-backend-rust)!: make constrained named scalars value objects

The inner value becomes private and construction goes through new, which
enforces the typl range and length constraints, or new_unchecked, which
does not. TryFrom is the fallible inbound conversion and From the
infallible outbound one; From<Inner> is never emitted for a constrained
type, because it would reintroduce unchecked construction.

Default derivation and constant emission now route through the public
const new_unchecked, which is what lets the field be genuinely private
while cross-package Default derivation keeps working.

BREAKING CHANGE: generated code no longer exposes the inner field. Read it
with get() or From, and construct with new() or new_unchecked()."
```

---

### Task 4: Vacuous named scalars — infallible construction

**Model:** Sonnet (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs`
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `constraint_is_vacuous` (Task 1), `scalar_getter` (Task 3).
- Produces:
  `fn emit_vacuous_type_def(decl: &v2::Decl, td: &v2::TypeDef) -> TokenStream`,
  called from `emit_type_def` (Task 3).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn vacuous_scalar_constructs_infallibly() {
    let decls = vec![public_decl(
        "Enabled",
        primitive_type(v2::PrimitiveType::Boolean, init_value(true, Some("false")), None),
    )];
    let source = rust_for(decls);
    assert!(source.contains("pub const fn new(value: bool) -> Self"));
    assert!(source.contains("impl From<bool> for Enabled"));
    assert!(source.contains("impl From<Enabled> for bool"));
    // No escape hatch is emitted: `new` already is one.
    assert!(
        !source.contains("Enabled::new_unchecked") && !source.contains("fn new_unchecked(value: bool)"),
        "new_unchecked would duplicate new on a vacuous type"
    );
    // And no manual TryFrom, which would collide with core's blanket impl.
    assert!(!source.contains("impl TryFrom<bool> for Enabled"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
`cargo test -p ridl-backend-rust --locked vacuous_scalar_constructs_infallibly`
Expected: FAIL — `emit_vacuous_type_def` does not exist yet, so `Enabled` still
takes the constrained path.

- [ ] **Step 3: Write the implementation**

```rust
/// A named scalar whose constraint checks nothing: `boolean`, and `integer` or
/// `float` with no declared range.
///
/// Construction is infallible, so `From<Inner>` is correct here — there is no
/// invariant for it to bypass. Core's blanket `impl<T, U: Into<T>> TryFrom<U>
/// for T` then supplies `TryFrom<Inner>` with `Error = Infallible`, so generic
/// consumer code calling `try_from` compiles against both kinds of scalar.
/// `new_unchecked` is deliberately absent: `new` already is the unchecked path.
fn emit_vacuous_type_def(decl: &v2::Decl, td: &v2::TypeDef) -> TokenStream {
    let name = ident(&decl.name);
    let inner = newtype_inner(td);
    let attrs = decl_attrs(decl);
    let vis = vis_tokens(decl.visibility);
    let getter = scalar_getter(td, vis.clone(), inner.clone());

    // A `String`/`Vec<u8>` backing cannot appear here: the checker always
    // materializes the typl §4.4 default `[0..256]`, so both are non-vacuous.
    quote! {
        #attrs
        #[repr(transparent)]
        #vis struct #name(#inner);

        impl #name {
            /// Constructs the value. This type declares no constraint, so
            /// construction cannot fail.
            #vis const fn new(value: #inner) -> Self { Self(value) }
            #getter
        }

        impl From<#inner> for #name {
            fn from(value: #inner) -> Self { Self(value) }
        }

        impl From<#name> for #inner {
            fn from(value: #name) -> Self { value.0 }
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS. The
`named_scalar_backings` snapshot (`tests.rs:172`) now shows `Enabled` and
`Counter` on the vacuous path and `Label`/`Blob` on the constrained path, since
both carry the defaulted length bound.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/
git commit -m "feat(ridl-backend-rust): construct vacuous named scalars infallibly

A boolean, or an integer or float with no declared range, has nothing to
check. Such a type gets a const new, From in both directions, and neither
new_unchecked nor a manual TryFrom - core's blanket impl supplies the
latter with Error = Infallible."
```

---

### Task 5: `TryFrom<i64>` for enum and enum set

**Model:** Sonnet (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_enum` (`lib.rs:389`),
  `emit_enum_set` (`lib.rs:413`)
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `ridl_rt::payload::{Violation, Rule}` (Task 2).
- Produces: `impl TryFrom<i64> for <Enum>`, `impl From<<Enum>> for i64`, and the
  same pair for each enum set.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn enum_converts_from_a_raw_discriminant() {
    let source = rust_for(vec![gear_position_decl()]);
    assert!(source.contains("impl TryFrom<i64> for GearPosition"));
    assert!(source.contains("impl From<GearPosition> for i64"));
    assert!(source.contains("Rule::Variant"));
}

#[test]
fn enum_set_rejects_bits_outside_the_declared_mask() {
    let source = rust_for(vec![features_decl()]);
    assert!(source.contains("impl TryFrom<i64> for Features"));
    assert!(source.contains("const DECLARED_MASK: i64"));
}
```

If `gear_position_decl` and `features_decl` do not already exist in `tests.rs`,
build them with the existing `public_decl` helper and `v2::decl::Kind::EnumDef`
/ `EnumSetDef`, mirroring the fixtures the `enum` and `enumset` snapshot tests
already use.

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-backend-rust --locked enum_converts_from_a_raw enum_set_rejects_bits`
Expected: FAIL — no `TryFrom` impl is emitted for either kind.

- [ ] **Step 3: Write the implementation**

Append to `emit_enum`'s returned stream:

```rust
    // A raw discriminant off the wire is where an out-of-contract value
    // actually enters a program: a wire backend emits no constructor
    // (ADR-0013 decision 2), so this is the validating seam.
    let arms = ed.values.iter().map(|value| {
        let vname = ident(&value.name);
        let disc = int_tokens(value.value);
        quote! { #disc => Ok(Self::#vname) }
    });
    let type_name = decl.name.as_str();

    quote! {
        impl TryFrom<i64> for #name {
            type Error = Violation;
            fn try_from(value: i64) -> Result<Self, Self::Error> {
                match value {
                    #(#arms,)*
                    _ => Err(Violation { type_name: #type_name, rule: Rule::Variant }),
                }
            }
        }

        impl From<#name> for i64 {
            fn from(value: #name) -> Self { value as i64 }
        }
    }
```

Append to `emit_enum_set`'s returned stream:

```rust
    let mask = esd.bits.iter().fold(0i64, |acc, bit| acc | (1i64 << bit.value));
    let mask_lit = int_tokens(mask);
    let type_name = decl.name.as_str();

    quote! {
        impl #name {
            /// The union of every declared bit. A value carrying any other
            /// bit is not a member of this set (typl §9).
            #vis const DECLARED_MASK: i64 = #mask_lit;
        }

        impl TryFrom<i64> for #name {
            type Error = Violation;
            fn try_from(value: i64) -> Result<Self, Self::Error> {
                if value & !Self::DECLARED_MASK != 0 {
                    return Err(Violation { type_name: #type_name, rule: Rule::Variant });
                }
                Ok(Self(value))
            }
        }

        impl From<#name> for i64 {
            fn from(value: #name) -> Self { value.0 }
        }
    }
```

Note the enum set's inner field is already emitted as `#vis i64` —
`#vis struct #name(#vis i64);` at `lib.rs:427` — change it to a private `i64`
for consistency with Task 3, and add `#vis const fn get(self) -> i64 { self.0 }`
to its impl block.

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS, including the
`rustc` compile proofs.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/
git commit -m "feat(ridl-backend-rust)!: validate enum and enum-set conversion from i64

A raw discriminant or bit pattern off the wire is where an out-of-contract
value enters a program, and ADR-0013 decision 2 gives a wire backend no
constructor of its own, so this is the validating seam.

BREAKING CHANGE: an enum set's inner value is now private; read it with
get()."
```

---

### Task 6: Sound derives

**Model:** Fable (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**Files:**

- Create: `crates/ridl-backend-rust/src/derives.rs`
- Modify: `crates/ridl-backend-rust/src/lib.rs` (declare the module; call it
  from `emit_decl`)
- Test: `crates/ridl-backend-rust/src/tests.rs`

`derives.rs` is its own file because the eligibility recursion is the same shape
and size as `defaults.rs`, which is already separate for the same reason.

**Interfaces:**

- Consumes: the `Ctx` type in `lib.rs`, `backing_scalar`, `ScalarBacking`.
- Produces:
  `pub(crate) fn derives::derive_attr(ctx: &Ctx, decl: &v2::Decl) -> TokenStream`
  returning the `#[derive(...)]` attribute for one declaration.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn float_backed_scalar_derives_partial_ord_but_not_ord() {
    let source = rust_for(vec![speed_decl()]);
    // Ord requires Eq, and f64 is neither.
    assert!(source.contains("#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]"));
    assert!(!source.contains("Eq, Hash"));
}

#[test]
fn integer_backed_scalar_derives_the_full_ordering_set() {
    let source = rust_for(vec![counter_decl()]);
    assert!(source.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"));
}

#[test]
fn string_backed_scalar_is_not_copy() {
    let decls = vec![public_decl(
        "Label",
        primitive_type(v2::PrimitiveType::String, init_value(true, Some("")), None),
    )];
    let source = rust_for(decls);
    assert!(source.contains("#[derive(Debug, Clone, PartialEq, Eq, Hash)]"));
    assert!(!source.contains("Copy"));
}

#[test]
fn struct_with_a_float_field_is_not_eq() {
    // A float anywhere in the transitive closure removes Eq and Hash.
    let decls = vec![speed_decl(), struct_with_field("Telemetry", "speed", "Speed")];
    let source = rust_for(decls);
    let telemetry = source
        .split("pub struct Telemetry")
        .next()
        .expect("the struct is emitted");
    assert!(!telemetry.ends_with("Eq, Hash)]\n"));
}

#[test]
fn struct_with_a_cross_package_field_drops_conditional_derives() {
    // The referenced package is not in this IR, so Copy and Eq cannot be
    // proven and must not be asserted.
    let decls = vec![struct_with_field("Telemetry", "speed", "veh.other.Speed")];
    let source = rust_for(decls);
    assert!(source.contains("#[derive(Debug, Clone, PartialEq)]"));
}
```

Add a `struct_with_field(name, field_name, type_ref)` fixture builder beside the
existing ones if absent.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-backend-rust --locked derives` Expected: FAIL — no
`#[derive]` is emitted on the typl surface at all.

- [ ] **Step 3: Write the implementation**

Create `crates/ridl-backend-rust/src/derives.rs`:

```rust
//! Derive eligibility for the typl surface.
//!
//! `Debug`, `Clone`, and `PartialEq` are sound on every generated type: every
//! backing has them and every generated type receives them, so the recursion
//! cannot fail. The rest are conditional and need the transitive closure:
//!
//! - `Copy` — every leaf must be `f64`, `i64`, or `bool`.
//! - `Eq`, `Hash` — no `f64` anywhere, including unit-backed types, since a
//!   unit backing implies float (typl §5.1).
//! - `PartialOrd`/`Ord` — named scalars over a numeric backing only. Ordering
//!   a struct's fields lexicographically, or a union's arms by declaration
//!   order, is not a property typl states.
//!
//! **Cross-package references are handled conservatively.** `defaults.rs` can
//! be optimistic — it emits `path::default()` and lets rustc verify. A derive
//! cannot: `#[derive(Copy)]` on a struct whose cross-package field is not
//! `Copy` is a hard error in the consumer's build. So an unresolvable
//! reference disables every conditional derive.
//!
//! The recursion mirrors `defaults.rs`: leaf recursion with a cycle guard, and
//! a composite reference re-checked rather than trusted.

use crate::{Ctx, ScalarBacking, backing_scalar};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::v2;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Eligibility {
    pub(crate) copy: bool,
    pub(crate) eq: bool,
}

impl Eligibility {
    const NONE: Self = Self { copy: false, eq: false };
    const ALL: Self = Self { copy: true, eq: true };

    fn meet(self, other: Self) -> Self {
        Self { copy: self.copy && other.copy, eq: self.eq && other.eq }
    }
}

pub(crate) fn derive_attr(ctx: &Ctx, decl: &v2::Decl) -> TokenStream {
    let mut seen = HashSet::new();
    let e = decl_eligibility(ctx, decl, &mut seen);
    let ordered = numeric_named_scalar(decl);

    let mut traits = vec![quote! { Debug }, quote! { Clone }];
    if e.copy {
        traits.push(quote! { Copy });
    }
    traits.push(quote! { PartialEq });
    if e.eq {
        traits.push(quote! { Eq });
        traits.push(quote! { Hash });
    }
    if ordered {
        traits.push(quote! { PartialOrd });
        if e.eq {
            traits.push(quote! { Ord });
        }
    }
    quote! { #[derive(#(#traits),*)] }
}

/// True for a `type` declaration over an integer or float backing. A unit
/// backing implies float (typl §5.1), so it qualifies for `PartialOrd`.
fn numeric_named_scalar(decl: &v2::Decl) -> bool {
    let Some(v2::decl::Kind::TypeDef(td)) = &decl.kind else {
        return false;
    };
    matches!(
        backing_scalar(td),
        ScalarBacking::Float | ScalarBacking::Integer
    )
}

fn decl_eligibility(ctx: &Ctx, decl: &v2::Decl, seen: &mut HashSet<String>) -> Eligibility {
    if !seen.insert(decl.name.clone()) {
        // A cyclic IR: refuse the conditional derives rather than recurse
        // forever (the C1b guard in `defaults.rs`).
        return Eligibility::NONE;
    }
    let result = match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => scalar_eligibility(td),
        Some(v2::decl::Kind::EnumDef(_)) => Eligibility::ALL,
        Some(v2::decl::Kind::EnumSetDef(_)) => Eligibility::ALL,
        Some(v2::decl::Kind::StructDef(sd)) => sd
            .members
            .iter()
            .filter_map(|m| match &m.member {
                Some(v2::struct_member::Member::Field(field)) => Some(field),
                _ => None,
            })
            .fold(Eligibility::ALL, |acc, f| acc.meet(field_eligibility(ctx, f, seen))),
        Some(v2::decl::Kind::UnionDef(ud)) => ud
            .arms
            .iter()
            .fold(Eligibility::ALL, |acc, arm| {
                acc.meet(type_ref_eligibility(ctx, &arm.type_ref, seen))
            }),
        _ => Eligibility::NONE,
    };
    seen.remove(&decl.name);
    result
}

fn scalar_eligibility(td: &v2::TypeDef) -> Eligibility {
    match backing_scalar(td) {
        // A unit backing implies float, so it lands here too, as does an
        // absent backing: `backing_scalar` is total and maps both to `Float`.
        ScalarBacking::Float => Eligibility { copy: true, eq: false },
        ScalarBacking::Integer | ScalarBacking::Boolean => Eligibility::ALL,
        ScalarBacking::String | ScalarBacking::Bytes => {
            Eligibility { copy: false, eq: true }
        }
    }
}
```

Two IR shapes to note, because the obvious spelling does not compile. A
`v2::StructDef` holds `repeated StructMember members`, not `fields`, and each
`StructMember` is a `oneof { Field field; Reserved reserved; }` — which is why
the fold above filters. `defaults.rs` already walks it that way
(`defaults.rs:84`). And a `v2::Field`'s type is `field.r#type`, raw-identifier
escaped because `type` is a Rust keyword; there is no `field_type` member.

```rust
/// One field's contribution. An `Option<T>` keeps `T`'s eligibility — both
/// `Copy` and `Eq` pass through it. A collection is never `Copy` because
/// `Vec` is not, but keeps `Eq` when its element does.
fn field_eligibility(ctx: &Ctx, field: &v2::Field, seen: &mut HashSet<String>) -> Eligibility {
    let Some(ft) = field.r#type.as_ref() else {
        return Eligibility::NONE;
    };
    let base = match &ft.kind {
        Some(v2::field_type::Kind::InlineScalar(td)) => scalar_eligibility(td),
        Some(v2::field_type::Kind::Named(name)) => type_ref_eligibility(ctx, name, seen),
        Some(v2::field_type::Kind::Primitive(p)) => primitive_eligibility(*p),
        Some(v2::field_type::Kind::Array(a)) => {
            let inner = a
                .element
                .as_deref()
                .map(|e| field_type_eligibility(ctx, e, seen))
                .unwrap_or(Eligibility::NONE);
            Eligibility { copy: false, eq: inner.eq }
        }
        Some(v2::field_type::Kind::Map(m)) => {
            let key = m
                .key
                .as_deref()
                .map(|k| field_type_eligibility(ctx, k, seen))
                .unwrap_or(Eligibility::NONE);
            let value = m
                .value
                .as_deref()
                .map(|v| field_type_eligibility(ctx, v, seen))
                .unwrap_or(Eligibility::NONE);
            Eligibility { copy: false, eq: key.eq && value.eq }
        }
        Some(v2::field_type::Kind::Tuple(t)) => t
            .fields
            .iter()
            .fold(Eligibility::ALL, |acc, f| {
                // A `TupleField` is not a `Field`: it carries `name` and
                // `r#type` only, so it goes through the FieldType-taking twin.
                let inner = f
                    .r#type
                    .as_ref()
                    .map(|ft| field_type_eligibility(ctx, ft, seen))
                    .unwrap_or(Eligibility::NONE);
                acc.meet(inner)
            }),
        _ => Eligibility::NONE,
    };
    base
}

/// A named reference. A same-package name recurses; a dotted or unknown one
/// cannot be proven and therefore disables every conditional derive.
fn type_ref_eligibility(ctx: &Ctx, reference: &str, seen: &mut HashSet<String>) -> Eligibility {
    match ctx.lookup(reference) {
        Some(decl) => decl_eligibility(ctx, decl, seen),
        None => Eligibility::NONE,
    }
}

fn primitive_eligibility(p: i32) -> Eligibility {
    match v2::PrimitiveType::try_from(p) {
        Ok(v2::PrimitiveType::Float) => Eligibility { copy: true, eq: false },
        Ok(v2::PrimitiveType::Integer) | Ok(v2::PrimitiveType::Boolean) => Eligibility::ALL,
        Ok(v2::PrimitiveType::String) | Ok(v2::PrimitiveType::Bytes) => {
            Eligibility { copy: false, eq: true }
        }
        _ => Eligibility::NONE,
    }
}
```

Add `field_type_eligibility(ctx, ft, seen)` as the `FieldType`-taking twin of
`field_eligibility`; they differ only in unwrapping the `Field` envelope, so
factor the `match` into it and have `field_eligibility` delegate.

The `v2::field_type::Kind` variants at 86e10d7 are `Named(String)`,
`Primitive(i32)`, `InlineScalar(TypeDef)`, `Tuple(TupleType)`,
`Array(ArrayType)`, `Map(MapType)` and `Stream(StreamType)`
(`crates/ridl-ir/proto/ridl/ir/v2/ir.proto:355`). `Stream` falls to the
`_ => Eligibility::NONE` arm. `ArrayType::element`, `MapType::key` and
`MapType::value` are each a single boxed `FieldType`, so `.as_deref()` yields
`Option<&FieldType>`. Re-read the proto before writing this anyway: a mismatch
surfaces as a compile error rather than silently, but reading is faster.

In `lib.rs`: add `mod derives;` beside `mod defaults;` and prepend the attribute
in `emit_decl` so every emitted item carries it. No visibility change is needed
— `pub(crate) struct Ctx<'a>` (`lib.rs:183`) and `pub(crate) fn lookup`
(`lib.rs:208`) are already `pub(crate)`, and `defaults.rs` already uses
`crate::Ctx`.

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then:
`cargo test -p ridl-backend-rust --locked && cargo clippy -p ridl-backend-rust --all-targets -- -D warnings`
Expected: PASS. The `rustc` compile proofs are the real check that no unsound
derive was emitted — an ineligible `#[derive(Copy)]` fails there.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/
git commit -m "feat(ridl-backend-rust): derive the sound traits on the typl surface

Debug, Clone, and PartialEq on every type; Copy, Eq, and Hash where the
transitive closure permits; PartialOrd and Ord on numeric named scalars
only. Default is never derived - it comes from the typl init value.

Cross-package references disable the conditional derives, because an
unsound derive is a hard error in the consumer's build rather than a
graceful failure."
```

---

### Task 7: `--emit rust` writes a compiling crate

**Model:** Opus (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**Files:**

- Modify: `crates/ridlc/src/lib.rs` — `write_emits`, and the `Emit::Rust` arm
- Modify: `crates/ridl-backend-rust/Cargo.toml` — remove the
  `minijinja = "2.5.0"` dependency, which no `.rs` file in the workspace uses
  since commit `7d539bc` deleted `c_header.rs` and its template
- Test: `crates/ridlc/src/lib.rs` tests, or `crates/ridl/tests/`

The manifest and the crate root are built as plain strings, not from a template.
This plan first said to create `crates/ridlc/templates/cargo.toml.j2` "matching
the existing `templates/c_header.j2` precedent"; that precedent is gone —
`crates/ridl-backend-rust/templates/c_header.j2` was deleted by the same
retraction commit, no `.j2` file remains anywhere in the repository, and `ridlc`
has never depended on `minijinja`. Two short `format!` bodies need no templating
engine, and the crate root in step 3 below is already built that way.

**Interfaces:**

- Consumes: the set of package names in `checked` (already available in
  `run_build`).
- Produces: `<out_dir>/Cargo.toml` and `<out_dir>/lib.rs` whenever `Emit::Rust`
  is selected in package or workspace mode.

Single-file mode keeps writing only `<stem>.rs`, matching the existing
single-file asymmetry documented on `Emit::TypeScript`.

driftsys/ridl#252's constraint no longer bites here. It required that whatever
the backends emit once per package be "nameable across every package in a
workspace at once"; Task 2 settled that by taking the constraint error from
`ridl-rt` rather than generating one, so there is no per-package vocabulary left
to hoist. What this task must still do is add `ridl-rt` to the generated
manifest, with the version read from `crates/ridl-rt/Cargo.toml` rather than
written as a literal.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rust_emit_writes_a_compiling_crate() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = run_build(
        Path::new("tests/fixtures/workspace"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(!run.has_error());
    assert!(out.path().join("Cargo.toml").exists());
    assert!(out.path().join("lib.rs").exists());

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"validate-pattern\", \"std\"]"));
    assert!(manifest.contains("regex = { version = \"1\", optional = true }"));
    assert!(manifest.contains("ridl-rt = "));

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(lib.contains("pub mod veh"));
}
```

Use the workspace fixture the existing `ridlc` tests already build against; if
none exists, point it at `crates/ridl/tests/` fixtures.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridlc --locked rust_emit_writes_a_compiling_crate` Expected:
FAIL — `Cargo.toml` does not exist.

- [ ] **Step 3: Write the implementation**

Write the manifest from a `format!` body in `crates/ridlc/src/lib.rs`, with this
content:

```toml
# Generated by ridlc. Do not edit.
[package]
name = "{crate_name}"
version = "0.0.0"
edition = "2024"

[features]
default = ["validate-pattern", "std"]
# Enforce `match` patterns in generated constructors. Disable on a target
# that cannot carry the regex dependency; range and length checks are
# unaffected.
validate-pattern = ["dep:regex"]
std = []

[dependencies]
ridl-rt = "0.1"
regex = { version = "1", optional = true }

[lib]
path = "lib.rs"
```

`ridl-rt` is not optional and carries no feature here: it is `no_std`, has no
dependency of its own in any feature combination, and the generated constructors
name `payload::Violation` from it (Task 2). Read the version from
`crates/ridl-rt/Cargo.toml` at emit time rather than writing a literal, so the
two cannot drift.

Emit `lib.rs` from the package names. Each dotted name becomes a path in a
nested `mod` tree whose leaf carries `#[path]`, so the emitted file names stay
flat while `crate::veh::common` resolves:

```rust
/// Builds the module tree that makes the flat emitted files reachable at the
/// `crate::…` paths generated code already uses.
///
/// `veh.common` and `veh.adas` produce one `veh` module containing two leaves,
/// each pointing at its flat file. Sorted so the output is deterministic.
fn render_lib_rs(package_names: &[String]) -> String {
    #[derive(Default)]
    struct Node {
        children: std::collections::BTreeMap<String, Node>,
        file: Option<String>,
    }

    let mut root = Node::default();
    for name in package_names {
        let mut node = &mut root;
        for segment in name.split('.') {
            node = node.children.entry(segment.to_string()).or_default();
        }
        node.file = Some(format!("{name}.rs"));
    }

    fn render(node: &Node, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        for (segment, child) in &node.children {
            match &child.file {
                Some(file) if child.children.is_empty() => {
                    out.push_str(&format!("{pad}#[path = \"{file}\"]\n"));
                    out.push_str(&format!("{pad}pub mod {segment};\n"));
                }
                _ => {
                    out.push_str(&format!("{pad}pub mod {segment} {{\n"));
                    render(child, depth + 1, out);
                    out.push_str(&format!("{pad}}}\n"));
                }
            }
        }
    }

    let mut out = String::from("// Generated by ridlc. Do not edit.\n\n");
    render(&root, 0, &mut out);
    out
}
```

A package name that is a strict prefix of another (`veh` alongside `veh.common`)
lands in the `_` arm, which emits an inline `pub mod veh { … }` and drops the
prefix package's own file. Add a test for that case and emit the file as
`#[path]` on an inner `mod` if it occurs; `ridl.toml` naming makes it unlikely
but not impossible.

The crate name comes from the manifest's package name when one is present,
falling back to `ridl_generated`. See Open item 1 — if a `--crate-name` flag is
added, it takes precedence over both.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ridlc --locked && just test` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridlc/
git commit -m "feat(ridlc)!: emit a compiling crate from --emit rust

Generated Rust references cross-package types as crate::veh::common, so it
has always assumed every package is a module inside one crate while
supplying neither the module tree nor a manifest. --emit rust now writes
lib.rs and one Cargo.toml alongside the package sources.

The manifest is what lets codegen own the validate-pattern feature: regex
cannot be an optional dependency of a bare .rs file.

BREAKING CHANGE: --emit rust writes two additional files in package and
workspace mode. Single-file mode is unchanged."
```

---

### Task 8: Pattern validation behind `validate-pattern`

**Model:** Opus (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `constraint_checks`
- Test: `crates/ridl-backend-rust/src/tests.rs`

**The feature is on by default**, per design spec decision 4 and Task 7's
manifest, and a constrained target builds `--no-default-features`. The story
issue, driftsys/ridl#253, says "the feature is off by default"; the design spec
is authoritative where the two disagree (see the `Spec:` line above). Reconcile
the issue text in a comment when the task lands rather than changing the
default.

**Interfaces:**

- Consumes: `constraint_checks` (Task 3), the manifest feature (Task 7).
- Produces: nothing new; extends the generated `new`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pattern_check_is_feature_gated() {
    let source = rust_for(vec![vin_decl()]);
    assert!(source.contains("#[cfg(feature = \"validate-pattern\")]"));
    assert!(source.contains("Rule::Pattern"));
    // The length check is not gated - it needs no dependency.
    let gated = source.split("#[cfg(feature = \"validate-pattern\")]").next().unwrap();
    assert!(gated.contains("Rule::Length"));
}
```

`vin_decl` is a `string` type carrying `len_min == len_max == 17` and a
`pattern`; build it with the existing `primitive_type` helper.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridl-backend-rust --locked pattern_check_is_feature_gated`
Expected: FAIL — no `cfg` attribute is emitted.

- [ ] **Step 3: Write the implementation**

Append to `constraint_checks`, after the length checks:

```rust
if let Some(pattern) = c.pattern.as_deref() {
    // The pattern needs a regex engine, which `core` has none of. The
    // range and length checks above stay unconditional; only this one is
    // gated, so a `--no-default-features` build still validates bounds.
    let source = strip_regex_delimiters(pattern);
    checks.push(quote! {
        #[cfg(feature = "validate-pattern")]
        {
            static PATTERN: std::sync::LazyLock<regex::Regex> =
                std::sync::LazyLock::new(|| {
                    regex::Regex::new(#source).expect("ridlc emitted an invalid pattern")
                });
            if !PATTERN.is_match(&#value) {
                return Err(Violation { type_name: #type_name, rule: Rule::Pattern });
            }
        }
    });
}
```

Also emit a doc line on the type naming that the pattern is enforced only under
the feature, so the guarantee is not silently variable.

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS. The `rustc`
compile proofs run without the feature, so they exercise the gated-out path; the
enabled path is covered by Task 7's emitted crate building under default
features.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/
git commit -m "feat(ridl-backend-rust): check match patterns under validate-pattern

Range and length checks stay unconditional, so a --no-default-features
build still validates bounds. Only the pattern check needs a regex engine,
which core does not have."
```

---

### Task 9: TypeScript vocabulary and factories — moved to step 2

This task is not executed here. The TypeScript value layer moved out of Epic 10
on 2026-09-12 with the release re-scope, and this plan follows it.

**What moved.** The task body that stood here emitted a `ConstraintError` class,
a `TryResult<T>` type, and a `speed` / `trySpeed` / `speedUnchecked` factory
trio per named scalar from `crates/ridl-backend-ts/src/lib.rs`. It is superseded
rather than carried over: the TypeScript backend is rebuilt in step 2 as three
modules (`docs/wip/2026-09-12-release-scope-and-plugin-system-design.md` §3.5)
and is also built as a `ridlc-gen-ts` plugin binary over the lowered codegen
model (§3.8 of the same note), so a value layer written against today's emitter
would be written twice. The surface the design spec fixes — the brand kept, the
runtime value left a primitive, three functions for a constrained type and one
for a vacuous one — still stands as the specification of what step 2 builds.

**The tracker half is already done.** Story E10.9 (driftsys/ridl#254) was closed
as not planned on 2026-09-12, along with E10.11, and refiled under §3.5 and §3.8
of the release-scope note. `docs/ROADMAP.md`'s Epic 10 section records the move.
**Do not reopen driftsys/ridl#254.** No issue is filed or reopened for this
task; step 2's TypeScript stories carry it.

**What this leaves open in Epic 10.** Task 1's classifier has a two-backend
done-when — "both backends agree on which types need a fallible constructor" —
and only the Rust half completes here. The TypeScript half completes in step 2,
against the same `ridl_ir::v2::constraint_is_vacuous`, which is why the
classifier stays in `ridl-ir` rather than moving into the Rust backend. The
roadmap says the same beneath the Epic 10 table.

---

### Task 10: Record the decision and verify the diff classification

**Model:** Opus (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**Files:**

- Modify: `docs/decisions/ADR-0013-codegen-backend-scope.md`
- Modify: `docs/specification/typl-language-reference.md` — §5.7. This file is
  shared: the order is C1 (E14.1), then this task, then E14.3
  (`docs/wip/2026-09-13-step1-lanes-plan.md` §6). Do not open a pull request
  touching it while another lane's is open.
- Modify: `docs/wip/typl-value-objects-design.md` — the `ConstraintError`
  paragraph under "The Rust surface", and the `ConstraintError` occurrences in
  decisions 1 to 4 and in the surface listings. The spec says the type "joins
  the dependency-free package vocabulary the `interact` module already emits
  beside `Provenance` and `SignalHandle`". That module was retracted by
  driftsys/ridl#241, `Provenance` now lives in `ridl-rt`, and Task 2 takes the
  type from there too. Correct the spec before archiving it, so the archived
  record does not contradict the code that shipped from it.
- Delete: `docs/wip/typl-value-objects-design.md`,
  `docs/wip/typl-value-objects-plan.md` (archive per the working-memory rule)

- [ ] **Step 1: Verify the open question about `ridl-diff`**

Design spec Open item 2: decision 3's `From` → `TryFrom` flip assumes
`ridl-diff` classifies a constraint appearing where none existed as breaking.

Run: `cargo test -p ridl-diff --locked` Then read `crates/ridl-diff/src/` for
the constraint comparison, and add a test covering `min: None` →
`min: Some("0")`. If it is not classified as breaking, that is a separate defect
— record it as a GitHub issue and note it in the ADR rather than fixing it here.

- [ ] **Step 2: Amend ADR-0013**

Add to decision 1 that a language backend emits validating constructors and the
sound derives, giving the language class its positive definition. Move Open item
1 to a decision if the answer is now clear, or leave it and say why.

- [ ] **Step 3: Amend typl §5.7**

§5.7 currently says codegen realises nominal types "as distinct wrapper types
where the target allows (Rust newtype, Kotlin `value class`)". Extend it to say
the wrapper enforces the type's constraints at construction in a language
backend, and cite ADR-0013.

- [ ] **Step 4: Garden the working memory**

Per `sdd-working-memory-lifecycle`, move both `docs/wip/` files to
`docs/archive/` once the durable records above are written. Run the
`sdd-gardening` skill rather than doing it by hand. Task 11 is part of this plan
and runs independently of this task, so do this step only after Task 11 has also
landed; otherwise Task 11 loses the document it executes.

- [ ] **Step 5: Run the full gate and commit**

```bash
just verify
git add docs/
git commit -m "docs(typl): record validating constructors in ADR-0013 and typl 5.7"
```

---

### Task 11: The two Rust naming defects

**Model:** Fable (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

Clears driftsys/ridl#243 (a struct field name is emitted verbatim, so generated
Rust draws `non_snake_case`) and driftsys/ridl#237 (union arm names collide
under `camel_case`, emitting two variants of one name). Both moved into Epic 10
on 2026-09-15, recorded in `docs/ROADMAP.md`'s Epic 10 section and decided as
P-5 of `docs/wip/2026-09-13-step1-lanes-plan.md`: Task 6 changes the struct and
union declarations both defects are about, and Task 3 changes the named-scalar
emission in the same emitter, so clearing them here changes the snapshots once
instead of twice.

Read
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
decisions 1 to 5 before starting. The pinned transform is
`ridl_ir::name::snake_case` (`crates/ridl-ir/src/name.rs:24`); the Rust backend
is already a dependent of `ridl-ir`, so neither half needs a new transform
written.

**Run this task after Task 6.** Both halves rewrite the same snapshots Task 6
rewrites, and Task 6 is the larger change.

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_field` (`lib.rs:370`),
  `emit_union` (`lib.rs:436`), `camel_case` (`lib.rs:855`)
- Modify: `crates/ridl-ir/src/name.rs` — the pinned `camel_case`, which decision
  D moves here from the Rust backend
- Modify: `crates/ridl-sem/src/check.rs` — RIDL-149 over a union's arms, beside
  the existing member, parameter and struct-field namespaces
- Modify: `crates/ridlc/tests/corpus.rs` — the `rustc_accepts` lint list
- Test: `crates/ridl-backend-rust/src/tests.rs`,
  `crates/ridl-sem/src/check.rs`'s test module, `crates/ridlc/tests/corpus.rs`

**Not a shared file.** `crates/ridl-core/src/diag.rs` is in the §6 shared-file
order (S1 and S2 → L4 → C3 → B3) and this task does not touch it, because
decision D reuses RIDL-149 rather than minting a code.

**Interfaces:**

- Consumes: `ridl_ir::name::snake_case`.
- Produces: `pub fn ridl_ir::name::camel_case(name: &str) -> String` (decision
  D), and RIDL-149 over one more namespace.

#### Decision D — what #237 does about a colliding union arm

**DECIDED 2026-09-17 by Sebastien: report a diagnostic. Extend RIDL-149 to a
union's arms, keyed on both pinned transforms, and move `camel_case` into
`ridl-ir` beside `snake_case`. No rename.** The #237 half of this task is
unblocked; implement what follows. The #243 half never waited on it.

The options were: rename the arm, report a diagnostic, or both. The reasoning
for the one taken is kept below, because the implementation depends on the third
point and a reader who skips it will write a check that does not close the
defect.

1. **Fail-closed is the rule this defect class already has.** RIDL-149 was
   minted for exactly "two names distinct in source projecting to one
   identifier" (ADR-0016 decision 3) and it rejects the package; it does not
   rename. A rename invents an identifier the contract does not state, and the
   suffix it picks would move as arms are added, so a generated API would change
   under a change ADR-0016 decision 6 calls compatible.
2. **Two backends already refuse it, less helpfully.** The proto3 backend
   (`crates/ridl-backend-proto/src/lib.rs:496`) and the FlatBuffers backend
   (`crates/ridl-backend-flatbuffers/src/lib.rs:344`) both project an arm name
   through `ridl_ir::name::snake_case` and claim it in a symbol table that
   errors on a second claim. So a colliding package is already unbuildable for
   two of the four backends, as a `GenerateError` at generate time. A check in
   `ridl-sem` moves the refusal to `ridl check`, where it names the two arms in
   the source, and makes the backends' refusal unreachable for a checked package
   — the relationship the proto3 backend's own comment describes (`lib.rs:473`).
3. **The check cannot key on `snake_case` alone, and this is the part that is
   easy to get wrong.** The two transforms are incomparable: neither collision
   set contains the other. Three arm pairs, each computed by running the two
   functions as they stand on `main` (re-verified 2026-09-17):

   | Arms                       | `camel_case`                          | `snake_case`                            |
   | -------------------------- | ------------------------------------- | --------------------------------------- |
   | `foo_bar`, `fooBar`        | `FooBar`, `FooBar` — collides         | `foo_bar`, `foo_bar` — collides         |
   | `XY`, `x_y`                | `XY`, `XY` — collides                 | `xy`, `x_y` — distinct                  |
   | `HTTPServer`, `httpServer` | `HTTPServer`, `HttpServer` — distinct | `http_server`, `http_server` — collides |

   Row 2 is the Rust defect a `snake_case`-keyed check would pass through, and
   row 3 is the wire-backend refusal a `camel_case`-keyed check would pass
   through. So the union arm namespace is checked under **both** transforms, and
   the package is rejected when either collides. Row 2 and row 3 each become a
   test.
4. **The transform has to leave the backend for the check to key on it.**
   ADR-0016 decision 5 puts the check in `ridl-sem`, which does not depend on a
   backend, and decision 2's argument — a projection is a pure function from IR
   identity to a target namespace, so it belongs with the IR — applies to
   `camel_case` unchanged. One definition, two callers: the check and the Rust
   emitter. driftsys/ridl#237's own note raises this as the question worth
   settling first.

**What the decision costs, stated because it is a real break, and the first
draft of this paragraph got it wrong.** Two kinds of package become newly
rejected, and they are not alike — read the table above before reading this.

- **Arms `XY` and `x_y`** collide under `camel_case` only. Today `ridl check`
  passes them, the Rust backend emits E0428, and **both wire backends accept
  them**, because `snake_case` gives `xy` and `x_y`, which are distinct. After
  this change `ridl check` rejects the package. That is the defect being fixed:
  a check-time refusal replaces a `rustc` failure further down.
- **Arms `HTTPServer` and `httpServer`** collide under `snake_case` only. Today
  `ridl check` passes them, **the Rust backend compiles them fine** — the
  variants are `HTTPServer` and `HttpServer` — and only the wire backends refuse
  with a `GenerateError`. After this change `ridl check` rejects the package.
  **This is the genuine new cost**: a package that a Rust-only consumer builds
  successfully today starts failing at check time, because the contract must
  hold for every backend rather than the ones a given consumer happens to use.

An earlier draft asserted that the `XY` pair "already fails both wire backends",
which contradicted this document's own table three paragraphs above. It does
not. The correction matters because it moves the cost from the pair that is
already broken everywhere to the pair that is not.

No working consumer breaks either way. `ridlc`, `ridl-ir` and `ridl-core` exist
on crates.io only as `0.0.0` name reservations with no usable content, and
`ridl-rt`, whose 0.1.0 release is prepared, generates nothing and holds no
union.

**Why not "both".** A rename only ever applies to a package the diagnostic
already rejects, so "both" means downgrading the diagnostic to a warning and
letting `ridlc` choose the variant names. That is a different decision — fail
closed, or let the tool name things the contract does not — and it was not the
one taken.

**Recording it.** ADR-0016 is Accepted, and its consequences already record this
defect as one the record does not close (the entry ending "Recorded on
driftsys/ridl#237"). Write the amendment back there: union arms join the checked
namespaces, and `camel_case` joins the pinned transforms. Do it in Task 10's
documentation pull request or in this task's; it must not be skipped in both.

#### The #243 half

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_struct_field_name_is_projected_to_snake_case() {
    // typl §15.1 makes field names camelCase, so an untransformed name
    // draws `non_snake_case` at every consumer of the generated module.
    let source = rust_for(vec![struct_with_field("Reading", "sensorId", "Speed")]);
    assert!(source.contains("pub sensor_id:"), "got:\n{source}");
    assert!(!source.contains("pub sensorId:"));
}
```

`struct_with_field(name, field_name, type_ref)` is the fixture builder Task 6
adds; this task runs after Task 6, so it is present.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridl-backend-rust --locked a_struct_field_name_is_projected`
Expected: FAIL — `emit_field` renders `ident(&field.name)` with no transform, so
the emitted field is `pub sensorId`. The same string is visible in the committed
snapshots today: grep `crates/ridl-backend-rust/src/snapshots/` for `sensorId`.

- [ ] **Step 3: Write the implementation**

In `emit_field` (`lib.rs:370`), render the name through the pinned transform:

```rust
let field_name = ident(&ridl_ir::name::snake_case(&field.name));
```

The `hint` on the next line, which names an induced tuple struct, keeps
`camel_case(&field.name)` — it builds a type name, not a field name.

Nothing needs minting: ADR-0016 decision 4 held struct fields out of RIDL-149
"until E9.8 extends both the transform and this check to them in the commit that
starts projecting them", and E9.8 did extend the check —
`crates/ridl-sem/src/check.rs:5837`, "RIDL-149 over struct fields (ADR-0016
decision 4)". So the collision hazard is closed before this change lands, which
is the order that decision asked for, reached from the other side.

Then add the regression guard. No `rustc` compile proof fails on a warning
today, which is why this defect never failed a test, and the four differ in how
they reach that. `appendix_b_compiles_with_rustc`
(`crates/ridl-backend-rust/src/tests.rs:1232`) and
`constructible_collections_compile` (`tests.rs:1302`) pass no `-D` flag at all
and assert only `status.success()`.
`a_tuple_under_an_internal_declaration_is_package_private` (`tests.rs:611`) and
`rustc_accepts` in `crates/ridlc/tests/corpus.rs:1208` each deny two lints by
name, `private-interfaces` and `private-bounds`, and nothing else. Add
`-D non_snake_case` to all four — for the first two that means adding a deny
flag where there was none. Check first that no other generated item draws it —
an enum variant keeps its typl `SCREAMING_SNAKE` spelling and draws
`non_camel_case_types`, a different lint, which stays undenied.

The TypeScript backend is not touched: camelCase is idiomatic there, and
driftsys/ridl#243 says so.

- [ ] **Step 4: Run the tests and accept the snapshots**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test --workspace --locked` Expected: PASS. Every snapshot with a
multi-word field changes, in this crate and in the `ridlc` corpus. The proto3
and FlatBuffers snapshots do not change — they already project through
`snake_case`, so their output is the same before and after.

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "fix(ridl-backend-rust)!: project a struct field name to snake_case

A field name reached generated Rust verbatim, so every multi-word typl
field drew non_snake_case at every consumer. It now goes through the
pinned transform, ridl_ir::name::snake_case (ADR-0016 decisions 1 and 2).
RIDL-149 already rejects a package whose field names would collide under
that transform, so the collision hazard was closed before this change.

The compile proofs now deny non_snake_case by name, which is what would
have caught this.

BREAKING CHANGE: a multi-word field is renamed on every generated Rust
struct. Read it at its snake_case name.

Closes #243."
```

#### The #237 half

Unblocked: decision D was taken on 2026-09-17, as proposed. The steps below
implement it.

- [ ] **Step 6: Write the failing tests**

In `crates/ridl-ir/src/name.rs`, for the moved transform:

```rust
#[test]
fn the_two_transforms_are_incomparable() {
    // Neither collision set contains the other, which is why a union's
    // arms are checked under both (ADR-0016 amendment, Task 11 decision D).
    assert_eq!(camel_case("XY"), camel_case("x_y"));
    assert_ne!(snake_case("XY"), snake_case("x_y"));
    assert_ne!(camel_case("HTTPServer"), camel_case("httpServer"));
    assert_eq!(snake_case("HTTPServer"), snake_case("httpServer"));
}
```

In `crates/ridl-sem/src/check.rs`'s test module, beside the struct-field cases
at `check.rs:5837`, one case per row of decision D's table: arms `foo_bar` and
`fooBar` draw RIDL-149, arms `XY` and `x_y` draw RIDL-149, arms `HTTPServer` and
`httpServer` draw RIDL-149, and a union whose arms collide under neither draws
nothing.

- [ ] **Step 7: Run the tests to verify they fail**

Run: `cargo test -p ridl-ir --locked the_two_transforms_are_incomparable` and
`cargo test -p ridl-sem --locked union_arm` Expected: FAIL — `camel_case` is not
in `ridl-ir`, and no check covers a union's arms.

- [ ] **Step 8: Write the implementation**

Move `camel_case` from `crates/ridl-backend-rust/src/lib.rs:855` to
`crates/ridl-ir/src/name.rs` as a `pub fn`, with a doc comment that says what
`snake_case`'s says: the transform is not injective, and a package whose names
collide under it is rejected by RIDL-149. Delete the backend copy and have
`emit_union` and the induced-tuple `hint` call the pinned one. The behaviour
does not change, so no snapshot moves in this step.

Then extend the RIDL-149 check. `Checker::colliding_projected_name`
(`check.rs:3306`) is already written as "two names in one scope that collide
after the pinned name transform" and is already shared by the member and
parameter checks; add a union's arms as a fourth namespace, keyed on both
transforms — a package is rejected when either collides. A reserved arm is
skipped, matching `emit_union`.

- [ ] **Step 9: Run the tests**

Run:
`cargo test -p ridl-ir -p ridl-sem -p ridl-backend-rust --locked && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. Add the new diagnostic to the corpus expectation table
(`crates/ridlc/tests/corpus.rs:983` lists RIDL-149 already) if a corpus fixture
exercises it.

- [ ] **Step 10: Commit**

```bash
git add crates/
git commit -m "fix(ridl-sem): reject a union whose arms collide after a name transform

ridlc accepted a union with arms foo_bar and fooBar and emitted a Rust enum
with the variant FooBar twice, which rustc rejects with E0428, while ridl
check reported nothing. RIDL-149 now covers a union's arms.

The arms are checked under both pinned transforms, because neither
collision set contains the other: XY and x_y collide under camel_case but
not under snake_case, and HTTPServer and httpServer collide under
snake_case but not under camel_case. camel_case moves to ridl-ir beside
snake_case for the same reason snake_case is there - the check runs in
ridl-sem, which depends on no backend (ADR-0016 decisions 2 and 5).

Closes #237."
```

---

## Verification

Before opening the PR:

```bash
just verify     # lint-commits, then the full build gate
```

`just build` covers `toolchain-check`, `gate-parity`, `install-check`,
`fmt-check`, `book-check`, `link-check`, `compile`, `test`, `lint`,
`wasm-check`, `compat-check`, and `check`. This plan's own two `docs/wip/` files
are archived by Task 10 step 4, after Task 11 has landed;
`sdd-working-memory-lifecycle` asks that of the documents a piece of work owns,
not of the whole directory, which carries other lanes' working memory.

## Open

1. **The crate name for the generated `Cargo.toml`** (Task 7). Default proposed:
   the `ridl.toml` package name, falling back to `ridl_generated`, overridable
   by a `--crate-name` flag. Confirm before implementing Task 7.
2. **Whether `ridl-diff` classifies a constraint appearing where none existed as
   breaking** (Task 10, Step 1). Verify rather than assume.
3. **Where the constraint error is defined — answered 2026-09-16, not open.** It
   is `ridl_rt::payload::Violation`; the generated code defines no error type at
   all. Task 2 carries the reasoning and the one cost. Kept here as a numbered
   item because Task 7's manifest and Tasks 3, 4, 5 and 8 all rest on it, and
   because the design spec still says otherwise — the spec is dated 2026-08-03,
   before `ridl-rt` existed, and Task 10 is where that sentence is corrected.
4. **What a colliding union arm does — decided 2026-09-17, not open.** Report a
   diagnostic: extend RIDL-149 to a union's arms over both pinned transforms,
   move `camel_case` into `ridl-ir`, no rename. Kept here as a numbered item
   because Task 11's #237 half rests on it and because ADR-0016's consequences
   still record the defect as one that record does not close — Task 11 or Task
   10 writes the amendment. The reasoning is in Task 11 under "Decision D".
5. **`Violation` implements neither `Display` nor `std::error::Error`**
   (`crates/ridl-rt/src/payload.rs:177`), and `ridl-rt` declares no `std`
   feature. Every generated constructor returns this type, so a consumer writing
   `fn main() -> Result<(), Box<dyn Error>>` cannot apply `?` to it. Both
   additions are additive rather than breaking under ADR-0021 decision 10 — that
   rule is about a public struct gaining a _field_ — so this can ship in a later
   0.x without a break, and none of Epic 10 is blocked on it. It is recorded
   here because Epic 10 is what makes the gap reachable: before this epic,
   nothing outside a runtime ever held a `Violation`.
