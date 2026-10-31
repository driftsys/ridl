//! The podbuf projection, carried whole, so that a plugin never computes the
//! layout law again (ADR-0020 decisions 8 and 9).
//!
//! [`crate::projection::podbuf`] computes the layout of every head. This
//! module flattens its nested [`Head`] values into one list, [`v1::PodbufHead`],
//! and refers from one head to another by index into that list.
//!
//! The order of the list is fixed. The declarations come first, in
//! `Package.decls` order: a struct and a union each have one head, any other
//! declaration has none. The named induced tuples follow, in `Model.tuples`
//! order. The heads reached from those come last, in the order a depth-first
//! walk of the fields reaches them: a map entry, an arm of a union, and a
//! foreign struct or union reached inline.
//!
//! A struct, a tuple or a union written as a field is laid out inline as a
//! copy of its own head. That copy is not stored again: the field names the
//! head of the type it lays out, by the identity the projection records
//! ([`Inlined`]): the declaration, or for a tuple, the tuple at that position
//! of the lowering ([`TupleKeys`]). A tuple inside a foreign declaration gets
//! one head per tuple, keyed by its home package and its induced wire name,
//! so that a package that lays it out inline names the head the declaring
//! package names, and never a head of the same shape.
//!
//! A head that cannot be laid out is not in the list. It is listed in
//! `refused` with its cause, and so is every head that needs it.

use std::collections::{HashMap, HashSet};

use super::resolve::Scope;
use super::{flatbuffers, v1};
use crate::projection::flatbuffers::Packages;
use crate::projection::podbuf::{self, DescriptorKind, Form, Head, Inlined, Source};
use crate::v2;

/// The tuple at each position the walk can reach, by the walk's path to it:
/// the root (`Decl`, `tuple N` or `pkg.Decl`), then a struct field's name, a
/// tuple field's position, `key` or `value`.
#[derive(Default)]
pub(crate) struct TupleKeys {
    /// The index in `Model.tuples` of a named tuple.
    pub local: HashMap<Vec<String>, u32>,
    /// A tuple inside a declaration of another package, which `Model.tuples`
    /// does not hold.
    pub foreign: HashMap<Vec<String>, ForeignTuple>,
}

/// One tuple of another package, as the lowering lifted it.
pub(crate) struct ForeignTuple {
    /// The package that declares it.
    pub package: String,
    /// The induced wire name that package gives it.
    pub wire: String,
    /// The lowering's entry: two positions with one entry are one tuple.
    pub entry: u32,
}

/// The podbuf heads of the package. A head the projection refuses, which is
/// malformed IR or a type too large for a `u32` offset, is listed in
/// `refused` with its cause, and so is each head that needs it.
pub(crate) fn project(
    scope: Scope<'_>,
    named: &[flatbuffers::NamedTuple<'_>],
    foreign_at: &HashMap<(String, String), u32>,
    keys: &TupleKeys,
) -> v1::PodbufProjection {
    let packages = Packages {
        package: scope.package,
        others: scope.others,
    };
    let mut roots: Vec<Root> = Vec::new();

    for (index, decl) in scope.package.decls.iter().enumerate() {
        let head = match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => podbuf::struct_head(packages, &decl.name, def),
            Some(v2::decl::Kind::UnionDef(def)) => podbuf::union_head(packages, &decl.name, def),
            _ => continue,
        }
        .map_err(|error| error.message);
        roots.push(Root {
            source: v1::podbuf_refused::Source::Declaration(index as u32),
            name: Some(decl.name.clone()),
            head,
        });
    }
    for tuple in named {
        let packages = Packages {
            package: tuple.home,
            others: scope.others,
        };
        let head =
            podbuf::tuple_head(packages, tuple.wire, tuple.source).map_err(|error| error.message);
        roots.push(Root {
            source: v1::podbuf_refused::Source::Tuple(tuple.index),
            name: None,
            head,
        });
    }

    // A head that fails to lay out names the refused head it needs, when the
    // declaration it needs is one that failed.
    let failed: HashSet<String> = roots
        .iter()
        .filter(|root| root.head.is_err())
        .filter_map(|root| root.name.clone())
        .collect();
    for root in &mut roots {
        let (Err(cause), Some(name)) = (&root.head, &root.name) else {
            continue;
        };
        let needed = scope
            .package
            .decls
            .iter()
            .find(|decl| &decl.name == name)
            .map(|decl| needs(scope.package, decl))
            .unwrap_or_default();
        if let Some(dependency) = needed.iter().find(|n| *n != name && failed.contains(*n)) {
            root.head = Err(format!("needs the refused head {dependency}: {cause}"));
        }
    }

    // A head whose fields cannot all be named is refused too, and the walk
    // starts again without it, so that no index of the list moves after the
    // fact. Each pass refuses at least one more root, so the loop ends.
    let mut excluded: HashSet<usize> = HashSet::new();
    let mut causes: HashMap<usize, String> = HashMap::new();
    loop {
        match attempt(scope, &roots, &excluded, foreign_at, keys) {
            Ok(heads) => {
                let refused = roots
                    .iter()
                    .enumerate()
                    .filter_map(|(at, root)| {
                        let cause = match &root.head {
                            Err(cause) => cause.clone(),
                            Ok(_) => causes.get(&at)?.clone(),
                        };
                        Some(v1::PodbufRefused {
                            source: Some(root.source),
                            cause,
                        })
                    })
                    .collect();
                let boxes = boxes(scope);
                return v1::PodbufProjection {
                    heads,
                    refused,
                    boxes,
                };
            }
            Err((at, cause)) => {
                excluded.insert(at);
                causes.insert(at, cause);
            }
        }
    }
}

/// The one-field head of each named scalar, enum and enum set of the package
/// that can be laid out, in declaration order.
fn boxes(scope: Scope<'_>) -> Vec<v1::PodbufBox> {
    let packages = Packages {
        package: scope.package,
        others: scope.others,
    };
    let no_heads = HashMap::new();
    let no_tuples = TupleKeys::default();
    let mut out = Vec::new();
    for (index, decl) in scope.package.decls.iter().enumerate() {
        let Some(head) = podbuf::boxed_head_of(packages, decl) else {
            continue;
        };
        let mut walk = Walk {
            scope,
            foreign_at: &no_heads,
            foreign_names: HashMap::new(),
            keys: &no_tuples,
            heads: vec![v1::PodbufHead::default()],
            declared: HashMap::new(),
            tuples: HashMap::new(),
            nested: HashMap::new(),
            arms: HashMap::new(),
            foreign: HashMap::new(),
            refused: HashSet::new(),
        };
        let root = std::slice::from_ref(&decl.name);
        if walk.fill(0, &head, root, root, Owner::None).is_ok() {
            out.push(v1::PodbufBox {
                declaration: index as u32,
                head: walk.heads.pop(),
            });
        }
    }
    out
}

/// The declarations of the package that `decl` names in its fields or arms.
fn needs(package: &v2::Package, decl: &v2::Decl) -> Vec<String> {
    let mut out = Vec::new();
    match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            for member in &def.members {
                if let Some(v2::struct_member::Member::Field(field)) = &member.member
                    && let Some(ty) = field.r#type.as_ref()
                {
                    names_in(package, ty, &mut out);
                }
            }
        }
        Some(v2::decl::Kind::UnionDef(def)) => {
            for arm in &def.arms {
                out.push(local_name(package, &arm.type_ref));
            }
        }
        _ => {}
    }
    out
}

fn names_in(package: &v2::Package, ty: &v2::FieldType, out: &mut Vec<String>) {
    match ty.kind.as_ref() {
        Some(v2::field_type::Kind::Named(reference)) => out.push(local_name(package, reference)),
        Some(v2::field_type::Kind::Tuple(tuple)) => {
            for field in &tuple.fields {
                if let Some(ty) = field.r#type.as_ref() {
                    names_in(package, ty, out);
                }
            }
        }
        Some(v2::field_type::Kind::Array(array)) => {
            if let Some(element) = array.element.as_deref() {
                names_in(package, element, out);
            }
        }
        Some(v2::field_type::Kind::Map(map)) => {
            for ty in [map.key.as_deref(), map.value.as_deref()]
                .into_iter()
                .flatten()
            {
                names_in(package, ty, out);
            }
        }
        _ => {}
    }
}

/// The bare name of a reference into `package`.
fn local_name(package: &v2::Package, reference: &str) -> String {
    reference
        .strip_prefix(&format!("{}.", package.name))
        .unwrap_or(reference)
        .to_string()
}

/// A declaration or a tuple, and what the projection computed for it.
struct Root {
    source: v1::podbuf_refused::Source,
    /// The declared name; `None` for a tuple.
    name: Option<String>,
    head: Result<Head, String>,
}

/// What a head belongs to, which names the arms of a union.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Owner {
    /// A tuple, which has no union.
    None,
    /// The declaration of this package at that index.
    Declaration(u32),
    /// The foreign declaration at that index of `Model.foreign`.
    Foreign(u32),
}

/// A root to fill: its position, its head slot, its layout, the path to it and
/// the declaration index it belongs to.
type Pending<'a> = (usize, u32, &'a Head, Vec<String>, Owner);

/// One walk over every root that is not excluded. An error names the root
/// whose fields cannot all be named and the cause.
fn attempt(
    scope: Scope<'_>,
    roots: &[Root],
    excluded: &HashSet<usize>,
    foreign_at: &HashMap<(String, String), u32>,
    keys: &TupleKeys,
) -> Result<Vec<v1::PodbufHead>, (usize, String)> {
    let mut walk = Walk {
        scope,
        foreign_at,
        foreign_names: foreign_at
            .iter()
            .map(|((package, name), index)| (*index, (package.as_str(), name.as_str())))
            .collect(),
        keys,
        heads: Vec::new(),
        declared: HashMap::new(),
        tuples: HashMap::new(),
        nested: HashMap::new(),
        arms: HashMap::new(),
        foreign: HashMap::new(),
        refused: roots
            .iter()
            .enumerate()
            .filter(|(at, root)| root.head.is_err() || excluded.contains(at))
            .filter_map(|(_, root)| root.name.clone())
            .collect(),
    };
    let mut pending: Vec<Pending<'_>> = Vec::new();
    for (at, root) in roots.iter().enumerate() {
        let Ok(head) = &root.head else { continue };
        if excluded.contains(&at) {
            continue;
        }
        let (source, declaration, path) = match &root.source {
            v1::podbuf_refused::Source::Declaration(index) => (
                v1::podbuf_head::Source::Declaration(*index),
                Owner::Declaration(*index),
                root.name.clone().unwrap_or_default(),
            ),
            v1::podbuf_refused::Source::Tuple(index) => (
                v1::podbuf_head::Source::Tuple(*index),
                Owner::None,
                format!("tuple {index}"),
            ),
        };
        let index = walk.push(source);
        match (&root.source, &root.name) {
            (v1::podbuf_refused::Source::Declaration(_), Some(name)) => {
                walk.declared.insert(name.clone(), index);
            }
            (v1::podbuf_refused::Source::Tuple(tuple), _) => {
                walk.tuples.insert(*tuple, index);
            }
            _ => {}
        }
        pending.push((at, index, head, vec![path], declaration));
    }
    for (at, index, head, path, declaration) in pending {
        walk.fill(index, head, &path, &path, declaration)
            .map_err(|cause| (at, cause))?;
    }
    Ok(walk.heads)
}

struct Walk<'a> {
    scope: Scope<'a>,
    foreign_at: &'a HashMap<(String, String), u32>,
    /// `foreign_at` inverted: the package and name at each `Model.foreign`
    /// index.
    foreign_names: HashMap<u32, (&'a str, &'a str)>,
    /// The tuple at each position, by the path to it.
    keys: &'a TupleKeys,
    heads: Vec<v1::PodbufHead>,
    /// The head of each local struct or union, by name.
    declared: HashMap<String, u32>,
    /// The head of each named tuple, by its index in `Model.tuples`.
    tuples: HashMap<u32, u32>,
    /// The head of each tuple reached inside a foreign aggregate, by its home
    /// package and its induced wire name, with the lowering's entry and the
    /// path that reached it first: a second entry under the same name is
    /// refused, never merged.
    nested: HashMap<(String, String), (u32, u32, Vec<String>)>,
    /// The head of each union arm, by the union's owner and the arm ordinal:
    /// a union reached again as the arm of another union shares the arm heads
    /// of its own walk, so a header defines each once.
    arms: HashMap<(Owner, u32), u32>,
    /// The head of each foreign struct or union reached inline, by package and
    /// name.
    foreign: HashMap<(String, String), u32>,
    /// The local declarations with no head.
    refused: HashSet<String>,
}

impl Walk<'_> {
    fn push(&mut self, source: v1::podbuf_head::Source) -> u32 {
        self.heads.push(v1::PodbufHead {
            source: Some(source),
            ..Default::default()
        });
        (self.heads.len() - 1) as u32
    }

    /// Writes the size, the fields and the pads of `head` into slot `index`.
    /// `declaration` is the union or struct the walk started from, which names
    /// the arms of a union. `path` is the declaration name followed by the
    /// names of the fields that lead to `head`. `key` is the same path as the
    /// lowering spells it ([`TupleKeys`]): the two differ inside an arm of a
    /// union, whose head is a copy of the arm type's head, so the key starts
    /// again at that type.
    fn fill(
        &mut self,
        index: u32,
        head: &Head,
        path: &[String],
        key: &[String],
        declaration: Owner,
    ) -> Result<(), String> {
        let mut fields = Vec::with_capacity(head.fields.len());
        for placement in &head.fields {
            let mut at = path.to_vec();
            let mut key_at = key.to_vec();
            if let Some(segment) = segment(&placement.source) {
                at.push(segment.clone());
                key_at.push(segment);
            }
            fields.push(v1::PodbufField {
                source: Some(source(&placement.source)),
                offset: placement.offset,
                size: placement.size,
                align: placement.align,
                form: Some(self.form(&placement.form, &at, &key_at, declaration)?),
                optional: placement.optional.map(|bit| v1::PodbufFlagBit {
                    flags: bit.flags,
                    bit: u32::from(bit.bit),
                }),
            });
        }
        let slot = &mut self.heads[index as usize];
        slot.size = head.size;
        slot.align = podbuf::HEAD_ALIGN;
        slot.fields = fields;
        slot.pads = head
            .pads
            .iter()
            .map(|pad| v1::PodbufPad {
                offset: pad.offset,
                size: pad.size,
            })
            .collect();
        Ok(())
    }

    fn form(
        &mut self,
        form: &Form,
        path: &[String],
        key: &[String],
        declaration: Owner,
    ) -> Result<v1::PodbufForm, String> {
        use v1::podbuf_form::Kind;
        let kind = match form {
            Form::Bool => Kind::Boolean(true),
            Form::Int { width, signed } => Kind::Int(v1::PodbufInt {
                width: *width as i32,
                signed: *signed,
            }),
            Form::Float(width) => Kind::Float(*width as i32),
            Form::Enum {
                width,
                signed,
                reference,
            } => Kind::Enum(v1::PodbufEnum {
                width: *width as i32,
                signed: *signed,
                reference: reference.clone(),
            }),
            Form::EnumSet { width, reference } => Kind::EnumSet(v1::PodbufEnumSet {
                width: *width as i32,
                reference: reference.clone(),
            }),
            Form::Inline { head, of } => {
                Kind::InlineHead(self.inline(head, of, path, key, declaration)?)
            }
            Form::InlineArray { element, count } => {
                Kind::InlineArray(Box::new(v1::PodbufInlineArray {
                    element: Some(Box::new(self.form(element, path, key, declaration)?)),
                    count: *count,
                }))
            }
            Form::Descriptor(descriptor) => Kind::Descriptor(Box::new(self.descriptor(
                descriptor,
                path,
                key,
                declaration,
            )?)),
        };
        Ok(v1::PodbufForm { kind: Some(kind) })
    }

    /// The index of the head an inline field lays out.
    fn inline(
        &mut self,
        head: &Head,
        of: &Inlined,
        path: &[String],
        key: &[String],
        declaration: Owner,
    ) -> Result<u32, String> {
        let at = path.join(".");
        match of {
            Inlined::Tuple => {
                // The tuple at this position of the lowering: one of this
                // package's tuple list, whose head is a root, or one reached
                // inside a foreign aggregate, which the list does not hold.
                // That one gets a head of its own, found again by the tuple's
                // identity, never by a head of the same shape.
                if let Some(tuple) = self.keys.local.get(key) {
                    return self
                        .tuples
                        .get(tuple)
                        .copied()
                        .ok_or_else(|| format!("{at}: needs the refused head of tuple {tuple}"));
                }
                // Unreachable for checked input: the lowering lifts a tuple
                // at every position the projection lays one out, under the
                // same path. A miss is a disagreement between the two walks.
                let Some(tuple) = self.keys.foreign.get(key) else {
                    return Err(format!(
                        "{at}: internal: no tuple was lowered at this position (the \
                         lowering's tuple keys and the podbuf walk disagree on the path)"
                    ));
                };
                let ForeignTuple {
                    package,
                    wire,
                    entry,
                } = tuple;
                let identity = (package.clone(), wire.clone());
                if let Some((index, known, first)) = self.nested.get(&identity) {
                    if known == entry {
                        return Ok(*index);
                    }
                    // Two tuples of one package induce one wire name from two
                    // paths (a struct field `sizeField2` next to position 2 of
                    // the tuple at `size`), whatever their layouts. The
                    // declaring header refuses the pair through its C name
                    // claim; this package refuses it here, so the two headers
                    // agree and neither types one tuple with the other's head.
                    return Err(format!(
                        "{at}: the tuple at {} and the tuple at {at} of package {package} both \
                         induce the wire name {wire}; rename one of the two fields so that \
                         each tuple has a name of its own",
                        first.join(".")
                    ));
                }
                let index = self.push(v1::podbuf_head::Source::NestedTuple(v1::NestedTuple {
                    path: path.to_vec(),
                    package: package.clone(),
                    wire: wire.clone(),
                }));
                self.nested.insert(identity, (index, *entry, path.to_vec()));
                self.fill(index, head, path, key, declaration)?;
                Ok(index)
            }
            Inlined::Declaration { package, name } if *package == self.scope.package.name => {
                match self.declared.get(name) {
                    Some(index) => Ok(*index),
                    None if self.refused.contains(name) => {
                        Err(format!("{at}: needs the refused head {name}"))
                    }
                    None => Err(format!("{at}: {name} has no head")),
                }
            }
            Inlined::Declaration { package, name } => {
                let key = (package.clone(), name.clone());
                if let Some(index) = self.foreign.get(&key) {
                    return Ok(*index);
                }
                let declaration = *self
                    .foreign_at
                    .get(&key)
                    .ok_or_else(|| format!("{at}: {package}.{name} is not in the foreign list"))?;
                let index = self.push(v1::podbuf_head::Source::Foreign(declaration));
                self.foreign.insert(key, index);
                let root = [format!("{package}.{name}")];
                self.fill(index, head, &root, &root, Owner::Foreign(declaration))?;
                Ok(index)
            }
        }
    }

    fn descriptor(
        &mut self,
        descriptor: &DescriptorKind,
        path: &[String],
        key: &[String],
        declaration: Owner,
    ) -> Result<v1::PodbufDescriptor, String> {
        use v1::podbuf_descriptor::Kind;
        let kind = match descriptor {
            DescriptorKind::Text { min, max } => Kind::Text(v1::PodbufBound {
                min: *min,
                max: *max,
            }),
            DescriptorKind::Bytes { min, max } => Kind::Bytes(v1::PodbufBound {
                min: *min,
                max: *max,
            }),
            DescriptorKind::Array {
                element,
                elem_size,
                min,
                max,
            } => Kind::Array(Box::new(v1::PodbufArray {
                element: Some(Box::new(self.form(element, path, key, declaration)?)),
                elem_size: *elem_size,
                min: *min,
                max: *max,
            })),
            DescriptorKind::Map {
                entry,
                elem_size,
                min,
                max,
            } => {
                let index = self.push(v1::podbuf_head::Source::MapEntry(v1::MapEntry {
                    path: path.to_vec(),
                }));
                self.fill(index, entry, path, key, declaration)?;
                Kind::Map(v1::PodbufMap {
                    entry_head: index,
                    elem_size: *elem_size,
                    min: *min,
                    max: *max,
                })
            }
            DescriptorKind::Arm { arms } => {
                let (union, foreign) = match declaration {
                    Owner::Declaration(index) => (index, None),
                    Owner::Foreign(index) => (0, Some(index)),
                    Owner::None => {
                        return Err(format!(
                            "{}: a union arm outside a union declaration",
                            path.join(".")
                        ));
                    }
                };
                let mut out = Vec::with_capacity(arms.len());
                for (ordinal, head) in arms {
                    if let Some(index) = self.arms.get(&(declaration, *ordinal)) {
                        out.push(v1::PodbufArm {
                            ordinal: *ordinal,
                            head: *index,
                        });
                        continue;
                    }
                    let index = self.push(v1::podbuf_head::Source::Arm(v1::ArmHead {
                        declaration: union,
                        arm: *ordinal,
                        foreign,
                    }));
                    let mut at = path.to_vec();
                    at.push(ordinal.to_string());
                    // The arm's head is a copy of the arm type's head, so a
                    // tuple inside it is the one the lowering reached under
                    // that type, and when that type is a union, its arms
                    // belong to it, not to the union being filled.
                    let (key_at, owner) = match self.arm_type(declaration, *ordinal) {
                        Some((target, declaring)) => (
                            vec![self.root_of(target, declaring)],
                            self.owner_of(target, declaring)?.unwrap_or(declaration),
                        ),
                        None => {
                            let mut key_at = key.to_vec();
                            key_at.push(ordinal.to_string());
                            (key_at, declaration)
                        }
                    };
                    self.arms.insert((declaration, *ordinal), index);
                    self.fill(index, head, &at, &key_at, owner)?;
                    out.push(v1::PodbufArm {
                        ordinal: *ordinal,
                        head: index,
                    });
                }
                Kind::Arms(v1::PodbufArms { arms: out })
            }
        };
        Ok(v1::PodbufDescriptor { kind: Some(kind) })
    }
}

impl<'a> Walk<'a> {
    /// The type of arm `ordinal` of the union `declaration`, and the package
    /// that declares that type. `None` when the union or the arm's type is
    /// not found, which the projection has already refused.
    fn arm_type(
        &self,
        declaration: Owner,
        ordinal: u32,
    ) -> Option<(&'a v2::Decl, &'a v2::Package)> {
        let (home, decl) = match declaration {
            Owner::None => return None,
            Owner::Declaration(index) => (
                self.scope.package,
                self.scope.package.decls.get(index as usize)?,
            ),
            Owner::Foreign(index) => {
                let (package, name) = *self.foreign_names.get(&index)?;
                let home = self
                    .scope
                    .others
                    .iter()
                    .copied()
                    .find(|other| other.name == package)?;
                let decl = home.decls.iter().find(|decl| decl.name == name)?;
                (home, decl)
            }
        };
        let Some(v2::decl::Kind::UnionDef(def)) = decl.kind.as_ref() else {
            return None;
        };
        let arm = def.arms.iter().find(|arm| arm.ordinal == ordinal)?;
        self.scope.resolve(home, &arm.type_ref)
    }

    /// The root of the lowering's path to `decl` of `declaring`: its bare
    /// name for a type of this package, its qualified name for one of another
    /// package.
    fn root_of(&self, decl: &v2::Decl, declaring: &v2::Package) -> String {
        if declaring.name == self.scope.package.name {
            decl.name.clone()
        } else {
            format!("{}.{}", declaring.name, decl.name)
        }
    }

    /// The owner a head copied from `decl` of `declaring` names for its arms:
    /// the union itself, when `decl` is one; `None` for any other declaration.
    /// A foreign union the lowering did not reach is an error: the lowering
    /// adds every declaration it resolves to the foreign list, so the walk
    /// and the lowering disagree.
    fn owner_of(&self, decl: &v2::Decl, declaring: &v2::Package) -> Result<Option<Owner>, String> {
        if !matches!(decl.kind.as_ref(), Some(v2::decl::Kind::UnionDef(_))) {
            return Ok(None);
        }
        if declaring.name == self.scope.package.name {
            let index = declaring
                .decls
                .iter()
                .position(|candidate| candidate.name == decl.name)
                .ok_or_else(|| {
                    format!(
                        "internal: the union {} is not a declaration of {}",
                        decl.name, declaring.name
                    )
                })?;
            Ok(Some(Owner::Declaration(index as u32)))
        } else {
            let index = self
                .foreign_at
                .get(&(declaring.name.clone(), decl.name.clone()))
                .ok_or_else(|| {
                    format!(
                        "internal: the union {}.{} is an arm's type but not in the foreign \
                         list (the lowering and the podbuf walk disagree)",
                        declaring.name, decl.name
                    )
                })?;
            Ok(Some(Owner::Foreign(*index)))
        }
    }
}

/// The path segment a field adds, when it has a name.
fn segment(source: &Source) -> Option<String> {
    match source {
        Source::Field { name, .. } => Some(name.clone()),
        Source::Position { position, .. } => Some(position.to_string()),
        Source::Key => Some("key".to_string()),
        Source::Value => Some("value".to_string()),
        Source::Flags { .. } | Source::Tag | Source::Arm => None,
    }
}

fn source(source: &Source) -> v1::PodbufSource {
    use v1::podbuf_source::Kind;
    let kind = match source {
        Source::Field { name, ordinal } => Kind::Field(v1::PodbufNamedField {
            name: name.clone(),
            ordinal: *ordinal,
        }),
        Source::Flags { index } => Kind::Flags(*index),
        Source::Tag => Kind::Tag(true),
        Source::Arm => Kind::Arm(true),
        Source::Position { position, .. } => Kind::Position(*position),
        Source::Key => Kind::Key(true),
        Source::Value => Kind::Value(true),
    };
    let position_name = match source {
        Source::Position { name, .. } => name.clone(),
        _ => String::new(),
    };
    v1::PodbufSource {
        kind: Some(kind),
        position_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A named tuple whose root is excluded from the walk is a refused head:
    /// the position that lays it out inline names it as the cause, as a
    /// refused declaration is named, instead of getting a head of its own.
    #[test]
    fn an_inline_position_of_an_excluded_tuple_is_refused() {
        let package = v2::Package {
            name: "p.a".to_string(),
            ..Default::default()
        };
        let scope = Scope {
            package: &package,
            others: &[],
        };
        let path = vec!["Holder".to_string(), "t".to_string()];
        let mut keys = TupleKeys::default();
        keys.local.insert(path.clone(), 3);
        let no_heads = HashMap::new();
        let mut walk = Walk {
            scope,
            foreign_at: &no_heads,
            foreign_names: HashMap::new(),
            keys: &keys,
            heads: Vec::new(),
            declared: HashMap::new(),
            tuples: HashMap::new(),
            nested: HashMap::new(),
            arms: HashMap::new(),
            foreign: HashMap::new(),
            refused: HashSet::new(),
        };
        let head = Head {
            size: 0,
            fields: Vec::new(),
            pads: Vec::new(),
        };
        let error = walk
            .inline(&head, &Inlined::Tuple, &path, &path, Owner::None)
            .expect_err("the excluded tuple is refused");
        assert_eq!(error, "Holder.t: needs the refused head of tuple 3");
        assert!(walk.heads.is_empty(), "no head was pushed");
    }
}
