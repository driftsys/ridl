//! The catalog hash (rsdl note D-8): SHA-256 over the interfaces of one
//! unit, their numbers and every type they reach, transitively, wherever
//! declared. Derived, never recorded. The input is the protobuf binary of
//! the reduced unit (ADR-0014 decision 15); see that decision for the
//! determinism rule and for why the canonical JSON is not the input.
//!
//! A unit is one package manifest and the source packages in its directory
//! tree (`Package.unit`). Every function here takes the unit name and every
//! package of the build, and selects the unit's packages by
//! [`crate::v2::unit_of`], never by a name prefix: a unit named `u.x` is
//! not part of the unit `u`.
//!
//! The hash lives in this crate, not in `ridl-descriptor`, because three
//! artifacts carry it: the codegen model's `Catalog.hash`, lowered in
//! [`crate::codegen`]; the catalog descriptor, whose crate re-exports these
//! functions as `ridl_descriptor::hash`; and each `Region` of the lowered rsdl
//! system, where `ridlc` embeds it.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::v2::{
    Decl, DocLink, FieldType, Package, Param, TypeDef, decl, field_type, members_of_unit,
    return_type, stream_type, struct_member,
};

/// Every declaration an interface shape of `unit` reaches, in any package
/// of `packages`, keyed by canonical name (`pkg.Name`).
pub fn reachable_decls<'a>(unit: &str, packages: &[&'a Package]) -> BTreeMap<String, &'a Decl> {
    let index = Index::new(unit, packages);
    index
        .closure()
        .into_iter()
        .map(|(canonical, (_, decl))| (canonical, decl))
        .collect()
}

/// The exact input of the hash, one IR package named after the unit: every
/// interface shape of every package of the unit under its catalog name
/// (`Package::catalog_name`: the declared name relative to the unit, or the
/// owning service's dotted global name for an inline shape), with the
/// owning service's visibility for an inline shape, the IR's `number` and
/// `provisional`, and its interactions, in (number, name) order because the
/// lock makes the number the identity; the reached declarations under their
/// full canonical names (`pkg.Name`, for the unit's own packages too, since
/// two packages of one unit can declare the same short name), in
/// canonical-name order, every type reference inside them rewritten to the
/// canonical name of the declaration it resolves to, and every expression
/// string left as written; doc strings, doc links and doc tags (`labels`,
/// `deprecated`, `see`, `since`) blanked, a parameter's included; no
/// services and no retired entries.
///
/// `packages` is every package of the build; a package named twice is read
/// once (see [`Index`]).
pub fn reduced_unit(unit: &str, packages: &[&Package]) -> Package {
    let index = Index::new(unit, packages);
    let mut reduced = Package {
        name: unit.to_owned(),
        unit: unit.to_owned(),
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
        interfaces: vec![],
        services: vec![],
        retired: vec![],
    };
    // Each shape's interaction references are read in the package that
    // declares the shape, so the owner travels with the interface until the
    // references are rewritten.
    let mut interfaces: Vec<(usize, _)> = index
        .members()
        .flat_map(|(owner, package)| {
            package.shapes().map(move |shape| {
                let mut interface = shape.interface.clone();
                interface.name = package.catalog_name(&shape);
                interface.visibility = shape.visibility();
                (owner, interface)
            })
        })
        .collect();
    for (owner, interface) in &mut interfaces {
        interface.doc.clear();
        interface.labels.clear();
        interface.deprecated = None;
        interface.links.clear();
        interface.see.clear();
        interface.since.clear();
        for interaction in &mut interface.interactions {
            visit_refs(interaction, &mut |name| index.canonicalize(name, *owner));
            blank_docs(interaction);
        }
    }
    interfaces.sort_by(|(_, a), (_, b)| (a.number, &a.name).cmp(&(b.number, &b.name)));
    reduced.interfaces = interfaces
        .into_iter()
        .map(|(_, interface)| interface)
        .collect();
    reduced
}

/// SHA-256 over the protobuf binary of [`reduced_unit`]. The numbers are
/// inside: each reduced interface carries the IR's `number` and
/// `provisional`. A package named twice in `packages` is read once, so the
/// hash is the same whether or not the caller lists a package of the unit
/// twice.
pub fn catalog_hash(unit: &str, packages: &[&Package]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(crate::v2::to_binary(&reduced_unit(unit, packages)));
    hasher.finalize().into()
}

/// The declarations of every package of the build, for name resolution.
///
/// The IR writes a reference as the bare `Name` when the referenced
/// declaration is in the same package as the referencing one, and as the
/// fully qualified `pkg.Name` otherwise (`ir.proto` header). A bare name is
/// therefore resolved in the package that holds the declaration it was read
/// from, never in the unit's root package. Package names contain dots, so a
/// qualified name is resolved by lookup, not by splitting it.
struct Index<'a> {
    /// The unit's packages first, then every other package, each in the
    /// order given, with one entry per package name. `ridlc`'s
    /// `catalog_scope` holds every package of the build, so a caller that
    /// also passes the unit's own package names it twice; the first entry
    /// with a name is kept, and a second copy would otherwise overwrite its
    /// `pkg.Name` entries and list its shapes twice.
    packages: Vec<&'a Package>,
    /// How many leading entries of `packages` belong to the unit.
    member_count: usize,
    /// Per package, its declarations by bare name.
    bare: Vec<BTreeMap<&'a str, &'a Decl>>,
    /// Every declaration of every package by `pkg.Name`, with the index of
    /// its package.
    qualified: BTreeMap<String, (usize, &'a Decl)>,
}

impl<'a> Index<'a> {
    fn new(unit: &str, all: &[&'a Package]) -> Self {
        let mut packages: Vec<&'a Package> = Vec::new();
        for package in members_of_unit(unit, all) {
            push_once(&mut packages, package);
        }
        let member_count = packages.len();
        for package in all {
            push_once(&mut packages, package);
        }
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
            member_count,
            bare,
            qualified,
        }
    }

    /// The unit's packages, each with its position in [`Self::packages`].
    fn members(&self) -> impl Iterator<Item = (usize, &'a Package)> + '_ {
        self.packages[..self.member_count]
            .iter()
            .copied()
            .enumerate()
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

    /// The canonical name of declaration `bare` of package `owner`:
    /// `pkg.Name`, for a package of the unit too.
    fn canonical(&self, owner: usize, bare: &str) -> String {
        format!("{}.{}", self.packages[owner].name, bare)
    }

    /// Rewrites `name`, read in package `context`, to its canonical name. A
    /// name that resolves to nothing stays as written.
    fn canonicalize(&self, name: &mut String, context: usize) {
        if let Some((canonical, _, _)) = self.resolve(name, context) {
            *name = canonical;
        }
    }

    /// Every declaration the unit's interface shapes reach, transitively,
    /// keyed by canonical name, with the index of the package that declares
    /// it.
    fn closure(&self) -> BTreeMap<String, (usize, &'a Decl)> {
        // Each pending reference carries the package it was read in and
        // where it was read.
        let mut pending: Vec<Pending> = Vec::new();
        for (owner, package) in self.members() {
            for shape in package.shapes() {
                for interaction in &shape.interface.interactions {
                    collect_refs(interaction, owner, &mut pending);
                }
            }
        }
        let mut reached: BTreeMap<String, (usize, &'a Decl)> = BTreeMap::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        while let Some((name, context, origin)) = pending.pop() {
            // A primitive spelled as a name, or a name the checker already
            // rejected, has no declaration: nothing more to reach.
            let found = match origin {
                Origin::TypeRef => self.resolve(&name, context).into_iter().collect(),
                Origin::Expr => self.resolve_expr(&name, context),
            };
            for (canonical, owner, decl) in found {
                if !seen.insert(canonical.clone()) {
                    continue;
                }
                reached.insert(canonical, (owner, decl));
                collect_refs(decl, owner, &mut pending);
            }
        }
        reached
    }

    /// The declarations a name read inside an expression string of package
    /// `context` may mean. A type reference is canonical in the IR, but an
    /// expression is source text: it names a declaration of another package
    /// by the name the file's import binds, and the contract checker accepts
    /// only that form (it rejects a qualified `pkg.Name`, expr-core §3.1).
    /// The IR records no imports, so a bare name that `context` does not
    /// declare is looked up in every other package of the build, and every
    /// match is returned. That can include a declaration the expression does
    /// not mean, which only widens what the hash covers. An import alias
    /// (`import a.B as C`, typl §3.2) binds a name no package declares, so a
    /// declaration named through an alias is not found.
    fn resolve_expr(&self, name: &str, context: usize) -> Vec<(String, usize, &'a Decl)> {
        if let Some(found) = self.resolve(name, context) {
            return vec![found];
        }
        if name.contains('.') {
            return vec![];
        }
        self.bare
            .iter()
            .enumerate()
            .filter_map(|(owner, decls)| {
                let decl = decls.get(name)?;
                Some((self.canonical(owner, name), owner, *decl))
            })
            .collect()
    }
}

/// Appends `package` to `packages` unless a package of that name is there.
fn push_once<'a>(packages: &mut Vec<&'a Package>, package: &'a Package) {
    if !packages.iter().any(|known| known.name == package.name) {
        packages.push(package);
    }
}

/// A name waiting to be resolved: the name, the package it was read in, and
/// where it was read.
type Pending = (String, usize, Origin);

/// Where a pending name was read, which decides how it resolves.
#[derive(Clone, Copy)]
enum Origin {
    /// A type reference field: canonical, resolved by [`Index::resolve`].
    TypeRef,
    /// A name inside an expression string: resolved by
    /// [`Index::resolve_expr`].
    Expr,
}

/// Pushes every type name `decl` references, and every name chain inside
/// its expression strings, each tagged with `context`, the package `decl`
/// belongs to.
fn collect_refs(decl: &Decl, context: usize, out: &mut Vec<Pending>) {
    // The visitor is written once, over `&mut`, so that the rewrite in
    // `reduced_unit` and this read share one exhaustive walk; the clone
    // is the price of not writing the walk twice.
    let mut copy = decl.clone();
    visit_refs(&mut copy, &mut |name| {
        out.push((name.clone(), context, Origin::TypeRef));
    });
    visit_exprs(decl, &mut |source| {
        expr_names(source, &mut |name| {
            out.push((name.to_owned(), context, Origin::Expr));
        });
    });
}

/// Calls `f` on every expression string inside `decl` that can name a
/// constant or an enum: the `source` of every `Contract` of a command or a
/// query, and `ConstDef.value`. These strings are hashed as written and
/// never rewritten; the names inside them are only followed, so the
/// declarations they name enter the closure.
///
/// `ConstDef.value` normally holds a value: `lower_const` follows a chain
/// of constants (`const B : Level = A`) and stores the resolved value. It
/// stores the written name only when the chain does not resolve to a
/// value: a cycle (`const A : integer = B` with `const B : integer = A`,
/// which `ridl check` accepts) or an unknown name. The arm follows that
/// name, so the other constants of the cycle are hashed too.
///
/// The IR's other value strings hold resolved values, not names: a
/// constant named in a declared init is lowered as its value (`declared_init`
/// and `InitValue.value` on `TypeDef`, `Field` and `SignalDef`), and range
/// bounds, steps and timing bounds are canonical decimals. A change to the
/// constant changes those strings, so they need no following.
/// `Constraint.pattern_const` is a reference, visited by [`visit_refs`].
///
/// The `match` is exhaustive for the same reason as in [`visit_refs`].
fn visit_exprs(decl: &Decl, f: &mut dyn FnMut(&str)) {
    match &decl.kind {
        Some(decl::Kind::ConstDef(def)) => f(&def.value),
        Some(decl::Kind::CommandDef(def)) => {
            for contract in &def.contracts {
                f(&contract.source);
            }
        }
        Some(decl::Kind::QueryDef(def)) => {
            for contract in &def.contracts {
                f(&contract.source);
            }
        }
        Some(decl::Kind::TypeDef(_))
        | Some(decl::Kind::StructDef(_))
        | Some(decl::Kind::EnumDef(_))
        | Some(decl::Kind::EnumSetDef(_))
        | Some(decl::Kind::UnionDef(_))
        | Some(decl::Kind::SignalDef(_))
        | Some(decl::Kind::EventDef(_))
        | Some(decl::Kind::FixedDef(_))
        | Some(decl::Kind::ReservedSlot(_))
        | None => {}
    }
}

/// Calls `f` on every name chain in the expression text `source` (identifiers
/// `[A-Za-z_][A-Za-z0-9_]*` joined by `.`) and on every dotted prefix of
/// each chain: `Mode.off` yields `Mode` and `Mode.off`, and `a.b.MAX` yields
/// `a`, `a.b` and `a.b.MAX`, because a package name contains dots and an
/// enum value is written after its enum's name. The caller keeps what
/// [`Index::resolve_expr`] resolves: a name the declaring package holds,
/// a qualified name, or else a bare name any other package of the build
/// declares, which is how an imported declaration is named.
///
/// A string literal (`"..."`, RFC 8259 escapes, typl §2.6) is skipped, and so
/// is a number with its suffix or exponent (`1e3`, `10ms`). Anything else is
/// scanned, so a parameter or a field named like a declaration also reaches
/// that declaration. That over-inclusion only widens what the hash covers.
/// A declaration named through an import alias is not reached (see
/// [`Index::resolve_expr`]).
fn expr_names(source: &str, f: &mut dyn FnMut(&str)) {
    let bytes = source.as_bytes();
    let is_start = |b: u8| b.is_ascii_alphabetic() || b == b'_';
    let is_part = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if b.is_ascii_digit() {
            while i < bytes.len()
                && (is_part(bytes[i])
                    || (bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)))
            {
                i += 1;
            }
        } else if is_start(b) {
            let start = i;
            loop {
                while i < bytes.len() && is_part(bytes[i]) {
                    i += 1;
                }
                f(&source[start..i]);
                if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(|&b| is_start(b)) {
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            i += 1;
        }
    }
}

/// Calls `f` on every type reference inside `decl`. The IR references a type
/// by name string in: `SignalDef.payload`, `EventDef.payload`,
/// `FieldType::Named`, `UnionArm.type_ref`, `ConstDef.type_ref`,
/// `EnumSetDef.backing_enum`, `Constraint.pattern_const`,
/// `StreamType::Named` and `FallibleType.ok`/`err`. `Contract.signal_refs`
/// and `Contract.param_refs` name interactions and parameters of the same
/// interface, not types, so they are not visited. Names inside expression
/// strings (`Contract.source`, `ConstDef.value`) are followed by
/// [`visit_exprs`] and never rewritten.
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

/// Clears every doc field inside `decl` (typl §14, ADR-0026): `doc`, `labels`,
/// `deprecated`, `links`, `see` and `since`. The IR carries `doc`, `links`,
/// `see` and `since` on every doc carrier inside a declaration — `Decl`,
/// `Field`, `EnumValue` (in `EnumDef.values` and `EnumSetDef.bits`),
/// `UnionArm` and `Param` — and `labels` and `deprecated` on `Decl` and
/// `Field`; no other message that can occur inside a declaration has one.
fn blank_docs(decl: &mut Decl) {
    decl.doc.clear();
    decl.labels.clear();
    decl.deprecated = None;
    blank_doc_links(&mut decl.links, &mut decl.see, &mut decl.since);
    match &mut decl.kind {
        Some(decl::Kind::StructDef(def)) => {
            for member in &mut def.members {
                if let Some(struct_member::Member::Field(field)) = &mut member.member {
                    field.doc.clear();
                    field.labels.clear();
                    field.deprecated = None;
                    blank_doc_links(&mut field.links, &mut field.see, &mut field.since);
                }
            }
        }
        Some(decl::Kind::UnionDef(def)) => {
            for arm in &mut def.arms {
                arm.doc.clear();
                blank_doc_links(&mut arm.links, &mut arm.see, &mut arm.since);
            }
        }
        Some(decl::Kind::EnumDef(def)) => {
            for value in &mut def.values {
                value.doc.clear();
                blank_doc_links(&mut value.links, &mut value.see, &mut value.since);
            }
        }
        Some(decl::Kind::EnumSetDef(def)) => {
            for bit in &mut def.bits {
                bit.doc.clear();
                blank_doc_links(&mut bit.links, &mut bit.see, &mut bit.since);
            }
        }
        Some(decl::Kind::CommandDef(def)) => blank_param_docs(&mut def.params),
        Some(decl::Kind::QueryDef(def)) => blank_param_docs(&mut def.params),
        Some(decl::Kind::TypeDef(_))
        | Some(decl::Kind::ConstDef(_))
        | Some(decl::Kind::SignalDef(_))
        | Some(decl::Kind::EventDef(_))
        | Some(decl::Kind::FixedDef(_))
        | Some(decl::Kind::ReservedSlot(_))
        | None => {}
    }
}

fn blank_param_docs(params: &mut [Param]) {
    for param in params {
        param.doc.clear();
        blank_doc_links(&mut param.links, &mut param.see, &mut param.since);
    }
}

/// Clears the three doc fields every doc carrier shares besides `doc`
/// (ADR-0026). Taken as three borrows because the carriers are unrelated
/// generated structs with no shared trait.
fn blank_doc_links(links: &mut Vec<DocLink>, see: &mut Vec<DocLink>, since: &mut Vec<String>) {
    links.clear();
    see.clear();
    since.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::{
        ArrayType, CommandDef, ConstDef, Constraint, Contract, ContractKind, Decl, DocLink,
        EnumDef, EnumSetDef, EnumValue, EventDef, FallibleType, Field, FieldType, FixedDef,
        Interface, MapType, Param, QueryDef, RetiredInterface, ReturnType, Service, ServiceShape,
        SignalDef, StreamType, StructDef, StructMember, TupleField, TupleType, TypeDef, UnionArm,
        UnionDef, Visibility, decl, field_type, return_type, service_shape, stream_type,
        struct_member,
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
                        member: Some(struct_member::Member::Field(Box::new(Field {
                            name: format!("f{i}"),
                            ordinal: i as u32 + 1,
                            r#type: Some(named(ty)),
                            ..Default::default()
                        }))),
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
        catalog_hash("p", &[p, fw])
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
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.Unit", "p.Coord", "p.Point"]);
    }

    /// `ridlc`'s `catalog_scope` relies on this: the order of `others` does
    /// not change the hash, because every declaration is keyed by its
    /// qualified name.
    #[test]
    fn the_order_of_the_other_packages_does_not_move_the_hash() {
        let p = Package {
            name: "p".to_owned(),
            decls: vec![struct_decl("Point", &["fa.A", "fb.B"])],
            interfaces: vec![Interface {
                name: "I".to_owned(),
                interactions: vec![signal("pos", "Point")],
                number: 1,
                provisional: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let fa = Package {
            name: "fa".to_owned(),
            decls: vec![scalar_decl("A")],
            ..Default::default()
        };
        let fb = Package {
            name: "fb".to_owned(),
            decls: vec![scalar_decl("B")],
            ..Default::default()
        };
        let reached: Vec<String> = reachable_decls("p", &[&p, &fa, &fb]).into_keys().collect();
        assert_eq!(reached, vec!["fa.A", "fb.B", "p.Point"]);
        assert_eq!(
            catalog_hash("p", &[&p, &fa, &fb]),
            catalog_hash("p", &[&p, &fb, &fa])
        );
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
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.Coord", "fw.Unit", "p.Point"]);

        let reduced = reduced_unit("p", &[&p, &fw]);
        let unit = reduced.decls.iter().find(|d| d.name == "fw.Unit").unwrap();
        assert_eq!(field_type_names(unit), vec!["fw.Coord"]);

        let before = hash_of(&p, &fw);
        fw.decls[1] = struct_decl("Coord", &[]);
        assert_ne!(hash_of(&p, &fw), before);
    }

    /// The unit's own `Coord` is not what `fw.Unit`'s bare `Coord` means, so
    /// it stays unreached and a change to it does not move the hash.
    #[test]
    fn a_bare_name_in_a_foreign_declaration_does_not_pick_the_units_homonym() {
        let (mut p, fw) = foreign_fixture();
        p.decls.push(scalar_decl("Coord"));
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.Coord", "fw.Unit", "p.Point"]);

        let reduced = reduced_unit("p", &[&p, &fw]);
        let unit = reduced.decls.iter().find(|d| d.name == "fw.Unit").unwrap();
        assert_eq!(field_type_names(unit), vec!["fw.Coord"]);

        let before = hash_of(&p, &fw);
        p.decls[1] = struct_decl("Coord", &[]);
        assert_eq!(hash_of(&p, &fw), before);
    }

    /// A reference into a package of the unit, qualified from a foreign
    /// declaration or bare from the package's own interaction, is canonical
    /// as the qualified name: the unit's own declarations carry `pkg.Name`
    /// like every other.
    #[test]
    fn a_reference_to_a_package_of_the_unit_is_canonical_as_qualified() {
        let (mut p, mut fw) = foreign_fixture();
        p.decls.push(scalar_decl("X"));
        p.interfaces[0].interactions.push(signal("x", "p.X"));
        fw.decls[0] = struct_decl("Unit", &["p.X"]);
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.Unit", "p.Point", "p.X"]);

        let reduced = reduced_unit("p", &[&p, &fw]);
        let unit = reduced.decls.iter().find(|d| d.name == "fw.Unit").unwrap();
        assert_eq!(field_type_names(unit), vec!["p.X"]);
        let Some(decl::Kind::SignalDef(def)) = &reduced.interfaces[0].interactions[1].kind else {
            panic!("not a signal");
        };
        assert_eq!(def.payload, "p.X");
    }

    /// A name that resolves to nothing — a primitive spelled as a name, or a
    /// name the checker rejected — stays as written.
    #[test]
    fn an_unresolved_name_stays_as_written() {
        let (mut p, fw) = fixture();
        p.decls[0] = struct_decl("Point", &["u32", "nowhere.Missing"]);
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["p.Point"]);
        let reduced = reduced_unit("p", &[&p, &fw]);
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
                    links: Vec::new(),
                    see: Vec::new(),
                    since: Vec::new(),
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
        assert!(reduced_unit("p", &[&p, &fw]).retired.is_empty());
    }

    #[test]
    fn an_inline_service_shape_reaches_its_types_and_moves_the_hash() {
        let (mut p, fw) = fixture();
        p.interfaces.clear();
        let before = hash_of(&p, &fw);
        p.services
            .push(inline_service("p.hvac", vec![signal("temp", "Point")]));
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.Unit", "p.Coord", "p.Point"]);
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
        let reduced = reduced_unit("p", &[&p, &fw]);
        assert_eq!(reduced.interfaces[0].name, "p.hvac");
        assert_eq!(
            reduced.interfaces[0].visibility,
            Visibility::Internal as i32
        );
    }

    /// Two packages built separately with the same declarations and the same
    /// numbering hash alike: the reduced unit orders declarations by
    /// canonical name and interfaces by (number, name), so the source order
    /// of neither enters the hash (ADR-0014 decision 15).
    #[test]
    fn equal_packages_hash_alike_whatever_the_declaration_order() {
        let (mut p, fw) = fixture();
        p.interfaces.push(Interface {
            name: "J".to_owned(),
            interactions: vec![signal("pos", "Point")],
            number: 2,
            ..Default::default()
        });
        let mut reordered = p.clone();
        reordered.decls.reverse();
        reordered.interfaces.reverse();
        let mut fw_reordered = fw.clone();
        fw_reordered.decls.reverse();
        assert_ne!(reordered.decls, p.decls);
        assert_ne!(reordered.interfaces, p.interfaces);
        assert_eq!(
            catalog_hash("p", &[&reordered, &fw_reordered]),
            catalog_hash("p", &[&p, &fw])
        );
        assert_eq!(
            reduced_unit("p", &[&reordered, &fw_reordered]),
            reduced_unit("p", &[&p, &fw])
        );
    }

    /// `ridlc build` passes every package of the build, and the codegen
    /// lowering lists the package before that scope, so a package of the
    /// unit can be named twice. The second copy is skipped: its shapes are
    /// listed once, `p.Coord` is reached once, and the hash equals the hash
    /// without the copy.
    #[test]
    fn a_package_named_twice_is_read_once() {
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
        let reached: Vec<String> = reachable_decls("p", &[&p, &p, &fw]).into_keys().collect();
        assert_eq!(reached, ["fw.Unit", "p.Coord", "p.Point"]);
        assert_eq!(
            reduced_unit("p", &[&p, &p, &fw]),
            reduced_unit("p", &[&p, &fw])
        );
        assert_eq!(
            catalog_hash("p", &[&p, &p, &fw]),
            catalog_hash("p", &[&p, &fw])
        );
    }

    fn const_decl(name: &str, type_ref: &str, value: &str) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::ConstDef(ConstDef {
                type_ref: Some(type_ref.to_owned()),
                value: value.to_owned(),
                regex: None,
            })),
            ..Default::default()
        }
    }

    /// A command `set(level: Level)` with one `require` clause.
    fn guarded_command(source: &str) -> Decl {
        Decl {
            name: "set".to_owned(),
            ordinal: 1,
            kind: Some(decl::Kind::CommandDef(CommandDef {
                params: vec![Param {
                    name: "level".to_owned(),
                    r#type: Some(named("Level")),
                    doc: String::new(),
                    links: Vec::new(),
                    see: Vec::new(),
                    since: Vec::new(),
                }],
                contracts: vec![Contract {
                    kind: ContractKind::Require as i32,
                    source: source.to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// `p`: interface `I` with the command `set(level: Level)` guarded by
    /// `require <source>`; `MAX : Level = 100` is declared.
    fn guarded_fixture(source: &str) -> Package {
        Package {
            name: "p".to_owned(),
            decls: vec![scalar_decl("Level"), const_decl("MAX", "Level", "100")],
            interfaces: vec![Interface {
                name: "I".to_owned(),
                interactions: vec![guarded_command(source)],
                number: 1,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn set_const_value(package: &mut Package, name: &str, value: &str) {
        let decl = package.decls.iter_mut().find(|d| d.name == name).unwrap();
        let Some(decl::Kind::ConstDef(def)) = &mut decl.kind else {
            panic!("{name} is not a constant");
        };
        def.value = value.to_owned();
    }

    /// A constant named only inside a contract clause is reached, so a change
    /// to its value moves the hash.
    #[test]
    fn a_constant_named_in_a_contract_clause_is_reached() {
        let mut p = guarded_fixture("level < MAX");
        let reached: Vec<String> = reachable_decls("p", &[&p]).into_keys().collect();
        assert_eq!(reached, vec!["p.Level", "p.MAX"]);
        let before = catalog_hash("p", &[&p]);
        set_const_value(&mut p, "MAX", "200");
        assert_ne!(catalog_hash("p", &[&p]), before);
    }

    /// A constant whose `value` is the name of another constant reaches it.
    /// The IR built here is what `lower_const` writes when it cannot follow
    /// a constant chain to a value: a cycle (`const A : integer = B` with
    /// `const B : integer = A`, which `ridl check` accepts) or an unknown
    /// name. A resolvable chain is lowered as the resolved value instead.
    #[test]
    fn a_constant_named_in_a_constant_value_is_reached() {
        let mut p = guarded_fixture("level < LIMIT");
        p.decls.push(const_decl("LIMIT", "Level", "MAX"));
        let reached: Vec<String> = reachable_decls("p", &[&p]).into_keys().collect();
        assert_eq!(reached, vec!["p.LIMIT", "p.Level", "p.MAX"]);
        let before = catalog_hash("p", &[&p]);
        set_const_value(&mut p, "MAX", "200");
        assert_ne!(catalog_hash("p", &[&p]), before);
    }

    fn enum_decl(name: &str) -> Decl {
        Decl {
            name: name.to_owned(),
            kind: Some(decl::Kind::EnumDef(EnumDef::default())),
            ..Default::default()
        }
    }

    /// Every dotted prefix of a name chain is followed, in the form the
    /// contract checker accepts (expr-core §3.1): an enum member access
    /// `Mode.OFF` reaches the local enum `Mode`, and `Gear.PARK`, with `Gear`
    /// imported from `fw` under its own name, reaches `fw.Gear`.
    #[test]
    fn a_dotted_name_in_an_expression_reaches_its_declarations() {
        let mut p = guarded_fixture("mode == Mode.OFF && gear != Gear.PARK");
        p.decls.push(enum_decl("Mode"));
        let fw = Package {
            name: "fw".to_owned(),
            decls: vec![enum_decl("Gear")],
            ..Default::default()
        };
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.Gear", "p.Level", "p.Mode"]);
    }

    /// A contract names a constant of another package by the name its
    /// import binds, which is the constant's bare name: the checker rejects
    /// the qualified `fw.LIMIT` in an expression. The IR records no imports,
    /// so the bare name is looked up in every other package of the build.
    #[test]
    fn an_imported_constant_named_in_a_contract_is_reached() {
        let p = guarded_fixture("level < LIMIT");
        let mut fw = Package {
            name: "fw".to_owned(),
            decls: vec![const_decl("LIMIT", "Level", "130.0")],
            ..Default::default()
        };
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.LIMIT", "p.Level"]);
        let before = hash_of(&p, &fw);
        set_const_value(&mut fw, "LIMIT", "140.0");
        assert_ne!(hash_of(&p, &fw), before);
    }

    /// A name inside an expression string is resolved in the package that
    /// declares the string. `fw.LIMIT`'s value `MAX` means `fw.MAX`, not
    /// the unit's own `MAX`: a bare name that the declaring
    /// package holds resolves there before any other package is searched.
    #[test]
    fn a_name_in_a_foreign_constant_value_resolves_in_its_own_package() {
        let p = guarded_fixture("level < LIMIT");
        let fw = Package {
            name: "fw".to_owned(),
            decls: vec![
                const_decl("LIMIT", "Level", "MAX"),
                const_decl("MAX", "Level", "7"),
            ],
            ..Default::default()
        };
        let reached: Vec<String> = reachable_decls("p", &[&p, &fw]).into_keys().collect();
        assert_eq!(reached, vec!["fw.LIMIT", "fw.MAX", "p.Level"]);
    }

    /// A query `get(level: Level) -> Level` with one `require` clause.
    fn guarded_query(source: &str) -> Decl {
        decl_of(
            "get",
            decl::Kind::QueryDef(QueryDef {
                params: vec![param(named("Level"))],
                return_type: Some(ReturnType {
                    kind: Some(return_type::Kind::Value(named("Level"))),
                }),
                contracts: vec![Contract {
                    kind: ContractKind::Require as i32,
                    source: source.to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
        )
    }

    /// A query's contract clause is followed like a command's.
    #[test]
    fn a_constant_named_in_a_query_contract_is_reached() {
        let mut p = guarded_fixture("");
        p.interfaces[0].interactions = vec![guarded_query("p < MAX")];
        let reached: Vec<String> = reachable_decls("p", &[&p]).into_keys().collect();
        assert_eq!(reached, vec!["p.Level", "p.MAX"]);
        let before = catalog_hash("p", &[&p]);
        set_const_value(&mut p, "MAX", "200");
        assert_ne!(catalog_hash("p", &[&p]), before);
    }

    /// A name inside a string literal of an expression is not a reference,
    /// and neither is the exponent or suffix of a number.
    #[test]
    fn a_name_in_a_string_literal_or_a_number_is_not_followed() {
        let mut p = guarded_fixture(r#"label != "say \"MAX\"" && level > 1e3 && level < 10MAX"#);
        p.decls.push(const_decl("e3", "Level", "1"));
        let reached: Vec<String> = reachable_decls("p", &[&p]).into_keys().collect();
        assert_eq!(reached, vec!["p.Level"]);
    }

    /// Expression strings are hashed as written: following a name does not
    /// rewrite it. Each string here is exactly one bare name that resolves
    /// to a declaration of the unit's package, whose canonical name is
    /// qualified, so a rewrite would be visible as `p.LIMIT` or `p.FLAG`.
    #[test]
    fn an_expression_string_is_hashed_as_written() {
        let mut p = guarded_fixture("LIMIT");
        p.decls.push(const_decl("LIMIT", "Level", "FLAG"));
        p.decls.push(const_decl("FLAG", "Level", "1"));
        let reduced = reduced_unit("p", &[&p]);
        assert!(reduced.decls.iter().any(|d| d.name == "p.FLAG"));
        let Some(decl::Kind::CommandDef(def)) = &reduced.interfaces[0].interactions[0].kind else {
            panic!("not a command");
        };
        assert_eq!(def.contracts[0].source, "LIMIT");
        let limit = reduced.decls.iter().find(|d| d.name == "p.LIMIT").unwrap();
        let Some(decl::Kind::ConstDef(def)) = &limit.kind else {
            panic!("not a constant");
        };
        assert_eq!(def.value, "FLAG");
    }

    fn field_of(kind: field_type::Kind) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(kind),
        }
    }

    /// A declaration `S` with one struct field of type `ty`.
    fn holder(ty: FieldType) -> Decl {
        Decl {
            name: "S".to_owned(),
            kind: Some(decl::Kind::StructDef(StructDef {
                members: vec![StructMember {
                    member: Some(struct_member::Member::Field(Box::new(Field {
                        name: "f".to_owned(),
                        ordinal: 1,
                        r#type: Some(ty),
                        ..Default::default()
                    }))),
                }],
                fixed_layout: false,
            })),
            ..Default::default()
        }
    }

    fn decl_of(name: &str, kind: decl::Kind) -> Decl {
        Decl {
            name: name.to_owned(),
            ordinal: 1,
            kind: Some(kind),
            ..Default::default()
        }
    }

    fn query(params: Vec<Param>, return_type: Option<return_type::Kind>) -> Decl {
        decl_of(
            "q",
            decl::Kind::QueryDef(QueryDef {
                params,
                return_type: return_type.map(|kind| ReturnType { kind: Some(kind) }),
                ..Default::default()
            }),
        )
    }

    fn param(ty: FieldType) -> Param {
        Param {
            name: "p".to_owned(),
            r#type: Some(ty),
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        }
    }

    fn pattern_const_def(name: &str) -> TypeDef {
        TypeDef {
            constraint: Some(Constraint {
                pattern_const: Some(name.to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// One case per kind of type reference the IR holds: the interactions of
    /// interface `I` and the declarations beside `T`, built so that `T` is
    /// reached only through that one reference.
    fn reference_cases() -> Vec<(&'static str, Vec<Decl>, Vec<Decl>)> {
        let t = || named("T");
        let via_s = || vec![signal("s", "S")];
        vec![
            ("SignalDef.payload", vec![signal("s", "T")], vec![]),
            (
                "EventDef.payload",
                vec![decl_of(
                    "e",
                    decl::Kind::EventDef(EventDef {
                        payload: "T".to_owned(),
                        ..Default::default()
                    }),
                )],
                vec![],
            ),
            ("FieldType::Named", via_s(), vec![holder(t())]),
            (
                "UnionArm.type_ref",
                via_s(),
                vec![decl_of(
                    "S",
                    decl::Kind::UnionDef(UnionDef {
                        arms: vec![UnionArm {
                            name: "a".to_owned(),
                            ordinal: 1,
                            type_ref: "T".to_owned(),
                            doc: String::new(),
                            links: Vec::new(),
                            see: Vec::new(),
                            since: Vec::new(),
                        }],
                        ..Default::default()
                    }),
                )],
            ),
            (
                "EnumSetDef.backing_enum",
                via_s(),
                vec![decl_of(
                    "S",
                    decl::Kind::EnumSetDef(EnumSetDef {
                        backing_enum: Some("T".to_owned()),
                        ..Default::default()
                    }),
                )],
            ),
            (
                "ConstDef.type_ref",
                via_s(),
                vec![
                    decl_of("S", decl::Kind::TypeDef(pattern_const_def("C"))),
                    decl_of(
                        "C",
                        decl::Kind::ConstDef(ConstDef {
                            type_ref: Some("T".to_owned()),
                            ..Default::default()
                        }),
                    ),
                ],
            ),
            (
                "Constraint.pattern_const",
                via_s(),
                vec![decl_of("S", decl::Kind::TypeDef(pattern_const_def("T")))],
            ),
            (
                "FieldType::InlineScalar",
                via_s(),
                vec![holder(field_of(field_type::Kind::InlineScalar(Box::new(
                    pattern_const_def("T"),
                ))))],
            ),
            (
                "TupleType.fields",
                via_s(),
                vec![holder(field_of(field_type::Kind::Tuple(TupleType {
                    fields: vec![TupleField {
                        name: "x".to_owned(),
                        r#type: Some(t()),
                    }],
                })))],
            ),
            (
                "ArrayType.element",
                via_s(),
                vec![holder(field_of(field_type::Kind::Array(Box::new(
                    ArrayType {
                        element: Some(Box::new(t())),
                        ..Default::default()
                    },
                ))))],
            ),
            (
                "MapType.key",
                via_s(),
                vec![holder(field_of(field_type::Kind::Map(Box::new(MapType {
                    key: Some(Box::new(t())),
                    ..Default::default()
                }))))],
            ),
            (
                "MapType.value",
                via_s(),
                vec![holder(field_of(field_type::Kind::Map(Box::new(MapType {
                    value: Some(Box::new(t())),
                    ..Default::default()
                }))))],
            ),
            (
                "StreamType::Named",
                via_s(),
                vec![holder(field_of(field_type::Kind::Stream(StreamType {
                    element: Some(stream_type::Element::Named("T".to_owned())),
                })))],
            ),
            (
                "FixedDef.payload",
                vec![decl_of(
                    "k",
                    decl::Kind::FixedDef(FixedDef { payload: Some(t()) }),
                )],
                vec![],
            ),
            (
                "CommandDef.params",
                vec![decl_of(
                    "c",
                    decl::Kind::CommandDef(CommandDef {
                        params: vec![param(t())],
                        ..Default::default()
                    }),
                )],
                vec![],
            ),
            (
                "QueryDef.params",
                vec![query(vec![param(t())], None)],
                vec![],
            ),
            (
                "ReturnType::Value",
                vec![query(vec![], Some(return_type::Kind::Value(t())))],
                vec![],
            ),
            (
                "FallibleType.ok",
                vec![query(
                    vec![],
                    Some(return_type::Kind::Fallible(FallibleType {
                        ok: "T".to_owned(),
                        err: String::new(),
                    })),
                )],
                vec![],
            ),
            (
                "FallibleType.err",
                vec![query(
                    vec![],
                    Some(return_type::Kind::Fallible(FallibleType {
                        ok: String::new(),
                        err: "T".to_owned(),
                    })),
                )],
                vec![],
            ),
        ]
    }

    /// Every kind of type reference reaches the declaration it names, and a
    /// change to that declaration moves the hash.
    #[test]
    fn every_reference_kind_reaches_its_declaration_and_moves_the_hash() {
        for (kind, interactions, mut decls) in reference_cases() {
            decls.push(scalar_decl("T"));
            let mut p = Package {
                name: "p".to_owned(),
                decls,
                interfaces: vec![Interface {
                    name: "I".to_owned(),
                    interactions,
                    number: 1,
                    ..Default::default()
                }],
                ..Default::default()
            };
            assert!(
                reachable_decls("p", &[&p]).contains_key("p.T"),
                "{kind}: the closure must reach `T`",
            );
            let before = catalog_hash("p", &[&p]);
            let target = p.decls.iter_mut().find(|d| d.name == "T").unwrap();
            target.kind = Some(decl::Kind::TypeDef(TypeDef {
                constraint: Some(Constraint {
                    min: Some("1".to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            }));
            assert_ne!(
                catalog_hash("p", &[&p]),
                before,
                "{kind}: a change to `T` must move the hash",
            );
        }
    }

    /// `p` with two interfaces, `I` (number 1) and `J` (number 2), each with
    /// a signal of `Point`.
    fn two_interface_fixture() -> (Package, Package) {
        let (mut p, fw) = fixture();
        p.interfaces.push(Interface {
            name: "J".to_owned(),
            interactions: vec![signal("pos", "Point")],
            number: 2,
            provisional: true,
            ..Default::default()
        });
        (p, fw)
    }

    /// The lock makes the number an interface's identity, so the reduced
    /// unit lists interfaces by (number, name), and the order in which
    /// the source declares them does not enter the hash.
    #[test]
    fn reordering_two_interfaces_does_not_move_the_hash() {
        let (mut p, fw) = two_interface_fixture();
        let before = hash_of(&p, &fw);
        p.interfaces.reverse();
        assert_eq!(hash_of(&p, &fw), before);
        let names: Vec<String> = reduced_unit("p", &[&p, &fw])
            .interfaces
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["I", "J"]);
    }

    fn reduced_interface_names(p: &Package, fw: &Package) -> Vec<String> {
        reduced_unit("p", &[p, fw])
            .interfaces
            .into_iter()
            .map(|i| i.name)
            .collect()
    }

    /// The number orders the interfaces, whatever their names: `I` numbered
    /// 2 comes after `J` numbered 1.
    #[test]
    fn interfaces_are_ordered_by_number() {
        let (mut p, fw) = two_interface_fixture();
        p.interfaces[0].number = 2;
        p.interfaces[1].number = 1;
        assert_eq!(reduced_interface_names(&p, &fw), ["J", "I"]);
    }

    /// Two interfaces with the same number are ordered by name, whatever
    /// their source order.
    #[test]
    fn interfaces_with_equal_numbers_are_ordered_by_name() {
        let (mut p, fw) = two_interface_fixture();
        p.interfaces[1].number = 1;
        p.interfaces.reverse();
        assert_eq!(reduced_interface_names(&p, &fw), ["I", "J"]);
    }

    #[test]
    fn swapping_two_interfaces_numbers_moves_the_hash() {
        let (mut p, fw) = two_interface_fixture();
        let before = hash_of(&p, &fw);
        p.interfaces[0].number = 2;
        p.interfaces[1].number = 1;
        assert_ne!(hash_of(&p, &fw), before);
    }

    /// A union arm's doc and an enum value's doc are blanked like the
    /// others; each is checked on its own, so the test fails when either
    /// clear is missing.
    #[test]
    fn a_union_arm_or_enum_value_doc_comment_does_not_move_the_hash() {
        let (mut p, fw) = fixture();
        p.decls.push(decl_of(
            "U",
            decl::Kind::UnionDef(UnionDef {
                arms: vec![UnionArm {
                    name: "a".to_owned(),
                    ordinal: 1,
                    type_ref: "Coord".to_owned(),
                    doc: String::new(),
                    links: Vec::new(),
                    see: Vec::new(),
                    since: Vec::new(),
                }],
                ..Default::default()
            }),
        ));
        p.decls.push(decl_of(
            "E",
            decl::Kind::EnumDef(EnumDef {
                values: vec![EnumValue {
                    name: "on".to_owned(),
                    value: 1,
                    doc: String::new(),
                    links: Vec::new(),
                    see: Vec::new(),
                    since: Vec::new(),
                }],
                ..Default::default()
            }),
        ));
        p.interfaces[0].interactions.push(signal("u", "U"));
        p.interfaces[0].interactions.push(signal("e", "E"));
        let before = hash_of(&p, &fw);

        let mut arm_doc = p.clone();
        if let Some(decl::Kind::UnionDef(def)) = &mut arm_doc.decls[3].kind {
            def.arms[0].doc = "documented".to_owned();
        }
        assert_ne!(arm_doc, p);
        assert_eq!(hash_of(&arm_doc, &fw), before, "a union arm's doc");

        let mut value_doc = p.clone();
        if let Some(decl::Kind::EnumDef(def)) = &mut value_doc.decls[4].kind {
            def.values[0].doc = "documented".to_owned();
        }
        assert_ne!(value_doc, p);
        assert_eq!(hash_of(&value_doc, &fw), before, "an enum value's doc");
    }
    /// Writes a value into every doc field `$carrier` has: `doc`, `links`,
    /// `see` and `since`, then each extra field named after the semicolon.
    macro_rules! document {
        ($carrier:expr $(; $tag:ident = $value:expr)*) => {{
            let carrier = &mut $carrier;
            carrier.doc = "documented, see [Other]".to_owned();
            carrier.links.push(DocLink {
                text: "Other".to_owned(),
                offset: 16,
                len: 7,
                target: "fw.Other".to_owned(),
            });
            carrier.see.push(DocLink {
                text: "fw.Other".to_owned(),
                target: "fw.Other".to_owned(),
                ..Default::default()
            });
            carrier.since.push("1.2".to_owned());
            $(carrier.$tag = $value;)*
        }};
    }

    /// `p`: one declaration of each kind, every one reached from interface
    /// `I` — a signal of the struct `S`, an event of the union `U`, a
    /// command whose parameter is the enum `E` and whose contract names the
    /// constant `C`, a query of the enum set `F` returning the scalar `T`,
    /// and a fixed member of `T` — and the service `svc` with an inline
    /// shape holding a signal of `S`.
    fn every_carrier_fixture() -> Package {
        let field = Field {
            name: "f".to_owned(),
            ordinal: 1,
            r#type: Some(named("T")),
            ..Default::default()
        };
        let command = decl_of(
            "c",
            decl::Kind::CommandDef(CommandDef {
                params: vec![Param {
                    name: "x".to_owned(),
                    r#type: Some(named("E")),
                    ..Default::default()
                }],
                contracts: vec![Contract {
                    kind: ContractKind::Require as i32,
                    source: "x != C".to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
        );
        let query = decl_of(
            "q",
            decl::Kind::QueryDef(QueryDef {
                params: vec![Param {
                    name: "y".to_owned(),
                    r#type: Some(named("F")),
                    ..Default::default()
                }],
                return_type: Some(ReturnType {
                    kind: Some(return_type::Kind::Value(named("T"))),
                }),
                ..Default::default()
            }),
        );
        Package {
            name: "p".to_owned(),
            decls: vec![
                scalar_decl("T"),
                const_decl("C", "E", "E.ON"),
                decl_of(
                    "S",
                    decl::Kind::StructDef(StructDef {
                        members: vec![StructMember {
                            member: Some(struct_member::Member::Field(Box::new(field))),
                        }],
                        fixed_layout: false,
                    }),
                ),
                decl_of(
                    "E",
                    decl::Kind::EnumDef(EnumDef {
                        values: vec![EnumValue {
                            name: "ON".to_owned(),
                            value: 1,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                ),
                decl_of(
                    "F",
                    decl::Kind::EnumSetDef(EnumSetDef {
                        bits: vec![EnumValue {
                            name: "A".to_owned(),
                            value: 0,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                ),
                decl_of(
                    "U",
                    decl::Kind::UnionDef(UnionDef {
                        arms: vec![UnionArm {
                            name: "s".to_owned(),
                            ordinal: 1,
                            type_ref: "S".to_owned(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                ),
            ],
            interfaces: vec![Interface {
                name: "I".to_owned(),
                interactions: vec![
                    signal("s", "S"),
                    decl_of(
                        "e",
                        decl::Kind::EventDef(EventDef {
                            payload: "U".to_owned(),
                            ..Default::default()
                        }),
                    ),
                    command,
                    query,
                    decl_of(
                        "fx",
                        decl::Kind::FixedDef(FixedDef {
                            payload: Some(named("T")),
                        }),
                    ),
                ],
                number: 1,
                ..Default::default()
            }],
            services: vec![inline_service("svc", vec![signal("t", "S")])],
            ..Default::default()
        }
    }

    fn decl_named<'a>(package: &'a mut Package, name: &str) -> &'a mut Decl {
        package.decls.iter_mut().find(|d| d.name == name).unwrap()
    }

    fn interaction_named<'a>(package: &'a mut Package, name: &str) -> &'a mut Decl {
        package.interfaces[0]
            .interactions
            .iter_mut()
            .find(|d| d.name == name)
            .unwrap()
    }

    fn inline_shape_of(package: &mut Package) -> &mut Interface {
        match &mut package.services[0].shapes[0].kind {
            Some(service_shape::Kind::Inline(interface)) => interface,
            _ => panic!("svc has no inline shape"),
        }
    }

    /// Writes doc fields into one carrier of a package.
    type SetDocs<'a> = Box<dyn Fn(&mut Package) + 'a>;

    /// Every doc field the package IR carries — `doc`, `links`, `see` and
    /// `since`, and `labels` and `deprecated` where the message has them —
    /// is blanked before hashing, on every message that carries one. Each
    /// carrier is documented on its own, so the test fails when any one
    /// clear is missing.
    #[test]
    fn catalog_hash_ignores_every_doc_field() {
        let p = every_carrier_fixture();
        let reached: Vec<String> = reachable_decls("p", &[&p]).into_keys().collect();
        assert_eq!(reached, vec!["p.C", "p.E", "p.F", "p.S", "p.T", "p.U"]);
        let before = catalog_hash("p", &[&p]);

        let labels = vec!["tagged".to_owned()];
        let deprecated = Some("use another".to_owned());
        let cases: Vec<(&str, SetDocs<'_>)> = vec![
            (
                "a declaration",
                Box::new(
                    |p| document!(*decl_named(p, "T"); labels = labels.clone(); deprecated = deprecated.clone()),
                ),
            ),
            (
                "a struct field",
                Box::new(|p| {
                    if let Some(decl::Kind::StructDef(def)) = &mut decl_named(p, "S").kind
                        && let Some(struct_member::Member::Field(field)) =
                            &mut def.members[0].member
                    {
                        document!(*field; labels = labels.clone(); deprecated = deprecated.clone());
                    }
                }),
            ),
            (
                "an enum value",
                Box::new(|p| {
                    if let Some(decl::Kind::EnumDef(def)) = &mut decl_named(p, "E").kind {
                        document!(def.values[0]);
                    }
                }),
            ),
            (
                "an enum set bit",
                Box::new(|p| {
                    if let Some(decl::Kind::EnumSetDef(def)) = &mut decl_named(p, "F").kind {
                        document!(def.bits[0]);
                    }
                }),
            ),
            (
                "a union arm",
                Box::new(|p| {
                    if let Some(decl::Kind::UnionDef(def)) = &mut decl_named(p, "U").kind {
                        document!(def.arms[0]);
                    }
                }),
            ),
            (
                "an interface",
                Box::new(
                    |p| document!(p.interfaces[0]; labels = labels.clone(); deprecated = deprecated.clone()),
                ),
            ),
            (
                "an interaction",
                Box::new(
                    |p| document!(*interaction_named(p, "s"); labels = labels.clone(); deprecated = deprecated.clone()),
                ),
            ),
            (
                "a command parameter",
                Box::new(|p| {
                    if let Some(decl::Kind::CommandDef(def)) = &mut interaction_named(p, "c").kind {
                        document!(def.params[0]);
                    }
                }),
            ),
            (
                "a query parameter",
                Box::new(|p| {
                    if let Some(decl::Kind::QueryDef(def)) = &mut interaction_named(p, "q").kind {
                        document!(def.params[0]);
                    }
                }),
            ),
            (
                "a service",
                Box::new(
                    |p| document!(p.services[0]; labels = labels.clone(); deprecated = deprecated.clone()),
                ),
            ),
            (
                "an inline shape",
                Box::new(
                    |p| document!(*inline_shape_of(p); labels = labels.clone(); deprecated = deprecated.clone()),
                ),
            ),
            (
                "an inline shape's interaction",
                Box::new(
                    |p| document!(inline_shape_of(p).interactions[0]; labels = labels.clone(); deprecated = deprecated.clone()),
                ),
            ),
        ];
        for (carrier, set_docs) in &cases {
            let mut documented = p.clone();
            set_docs(&mut documented);
            assert_ne!(documented, p, "{carrier}: the case must change the IR");
            assert_eq!(catalog_hash("p", &[&documented]), before, "{carrier}");
        }
    }

    /// A package of unit `unit` with `decls` and one declared interface
    /// `name`, numbered `number`, whose interactions are `interactions`.
    fn unit_package(
        name: &str,
        unit: &str,
        decls: Vec<Decl>,
        interface: (&str, u32, Vec<Decl>),
    ) -> Package {
        let (interface_name, number, interactions) = interface;
        Package {
            name: name.to_owned(),
            unit: unit.to_owned(),
            decls,
            interfaces: vec![Interface {
                name: interface_name.to_owned(),
                interactions,
                number,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// Unit `u`: the root package `u` with `Session` (1, reaches nothing)
    /// and `u.cluster` with `Speed` (2, reaches the struct `Pos`); unit `v`:
    /// the package `v` with `V` (1, reaches the struct `Q`).
    fn two_unit_fixture() -> (Package, Package, Package) {
        let u = unit_package("u", "u", vec![], ("Session", 1, vec![]));
        let cluster = unit_package(
            "u.cluster",
            "u",
            vec![struct_decl("Pos", &[])],
            ("Speed", 2, vec![signal("pos", "Pos")]),
        );
        let v = unit_package(
            "v",
            "v",
            vec![struct_decl("Q", &[])],
            ("V", 1, vec![signal("q", "Q")]),
        );
        (u, cluster, v)
    }

    fn names(interfaces: &[Interface]) -> Vec<&str> {
        interfaces.iter().map(|i| i.name.as_str()).collect()
    }

    fn decl_names(package: &Package) -> Vec<&str> {
        package.decls.iter().map(|d| d.name.as_str()).collect()
    }

    #[test]
    fn the_reduced_unit_holds_every_package_of_the_unit_under_catalog_names() {
        let (u, cluster, v) = two_unit_fixture();
        let reduced = reduced_unit("u", &[&u, &cluster, &v]);
        assert_eq!(reduced.name, "u");
        assert_eq!(reduced.unit, "u");
        assert_eq!(names(&reduced.interfaces), ["Session", "cluster.Speed"]);
        assert_eq!(decl_names(&reduced), ["u.cluster.Pos"]);
    }

    /// A bare reference is resolved in the package that declares the shape:
    /// `Pos` in `u.cluster`'s `Speed` is `u.cluster.Pos` although the root
    /// package declares a `Pos` too, and a `Speed` that names the root's
    /// `u.Pos` hashes differently.
    #[test]
    fn a_bare_reference_in_a_subpackage_resolves_to_its_own_homonym() {
        let (mut u, cluster, v) = two_unit_fixture();
        u.decls.push(struct_decl("Pos", &["u32"]));
        let reduced = reduced_unit("u", &[&u, &cluster, &v]);
        assert_eq!(decl_names(&reduced), ["u.cluster.Pos"]);
        let speed = reduced
            .interfaces
            .iter()
            .find(|interface| interface.name == "cluster.Speed")
            .expect("the subpackage interface");
        let Some(decl::Kind::SignalDef(signal)) = &speed.interactions[0].kind else {
            panic!("the interaction is a signal");
        };
        assert_eq!(signal.payload, "u.cluster.Pos");

        let mut root_pos = cluster.clone();
        root_pos.interfaces[0].interactions[0] = self::signal("pos", "u.Pos");
        let reduced = reduced_unit("u", &[&u, &root_pos, &v]);
        assert_eq!(decl_names(&reduced), ["u.Pos"]);
        assert_ne!(
            catalog_hash("u", &[&u, &cluster, &v]),
            catalog_hash("u", &[&u, &root_pos, &v])
        );
    }

    #[test]
    fn a_change_in_one_package_moves_the_hash_of_the_unit() {
        let (u, mut cluster, v) = two_unit_fixture();
        let before = catalog_hash("u", &[&u, &cluster, &v]);
        cluster.decls[0] = struct_decl("Pos", &["u32"]);
        assert_ne!(catalog_hash("u", &[&u, &cluster, &v]), before);
    }

    /// Unit selection is by the recorded unit, never by a name prefix: the
    /// unit `u.x` is not part of unit `u`.
    #[test]
    fn a_sibling_unit_with_a_prefix_name_is_not_in_the_hash() {
        let (u, cluster, _) = two_unit_fixture();
        let ux = unit_package(
            "u.x",
            "u.x",
            vec![struct_decl("W", &[])],
            ("X", 1, vec![signal("w", "W")]),
        );
        assert_eq!(
            catalog_hash("u", &[&u, &cluster, &ux]),
            catalog_hash("u", &[&u, &cluster])
        );
        assert_eq!(
            names(&reduced_unit("u", &[&u, &cluster, &ux]).interfaces),
            ["Session", "cluster.Speed"]
        );
    }

    #[test]
    fn the_same_short_name_in_two_packages_gives_two_reduced_declarations() {
        let a = unit_package(
            "u.a",
            "u",
            vec![struct_decl("Foo", &[])],
            ("A", 1, vec![signal("foo", "Foo")]),
        );
        let b = unit_package(
            "u.b",
            "u",
            vec![struct_decl("Foo", &["u32"])],
            ("B", 2, vec![signal("foo", "Foo")]),
        );
        let reduced = reduced_unit("u", &[&a, &b]);
        assert_eq!(decl_names(&reduced), ["u.a.Foo", "u.b.Foo"]);
        let reached: Vec<String> = reachable_decls("u", &[&a, &b]).into_keys().collect();
        assert_eq!(reached, ["u.a.Foo", "u.b.Foo"]);
    }

    /// The checker spells every retired entry as its lock key, so the unit's
    /// list concatenates the packages' entries as they are, in number order.
    #[test]
    fn unit_retired_keeps_every_entry_as_spelled() {
        let (mut u, mut cluster, mut v) = two_unit_fixture();
        cluster.retired.push(RetiredInterface {
            name: "cluster.Old".to_owned(),
            number: 3,
        });
        u.retired.push(RetiredInterface {
            name: "service:veh.x".to_owned(),
            number: 4,
        });
        v.retired.push(RetiredInterface {
            name: "Gone".to_owned(),
            number: 5,
        });
        let retired: Vec<(String, u32)> = crate::v2::unit_retired("u", &[&u, &cluster, &v])
            .into_iter()
            .map(|entry| (entry.name, entry.number))
            .collect();
        assert_eq!(
            retired,
            [
                ("cluster.Old".to_owned(), 3),
                ("service:veh.x".to_owned(), 4)
            ]
        );
    }

    /// A unit with an empty root: the anchor `u.a` carries the root's
    /// retired `Old` beside its own `a.Gone`, both as the lock spells them.
    /// No entry is re-qualified by the package that carries it.
    #[test]
    fn unit_retired_does_not_qualify_a_root_entry_by_its_anchor() {
        let mut a = unit_package("u.a", "u", vec![], ("A", 1, vec![]));
        a.retired.push(RetiredInterface {
            name: "Old".to_owned(),
            number: 3,
        });
        a.retired.push(RetiredInterface {
            name: "a.Gone".to_owned(),
            number: 5,
        });
        let retired: Vec<(String, u32)> = crate::v2::unit_retired("u", &[&a])
            .into_iter()
            .map(|entry| (entry.name, entry.number))
            .collect();
        assert_eq!(retired, [("Old".to_owned(), 3), ("a.Gone".to_owned(), 5)]);
    }
}
