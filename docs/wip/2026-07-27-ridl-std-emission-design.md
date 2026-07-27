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
enumerates the eleven fields that carry one (`FieldType.named`,
`UnionArm.type_ref`, `ConstDef.type_ref`, `EnumSetDef.backing_enum`,
`Constraint.pattern_const`, `SignalDef.payload`, `EventDef.payload`,
`StreamType.named`, `FallibleType.ok`, `FallibleType.err`,
`Service.interface_ref`).

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

Removing them is what makes this design verifiable rather than merely correct.
Once the proofs compile what the command actually emits, they become end-to-end
proofs of the user's artifact, and a detection rule that under-reports fails
them instead of passing silently.

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
`write_emits(out_dir, "ridl.std", &std_ir, emits, &mut diagnostics)` call runs.

Passing the same `emits` slice means the artifact follows the selected kinds
with no extra logic: `--emit rust` alone writes only `ridl.std.rs`.

### 4.3 Artifacts

`ridl.std.rs` and `ridl.std.ts`, named exactly as every other package is. No
change to the reference form, to the emitted content of any existing artifact,
or to file naming.

### 4.4 Proofs

The rustc corpus proof drops `PRELUDE` and compiles the emitted `ridl.std.rs`
alongside the package sources. The tsc corpus proof drops the copy of
`crates/ridlc/tests/tsc/ridl.std.ts` and resolves against the emitted
`ridl.std.ts`. The stand-in file is deleted.

A test covers the negative case: a workspace referencing no standard type emits
no `ridl.std` artifact.

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
   corpus proofs pass against the emitted artifacts.
3. A workspace that references no standard type emits no `ridl.std` artifact.
4. The drift test fails when the asset and Appendix A disagree.
5. `just build` passes.

## 7. Risks

- **Detection under-reports.** A reference-bearing IR field added later — E3
  will add several — that `referenced_packages` does not read makes the emit
  silently stop happening for workspaces that only reach the standard package
  through that field. Mitigation: the corpus proofs consume the emitted artifact
  (§3.4), so a missed field becomes a compile failure rather than silence, for
  every construct the corpus exercises. Residual: constructs the corpus does not
  exercise. The doc comment naming the `ir.proto` paragraph is what makes the
  pairing visible to whoever adds the field.
- **The negative-case test is the only guard on over-reporting.** If detection
  returned every package unconditionally the positive proofs would still pass.
  §4.4's negative test is what discriminates.
- **Deleting the stand-ins removes a backend-level signal.** The stand-ins let
  the backend proofs fail for backend reasons alone. After this change a corpus
  failure could come from either the backend or the emit path. Accepted: the
  backends keep their own unit tests, and the corpus proof is more valuable as
  an end-to-end proof of the shipped artifact.
