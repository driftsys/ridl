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
//! order. The heads reached from those, a map entry and an arm of a union,
//! come last, in the order a depth-first walk of the fields reaches them.
//!
//! A struct, a tuple or a union written as a field is laid out inline as a
//! copy of its own head. That copy is not stored again: the field names the
//! head that has the same layout, and when two declarations share one layout
//! it names the first of them.

use super::resolve::Scope;
use super::{flatbuffers, v1};
use crate::projection::flatbuffers::Packages;
use crate::projection::podbuf::{self, DescriptorKind, Form, Head, Source};
use crate::v2;

/// The podbuf heads of the package. An error is a layout the projection
/// refuses, which is malformed IR or a type too large for a `u32` offset; the
/// podbuf check reports the cause.
pub(crate) fn project(
    scope: Scope<'_>,
    named: &[flatbuffers::NamedTuple<'_>],
) -> Result<v1::PodbufProjection, String> {
    let packages = Packages {
        package: scope.package,
        others: scope.others,
    };
    let mut walk = Walk::default();

    for (index, decl) in scope.package.decls.iter().enumerate() {
        let head = match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => podbuf::struct_head(packages, &decl.name, def),
            Some(v2::decl::Kind::UnionDef(def)) => podbuf::union_head(packages, &decl.name, def),
            _ => continue,
        }
        .map_err(|error| format!("{}: {error:?}", decl.name))?;
        let index = index as u32;
        walk.root(
            head,
            v1::podbuf_head::Source::Declaration(index),
            Some(index),
            vec![decl.name.clone()],
        );
    }
    for tuple in named {
        let packages = Packages {
            package: tuple.home,
            others: scope.others,
        };
        let head = podbuf::tuple_head(packages, tuple.wire, tuple.source)
            .map_err(|error| format!("{}: {error:?}", tuple.wire))?;
        walk.root(
            head,
            v1::podbuf_head::Source::Tuple(tuple.index),
            None,
            vec![tuple.wire.to_string()],
        );
    }

    let roots = std::mem::take(&mut walk.pending);
    for root in roots {
        walk.fill(root.index, &root.head, &root.path, root.declaration)?;
    }
    Ok(v1::PodbufProjection { heads: walk.heads })
}

/// A head whose fields are still to be converted.
struct Root {
    index: u32,
    head: Head,
    path: Vec<String>,
    declaration: Option<u32>,
}

#[derive(Default)]
struct Walk {
    heads: Vec<v1::PodbufHead>,
    /// Every head of a declaration or a tuple with its index, the heads an
    /// inline field is matched against.
    named: Vec<(Head, u32)>,
    pending: Vec<Root>,
}

impl Walk {
    /// Reserves the slot of a declaration's or a tuple's head. The fields are
    /// filled in later, after every such head has an index, so that an inline
    /// field can name any of them.
    fn root(
        &mut self,
        head: Head,
        source: v1::podbuf_head::Source,
        declaration: Option<u32>,
        path: Vec<String>,
    ) {
        let index = self.push(source);
        self.named.push((head.clone(), index));
        self.pending.push(Root {
            index,
            head,
            path,
            declaration,
        });
    }

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
        declaration: Option<u32>,
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
        declaration: Option<u32>,
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
            Form::Inline(head) => {
                let found = self.named.iter().find(|(named, _)| named == &**head);
                match found {
                    Some((_, index)) => Kind::InlineHead(*index),
                    None => {
                        return Err(format!(
                            "{}: an inline head with no declaration",
                            path.join(".")
                        ));
                    }
                }
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

    fn descriptor(
        &mut self,
        descriptor: &DescriptorKind,
        path: &[String],
        declaration: Option<u32>,
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
                let union = declaration.ok_or_else(|| {
                    format!(
                        "{}: a union arm outside a union declaration",
                        path.join(".")
                    )
                })?;
                let mut out = Vec::with_capacity(arms.len());
                for (ordinal, head) in arms {
                    let index = self.push(v1::podbuf_head::Source::Arm(v1::ArmHead {
                        declaration: union,
                        arm: *ordinal,
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
