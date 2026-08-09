//! IR v2 package to TypeScript source (E2.6a, ADR-0008 decision 7).
//!
//! The typl surface of a [`v2::Package`] is realized as one TypeScript module
//! per package. The TypeScript language layer, fixed by this backend (typl
//! reference Appendix D):
//!
//! - **Named scalar** — a branded type,
//!   `export type Speed = number & { readonly __ridl: 'veh.common.Speed' };`,
//!   so nominal identity survives TypeScript's structural typing (typl §5.7).
//!   The brand base follows the backing: float and narrow integers are
//!   `number`; U64/I64 widths are `bigint` — a value past 2^53 has no exact
//!   `number` form; boolean/string map directly; bytes is `Uint8Array`.
//! - **Constant** — `export const MAX_SPEED = 250.0;` (`as const` for
//!   strings). A constant of a bigint-branded type is an `n`-suffixed
//!   literal.
//! - **Struct** — `export interface`, optional fields as `name?:`, reserved
//!   tombstones occupy an ordinal but emit no property (typl §7.4).
//! - **Enum** — `export enum` with the declared discriminants. TypeScript
//!   enums are number-valued, so a discriminant beyond
//!   `Number.MAX_SAFE_INTEGER` is [`GenerateError::Unrepresentable`].
//! - **Enumset** — a branded number plus a `<Name>Bits` const object
//!   (typl §9). U64/I64 widths brand `bigint`: JS number bitwise operators
//!   truncate to 32 bits, so a narrow-branded set past bit 31 would be
//!   silently wrong and is rejected instead.
//! - **Union** — a discriminated union of `{ kind: '<arm>'; value: T }`
//!   members (typl §10).
//! - **Tuple** — an inline object type with the named fields (typl §11).
//! - **Array** — `readonly T[]`, bounds in a JSDoc `@bounds` tag.
//! - **Map** — `ReadonlyArray<readonly [K, V]>`, deterministic entry order
//!   (the Rust `Vec<(K, V)>` decision carried over, typl §12.2).
//! - **Bare `integer` primitive** in field position — `bigint`: it carries no
//!   width, so there is no proof the value fits 2^53.
//! - **Docs** — JSDoc with `@unit`, `@range`, `@bounds`, and `@deprecated`
//!   tags (typl §14). `internal` declarations stay module-local — no
//!   `export` (ADR-0002 §8).
//! - **Cross-package reference** `pkg.Name` — a namespace import
//!   `import * as pkg_name from './pkg.name';`, one module per package.
//!
//! Init derivation mirrors the Rust backend's `Default` derivation
//! (`crates/ridl-backend-rust/src/defaults.rs`): one `export function init<Name>()` per
//! declaration whose init is transitively derivable, following the
//! leaf-recursion rule — the IR `InitValue.derivable` flag on a
//! composite-typed field is a one-level flag, so same-package composite
//! references are re-checked by recursion, with a cycle guard (C1b).
//!
//! The emitter is a plain string emitter: deterministic, stable source-order
//! output, and total — every failure is a [`GenerateError`], never a panic
//! (ADR-0004 section 5). typl names never need escaping in TypeScript:
//! declaration names are CamelCase or SCREAMING_SNAKE (typl §15.1), which no
//! all-lowercase TypeScript reserved word collides with, and property names
//! admit any identifier, reserved words included.

use ridl_ir::v2;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};

/// The generated TypeScript for one package: one module per package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTs {
    pub source: String,
}

/// A failure to generate code from a package. Carried as a value so codegen
/// stays total: no stage panics (ADR-0004 section 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateError {
    /// The package contains a construct with no sound TypeScript
    /// representation; the message names the declaration and the reason.
    Unrepresentable(String),
}

/// The largest integer TypeScript's `number` holds exactly
/// (`Number.MAX_SAFE_INTEGER`, 2^53 - 1).
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Generates the TypeScript module for `package`: the typl surface first, in
/// declaration order, then the ridl interaction layer ([`interact`]) — the
/// interaction faces reference the typl types above them, so the reading
/// order matches the dependency order.
///
/// The call is total: it returns [`GenerateError`] rather than panicking.
pub fn generate(package: &v2::Package) -> Result<GeneratedTs, GenerateError> {
    let ctx = Ctx::new(package);

    let mut blocks: Vec<String> = Vec::new();
    for decl in &package.decls {
        emit_decl(&ctx, decl, &mut blocks)?;
    }

    let mut source = String::new();
    let imports = ctx.imports.borrow();
    for import in imports.iter() {
        source.push_str(&format!(
            "import * as {} from './{}';\n",
            package_alias(import),
            import
        ));
    }
    if !imports.is_empty() && !blocks.is_empty() {
        source.push('\n');
    }
    source.push_str(&blocks.join("\n"));
    Ok(GeneratedTs { source })
}

// ---------------------------------------------------------------------------
// Package context — same-package name lookups for the leaf-recursion rules.
// ---------------------------------------------------------------------------

/// Read-only view of a package indexed by declaration name, so the emitter
/// and the init-derivation pass can resolve a same-package reference to its
/// declaration (cross-package references stay unresolved by design — this
/// backend generates one package at a time).
struct Ctx<'a> {
    package: &'a str,
    decls: HashMap<&'a str, &'a v2::Decl>,
    /// The set of declaration names currently being expanded by the
    /// init-derivation recursion. It guards against a cyclic IR: a
    /// same-package composite that reaches itself would otherwise recurse
    /// forever (C1b).
    visiting: RefCell<HashSet<String>>,
    /// External packages referenced by emitted code, collected during
    /// emission; a `BTreeSet` keeps the import lines sorted and
    /// deterministic.
    imports: RefCell<BTreeSet<String>>,
}

impl<'a> Ctx<'a> {
    fn new(package: &'a v2::Package) -> Self {
        let decls = package
            .decls
            .iter()
            .map(|decl| (decl.name.as_str(), decl))
            .collect();
        Ctx {
            package: &package.name,
            decls,
            visiting: RefCell::new(HashSet::new()),
            imports: RefCell::new(BTreeSet::new()),
        }
    }

    /// The declaration named `name` in this package, or `None` for a
    /// cross-package (dotted) or unknown reference.
    fn lookup(&self, name: &str) -> Option<&'a v2::Decl> {
        self.decls.get(name).copied()
    }

    /// Marks `name` as being expanded by the init recursion. Returns `true`
    /// when it was newly inserted, `false` when it is already on the
    /// expansion stack — a reference cycle the caller must not recurse into.
    fn enter_init(&self, name: &str) -> bool {
        self.visiting.borrow_mut().insert(name.to_string())
    }

    /// Removes `name` from the init-recursion expansion stack, balancing a
    /// prior [`enter_init`](Ctx::enter_init) that returned `true`.
    fn leave_init(&self, name: &str) {
        self.visiting.borrow_mut().remove(name);
    }

    /// The TypeScript reference for a resolved type reference: the bare name
    /// for a same-package reference, `pkg_alias.Name` (with the namespace
    /// import registered) for a cross-package `pkg.Name` reference.
    fn type_ref(&self, reference: &str) -> String {
        match reference.rsplit_once('.') {
            Some((package, name)) => {
                self.imports.borrow_mut().insert(package.to_string());
                format!("{}.{}", package_alias(package), name)
            }
            None => reference.to_string(),
        }
    }

    /// The cross-package init-function call `pkg_alias.initName()`, with the
    /// namespace import registered.
    fn cross_package_init_call(&self, package: &str, name: &str) -> String {
        self.imports.borrow_mut().insert(package.to_string());
        format!("{}.init{}()", package_alias(package), name)
    }
}

/// The namespace-import alias for a package name: dots become underscores
/// (`ridl.std` imports as `ridl_std`).
fn package_alias(package: &str) -> String {
    package.replace('.', "_")
}

// ---------------------------------------------------------------------------
// Declaration emission.
// ---------------------------------------------------------------------------

fn emit_decl(ctx: &Ctx, decl: &v2::Decl, blocks: &mut Vec<String>) -> Result<(), GenerateError> {
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => blocks.push(emit_type_def(ctx, decl, td)),
        Some(v2::decl::Kind::ConstDef(cd)) => {
            if let Some(block) = emit_const(ctx, decl, cd)? {
                blocks.push(block);
            }
        }
        Some(v2::decl::Kind::StructDef(sd)) => blocks.push(emit_struct(ctx, decl, sd)?),
        Some(v2::decl::Kind::EnumDef(ed)) => blocks.push(emit_enum(decl, ed)?),
        Some(v2::decl::Kind::EnumSetDef(esd)) => blocks.push(emit_enum_set(ctx, decl, esd)?),
        Some(v2::decl::Kind::UnionDef(ud)) => blocks.push(emit_union(ctx, decl, ud)),
        // Interaction kinds and tombstones belong to the ridl layer inside
        // interfaces (task 14); at package level they carry nothing to emit.
        Some(_) | None => return Ok(()),
    }

    if let Some(init_fn) = init_function(ctx, decl) {
        blocks.push(init_fn);
    }
    Ok(())
}

/// A named scalar type becomes a branded type (typl §5.7): the structural
/// base intersected with a unique literal-typed marker property, so two
/// scalars with the same base stay non-assignable.
fn emit_type_def(ctx: &Ctx, decl: &v2::Decl, td: &v2::TypeDef) -> String {
    let doc = jsdoc("", &decl.doc, &type_def_tags(decl, td));
    let export = export_kw(decl.visibility);
    let base = scalar_base(td);
    let brand = ts_string(&format!("{}.{}", ctx.package, decl.name));
    format!(
        "{doc}{export}type {name} = {base} & {{ readonly __ridl: {brand} }};\n",
        name = decl.name
    )
}

/// A constant becomes an `export const` with an inferred literal type
/// (`as const` for strings, so the literal type is kept). A constant whose
/// backing cannot be resolved here (a cross-package type) or has no literal
/// form (bytes) is skipped rather than mis-typed, mirroring the Rust
/// backend.
fn emit_const(
    ctx: &Ctx,
    decl: &v2::Decl,
    cd: &v2::ConstDef,
) -> Result<Option<String>, GenerateError> {
    let doc = jsdoc("", &decl.doc, &deprecated_tags(decl.deprecated.as_deref()));
    let export = export_kw(decl.visibility);
    let name = &decl.name;

    // A regex constant declares no type; it holds the pattern source text.
    // The IR stores that text with its typl `/…/` delimiters, which are
    // syntax, not pattern content, so they are stripped before the string
    // value is emitted (M1).
    if let Some(regex) = &cd.regex {
        let pattern = ts_string(strip_regex_delimiters(regex));
        return Ok(Some(format!(
            "{doc}{export}const {name} = {pattern} as const;\n"
        )));
    }

    let Some(type_ref) = cd.type_ref.as_deref() else {
        return Ok(None);
    };

    let value = if let Some((backing, wide)) = same_package_scalar_backing(ctx, type_ref) {
        match backing {
            ScalarBacking::Float => float_literal(&cd.value),
            ScalarBacking::Integer if wide => bigint_literal(name, &cd.value)?,
            ScalarBacking::Integer => {
                // Defense in depth: the width claims the value is narrow, but
                // a malformed IR could still carry a value past 2^53 — as a
                // `number` literal it would silently round, and TypeScript
                // would compile the wrong value without complaint.
                if !fits_safe_number(&cd.value) {
                    return Err(GenerateError::Unrepresentable(format!(
                        "constant {name}: value {value:?} exceeds Number.MAX_SAFE_INTEGER \
                         but its type claims a narrow (non-64-bit) integer width",
                        value = cd.value
                    )));
                }
                cd.value.clone()
            }
            ScalarBacking::Boolean => bool_literal(&cd.value).to_string(),
            ScalarBacking::String => format!("{} as const", ts_string(&cd.value)),
            ScalarBacking::Bytes => return Ok(None),
        }
    } else if let Some(prim) = primitive_keyword(type_ref) {
        match prim {
            v2::PrimitiveType::Integer => {
                // The value is known here, so the Appendix D footnote applies
                // directly: `number` when it fits 2^53, `bigint` otherwise.
                if fits_safe_number(&cd.value) {
                    cd.value.clone()
                } else {
                    bigint_literal(name, &cd.value)?
                }
            }
            v2::PrimitiveType::Float => float_literal(&cd.value),
            v2::PrimitiveType::Boolean => bool_literal(&cd.value).to_string(),
            v2::PrimitiveType::String => format!("{} as const", ts_string(&cd.value)),
            v2::PrimitiveType::Bytes | v2::PrimitiveType::Unspecified => return Ok(None),
        }
    } else {
        // A cross-package or unresolved constant type: the backing is unknown
        // here, so the constant is skipped rather than mis-typed.
        return Ok(None);
    };

    Ok(Some(format!("{doc}{export}const {name} = {value};\n")))
}

fn emit_struct(ctx: &Ctx, decl: &v2::Decl, sd: &v2::StructDef) -> Result<String, GenerateError> {
    let doc = jsdoc("", &decl.doc, &deprecated_tags(decl.deprecated.as_deref()));
    let export = export_kw(decl.visibility);
    let name = &decl.name;

    let mut props = String::new();
    for member in &sd.members {
        match &member.member {
            Some(v2::struct_member::Member::Field(field)) => {
                props.push_str(&emit_field(ctx, field)?);
            }
            // A reserved tombstone occupies an ordinal but emits no property
            // (typl §7.4).
            Some(v2::struct_member::Member::Reserved(_)) | None => {}
        }
    }

    if props.is_empty() {
        Ok(format!("{doc}{export}interface {name} {{}}\n"))
    } else {
        Ok(format!("{doc}{export}interface {name} {{\n{props}}}\n"))
    }
}

fn emit_field(ctx: &Ctx, field: &v2::Field) -> Result<String, GenerateError> {
    let mut tags = Vec::new();
    if let Some(v2::field_type::Kind::InlineScalar(td)) =
        field.r#type.as_ref().and_then(|ft| ft.kind.as_ref())
    {
        if let Some(unit) = unit_tag(td) {
            tags.push(unit);
        }
        if let Some(range) = td.constraint.as_ref().and_then(range_tag) {
            tags.push(range);
        }
    }
    if let Some(bounds) = field
        .r#type
        .as_ref()
        .and_then(|ft| ft.kind.as_ref())
        .and_then(bounds_tag)
    {
        tags.push(bounds);
    }
    tags.extend(deprecated_tags(field.deprecated.as_deref()));

    let doc = jsdoc("  ", &field.doc, &tags);
    let (optional, ty) = match field.r#type.as_ref() {
        // The property form `name?: T` carries the absence, so the top-level
        // optional flag is peeled here rather than rendered `T | undefined`.
        Some(ft) => (ft.optional, kind_ts(ctx, ft.kind.as_ref())?),
        None => (false, "unknown".to_string()),
    };
    let marker = if optional { "?" } else { "" };
    Ok(format!("{doc}  {name}{marker}: {ty};\n", name = field.name))
}

/// An enum becomes a TypeScript `enum` with the declared discriminants
/// (typl §8). Variant names keep their typl `SCREAMING_SNAKE` spelling.
fn emit_enum(decl: &v2::Decl, ed: &v2::EnumDef) -> Result<String, GenerateError> {
    let doc = jsdoc("", &decl.doc, &deprecated_tags(decl.deprecated.as_deref()));
    let export = export_kw(decl.visibility);
    let name = &decl.name;

    let mut members = String::new();
    for value in &ed.values {
        if value.value.unsigned_abs() > MAX_SAFE_INTEGER {
            return Err(GenerateError::Unrepresentable(format!(
                "enum {name} value {member} = {value} exceeds Number.MAX_SAFE_INTEGER; \
                 TypeScript enums are number-valued",
                member = value.name,
                value = value.value
            )));
        }
        members.push_str(&jsdoc("  ", &value.doc, &[]));
        members.push_str(&format!(
            "  {member} = {value},\n",
            member = value.name,
            value = value.value
        ));
    }

    if members.is_empty() {
        Ok(format!("{doc}{export}enum {name} {{}}\n"))
    } else {
        Ok(format!("{doc}{export}enum {name} {{\n{members}}}\n"))
    }
}

/// An enum set becomes a branded scalar plus a `<Name>Bits` const object
/// holding the computed masks (typl §9). Narrow widths brand `number` and
/// admit bits 0..=31 (JS number bitwise operators truncate to 32 bits);
/// U64/I64 widths brand `bigint` and admit bits 0..=63.
fn emit_enum_set(
    ctx: &Ctx,
    decl: &v2::Decl,
    esd: &v2::EnumSetDef,
) -> Result<String, GenerateError> {
    let doc = jsdoc("", &decl.doc, &deprecated_tags(decl.deprecated.as_deref()));
    let export = export_kw(decl.visibility);
    let name = &decl.name;
    let wide = matches!(
        v2::IntWidth::try_from(esd.width),
        Ok(v2::IntWidth::U64 | v2::IntWidth::I64)
    );
    let (base, max_bit, suffix) = if wide {
        ("bigint", 63, "n")
    } else {
        ("number", 31, "")
    };

    let mut bits = String::new();
    for bit in &esd.bits {
        if bit.value < 0 || bit.value > max_bit {
            return Err(GenerateError::Unrepresentable(format!(
                "enumset {name} bit {member} = {value} does not fit the {base}-branded \
                 form (bits 0..={max_bit})",
                member = bit.name,
                value = bit.value
            )));
        }
        let mask = 1u128 << bit.value;
        bits.push_str(&jsdoc("  ", &bit.doc, &[]));
        bits.push_str(&format!("  {member}: {mask}{suffix},\n", member = bit.name));
    }

    let brand = ts_string(&format!("{}.{}", ctx.package, decl.name));
    Ok(format!(
        "{doc}{export}type {name} = {base} & {{ readonly __ridl: {brand} }};\n\
         \n\
         {export}const {name}Bits = {{\n{bits}}} as const;\n"
    ))
}

/// A union becomes a discriminated union type: one `{ kind; value }` member
/// per arm, the arm name as the `kind` literal (typl §10). Reserved arms
/// occupy an ordinal but emit no member.
fn emit_union(ctx: &Ctx, decl: &v2::Decl, ud: &v2::UnionDef) -> String {
    let doc = jsdoc("", &decl.doc, &deprecated_tags(decl.deprecated.as_deref()));
    let export = export_kw(decl.visibility);
    let name = &decl.name;

    if ud.arms.is_empty() {
        return format!("{doc}{export}type {name} = never;\n");
    }

    let mut members = String::new();
    for arm in &ud.arms {
        members.push_str(&jsdoc("  ", &arm.doc, &[]));
        members.push_str(&format!(
            "  | {{ kind: {kind}; value: {ty} }}\n",
            kind = ts_string(&arm.name),
            ty = ctx.type_ref(&arm.type_ref)
        ));
    }
    // The last member line carries the terminating semicolon.
    let members = members.trim_end_matches('\n');
    format!("{doc}{export}type {name} =\n{members};\n")
}

// ---------------------------------------------------------------------------
// Type mapping.
// ---------------------------------------------------------------------------

/// The TypeScript type of a field-type kind. `None` (an absent kind) maps to
/// `unknown` — total, and honest about the missing information.
fn kind_ts(ctx: &Ctx, kind: Option<&v2::field_type::Kind>) -> Result<String, GenerateError> {
    let Some(kind) = kind else {
        return Ok("unknown".to_string());
    };
    match kind {
        v2::field_type::Kind::Named(name) => Ok(ctx.type_ref(name)),
        v2::field_type::Kind::Primitive(prim) => Ok(primitive_ts(*prim).to_string()),
        v2::field_type::Kind::InlineScalar(td) => Ok(scalar_base(td).to_string()),
        v2::field_type::Kind::Tuple(tuple) => tuple_type_ts(ctx, tuple),
        v2::field_type::Kind::Array(array) => {
            let element = match array.element.as_deref() {
                Some(element) => field_type_ts(ctx, element)?,
                None => "unknown".to_string(),
            };
            Ok(format!("readonly {}[]", parenthesized(element)))
        }
        v2::field_type::Kind::Map(map) => {
            let key = match map.key.as_deref() {
                Some(key) => field_type_ts(ctx, key)?,
                None => "unknown".to_string(),
            };
            let value = match map.value.as_deref() {
                Some(value) => field_type_ts(ctx, value)?,
                None => "unknown".to_string(),
            };
            Ok(format!("ReadonlyArray<readonly [{key}, {value}]>"))
        }
        // The stream container is a ridl interaction shape (ridl §12); it has
        // no typl-surface position, so meeting one here is an IR
        // inconsistency, not a mapping gap to guess around.
        v2::field_type::Kind::Stream(_) => Err(GenerateError::Unrepresentable(
            "a stream type has no typl-surface representation; streams belong to \
             interaction parameters and returns (ridl §12)"
                .to_string(),
        )),
    }
}

/// The TypeScript type of a nested field type (array element, map key or
/// value): the optional flag renders as `| undefined` — the property form
/// `?:` only exists on struct and tuple fields.
fn field_type_ts(ctx: &Ctx, ft: &v2::FieldType) -> Result<String, GenerateError> {
    let inner = kind_ts(ctx, ft.kind.as_ref())?;
    if ft.optional {
        Ok(format!("{inner} | undefined"))
    } else {
        Ok(inner)
    }
}

/// An anonymous tuple becomes an inline object type with the named fields
/// (typl §11): `{ min: Speed; max: Speed }`.
fn tuple_type_ts(ctx: &Ctx, tuple: &v2::TupleType) -> Result<String, GenerateError> {
    if tuple.fields.is_empty() {
        return Ok("{}".to_string());
    }
    let mut fields = Vec::with_capacity(tuple.fields.len());
    for field in &tuple.fields {
        let (optional, ty) = match field.r#type.as_ref() {
            Some(ft) => (ft.optional, kind_ts(ctx, ft.kind.as_ref())?),
            None => (false, "unknown".to_string()),
        };
        let marker = if optional { "?" } else { "" };
        fields.push(format!("{}{marker}: {ty}", field.name));
    }
    Ok(format!("{{ {} }}", fields.join("; ")))
}

/// Wraps an array element type in parentheses when the unparenthesized form
/// would misparse (`readonly (readonly T[])[]`) or mis-bind
/// (`(T | undefined)[]`). Extra parentheses on an inline object type that
/// happens to contain `|` in a nested position are harmless.
fn parenthesized(element: String) -> String {
    if element.starts_with("readonly ") || element.contains(" | ") {
        format!("({element})")
    } else {
        element
    }
}

/// The brand base of a named or inline scalar (Appendix D language layer):
/// float backs to `number` (a JS number is an IEEE 754 double), integers to
/// `number` except the U64/I64 widths, whose values may exceed 2^53 and
/// therefore back to `bigint`.
fn scalar_base(td: &v2::TypeDef) -> &'static str {
    match backing_scalar(td) {
        ScalarBacking::Float => "number",
        ScalarBacking::Integer if is_wide(td) => "bigint",
        ScalarBacking::Integer => "number",
        ScalarBacking::Boolean => "boolean",
        ScalarBacking::String => "string",
        ScalarBacking::Bytes => "Uint8Array",
    }
}

/// A bare primitive in field position (typl §15.3, map keys §12.2). The
/// unconstrained `integer` carries no width, so nothing proves its values
/// fit 2^53 — it maps to `bigint`, the widest exact integer TypeScript has.
fn primitive_ts(prim: i32) -> &'static str {
    match v2::PrimitiveType::try_from(prim).unwrap_or(v2::PrimitiveType::Unspecified) {
        v2::PrimitiveType::Boolean => "boolean",
        v2::PrimitiveType::Integer => "bigint",
        v2::PrimitiveType::Float => "number",
        v2::PrimitiveType::String => "string",
        v2::PrimitiveType::Bytes => "Uint8Array",
        v2::PrimitiveType::Unspecified => "unknown",
    }
}

/// True when the type's derived width is one of the 64-bit integer widths —
/// the bigint-branding rule (exactness beyond 2^53).
fn is_wide(td: &v2::TypeDef) -> bool {
    matches!(
        td.width,
        Some(v2::type_def::Width::IntWidth(w))
            if matches!(v2::IntWidth::try_from(w), Ok(v2::IntWidth::U64 | v2::IntWidth::I64))
    )
}

/// Strips a regex literal's surrounding `/…/` delimiters, leaving the
/// pattern body. A value without both delimiters is returned unchanged.
fn strip_regex_delimiters(regex: &str) -> &str {
    regex
        .strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
        .unwrap_or(regex)
}

// ---------------------------------------------------------------------------
// Scalar backing classification (shared by emission and init derivation).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarBacking {
    Float,
    Integer,
    Boolean,
    String,
    Bytes,
}

/// The scalar class of a type definition's backing. A unit backing implies
/// float (typl §5.1).
fn backing_scalar(td: &v2::TypeDef) -> ScalarBacking {
    match td.backing.as_ref().and_then(|b| b.kind.as_ref()) {
        Some(v2::backing::Kind::Unit(_)) => ScalarBacking::Float,
        Some(v2::backing::Kind::Primitive(prim)) => {
            match v2::PrimitiveType::try_from(*prim).unwrap_or(v2::PrimitiveType::Unspecified) {
                v2::PrimitiveType::Boolean => ScalarBacking::Boolean,
                v2::PrimitiveType::Integer => ScalarBacking::Integer,
                v2::PrimitiveType::Float => ScalarBacking::Float,
                v2::PrimitiveType::String => ScalarBacking::String,
                v2::PrimitiveType::Bytes | v2::PrimitiveType::Unspecified => ScalarBacking::Bytes,
            }
        }
        None => ScalarBacking::Float,
    }
}

/// The backing class and 64-bit-width flag of a same-package named scalar
/// type, or `None` when the reference does not name a scalar `TypeDef` in
/// this package.
fn same_package_scalar_backing(ctx: &Ctx, reference: &str) -> Option<(ScalarBacking, bool)> {
    match &ctx.lookup(reference)?.kind {
        Some(v2::decl::Kind::TypeDef(td)) => Some((backing_scalar(td), is_wide(td))),
        _ => None,
    }
}

/// Maps a typl primitive keyword written as a type reference to its
/// primitive.
fn primitive_keyword(reference: &str) -> Option<v2::PrimitiveType> {
    match reference {
        "boolean" => Some(v2::PrimitiveType::Boolean),
        "integer" => Some(v2::PrimitiveType::Integer),
        "float" => Some(v2::PrimitiveType::Float),
        "string" => Some(v2::PrimitiveType::String),
        "bytes" => Some(v2::PrimitiveType::Bytes),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Init-function derivation — the leaf-recursion rule (typl §5.8), mirroring
// crates/ridl-backend-rust/src/defaults.rs.
// ---------------------------------------------------------------------------

/// The complete `function init<Name>()` block for a declaration, or `None`
/// when the type is not fully init-constructible.
fn init_function(ctx: &Ctx, decl: &v2::Decl) -> Option<String> {
    let expr = decl_init_expr(ctx, decl)?;
    let export = export_kw(decl.visibility);
    let name = &decl.name;
    Some(format!(
        "{export}function init{name}(): {name} {{\n  return {expr};\n}}\n"
    ))
}

/// The expression behind `return` for a top-level declaration's init
/// function, or `None` when the declaration is not init-constructible.
fn decl_init_expr(ctx: &Ctx, decl: &v2::Decl) -> Option<String> {
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => type_def_init(&decl.name, td),
        Some(v2::decl::Kind::StructDef(sd)) => struct_init(ctx, sd),
        Some(v2::decl::Kind::EnumDef(ed)) => enum_init(&decl.name, ed),
        Some(v2::decl::Kind::EnumSetDef(esd)) => Some(enum_set_init(&decl.name, esd)),
        Some(v2::decl::Kind::UnionDef(ud)) => union_init(ctx, ud),
        _ => None,
    }
}

/// Reference position: what is known about the slot a value fills. `flag` is
/// the enclosing field's derivability flag (`Some` for a struct field,
/// `None` for a tuple field or a collection element, which carry no
/// `InitValue`).
struct Slot<'a> {
    init_value: Option<&'a str>,
    declared_init: Option<&'a str>,
    flag: Option<bool>,
}

impl Slot<'_> {
    const BARE: Slot<'static> = Slot {
        init_value: None,
        declared_init: None,
        flag: None,
    };
}

fn type_def_init(name: &str, td: &v2::TypeDef) -> Option<String> {
    let init = td.init.as_ref()?;
    if !init.derivable {
        return None;
    }
    let value = scalar_init_value(backing_scalar(td), is_wide(td), init.value.as_deref())?;
    Some(format!("{value} as {name}"))
}

/// The unbranded init value for a scalar backing. A derivable numeric or
/// unit type carries its init text (`"0"` or `min`). A string type with a
/// declared init emits that init verbatim; without one the derivable case
/// admits length 0 and defaults to the empty string (I1). A bytes type with
/// a declared init has no faithful literal form here, so it gets no init
/// rather than a wrong (empty) one; the derivable zero-length case defaults
/// to the empty `Uint8Array`. A 64-bit integer whose init text is not a
/// plain integer has no bigint literal form and gets no init.
fn scalar_init_value(backing: ScalarBacking, wide: bool, value: Option<&str>) -> Option<String> {
    match backing {
        ScalarBacking::Float => Some(float_literal(value?)),
        ScalarBacking::Integer if wide => {
            let value = value?;
            is_integer_form(value).then(|| format!("{value}n"))
        }
        ScalarBacking::Integer => {
            // Defense in depth: a value past 2^53 under a narrow width would
            // round as a `number` literal, so the init is denied instead.
            let value = value?;
            fits_safe_number(value).then(|| value.to_string())
        }
        ScalarBacking::Boolean => Some(bool_literal(value.unwrap_or("false")).to_string()),
        ScalarBacking::String => match value {
            Some(text) if !text.is_empty() => Some(ts_string(text)),
            _ => Some("''".to_string()),
        },
        ScalarBacking::Bytes => match value {
            Some(text) if !text.is_empty() => None,
            _ => Some("new Uint8Array(0)".to_string()),
        },
    }
}

/// The init object literal for a struct: one property per derivable field,
/// optional fields omitted (absence is the typl `?` init, typl §7.1).
fn struct_init(ctx: &Ctx, sd: &v2::StructDef) -> Option<String> {
    let mut props = Vec::new();
    for member in &sd.members {
        if let Some(v2::struct_member::Member::Field(field)) = &member.member {
            let ft = field.r#type.as_ref()?;
            if ft.optional {
                continue;
            }
            let init = field.init.as_ref();
            let slot = Slot {
                init_value: init.and_then(|i| i.value.as_deref()),
                declared_init: field.declared_init.as_deref(),
                flag: Some(init.map(|i| i.derivable).unwrap_or(false)),
            };
            let expr = slot_init(ctx, ft, &slot)?;
            props.push(format!("    {}: {expr},\n", field.name));
        }
    }
    if props.is_empty() {
        return Some("{}".to_string());
    }
    Some(format!("{{\n{}  }}", props.concat()))
}

fn slot_init(ctx: &Ctx, ft: &v2::FieldType, slot: &Slot) -> Option<String> {
    if ft.optional {
        // Only reachable in nested positions (struct and tuple fields omit
        // the property instead); absence renders as `undefined`.
        return Some("undefined".to_string());
    }
    match &ft.kind {
        Some(v2::field_type::Kind::Named(reference)) => named_init(ctx, reference, slot),
        Some(v2::field_type::Kind::Primitive(prim)) => primitive_init(*prim, slot),
        Some(v2::field_type::Kind::InlineScalar(td)) => {
            if slot.flag == Some(false) {
                None
            } else {
                scalar_init_value(backing_scalar(td), is_wide(td), slot.init_value)
            }
        }
        Some(v2::field_type::Kind::Tuple(tuple)) => tuple_init(ctx, tuple),
        Some(v2::field_type::Kind::Array(array)) => array_init(ctx, array, slot),
        Some(v2::field_type::Kind::Map(map)) => map_init(ctx, map, slot),
        Some(v2::field_type::Kind::Stream(_)) | None => None,
    }
}

fn named_init(ctx: &Ctx, reference: &str, slot: &Slot) -> Option<String> {
    if let Some((package, name)) = reference.rsplit_once('.') {
        // Cross-package: the remote backing is not resolvable here. A
        // declared init on such a field cannot be faithfully cast without
        // that backing, so the containing type gets no init at all (I2).
        // Without a declared init, trust the enclosing field's flag and call
        // the referenced package's generated init function.
        if slot.declared_init.is_some() {
            None
        } else if slot.flag == Some(true) {
            Some(ctx.cross_package_init_call(package, name))
        } else {
            None
        }
    } else if let Some(decl) = ctx.lookup(reference) {
        match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => {
                type_def_init(reference, td)?;
                if let Some(declared) = slot.declared_init {
                    let value = scalar_init_value(backing_scalar(td), is_wide(td), Some(declared))?;
                    Some(format!("{value} as {reference}"))
                } else {
                    Some(format!("init{reference}()"))
                }
            }
            // Same-package composite, enum, or enum set: recurse rather than
            // trust a one-level flag.
            _ => named_same_package_init(ctx, reference),
        }
    } else {
        None
    }
}

fn named_same_package_init(ctx: &Ctx, reference: &str) -> Option<String> {
    let decl = ctx.lookup(reference)?;
    // Guard the one recursion point into a same-package declaration's init.
    // A cyclic composite (`struct S { next: S }`) is TYPL-206 upstream, but
    // the backend must not trust that gate: on a cycle it denies an init
    // rather than recurse forever and overflow the stack (C1b).
    if !ctx.enter_init(reference) {
        return None;
    }
    let derivable = decl_init_expr(ctx, decl).is_some();
    ctx.leave_init(reference);
    derivable.then(|| format!("init{reference}()"))
}

fn primitive_init(prim: i32, slot: &Slot) -> Option<String> {
    match v2::PrimitiveType::try_from(prim).unwrap_or(v2::PrimitiveType::Unspecified) {
        // The bare `integer` maps to `bigint` (no width), so its init is an
        // `n`-suffixed literal.
        v2::PrimitiveType::Integer => {
            let value = slot.init_value.unwrap_or("0");
            is_integer_form(value).then(|| format!("{value}n"))
        }
        v2::PrimitiveType::Float => Some(float_literal(slot.init_value.unwrap_or("0"))),
        v2::PrimitiveType::Boolean => {
            Some(bool_literal(slot.init_value.unwrap_or("false")).to_string())
        }
        v2::PrimitiveType::String if slot.flag != Some(false) => Some("''".to_string()),
        v2::PrimitiveType::Bytes if slot.flag != Some(false) => {
            Some("new Uint8Array(0)".to_string())
        }
        _ => None,
    }
}

/// The inline init object for a tuple (typl §11): tuple fields carry no
/// `InitValue`, so each slot is bare — a cross-package tuple field cannot be
/// resolved and makes the tuple non-constructible.
fn tuple_init(ctx: &Ctx, tuple: &v2::TupleType) -> Option<String> {
    if tuple.fields.is_empty() {
        return Some("{}".to_string());
    }
    let mut props = Vec::with_capacity(tuple.fields.len());
    for field in &tuple.fields {
        let ft = field.r#type.as_ref()?;
        if ft.optional {
            continue;
        }
        let expr = slot_init(ctx, ft, &Slot::BARE)?;
        props.push(format!("{}: {expr}", field.name));
    }
    Some(format!("{{ {} }}", props.join(", ")))
}

/// The init of an array slot: the min-bound count of element inits
/// (typl §5.8) — `[]` when the minimum is zero.
fn array_init(ctx: &Ctx, array: &v2::ArrayType, slot: &Slot) -> Option<String> {
    // A fixed array has min == max, so `min` is the count in both the fixed
    // and the bounded form.
    let count = array.min;
    if count == 0 {
        return Some("[]".to_string());
    }
    let element_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
    };
    let element = slot_init(ctx, array.element.as_deref()?, &element_slot)?;
    Some(array_from(count, &element))
}

/// The init of a map slot: the min-bound count of entry inits (typl §5.8) —
/// `[]` when the minimum is zero.
fn map_init(ctx: &Ctx, map: &v2::MapType, slot: &Slot) -> Option<String> {
    if map.min == 0 {
        return Some("[]".to_string());
    }
    let entry_slot = Slot {
        init_value: None,
        declared_init: None,
        flag: slot.flag,
    };
    let key = slot_init(ctx, map.key.as_deref()?, &entry_slot)?;
    let value = slot_init(ctx, map.value.as_deref()?, &entry_slot)?;
    Some(array_from(map.min, &format!("[{key}, {value}] as const")))
}

/// `Array.from({ length: n }, () => body)` — the one place this backend emits
/// an arrow function **expression**, so the rule below cannot be missed by a
/// later call site. Arrow **types** are a separate matter: [`interact`] writes
/// two of them into the prelude's handle interfaces
/// (`subscribe(fn: (value: T, …) => void): () => void`), which are type
/// annotations with no body and no parse ambiguity to resolve.
///
/// A concise arrow body that starts with `{` is parsed as a **block**, not as
/// an object literal: `() => { first: x }` is a block holding a labelled
/// statement, so the callback returns `undefined`, and `() => { first: x,
/// latest: y }` is a syntax error. Such a body is parenthesised (issue #177).
///
/// Of the init forms [`slot_init`] produces, only [`tuple_init`] renders as an
/// object literal, so today the array element form is the one that reaches the
/// rule — an array whose element is a tuple. The map entry form renders
/// `[k, v] as const` and is left unchanged. The check is on the emitted text
/// rather than on the element kind, so a later init form that renders as an
/// object literal is covered by construction.
fn array_from(count: u64, body: &str) -> String {
    let body = if body.starts_with('{') {
        format!("({body})")
    } else {
        body.to_string()
    };
    format!("Array.from({{ length: {count} }}, () => {body})")
}

fn enum_init(name: &str, ed: &v2::EnumDef) -> Option<String> {
    // The value 0 if declared, else the lowest declared value (typl §5.8).
    let chosen = ed
        .values
        .iter()
        .find(|value| value.value == 0)
        .or_else(|| ed.values.iter().min_by_key(|value| value.value))?;
    Some(format!("{name}.{}", chosen.name))
}

fn enum_set_init(name: &str, esd: &v2::EnumSetDef) -> String {
    // The empty set — no bits set (typl §5.8, §9).
    let wide = matches!(
        v2::IntWidth::try_from(esd.width),
        Ok(v2::IntWidth::U64 | v2::IntWidth::I64)
    );
    let zero = if wide { "0n" } else { "0" };
    format!("{zero} as {name}")
}

fn union_init(ctx: &Ctx, ud: &v2::UnionDef) -> Option<String> {
    // The first arm's init (typl §5.8). The arm references a named type.
    let first = ud.arms.first()?;
    let value = if first.type_ref.contains('.') {
        None
    } else {
        named_same_package_init(ctx, &first.type_ref)
    }?;
    Some(format!(
        "{{ kind: {kind}, value: {value} }}",
        kind = ts_string(&first.name)
    ))
}

// ---------------------------------------------------------------------------
// Attributes: docs, deprecation, visibility.
// ---------------------------------------------------------------------------

/// A JSDoc block from a CommonMark doc body and structured tags, indented by
/// `indent`; empty when there is nothing to say. A body and tags are
/// separated by a blank line, JSDoc convention.
fn jsdoc(indent: &str, body: &str, tags: &[String]) -> String {
    let body_lines: Vec<&str> = if body.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };
    if body_lines.is_empty() && tags.is_empty() {
        return String::new();
    }

    let mut out = format!("{indent}/**\n");
    for line in &body_lines {
        // `*/` inside a doc body would terminate the comment; escape it.
        let line = line.replace("*/", "*\\/");
        if line.is_empty() {
            out.push_str(&format!("{indent} *\n"));
        } else {
            out.push_str(&format!("{indent} * {line}\n"));
        }
    }
    if !body_lines.is_empty() && !tags.is_empty() {
        out.push_str(&format!("{indent} *\n"));
    }
    for tag in tags {
        out.push_str(&format!("{indent} * {tag}\n"));
    }
    out.push_str(&format!("{indent} */\n"));
    out
}

/// The structured JSDoc tags of a named scalar declaration: unit, range,
/// deprecation.
fn type_def_tags(decl: &v2::Decl, td: &v2::TypeDef) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(unit) = unit_tag(td) {
        tags.push(unit);
    }
    if let Some(range) = td.constraint.as_ref().and_then(range_tag) {
        tags.push(range);
    }
    tags.extend(deprecated_tags(decl.deprecated.as_deref()));
    tags
}

fn unit_tag(td: &v2::TypeDef) -> Option<String> {
    match td.backing.as_ref().and_then(|b| b.kind.as_ref()) {
        Some(v2::backing::Kind::Unit(unit)) => Some(format!("@unit {unit}")),
        _ => None,
    }
}

/// `@range [min..max step s]` in the typl surface spelling; absent when the
/// constraint carries no numeric bounds.
fn range_tag(constraint: &v2::Constraint) -> Option<String> {
    if constraint.min.is_none() && constraint.max.is_none() && constraint.step.is_none() {
        return None;
    }
    let mut text = format!(
        "@range [{}..{}",
        constraint.min.as_deref().unwrap_or(""),
        constraint.max.as_deref().unwrap_or("")
    );
    if let Some(step) = &constraint.step {
        text.push_str(&format!(" step {step}"));
    }
    text.push(']');
    Some(text)
}

/// `@bounds N` for a fixed collection, `@bounds min..max` for a bounded one
/// — the array and map bounds of a field's top-level type (typl §12).
fn bounds_tag(kind: &v2::field_type::Kind) -> Option<String> {
    let (min, max) = match kind {
        v2::field_type::Kind::Array(array) => (array.min, array.max),
        v2::field_type::Kind::Map(map) => (map.min, map.max),
        _ => return None,
    };
    if min == max {
        Some(format!("@bounds {max}"))
    } else {
        Some(format!("@bounds {min}..{max}"))
    }
}

/// `@deprecated` maps to the JSDoc tag; a present-but-empty reason (the
/// IR's `Some("")`) still emits the bare tag (typl §14.2).
fn deprecated_tags(reason: Option<&str>) -> Vec<String> {
    match reason {
        Some("") => vec!["@deprecated".to_string()],
        Some(reason) => vec![format!("@deprecated {reason}")],
        None => Vec::new(),
    }
}

/// `internal` maps to a module-local (non-exported) declaration — the
/// TypeScript package-private mechanism (ADR-0002 §8, ADR-0008 decision 7,
/// typl §3.3). The rule is per declaration, not per module: a package holding
/// one `internal` and one public declaration emits one module-local shape and
/// one exported shape.
pub(crate) fn export_kw(visibility: i32) -> &'static str {
    match v2::Visibility::try_from(visibility).unwrap_or(v2::Visibility::Unspecified) {
        v2::Visibility::Internal => "",
        _ => "export ",
    }
}

// ---------------------------------------------------------------------------
// Literals.
// ---------------------------------------------------------------------------

/// A float literal from a canonical decimal string: a decimal point is added
/// to an integer-form value so the emitted literal reads as a float.
fn float_literal(value: &str) -> String {
    if !value.contains('.') && !value.contains('e') && !value.contains('E') {
        format!("{value}.0")
    } else {
        value.to_string()
    }
}

/// An `n`-suffixed bigint literal, or [`GenerateError::Unrepresentable`]
/// when the canonical value is not a plain integer.
fn bigint_literal(name: &str, value: &str) -> Result<String, GenerateError> {
    if is_integer_form(value) {
        Ok(format!("{value}n"))
    } else {
        Err(GenerateError::Unrepresentable(format!(
            "constant {name}: value {value:?} is not an integer, but its type is \
             bigint-branded (64-bit width)"
        )))
    }
}

/// True when `value` is a plain decimal integer (an optional leading minus
/// and digits) — the only form a bigint literal admits.
fn is_integer_form(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// True when the canonical integer value fits TypeScript's exact `number`
/// range (|v| <= 2^53 - 1) — the Appendix D footnote applied to a known
/// value.
fn fits_safe_number(value: &str) -> bool {
    value
        .parse::<i128>()
        .is_ok_and(|v| v.unsigned_abs() <= u128::from(MAX_SAFE_INTEGER))
}

fn bool_literal(value: &str) -> &'static str {
    if value == "true" { "true" } else { "false" }
}

/// A single-quoted TypeScript string literal with the JS escapes applied.
fn ts_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests;
