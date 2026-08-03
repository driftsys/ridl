# typl Value Objects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every typl named scalar a value object whose constraint is
enforced at construction, and give every generated type the derives that are
sound for it.

**Architecture:** The IR already carries the constraints; no checker or IR
schema change is needed except one shared classifier. The Rust backend gains a
private inner field, `new`/`new_unchecked`, `TryFrom`/`From`, and a derive
eligibility pass reusing the recursion in `defaults.rs`. The TypeScript backend
keeps its compile-time brand and gains factory functions. `ridlc` starts
emitting a `lib.rs` and `Cargo.toml` so the Rust output compiles standalone.

**Tech Stack:** Rust 2024, `proc-macro2`/`quote`/`prettyplease` (Rust emitter),
plain string emission (TypeScript emitter), `insta` snapshots,
`protox`-generated IR types.

**Spec:** `docs/wip/typl-value-objects-design.md`. Where this plan and the spec
disagree, the spec is authoritative.

## Global Constraints

- **The generated Rust default build stays dependency-free** except for the
  optional `regex` behind `validate-pattern`. Emitted code names only `core`,
  `alloc`, and `std` paths otherwise.
- **Codegen never panics.** Every failure is a `GenerateError` value (ADR-0004
  section 5).
- **Conventional Commits**, linted by git-std against `.git-std.toml`. Scopes
  used here: `ridl-ir`, `ridl-backend-rust`, `ridl-backend-ts`, `ridlc`, `adr`,
  `typl`.
- **Never push to `main`.** This work lands on `feat/typl-value-objects` via PR.
- **Run `just verify` before opening the PR** (`lint-commits`, then the full
  `build` gate). Individual tasks run `just test` and `just lint`.
- **Snapshots are `insta`.** Review changes with `cargo insta review`; never
  hand-edit a `.snap` file.
- **Prose is plain and literal** — comments, commit messages, docs. No idioms.
- **`Default` is never derived** — it comes from the typl init value (§5.8).
- **`Send`/`Sync` are never emitted** — they are auto traits.

---

### Task 1: The shared vacuous-constraint classifier

Both backends must agree on when a constraint has nothing to check. A duplicated
predicate would drift, so it lives in `ridl-ir` beside the generated types.

**Files:**

- Modify: `crates/ridl-ir/src/lib.rs`
- Test: `crates/ridl-ir/src/lib.rs` (the existing `#[cfg(test)] mod tests`)

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

### Task 2: The Rust `ConstraintError` vocabulary

**Files:**

- Modify: `crates/ridl-backend-rust/src/interact.rs` (the vocabulary emitter,
  around lines 187-300 where `Provenance` and the metadata structs are emitted)
- Modify: `crates/ridl-backend-rust/src/lib.rs` (call the vocabulary emitter
  unconditionally — today it returns empty for a pure typl package, see
  `interact::emit`'s doc comment "Returns an empty stream when the package
  declares neither an interface nor a service")
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: nothing.
- Produces: an emitted `pub enum ConstraintError` with variants
  `Range { type_name: &'static str }`, `Length { type_name: &'static str }`,
  `Pattern { type_name: &'static str }`, `Variant { type_name: &'static str }`.
  Tasks 3, 4, 5, and 8 construct these. Also
  `pub(crate) fn interact::emit_constraint_error() -> TokenStream`.

Note the behaviour change: a pure typl package currently emits no vocabulary at
all. `ConstraintError` must now be emitted for any package that declares a
non-vacuous named scalar, an `enum`, or an `enumset`, whether or not it declares
an interface.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn constraint_error_vocabulary_for_pure_typl_package() {
    // A package with no interface still needs the error type, because a
    // named scalar's constructor returns it.
    insta::assert_snapshot!(rust_for(vec![speed_decl()]));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridl-backend-rust --locked constraint_error_vocabulary`
Expected: FAIL — insta reports a new snapshot; inspect it and confirm no
`ConstraintError` appears in the output.

- [ ] **Step 3: Write the implementation**

In `crates/ridl-backend-rust/src/interact.rs`:

```rust
/// The constraint-violation error every validating constructor returns.
///
/// Dependency-free: it names only `core` paths and holds a `&'static str`, so
/// it allocates nothing and compiles under `no_std`. `std::error::Error` is
/// implemented under the `std` feature, which the generated manifest enables
/// by default (design spec, decision 4).
pub(crate) fn emit_constraint_error() -> TokenStream {
    quote! {
        /// A value rejected by its type's typl constraints.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ConstraintError {
            /// Outside the declared `[min..max]` range.
            Range { type_name: &'static str },
            /// Outside the declared length bounds.
            Length { type_name: &'static str },
            /// Did not satisfy the declared `match` pattern.
            Pattern { type_name: &'static str },
            /// Not a declared enum discriminant or enum-set bit.
            Variant { type_name: &'static str },
        }

        impl core::fmt::Display for ConstraintError {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self {
                    Self::Range { type_name } => write!(f, "{type_name}: value outside its declared range"),
                    Self::Length { type_name } => write!(f, "{type_name}: value outside its declared length bounds"),
                    Self::Pattern { type_name } => write!(f, "{type_name}: value did not match its declared pattern"),
                    Self::Variant { type_name } => write!(f, "{type_name}: value is not a declared variant"),
                }
            }
        }

        #[cfg(feature = "std")]
        impl std::error::Error for ConstraintError {}
    }
}
```

In `crates/ridl-backend-rust/src/lib.rs`, in `generate`, emit it before the
declaration loop and independently of `interact::emit`'s interface check:

```rust
let mut items: Vec<TokenStream> = Vec::new();
items.push(interact::emit_constraint_error());
```

- [ ] **Step 4: Run the test and accept the snapshot**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked` Expected: PASS. Review the diff
— every existing snapshot gains the `ConstraintError` block.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-rust/
git commit -m "feat(ridl-backend-rust)!: emit the ConstraintError vocabulary

Every validating constructor returns it, so it is emitted for any package
declaring a constrained named scalar, an enum, or an enumset - not only for
one declaring an interface."
```

---

### Task 3: Constrained named scalars — private inner, `new`, `TryFrom`

The core change, and the breaking one.

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_type_def` (line 266),
  `emit_const` (line 283)
- Modify: `crates/ridl-backend-rust/src/defaults.rs` — lines 55 and 183
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `ridl_ir::v2::constraint_is_vacuous` (Task 1), the emitted
  `ConstraintError` (Task 2).
- Produces: for a constrained named scalar `Speed` over `f64`, an emitted
  `Speed::new(f64) -> Result<Speed, ConstraintError>`,
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
    assert!(source.contains("pub fn new(value: f64) -> Result<Self, ConstraintError>"));
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
            #vis fn new(value: #inner) -> Result<Self, ConstraintError> {
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
            type Error = ConstraintError;
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
                return Err(ConstraintError::Range { type_name: #type_name });
            }
        });
    }
    if let Some(max) = c.max.as_deref() {
        let lit = scalar_literal(td, max);
        checks.push(quote! {
            if #value > #lit {
                return Err(ConstraintError::Range { type_name: #type_name });
            }
        });
    }
    // Length is in characters for string (typl §5.3) and bytes for bytes
    // (§5.4), which is why the two use different expressions.
    if c.len_min.is_some() || c.len_max.is_some() {
        let len = match backing_scalar(td) {
            Some(ScalarBacking::String) => quote! { #value.chars().count() as u64 },
            _ => quote! { #value.len() as u64 },
        };
        if let Some(min) = c.len_min {
            let lit = proc_macro2::Literal::u64_unsuffixed(min);
            checks.push(quote! {
                if #len < #lit {
                    return Err(ConstraintError::Length { type_name: #type_name });
                }
            });
        }
        if let Some(max) = c.len_max {
            let lit = proc_macro2::Literal::u64_unsuffixed(max);
            checks.push(quote! {
                if #len > #lit {
                    return Err(ConstraintError::Length { type_name: #type_name });
                }
            });
        }
    }
    quote! { #(#checks)* }
}

/// The accessor. A `Copy` backing returns by value from a `const fn`; `String`
/// and `Vec<u8>` borrow, and gain `into_inner` for the owned form.
fn scalar_getter(td: &v2::TypeDef, vis: TokenStream, inner: TokenStream) -> TokenStream {
    match backing_scalar(td) {
        Some(ScalarBacking::String) => quote! {
            #vis fn get(&self) -> &str { &self.0 }
            #vis fn into_inner(self) -> String { self.0 }
        },
        Some(ScalarBacking::Bytes) => quote! {
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
// line 55
Some(quote! { #name_id::new_unchecked(#inner) })
// line 183
Some(quote! { #path::new_unchecked(#inner) })
```

In `emit_const`, change the three `#type_name(#value)` forms to
`#type_name::new_unchecked(#value)`.

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-rust --accept --unreferenced=reject`
Then: `cargo test -p ridl-backend-rust --locked && cargo test -p ridl --locked`
Expected: PASS, including the `rustc` compile-proof tests at `tests.rs:610` and
`tests.rs:788` — those are the real check that the emitted code is valid Rust.

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
`named_scalar_backings` snapshot at `tests.rs:171` now shows `Enabled` and
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

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `emit_enum` (line 400),
  `emit_enum_set` (line 424)
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: the emitted `ConstraintError` (Task 2).
- Produces: `impl TryFrom<i64> for <Enum>`, `impl From<<Enum>> for i64`, and the
  same pair for each enum set.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn enum_converts_from_a_raw_discriminant() {
    let source = rust_for(vec![gear_position_decl()]);
    assert!(source.contains("impl TryFrom<i64> for GearPosition"));
    assert!(source.contains("impl From<GearPosition> for i64"));
    assert!(source.contains("ConstraintError::Variant"));
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
            type Error = ConstraintError;
            fn try_from(value: i64) -> Result<Self, Self::Error> {
                match value {
                    #(#arms,)*
                    _ => Err(ConstraintError::Variant { type_name: #type_name }),
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
            type Error = ConstraintError;
            fn try_from(value: i64) -> Result<Self, Self::Error> {
                if value & !Self::DECLARED_MASK != 0 {
                    return Err(ConstraintError::Variant { type_name: #type_name });
                }
                Ok(Self(value))
            }
        }

        impl From<#name> for i64 {
            fn from(value: #name) -> Self { value.0 }
        }
    }
```

Note the enum set's inner field is already emitted as `#vis i64` at
`lib.rs:438`; change it to private `i64` for consistency with Task 3, and add
`#vis const fn get(self) -> i64 { self.0 }` to its impl block.

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
        Some(ScalarBacking::Float) | Some(ScalarBacking::Integer)
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
            .fields
            .iter()
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
        // A unit backing implies float, so it lands here too.
        Some(ScalarBacking::Float) => Eligibility { copy: true, eq: false },
        Some(ScalarBacking::Integer) | Some(ScalarBacking::Boolean) => Eligibility::ALL,
        Some(ScalarBacking::String) | Some(ScalarBacking::Bytes) => {
            Eligibility { copy: false, eq: true }
        }
        None => Eligibility::NONE,
    }
}
```

```rust
/// One field's contribution. An `Option<T>` keeps `T`'s eligibility — both
/// `Copy` and `Eq` pass through it. A collection is never `Copy` because
/// `Vec` is not, but keeps `Eq` when its element does.
fn field_eligibility(ctx: &Ctx, field: &v2::Field, seen: &mut HashSet<String>) -> Eligibility {
    let Some(ft) = field.field_type.as_ref() else {
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
            .fold(Eligibility::ALL, |acc, f| acc.meet(field_eligibility(ctx, f, seen))),
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

The exact `v2::field_type::Kind` variant names and the `Array`/`Map` element
accessors must be read from `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` before
writing this — the names above follow the shape `defaults.rs` walks, and any
mismatch surfaces immediately as a compile error rather than silently.

In `lib.rs`: add `mod derives;` beside `mod defaults;`, prepend the attribute in
`emit_decl` so every emitted item carries it, and raise `struct Ctx<'a>` and its
`lookup` method to `pub(crate)` so `derives.rs` can reach them — `defaults.rs`
already takes `&Ctx`, so this is a visibility change only.

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

**Files:**

- Modify: `crates/ridlc/src/lib.rs` — `write_emits`, and the `Emit::Rust` arm
- Create: `crates/ridlc/templates/cargo.toml.j2` (minijinja, matching the
  existing `templates/c_header.j2` precedent)
- Test: `crates/ridlc/src/lib.rs` tests, or `crates/ridl/tests/`

**Interfaces:**

- Consumes: the set of package names in `checked` (already available in
  `run_build`).
- Produces: `<out_dir>/Cargo.toml` and `<out_dir>/lib.rs` whenever `Emit::Rust`
  is selected in package or workspace mode.

Single-file mode keeps writing only `<stem>.rs`, matching the existing
single-file asymmetry documented on `Emit::TypeScript`.

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

Create `crates/ridlc/templates/cargo.toml.j2`:

```jinja
# Generated by ridlc. Do not edit.
[package]
name = "{{ crate_name }}"
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
regex = { version = "1", optional = true }

[lib]
path = "lib.rs"
```

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

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` — `constraint_checks`
- Test: `crates/ridl-backend-rust/src/tests.rs`

**Interfaces:**

- Consumes: `constraint_checks` (Task 3), the manifest feature (Task 7).
- Produces: nothing new; extends the generated `new`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pattern_check_is_feature_gated() {
    let source = rust_for(vec![vin_decl()]);
    assert!(source.contains("#[cfg(feature = \"validate-pattern\")]"));
    assert!(source.contains("ConstraintError::Pattern"));
    // The length check is not gated - it needs no dependency.
    let gated = source.split("#[cfg(feature = \"validate-pattern\")]").next().unwrap();
    assert!(gated.contains("ConstraintError::Length"));
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
                return Err(ConstraintError::Pattern { type_name: #type_name });
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

### Task 9: TypeScript vocabulary and factories

**Files:**

- Modify: `crates/ridl-backend-ts/src/lib.rs` — `emit_type_def` (line 225),
  `generate` (to emit the vocabulary)
- Test: `crates/ridl-backend-ts/src/tests.rs`

**Interfaces:**

- Consumes: `constraint_is_vacuous` (Task 1).
- Produces: emitted `ConstraintError` class, `TryResult<T>` type, and per type
  `speed(v)`, `trySpeed(v)`, `speedUnchecked(v)`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn constrained_scalar_emits_three_factories() {
    let source = ts_for(vec![speed_decl()]);
    assert!(source.contains("export function speed(value: number): Speed"));
    assert!(source.contains("export function trySpeed(value: number): TryResult<Speed>"));
    assert!(source.contains("export function speedUnchecked(value: number): Speed"));
    assert!(source.contains("export type TryResult<T>"));
}

#[test]
fn vacuous_scalar_emits_one_factory() {
    let decls = vec![public_decl(
        "Enabled",
        primitive_type(v2::PrimitiveType::Boolean, init_value(true, Some("false")), None),
    )];
    let source = ts_for(decls);
    assert!(source.contains("export function enabled(value: boolean): Enabled"));
    assert!(!source.contains("tryEnabled"));
    assert!(!source.contains("enabledUnchecked"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-backend-ts --locked factories` Expected: FAIL — only
the brand type is emitted.

- [ ] **Step 3: Write the implementation**

Emit the vocabulary once per module:

```ts
export class ConstraintError extends Error {
  constructor(readonly typeName: string, readonly constraint: 'range' | 'length' | 'pattern' | 'variant') {
    super(`${typeName}: value violates its declared ${constraint} constraint`);
    this.name = 'ConstraintError';
  }
}

export type TryResult<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: ConstraintError };
```

Then per constrained type, with the factory name being the lowerCamel of the
declaration name — safe because type names are CamelCase and constants are
SCREAMING_SNAKE (typl §15.1), so the namespaces cannot collide:

```ts
export function speed(value: number): Speed {
  if (value < 0.0 || value > 250.0) throw new ConstraintError('Speed', 'range');
  return value as Speed;
}

export function trySpeed(value: number): TryResult<Speed> {
  try { return { ok: true, value: speed(value) }; }
  catch (error) { return { ok: false, error: error as ConstraintError }; }
}

export function speedUnchecked(value: number): Speed {
  return value as Speed;
}
```

Length checks use `[...value].length` for strings, so a surrogate pair counts as
one character — typl §5.3 bounds strings in characters, not UTF-16 code units.
Pattern checks use `RegExp` directly; no feature gate exists in TypeScript.

A vacuous type emits only the first function, with no checks in its body.

- [ ] **Step 4: Run the tests**

Run: `cargo insta test -p ridl-backend-ts --accept --unreferenced=reject` Then:
`cargo test -p ridl-backend-ts --locked` Expected: PASS, including
`appendix_b_compiles_with_tsc_strict` at `tests.rs:1378`, which is the real
check.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-backend-ts/
git commit -m "feat(ridl-backend-ts): emit validating factories for named scalars

The brand is kept rather than replaced by a class, so runtime values stay
primitives and JSON.stringify and the wire shape are unchanged. A
constrained type emits a throwing factory, a result-returning one, and an
unchecked cast; a vacuous type emits only the first.

String length is measured in code points, matching typl 5.3, which bounds
strings in characters rather than UTF-16 code units."
```

---

### Task 10: Record the decision and verify the diff classification

**Files:**

- Modify: `docs/decisions/ADR-0013-codegen-backend-scope.md`
- Modify: `docs/specification/typl-language-reference.md` — §5.7
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
`sdd-gardening` skill rather than doing it by hand.

- [ ] **Step 5: Run the full gate and commit**

```bash
just verify
git add docs/
git commit -m "docs(typl): record validating constructors in ADR-0013 and typl 5.7"
```

---

## Verification

Before opening the PR:

```bash
just verify     # lint-commits, then the full build gate
```

`just build` covers `toolchain-check`, `gate-parity`, `fmt-check`, `book-check`,
`compile`, `test`, `lint`, `wasm-check`, and `check`. The `docs/wip/` directory
must be empty by then, or the working-memory gate flags the branch as
unfinished.

## Open

1. **The crate name for the generated `Cargo.toml`** (Task 7). Default proposed:
   the `ridl.toml` package name, falling back to `ridl_generated`, overridable
   by a `--crate-name` flag. Confirm before implementing Task 7.
2. **Whether `ridl-diff` classifies a constraint appearing where none existed as
   breaking** (Task 10, Step 1). Verify rather than assume.
