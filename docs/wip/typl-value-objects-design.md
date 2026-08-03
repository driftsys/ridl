# typl value objects — validating constructors in the language backends

Working memory (Superpowers spec). Status: designed, not implemented. Garden
into `docs/decisions/` and `docs/specification/` when the work lands.

## Problem

typl §1.1 states that a pure typl package "generates data types, **validators**,
and documentation across every backend", and the glossary defines SSOT as "one
vocabulary from which types, validators, schemas, and documentation derive
across every backend". Appendix F cites Zod and valibot as prior art and claims
typl "moves the same checks to compile time and codegen".

Neither language backend emits a validator.

- Rust emits `#[repr(transparent)] pub struct Speed(pub f64);`. The inner field
  is public, so the range constraint is unenforceable regardless of what
  constructor exists.
- TypeScript emits `export type Speed = number & { readonly __ridl: '…' };` — a
  compile-time brand with no runtime presence. Construction is `x as Speed`.

Neither backend emits any derive on the typl surface either: no `Debug`, no
`Clone`, no `PartialEq`. The only `#[derive]`s inside a `quote!` block are on
the hand-written interaction vocabulary.

The IR already carries everything needed. `Constraint` holds `min`, `max`,
`step`, `len_min`, `len_max`, `pattern`, and `pattern_const` as exact decimal
strings, and the checker materialises the §4.4 default `[0..256]` for string and
bytes into `len_min`/`len_max`.

## Scope

In scope: named scalar (`type`) declarations, and `TryFrom<i64>` for `enum` and
`enumset` — the point where a raw discriminant or bit pattern off the wire
becomes a validated value.

Out of scope: struct and union constructors (their fields are already validated
types); induced value objects for inline-constrained struct fields; cross-field
struct invariants, which typl §17.7 defers to a future `invariant` block; serde.

## Decisions

1. **The inner value is private; the safe path is the default.** `new` validates
   and returns `Result`; `new_unchecked` escapes for hot paths and values
   already proven valid. `new_unchecked` is **safe, not `unsafe`** — nothing
   here relies on the invariant for memory soundness, and Rust convention
   reserves `unsafe` for that.

2. **Conversions are fallible inbound, infallible outbound.**
   `TryFrom<Inner> for Type` validates; `From<Type> for Inner` extracts. `new`
   delegates to `TryFrom` and remains as the discoverable form, the way
   `NonZeroU32` ships both. Coherence permits `impl From<Speed> for f64` because
   the local type appears as the trait parameter (RFC 2451).

   **`From<Inner> for Type` is never emitted for a constrained type.** It would
   reintroduce infallible construction, and it would collide with the manual
   `TryFrom` through core's `impl<T, U: Into<T>> TryFrom<U> for T` blanket.

3. **A vacuous constraint means no `TryFrom`.** The constraint is vacuous when
   `min`, `max`, `len_min`, `len_max`, and `pattern` are all absent — which,
   because string and bytes always carry a resolved length bound, means exactly:
   `boolean` backings, and `integer`/`float` with no declared range.

   Such a type emits an infallible `const fn new`, `From<Inner> for Type`, and
   `From<Type> for Inner`, and **no** `new_unchecked` (it would duplicate
   `new`). Core's blanket impl then supplies `TryFrom<Inner>` with
   `Error = Infallible`, so generic consumer code calling `try_from` compiles
   uniformly across both kinds.

   The inner field stays private in both cases. Otherwise adding a constraint
   later would flip both field access and constructor fallibility — two breaks
   instead of one.

4. **Pattern validation is a Cargo feature that codegen owns.** `min`/`max` and
   `len_min`/`len_max` are checked unconditionally. `pattern` is checked under
   `validate-pattern`, on by default, which enables an optional `regex`
   dependency. A constrained target builds `--no-default-features`.

   This is why codegen must emit a manifest: `regex` cannot be an optional
   dependency of a bare `.rs` file, and a `#[cfg(feature = "std")]` gate would
   be wrong because most std crates declare no `std` feature at all.

5. **`--emit rust` now produces a compiling crate.** It writes the per-package
   `.rs` files, a `lib.rs` wiring them into the `crate::veh::common` module tree
   with `#[path]`, and one `Cargo.toml` for the whole output directory.

   One manifest, not one per package: `type_path` emits `crate::…` for a
   cross-package reference, so the generated Rust already assumes every package
   is a module inside one crate. One crate per package would break every
   cross-package path.

6. **TypeScript keeps the brand and adds factories.** The runtime value stays a
   primitive, so `JSON.stringify` and the wire shape are unchanged and nothing
   allocates. A constrained type emits three functions; a vacuous one emits only
   the first.

   Type names are CamelCase and constants SCREAMING_SNAKE (typl §15.1), so the
   lowerCamel factory namespace cannot collide.

   `x as Speed` still bypasses validation. No TypeScript design prevents that; a
   class would, at the cost of breaking `JSON.stringify` and every payload
   shape.

7. **Derives.** Always sound, on every generated type: `Debug`, `Clone`,
   `PartialEq`. Conditional: `Copy` when the transitive closure is
   `f64`/`i64`/`bool` only; `Eq` and `Hash` when no `f64` appears anywhere in
   the closure, including unit-backed types since a unit backing implies float.

   `PartialOrd`/`Ord` are derived on **numeric named scalars only**. Ordering a
   struct's fields lexicographically, or a union's arms by declaration order, is
   not a property typl states, and deriving it would invent contract semantics.
   `Ord` requires `Eq`, so a float-backed scalar receives `PartialOrd` alone
   while an integer-backed one receives both.

   Eligibility uses the recursion `defaults.rs` already implements: leaf
   recursion, the C1b cycle guard, and the rule that a composite's one-level
   `derivable` flag is re-checked by recursion rather than trusted.

   **Cross-package references are handled conservatively.** `defaults.rs` can be
   optimistic — it emits `path::default()` and lets rustc verify. A derive
   cannot: `#[derive(Copy)]` on a struct whose cross-package field is not `Copy`
   is a hard error in the consumer's build. So an unresolvable cross-package
   reference in the transitive closure disables the conditional derives.

8. **`Default` is never derived.** `defaults.rs` builds it from the typl init
   value (§5.8), which may be a declared `= 0.5`. `#[derive(Default)]` would
   give the backing's `0.0` and silently contradict the contract.

9. **`Send` and `Sync` emit nothing.** They are auto traits. Every backing is
   `Send + Sync`, so every generated type already is, transitively. Emitting
   anything would mean `unsafe impl`.

## The Rust surface

```rust
// constrained
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Speed(f64);

impl Speed {
    pub fn new(value: f64) -> Result<Self, ConstraintError>;
    pub const fn new_unchecked(value: f64) -> Self;
    pub const fn get(self) -> f64;
}
impl TryFrom<f64> for Speed { type Error = ConstraintError; }
impl From<Speed> for f64 {}

// vacuous
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Enabled(bool);

impl Enabled {
    pub const fn new(value: bool) -> Self;
    pub const fn get(self) -> bool;
}
impl From<bool> for Enabled {}
impl From<Enabled> for bool {}
```

`new_unchecked` must be `const fn` because two existing emitters depend on
tuple-struct construction and switch to it: `defaults.rs` builds `Speed(0.0)`,
and constants emit `const MAX_SPEED: Speed = Speed(250.0)`. Routing both through
the public `new_unchecked` is what lets the field be genuinely private while
cross-package `Default` derivation keeps working — no `pub(crate)` compromise.

`ConstraintError` joins the dependency-free package vocabulary the `interact`
module already emits beside `Provenance` and `SignalHandle`. It carries the type
name, the violated constraint, and no allocation. `core::fmt::Display` always;
`std::error::Error` under the `std` feature.

## The TypeScript surface

```ts
export type Speed = number & { readonly __ridl: 'veh.common.Speed' };

export function speed(v: number): Speed;                 // throws
export function trySpeed(v: number): TryResult<Speed>;   // no throw
export function speedUnchecked(v: number): Speed;        // cast only
```

`TryResult<T>` is
`{ ok: true; value: T } | { ok: false; error: ConstraintError }`, emitted once
per module alongside `ConstraintError` as part of the package vocabulary.

## Not validated, and documented as such

Each generated type carries a doc comment naming what its constructor does not
check, rather than staying silent:

- **`step` quantization** — see Deferred below.
- **Cross-field struct invariants** — deferred by typl §17.7 to an `invariant`
  block.
- **Anything reached through `new_unchecked`, or a TypeScript `as` cast.**

## Deferred — step normalization and steppers

Recorded in the style of typl §17.11: deferred, with the constraints already
settled so the later work does not restart.

Rather than _checking_ that a value sits on the step lattice — which needs a
tolerance the contract does not specify — the constructor **rounds** to the
nearest lattice point. There is then no check to get wrong. Paired with it, a
generated stepper (increment and decrement by one step) serves ADR-0012's
`adjust` operation shape directly: `step` and `adjust` are the same concept at
the vocabulary and interaction layers.

Settled constraints for whenever it lands:

- Rounding replaces checking; no tolerance is introduced anywhere.
- **The rounding mode must be named explicitly.** Rust's `f64::round()` is
  ties-away-from-zero, which is not the IEEE 754 default of ties-to-even, so
  leaving it implicit picks a mode by accident.
- **Round first, then range-check.** Rounding can push a value past `max`:
  `250.4` on `[0.0..250.0 step 0.5]` rounds to `250.5`.
- The stored `f64` is the nearest lattice point _as computed in `f64`_, not an
  exact `min + n·step`. Exactness lives in the scaled-integer transport form of
  typl §4.3, not the language layer.
- It changes `new` from validating to normalizing — a semantic change to a
  shipped API — so it lands with or before anything that depends on step
  exactness.

## Breaking change and migration

Every consumer that reads `.0` or constructs `Speed(x)` breaks. This lands as a
flag day rather than a deprecation path:

- The workspace is at `version = "0.0.0"` with nothing published, so no consumer
  can be pinned to an older release.
- Generated code is derived, not authored. A consumer regenerates the whole
  package at once; the "stay on the old version while migrating" benefit does
  not apply.
- A deprecation path would be actively harmful: emitting both forms keeps the
  field `pub` through the interim, shipping a validation feature that does not
  validate, and inviting new consumer code against a field that breaks later.
- Migration is compiler-driven — every `.0` is a rustc error at the exact site,
  fixed by `.get()` or `From`.

It should land before epic E3 starts, so the boundary-model work is written
against the final shape rather than migrated into it.

The C header is unaffected: `repr(transparent)` with a private field has the
same C ABI.

## Testing

- Snapshot tests in both backends over the constrained and vacuous cases, each
  backing, and each conditional-derive outcome including the cross-package
  conservative path. Both suites largely rewrite.
- A compile test that the emitted crate builds with default features and with
  `--no-default-features`.
- The `crates/ridl/tests/book_examples.rs` harness continues to compile every
  `ridl`/`typl` fence in `docs/book/`.

## Where it gets recorded

Fold into ADR-0013, which is still Proposed. It already classifies backends by
target capability; "a language backend emits validating constructors and the
sound derives" gives the language class the positive definition it currently
lacks, and relieves its Open item 1.

## Open

1. **The crate name for the generated `Cargo.toml`.** `ridlc` has no natural
   source. Default proposed: the `ridl.toml` package name, overridable by
   `--crate-name`. Not settled.
2. **Whether `ridl-diff` classifies a constraint appearing where none existed as
   breaking.** The `From` → `TryFrom` flip in decision 3 depends on it. Verify
   during implementation rather than assume.

## References

- `docs/specification/typl-language-reference.md` — §1.1, §4.2–§4.5, §5.7, §5.8,
  §6, §15.1, §17.7, §17.11, Appendix D, Appendix F
- `docs/decisions/ADR-0013-codegen-backend-scope.md`
- `docs/decisions/ADR-0012-interaction-boundary-model.md` — the `adjust`
  operation shape
- `crates/ridl-backend-rust/src/{lib.rs,defaults.rs,interact.rs}`
- `crates/ridl-backend-ts/src/lib.rs`
- `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` — `Constraint`, `TypeDef`
