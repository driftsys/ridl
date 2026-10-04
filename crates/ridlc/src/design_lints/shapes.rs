//! Compare field sets using nominal, qualified type identities.

use std::collections::BTreeMap;

use ridl_core::diag::{DiagCode, Diagnostic, Label, Span};
use ridl_core::lint::lint_by_name;
use ridl_ir::v2::{self, decl, field_type, stream_type, struct_member};

use super::{Ctx, qualify};

pub(crate) const DUPLICATE_SHAPE_MIN_FIELDS: usize = 2;
pub(crate) const DUPLICATE_SHAPE_MIN_VARIANTS: usize = 2;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    Struct(Vec<(String, String)>),
    Enum(Vec<String>),
}

pub(crate) fn check(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let mut packages: Vec<_> = ctx
        .checked
        .iter()
        .filter(|p| p.ir.name != "ridl.std")
        .collect();
    packages.sort_by(|a, b| a.ir.name.cmp(&b.ir.name));
    let mut first: BTreeMap<Shape, (String, Span)> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for checked in packages {
        let pkg = &checked.ir;
        // Checked declarations retain the source order and resolver winners.
        for declaration in &pkg.decls {
            let shape = match &declaration.kind {
                Some(decl::Kind::StructDef(def)) => {
                    let mut fields: Vec<_> = def
                        .members
                        .iter()
                        .filter_map(|member| {
                            let struct_member::Member::Field(field) = member.member.as_ref()?
                            else {
                                return None;
                            };
                            Some((
                                field.name.clone(),
                                type_key(&pkg.name, field.r#type.as_ref()?),
                            ))
                        })
                        .collect();
                    if fields.len() < DUPLICATE_SHAPE_MIN_FIELDS {
                        continue;
                    }
                    fields.sort();
                    Shape::Struct(fields)
                }
                Some(decl::Kind::EnumDef(def)) => {
                    let mut variants: Vec<_> = def.values.iter().map(|v| v.name.clone()).collect();
                    if variants.len() < DUPLICATE_SHAPE_MIN_VARIANTS {
                        continue;
                    }
                    variants.sort();
                    Shape::Enum(variants)
                }
                _ => continue,
            };
            let Some(span) = ctx.sites.decl(&pkg.name, &declaration.name) else {
                continue;
            };
            let name = format!("{}.{}", pkg.name, declaration.name);
            if let Some((earlier, earlier_span)) = first.get(&shape) {
                let (count, kind) = match &shape {
                    Shape::Struct(fields) => (fields.len(), "fields"),
                    Shape::Enum(variants) => (variants.len(), "variants"),
                };
                diagnostics.push(Diagnostic {
                    code: DiagCode::TYPL_224,
                    severity: lint_by_name("duplicate-shape")
                        .expect("registered lint")
                        .severity,
                    message: format!("`{name}` has the same {count} {kind} as `{earlier}`"),
                    primary: span,
                    labels: vec![Label {
                        span: *earlier_span,
                        message: format!("`{earlier}` declared here"),
                    }],
                    fixits: Vec::new(),
                });
            } else {
                first.insert(shape, (name, span));
            }
        }
    }
    diagnostics
}

fn canonicalize(pkg: &str, ty: &mut v2::FieldType) {
    match &mut ty.kind {
        Some(field_type::Kind::Named(name)) => qualify(pkg, name),
        Some(field_type::Kind::Tuple(tuple)) => {
            for field in &mut tuple.fields {
                if let Some(ty) = &mut field.r#type {
                    canonicalize(pkg, ty);
                }
            }
        }
        Some(field_type::Kind::Array(array)) => {
            if let Some(ty) = &mut array.element {
                canonicalize(pkg, ty);
            }
        }
        Some(field_type::Kind::Map(map)) => {
            for ty in [&mut map.key, &mut map.value].into_iter().flatten() {
                canonicalize(pkg, ty);
            }
        }
        Some(field_type::Kind::Stream(stream)) => {
            if let Some(stream_type::Element::Named(name)) = &mut stream.element {
                qualify(pkg, name);
            }
        }
        Some(field_type::Kind::InlineScalar(scalar)) => {
            if let Some(name) = scalar
                .constraint
                .as_mut()
                .and_then(|c| c.pattern_const.as_mut())
            {
                qualify(pkg, name);
            }
        }
        Some(field_type::Kind::Primitive(_)) | None => {}
    }
}

fn type_key(pkg: &str, ty: &v2::FieldType) -> String {
    let mut ty = ty.clone();
    canonicalize(pkg, &mut ty);
    // FieldType contains only type data, including optionality and bounds;
    // field ordinals, documentation and initial values are outside this key.
    serde_json::to_string(&ty).expect("IR field type is serializable")
}
