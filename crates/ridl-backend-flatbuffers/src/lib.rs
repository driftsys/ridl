//! IR v2 package to a FlatBuffers schema (roadmap story E9.9, ADR-0013
//! decision 2).
//!
//! The second wire backend. The emit ceiling is two tiers — the typl surface
//! and the interaction identity table — and nothing above them. No
//! `rpc_service`, no reply carriers, no store.
//!
//! Six rules here differ from the proto3 backend and are not interchangeable
//! with it: a union is isolated in a wrapper table because a native union
//! owns two id slots; a struct is always emitted as a `table` because a
//! FlatBuffers `struct` fabricates a value after a compatible append; enum
//! values are scoped to their enum rather than to the namespace, so no value
//! prefixing is emitted and no zero member is synthesized into the
//! declaration; a table field whose enum declares no zero-valued member
//! takes `= null`, because FlatBuffers gives every table field a default and
//! cannot mark a scalar or enum field required in any case; a map becomes a
//! vector of generated entry tables with no `(key)` attribute, because
//! FlatBuffers has no map type and the attribute would oblige the producer
//! to sort a container typl §12.2 gives no ordering; and the name guard
//! models FlatBuffers' own scopes ([`Namespace`]), because proto3 scopes
//! enum values as namespace siblings and this target does not.

use std::collections::{BTreeSet, HashMap};

use ridl_ir::v2;

#[cfg(test)]
mod tests;

/// The generated FlatBuffers schema for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub fbs_source: String,
}

/// A failure to generate a schema from a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates the FlatBuffers schema for `package`, resolving foreign
/// references against `others` (ADR-0017 decision 1): a foreign named scalar
/// or enum set inlines exactly as a local one does — there is no declaration
/// to reference, so no include is ever emitted for one — while a foreign
/// struct, enum or union emits `include "<package>.fbs";` and is referenced
/// by its fully qualified `<package>.<Name>` ([`named_field_type`]). The
/// include block is collected into a `BTreeSet<String>` so it is
/// deterministic, and is written before the declarations are, into `body`,
/// so the set is complete before the include block is rendered — and the
/// include block itself is written before the `namespace` line, which is
/// what FlatBuffers requires.
pub fn generate_with(
    package: &v2::Package,
    others: &[&v2::Package],
) -> Result<Generated, GenerateError> {
    let packages = Packages { package, others };
    let mut includes: BTreeSet<String> = BTreeSet::new();
    // One namespace scope spans both walks, because the identity tables are
    // namespace-level enums the same as the declared types: a declared type
    // named `<Interface>Ordinal` and the generated identity table for
    // `<Interface>` collide, and only a scope that has seen both can refuse
    // it. Declared names are registered before any generated name, so a
    // refusal always names the declaration as the first claimant.
    let mut names = Namespace::types(&package.name);
    claim_declared_names(&mut names, package)?;

    let mut body = String::new();
    emit_structs(&mut body, packages, &mut names, &mut includes)?;
    emit_identity_tables(&mut body, package, &mut names)?;

    let mut out = String::new();
    for include in &includes {
        out.push_str(&format!("include \"{include}\";\n"));
    }
    out.push_str(&format!("namespace {};\n", package.name));
    out.push_str(&body);
    Ok(Generated { fbs_source: out })
}

/// The package being generated, plus every other package it might
/// cross-reference — both read-only for the whole declaration walk, so they
/// travel together as one bundle rather than as two positional parameters
/// repeated at every call site (mirrors `ridl-backend-proto`'s `Packages`).
#[derive(Clone, Copy)]
struct Packages<'a> {
    package: &'a v2::Package,
    others: &'a [&'a v2::Package],
}

/// Registers every declared name that becomes a FlatBuffers declaration: a
/// struct's table, an enum, and a union — whose wrapper table takes the
/// declared name ([`emit_union`]). A named scalar, enum set or constant
/// mints no FlatBuffers declaration ([`emit_structs`]), so their names stay
/// free for a generated name to take.
fn claim_declared_names(names: &mut Namespace, package: &v2::Package) -> Result<(), GenerateError> {
    for decl in &package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(_)) => {
                names.claim(&decl.name, &format!("struct `{}`", decl.name))?;
            }
            Some(v2::decl::Kind::EnumDef(_)) => {
                names.claim(&decl.name, &format!("enum `{}`", decl.name))?;
            }
            Some(v2::decl::Kind::UnionDef(_)) => {
                names.claim(&decl.name, &format!("union `{}`", decl.name))?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// One FlatBuffers name scope, tracking every name this backend emits into
/// it so a second claim on one name is refused before the target sees the
/// redefinition — ADR-0017 decision 4's totality obligation, modelled on
/// this target's scopes. They are not proto3's: `ridl-backend-proto`'s
/// `SymbolScope` registers enum values in its package scope, because proto3
/// scopes them as siblings of their enum, and lifting it here would
/// over-refuse. Verified against `flatc`, FlatBuffers gives this backend
/// three scopes:
///
/// - the **namespace** — every table, struct, enum and union name shares one
///   space (`flatc`: "datatype already exists"). Declared names register
///   first ([`claim_declared_names`]), then every generated name: union
///   declarations ([`emit_union`]), entry and tuple tables
///   ([`emit_induced_tables`]), and identity-table enums
///   ([`emit_identity_tables`]).
/// - one **table's body** — its field names.
/// - one **enum's body** — its value names. Two enums may each declare a
///   value named `OK`, which is where FlatBuffers differs from proto3 and
///   why no value is prefixed ([`emit_enum`]).
struct Namespace {
    /// Named in the refusal: ``namespace `veh.common` ``, ``table `Payload` ``
    /// or ``enum `Gear` ``.
    scope: String,
    /// Each projected name, to the plain description of the construct that
    /// claimed it — quoted back when a second claim on the name is refused.
    claimed: HashMap<String, String>,
}

impl Namespace {
    /// The namespace scope: every type name in one package's schema.
    fn types(package: &str) -> Self {
        Self {
            scope: format!("namespace `{package}`"),
            claimed: HashMap::new(),
        }
    }

    /// One table's field-name scope.
    fn fields(table: &str) -> Self {
        Self {
            scope: format!("table `{table}`"),
            claimed: HashMap::new(),
        }
    }

    /// One enum's value-name scope.
    fn values(enum_name: &str) -> Self {
        Self {
            scope: format!("enum `{enum_name}`"),
            claimed: HashMap::new(),
        }
    }

    /// One union's member-name scope — the alias names, which the target
    /// scopes like an enum's values ([`emit_union`]).
    fn members(union_name: &str) -> Self {
        Self {
            scope: format!("union `{union_name}`"),
            claimed: HashMap::new(),
        }
    }

    /// Registers the FlatBuffers name that `source` — a plain description
    /// such as ``struct `Reading` `` — is about to emit. The second claim on
    /// one name is refused with an error naming both claimants.
    fn claim(&mut self, name: &str, source: &str) -> Result<(), GenerateError> {
        if let Some(previous) = self.claimed.insert(name.to_string(), source.to_string()) {
            return Err(GenerateError {
                message: format!(
                    "`{name}` is claimed twice in {}: once by {previous}, and again by \
                     {source}. FlatBuffers rejects a name declared twice in one scope, so \
                     rename one of the two so their projected names differ.",
                    self.scope
                ),
            });
        }
        Ok(())
    }
}

/// A dotted address becomes one CamelCase FlatBuffers identifier:
/// `corpus.baseline.hvac` gives `CorpusBaselineHvac`. A named interface has
/// no dots, so its name passes through unchanged.
fn type_name(dotted: &str) -> String {
    dotted
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// SCREAMING_SNAKE of a ridl name, built on the pinned transform so there is
/// exactly one case algorithm in the toolchain (ADR-0016 decision 2).
fn screaming_snake_case(name: &str) -> String {
    ridl_ir::name::snake_case(name).to_uppercase()
}

/// Tier 1 (ADR-0013 decision 2): walks every top-level declaration, emitting
/// a `table` for each struct ([`emit_struct`]), an `enum` for each typl enum
/// ([`emit_enum`]), and a union declaration plus its wrapper table for each
/// union ([`emit_union`]). A named scalar and an enum set inline at each use
/// site instead of becoming a declaration of their own, the same as in
/// `ridl-backend-proto`'s tier 1, and a constant is never emitted (ADR-0013
/// decision 5) — so a `TypeDef`, `EnumSetDef` or `ConstDef` declaration
/// simply contributes nothing here. The walk collects the tables a container
/// field induces — a map's entry table and a tuple's positional table —
/// which are emitted after the declarations that reached them
/// ([`emit_induced_tables`]).
fn emit_structs(
    out: &mut String,
    packages: Packages,
    names: &mut Namespace,
    includes: &mut BTreeSet<String>,
) -> Result<(), GenerateError> {
    let mut induced: Vec<Induced> = Vec::new();
    for decl in &packages.package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => {
                emit_struct(out, packages, &decl.name, def, &mut induced, includes)?;
            }
            Some(v2::decl::Kind::EnumDef(def)) => emit_enum(out, &decl.name, def)?,
            Some(v2::decl::Kind::UnionDef(def)) => {
                emit_union(out, packages.package, &decl.name, def, names)?;
            }
            _ => {}
        }
    }
    emit_induced_tables(out, packages, &mut induced, names, includes)?;
    Ok(())
}

/// One enum as one FlatBuffers `enum` (typl §8): values keep their explicitly
/// assigned numbers, unprefixed and in declaration order, because FlatBuffers
/// scopes an enum's values inside the enum rather than as siblings of it —
/// two typl enums in one package may each declare a value named `OK` without
/// conflict, unlike proto3. A `Reserved` retired value ([`v2::EnumDef::reserved`])
/// contributes nothing: FlatBuffers has no `reserved` construct for an enum
/// declaration.
///
/// The underlying type is always `long`. `EnumValue.value` is `int64` in the
/// IR, and narrowing to the smallest type that fits the declared values would
/// make the underlying type a function of those values — a later value could
/// then widen it, which is a wire change under an otherwise compatible edit.
///
/// Each value name is claimed in the enum's own scope ([`Namespace`]): typl
/// source cannot declare one name twice, but IR handed to this backend
/// directly can, and the target rejects the repeat.
fn emit_enum(out: &mut String, name: &str, def: &v2::EnumDef) -> Result<(), GenerateError> {
    let mut values = Namespace::values(name);
    out.push_str(&format!("\nenum {name} : long {{\n"));
    for value in &def.values {
        values.claim(
            &value.name,
            &format!("value `{}` of enum `{name}`", value.name),
        )?;
        out.push_str(&format!("  {} = {},\n", value.name, value.value));
    }
    out.push_str("}\n");
    Ok(())
}

/// One union as two declarations (typl §10): `union <Name>Union` listing the
/// arms, and the wrapper `table <Name>` holding it — the declared name goes
/// to the wrapper, because the wrapper is what a field position references
/// ([`named_field_type`]).
///
/// The wrapper exists because a native union owns TWO id slots: a union
/// field declared `(id: N)` puts its hidden `_type` discriminant at `N - 1`
/// and the value at `N`. A native union placed in an ordinal-owned slot
/// would therefore shift every later field's id, and `flatc` refuses the
/// schema ("field id's must be consecutive from 0"). The wrapper takes one
/// slot in its parent and keeps the union's two inside its own id space,
/// where contiguity binds nothing else — so the value field is `(id: 1)`,
/// not 0, because the implicit `_type` takes 0 (verified against `flatc`
/// 25.12.19).
///
/// Every arm is emitted with FlatBuffers union alias syntax —
/// `member: Type` — accepted by both `flatc` 25.12.19 and `planus`, so the
/// form stays inside the validity oracle. The member name is the typl arm
/// name through the pinned transform, not the type name, which keeps two
/// arms sharing one type distinct — a union listing one type twice without
/// aliases is refused by the target ("enum value already exists"). The
/// member names are claimed in the union's own scope ([`Namespace`]),
/// because the target scopes them like an enum's values and two arm names
/// colliding under the transform would emit that same redefinition —
/// ADR-0017 decision 5 places this guard in the backend. An arm whose
/// target is not a table is refused before any of this
/// ([`check_union_arm_target`]).
///
/// A hand-rolled discriminant-plus-arms table is not an alternative: it
/// cannot hold typl §10's guarantee that exactly one arm is active, because
/// nothing ties the discriminant to the arm that is set. A retired arm
/// ([`v2::UnionDef::reserved`]) contributes nothing to the union list, the
/// same as a retired value in [`emit_enum`].
fn emit_union(
    out: &mut String,
    package: &v2::Package,
    name: &str,
    def: &v2::UnionDef,
    names: &mut Namespace,
) -> Result<(), GenerateError> {
    let union_name = format!("{name}Union");
    names.claim(
        &union_name,
        &format!("the union declaration generated for union `{name}`"),
    )?;
    let mut members = Namespace::members(&union_name);
    let mut arms: Vec<String> = Vec::new();
    for arm in &def.arms {
        check_union_arm_target(package, name, arm)?;
        let member = ridl_ir::name::snake_case(&arm.name);
        members.claim(&member, &format!("arm `{}` of union `{name}`", arm.name))?;
        arms.push(format!("{member}: {}", arm.type_ref));
    }
    out.push_str(&format!("\nunion {union_name} {{ {} }}\n", arms.join(", ")));
    out.push_str(&format!(
        "\ntable {name} {{\n  value: {union_name} (id: 1);\n}}\n"
    ));
    Ok(())
}

/// Refuses a union arm whose target FlatBuffers cannot carry: a union
/// member must be a table in this projection. A named scalar and an enum
/// set inline at their use sites ([`named_field_type`]), so neither has a
/// declaration for the union list to reference at all, and an enum has one
/// but is not a table. A struct arm references its table, and a union arm
/// references the target union's wrapper table — both are tables, so both
/// pass. Resolution is same-package only, the standing caveat of this
/// backend; an unresolved reference is emitted as written.
fn check_union_arm_target(
    package: &v2::Package,
    union_name: &str,
    arm: &v2::UnionArm,
) -> Result<(), GenerateError> {
    let Some(decl) = package.decls.iter().find(|decl| decl.name == arm.type_ref) else {
        return Ok(());
    };
    let refused = match &decl.kind {
        Some(v2::decl::Kind::TypeDef(_)) => {
            "a named scalar, which inlines to a bare scalar and has no declaration to reference"
        }
        Some(v2::decl::Kind::EnumDef(_)) => "an enum, which is not a table",
        Some(v2::decl::Kind::EnumSetDef(_)) => {
            "an enum set, which inlines to an integer and has no declaration to reference"
        }
        _ => return Ok(()),
    };
    Err(GenerateError {
        message: format!(
            "union `{union_name}` arm `{}` is typed by `{}`, {refused} — a FlatBuffers \
             union member must be a table, so the arm is refused rather than emitted as \
             a schema the target rejects.",
            arm.name, arm.type_ref
        ),
    })
}

/// One struct as one FlatBuffers `table`, one field per member, its `id` the
/// typl ordinal minus one — FlatBuffers ids start at 0, typl ordinals at 1
/// (typl §7.4). A field name goes through the pinned transform
/// ([`ridl_ir::name::snake_case`]). A `Reserved` member holds its ordinal's
/// slot as a placeholder field marked `deprecated`, so a later id never moves
/// onto it; the placeholder's declared type is inert, because a deprecated
/// field generates no accessor, so `ubyte` is used regardless of what was
/// retired.
///
/// **`def.fixed_layout` is read nowhere in this function.** typl Appendix D
/// permits a struct whose fields are all fixed-width and non-optional to be
/// emitted as a FlatBuffers `struct` instead — inline, zero indirection.
/// This backend never takes that allowance: appending a struct field is a
/// compatible change in typl, and a FlatBuffers `struct` reads past what an
/// older writer wrote after such an append, fabricating a value from padding,
/// rather than reporting the new field absent — which makes ADR-0016
/// decision 6 property 3 unsatisfiable, and silently. A `table` reports the
/// field correctly absent, so every struct is emitted as one, whatever
/// `fixed_layout` says.
///
/// A field's projected name is claimed in the table's own scope
/// ([`Namespace`]): two source names colliding under the pinned transform
/// would emit a field redefinition the target rejects. The retired-slot
/// placeholders claim their names too — a live field spelling
/// `reserved_<N>` after the transform is the same redefinition. A table a
/// field's type induces — a map's entry table, a tuple's positional table —
/// is collected into `induced` under the name `<Owner><Field>` in CamelCase,
/// and emitted after the declaration walk ([`emit_induced_tables`]).
fn emit_struct(
    out: &mut String,
    packages: Packages,
    name: &str,
    def: &v2::StructDef,
    induced: &mut Vec<Induced>,
    includes: &mut BTreeSet<String>,
) -> Result<(), GenerateError> {
    let mut fields = Namespace::fields(name);
    out.push_str(&format!("\ntable {name} {{\n"));
    for member in &def.members {
        match &member.member {
            Some(v2::struct_member::Member::Field(field)) => {
                let id = member_id(name, field.ordinal)?;
                let field_name = ridl_ir::name::snake_case(&field.name);
                fields.claim(
                    &field_name,
                    &format!("field `{}` of struct `{name}`", field.name),
                )?;
                let ty = field.r#type.as_ref().ok_or_else(|| GenerateError {
                    message: format!("{name}.{} carries no type in the IR.", field.name),
                })?;
                let hint = format!("{name}{}", type_name(&field.name));
                let (type_text, needs_null_default, comment) =
                    resolve_field_type(packages, name, &field.name, &hint, ty, induced, includes)?;
                push_field(
                    out,
                    &field_name,
                    &type_text,
                    needs_null_default,
                    comment,
                    id,
                );
            }
            Some(v2::struct_member::Member::Reserved(reserved)) => {
                let id = member_id(name, reserved.ordinal)?;
                let placeholder = format!("reserved_{}", reserved.ordinal);
                fields.claim(
                    &placeholder,
                    &format!(
                        "the placeholder holding retired ordinal {} of struct `{name}`",
                        reserved.ordinal
                    ),
                )?;
                out.push_str(&format!("  {placeholder}: ubyte (id: {id}, deprecated);\n"));
            }
            None => {}
        }
    }
    out.push_str("}\n");
    Ok(())
}

/// One table field line: the constraint comment on its own line above when
/// the resolved type carries one, then `name: type (id: N);` with `= null`
/// between the type and the id clause when the type calls for it. Shared by
/// the declared tables ([`emit_struct`]) and the generated ones
/// ([`emit_tuple_table`], [`emit_entry_table`]), whose fields are ordinary
/// table fields.
fn push_field(
    out: &mut String,
    field_name: &str,
    type_text: &str,
    needs_null_default: bool,
    comment: Option<String>,
    id: u32,
) {
    if let Some(comment) = comment {
        out.push_str(&format!("  {comment}\n"));
    }
    // `flatc` refuses a field whose implicit default of 0 is not a member of
    // its enum. This applies whether or not the typl field is optional:
    // FlatBuffers cannot mark a scalar or enum field `required` in any case,
    // so `= null` is the rendering that never fabricates a reading.
    let default_clause = if needs_null_default { " = null" } else { "" };
    out.push_str(&format!(
        "  {field_name}: {type_text}{default_clause} (id: {id});\n"
    ));
}

/// The FlatBuffers `id` for one struct member: the typl ordinal minus one.
/// Refused rather than subtracted with a wrapping or panicking underflow if
/// the IR ever carries ordinal 0 — an ordinal typl itself never assigns
/// (typl §7.4), so this is defensive against malformed IR, not a case
/// reachable through the compiler.
fn member_id(owner: &str, ordinal: u32) -> Result<u32, GenerateError> {
    ordinal.checked_sub(1).ok_or_else(|| GenerateError {
        message: format!(
            "`{owner}` carries a struct member with ordinal 0, which FlatBuffers ids cannot \
             represent — typl ordinals start at 1 (typl §7.4)."
        ),
    })
}

/// The FlatBuffers type at one field position — tier 1's typl surface: a
/// bare primitive resolves directly ([`fbs_primitive`]); a named-type
/// reference resolves through [`named_field_type`]; an array is `[T]`; a map
/// is a vector of a generated entry table ([`emit_entry_table`]); and a
/// tuple induces a positional table ([`emit_tuple_table`]). A stream is
/// refused: tier 1 covers the typl surface only (ADR-0013 decision 2).
///
/// `hint` is the CamelCase name a table induced at this exact position would
/// take: `<OwnerType><FieldName>` at the field itself, extended with
/// `Element` for an array element, `Key`/`Value` for a map's positions and
/// `Field<N>` for a tuple field — so a generated name always spells the path
/// that reached it. An induced table is pushed onto `induced` and emitted
/// after the declaration walk ([`emit_induced_tables`]).
///
/// The middle element of the result is whether the field needs an explicit
/// `= null` default (only ever true for an enum reference — see
/// [`push_field`]). At a vector-element position it is dropped, never
/// forwarded: only a table field carries a default in FlatBuffers, and a
/// vector element must not inherit one.
fn resolve_field_type(
    packages: Packages,
    owner: &str,
    field_name: &str,
    hint: &str,
    ty: &v2::FieldType,
    induced: &mut Vec<Induced>,
    includes: &mut BTreeSet<String>,
) -> Result<(String, bool, Option<String>), GenerateError> {
    match ty.kind.as_ref() {
        Some(v2::field_type::Kind::Primitive(primitive)) => {
            Ok((fbs_primitive(*primitive).to_string(), false, None))
        }
        Some(v2::field_type::Kind::Named(reference)) => {
            named_field_type(packages, owner, field_name, reference, includes)
        }
        Some(v2::field_type::Kind::Array(array)) => {
            let element = array.element.as_ref().ok_or_else(|| GenerateError {
                message: format!(
                    "{owner}.{field_name} declares an array with no element type in the IR."
                ),
            })?;
            // FlatBuffers has no vector of vectors, and both of these would
            // emit one: a nested array directly, and a map because a map is
            // itself projected as a vector of entry tables. Checked here so
            // neither rejection reaches the target as a schema it must
            // refuse (ADR-0017 decision 4's totality obligation).
            match element.kind.as_ref() {
                Some(v2::field_type::Kind::Array(_)) => {
                    return Err(GenerateError {
                        message: format!(
                            "{owner}.{field_name} is an array of arrays, which FlatBuffers \
                             cannot carry — a vector's element cannot itself be a vector. \
                             Wrap the inner array in a struct."
                        ),
                    });
                }
                Some(v2::field_type::Kind::Map(_)) => {
                    return Err(GenerateError {
                        message: format!(
                            "{owner}.{field_name} is an array of maps, which FlatBuffers \
                             cannot carry — a map is projected as a vector of entry tables, \
                             and a vector's element cannot itself be a vector. Wrap the map \
                             in a struct."
                        ),
                    });
                }
                _ => {}
            }
            // The element's `= null` marker is dropped, never forwarded: a
            // vector element carries no per-element default — only a table
            // field does.
            let (element_text, _needs_null_default, comment) = resolve_field_type(
                packages,
                owner,
                field_name,
                &format!("{hint}Element"),
                element,
                induced,
                includes,
            )?;
            // The kind checks above cannot see an element that *resolves*
            // to a vector: `bytes` — bare or behind a named scalar — maps
            // to `[ubyte]` ([`fbs_primitive`], [`fbs_scalar`]), so the
            // resolved text is checked too.
            if element_text.starts_with('[') {
                return Err(GenerateError {
                    message: format!(
                        "{owner}.{field_name} is an array whose element resolves to \
                         `{element_text}`, itself a FlatBuffers vector — a vector's \
                         element cannot be a vector. Wrap the element in a struct."
                    ),
                });
            }
            Ok((format!("[{element_text}]"), false, comment))
        }
        Some(v2::field_type::Kind::Map(map)) => {
            let entry_name = format!("{hint}Entry");
            induced.push(Induced {
                name: entry_name.clone(),
                kind: InducedKind::Entry(map.as_ref().clone()),
            });
            Ok((format!("[{entry_name}]"), false, None))
        }
        Some(v2::field_type::Kind::Tuple(tuple)) => {
            induced.push(Induced {
                name: hint.to_string(),
                kind: InducedKind::Tuple(tuple.clone()),
            });
            Ok((hint.to_string(), false, None))
        }
        _ => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} uses a field type this tier does not project yet."
            ),
        }),
    }
}

/// A resolved named-type reference at a field position. A named scalar
/// inlines to its FlatBuffers scalar and an enum set inlines to an integer —
/// **whether the reference is local or foreign** (ADR-0017 decision 1),
/// because neither ever becomes a declaration of its own for another file to
/// include ([`fbs_scalar`], [`constraint_comment`], [`enum_set_field_type`]).
/// `ridl.std` is not a special case of this: every one of its members is a
/// named scalar (typl reference Appendix A), so it always takes this same
/// inlining path.
///
/// A struct, enum or union reference names the table, enum or wrapper table
/// it becomes — a struct as its table ([`emit_struct`]), an enum as its own
/// declared name ([`emit_enum`]), a union as its wrapper table's declared
/// name ([`emit_union`]) — and each of those three is qualified
/// `pkg.Name` when foreign, with `includes` gaining `pkg.fbs`
/// ([`qualified_type_name`]): they are the only typl kinds this backend gives
/// a declaration of their own, so they are the only case an include is
/// needed. An enum reference also reports whether the field needs `= null`:
/// `flatc` requires every table field to carry a default, and refuses one
/// whose implicit default of 0 is not a member of the referenced enum — so
/// this is true exactly when the enum declares no zero-valued member.
///
/// A reference is cross-package exactly when it carries a `.` — the IR's
/// canonical form gives a cross-package reference as the fully qualified
/// `pkg.Name` and a same-package one as the bare `Name`, never an include
/// alias (`ridl_ir::v2::referenced_packages`'s doc comment). A same-package
/// reference is looked up in `packages.package`; a cross-package one is
/// looked up in `packages.others`, the other packages this backend was
/// given — this resolution has to happen before the inline-or-qualify
/// decision above either way, because that decision depends on the
/// referenced declaration's own kind. A foreign reference `others` cannot
/// resolve — no package by that name is present, or it holds no declaration
/// by that name — is refused rather than emitted as a name `flatc` would then
/// fail to resolve.
///
/// Any other declaration kind is refused: only a named scalar, struct, enum,
/// enum set or union is projected from a field position yet.
fn named_field_type(
    packages: Packages,
    owner: &str,
    field_name: &str,
    reference: &str,
    includes: &mut BTreeSet<String>,
) -> Result<(String, bool, Option<String>), GenerateError> {
    let (decl, foreign_package) = match reference.rsplit_once('.') {
        Some((referenced_package, member)) => {
            let resolved = packages
                .others
                .iter()
                .find(|candidate| candidate.name == referenced_package)
                .and_then(|candidate| candidate.decls.iter().find(|decl| decl.name == member));
            let Some(decl) = resolved else {
                return Err(GenerateError {
                    message: format!(
                        "{owner}.{field_name} references `{reference}`, which cannot be \
                         resolved — no package `{referenced_package}` with a declaration \
                         named `{member}` was given to this backend."
                    ),
                });
            };
            (decl, Some(referenced_package))
        }
        None => {
            let Some(decl) = packages
                .package
                .decls
                .iter()
                .find(|decl| decl.name == *reference)
            else {
                return Err(GenerateError {
                    message: format!(
                        "{owner}.{field_name} references `{reference}`, which is not a \
                         declaration of this package."
                    ),
                });
            };
            (decl, None)
        }
    };
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => Ok((
            fbs_scalar(td).to_string(),
            false,
            Some(constraint_comment(reference, td)),
        )),
        Some(v2::decl::Kind::StructDef(_)) => Ok((
            qualified_type_name(decl, foreign_package, includes),
            false,
            None,
        )),
        Some(v2::decl::Kind::EnumDef(def)) => {
            let zero_declared = def.values.iter().any(|value| value.value == 0);
            Ok((
                qualified_type_name(decl, foreign_package, includes),
                !zero_declared,
                None,
            ))
        }
        Some(v2::decl::Kind::EnumSetDef(esd)) => {
            let (scalar, comment) = enum_set_field_type(esd);
            Ok((scalar, false, comment))
        }
        Some(v2::decl::Kind::UnionDef(_)) => Ok((
            qualified_type_name(decl, foreign_package, includes),
            false,
            None,
        )),
        _ => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} references `{reference}`, a declaration kind this \
                 tier does not project yet — only a named scalar, struct, enum, enum set or \
                 union may be referenced from a field position here."
            ),
        }),
    }
}

/// The FlatBuffers name of a resolved struct, enum or union reference: the
/// bare declaration name for a same-package reference, or `pkg.Name` for a
/// foreign one, with `includes` gaining that package's
/// `include "pkg.fbs";` line (mirrors `ridl-backend-proto`'s
/// `qualified_message_name`). This only ever names the declaration — a
/// constraint comment and the `= null` default decision are each the calling
/// arm's own concern in [`named_field_type`].
fn qualified_type_name(
    decl: &v2::Decl,
    foreign_package: Option<&str>,
    includes: &mut BTreeSet<String>,
) -> String {
    match foreign_package {
        Some(referenced_package) => {
            includes.insert(format!("{referenced_package}.fbs"));
            format!("{referenced_package}.{}", decl.name)
        }
        None => decl.name.clone(),
    }
}

/// The FlatBuffers scalar and use-site comment for an enum set (mirrors
/// `ridl-backend-proto`'s `enum_set_field_type`). A FlatBuffers enum field
/// holds one value, and an enum set is a combination of bits, so it gains no
/// declaration of its own — like a named scalar, it resolves at each use
/// site instead. The scalar is [`fbs_scalar`] at the enum set's declared
/// width; the bit names and positions become one comment line each, in the
/// form `LOW_FUEL = bit 0`. Emitting a FlatBuffers enum here instead would
/// imply a guarantee the target does not make (ADR-0013 decision 2): one
/// enum field holds one value, never a combination of bits.
fn enum_set_field_type(esd: &v2::EnumSetDef) -> (String, Option<String>) {
    let scalar = fbs_scalar(&v2::TypeDef {
        width: Some(v2::type_def::Width::IntWidth(esd.width)),
        ..Default::default()
    });
    let lines: Vec<String> = esd
        .bits
        .iter()
        .map(|bit| format!("// {} = bit {}", bit.name, bit.value))
        .collect();
    let comment = if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n  "))
    };
    (scalar.to_string(), comment)
}

/// A table a container field induces while walking the package — a tuple's
/// positional table or a map's entry table — emitted after the declaration
/// that reached it, since neither exists as a declaration in source. Matches
/// the worklist pattern `ridl-backend-proto`'s `InducedTuple` uses; the kind
/// travels along because this backend generates entry tables too, which
/// proto3's native `map<K, V>` never needs.
#[derive(Debug, Clone)]
struct Induced {
    /// The generated table name: the [`resolve_field_type`] `hint` for a
    /// tuple, or that hint plus `Entry` for a map.
    name: String,
    kind: InducedKind,
}

/// The container that induced a generated table, carrying the type to emit
/// it from — compared whole when two paths reach one name
/// ([`emit_induced_tables`]).
#[derive(Debug, Clone, PartialEq)]
enum InducedKind {
    Tuple(v2::TupleType),
    Entry(v2::MapType),
}

/// Emits every table the walk in [`emit_structs`] induced, named for the
/// path that reached it. The worklist is drained rather than iterated once:
/// emitting one induced table's fields can discover a container nested
/// inside it, which is appended to `induced` and picked up by a later pass
/// of this same loop.
///
/// Two different container types reaching one generated name is refused
/// rather than resolved by picking one: nothing upstream keeps two field
/// paths from mangling to the same CamelCase string, and there is no sound
/// way to choose between two different table shapes for one name. Identical
/// claims collapse to one emission. The generated name is also claimed in
/// the namespace scope, because nothing keeps a field path from mangling to
/// a name the package already declares — `ReadingBounds` induced at
/// `Reading.bounds` beside a declared `ReadingBounds` — and that is a
/// redefinition the target rejects ([`Namespace`]).
fn emit_induced_tables(
    out: &mut String,
    packages: Packages,
    induced: &mut Vec<Induced>,
    names: &mut Namespace,
    includes: &mut BTreeSet<String>,
) -> Result<(), GenerateError> {
    let mut seen: HashMap<String, InducedKind> = HashMap::new();
    let mut index = 0;
    while index < induced.len() {
        let item = induced[index].clone();
        index += 1;
        if let Some(previous) = seen.get(&item.name) {
            if *previous != item.kind {
                return Err(GenerateError {
                    message: format!(
                        "the generated table name `{}` is claimed by two different container \
                         types; a container generates a table named for the field path that \
                         reaches it, and two different paths spelled one name here — rename \
                         a field so they differ.",
                        item.name
                    ),
                });
            }
            continue;
        }
        seen.insert(item.name.clone(), item.kind.clone());
        match &item.kind {
            InducedKind::Tuple(tuple) => {
                names.claim(
                    &item.name,
                    "a table generated for a tuple, named for the field path that reaches it",
                )?;
                emit_tuple_table(out, packages, &item.name, tuple, induced, includes)?;
            }
            InducedKind::Entry(map) => {
                names.claim(
                    &item.name,
                    "an entry table generated for a map, named for the field path that \
                     reaches it",
                )?;
                emit_entry_table(out, packages, &item.name, map, induced, includes)?;
            }
        }
    }
    Ok(())
}

/// One induced tuple table: positional fields `field_1`, `field_2`, … with
/// ids from 0 (mirrors `ridl-backend-proto`'s `emit_induced_tuple`). A tuple
/// field is always named in typl source (typl §11), but positional access is
/// what a tuple actually offers, so the generated table uses the position
/// rather than carry the source name onto the wire.
fn emit_tuple_table(
    out: &mut String,
    packages: Packages,
    name: &str,
    tuple: &v2::TupleType,
    induced: &mut Vec<Induced>,
    includes: &mut BTreeSet<String>,
) -> Result<(), GenerateError> {
    out.push_str(&format!("\ntable {name} {{\n"));
    for (index, field) in tuple.fields.iter().enumerate() {
        let position = index + 1;
        let field_name = format!("field_{position}");
        let id = u32::try_from(index).map_err(|_| GenerateError {
            message: format!("{name} has more tuple fields than a FlatBuffers id can carry."),
        })?;
        let ty = field.r#type.as_ref().ok_or_else(|| GenerateError {
            message: format!("{name}.{field_name} carries no type in the IR."),
        })?;
        let hint = format!("{name}Field{position}");
        let (type_text, needs_null_default, comment) =
            resolve_field_type(packages, name, &field_name, &hint, ty, induced, includes)?;
        push_field(
            out,
            &field_name,
            &type_text,
            needs_null_default,
            comment,
            id,
        );
    }
    out.push_str("}\n");
    Ok(())
}

/// One entry table for a map field: `key` at id 0, `value` at id 1, both
/// ordinary table fields — a value typed by an enum with no zero member
/// takes `= null` here the same as anywhere else ([`push_field`]).
///
/// FlatBuffers has no map type, so a map is a vector of these (typl §12.2).
/// The `(key)` attribute is deliberately NOT emitted: it obliges the
/// producer to write the vector sorted and nothing checks that at read
/// time, while typl §12.2 gives a map no ordering at all — and `planus`
/// cannot parse the attribute, which would put the map path outside the
/// validity oracle.
fn emit_entry_table(
    out: &mut String,
    packages: Packages,
    name: &str,
    map: &v2::MapType,
    induced: &mut Vec<Induced>,
    includes: &mut BTreeSet<String>,
) -> Result<(), GenerateError> {
    let key = map.key.as_ref().ok_or_else(|| GenerateError {
        message: format!("{name} is generated for a map that carries no key type in the IR."),
    })?;
    let value = map.value.as_ref().ok_or_else(|| GenerateError {
        message: format!("{name} is generated for a map that carries no value type in the IR."),
    })?;
    out.push_str(&format!("\ntable {name} {{\n"));
    let (key_text, key_needs_null, key_comment) = resolve_field_type(
        packages,
        name,
        "key",
        &format!("{name}Key"),
        key,
        induced,
        includes,
    )?;
    push_field(out, "key", &key_text, key_needs_null, key_comment, 0);
    let (value_text, value_needs_null, value_comment) = resolve_field_type(
        packages,
        name,
        "value",
        &format!("{name}Value"),
        value,
        induced,
        includes,
    )?;
    push_field(
        out,
        "value",
        &value_text,
        value_needs_null,
        value_comment,
        1,
    );
    out.push_str("}\n");
    Ok(())
}

/// The FlatBuffers scalar for a direct primitive use at a field position. A
/// bare `integer` or `float` carries no derived width, so the full typl
/// domain is emitted: `long` and `double` (typl §4).
fn fbs_primitive(primitive: i32) -> &'static str {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => "bool",
        Ok(v2::PrimitiveType::Integer) => "long",
        Ok(v2::PrimitiveType::Float) => "double",
        Ok(v2::PrimitiveType::Bytes) => "[ubyte]",
        _ => "string",
    }
}

/// The FlatBuffers scalar for a resolved typl width (typl Appendix D).
/// Unlike proto3's varint encoding, FlatBuffers stores every scalar at its
/// declared byte width, so the full `uint8`..`uint64` palette is used rather
/// than widened to the nearest transport-native size — the narrow width is
/// real bytes saved on the wire, which is the reason to choose this target
/// (ADR-0013 decision 4).
fn fbs_scalar(td: &v2::TypeDef) -> &'static str {
    match &td.width {
        Some(v2::type_def::Width::IntWidth(width)) => match v2::IntWidth::try_from(*width) {
            Ok(v2::IntWidth::U8) => "ubyte",
            Ok(v2::IntWidth::U16) => "ushort",
            Ok(v2::IntWidth::U32) => "uint",
            Ok(v2::IntWidth::U64) => "ulong",
            Ok(v2::IntWidth::I8) => "byte",
            Ok(v2::IntWidth::I16) => "short",
            Ok(v2::IntWidth::I32) => "int",
            Ok(v2::IntWidth::I64) => "long",
            _ => "long",
        },
        Some(v2::type_def::Width::FloatWidth(width)) => match v2::FloatWidth::try_from(*width) {
            Ok(v2::FloatWidth::F32) => "float",
            _ => "double",
        },
        // No width table: boolean, string and bytes backings. A unit backing
        // implies the float primitive (typl §5.1), so its width is always
        // derived and never reaches this arm.
        None => match td
            .backing
            .as_ref()
            .and_then(|backing| backing.kind.as_ref())
        {
            Some(v2::backing::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive) {
                    Ok(v2::PrimitiveType::Boolean) => "bool",
                    Ok(v2::PrimitiveType::Bytes) => "[ubyte]",
                    _ => "string",
                }
            }
            _ => "string",
        },
    }
}

/// The constraint information FlatBuffers has no construct for, as a comment
/// on the line above the field it describes (design mirrors
/// `ridl-backend-proto`'s comment of the same purpose — the constraint text
/// itself does not depend on the target).
fn constraint_comment(declared: &str, td: &v2::TypeDef) -> String {
    let mut form = Vec::new();
    if let Some(v2::backing::Kind::Unit(unit)) = td.backing.as_ref().and_then(|b| b.kind.as_ref()) {
        form.push(unit.clone());
    }
    if let Some(constraint) = &td.constraint {
        let rendered = render_constraint(constraint);
        if !rendered.is_empty() {
            form.push(rendered);
        }
    }
    if form.is_empty() {
        format!("// {declared}")
    } else {
        format!("// {declared} — {}", form.join(" "))
    }
}

/// The source form of a scalar constraint (typl §5.2–§5.5): the range
/// bracket with its optional step, the length bracket, and the `match`
/// pattern. The strings are canonical text already (ADR-0007 decision 9), so
/// they are joined, never reformatted.
fn render_constraint(constraint: &v2::Constraint) -> String {
    let mut parts = Vec::new();
    if constraint.min.is_some() || constraint.max.is_some() || constraint.step.is_some() {
        let min = constraint.min.as_deref().unwrap_or("");
        let max = constraint.max.as_deref().unwrap_or("");
        let step = match &constraint.step {
            Some(step) => format!(" step {step}"),
            None => String::new(),
        };
        parts.push(format!("[{min}..{max}{step}]"));
    }
    match (constraint.len_min, constraint.len_max) {
        // A fixed `[N]` constraint has len_min == len_max == N.
        (Some(min), Some(max)) if min == max => parts.push(format!("[{min}]")),
        (None, None) => {}
        (min, max) => parts.push(format!(
            "[{}..{}]",
            min.map(|bound| bound.to_string()).unwrap_or_default(),
            max.map(|bound| bound.to_string()).unwrap_or_default()
        )),
    }
    // A pattern given by a named regex constant renders by that name, the
    // way it was written (typl §5.3, §6.2).
    if let Some(name) = &constraint.pattern_const {
        parts.push(format!("match {name}"));
    } else if let Some(pattern) = &constraint.pattern {
        parts.push(format!("match {pattern}"));
    }
    parts.join(" ")
}

/// Tier 2 (ADR-0013 decision 2): one enum per interface shape, interface-wide
/// and kind-blind, matching ridl §11's single ordinal sequence.
///
/// Two rules here differ from the proto3 backend's identity table.
/// FlatBuffers scopes an enum's values inside the enum, not as siblings of
/// it, so no zero member needs synthesizing and no value is prefixed with
/// the enum's own name — two enums may each declare a member named `OK`
/// without conflict. And a retired ordinal (a `Reserved` tombstone)
/// contributes no member at all: a FlatBuffers enum declaration has no
/// `reserved` construct, and this table is a name-to-number map rather than
/// a wire layout, so a gap in the numbering is simply a gap.
///
/// The generated enum name is claimed in the namespace scope — a declared
/// type spelling `<Interface>Ordinal` collides with it — and each member is
/// claimed in the enum's own scope, because two interaction names colliding
/// under SCREAMING_SNAKE would emit a value redefinition the target rejects
/// ([`Namespace`]).
fn emit_identity_tables(
    out: &mut String,
    package: &v2::Package,
    names: &mut Namespace,
) -> Result<(), GenerateError> {
    for shape in package.shapes() {
        let enum_name = format!("{}Ordinal", type_name(shape.name));
        names.claim(
            &enum_name,
            &format!(
                "the identity table generated for interface `{}`",
                shape.name
            ),
        )?;
        let mut values = Namespace::values(&enum_name);
        out.push_str(&format!("\nenum {enum_name} : uint {{\n"));
        for decl in &shape.interface.interactions {
            if matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_))) {
                continue;
            }
            let member = screaming_snake_case(&decl.name);
            values.claim(
                &member,
                &format!("interaction `{}` of interface `{}`", decl.name, shape.name),
            )?;
            out.push_str(&format!("  {member} = {},\n", decl.ordinal));
        }
        out.push_str("}\n");
    }
    Ok(())
}

/// [`generate_with`] with no other packages — the single-package case. A
/// foreign reference fails rather than emitting an unresolvable name.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    generate_with(package, &[])
}
