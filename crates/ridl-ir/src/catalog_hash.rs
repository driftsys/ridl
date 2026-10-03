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
    Decl, FieldType, Package, TypeDef, decl, field_type, return_type, stream_type, struct_member,
};

/// Every declaration an interface of `package` reaches, keyed by canonical
/// name: bare for this package, `pkg.Name` for another.
pub fn reachable_decls<'a>(
    package: &'a Package,
    others: &[&'a Package],
) -> BTreeMap<String, &'a Decl> {
    let index = Index::new(package, others);
    index
        .closure()
        .into_iter()
        .map(|(canonical, (_, decl))| (canonical, decl))
        .collect()
}

/// The exact input of the hash: the package name; every interface shape
/// under its identity name (`Package::shapes()` order) with the owning
/// service's visibility for an inline shape, the IR's `number` and
/// `provisional`, and its interactions; the reached declarations under
/// canonical names, in canonical-name order, every type reference inside
/// them rewritten to the canonical name of the declaration it resolves to;
/// doc strings and doc tags (`labels`, `deprecated`) blanked; no services
/// and no retired entries.
pub fn reduced_package(package: &Package, others: &[&Package]) -> Package {
    let index = Index::new(package, others);
    let mut reduced = Package {
        name: package.name.clone(),
        decls: index
            .closure()
            .into_iter()
            .map(|(canonical, (owner, decl))| {
                let mut decl = decl.clone();
                decl.name = canonical;
                visit_refs(&mut decl, &mut |name| index.canonicalize(name, owner));
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
        interface.labels.clear();
        interface.deprecated = None;
        for interaction in &mut interface.interactions {
            visit_refs(interaction, &mut |name| index.canonicalize(name, ROOT));
            blank_docs(interaction);
        }
    }
    reduced
}

/// SHA-256 over the protobuf binary of [`reduced_package`]. The numbers are
/// inside: each reduced interface carries the IR's `number` and
/// `provisional`. An entry of `others` named like `package` is ignored, so
/// the hash is the same whether or not the caller includes `package` there.
pub fn catalog_hash(package: &Package, others: &[&Package]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(crate::v2::to_binary(&reduced_package(package, others)));
    hasher.finalize().into()
}

/// The position of the hashed package in [`Index::packages`].
const ROOT: usize = 0;

/// The declarations of every package of the build, for name resolution.
///
/// The IR writes a reference as the bare `Name` when the referenced
/// declaration is in the same package as the referencing one, and as the
/// fully qualified `pkg.Name` otherwise (`ir.proto` header). A bare name is
/// therefore resolved in the package that holds the declaration it was read
/// from, never in the hashed package. Package names contain dots, so a
/// qualified name is resolved by lookup, not by splitting it.
struct Index<'a> {
    /// The hashed package at [`ROOT`], then `others` in the order given,
    /// without an entry named like the hashed package.
    packages: Vec<&'a Package>,
    /// Per package, its declarations by bare name.
    bare: Vec<BTreeMap<&'a str, &'a Decl>>,
    /// Every declaration of every package by `pkg.Name`, with the index of
    /// its package.
    qualified: BTreeMap<String, (usize, &'a Decl)>,
}

impl<'a> Index<'a> {
    fn new(package: &'a Package, others: &[&'a Package]) -> Self {
        // `ridlc build` passes every package of the build as `others`, the
        // hashed package included. A second copy of the hashed package would
        // overwrite its `pkg.Name` entries below, and a declaration reached
        // through both names would be keyed twice, so that copy is skipped.
        let packages: Vec<&'a Package> = std::iter::once(package)
            .chain(
                others
                    .iter()
                    .copied()
                    .filter(|other| other.name != package.name),
            )
            .collect();
        let bare = packages
            .iter()
            .map(|p| p.decls.iter().map(|d| (d.name.as_str(), d)).collect())
            .collect();
        let mut qualified = BTreeMap::new();
        for (i, p) in packages.iter().enumerate() {
            for d in &p.decls {
                qualified.insert(format!("{}.{}", p.name, d.name), (i, d));
            }
        }
        Self {
            packages,
            bare,
            qualified,
        }
    }

    /// The declaration `name` means when read inside a declaration of
    /// package `context`, with its canonical name and its package. `None`
    /// for a primitive spelled as a name or a name the checker rejected.
    fn resolve(&self, name: &str, context: usize) -> Option<(String, usize, &'a Decl)> {
        if let Some(decl) = self.bare[context].get(name) {
            return Some((self.canonical(context, name), context, decl));
        }
        let (owner, decl) = self.qualified.get(name)?;
        Some((self.canonical(*owner, &decl.name), *owner, decl))
    }

    /// The canonical name of declaration `bare` of package `owner`: bare for
    /// the hashed package, `pkg.Name` for another.
    fn canonical(&self, owner: usize, bare: &str) -> String {
        if owner == ROOT {
            bare.to_owned()
        } else {
            format!("{}.{}", self.packages[owner].name, bare)
        }
    }

    /// Rewrites `name`, read in package `context`, to its canonical name. A
    /// name that resolves to nothing stays as written.
    fn canonicalize(&self, name: &mut String, context: usize) {
        if let Some((canonical, _, _)) = self.resolve(name, context) {
            *name = canonical;
        }
    }

    /// Every declaration the hashed package's interface shapes reach,
    /// transitively, keyed by canonical name, with the index of the package
    /// that declares it.
    fn closure(&self) -> BTreeMap<String, (usize, &'a Decl)> {
        // Each pending reference carries the package it was read in.
        let mut pending: Vec<(String, usize)> = Vec::new();
        for shape in self.packages[ROOT].shapes() {
            for interaction in &shape.interface.interactions {
                collect_refs(interaction, ROOT, &mut pending);
            }
        }
        let mut reached: BTreeMap<String, (usize, &'a Decl)> = BTreeMap::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        while let Some((name, context)) = pending.pop() {
            // A primitive spelled as a name, or a name the checker already
            // rejected, has no declaration: nothing more to reach.
            let Some((canonical, owner, decl)) = self.resolve(&name, context) else {
                continue;
            };
            if !seen.insert(canonical.clone()) {
                continue;
            }
            reached.insert(canonical, (owner, decl));
            collect_refs(decl, owner, &mut pending);
        }
        reached
    }
}

/// Pushes every type name `decl` references, each tagged with `context`,
/// the package `decl` belongs to.
fn collect_refs(decl: &Decl, context: usize, out: &mut Vec<(String, usize)>) {
    // The visitor is written once, over `&mut`, so that the rewrite in
    // `reduced_package` and this read share one exhaustive walk; the clone
    // is the price of not writing the walk twice.
    let mut copy = decl.clone();
    visit_refs(&mut copy, &mut |name| out.push((name.clone(), context)));
}

/// Calls `f` on every type reference inside `decl`. The IR references a type
/// by name string in: `SignalDef.payload`, `EventDef.payload`,
/// `FieldType::Named`, `UnionArm.type_ref`, `ConstDef.type_ref`,
/// `EnumSetDef.backing_enum`, `Constraint.pattern_const`,
/// `StreamType::Named` and `FallibleType.ok`/`err`. `Contract.signal_refs`
/// and `Contract.param_refs` name interactions and parameters of the same
/// interface, not types, so they are not visited.
///
/// The `match` is exhaustive on purpose: an IR variant added later is a
/// compile error here, which is the reminder to decide whether the new
/// variant reaches a type.
fn visit_refs(decl: &mut Decl, f: &mut dyn FnMut(&mut String)) {
    match &mut decl.kind {
        Some(decl::Kind::TypeDef(def)) => visit_type_def(def, f),
        Some(decl::Kind::ConstDef(def)) => {
            if let Some(name) = &mut def.type_ref {
                f(name);
            }
        }
        Some(decl::Kind::StructDef(def)) => {
            for member in &mut def.members {
                if let Some(struct_member::Member::Field(field)) = &mut member.member
                    && let Some(ty) = &mut field.r#type
                {
                    visit_field_type(ty, f);
                }
            }
        }
        Some(decl::Kind::EnumDef(_)) | Some(decl::Kind::ReservedSlot(_)) | None => {}
        Some(decl::Kind::EnumSetDef(def)) => {
            if let Some(name) = &mut def.backing_enum {
                f(name);
            }
        }
        Some(decl::Kind::UnionDef(def)) => {
            for arm in &mut def.arms {
                f(&mut arm.type_ref);
            }
        }
        Some(decl::Kind::SignalDef(def)) => f(&mut def.payload),
        Some(decl::Kind::EventDef(def)) => f(&mut def.payload),
        Some(decl::Kind::CommandDef(def)) => {
            for param in &mut def.params {
                if let Some(ty) = &mut param.r#type {
                    visit_field_type(ty, f);
                }
            }
        }
        Some(decl::Kind::QueryDef(def)) => {
            for param in &mut def.params {
                if let Some(ty) = &mut param.r#type {
                    visit_field_type(ty, f);
                }
            }
            match def.return_type.as_mut().and_then(|r| r.kind.as_mut()) {
                Some(return_type::Kind::Value(ty)) => visit_field_type(ty, f),
                Some(return_type::Kind::Fallible(fallible)) => {
                    f(&mut fallible.ok);
                    f(&mut fallible.err);
                }
                None => {}
            }
        }
        Some(decl::Kind::FixedDef(def)) => {
            if let Some(ty) = &mut def.payload {
                visit_field_type(ty, f);
            }
        }
    }
}

fn visit_type_def(def: &mut TypeDef, f: &mut dyn FnMut(&mut String)) {
    // `backing.unit` is a UCUM unit expression, not a type reference.
    if let Some(constant) = def
        .constraint
        .as_mut()
        .and_then(|c| c.pattern_const.as_mut())
    {
        f(constant);
    }
}

fn visit_field_type(ty: &mut FieldType, f: &mut dyn FnMut(&mut String)) {
    match &mut ty.kind {
        Some(field_type::Kind::Named(name)) => f(name),
        Some(field_type::Kind::Primitive(_)) | None => {}
        Some(field_type::Kind::InlineScalar(def)) => visit_type_def(def, f),
        Some(field_type::Kind::Tuple(tuple)) => {
            for field in &mut tuple.fields {
                if let Some(ty) = &mut field.r#type {
                    visit_field_type(ty, f);
                }
            }
        }
        Some(field_type::Kind::Array(array)) => {
            if let Some(element) = &mut array.element {
                visit_field_type(element, f);
            }
        }
        Some(field_type::Kind::Map(map)) => {
            for ty in [&mut map.key, &mut map.value].into_iter().flatten() {
                visit_field_type(ty, f);
            }
        }
        Some(field_type::Kind::Stream(stream)) => {
            if let Some(stream_type::Element::Named(name)) = &mut stream.element {
                f(name);
            }
        }
    }
}

/// Clears every doc string and doc tag inside `decl`: `doc`, `labels` and
/// `deprecated` (typl §14). The IR carries `doc` on `Decl`, `Field`,
/// `EnumValue` (in `EnumDef.values` and `EnumSetDef.bits`) and `UnionArm`,
/// and `labels` and `deprecated` on `Decl` and `Field`; no other message that
/// can occur inside a declaration has one.
fn blank_docs(decl: &mut Decl) {
    decl.doc.clear();
    decl.labels.clear();
    decl.deprecated = None;
    match &mut decl.kind {
        Some(decl::Kind::StructDef(def)) => {
            for member in &mut def.members {
                if let Some(struct_member::Member::Field(field)) = &mut member.member {
                    field.doc.clear();
                    field.labels.clear();
                    field.deprecated = None;
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

    /// `p`: interface `I` with a signal of `Point`; `Point` has one field of
    /// `fw.Unit`; `fw.Unit` has one field of `Coord`, the bare name of a
    /// declaration of `fw`. `p` declares no `Coord`.
    fn foreign_fixture() -> (Package, Package) {
        let p = Package {
            name: "p".to_owned(),
            decls: vec![struct_decl("Point", &["fw.Unit"])],
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
            decls: vec![
                struct_decl("Unit", &["Coord"]),
                scalar_decl("Coord"),
                scalar_decl("Other"),
            ],
            ..Default::default()
        };
        (p, fw)
    }

    fn hash_of(p: &Package, fw: &Package) -> [u8; 32] {
        catalog_hash(p, &[fw])
    }

    fn field_type_names(decl: &Decl) -> Vec<String> {
        let Some(decl::Kind::StructDef(def)) = &decl.kind else {
            panic!("{} is not a struct", decl.name);
        };
        def.members
            .iter()
            .filter_map(|m| match &m.member {
                Some(struct_member::Member::Field(field)) => match &field.r#type.as_ref()?.kind {
                    Some(field_type::Kind::Named(name)) => Some(name.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
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

    /// A bare name inside a foreign declaration means a declaration of that
    /// foreign package (`ir.proto` header), so `fw.Unit`'s field of `Coord`
    /// reaches `fw.Coord`, and the reduced `fw.Unit` names it canonically.
    #[test]
    fn a_bare_name_in_a_foreign_declaration_resolves_in_its_own_package() {
        let (p, mut fw) = foreign_fixture();
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Point", "fw.Coord", "fw.Unit"]);

        let reduced = reduced_package(&p, &[&fw]);
        let unit = reduced.decls.iter().find(|d| d.name == "fw.Unit").unwrap();
        assert_eq!(field_type_names(unit), vec!["fw.Coord"]);

        let before = hash_of(&p, &fw);
        fw.decls[1] = struct_decl("Coord", &[]);
        assert_ne!(hash_of(&p, &fw), before);
    }

    /// The hashed package's own `Coord` is not what `fw.Unit`'s bare `Coord`
    /// means, so it stays unreached and a change to it does not move the
    /// hash.
    #[test]
    fn a_bare_name_in_a_foreign_declaration_does_not_pick_the_root_packages_homonym() {
        let (mut p, fw) = foreign_fixture();
        p.decls.push(scalar_decl("Coord"));
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Point", "fw.Coord", "fw.Unit"]);

        let reduced = reduced_package(&p, &[&fw]);
        let unit = reduced.decls.iter().find(|d| d.name == "fw.Unit").unwrap();
        assert_eq!(field_type_names(unit), vec!["fw.Coord"]);

        let before = hash_of(&p, &fw);
        p.decls[1] = struct_decl("Coord", &[]);
        assert_eq!(hash_of(&p, &fw), before);
    }

    /// A qualified reference back into the hashed package, from a foreign
    /// declaration or from the package's own interaction, is canonical as
    /// the bare name.
    #[test]
    fn a_qualified_reference_to_the_root_package_is_canonical_as_bare() {
        let (mut p, mut fw) = foreign_fixture();
        p.decls.push(scalar_decl("X"));
        p.interfaces[0].interactions.push(signal("x", "p.X"));
        fw.decls[0] = struct_decl("Unit", &["p.X"]);
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Point", "X", "fw.Unit"]);

        let reduced = reduced_package(&p, &[&fw]);
        let unit = reduced.decls.iter().find(|d| d.name == "fw.Unit").unwrap();
        assert_eq!(field_type_names(unit), vec!["X"]);
        let Some(decl::Kind::SignalDef(def)) = &reduced.interfaces[0].interactions[1].kind else {
            panic!("not a signal");
        };
        assert_eq!(def.payload, "X");
    }

    /// A name that resolves to nothing — a primitive spelled as a name, or a
    /// name the checker rejected — stays as written.
    #[test]
    fn an_unresolved_name_stays_as_written() {
        let (mut p, fw) = fixture();
        p.decls[0] = struct_decl("Point", &["u32", "nowhere.Missing"]);
        let reached: Vec<String> = reachable_decls(&p, &[&fw]).into_keys().collect();
        assert_eq!(reached, vec!["Point"]);
        let reduced = reduced_package(&p, &[&fw]);
        assert_eq!(
            field_type_names(&reduced.decls[0]),
            vec!["u32", "nowhere.Missing"]
        );
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

    /// `@labels` and `@deprecated` are doc tags (typl §14), blanked like doc
    /// strings on a declaration, a struct field, an interaction and an
    /// interface.
    #[test]
    fn a_doc_tag_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        let before = hash_of(&p, &fw);

        p.decls[0].labels.push("tagged".to_owned());
        p.decls[0].deprecated = Some("use Point2".to_owned());
        if let Some(decl::Kind::StructDef(def)) = &mut p.decls[0].kind
            && let Some(struct_member::Member::Field(field)) = &mut def.members[0].member
        {
            field.labels.push("tagged".to_owned());
            field.deprecated = Some("use f9".to_owned());
        }
        p.interfaces[0].labels.push("tagged".to_owned());
        p.interfaces[0].deprecated = Some("use J".to_owned());
        p.interfaces[0].interactions[0]
            .labels
            .push("tagged".to_owned());
        p.interfaces[0].interactions[0].deprecated = Some("use pos2".to_owned());
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

    /// `ridlc build` passes every package of the build as `others`, so the
    /// hashed package is among its own `others`. That copy is skipped: a
    /// reference back to the hashed package written `p.Coord` (from inside
    /// `fw`) still resolves to the bare canonical name `Coord`, so `Coord`
    /// is reached once and the hash equals the hash without the copy.
    #[test]
    fn the_hashed_package_among_others_is_skipped() {
        let p = Package {
            name: "p".to_owned(),
            decls: vec![
                struct_decl("Point", &["Coord", "fw.Unit"]),
                scalar_decl("Coord"),
            ],
            interfaces: vec![Interface {
                name: "I".to_owned(),
                interactions: vec![signal("pos", "Point")],
                number: 1,
                ..Default::default()
            }],
            ..Default::default()
        };
        let fw = Package {
            name: "fw".to_owned(),
            decls: vec![struct_decl("Unit", &["p.Coord"])],
            ..Default::default()
        };
        let reached: Vec<String> = reachable_decls(&p, &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, ["Coord", "Point", "fw.Unit"]);
        assert_eq!(reduced_package(&p, &[&p, &fw]), reduced_package(&p, &[&fw]));
        assert_eq!(catalog_hash(&p, &[&p, &fw]), catalog_hash(&p, &[&fw]));
    }
}
