//! The verdict of a [`DiffReport`] restricted to one unit.

use std::collections::BTreeSet;

use ridl_ir::catalog_hash::reachable_decls;
use ridl_ir::v2::{Package, unit_of};

use crate::{DiffReport, Verdict};

/// The verdict of `report` over the changes that concern `unit`: the maximum
/// [`Change::verdict`](crate::Change::verdict) of those changes, or
/// [`Verdict::Identical`] when none concerns the unit. `old` and `new` are the
/// two package sets the report compared.
///
/// The function reads each change's [`Change::path`](crate::Change::path),
/// which the walk builds as `<package>/<name>[/<member>...]`: the first
/// `/`-separated segment is the dotted name of the package that holds the new
/// side, or the old side when the package is gone, and the second, when present, is the bare name of a package-level
/// declaration, interface or service of that package. A package name holds
/// dots and a declaration name holds none, so the first two segments joined
/// by `.` are the declaration's canonical name, the key that
/// [`reachable_decls`] uses.
///
/// A change concerns `unit` when either holds:
///
/// - its first segment names a package whose unit ([`unit_of`]) is `unit`, in
///   `old` or in `new`;
/// - its first two segments, joined by `.`, are a key of
///   `reachable_decls(unit, old)` or of `reachable_decls(unit, new)`: a
///   declaration of another unit that an interface shape of `unit` reaches on
///   either side.
///
/// So a breaking change to a struct of another unit that `unit` reaches is
/// breaking for `unit`, and a breaking change in a unit that `unit` does not
/// reach leaves `unit`'s verdict unchanged. The report's own
/// [`DiffReport::verdict`] and its [`DiffReport::system`] changes are not
/// read.
pub fn unit_verdict(report: &DiffReport, unit: &str, old: &[Package], new: &[Package]) -> Verdict {
    let packages: BTreeSet<&str> = old
        .iter()
        .chain(new)
        .filter(|pkg| unit_of(pkg) == unit)
        .map(|pkg| pkg.name.as_str())
        .collect();
    let old_refs: Vec<&Package> = old.iter().collect();
    let new_refs: Vec<&Package> = new.iter().collect();
    let reached_old = reachable_decls(unit, &old_refs);
    let reached_new = reachable_decls(unit, &new_refs);

    report
        .changes
        .iter()
        .filter(|change| {
            let mut segments = change.path.split('/');
            let package = segments.next().unwrap_or_default();
            if packages.contains(package) {
                return true;
            }
            segments.next().is_some_and(|name| {
                let canonical = format!("{package}.{name}");
                reached_old.contains_key(&canonical) || reached_new.contains_key(&canonical)
            })
        })
        .map(|change| change.verdict)
        .max()
        .unwrap_or(Verdict::Identical)
}

#[cfg(test)]
mod tests {
    use ridl_ir::v2::{
        Decl, EventDef, Field, FieldType, IntWidth, Interface, Package, Reserved, SignalDef,
        StructDef, StructMember, TypeDef, Visibility, backing, decl, field_type, struct_member,
        type_def,
    };

    use super::unit_verdict;
    use crate::{Verdict, diff_sets};

    /// An inline integer scalar of the given width.
    fn int(width: IntWidth) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::InlineScalar(Box::new(TypeDef {
                backing: Some(ridl_ir::v2::Backing {
                    kind: Some(backing::Kind::Primitive(
                        ridl_ir::v2::PrimitiveType::Integer as i32,
                    )),
                }),
                width: Some(type_def::Width::IntWidth(width as i32)),
                ..Default::default()
            }))),
        }
    }

    /// `struct <name> { <field>: <width> }`.
    fn one_field_struct(name: &str, field: &str, width: IntWidth) -> Decl {
        Decl {
            name: name.to_owned(),
            visibility: Visibility::Public as i32,
            kind: Some(decl::Kind::StructDef(StructDef {
                members: vec![StructMember {
                    member: Some(struct_member::Member::Field(Box::new(Field {
                        name: field.to_owned(),
                        ordinal: 1,
                        r#type: Some(int(width)),
                        ..Default::default()
                    }))),
                }],
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn package(name: &str, decls: Vec<Decl>, interfaces: Vec<Interface>) -> Package {
        Package {
            name: name.to_owned(),
            decls,
            interfaces,
            unit: name.to_owned(),
            ..Default::default()
        }
    }

    /// Unit `a`: one interface whose signal carries `payload`.
    fn unit_a(payload: &str) -> Package {
        let signal = Decl {
            name: "status".to_owned(),
            ordinal: 1,
            kind: Some(decl::Kind::SignalDef(SignalDef {
                payload: payload.to_owned(),
                ..Default::default()
            })),
            ..Default::default()
        };
        package(
            "a",
            Vec::new(),
            vec![Interface {
                name: "Status".to_owned(),
                visibility: Visibility::Public as i32,
                number: 1,
                provisional: false,
                interactions: vec![signal],
                ..Default::default()
            }],
        )
    }

    fn unit_b(width: IntWidth) -> Package {
        package("b", vec![one_field_struct("S", "x", width)], Vec::new())
    }

    fn unit_c(width: IntWidth) -> Package {
        package("c", vec![one_field_struct("T", "y", width)], Vec::new())
    }

    fn baseline() -> Vec<Package> {
        vec![unit_a("b.S"), unit_b(IntWidth::I32), unit_c(IntWidth::I32)]
    }

    #[test]
    fn a_break_in_a_reached_declaration_of_another_unit_concerns_this_unit() {
        let old = baseline();
        let new = vec![unit_a("b.S"), unit_b(IntWidth::I64), unit_c(IntWidth::I32)];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "c", &old, &new), Verdict::Identical);
    }

    #[test]
    fn a_break_in_an_unreached_unit_does_not_concern_this_unit() {
        let old = baseline();
        let new = vec![unit_a("b.S"), unit_b(IntWidth::I32), unit_c(IntWidth::I64)];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Identical);
        assert_eq!(unit_verdict(&report, "c", &old, &new), Verdict::Breaking);
    }

    #[test]
    fn an_appended_interaction_is_compatible_for_its_own_unit() {
        let old = baseline();
        let mut a = unit_a("b.S");
        a.interfaces[0].interactions.push(appended_event("b.S"));
        let new = vec![a, unit_b(IntWidth::I32), unit_c(IntWidth::I32)];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Compatible);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    #[test]
    fn a_package_added_to_a_unit_concerns_that_unit() {
        let old = baseline();
        let mut sub = package(
            "a.sub",
            vec![one_field_struct("U", "z", IntWidth::I32)],
            Vec::new(),
        );
        sub.unit = "a".to_owned();
        let mut new = baseline();
        new.push(sub);
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Compatible);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    /// An event appended to unit `a`'s interface, with payload `payload`.
    fn appended_event(payload: &str) -> Decl {
        Decl {
            name: "alarm".to_owned(),
            ordinal: 2,
            kind: Some(decl::Kind::EventDef(EventDef {
                payload: payload.to_owned(),
                timing: None,
            })),
            ..Default::default()
        }
    }

    // The change to unit `a`'s own package, an appended event, is compatible,
    // so only the new-side reach of `c.T` makes `a` breaking.
    #[test]
    fn a_declaration_reached_only_on_the_new_side_concerns_the_unit() {
        let old = baseline();
        let mut a = unit_a("b.S");
        a.interfaces[0].interactions.push(appended_event("c.T"));
        let new = vec![a, unit_b(IntWidth::I32), unit_c(IntWidth::I64)];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    // The mirror of `a_declaration_reached_only_on_the_new_side_concerns_the_unit`: the interaction that reached `c.T` is
    // retired to a tombstone in its own slot, which is compatible, so only the
    // old-side reach of `c.T` makes `a` breaking.
    #[test]
    fn a_break_reached_only_through_a_retired_interaction_concerns_the_unit() {
        let mut a_old = unit_a("b.S");
        a_old.interfaces[0].interactions.push(appended_event("c.T"));
        let old = vec![a_old, unit_b(IntWidth::I32), unit_c(IntWidth::I32)];
        let mut a_new = unit_a("b.S");
        a_new.interfaces[0].interactions.push(Decl {
            ordinal: 2,
            kind: Some(decl::Kind::ReservedSlot(Reserved {
                ordinal: 2,
                name: Some("alarm".to_owned()),
                value: None,
            })),
            ..Default::default()
        });
        let new = vec![a_new, unit_b(IntWidth::I32), unit_c(IntWidth::I64)];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    /// Package `a.sub` of unit `a`, holding `struct U { z: i32 }`.
    fn sub_package() -> Package {
        let mut sub = package(
            "a.sub",
            vec![one_field_struct("U", "z", IntWidth::I32)],
            Vec::new(),
        );
        sub.unit = "a".to_owned();
        sub
    }

    // The removed package exists only on the old side, so only the old-side
    // package rule makes the removal concern unit `a`.
    #[test]
    fn a_package_removed_from_a_unit_concerns_that_unit() {
        let mut old = baseline();
        old.push(sub_package());
        let new = baseline();
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    // The compatible package addition is reported after the breaking change to
    // `b.S`, so a fold that keeps the last verdict instead of the maximum
    // returns `Compatible`.
    #[test]
    fn the_unit_verdict_is_the_maximum_over_its_changes() {
        let old = baseline();
        let mut new = vec![unit_a("b.S"), unit_b(IntWidth::I64), unit_c(IntWidth::I32)];
        new.push(sub_package());
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "c", &old, &new), Verdict::Identical);
    }
}
