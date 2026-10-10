//! The verdict of a [`DiffReport`] restricted to one unit.

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::catalog_hash::reachable_decls;
use ridl_ir::v2::{Package, unit_of};

use crate::{DiffReport, Verdict};

/// The verdict of `report` over the changes that concern `unit`: the maximum
/// [`Change::verdict`](crate::Change::verdict) of those changes, or
/// [`Verdict::Identical`] when none concerns the unit. `old` and `new` are the
/// two package sets the report compared.
///
/// A caller that asks for several units of one diff builds a
/// [`UnitVerdicts`] once and calls [`UnitVerdicts::verdict`] per unit; this
/// function builds one for a single call.
///
/// The change's package and declaration come from
/// [`Change::package`](crate::Change::package) and
/// [`Change::declaration`](crate::Change::declaration): the dotted name of
/// the package that holds the new side, or the old side when the package is
/// gone, and, when present, the bare name of a package-level declaration,
/// interface or service of that package. A package name holds dots and a
/// declaration name holds none, so the two joined by `.` are the
/// declaration's canonical name, the key that [`reachable_decls`] uses.
///
/// A change concerns `unit` when either holds:
///
/// - its package's unit ([`unit_of`]) is `unit`, in `old` or in `new`;
/// - its package and declaration, joined by `.`, are a key of
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
    UnitVerdicts::new(report, old, new).verdict(unit)
}

/// The per-unit verdicts of one diff, with the work every unit shares done
/// once: the package references of both sides, the package names of each
/// unit and the canonical key of each change. See [`unit_verdict`] for what
/// concerns a unit.
pub struct UnitVerdicts<'a> {
    old: Vec<&'a Package>,
    new: Vec<&'a Package>,
    /// The names of the packages of each unit, on either side.
    names_by_unit: BTreeMap<&'a str, BTreeSet<&'a str>>,
    /// Each change's package, canonical declaration key (absent for a change
    /// to the package itself) and verdict.
    changes: Vec<(&'a str, Option<String>, Verdict)>,
}

impl<'a> UnitVerdicts<'a> {
    /// Prepares the verdicts of `report`, the diff of `old` against `new`.
    pub fn new(report: &'a DiffReport, old: &'a [Package], new: &'a [Package]) -> Self {
        let old: Vec<&Package> = old.iter().collect();
        let new: Vec<&Package> = new.iter().collect();
        Self::from_refs(report, &old, &new)
    }

    /// [`UnitVerdicts::new`] over borrowed packages, for a caller that holds
    /// the packages by reference and would otherwise clone them.
    pub fn from_refs(report: &'a DiffReport, old: &[&'a Package], new: &[&'a Package]) -> Self {
        let mut names_by_unit: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for package in old.iter().chain(new) {
            names_by_unit
                .entry(unit_of(package))
                .or_default()
                .insert(package.name.as_str());
        }
        Self {
            old: old.to_vec(),
            new: new.to_vec(),
            names_by_unit,
            changes: report
                .changes
                .iter()
                .map(|change| {
                    let package = change.package();
                    let canonical = change.declaration().map(|name| format!("{package}.{name}"));
                    (package, canonical, change.verdict)
                })
                .collect(),
        }
    }

    /// The verdict over the changes that concern `unit`.
    pub fn verdict(&self, unit: &str) -> Verdict {
        let no_packages = BTreeSet::new();
        let packages = self.names_by_unit.get(unit).unwrap_or(&no_packages);
        let reached_old = reachable_decls(unit, &self.old);
        let reached_new = reachable_decls(unit, &self.new);

        self.changes
            .iter()
            .filter(|(package, canonical, _)| {
                packages.contains(package)
                    || canonical.as_ref().is_some_and(|canonical| {
                        reached_old.contains_key(canonical) || reached_new.contains_key(canonical)
                    })
            })
            .map(|(_, _, verdict)| *verdict)
            .max()
            .unwrap_or(Verdict::Identical)
    }
}

#[cfg(test)]
mod tests {
    use ridl_ir::v2::{
        Decl, EnumDef, EnumValue, EventDef, Field, FieldType, IntWidth, Interface, Package,
        Reserved, SignalDef, StructDef, StructMember, TypeDef, Visibility, backing, decl,
        field_type, struct_member, type_def,
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

    // The mirror of `a_declaration_reached_only_on_the_new_side_concerns_the_unit`:
    // the interaction that reached `c.T` is retired to a tombstone in its own
    // slot, which is compatible, so only the old-side reach of `c.T` makes `a`
    // breaking.
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

    /// Unit `b` where `struct S { t: c.T }`, so `b.S` reaches `c.T`.
    fn unit_b_referencing_c() -> Package {
        let mut s = one_field_struct("S", "t", IntWidth::I32);
        if let Some(decl::Kind::StructDef(def)) = &mut s.kind
            && let Some(struct_member::Member::Field(field)) = &mut def.members[0].member
        {
            field.r#type = Some(FieldType {
                optional: false,
                kind: Some(field_type::Kind::Named("c.T".to_owned())),
            });
        }
        package("b", vec![s], Vec::new())
    }

    // `a` reaches `b.S`, which reaches `c.T`. The break is in `c.T` only, so
    // it concerns `a` only through two hops. Unit `d` reaches nothing.
    #[test]
    fn a_break_reached_through_two_hops_concerns_the_unit() {
        let old = vec![
            unit_a("b.S"),
            unit_b_referencing_c(),
            unit_c(IntWidth::I32),
            package("d", Vec::new(), Vec::new()),
        ];
        let mut new = old.clone();
        new[2] = unit_c(IntWidth::I64);
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "d", &old, &new), Verdict::Identical);
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

    /// Unit `a` with its interface's number set to `number` and its
    /// provisional flag to `provisional`, beside units `b` and `c`.
    fn numbered(number: u32, provisional: bool) -> Vec<Package> {
        let mut a = unit_a("b.S");
        a.interfaces[0].number = number;
        a.interfaces[0].provisional = provisional;
        vec![a, unit_b(IntWidth::I32), unit_c(IntWidth::I32)]
    }

    // `ridl lock` freezing the number in place changes the catalog hash and
    // is compatible, so the unit's verdict keeps the compatible-hash list
    // that `write_catalog_histories` carries forward.
    #[test]
    fn a_number_frozen_in_place_is_compatible_for_its_unit() {
        let old = numbered(1, true);
        let new = numbered(1, false);
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Compatible);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    // A moved number is a new routing key, so the unit's verdict is breaking
    // and `write_catalog_histories` restarts the compatible-hash list.
    #[test]
    fn a_moved_number_is_breaking_for_its_unit() {
        let old = numbered(1, true);
        let new = numbered(2, true);
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Identical);
    }

    /// Unit `b` holding `enum Mode` with `values` in the given order and the
    /// retired numbers `retired` in the given order.
    fn unit_b_with_enum(values: &[(&str, i64)], retired: &[i64]) -> Package {
        let mode = Decl {
            name: "Mode".to_owned(),
            visibility: Visibility::Public as i32,
            kind: Some(decl::Kind::EnumDef(EnumDef {
                values: values
                    .iter()
                    .map(|(name, value)| EnumValue {
                        name: (*name).to_owned(),
                        value: *value,
                        ..Default::default()
                    })
                    .collect(),
                reserved: retired
                    .iter()
                    .map(|value| Reserved {
                        value: Some(*value),
                        ..Default::default()
                    })
                    .collect(),
            })),
            ..Default::default()
        };
        package("b", vec![mode], Vec::new())
    }

    // A textual reorder of an enum's values moves nothing on the wire, so the
    // unit's verdict is compatible. The fixture has no interface, so no
    // catalog hash is involved here.
    #[test]
    fn an_enum_value_reorder_is_compatible_for_its_unit() {
        let old = vec![unit_b_with_enum(&[("OFF", 0), ("ON", 1)], &[])];
        let new = vec![unit_b_with_enum(&[("ON", 1), ("OFF", 0)], &[])];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Compatible);
    }

    // The same holds for a reordered `reserved` list.
    #[test]
    fn an_enum_reserved_list_reorder_is_compatible_for_its_unit() {
        let old = vec![unit_b_with_enum(&[("OFF", 0)], &[3, 4])];
        let new = vec![unit_b_with_enum(&[("OFF", 0)], &[4, 3])];
        let report = diff_sets(&old, &new);
        assert_eq!(unit_verdict(&report, "b", &old, &new), Verdict::Compatible);
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

    // The walk's path layout is `<package>/<name>[/<member>...]`; the verdicts
    // read it through the accessors, so a change of layout fails here.
    #[test]
    fn a_walked_change_names_its_package_and_declaration() {
        let old = baseline();
        let new = vec![unit_a("b.S"), unit_b(IntWidth::I64), unit_c(IntWidth::I32)];
        let report = diff_sets(&old, &new);
        let change = report
            .changes
            .iter()
            .find(|change| change.package() == "b")
            .expect("the width change in b.S is reported");
        assert_eq!(change.declaration(), Some("S"));
    }

    #[test]
    fn a_change_matches_by_its_structured_package_and_declaration() {
        let old = baseline();
        let new = baseline();
        let mut report = diff_sets(&old, &new);
        report.changes.push(crate::Change {
            path: "b/S/x".to_owned(),
            category: crate::Category::DeclRemoved,
            verdict: Verdict::Breaking,
            before: None,
            after: None,
        });
        // The change names declaration `S` of package `b`, which unit `a`
        // reaches through its signal payload.
        assert_eq!(unit_verdict(&report, "a", &old, &new), Verdict::Breaking);
        assert_eq!(unit_verdict(&report, "c", &old, &new), Verdict::Identical);
    }

    #[test]
    fn one_prepared_value_answers_several_units_in_any_order() {
        let old = baseline();
        let new = vec![unit_a("b.S"), unit_b(IntWidth::I64), unit_c(IntWidth::I32)];
        let report = diff_sets(&old, &new);
        let prepared = super::UnitVerdicts::new(&report, &old, &new);
        assert_eq!(prepared.verdict("c"), Verdict::Identical);
        assert_eq!(prepared.verdict("a"), Verdict::Breaking);
        assert_eq!(prepared.verdict("c"), Verdict::Identical);
        assert_eq!(prepared.verdict("b"), Verdict::Breaking);
        assert_eq!(prepared.verdict("absent"), Verdict::Identical);
        assert_eq!(prepared.verdict("a"), Verdict::Breaking);
    }

    #[test]
    fn a_change_to_a_package_has_no_declaration() {
        let change = |path: &str| crate::Change {
            path: path.to_owned(),
            category: crate::Category::DeclRemoved,
            verdict: Verdict::Breaking,
            before: None,
            after: None,
        };
        assert_eq!(change("veh.cluster").package(), "veh.cluster");
        assert_eq!(change("veh.cluster").declaration(), None);
        assert_eq!(
            change("veh.cluster/Status/doorOpened").package(),
            "veh.cluster"
        );
        assert_eq!(
            change("veh.cluster/Status/doorOpened").declaration(),
            Some("Status")
        );
    }
}
