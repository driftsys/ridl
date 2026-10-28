//! RIDL-153, RIDL-154 and RIDL-155: the build-time errors of the podbuf
//! encoding. They run only when a `[backend.<name>]` table of the workspace
//! manifest enables `podbuf`.
//!
//! - RIDL-153: a struct, a union, or a named tuple (the argument struct a
//!   call with several parameters induces, or a tuple reply) whose largest
//!   podbuf encoding is above `u32::MAX` bytes. It is reported on the
//!   declaration name, or on the interaction for a named tuple. A type that
//!   has no bound only because an array or map inside it is past a descriptor
//!   limit draws RIDL-154 on that field instead, so each defect is reported
//!   once.
//! - RIDL-154: a field whose array or map declares more than 65,535 elements,
//!   or whose element or entry is larger than 65,535 bytes. It is reported on
//!   the field of a struct, or on the interaction for a field of a tuple
//!   reply.
//! - RIDL-155: a deprecated, non-optional field that has no declared and no
//!   derivable init value, and whose type excludes 0. The encoder writes a
//!   deprecated field's init value, so it has no valid value to write. It is
//!   reported on the field.
//!
//! The limits and the sizes are the ones of [`ridl_ir::projection::podbuf`],
//! the one definition of the layout.

use ridl_core::diag::{DiagCode, Diagnostic, Severity, Span};
use ridl_ir::codegen;
use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::projection::podbuf::{
    self, DescriptorKind, Form, MAX_COUNT, MAX_ELEM_SIZE, exceeds_descriptor_limits, field_form,
    max_size, struct_head, tuple_head, tuple_max_size, union_head,
};
use ridl_ir::projection::size::{Ctx as SizeCtx, PayloadShape};
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
        // The named tuples the lowering induces: the argument struct of a
        // call with several parameters, and a tuple reply. Each is a head of
        // its own, reported on the interaction.
        for shape in package.shapes() {
            for interaction in &shape.interface.interactions {
                let Some(site) = ctx
                    .sites
                    .member(&package.name, shape.name, &interaction.name)
                else {
                    continue;
                };
                let owner = format!("{}.{}.{}", package.name, shape.name, interaction.name);
                let (params, reply) = match interaction.kind.as_ref() {
                    Some(decl::Kind::CommandDef(def)) => (&def.params, None),
                    Some(decl::Kind::QueryDef(def)) => (
                        &def.params,
                        match codegen::query_reply_type(def) {
                            Some(_) => None,
                            None => codegen::reply_as_tuple(def.return_type.as_ref()),
                        },
                    ),
                    _ => continue,
                };
                let induced = codegen::params_as_tuple(params);
                if let PayloadShape::Tuple(tuple) = codegen::request_shape(params, induced.as_ref())
                {
                    diagnostics.extend(tuple_too_large(
                        &format!("the request of `{owner}`"),
                        site,
                        packages,
                        tuple,
                    ));
                }
                if let Some(tuple) = reply.as_ref() {
                    diagnostics.extend(tuple_too_large(
                        &format!("the reply of `{owner}`"),
                        site,
                        packages,
                        tuple,
                    ));
                    diagnostics.extend(tuple_field_limits(&owner, site, packages, tuple));
                }
            }
        }
    }
    diagnostics
}

/// RIDL-154 for each field of the tuple reply of `owner` that is past a
/// descriptor limit, reported on the interaction. The parameters of a call are
/// named types or streams, so the induced argument struct has no inline array
/// or map to check.
fn tuple_field_limits<'a>(
    owner: &'a str,
    site: Span,
    packages: Packages<'a>,
    tuple: &'a v2::TupleType,
) -> impl Iterator<Item = Diagnostic> + 'a {
    tuple.fields.iter().filter_map(move |field| {
        let past = field
            .r#type
            .as_ref()
            .is_some_and(|ty| past_descriptor_limits(packages, ty));
        past.then(|| {
            error(
                DiagCode::RIDL_154,
                site,
                format!(
                    "the field `{}` of the reply of `{owner}` declares an array or map with \
                     more than {MAX_COUNT} elements, or an element or entry larger than \
                     {MAX_ELEM_SIZE} bytes, which a podbuf descriptor cannot record",
                    field.name
                ),
            )
        })
    })
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
    ridl_153(
        &format!("`{name}`"),
        site,
        head.as_ref().is_ok_and(exceeds_descriptor_limits),
        max_size(packages, declaration),
    )
}

/// RIDL-153 for a named tuple the lowering induces, described by `what`.
fn tuple_too_large(
    what: &str,
    site: Span,
    packages: Packages<'_>,
    tuple: &v2::TupleType,
) -> Option<Diagnostic> {
    let head = tuple_head(packages, what, tuple);
    ridl_153(
        what,
        site,
        head.as_ref().is_ok_and(exceeds_descriptor_limits),
        tuple_max_size(packages, tuple),
    )
}

/// RIDL-153 when `bound` is `None` and the head is not past a descriptor
/// limit (which RIDL-154 reports on the field instead).
fn ridl_153(what: &str, site: Span, limit_defect: bool, bound: Option<u64>) -> Option<Diagnostic> {
    if limit_defect || bound.is_some() {
        return None;
    }
    Some(error(
        DiagCode::RIDL_153,
        site,
        format!(
            "{what} is too large for podbuf: its largest encoding is above {} bytes, which a \
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
/// appended field, [`ridl_ir::zero::field_type_holds_zero`].
fn holds_zero(scope: &SizeCtx<'_>, home: &v2::Package, ty: &v2::FieldType) -> bool {
    ridl_ir::zero::field_type_holds_zero(ty, |reference| {
        scope
            .resolve(home, reference)
            .map(|(declaration, _)| declaration)
    })
}
