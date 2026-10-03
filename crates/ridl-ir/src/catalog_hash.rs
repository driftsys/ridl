//! The catalog hash (rsdl note D-8): SHA-256 over the interfaces, their
//! numbers and every type they reach, transitively, wherever declared.
//! Derived, never recorded. The input is the protobuf binary of the reduced
//! package (ADR-0014 decision 15); see that decision for the determinism
//! rule and for why the canonical JSON is not the input.
//!
//! The hash lives in this crate, not in `ridl-descriptor`, because two
//! artifacts carry it: the codegen model's `Catalog.hash`, lowered in
//! [`crate::codegen`], and the catalog descriptor, whose crate re-exports
//! these functions as `ridl_descriptor::hash`.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::v2::{
    Decl, FieldType, Interface, Package, TypeDef, decl, field_type, return_type, stream_type,
    struct_member,
};

/// Every declaration an interface of `package` reaches, keyed by canonical
/// name: bare for this package, `pkg.Name` for another.
pub fn reachable_decls<'a>(
    package: &'a Package,
    others: &[&'a Package],
) -> BTreeMap<String, &'a Decl> {
    let mut index: BTreeMap<String, &'a Decl> = BTreeMap::new();
    for decl in &package.decls {
        index.insert(decl.name.clone(), decl);
    }
    for other in others {
        for decl in &other.decls {
            index.insert(format!("{}.{}", other.name, decl.name), decl);
        }
    }

    let mut pending: Vec<String> = Vec::new();
    for shape in package.shapes() {
        collect_interface(shape.interface, &mut pending);
    }
    let mut reached: BTreeMap<String, &'a Decl> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        // A primitive spelled as a name, or a name the checker already
        // rejected, has no declaration: nothing more to reach.
        let Some(decl) = index.get(&name) else {
            continue;
        };
        reached.insert(name, decl);
        collect_decl(decl, &mut pending);
    }
    reached
}

fn collect_interface(interface: &Interface, out: &mut Vec<String>) {
    for interaction in &interface.interactions {
        collect_decl(interaction, out);
    }
}

/// Pushes every type name `decl` references. The IR references a type by
/// name string in: `SignalDef.payload`, `EventDef.payload`,
/// `FieldType::Named`, `UnionArm.type_ref`, `ConstDef.type_ref`,
/// `EnumSetDef.backing_enum`, `Constraint.pattern_const`,
/// `StreamType::Named` and `FallibleType.ok`/`err`. `Contract.signal_refs`
/// and `Contract.param_refs` name interactions and parameters of the same
/// interface, not types, so they add nothing to the closure.
///
/// The `match` is exhaustive on purpose: an IR variant added later is a
/// compile error here, which is the reminder to decide whether the new
/// variant reaches a type.
fn collect_decl(decl: &Decl, out: &mut Vec<String>) {
    match &decl.kind {
        Some(decl::Kind::TypeDef(def)) => collect_type_def(def, out),
        Some(decl::Kind::ConstDef(def)) => out.extend(def.type_ref.clone()),
        Some(decl::Kind::StructDef(def)) => {
            for member in &def.members {
                if let Some(struct_member::Member::Field(field)) = &member.member
                    && let Some(ty) = &field.r#type
                {
                    collect_field_type(ty, out);
                }
            }
        }
        Some(decl::Kind::EnumDef(_)) | Some(decl::Kind::ReservedSlot(_)) | None => {}
        Some(decl::Kind::EnumSetDef(def)) => out.extend(def.backing_enum.clone()),
        Some(decl::Kind::UnionDef(def)) => {
            out.extend(def.arms.iter().map(|arm| arm.type_ref.clone()))
        }
        Some(decl::Kind::SignalDef(def)) => out.push(def.payload.clone()),
        Some(decl::Kind::EventDef(def)) => out.push(def.payload.clone()),
        Some(decl::Kind::CommandDef(def)) => {
            for param in &def.params {
                if let Some(ty) = &param.r#type {
                    collect_field_type(ty, out);
                }
            }
        }
        Some(decl::Kind::QueryDef(def)) => {
            for param in &def.params {
                if let Some(ty) = &param.r#type {
                    collect_field_type(ty, out);
                }
            }
            match def.return_type.as_ref().and_then(|r| r.kind.as_ref()) {
                Some(return_type::Kind::Value(ty)) => collect_field_type(ty, out),
                Some(return_type::Kind::Fallible(f)) => {
                    out.push(f.ok.clone());
                    out.push(f.err.clone());
                }
                None => {}
            }
        }
        Some(decl::Kind::FixedDef(def)) => {
            if let Some(ty) = &def.payload {
                collect_field_type(ty, out);
            }
        }
    }
}

fn collect_type_def(def: &TypeDef, out: &mut Vec<String>) {
    // `backing.unit` is a UCUM unit expression, not a type reference.
    if let Some(constant) = def
        .constraint
        .as_ref()
        .and_then(|c| c.pattern_const.clone())
    {
        out.push(constant);
    }
}

fn collect_field_type(ty: &FieldType, out: &mut Vec<String>) {
    match &ty.kind {
        Some(field_type::Kind::Named(name)) => out.push(name.clone()),
        Some(field_type::Kind::Primitive(_)) | None => {}
        Some(field_type::Kind::InlineScalar(def)) => collect_type_def(def, out),
        Some(field_type::Kind::Tuple(tuple)) => {
            for field in &tuple.fields {
                if let Some(ty) = &field.r#type {
                    collect_field_type(ty, out);
                }
            }
        }
        Some(field_type::Kind::Array(array)) => {
            if let Some(element) = &array.element {
                collect_field_type(element, out);
            }
        }
        Some(field_type::Kind::Map(map)) => {
            for ty in [&map.key, &map.value].into_iter().flatten() {
                collect_field_type(ty, out);
            }
        }
        Some(field_type::Kind::Stream(stream)) => {
            if let Some(stream_type::Element::Named(name)) = &stream.element {
                out.push(name.clone());
            }
        }
    }
}

/// The exact input of the hash: the package name; every interface shape
/// under its identity name (`Package::shapes()` order) with the owning
/// service's visibility for an inline shape, the IR's `number` and
/// `provisional`, and its interactions; the reached declarations under
/// canonical names, in canonical-name order; doc strings blanked; no
/// services and no retired entries.
pub fn reduced_package(package: &Package, others: &[&Package]) -> Package {
    let mut reduced = Package {
        name: package.name.clone(),
        decls: reachable_decls(package, others)
            .into_iter()
            .map(|(canonical, decl)| {
                let mut decl = decl.clone();
                decl.name = canonical;
                blank_docs(&mut decl);
                decl
            })
            .collect(),
        interfaces: package
            .shapes()
            .map(|shape| {
                let mut interface = shape.interface.clone();
                interface.name = shape.name.to_owned();
                interface.visibility = shape.visibility();
                interface
            })
            .collect(),
        services: vec![],
        retired: vec![],
    };
    for interface in &mut reduced.interfaces {
        interface.doc.clear();
        for interaction in &mut interface.interactions {
            blank_docs(interaction);
        }
    }
    reduced
}

/// SHA-256 over the protobuf binary of [`reduced_package`]. The numbers are
/// inside: each reduced interface carries the IR's `number` and
/// `provisional`.
pub fn catalog_hash(package: &Package, others: &[&Package]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(crate::v2::to_binary(&reduced_package(package, others)));
    hasher.finalize().into()
}

/// Clears every `doc` field inside `decl`. The IR carries a `doc` on
/// `Decl`, `Field`, `EnumValue` (in `EnumDef.values` and `EnumSetDef.bits`)
/// and `UnionArm`; no other message that can occur inside a declaration has
/// one.
fn blank_docs(decl: &mut Decl) {
    decl.doc.clear();
    match &mut decl.kind {
        Some(decl::Kind::StructDef(def)) => {
            for member in &mut def.members {
                if let Some(struct_member::Member::Field(field)) = &mut member.member {
                    field.doc.clear();
                }
            }
        }
        Some(decl::Kind::UnionDef(def)) => {
            for arm in &mut def.arms {
                arm.doc.clear();
            }
        }
        Some(decl::Kind::EnumDef(def)) => {
            for value in &mut def.values {
                value.doc.clear();
            }
        }
        Some(decl::Kind::EnumSetDef(def)) => {
            for bit in &mut def.bits {
                bit.doc.clear();
            }
        }
        Some(decl::Kind::TypeDef(_))
        | Some(decl::Kind::ConstDef(_))
        | Some(decl::Kind::SignalDef(_))
        | Some(decl::Kind::EventDef(_))
        | Some(decl::Kind::CommandDef(_))
        | Some(decl::Kind::QueryDef(_))
        | Some(decl::Kind::FixedDef(_))
        | Some(decl::Kind::ReservedSlot(_))
        | None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::{
        Decl, EnumSetDef, EnumValue, Field, FieldType, Interface, RetiredInterface, Service,
        ServiceShape, SignalDef, StructDef, StructMember, TypeDef, Visibility, decl, field_type,
        service_shape, struct_member,
    };

    fn named(name: &str) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Named(name.to_owned())),
        }
    }

    fn struct_decl(name: &str, field_types: &[&str]) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members: field_types
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| StructMember {
                        member: Some(struct_member::Member::Field(Field {
                            name: format!("f{i}"),
                            ordinal: i as u32 + 1,
                            r#type: Some(named(ty)),
                            ..Default::default()
                        })),
                    })
                    .collect(),
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn scalar_decl(name: &str) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::TypeDef(TypeDef::default())),
            ..Default::default()
        }
    }

    fn signal(name: &str, payload: &str) -> Decl {
        Decl {
            name: name.to_owned(),
            ordinal: 1,
            kind: Some(decl::Kind::SignalDef(SignalDef {
                payload: payload.to_owned(),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// A `service` with an inline body carrying `interactions`; its
    /// `Interface` lives in the shape list, not in `Package::interfaces`.
    fn inline_service(name: &str, interactions: Vec<Decl>) -> Service {
        Service {
            name: name.to_owned(),
            visibility: Visibility::Public as i32,
            shapes: vec![ServiceShape {
                kind: Some(service_shape::Kind::Inline(Interface {
                    interactions,
                    number: 2,
                    provisional: true,
                    ..Default::default()
                })),
            }],
            ..Default::default()
        }
    }

    /// `p`: interface `I` (number 1, provisional) with a signal of `Point`;
    /// `Point` has fields of `Coord` (local) and `fw.Unit` (foreign);
    /// `Unused` is declared, not reached.
    fn fixture() -> (Package, Package) {
        let p = Package {
            name: "p".to_owned(),
            decls: vec![
                struct_decl("Point", &["Coord", "fw.Unit"]),
                scalar_decl("Coord"),
                scalar_decl("Unused"),
            ],
            interfaces: vec![Interface {
                name: "I".to_owned(),
                interactions: vec![signal("pos", "Point")],
                number: 1,
                provisional: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let fw = Package {
            name: "fw".to_owned(),
            decls: vec![scalar_decl("Unit"), scalar_decl("Other")],
            ..Default::default()
        };
        (p, fw)
    }

    fn hash_of(p: &Package, fw: &Package) -> [u8; 32] {
        catalog_hash(p, &[fw])
    }

    #[test]
    fn the_closure_reaches_local_and_foreign_types_and_nothing_else() {
        let (p, fw) = fixture();
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Coord", "Point", "fw.Unit"]);
    }

    #[test]
    fn the_hash_is_stable_across_runs() {
        let (p, fw) = fixture();
        assert_eq!(hash_of(&p, &fw), hash_of(&p, &fw));
    }

    #[test]
    fn an_unreached_declaration_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.decls.retain(|d| d.name != "Unused");
        assert_eq!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_reached_foreign_type_moves_the_hash() {
        let (p, mut fw) = fixture();
        let before = hash_of(&p, &fw);
        fw.decls[0] = struct_decl("Unit", &[]);
        assert_ne!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_doc_comment_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.decls[0].doc = "documented".to_owned();
        p.interfaces[0].doc = "documented".to_owned();
        assert_eq!(hash_of(&p, &fw), before);
    }

    /// Every nested `doc` the IR carries: a struct field's, an enum set
    /// bit's, an interaction's. Each is blanked, so none moves the hash.
    #[test]
    fn a_nested_doc_comment_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        p.decls.push(Decl {
            name: "Flags".to_owned(),
            kind: Some(decl::Kind::EnumSetDef(EnumSetDef {
                bits: vec![EnumValue {
                    name: "a".to_owned(),
                    value: 1,
                    doc: String::new(),
                }],
                ..Default::default()
            })),
            ..Default::default()
        });
        p.interfaces[0].interactions.push(signal("flags", "Flags"));
        let before = hash_of(&p, &fw);

        p.interfaces[0].interactions[0].doc = "documented".to_owned();
        if let Some(decl::Kind::StructDef(def)) = &mut p.decls[0].kind
            && let Some(struct_member::Member::Field(field)) = &mut def.members[0].member
        {
            field.doc = "documented".to_owned();
        }
        if let Some(decl::Kind::EnumSetDef(def)) = &mut p.decls[3].kind {
            def.bits[0].doc = "documented".to_owned();
        }
        assert_eq!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_number_moves_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.interfaces[0].number = 7;
        assert_ne!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_provisional_flag_moves_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.interfaces[0].provisional = false;
        assert_ne!(hash_of(&p, &fw), before);
    }

    #[test]
    fn a_retired_entry_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);
        p.retired.push(RetiredInterface {
            name: "Old".to_owned(),
            number: 9,
        });
        assert_eq!(hash_of(&p, &fw), before);
        assert!(reduced_package(&p, &[&fw]).retired.is_empty());
    }

    #[test]
    fn an_inline_service_shape_reaches_its_types_and_moves_the_hash() {
        let (mut p, fw) = fixture();
        p.interfaces.clear();
        let before = hash_of(&p, &fw);
        p.services
            .push(inline_service("p.hvac", vec![signal("temp", "Point")]));
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Coord", "Point", "fw.Unit"]);
        assert_ne!(hash_of(&p, &fw), before);
    }

    /// The reduced interface of an inline shape carries the owning service's
    /// visibility, so a declared interface and an inline shape hash alike
    /// (driftsys/ridl#326, third minor item).
    #[test]
    fn an_inline_shape_hashes_the_owning_services_visibility() {
        let (mut p, fw) = fixture();
        p.interfaces.clear();
        p.services
            .push(inline_service("p.hvac", vec![signal("temp", "Point")]));
        let before = hash_of(&p, &fw);
        p.services[0].visibility = Visibility::Internal as i32;
        assert_ne!(hash_of(&p, &fw), before);
        let reduced = reduced_package(&p, &[&fw]);
        assert_eq!(reduced.interfaces[0].name, "p.hvac");
        assert_eq!(
            reduced.interfaces[0].visibility,
            Visibility::Internal as i32
        );
    }

    /// Two packages built separately with the same declarations and the same
    /// numbering hash alike: the reduced package orders declarations by
    /// canonical name, so the source order of `decls` does not enter the
    /// hash. The order of the interfaces does: it is `Package::shapes()`
    /// order (ADR-0014 decision 15).
    #[test]
    fn equal_packages_hash_alike_whatever_the_declaration_order() {
        let (p, fw) = fixture();
        let mut reordered = p.clone();
        reordered.decls.reverse();
        let mut fw_reordered = fw.clone();
        fw_reordered.decls.reverse();
        assert_ne!(reordered.decls, p.decls);
        assert_eq!(
            catalog_hash(&reordered, &[&fw_reordered]),
            catalog_hash(&p, &[&fw])
        );
        assert_eq!(
            reduced_package(&reordered, &[&fw_reordered]),
            reduced_package(&p, &[&fw])
        );
    }
}
