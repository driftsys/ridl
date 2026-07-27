# Design — emit `ridl.std` from `ridlc build`

- Date: 2026-07-27
- Status: design approved, not yet implemented
- Closes: driftsys/ridl#190

## 1. The question

Both backends generate references to `ridl.std`, and no command writes the
package. The raw output of `ridlc build` therefore does not compile. Running the
shipped corpus through the shipped binary:

```
ridlc build crates/ridlc/tests/corpus/veh-cluster --out-dir out --emit typescript,rust
```

writes `veh.common.rs`, `veh.cluster.rs`, `veh.common.ts`, `veh.cluster.ts` and
no `ridl.std` artifact of either kind. `tsc` fails with TS2307 on both modules;
`rustc` fails with eight occurrences of `E0433: cannot find ridl in crate`.

The gap is symmetric across the two backends, pre-existing since E1 on the Rust
side and E2 on the TypeScript side, and no shipped document mentions it.

## 2. Decision

`ridlc build` emits the `ridl.std` artifact when the workspace references it.
The artifact is generated from the embedded standard package, named like every
other package, and the corpus proofs stop supplying hand-written stand-ins.

A second, smaller decision rides along: the embedded asset gains a test pinning
it to Appendix A, closing an unguarded duplication in the same area.

## 3. Rationale

### 3.1 `ridl.std` is already an ordinary package to both backends

The issue asked what the Rust artifact should be, and worried that a sibling
`ridl.std.rs` would not satisfy the `crate::ridl::std::…` path the backend emits
— possibly forcing a change to the reference form.

It does not. Both backends already treat `ridl.std` exactly as they treat any
other cross-package reference:

| Backend    | Standard package                         | Ordinary package                             |
| ---------- | ---------------------------------------- | -------------------------------------------- |
| Rust       | `crate::ridl::std::Timestamp`            | `crate::veh::common::Speed`                  |
| TypeScript | `import * as ridl_std from './ridl.std'` | `import * as veh_common from './veh.common'` |

Both forms above were read from a real `ridlc build` run, not from the source.
`type_path` maps any dotted reference to a `crate::`-anchored module path, and
the TypeScript `Ctx::type_ref` registers a namespace import for any dotted
reference. Neither special-cases the standard package.

A consumer therefore already has to place `veh.common.rs` at
`crate::veh::common` to compile the generated Rust — the `crate::` anchor is
deliberate, so that several generated packages compose as sibling modules rooted
at the crate. Doing the same for `ridl.std.rs` is the identical act, not a new
burden. The reference form is uniform and correct as it stands, and this design
does not change it.

The two backends are already symmetric here, so emitting one artifact per
selected emit kind keeps them symmetric at no cost.

### 3.2 Why only when referenced

The alternative — emitting the standard package on every build — is simpler and
cannot under-report. It was rejected because it writes a file into `--out-dir`
that the user did not ask for, in every build, including workspaces that never
name a standard type.

The cost of the choice is stated plainly, because it is real: the failure mode
of under-reporting is exactly the defect this design closes. Section 7 records
it as the primary risk and §3.4 as the mitigation.

### 3.3 Why detection lives in `ridl-ir`

Detection is a new `referenced_packages(&v2::Package) -> BTreeSet<String>` in
`ridl-ir`.

It belongs in that crate because it depends on an invariant that crate defines.
`ir.proto` states the canonical form directly: every resolved type-reference
string is the fully qualified `pkg.Name` for a cross-package reference and the
bare `Name` for a same-package reference, never an import alias — and it
enumerates the fields that carry one (`FieldType.named`, `UnionArm.type_ref`,
`ConstDef.type_ref`, `EnumSetDef.backing_enum`, `Constraint.pattern_const`,
`SignalDef.payload`, `EventDef.payload`, `StreamType.named`, `FallibleType.ok`,
`FallibleType.err`, `Service.interface_ref`).

That enumeration names the fields, not the shape. A reference is reachable at
arbitrary depth, because `FieldType` nests into itself through
`TupleType.fields`, `ArrayType.element`, `MapType.key`, `MapType.value`,
`FieldType.inline_scalar` and `FieldType.stream`. The walk is therefore
recursive over the type tree, not a read of eleven flat fields.

**The walk matches every `oneof` exhaustively, with no wildcard arm.** `prost`
lowers each `oneof` to a Rust enum, so a variant added later — E3 will add
several — stops the walk compiling instead of silently going unread. This is the
mechanism that makes the detection rule fail closed, and it is a requirement of
this design, not an implementation preference. A wildcard arm added later would
disarm it silently.

Placing the walk anywhere else separates it from the rule that makes it correct.
Placing it beside the schema means the enumeration and its consumer are edited
together.

The rejected alternative was to have each backend report the packages it
referenced during generation. That is exact by construction — the TypeScript
backend already accumulates the set in `Ctx::imports`. It was rejected because
the Rust backend has no equivalent: `type_path` is a free function called from
ten sites across four files with no context in scope, so giving it a collector
means threading state through all of them. The cost is disproportionate to a
detection rule that the IR already supports directly.

### 3.4 Why the stand-ins must go

The corpus proofs pass today only because each supplies the missing module
before it runs: the rustc proof prepends a hand-written `PRELUDE` declaring
`pub mod ridl { pub mod std { … } }`, and the tsc proof copies in
`crates/ridlc/tests/tsc/ridl.std.ts`, a file whose own header says it is not a
shipped artifact.

Each is defensible as a test of a backend, where the question is whether the
generated code is well-formed. Neither is a test of what `ridlc build` hands a
user, and nothing else tested that — which is why the defect shipped.

Removing them is what makes the emitted **content** answerable: each proof now
compiles the standard package the compiler generates from the embedded asset, so
a package the corpus references but the asset cannot express fails the proof
instead of being papered over by a hand-maintained module beside it.

The emit **decision** is a separate question, and the proofs do not answer it.
They call `check_package` and the backends directly and prepend the standard
package unconditionally; none of them calls `referenced_packages` or
`run_build`, so a detection rule that never fired would leave every one of them
green.

That decision has its own guard, in the same file:
`build_writes_the_standard_package_exactly_when_the_generated_rust_names_it`
drives the real `run_build` into a temporary directory for every corpus entry
that compiles, and asserts one biconditional — some emitted `.rs` names a
`crate::ridl::std::` path **exactly when** `ridl.std.rs` was written. One
equality covers both failure directions, and the test also asserts that the
corpus holds an entry on each side, so neither direction can go vacuous. Today
two entries are positive (`veh-common`, `veh-cluster`) and two negative
(`workspace-two-members`, `services-workspace`).

### 3.5 Why the drift guard rides along

`crates/ridl-core/src/std_lib.rs` describes the asset as committed verbatim from
Appendix A, and Appendix A is normative. Nothing enforces the claim. The two
were edited by hand in #198 and every gate would have passed had only one been
edited.

The guard is a test asserting the asset is byte-identical to Appendix A's
`ridl.std` fenced block. It is in scope here because this design is what makes
the asset's content reach users as a shipped artifact: an asset that has drifted
from the normative appendix now produces generated code that disagrees with the
specification, which is a worse failure than a stale document.

## 4. Scope

### 4.1 Detection

`ridl-ir` gains `referenced_packages`, returning the set of package names that
the package's type references name — the qualifier of every dotted reference,
and nothing for a bare one. Its doc comment cites the `ir.proto` canonical-form
paragraph as the list to update alongside it.

### 4.2 Emission

`run_build` already holds what it needs: `Compiled` carries `std: Package` and
`workspace`, so the standard package lowers with the same
`check_package(&*db, workspace, std, std).ir` used by `compile_workspace`. When
any checked package's referenced set contains `ridl.std`, one further
`write_emits` call runs for it.

**The emit kinds are the caller's, minus `ir-json`.** The three code targets —
`rust`, `c-header`, `typescript` — follow the selection with no extra logic, so
`--emit rust` alone writes only `ridl.std.rs`. `ir-json` is excluded, and the
reason is that a `.ir.json` is a **contract snapshot**, not code.
`ridl
baseline` is `run_build(path, &staging, &[Emit::IrJson], …)`, and
`ridl diff` compiles its current side through `compile_workspace().checked`,
which deliberately excludes `ridl.std` because the standard package is not a
workspace member. Writing `ridl.std.ir.json` into a published baseline therefore
leaves it with no counterpart on the compiled side, and every diff of an
**unedited** workspace against its own baseline reports `decl_removed ridl.std`
and exits 1. A baseline holds the packages the workspace _declares_; `ridl.std`
is version-locked to the compiler binary (ADR-0007 decision 15) and is not one
of them. Issue #190 is about generated _code_ failing to compile, and a
`.ir.json` is not code.

### 4.3 Artifacts

`ridl.std.rs`, `ridl.std.h` and `ridl.std.ts`, named exactly as every other
package is. No `ridl.std.ir.json` (§4.2). No change to the reference form, to
the emitted content of any existing artifact, or to file naming.

### 4.4 Proofs

The rustc corpus proof drops `PRELUDE` and compiles the standard package's own
generated Rust alongside the package sources. The tsc corpus proof drops the
copy of `crates/ridlc/tests/tsc/ridl.std.ts` and resolves against the standard
package's own generated TypeScript. The stand-in file is deleted. Both proofs
generate the standard package in-process, so what they establish is that its
content is sufficient — the emit decision is guarded separately, below.

Three tests cover the emit decision:

- `crates/ridlc/tests/cli.rs` — the positive case (a workspace naming a standard
  type gets `ridl.std.rs` and `ridl.std.ts`) and the negative case (a workspace
  naming none gets neither).
- `crates/ridlc/tests/corpus.rs` — the biconditional over every compilable
  corpus entry, driven through the real `run_build` (§3.4).
- `crates/ridl/tests/baseline_desk.rs` — `ridl baseline` followed by
  `ridl
  diff` over a fixture that names a standard type reports `identical`
  and exits 0, and the published baseline holds no `ridl.std.ir.json` (§4.2).
  Every other baseline and diff fixture names only its own declarations, which
  is why none of them reached this path.

### 4.5 The drift guard

A test in `ridl-core` asserting `RIDL_STD_SOURCE` equals the `ridl.std` fenced
block of `docs/specification/typl-language-reference.md`, with a failure message
naming both files and saying they are edited together.

## 5. Non-goals

- **No change to the reference form.** `crate::ridl::std::…` and `./ridl.std`
  stay as they are (§3.1).
- **No change to how `ridl.std` ships.** ADR-0007 decision 15 stands: embedded
  via `include_str!`, no filesystem or network lookup, version-locked to the
  compiler binary.
- **No mapping of standard types onto host types.** Rust has no `Uri` in its
  standard library, so mapping means a third-party dependency, and the typl
  declarations carry units, ranges, and nominal identity that host types drop —
  `Duration : ms [0.0..…]` against `std::time::Duration`, or the string-declared
  `IpV4` against `Ipv4Addr`. Ranges drive wire width, so substituting a host
  type would change serialization while `ridl-diff` still compares the IR.
- **No hand-written standard library per backend.** The standard package is
  defined once in typl; generating it keeps the artifact derived from that
  definition, where hand-coding would add one more source of truth per backend
  on top of the two §3.5 already guards.

**Open question, recorded not decided.** The previous two non-goals are one
question: _should generated code depend on a ridl support library?_ Answering
yes buys idiomatic host types, mapping configuration, and hand-written
ergonomics such as `Display` and `FromStr`; answering no keeps generated code
standing alone, which is what this design delivers. It is an ecosystem decision
(Epic 4 territory), it needs a distribution story the project does not yet have
— nothing is published, and the book states there is no runtime — and it must
not be settled as a side effect of a defect fix.

## 6. Success criteria

1. A build of the `veh-cluster` corpus with `--emit typescript,rust` writes a
   `ridl.std` artifact of each kind, and the raw output compiles with nothing
   added: `tsc --strict` over the emitted `.ts` files exits 0, and `rustc` over
   the emitted `.rs` files, with no prepended prelude, exits 0.
2. `PRELUDE` and `crates/ridlc/tests/tsc/ridl.std.ts` no longer exist, and the
   corpus proofs pass against the generated standard package.
3. A workspace that references no standard type emits no `ridl.std` artifact.
4. Over every compilable corpus entry, `ridl.std.rs` is written exactly when the
   entry's own generated Rust names a `crate::ridl::std::` path (§3.4).
5. `ridl baseline` followed by `ridl diff` over an unedited workspace that names
   a standard type reports `identical` and exits 0 (§4.2).
6. The drift test fails when the asset and Appendix A disagree.
7. `just build` passes.

## 7. Risks

- **Detection under-reports.** A reference the walk does not read makes the emit
  silently stop happening for a workspace that reaches the standard package only
  that way. Three defences, in order of strength. A new `oneof` **variant**
  cannot slip through at all: the exhaustive match without a wildcard arm (§3.3)
  turns it into a compile error. A new **field on an existing message** — a
  second `type_ref` beside one already read — does compile, and is caught by the
  corpus biconditional (§3.4) for every construct the corpus exercises: the
  field carries a reference the backend still renders, so the generated Rust
  names `crate::ridl::std::` while no `ridl.std.rs` is written, and the two
  sides of the equality disagree. Residual: a new field, on an existing message,
  reached only by a construct the corpus does not exercise. The doc comment
  naming the `ir.proto` paragraph is what makes the pairing visible to whoever
  adds that field.
- **Detection over-reports.** If detection returned every package
  unconditionally the positive proofs would still pass. Two tests discriminate:
  §4.4's negative CLI case, and the corpus biconditional, which fails on the two
  entries that reference nothing standard. The biconditional asserts that the
  corpus holds an entry on each side, so it cannot go vacuous if the corpus
  changes shape.
- **Deleting the stand-ins removes a backend-level signal.** The stand-ins let
  the backend proofs fail for backend reasons alone. After this change a corpus
  failure could come from either the backend or the standard package's own
  generated code. Accepted: the backends keep their own unit tests, and the
  corpus proof is more valuable as a proof over the generated standard package.
- **The `ir-json` exclusion is a rule about one emit kind, held by one test.**
  Nothing in the type system stops a later change from passing the caller's full
  `emits` slice through again, which is exactly how the baseline/diff asymmetry
  arrived (§4.2). §4.4's baseline test is the guard, and the comment at the
  filter in `run_build` states the reason at the site.
