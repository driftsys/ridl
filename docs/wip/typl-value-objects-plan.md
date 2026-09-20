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

**A fourth change followed on 2026-09-17, from review of driftsys/ridl#413.**
Task 2 first emitted `use ridl_rt::payload::{Rule, Violation};` per package
module. That is not total: a typl package may legally declare a type named
`Rule` or `Violation`, and the import collides with it (`rustc` reports
`E0255`). The generated code now names `::ridl_rt::payload::Violation` by
absolute path and imports nothing, so no typl name can collide with it. Task 2
therefore emits nothing; its code blocks and those of Tasks 3, 5 and 8 are
rewritten below to the absolute path.

**A fifth change followed on 2026-09-20, from Task 3 landing in
driftsys/ridl#420.** That pull request took two decisions about the emitted code
that this plan had not absorbed. Tasks 4, 5 and 8 all inherit the first, and
Task 8 also inherits the second — Tasks 4 and 5 emit no range or length check,
so there is nothing there for the second to guard. Executing any of the three
against the text as written would either fail to compile or be repaired by
reintroducing the defect #420 removed. Both decisions and their reasons are
recorded at the top of Task 3. The corrections made here:

- **Task 3** is recorded as landed, its steps are ticked, and its Step 1
  assertions and `quote!` bodies are rewritten to the code that is on `main`.
- **Task 4** Step 1 and Step 3 name `::core::convert::From` and
  `::core::convert::TryFrom`, and Step 3 says where the vacuous branch goes,
  which Task 3's quoted function no longer shows.
- **Task 5** Step 1 and both Step 3 blocks name `::core::convert::TryFrom`,
  `::core::convert::From`, `::core::result::Result`,
  `::core::result::Result::Ok` and `::core::result::Result::Err`. Three stale
  line references into `lib.rs` were replaced by the symbol names.
- **Tasks 4 and 5** also record a third thing driftsys/ridl#420 settled that
  they inherit: a deprecated declaration's impl blocks carry
  `#[allow(deprecated)]`. Emitting those tasks' impl blocks without it would put
  back the consumer warnings that pull request removed.
- **Task 8** names `::core::result::Result::Err`, and settles the two crate
  paths in the emitted pattern block as `::std::sync::LazyLock` and
  `::regex::Regex`, with the reason and the rustc check recorded in the task.
  The `validate-pattern` default is unchanged: the design spec is authoritative
  over driftsys/ridl#253, which Task 8 already says to reconcile in a comment on
  that issue.
- **The design spec's decision 4** no longer says `min`/`max` and
  `len_min`/`len_max` are checked unconditionally. It states the two guards, and
  a third skip that predates them: a range check is emitted only for a float or
  an integer backing, because `min` and `max` are numeric bounds.

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
  `pattern`, and `pattern_const` are all absent. `step` is deliberately ignored
  — step is not validated (spec, "Not validated").
- Review of the pull request added the `pattern_const` read and the one-sided
  fixtures.

- [ ] **Step 1: Write the failing test**

Add a test module of its own in `crates/ridl-ir/src/lib.rs`, beside
`mod v2_round_trip`, as the Files list above says:

```rust
#[cfg(test)]
mod vacuous_constraint {
    use crate::v2;

    /// A constraint with every field absent. Each test sets only the field it
    /// is about, so no assertion can pass through a neighbouring field.
    fn constraint() -> v2::Constraint {
        v2::Constraint {
            min: None,
            max: None,
            step: None,
            len_min: None,
            len_max: None,
            pattern: None,
            pattern_const: None,
        }
    }

    #[test]
    fn an_absent_or_empty_constraint_is_vacuous() {
        assert!(v2::constraint_is_vacuous(None));
        assert!(v2::constraint_is_vacuous(Some(&constraint())));
    }

    #[test]
    fn vacuous_constraint_ignores_step() {
        // A declared step alone leaves a constructor nothing to check: nothing
        // checks a step today, and the design rounds to the lattice rather
        // than rejecting (design spec, Deferred, not yet implemented).
        let stepped = v2::Constraint {
            step: Some("0.5".to_string()),
            ..constraint()
        };
        assert!(v2::constraint_is_vacuous(Some(&stepped)));
    }

    /// Every constrained field on its own. A fixture setting a pair — `min`
    /// with `max`, or `len_min` with `len_max` — cannot tell a predicate that
    /// reads both from one that reads either, so each bound here is one-sided.
    /// The paired shapes are pinned separately by
    /// [`a_bound_pair_set_together_is_non_vacuous`], which a one-sided fixture
    /// cannot do.
    #[test]
    fn any_single_constrained_field_is_non_vacuous() {
        let cases = [
            (
                "min",
                v2::Constraint {
                    min: Some("0.0".to_string()),
                    ..constraint()
                },
            ),
            (
                "max",
                v2::Constraint {
                    max: Some("250.0".to_string()),
                    ..constraint()
                },
            ),
            (
                "len_min",
                v2::Constraint {
                    len_min: Some(1),
                    ..constraint()
                },
            ),
            (
                "len_max",
                v2::Constraint {
                    len_max: Some(256),
                    ..constraint()
                },
            ),
            (
                "pattern",
                v2::Constraint {
                    pattern: Some("^[a-z]+$".to_string()),
                    ..constraint()
                },
            ),
            (
                "pattern_const",
                v2::Constraint {
                    pattern_const: Some("NAME_PATTERN".to_string()),
                    ..constraint()
                },
            ),
        ];
        for (field, case) in cases {
            assert!(
                !v2::constraint_is_vacuous(Some(&case)),
                "`{field}` alone must be non-vacuous"
            );
        }
    }

    /// The two shapes the checker actually emits: a declared range, and the
    /// typl §4.4 default `[0..256]` every string and bytes type carries.
    ///
    /// A one-sided fixture cannot pin these. A predicate reading each bound as
    /// a pair — `(c.min.is_none() == c.max.is_none())` and the same for the
    /// length bounds — passes every one-sided case and still reports both
    /// shapes below as vacuous, which would drop the range check from every
    /// bounded number and every string.
    #[test]
    fn a_bound_pair_set_together_is_non_vacuous() {
        let ranged = v2::Constraint {
            min: Some("0.0".to_string()),
            max: Some("250.0".to_string()),
            ..constraint()
        };
        assert!(!v2::constraint_is_vacuous(Some(&ranged)));

        let default_length = v2::Constraint {
            len_min: Some(0),
            len_max: Some(256),
            ..constraint()
        };
        assert!(!v2::constraint_is_vacuous(Some(&default_length)));
    }
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
/// purpose: nothing checks a step today, and the design this classifier
/// prepares for rounds a value to the nearest step-lattice point rather than
/// rejecting it, so a step-only constraint is meant to admit a constructor
/// with nothing to check (design spec, Deferred, not yet implemented).
///
/// A pattern given by name counts as a pattern: `pattern_const` is read as
/// well as `pattern`, because a pattern constant that did not resolve leaves
/// `pattern` absent while the type still carries a match constraint.
/// `ridl-sem` treats the two fields the same way in its derived-init rule
/// (`init.rs`).
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
        && c.pattern_const.is_none()
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
constructor nothing to check. step is excluded: nothing checks a step
today, and the design rounds to the lattice rather than rejecting."
```

---

### Task 2: The constraint error is `ridl_rt::payload::Violation`

**Model:** Sonnet (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

**The generated code defines no error type and imports nothing. It names
`::ridl_rt::payload::Violation` by its absolute path.** This reverses what this
plan said before 2026-09-16, and it closes Open item 3 rather than proposing an
answer to it — see "Why not a generated type" below.

**The absolute path, and why there is no `use` line.** This task first emitted
`use ridl_rt::payload::{Rule, Violation};` at the top of each package module.
Review of driftsys/ridl#413 found that this is not total. A typl package may
legally declare a type named `Rule` or a type named `Violation`, and the import
then collides with the declaration. Reproduced with `rustc`:

```text
error[E0255]: the name `Violation` is defined multiple times
`Violation` must be defined only once in the type namespace of this module
```

So the constructors spell `::ridl_rt::payload::Violation` and
`::ridl_rt::payload::Rule::Range` instead. The leading `::` matters: it also
survives a package that declares a type named `ridl_rt`, which a bare
`ridl_rt::…` path resolves to instead of the crate — `rustc` reports `E0223` for
a declared type and `E0433` for a module of that name, and accepts the absolute
path in both cases. No name enters the generated module's namespace, so no typl
name can collide with it. This is the same totality over names that ADR-0017
requires of the proto3 projection.

**Task 2 therefore emits nothing.** What it delivers is the decision recorded
above, two sentences on `generate` and `emit_decl` that the interaction-layer
retraction had left untrue, and the compile-proof harness that builds `ridl-rt`
as an rlib. **Task 3 is where generated code first names the runtime**, and
where the compile proofs first pass `--extern ridl_rt=<path>`. Wiring the
`--extern` before then would make the argument inert, and an inert `--extern`
removes a real detector: a proof whose generated source wrongly names `ridl_rt`
fails today with `E0433`, and would pass silently with the extern wired.

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — two doc and comment sentences
  only. No emission changes.
- Test: `crates/ridl-backend-rust/src/tests.rs` — the `ridl_rt_rlib` helper and
  the one test that keeps it honest.

Task 7 still gains `ridl-rt` as a dependency of the generated manifest
(`crates/ridlc/src/lib.rs`); naming the type by absolute path is a dependency
like any other.

**Interfaces:**

- Consumes: nothing. There is no import to consume — Tasks 3, 5 and 8 name
  `::ridl_rt::payload::Violation` and `::ridl_rt::payload::Rule` by absolute
  path. Both exist today in `crates/ridl-rt/src/payload.rs:177` and `:186`.
- Produces: no type, and no generated source. The `ridl_rt_rlib` test helper,
  for the compile proofs Tasks 3, 5 and 8 will write.

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

- [ ] **Step 1: Correct the two untrue sentences in `lib.rs`**

`generate`'s doc comment said it "generates Rust source and the extern-C header
for `package`"; ADR-0018 retired the extern-C face and nothing emits a header.
`emit_decl`'s comment on its catch-all arm said interfaces and services "are
emitted by the `interact` module"; driftsys/ridl#241 deleted that module. Both
now read as they are: "Generates the Rust source for `package`." and "nothing
emits them today."

- [ ] **Step 2: Write the compile-proof harness**

Add `ridl_rt_rlib` to `crates/ridl-backend-rust/src/tests.rs`: one `rustc` call
over `crates/ridl-rt/src/lib.rs` producing an rlib in the caller's temp dir.
`ridl-rt` is `no_std` with no dependency in any feature combination (ADR-0021
decision 8), so one call is the whole build. It must be built by the same
`rustc` the proof itself spawns; an rlib built by another toolchain is rejected
with `E0514`.

- [ ] **Step 3: Make the harness non-vacuous**

No compile proof passes `--extern` yet, so the helper would be dead code and a
helper producing an unusable rlib would go unnoticed until Task 3 needed it. One
test keeps it honest, and fails if the helper breaks in either direction: it
writes source naming `::ridl_rt::payload::Violation` and
`::ridl_rt::payload::Rule::Range` in the shape generated code will use, compiles
it with `--extern ridl_rt=<the rlib>` and asserts success, then compiles the
same source with no `--extern` and asserts failure.

```rust
#[test]
fn the_compile_proof_harness_links_ridl_rt() { /* … */ }
```

- [ ] **Step 4: Run the gates**

Run: `cargo fmt --all`, then `just test`, then `just lint`. Expected: PASS, with
no snapshot changed — this task emits nothing, so no generated output moves.

**The compile proofs stay unwired.** `appendix_b_compiles_with_rustc` and the
others drive `rustc` directly on a single file with no `--extern`. Nothing they
compile names `ridl_rt`, so passing one would be inert, and worse: an inert
`--extern` removes a real detector, because a proof whose generated source
wrongly names `ridl_rt` fails today with `E0433` and would then pass silently.
Task 3 wires them when its generated code first needs them.

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "fix(ridl-backend-rust): name the runtime by absolute path, importing nothing

A typl package may legally declare a type named Rule or Violation, and the
use line collided with it: rustc reports E0255, the name defined twice in
the module's type namespace. Generated constructors will spell
::ridl_rt::payload::Violation instead, which no typl name can collide with,
including a package that declares a type named ridl_rt.

So this task emits nothing. What it leaves is the recorded decision, two
sentences on generate and emit_decl that the interaction-layer retraction
had left untrue, and the harness that builds ridl-rt as an rlib for the
compile proofs."
```

---

### Task 3: Constrained named scalars — private inner, `new`, `TryFrom`

**Model:** Fable (`docs/wip/2026-09-13-step1-lanes-plan.md` §4, stage C4).

The core change, and the breaking one.

**Landed in driftsys/ridl#420**, merged as `ddcbe85`. The task body below was
corrected on 2026-09-20 to the code that landed, because two decisions taken
during that pull request's review are not the code this task first described,
and Tasks 4, 5 and 8 inherit both:

1. **The generated code names the prelude absolutely.** A typl type name is
   CamelCase (typl §15.1) and `ridl-sem` reserves no identifier, so a package
   may legally declare `type Result`, `type Ok`, `type Err`, `type From` or
   `type TryFrom`, and that declaration shadows the prelude in the module the
   generated constructors share with it. The emitter writes
   `::core::convert::From`, `::core::convert::TryFrom`,
   `::core::result::Result`, `::core::result::Result::Ok` and
   `::core::result::Result::Err`. This is the same totality argument that put
   `::ridl_rt::payload::Violation` on an absolute path.
2. **A check rustc can fold to a constant is not emitted.** A length minimum of
   0 — the typl §4.4 and §4.5 default lower bound of string and bytes — emits no
   branch, because `(… as u64) < 0` draws rustc's `unused_comparisons` warning
   in the consumer's build. The maximum range check is skipped on the same
   grounds when an integer bound equals `i64::MAX`, which is the newtype
   backing's own maximum. Read the comments at those two branches in
   `constraint_checks` rather than this summary.

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_type_def`, `emit_const`
- Modify: `crates/ridl-backend-rust/src/defaults.rs` — the two tuple-struct
  construction sites, `Some(quote! { #name_id(#inner) })` (`defaults.rs:55`) and
  `Some(quote! { #path(#inner) })` (`defaults.rs:183`)
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `ridl_ir::v2::constraint_is_vacuous` (Task 1). Nothing from Task 2:
  there is no import to consume, so the emitted code names
  `::ridl_rt::payload::Violation` and `::ridl_rt::payload::Rule` by absolute
  path, and the prelude names it uses by absolute path for the same reason. This
  is the first task whose generated code names the runtime, so it is also where
  the compile proofs first pass `--extern ridl_rt=<path>`, built by Task 2's
  `ridl_rt_rlib` helper.
- Produces: for a constrained named scalar `Speed` over `f64`, an emitted
  `Speed::new(f64) -> ::core::result::Result<Speed, ::ridl_rt::payload::Violation>`,
  `Speed::new_unchecked(f64) -> Speed` (`pub const`), `Speed::get(self) -> f64`,
  `impl ::core::convert::TryFrom<f64> for Speed`,
  `impl ::core::convert::From<Speed> for f64`. Task 4 emits the vacuous
  counterpart; Task 8 adds the pattern branch inside `new`.

Both call sites that construct a newtype by tuple syntax must move to
`new_unchecked`, which is why it is `const`.

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn constrained_scalar_is_a_value_object() {
    let source = rust_for(vec![speed_decl()]);
    // The field is private: no `pub` inside the tuple struct.
    assert!(
        source.contains("pub struct Speed(f64)"),
        "inner field must be private, got:\n{source}"
    );
    // The generated code names `Result`, `TryFrom` and `From` by absolute
    // path, because a package may declare a type of the same name in the same
    // module; these assertions follow the generated code. The signature is
    // checked in two parts because prettyplease wraps it at its own width.
    assert!(source.contains("pub fn new("));
    assert!(source.contains(") -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {"));
    assert!(source.contains("pub const fn new_unchecked(value: f64) -> Self"));
    assert!(source.contains("pub const fn get(self) -> f64"));
    assert!(source.contains("impl ::core::convert::TryFrom<f64> for Speed"));
    assert!(source.contains("impl ::core::convert::From<Speed> for f64"));
    // The infallible inbound conversion must never appear on a constrained type.
    assert!(
        !source.contains("impl ::core::convert::From<f64> for Speed")
            && !source.contains("impl From<f64> for Speed"),
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

- [x] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-backend-rust --locked constrained_scalar_is_a_value_object constant_of_a_constrained_type`
Expected: FAIL — the first on `inner field must be private`, the second on the
missing `new_unchecked` call.

- [x] **Step 3: Write the implementation**

In `emit_type_def`, replace the body so the field is private and the impl block
is emitted. `newtype_inner(td)` already yields the backing type. The block below
is `emit_type_def` as it stands on `main` after driftsys/ridl#420, quoted whole
so the prelude spellings and the `#[allow(deprecated)]` placement can be read
off it directly. Task 4 adds the `constraint_is_vacuous` branch to the top of
this function; it is not here because it had not landed when this was written.

```rust
/// A named scalar becomes a `#[repr(transparent)]` newtype with a private
/// inner value (typl §5.7). Construction goes through `new`, which enforces
/// the typl constraints, or `new_unchecked`, which does not.
///
/// `Violation` and `Rule` are named by absolute path and nothing is imported:
/// a typl package may declare a type named `Violation` or `Rule`, and a `use`
/// of either would collide with that declaration. The leading `::` covers a
/// package that declares a type named `ridl_rt`. The prelude names the
/// constructors use — `Result`, `Ok`, `Err`, `TryFrom`, `From` — are absolute
/// for the same reason: a type name is CamelCase (typl §15.1) and `ridl-sem`
/// reserves no identifier, so a package may declare `type Result`, and that
/// struct would shadow the prelude's in the module the constructors share
/// with it.
///
/// A deprecated declaration's impl blocks carry `#[allow(deprecated)]`, with
/// one exception: the `Default` impl `defaults::decl_default_expr` emits
/// carries no allow, because `emit_decl` attaches it outside this function.
/// Each covered impl block uses the deprecated type, and without the allow
/// the consumer's build draws the `deprecated` lint on code the consumer did
/// not write.
fn emit_type_def(decl: &v2::Decl, td: &v2::TypeDef) -> TokenStream {
    let name = ident(&decl.name);
    let inner = newtype_inner(td);
    let doc = doc_attrs(&decl.doc);
    let unchecked = unchecked_doc(td);
    // A blank doc line keeps the unchecked note out of the declaration's own
    // doc paragraph.
    let separator = if decl.doc.is_empty() || unchecked.is_empty() {
        quote! {}
    } else {
        quote! { #[doc = ""] }
    };
    let deprecated = deprecated_attr(decl.deprecated.as_deref());
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };
    let vis = vis_tokens(decl.visibility);
    let type_name = decl.name.as_str();

    let checks = constraint_checks(td, type_name, quote! { value });
    let getter = scalar_getter(td, vis.clone(), inner.clone());

    quote! {
        #doc
        #separator
        #unchecked
        #deprecated
        #[repr(transparent)]
        #vis struct #name(#inner);

        #allow_deprecated
        impl #name {
            /// Constructs the value, enforcing its typl constraints.
            #vis fn new(
                value: #inner,
            ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
                #checks
                ::core::result::Result::Ok(Self(value))
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

        #allow_deprecated
        impl ::core::convert::TryFrom<#inner> for #name {
            type Error = ::ridl_rt::payload::Violation;
            fn try_from(value: #inner) -> ::core::result::Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for #inner {
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
/// The range and length checks for one constraint, as statements that return
/// early with a `Violation`. Only the branches the constraint carries are
/// emitted, so a string with a length bound and no range gets only the length
/// check. The pattern check is not emitted here.
///
/// A `min` or `max` is a numeric bound (typl §5.5), so a range check is
/// emitted only for a float or integer backing; on any other backing the two
/// are ignored rather than rendered as a literal of the wrong type.
fn constraint_checks(td: &v2::TypeDef, type_name: &str, value: TokenStream) -> TokenStream {
    let Some(c) = td.constraint.as_ref() else {
        return quote! {};
    };
    let mut checks = Vec::new();

    let is_float = match backing_scalar(td) {
        ScalarBacking::Float => Some(true),
        ScalarBacking::Integer => Some(false),
        ScalarBacking::Boolean | ScalarBacking::String | ScalarBacking::Bytes => None,
    };
    if let Some(is_float) = is_float {
        if let Some(min) = c.min.as_deref() {
            let lit = numeric_tokens(min, is_float);
            checks.push(quote! {
                if #value < #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Range,
                    });
                }
            });
        }
        // The newtype backing an integer is always `i64` (`newtype_inner`), so
        // a declared maximum at `i64::MAX` (9223372036854775807) makes
        // `value > 9223372036854775807` never true: rustc draws its
        // `unused_comparisons` warning on it in the consumer's build. The
        // branch is emitted only when the maximum is below the inner type's
        // maximum.
        let checked_max = c
            .max
            .as_deref()
            .filter(|max| is_float || max.parse::<i64>() != Ok(i64::MAX));
        if let Some(max) = checked_max {
            let lit = numeric_tokens(max, is_float);
            checks.push(quote! {
                if #value > #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Range,
                    });
                }
            });
        }
    }
    // Length is in characters for string (typl §5.3) and bytes for bytes
    // (§5.4), which is why the two use different expressions. The cast is
    // parenthesized because `as u64 < 8` does not parse: after a cast type,
    // `<` opens a generic-argument list.
    if c.len_min.is_some() || c.len_max.is_some() {
        let len = match backing_scalar(td) {
            ScalarBacking::String => quote! { (#value.chars().count() as u64) },
            _ => quote! { (#value.len() as u64) },
        };
        // A minimum of 0 is the default length bound of string and bytes
        // (typl §4.4, §4.5), and `(… as u64) < 0` is never true: rustc draws
        // its `unused_comparisons` warning on it in the consumer's build. The
        // branch is emitted only for a positive minimum.
        if let Some(min) = c.len_min.filter(|min| *min > 0) {
            let lit = proc_macro2::Literal::u64_unsuffixed(min);
            checks.push(quote! {
                if #len < #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Length,
                    });
                }
            });
        }
        if let Some(max) = c.len_max {
            let lit = proc_macro2::Literal::u64_unsuffixed(max);
            checks.push(quote! {
                if #len > #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Length,
                    });
                }
            });
        }
    }
    quote! { #(#checks)* }
}

/// The accessor. A `Copy` backing returns by value from a `const fn`; `String`
/// and `Vec<u8>` borrow, and gain `into_inner` for the owned form.
///
/// `backing_scalar` is total: it maps a unit backing and an absent backing to
/// `Float`, so every named scalar gets exactly one of the three forms.
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

Add `unchecked_doc` and call it from `emit_type_def`, so a named scalar whose
constraint carries a `step`, or a `match` pattern the constructor does not yet
check, gains a doc line naming what `new` does not check, rather than staying
silent (spec, "Not validated"):

```rust
/// The gaps a generated constructor does not close, named on the type itself
/// rather than left silent: a `step` is not checked by `new`, and neither is
/// a `match` pattern until the pattern check lands. `pattern_const` is read as
/// well as `pattern`, because a pattern constant that did not resolve leaves
/// `pattern` absent while the type still carries a match constraint.
fn unchecked_doc(td: &v2::TypeDef) -> TokenStream {
    let Some(c) = td.constraint.as_ref() else {
        return quote! {};
    };
    let mut lines = Vec::new();
    if c.step.is_some() {
        lines.push(" Quantization (`step`) is not checked by `new`.");
    }
    if c.pattern.is_some() || c.pattern_const.is_some() {
        lines.push(" The `match` pattern is not checked by `new`.");
    }
    quote! { #(#[doc = #lines])* }
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

- [x] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked && cargo test -p ridl --locked`
Expected: PASS, including the `rustc` compile proofs in `tests.rs`. Those are
the real check that the emitted code is valid Rust. Three of them bear on this
task directly — `appendix_b_compiles_with_rustc`,
`constructible_collections_compile`, and
`a_tuple_under_an_internal_declaration_is_package_private`. None of those three
fails on a warning: the first two pass no `-D` flag at all and assert only
`status.success()`, and the third denies two lints by name. So a naming lint
does not fail any of them; Task 11 is where that matters.

driftsys/ridl#420 added a fourth,
`prelude_names_declared_by_the_package_compile`, which compiles a package
declaring `Result`, `Ok`, `Err`, `From` and `TryFrom` as its own types; it is
what pins correction 1. `tests.rs` carries two more,
`the_compile_proof_harness_links_ridl_rt` and `appendix_a_compiles_with_rustc`,
so the file now holds six in total and the count in this step is no longer
three.

- [x] **Step 5: Commit**

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

That commit is the first of the five on driftsys/ridl#420. The other four came
from review of that pull request and carry the two corrections recorded at the
top of this task, the tests that pin them, and the records those corrections
made stale.

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

**The prelude is named absolutely here too.** Task 3's correction 1 binds this
task: `From` is emitted as `::core::convert::From`, and the assertion that no
manual `TryFrom` appears is written against `::core::convert::TryFrom` as well
as the unqualified spelling, so it cannot pass by matching the wrong text.

**A deprecated declaration's impl blocks carry `#[allow(deprecated)]`.** This is
the third thing driftsys/ridl#420 settled and this task inherits: each emitted
impl block names the deprecated type, and without the allow the consumer's build
draws the `deprecated` lint on code the consumer did not write. `emit_type_def`
binds it as

```rust
let allow_deprecated = if decl.deprecated.is_some() {
    quote! { #[allow(deprecated)] }
} else {
    quote! {}
};
```

and prefixes each impl block with `#allow_deprecated`. Do the same here.

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
    assert!(source.contains("impl ::core::convert::From<bool> for Enabled"));
    assert!(source.contains("impl ::core::convert::From<Enabled> for bool"));
    // No escape hatch is emitted: `new` already is one.
    assert!(
        !source.contains("Enabled::new_unchecked") && !source.contains("fn new_unchecked(value: bool)"),
        "new_unchecked would duplicate new on a vacuous type"
    );
    // And no manual TryFrom, which would collide with core's blanket impl.
    assert!(
        !source.contains("impl ::core::convert::TryFrom<bool> for Enabled")
            && !source.contains("impl TryFrom<bool> for Enabled")
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
`cargo test -p ridl-backend-rust --locked vacuous_scalar_constructs_infallibly`
Expected: FAIL — `emit_vacuous_type_def` does not exist yet, so `Enabled` still
takes the constrained path.

- [ ] **Step 3: Write the implementation**

Add the branch at the top of `emit_type_def`, before it computes the checks and
the getter. Task 3's quoted copy of that function does not show it, because it
had not landed when Task 3 was executed:

```rust
if ridl_ir::v2::constraint_is_vacuous(td.constraint.as_ref()) {
    return emit_vacuous_type_def(decl, td);
}
```

Then add the function itself:

```rust
/// A named scalar whose constraint checks nothing: `boolean`, and `integer` or
/// `float` with no declared range.
///
/// Construction is infallible, so `From<Inner>` is correct here — there is no
/// invariant for it to bypass. Core's blanket `impl<T, U: Into<T>> TryFrom<U>
/// for T` then supplies `TryFrom<Inner>` with `Error = Infallible`, so generic
/// consumer code calling `try_from` compiles against both kinds of scalar.
/// `new_unchecked` is deliberately absent: `new` already is the unchecked path.
///
/// `From` is named by absolute path for the reason Task 3 records: a typl
/// package may declare `type From`, and that declaration shadows the prelude
/// in the module the generated impl shares with it.
fn emit_vacuous_type_def(decl: &v2::Decl, td: &v2::TypeDef) -> TokenStream {
    let name = ident(&decl.name);
    let inner = newtype_inner(td);
    let attrs = decl_attrs(decl);
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };
    let vis = vis_tokens(decl.visibility);
    let getter = scalar_getter(td, vis.clone(), inner.clone());

    // A `String`/`Vec<u8>` backing cannot appear here: the checker always
    // materializes the typl §4.4 default `[0..256]`, so both are non-vacuous.
    quote! {
        #attrs
        #[repr(transparent)]
        #vis struct #name(#inner);

        #allow_deprecated
        impl #name {
            /// Constructs the value. This type declares no constraint, so
            /// construction cannot fail.
            #vis const fn new(value: #inner) -> Self { Self(value) }
            #getter
        }

        #allow_deprecated
        impl ::core::convert::From<#inner> for #name {
            fn from(value: #inner) -> Self { Self(value) }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for #inner {
            fn from(value: #name) -> Self { value.0 }
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS. The
`named_scalar_backings` snapshot (`tests.rs:172`) then shows `Counter`,
`Enabled`, `Label` and `Blob` on the vacuous path and only `Speed` on the
constrained path. All four are built with the `primitive_type` helper, which
sets `constraint: None`, and `constraint_is_vacuous(None)` is `true`. That is a
property of this hand-built fixture rather than of the language: the checker
materializes the typl §4.4 default length bound, so a string or bytes type
reaching the backend through the compiler is never vacuous.

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

**Landed** in driftsys/ridl#433. Review corrected the mask fold, which panicked
in a debug build on a bit position `ridl-sem` reports TYPL-111 for and still
carries into the IR, and merged the enum set's two impl blocks so
`#[allow(deprecated)]` covers the bit constants. Both corrections and their
reasons are in the pull request.

**The blocks below are a summary of what landed, not a transcript of it.** They
were brought in line with the shipped behaviour so that replaying this task does
not reintroduce either defect, but they are shorter than the source: the
comments are paraphrased, and `crates/ridl-backend-rust/src/lib.rs` carries
reasoning that has no counterpart here — the whole `ridl-diff` paragraph on
whether an enum set is closed or open on the wire, and the impl-merge rationale,
which is a rustdoc comment on `emit_enum_set` rather than a comment inside
`quote!`. Step 1 below is likewise the test as first written; the landed test
also asserts the arm mapping, `7 => ::core::result::Result::Ok(Self::REVERSE)`,
which is what makes the discriminant gap in the fixture mean anything. **Read
the source, not this, before changing any of it.**

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_enum`, `emit_enum_set`
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: nothing from Task 2 — there is no import to consume, so the emitted
  code names `::ridl_rt::payload::Violation` and `::ridl_rt::payload::Rule` by
  absolute path. A compile proof over this output passes
  `--extern ridl_rt=<path>`, built by Task 2's `ridl_rt_rlib` helper.
- Produces: `impl ::core::convert::TryFrom<i64> for <Enum>`,
  `impl ::core::convert::From<<Enum>> for i64`, and the same pair for each enum
  set.

**The prelude is named absolutely here too.** Task 3's correction 1 binds this
task: `TryFrom`, `From`, `Result`, `Ok` and `Err` are emitted as
`::core::convert::TryFrom`, `::core::convert::From`, `::core::result::Result`,
`::core::result::Result::Ok` and `::core::result::Result::Err`, because a typl
package may declare a type of any of those names in the module these impl blocks
share with it.

**A deprecated declaration's impl blocks carry `#[allow(deprecated)]`.** This is
the third thing driftsys/ridl#420 settled and this task inherits: each emitted
impl block names the deprecated type, and without the allow the consumer's build
draws the `deprecated` lint on code the consumer did not write. `emit_type_def`
binds it as

```rust
let allow_deprecated = if decl.deprecated.is_some() {
    quote! { #[allow(deprecated)] }
} else {
    quote! {}
};
```

and prefixes each impl block with `#allow_deprecated`. Do the same here.

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn enum_converts_from_a_raw_discriminant() {
    let source = rust_for(vec![gear_position_decl()]);
    assert!(source.contains("impl ::core::convert::TryFrom<i64> for GearPosition"));
    assert!(source.contains("impl ::core::convert::From<GearPosition> for i64"));
    assert!(source.contains("::ridl_rt::payload::Rule::Variant"));
}

#[test]
fn enum_set_rejects_bits_outside_the_declared_mask() {
    let source = rust_for(vec![features_decl()]);
    assert!(source.contains("impl ::core::convert::TryFrom<i64> for Features"));
    // The declared bits are 0 to 3, so the mask is 0b1111. Assert the value:
    // a fold that ORed the bit positions instead of shifting by them would
    // still emit a `DECLARED_MASK` and still pass a presence check.
    assert!(source.contains("const DECLARED_MASK: i64 = 15"));
}
```

If `gear_position_decl` and `features_decl` do not already exist in `tests.rs`,
build them with the existing `public_decl` helper and `v2::decl::Kind::EnumDef`
/ `EnumSetDef`, mirroring the fixtures the `enum` and `enumset` snapshot tests
already use.

- [x] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-backend-rust --locked enum_converts_from_a_raw enum_set_rejects_bits`
Expected: FAIL — no `TryFrom` impl is emitted for either kind.

- [x] **Step 3: Write the implementation**

Append to `emit_enum`'s returned stream:

```rust
    // A raw discriminant off the wire is where an out-of-contract value
    // actually enters a program: a wire backend emits no constructor
    // (ADR-0013 decision 2), so this is the validating seam.
    let arms = ed.values.iter().map(|value| {
        let vname = ident(&value.name);
        let disc = int_tokens(value.value);
        quote! { #disc => ::core::result::Result::Ok(Self::#vname) }
    });
    let type_name = decl.name.as_str();
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };

    quote! {
        #allow_deprecated
        impl ::core::convert::TryFrom<i64> for #name {
            type Error = ::ridl_rt::payload::Violation;
            fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
                match value {
                    #(#arms,)*
                    _ => ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Variant,
                    }),
                }
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for i64 {
            fn from(value: #name) -> Self { value as i64 }
        }
    }
```

**Replace** `emit_enum_set`'s inherent impl block with the following, and append
the two trait impls. This is a replacement, not an addition: the function
already emits an `impl #name` holding the bit constants, and appending a second
block that defines them again is a duplicate definition (E0201).

```rust
    // A bit outside the int64 domain contributes nothing. `ridl-sem` reports
    // TYPL-111 for a position outside 0..=63 and still carries the bit into
    // the IR, so this fold can be handed one, and `1i64 << 64` panics in a
    // debug build. Codegen is total (Global Constraints, above).
    let mask = esd
        .bits
        .iter()
        .filter(|bit| (0..=63).contains(&bit.value))
        .fold(0i64, |acc, bit| acc | (1i64 << bit.value));
    let mask_lit = int_tokens(mask);
    let type_name = decl.name.as_str();
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };

    quote! {
        // The bit constants, `DECLARED_MASK` and `get` share one inherent
        // impl block, so `#[allow(deprecated)]` covers all three. Split
        // across two blocks with the allow on one, a deprecated enum set's
        // own constants draw the `deprecated` lint in the consumer's build.
        #allow_deprecated
        impl #name {
            #(#bits)*

            /// The union of every declared bit. `TryFrom` refuses a value
            /// that carries any other bit.
            #vis const DECLARED_MASK: i64 = #mask_lit;

            #vis const fn get(self) -> i64 { self.0 }
        }

        #allow_deprecated
        impl ::core::convert::TryFrom<i64> for #name {
            type Error = ::ridl_rt::payload::Violation;
            fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
                if value & !Self::DECLARED_MASK != 0 {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Variant,
                    });
                }
                ::core::result::Result::Ok(Self(value))
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for i64 {
            fn from(value: #name) -> Self { value.0 }
        }
    }
```

Note the enum set's inner field is already emitted as `#vis i64` —
`#vis struct #name(#vis i64);` in `emit_enum_set` — change it to a private `i64`
for consistency with Task 3. `get` joins the impl block above.

**`get` takes `self`, and the enum set has no `Copy` until Task 6.** Between
this task and Task 6 an enum set held in a struct field cannot be read through a
shared reference: `get` and `From<EnumSet> for i64` both move, and rustc reports
E0507. This is the state Task 3 already left every `Copy`-backed named scalar
in, for the same reason and with the same fix — Task 6 derives `Copy`, whose
leaf rule (`f64`, `i64` or `bool`) an enum set satisfies. **Task 6 closes this;
confirm it covers the enum set and not only the named scalar.**

- [x] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS, including the
`rustc` compile proofs.

- [x] **Step 5: Commit**

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
- Produces: `<out_dir>/Cargo.toml` and `<out_dir>/lib.rs` when `Emit::Rust` is
  selected in package or workspace mode and the build has something to produce.
  Two cases produce neither, and each is a build that writes nothing at all
  rather than a partial crate: a build that draws an error-severity diagnostic,
  and a build whose output directory already holds a `lib.rs` or a `Cargo.toml`
  that ridlc did not write.

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
name `payload::Violation` from it (Task 2). The requirement is written as the
literal `ridl-rt = "0.1"`, not read from `crates/ridl-rt/Cargo.toml` at emit
time: `ridlc` is an installed binary with no access to this repository's sources
when it runs, and it has no build script. A guard test reads that manifest and
asserts the emitted requirement still matches its major and minor, so the two
cannot drift apart silently, which is what reading it was for. This plan first
said to read it; the reason it cannot be read was found while executing this
task.

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
                    // An inline (non-leaf) module's own children would
                    // otherwise search a subdirectory named after every
                    // enclosing module (rustc's default module-path
                    // resolution for a module with no `#[path]`); anchoring
                    // this module at `.` keeps its children's own `#[path]`
                    // attributes resolving against the flat output directory.
                    out.push_str(&format!("{pad}#[path = \".\"]\n"));
                    out.push_str(&format!("{pad}pub mod {segment} {{\n"));
                    if let Some(file) = &child.file {
                        // `segment` is itself a package as well as a
                        // namespace for its children (`veh` alongside
                        // `veh.common`) — the `_` arm above would otherwise
                        // drop `segment`'s own file. It cannot simply be
                        // nested under its own name: generated code names a
                        // type in package `veh` as `crate::veh::Speed`, not
                        // `crate::veh::veh::Speed`, so the file is loaded as
                        // a private module and re-exported, which puts its
                        // items at the path the references use.
                        //
                        // The private module's name is the one place this
                        // shape is not total over names: a package `veh`
                        // declaring a type called `common` alongside a
                        // package `veh.common` would have that type shadowed
                        // by the module (issue #416; rustc reports E0573,
                        // so it fails loudly). `__ridl_package` cannot be a
                        // typl package segment, so the private module itself
                        // collides with nothing.
                        out.push_str(&format!("{pad}    #[path = \"{file}\"]\n"));
                        out.push_str(&format!("{pad}    mod __ridl_package;\n"));
                        out.push_str(&format!("{pad}    pub use __ridl_package::*;\n"));
                    }
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

**Two corrections this plan first got wrong**, both found by the `rustc` proof
over the emitted crate root and both carried in the code above.

1. **A non-leaf module needs `#[path = "."]`.** Without it, rustc resolves an
   inline module's un-annotated children against a subdirectory named after
   every enclosing module, so no dotted package resolved at all — not only the
   prefix case.
2. **A package name that is a strict prefix of another cannot be nested under
   its own name.** `veh` alongside `veh.common` lands in the `_` arm, and this
   plan first said to emit `veh`'s file as an inner `pub mod veh`. That leaves
   its items at `crate::veh::veh`, while generated code names them
   `crate::veh::Speed`, so every reference to the prefix package fails to
   resolve. The file is loaded as a private `__ridl_package` module and
   re-exported instead. `ridl.toml` naming makes the case unlikely but not
   impossible.

The one place this shape is not total over names: a package `veh` declaring a
type called `common`, alongside a package `veh.common`, has that type shadowed
by the module, and a reference to it fails with rustc's E0573 rather than
silently resolving to something else. That is recorded as driftsys/ridl#416,
which names the three ways it could be closed; it was not treated as blocking
because it fails loudly. The private module's own name cannot collide, because
`__ridl_package` is not a possible typl package segment.

The crate name comes from the manifest's package name when one is present, with
every `.` replaced by `_`, because a ridl package name is dotted
(`rsdl.showcase`) and a dot is not legal in a Cargo package name — this plan did
not say so and the first implementation of it emitted an invalid manifest. A
`[workspace]` manifest names no package, so it falls back to `ridl_generated`.
See Open item 1 — if a `--crate-name` flag is added, it takes precedence over
both.

`ridl.std` is a package in this tree too. `run_build` writes `ridl.std.rs`
whenever the workspace references it (issue #190), and generated code names
those types `crate::ridl::std::…`, so the name list is the checked packages plus
`ridl.std` when that file was written. This plan did not say so either; the
crate does not compile without it.

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

**Every path in the emitted block is absolute**, including `::std::sync` and
`::regex::Regex`. Task 3's correction 1 covers the prelude; this task has to
settle the two crate paths as well, and the answer is the same for the same
reason. The generated code for one package is one module, and `face.rs` emits
one `pub mod` per named interface, spelled `snake_case(<interface name>)` — so
an interface named `Std` puts a module named `std` in the module the
constructors live in, and an interface named `Regex` puts one named `regex`
there. A module item shadows the extern prelude for code in the same module, so
the unqualified `std::sync::LazyLock` then resolves to that interface's face
module and rustc reports `E0433`. A leading `::` is the extern crate
unconditionally, so it cannot be shadowed by any declaration. Confirmed with
rustc on 2026-09-20: a module holding both `pub mod std` and a
`std::sync::LazyLock` static fails to resolve, and the same module with
`::std::sync::LazyLock` compiles.

The crate-root package module tree is not the reason. `ridl.std` becomes
`pub mod ridl { pub mod std { … } }`, and a package that is also a namespace has
its own code loaded into a private `__ridl_package` module
(`ridlc::render_lib_rs`), so no generated package module ever has a sibling
named `std` from the tree alone. The face module is what puts one there.

Today that face module reaches no output of `ridl build`: `ridlc` calls
`generate`, and the face is emitted only by the companion entry point
`generate_face` (ADR-0023), whose callers are the backend's own tests and the
checked-in fixture
`crates/ridl-backend-rust/tests/generated/interaction_face.rs`. That fixture
does hold face modules and named-scalar constructors in one module, and ADR-0023
makes `generate_face` the entry point every later interaction-face story
extends. So the absolute paths are taken as a totality argument about a shipped
entry point, not as a repair of a defect a `ridl build` can produce today.

**Interfaces:**

- Consumes: `constraint_checks` (Task 3), the manifest feature (Task 7).
- Produces: nothing new; extends the generated `new`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pattern_check_is_feature_gated() {
    let source = rust_for(vec![vin_decl()]);
    assert!(source.contains("#[cfg(feature = \"validate-pattern\")]"));
    assert!(source.contains("::ridl_rt::payload::Rule::Pattern"));
    // Both crate paths are absolute, so an interface named `Std` or `Regex`
    // cannot shadow them from the module the constructor lives in.
    assert!(source.contains("::std::sync::LazyLock"));
    assert!(source.contains("::regex::Regex::new"));
    // The length check is not gated - it needs no dependency.
    let gated = source.split("#[cfg(feature = \"validate-pattern\")]").next().unwrap();
    assert!(gated.contains("::ridl_rt::payload::Rule::Length"));
}
```

`vin_decl` is a `string` type carrying `len_min == len_max == 17` and a
`pattern`; build it with the existing `primitive_type` helper. Its `len_min` has
to stay positive for the last assertion to mean anything: a `len_min` of 0 emits
no branch at all (Task 3, correction 2), so a fixture defaulting to 0 would
leave that assertion resting on the `len_max` branch alone.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridl-backend-rust --locked pattern_check_is_feature_gated`
Expected: FAIL — no `cfg` attribute is emitted.

- [ ] **Step 3: Write the implementation**

Append to `constraint_checks`, after the length checks:

```rust
if let Some(pattern) = c.pattern.as_deref() {
    // The pattern needs a regex engine, which `core` has none of. The
    // range and length checks above are not gated; only this one is, so a
    // `--no-default-features` build still validates the bounds it emits.
    //
    // `::std` and `::regex` are absolute for the reason the prelude names
    // are: the face module of an interface named `Std` or `Regex` is a
    // module of that name in this same module, and it would shadow the
    // extern crate.
    let source = strip_regex_delimiters(pattern);
    checks.push(quote! {
        #[cfg(feature = "validate-pattern")]
        {
            static PATTERN: ::std::sync::LazyLock<::regex::Regex> =
                ::std::sync::LazyLock::new(|| {
                    ::regex::Regex::new(#source).expect("ridlc emitted an invalid pattern")
                });
            if !PATTERN.is_match(&#value) {
                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: #type_name,
                    rule: ::ridl_rt::payload::Rule::Pattern,
                });
            }
        }
    });
}
```

Emit a doc line on the type naming that the pattern is enforced only under the
feature, so the guarantee is not silently variable. That line replaces one
rather than joining it: `unchecked_doc` (Task 3) emits " The `match` pattern is
not checked by `new`." whenever the constraint carries a `pattern` or a
`pattern_const`, and the `pattern` half of that becomes untrue here. An
unresolved `pattern_const` keeps the existing line, because no check is emitted
for it.

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

The range and length checks are not gated, so a --no-default-features
build still validates the bounds it emits. Only the pattern check needs a
regex engine, which core does not have."
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

1. **The crate name for the generated `Cargo.toml` — decided 2026-09-17, not
   open.** Task 7 implemented it: the crate name is the `ridl.toml` package name
   with every `.` replaced by `_` (`veh.common` becomes `veh_common`), because a
   dotted name is not a legal Cargo package name; a `[workspace]` manifest names
   no package, so it falls back to `ridl_generated`. The `--crate-name` flag was
   not implemented — nothing needs it yet, and a flag is additive, so it can be
   added later without breaking the default. Kept here as a numbered item
   because the proposal was recorded here and a reader of this list would
   otherwise take it as still open.
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
