//! The extern-C header face (ADR-0007 decision 13).
//!
//! A minijinja template (`templates/c_header.j2`) renders scalar typedefs, enum
//! constants, enum-set bit masks, and `repr(C)` struct declarations for
//! `fixed_layout` structs. Shapes with no fixed C ABI — bounded arrays, maps,
//! unions, optionals, strings, and variable-layout structs — are listed in a
//! trailing comment block rather than mis-mapped.

use crate::{GenerateError, ScalarBacking, backing_scalar};
use minijinja::{Environment, context};
use ridl_ir::v2;
use serde::Serialize;

const TEMPLATE: &str = include_str!("../templates/c_header.j2");
const IR_VERSION: &str = "ridl.ir.v2";

#[derive(Serialize)]
struct Typedef {
    c_type: String,
    c_ident: String,
}

#[derive(Serialize)]
struct EnumConst {
    name: String,
    value: i64,
}

#[derive(Serialize)]
struct CEnum {
    c_ident: String,
    values: Vec<EnumConst>,
}

#[derive(Serialize)]
struct EnumSetBit {
    name: String,
    shift: i64,
}

#[derive(Serialize)]
struct CEnumSet {
    c_ident: String,
    bits: Vec<EnumSetBit>,
}

#[derive(Serialize)]
struct CField {
    c_type: String,
    name: String,
}

#[derive(Serialize)]
struct CStruct {
    c_ident: String,
    fields: Vec<CField>,
}

pub(crate) fn render(package: &v2::Package) -> Result<String, GenerateError> {
    let mut typedefs: Vec<Typedef> = Vec::new();
    let mut enums: Vec<CEnum> = Vec::new();
    let mut enum_sets: Vec<CEnumSet> = Vec::new();
    let mut structs: Vec<CStruct> = Vec::new();
    let mut not_representable: Vec<String> = Vec::new();

    for decl in &package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => match c_scalar_type(backing_scalar(td)) {
                Some(c_type) => typedefs.push(Typedef {
                    c_type: c_type.to_string(),
                    c_ident: c_ident(&package.name, &decl.name),
                }),
                None => not_representable.push(format!(
                    "type {} — string/bytes backing has no fixed C ABI",
                    decl.name
                )),
            },
            Some(v2::decl::Kind::EnumDef(ed)) => {
                let ident = c_ident(&package.name, &decl.name);
                let prefix = ident.to_uppercase();
                enums.push(CEnum {
                    values: ed
                        .values
                        .iter()
                        .map(|value| EnumConst {
                            name: format!("{prefix}_{}", value.name),
                            value: value.value,
                        })
                        .collect(),
                    c_ident: ident,
                });
            }
            Some(v2::decl::Kind::EnumSetDef(esd)) => {
                let ident = c_ident(&package.name, &decl.name);
                let prefix = ident.to_uppercase();
                enum_sets.push(CEnumSet {
                    bits: esd
                        .bits
                        .iter()
                        .map(|bit| EnumSetBit {
                            name: format!("{prefix}_{}", bit.name),
                            shift: bit.value,
                        })
                        .collect(),
                    c_ident: ident,
                });
            }
            Some(v2::decl::Kind::StructDef(sd)) => match c_struct(package, decl, sd) {
                Ok(entry) => structs.push(entry),
                Err(reason) => not_representable.push(reason),
            },
            Some(v2::decl::Kind::UnionDef(_)) => {
                not_representable.push(format!("union {} — tagged union", decl.name));
            }
            Some(v2::decl::Kind::ConstDef(_)) | None => {}
            // Interaction kinds ride `Interface.interactions`, never a
            // package decl; the interaction codegen is E2 task 15.
            Some(_) => {}
        }
    }

    let mut env = Environment::new();
    env.add_template("c_header", TEMPLATE)
        .map_err(|err| GenerateError {
            message: format!("C header template does not compile: {err}"),
        })?;
    let template = env
        .get_template("c_header")
        .expect("the template was just added");
    let guard = format!("{}_H", package.name.replace('.', "_").to_uppercase());
    template
        .render(context! {
            package => package.name,
            ir_version => IR_VERSION,
            guard => guard,
            typedefs => typedefs,
            enums => enums,
            enumsets => enum_sets,
            structs => structs,
            not_representable => not_representable,
        })
        .map_err(|err| GenerateError {
            message: format!("C header rendering failed: {err}"),
        })
}

/// The C typedef target for a scalar backing (Appendix D language layer), or
/// `None` for string/bytes, which have no fixed C ABI.
fn c_scalar_type(backing: ScalarBacking) -> Option<&'static str> {
    match backing {
        ScalarBacking::Float => Some("double"),
        ScalarBacking::Integer => Some("int64_t"),
        ScalarBacking::Boolean => Some("bool"),
        ScalarBacking::String | ScalarBacking::Bytes => None,
    }
}

/// Builds a C struct declaration for a `fixed_layout` struct, or an error
/// reason when the struct is not C-representable.
fn c_struct(package: &v2::Package, decl: &v2::Decl, sd: &v2::StructDef) -> Result<CStruct, String> {
    if !sd.fixed_layout {
        return Err(format!(
            "struct {} — variable layout (optional, string, or collection field)",
            decl.name
        ));
    }
    let mut fields = Vec::new();
    for member in &sd.members {
        if let Some(v2::struct_member::Member::Field(field)) = &member.member {
            let c_type = c_field_type(package, field)
                .ok_or_else(|| format!("struct {} — field with no fixed C ABI", decl.name))?;
            fields.push(CField {
                c_type,
                name: field.name.clone(),
            });
        }
    }
    Ok(CStruct {
        c_ident: c_ident(&package.name, &decl.name),
        fields,
    })
}

fn c_field_type(package: &v2::Package, field: &v2::Field) -> Option<String> {
    let ft = field.r#type.as_ref()?;
    if ft.optional {
        return None;
    }
    match &ft.kind {
        Some(v2::field_type::Kind::Named(reference)) => {
            // A same-package reference must name a type this header actually
            // emits; otherwise the field would reference an undeclared typedef
            // (C2). With string/bytes-backed types excluded from `fixed_layout`,
            // a `fixed_layout` struct never reaches this guard's `None` arm, but
            // it keeps the emitter from ever writing a dangling type name. A
            // cross-package reference names a type in another package's header,
            // which the consumer includes alongside this one (documented in the
            // template).
            if reference.contains('.') || same_package_ref_is_c_representable(package, reference) {
                Some(c_ident_for_ref(&package.name, reference))
            } else {
                None
            }
        }
        Some(v2::field_type::Kind::Primitive(prim)) => c_primitive_type(*prim).map(str::to_string),
        Some(v2::field_type::Kind::InlineScalar(td)) => {
            c_scalar_type(backing_scalar(td)).map(str::to_string)
        }
        _ => None,
    }
}

/// Whether a same-package named reference resolves to a declaration this header
/// emits as a nameable C type: a scalar typedef with a fixed C ABI, an enum, an
/// enum-set mask type, or a `fixed_layout` struct. A string/bytes-backed type, a
/// union, a variable-layout struct, or an unknown name is not nameable here.
fn same_package_ref_is_c_representable(package: &v2::Package, reference: &str) -> bool {
    package
        .decls
        .iter()
        .find(|decl| decl.name == reference)
        .is_some_and(|decl| match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => c_scalar_type(backing_scalar(td)).is_some(),
            Some(v2::decl::Kind::EnumDef(_) | v2::decl::Kind::EnumSetDef(_)) => true,
            Some(v2::decl::Kind::StructDef(sd)) => sd.fixed_layout,
            _ => false,
        })
}

fn c_primitive_type(prim: i32) -> Option<&'static str> {
    match v2::PrimitiveType::try_from(prim).unwrap_or(v2::PrimitiveType::Unspecified) {
        v2::PrimitiveType::Integer => Some("int64_t"),
        v2::PrimitiveType::Float => Some("double"),
        v2::PrimitiveType::Boolean => Some("bool"),
        _ => None,
    }
}

/// The C identifier for a declaration: the package path and name flattened to
/// snake_case, for example package `veh.common` and type `Speed` give
/// `veh_common_speed`.
fn c_ident(package: &str, name: &str) -> String {
    format!("{}_{}", package.replace('.', "_"), snake_case(name))
}

/// The C identifier for a resolved type reference: a bare same-package `Name`
/// uses this package; a fully qualified `pkg.Name` uses its own package.
fn c_ident_for_ref(package: &str, reference: &str) -> String {
    match reference.rsplit_once('.') {
        Some((pkg, name)) => format!("{}_{}", pkg.replace('.', "_"), snake_case(name)),
        None => c_ident(package, reference),
    }
}

/// snake_case of a CamelCase or acronym name, for C identifiers. A boundary is
/// inserted before an uppercase letter that follows a lowercase letter or that
/// begins a word (an uppercase run followed by a lowercase letter).
fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (index, &current) in chars.iter().enumerate() {
        if current.is_uppercase() && index > 0 {
            let previous = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(|c| c.is_lowercase());
            if previous.is_lowercase()
                || previous.is_numeric()
                || (previous.is_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.extend(current.to_lowercase());
    }
    out
}
