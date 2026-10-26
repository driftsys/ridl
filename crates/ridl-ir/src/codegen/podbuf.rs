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
//! ([`Inlined`]): the declaration, or for a tuple, its structure.
//!
//! A head that cannot be laid out is not in the list. It is listed in
//! `refused` with its cause, and so is every head that needs it.

use std::collections::{HashMap, HashSet};

use super::resolve::Scope;
use super::{flatbuffers, v1};
use crate::projection::flatbuffers::Packages;
use crate::projection::podbuf::{self, DescriptorKind, Form, Head, Inlined, Source};
use crate::v2;

/// The podbuf heads of the package. A head the projection refuses, which is
/// malformed IR or a type too large for a `u32` offset, is listed in
/// `refused` with its cause, and so is each head that needs it.
pub(crate) fn project(
    scope: Scope<'_>,
    named: &[flatbuffers::NamedTuple<'_>],
    foreign_at: &HashMap<(String, String), u32>,
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
        match attempt(scope, &roots, &excluded, foreign_at) {
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
                return v1::PodbufProjection { heads, refused };
            }
            Err((at, cause)) => {
                excluded.insert(at);
                causes.insert(at, cause);
            }
        }
    }
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
#[derive(Clone, Copy)]
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
) -> Result<Vec<v1::PodbufHead>, (usize, String)> {
    let mut walk = Walk {
        package: scope.package.name.clone(),
        foreign_at,
        heads: Vec::new(),
        declared: HashMap::new(),
        tuples: Vec::new(),
        nested: Vec::new(),
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
            _ => walk.tuples.push((head.clone(), index)),
        }
        pending.push((at, index, head, vec![path], declaration));
    }
    for (at, index, head, path, declaration) in pending {
        walk.fill(index, head, &path, declaration)
            .map_err(|cause| (at, cause))?;
    }
    Ok(walk.heads)
}

struct Walk<'a> {
    /// The name of the package lowered.
    package: String,
    foreign_at: &'a HashMap<(String, String), u32>,
    heads: Vec<v1::PodbufHead>,
    /// The head of each local struct or union, by name.
    declared: HashMap<String, u32>,
    /// The head of each named tuple, the heads a tuple written at a field is
    /// matched against by structure.
    tuples: Vec<(Head, u32)>,
    /// The heads of the tuples reached inside a foreign aggregate.
    nested: Vec<(Head, u32)>,
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
    /// names of the fields that lead to `head`.
    fn fill(
        &mut self,
        index: u32,
        head: &Head,
        path: &[String],
        declaration: Owner,
    ) -> Result<(), String> {
        let mut fields = Vec::with_capacity(head.fields.len());
        for placement in &head.fields {
            let mut at = path.to_vec();
            if let Some(segment) = segment(&placement.source) {
                at.push(segment);
            }
            fields.push(v1::PodbufField {
                source: Some(source(&placement.source)),
                offset: placement.offset,
                size: placement.size,
                align: placement.align,
                form: Some(self.form(&placement.form, &at, declaration)?),
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
                Kind::InlineHead(self.inline(head, of, path, declaration)?)
            }
            Form::InlineArray { element, count } => {
                Kind::InlineArray(Box::new(v1::PodbufInlineArray {
                    element: Some(Box::new(self.form(element, path, declaration)?)),
                    count: *count,
                }))
            }
            Form::Descriptor(descriptor) => {
                Kind::Descriptor(Box::new(self.descriptor(descriptor, path, declaration)?))
            }
        };
        Ok(v1::PodbufForm { kind: Some(kind) })
    }

    /// The index of the head an inline field lays out.
    fn inline(
        &mut self,
        head: &Head,
        of: &Inlined,
        path: &[String],
        declaration: Owner,
    ) -> Result<u32, String> {
        let at = path.join(".");
        match of {
            Inlined::Tuple => {
                // A tuple of this package's tuple list, or one reached inside
                // a foreign aggregate, which the list does not hold: it gets
                // a head of its own, found again by structure.
                let known = self.tuples.iter().chain(&self.nested);
                if let Some((_, index)) = known.into_iter().find(|(named, _)| named == head) {
                    return Ok(*index);
                }
                let index = self.push(v1::podbuf_head::Source::NestedTuple(v1::NestedTuple {
                    path: path.to_vec(),
                }));
                self.nested.push((head.clone(), index));
                self.fill(index, head, path, declaration)?;
                Ok(index)
            }
            Inlined::Declaration { package, name } if *package == self.package => {
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
                self.fill(
                    index,
                    head,
                    &[format!("{package}.{name}")],
                    Owner::Foreign(declaration),
                )?;
                Ok(index)
            }
        }
    }

    fn descriptor(
        &mut self,
        descriptor: &DescriptorKind,
        path: &[String],
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
                element: Some(Box::new(self.form(element, path, declaration)?)),
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
                self.fill(index, entry, path, declaration)?;
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
                    let index = self.push(v1::podbuf_head::Source::Arm(v1::ArmHead {
                        declaration: union,
                        arm: *ordinal,
                        foreign,
                    }));
                    let mut at = path.to_vec();
                    at.push(ordinal.to_string());
                    self.fill(index, head, &at, declaration)?;
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

/// The path segment a field adds, when it has a name.
fn segment(source: &Source) -> Option<String> {
    match source {
        Source::Field { name, .. } => Some(name.clone()),
        Source::Position(position) => Some(position.to_string()),
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
        Source::Position(position) => Kind::Position(*position),
        Source::Key => Kind::Key(true),
        Source::Value => Kind::Value(true),
    };
    v1::PodbufSource { kind: Some(kind) }
}
