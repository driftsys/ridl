//! RIDL-153, RIDL-154 and RIDL-155: the build-time errors of the podbuf
//! encoding. They run only when a `[backend.<name>]` table of the workspace
//! manifest enables `podbuf`.
//!
//! - RIDL-153: a struct or union whose largest podbuf encoding is above
//!   `u32::MAX` bytes. It is reported on the declaration name. A type that
//!   has no bound only because an array or map inside it is past a descriptor
//!   limit draws RIDL-154 on that field instead, so each defect is reported
//!   once.
//! - RIDL-154: a field whose array or map declares more than 65,535 elements,
//!   or whose element or entry is larger than 65,535 bytes. It is reported on
//!   the field.
//! - RIDL-155: a deprecated, non-optional field that has no declared and no
//!   derivable init value, and whose type excludes 0. The encoder writes a
//!   deprecated field's init value, so it has no valid value to write. It is
//!   reported on the field.
//!
//! The limits and the sizes are the ones of [`ridl_ir::projection::podbuf`],
//! the one definition of the layout.

use ridl_core::diag::{DiagCode, Diagnostic, Severity, Span};
use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::projection::podbuf::{
    self, DescriptorKind, Form, MAX_COUNT, MAX_ELEM_SIZE, exceeds_descriptor_limits, field_form,
    max_size, struct_head, union_head,
};
use ridl_ir::projection::size::Ctx as SizeCtx;
use ridl_ir::v2::{self, decl, field_type, struct_member};

use super::Ctx;

/// Whether `encodings` names podbuf.
pub(super) fn enabled(encodings: &[String]) -> bool {
    encodings.iter().any(|name| name == "podbuf")
}

pub(crate) fn check(ctx: &Ctx<'_>, std_ir: &v2::Package, encodings: &[String]) -> Vec<Diagnostic> {
    if !enabled(encodings) {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for checked in ctx.subjects() {
        let package = &checked.ir;
        let others: Vec<&v2::Package> = ctx
            .checked
            .iter()
            .filter(|other| other.ir.name != package.name)
            .map(|other| &other.ir)
            .chain(std::iter::once(std_ir))
            .collect();
        let packages = Packages {
            package,
            others: &others,
        };
        let scope = SizeCtx::new(package, &others);
        for declaration in &package.decls {
            let Some(site) = ctx.sites.decl(&package.name, &declaration.name) else {
                continue;
            };
            let name = format!("{}.{}", package.name, declaration.name);
            match declaration.kind.as_ref() {
                Some(decl::Kind::StructDef(def)) => {
                    let head = struct_head(packages, &declaration.name, def);
                    diagnostics.extend(too_large(&name, site, packages, declaration, head));
                    for member in &def.members {
                        let Some(struct_member::Member::Field(field)) = &member.member else {
                            continue;
                        };
                        let field_site = ctx
                            .sites
                            .field(&package.name, &declaration.name, &field.name)
                            .unwrap_or(site);
                        let path = format!("{name}.{}", field.name);
                        if field
                            .r#type
                            .as_ref()
                            .is_some_and(|ty| past_descriptor_limits(packages, ty))
                        {
                            diagnostics.push(error(
                                DiagCode::RIDL_154,
                                field_site,
                                format!(
                                    "the field `{path}` declares an array or map with more than \
                                     {MAX_COUNT} elements, or an element or entry larger than \
                                     {MAX_ELEM_SIZE} bytes, which a podbuf descriptor cannot record"
                                ),
                            ));
                        }
                        if has_no_valid_value(&scope, package, field) {
                            diagnostics.push(error(
                                DiagCode::RIDL_155,
                                field_site,
                                format!(
                                    "the field `{path}` is deprecated, has no declared or \
                                     derivable init value, and its type excludes 0, so the \
                                     podbuf encoder has no valid value to write; declare an \
                                     init value"
                                ),
                            ));
                        }
                    }
                }
                Some(decl::Kind::UnionDef(def)) => {
                    let head = union_head(packages, &declaration.name, def);
                    diagnostics.extend(too_large(&name, site, packages, declaration, head));
                }
                _ => {}
            }
        }
    }
    diagnostics
}

fn error(code: DiagCode, primary: Span, message: String) -> Diagnostic {
    Diagnostic {
        code,
        severity: Severity::Error,
        message,
        primary,
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

/// RIDL-153 for `declaration`, whose layout is `head`. A layout that is an
/// error is a head a `u32` offset cannot address, because the IR of a package
/// without errors has no other defect.
fn too_large(
    name: &str,
    site: Span,
    packages: Packages<'_>,
    declaration: &v2::Decl,
    head: Result<podbuf::Head, ridl_ir::projection::flatbuffers::ProjectionError>,
) -> Option<Diagnostic> {
    let limit_defect = head.as_ref().is_ok_and(exceeds_descriptor_limits);
    if limit_defect || max_size(packages, declaration).is_some() {
        return None;
    }
    Some(error(
        DiagCode::RIDL_153,
        site,
        format!(
            "`{name}` is too large for podbuf: its largest encoding is above {} bytes, which a \
             u32 offset cannot address",
            u32::MAX
        ),
    ))
}

/// Whether an array or map written in `ty`, at any depth through inline
/// types, is past a descriptor limit. A named type is not followed: its own
/// declaration is reported on its own field.
fn past_descriptor_limits(packages: Packages<'_>, ty: &v2::FieldType) -> bool {
    match ty.kind.as_ref() {
        Some(field_type::Kind::Array(array)) => {
            let own = array.min != array.max
                && matches!(
                    field_form(packages, ty),
                    Ok(Form::Descriptor(DescriptorKind::Array { elem_size, max, .. }))
                        if max > MAX_COUNT || u64::from(elem_size) > MAX_ELEM_SIZE
                );
            own || array
                .element
                .as_ref()
                .is_some_and(|element| past_descriptor_limits(packages, element))
        }
        Some(field_type::Kind::Map(map)) => {
            let own = matches!(
                field_form(packages, ty),
                Ok(Form::Descriptor(DescriptorKind::Map { elem_size, max, .. }))
                    if max > MAX_COUNT || u64::from(elem_size) > MAX_ELEM_SIZE
            );
            own || [&map.key, &map.value]
                .into_iter()
                .flatten()
                .any(|inner| past_descriptor_limits(packages, inner))
        }
        Some(field_type::Kind::Tuple(tuple)) => tuple
            .fields
            .iter()
            .filter_map(|field| field.r#type.as_ref())
            .any(|inner| past_descriptor_limits(packages, inner)),
        _ => false,
    }
}

/// Whether `field` is deprecated and the encoder has no value to write for it:
/// it is not optional, declares no init, has no derivable init, and its type
/// excludes 0.
fn has_no_valid_value(scope: &SizeCtx<'_>, home: &v2::Package, field: &v2::Field) -> bool {
    let Some(ty) = field.r#type.as_ref() else {
        return false;
    };
    field.deprecated.is_some()
        && !ty.optional
        && field.declared_init.is_none()
        && !field.init.as_ref().is_some_and(|init| init.derivable)
        && !holds_zero(scope, home, ty)
}

/// Whether 0 is a valid value of `ty`: the rule `ridl diff` applies to an
/// appended field, with the same helpers of [`ridl_ir::zero`].
fn holds_zero(scope: &SizeCtx<'_>, home: &v2::Package, ty: &v2::FieldType) -> bool {
    match ty.kind.as_ref() {
        Some(field_type::Kind::Primitive(primitive)) => matches!(
            v2::PrimitiveType::try_from(*primitive),
            Ok(v2::PrimitiveType::Boolean | v2::PrimitiveType::Integer | v2::PrimitiveType::Float)
        ),
        Some(field_type::Kind::InlineScalar(def)) => scalar_holds_zero(def),
        Some(field_type::Kind::Named(reference)) => {
            match scope
                .resolve(home, reference)
                .and_then(|(declaration, _)| declaration.kind.as_ref())
            {
                Some(decl::Kind::TypeDef(def)) => scalar_holds_zero(def),
                Some(decl::Kind::EnumDef(def)) => {
                    ridl_ir::zero::enum_zero_member(&def.values).is_some()
                }
                // An enum set at 0 is the empty set.
                Some(decl::Kind::EnumSetDef(_)) => true,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Whether a scalar is valid at 0: a boolean, integer, float or unit backing
/// whose constraint holds 0. A string or bytes backing is not.
fn scalar_holds_zero(def: &v2::TypeDef) -> bool {
    let numeric = match def
        .backing
        .as_ref()
        .and_then(|backing| backing.kind.as_ref())
    {
        Some(v2::backing::Kind::Unit(_)) => true,
        Some(v2::backing::Kind::Primitive(primitive)) => matches!(
            v2::PrimitiveType::try_from(*primitive),
            Ok(v2::PrimitiveType::Boolean | v2::PrimitiveType::Integer | v2::PrimitiveType::Float)
        ),
        None => false,
    };
    numeric
        && def.constraint.as_ref().is_none_or(|constraint| {
            ridl_ir::zero::range_holds_zero(
                constraint.min.as_deref(),
                constraint.max.as_deref(),
                constraint.step.as_deref(),
            )
        })
}
