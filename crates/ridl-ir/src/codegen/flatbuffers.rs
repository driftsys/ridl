//! The FlatBuffers projection, carried whole, plus the wire kind of every
//! slot and the names of the induced tables (design note D-7).
//!
//! [`crate::projection::flatbuffers`] already computes the layouts, the
//! discriminant, the bound and the root. What is added here is what the `.fbs`
//! emitter and the payload codec each derive from the IR and must agree on
//! byte for byte: the wire kind of every slot with the scalar's FlatBuffers
//! type and byte width, and the names of the induced tables — `<Name>Box`,
//! `<Union><Arm>Box`, `<path>Entry`, and a tuple's table — which the schema
//! emitter spells and the codec repeats in its function names.
//!
//! The walk order is the `.fbs` emitter's own: the declared tables in
//! declaration order, then the induced tables in the order the walk reached
//! them, with a repeated name collapsed to one table.
//!
//! Two spellings of one induced name run through this walk. The `.fbs`
//! emitter inserts `Entry` at a map position and the model's wire induced
//! name does not (design note §3.2 gives the proto3 rule, which the
//! FlatBuffers emitter follows everywhere but there), so a table carries the
//! emitter's name and is keyed back into the model by the model's.

use std::collections::HashMap;

use super::names::joined_camel;
use super::resolve::Scope;
use super::unbounded;
use super::v1;
use crate::projection::flatbuffers as fb;
use crate::v2;

/// The projection plus the lookups the rest of the lowering patches back into
/// the model.
pub(crate) struct Projected {
    pub projection: v1::FlatBuffersProjection,
    /// A tuple's table, by the wire induced name the model carries.
    pub tuple_tables: HashMap<String, u32>,
    /// A map's entry table, by the wire induced name of the map's position.
    pub entry_tables: HashMap<String, u32>,
    /// A union arm's box table, by `(declaration index, arm index)`.
    pub arm_boxes: HashMap<(u32, u32), u32>,
}

/// One table the projection induces, before its slots are filled.
#[derive(Clone)]
enum Spec<'a> {
    Struct {
        decl: u32,
        def: &'a v2::StructDef,
    },
    UnionWrapper {
        decl: u32,
    },
    /// A union arm's box (ADR-0019 decision 2) or a declaration's root box
    /// (decision 8): one table shape, holding one resolved value.
    Boxed {
        decl: u32,
        arm: Option<u32>,
        wrapped: String,
    },
    Tuple {
        tuple: Option<u32>,
        source: &'a v2::TupleType,
        wire: String,
    },
    Entry {
        map: &'a v2::MapType,
        wire: String,
    },
}

#[derive(Clone)]
struct Table<'a> {
    name: String,
    spec: Spec<'a>,
    home: &'a v2::Package,
}

/// The name a container induced at one position takes, in both spellings.
#[derive(Clone)]
struct Hints {
    fbs: String,
    wire: String,
}

impl Hints {
    fn same(name: String) -> Self {
        Self {
            fbs: name.clone(),
            wire: name,
        }
    }

    fn extend(&self, suffix: &str) -> Self {
        Self {
            fbs: format!("{}{suffix}", self.fbs),
            wire: format!("{}{suffix}", self.wire),
        }
    }
}

pub(crate) fn project<'a>(
    scope: Scope<'a>,
    tuple_index: &HashMap<String, u32>,
    foreign_index: &HashMap<(String, String), u32>,
) -> Projected {
    let mut walk = Walk {
        scope,
        tables: Vec::new(),
        claimed: HashMap::new(),
        queue: Vec::new(),
        tuple_index,
        foreign_index,
    };
    walk.declared();
    walk.drain();
    walk.finish()
}

struct Walk<'a, 'i> {
    scope: Scope<'a>,
    tables: Vec<Table<'a>>,
    claimed: HashMap<String, u32>,
    queue: Vec<Table<'a>>,
    tuple_index: &'i HashMap<String, u32>,
    foreign_index: &'i HashMap<(String, String), u32>,
}

impl<'a> Walk<'a, '_> {
    /// The tables a declaration writes itself: a struct's own, and a union's
    /// wrapper. A named scalar, an enum and an enum set are rooted in a box,
    /// which is induced.
    fn declared(&mut self) {
        let package = self.scope.package;
        for (index, decl) in package.decls.iter().enumerate() {
            let index = index as u32;
            match &decl.kind {
                Some(v2::decl::Kind::StructDef(def)) => {
                    self.push(Table {
                        name: decl.name.clone(),
                        spec: Spec::Struct { decl: index, def },
                        home: package,
                    });
                    for member in &def.members {
                        if let Some(v2::struct_member::Member::Field(field)) = &member.member
                            && let Some(ty) = field.r#type.as_ref()
                        {
                            let hints =
                                Hints::same(format!("{}{}", decl.name, joined_camel(&field.name)));
                            self.discover(package, ty, &hints);
                        }
                    }
                }
                Some(v2::decl::Kind::UnionDef(def)) => {
                    self.push(Table {
                        name: decl.name.clone(),
                        spec: Spec::UnionWrapper { decl: index },
                        home: package,
                    });
                    for (arm_index, arm) in def.arms.iter().enumerate() {
                        if self.arm_needs_box(package, &arm.type_ref) {
                            self.queue.push(Table {
                                name: format!("{}{}Box", decl.name, joined_camel(&arm.name)),
                                spec: Spec::Boxed {
                                    decl: index,
                                    arm: Some(arm_index as u32),
                                    wrapped: arm.type_ref.clone(),
                                },
                                home: package,
                            });
                        }
                    }
                }
                Some(
                    v2::decl::Kind::TypeDef(_)
                    | v2::decl::Kind::EnumDef(_)
                    | v2::decl::Kind::EnumSetDef(_),
                ) => {
                    self.queue.push(Table {
                        name: format!("{}Box", decl.name),
                        spec: Spec::Boxed {
                            decl: index,
                            arm: None,
                            wrapped: decl.name.clone(),
                        },
                        home: package,
                    });
                }
                _ => {}
            }
        }
    }

    fn arm_needs_box(&self, home: &'a v2::Package, reference: &str) -> bool {
        matches!(
            self.scope
                .resolve(home, reference)
                .map(|(decl, _)| &decl.kind),
            Some(Some(
                v2::decl::Kind::TypeDef(_)
                    | v2::decl::Kind::EnumDef(_)
                    | v2::decl::Kind::EnumSetDef(_)
            ))
        )
    }

    /// The containers one type position induces a table for.
    fn discover(&mut self, home: &'a v2::Package, ty: &'a v2::FieldType, hints: &Hints) {
        match ty.kind.as_ref() {
            Some(v2::field_type::Kind::Array(array)) => {
                if let Some(element) = array.element.as_deref() {
                    self.discover(home, element, &hints.extend("Element"));
                }
            }
            Some(v2::field_type::Kind::Map(map)) => {
                self.queue.push(Table {
                    name: format!("{}Entry", hints.fbs),
                    spec: Spec::Entry {
                        map,
                        wire: hints.wire.clone(),
                    },
                    home,
                });
            }
            Some(v2::field_type::Kind::Tuple(tuple)) => {
                self.queue.push(Table {
                    name: hints.fbs.clone(),
                    spec: Spec::Tuple {
                        tuple: self.tuple_index.get(&hints.wire).copied(),
                        source: tuple,
                        wire: hints.wire.clone(),
                    },
                    home,
                });
            }
            _ => {}
        }
    }

    fn push(&mut self, table: Table<'a>) -> bool {
        if self.claimed.contains_key(&table.name) {
            return false;
        }
        self.claimed
            .insert(table.name.clone(), self.tables.len() as u32);
        self.tables.push(table);
        true
    }

    /// The induced worklist, drained rather than iterated once: filling one
    /// induced table can discover a container nested inside it.
    fn drain(&mut self) {
        let mut index = 0;
        while index < self.queue.len() {
            let table = self.queue[index].clone();
            index += 1;
            if !self.push(table.clone()) {
                continue;
            }
            let home = table.home;
            match &table.spec {
                Spec::Tuple { source, wire, .. } => {
                    for (position, field) in source.fields.iter().enumerate() {
                        if let Some(ty) = field.r#type.as_ref() {
                            let suffix = format!("Field{}", position + 1);
                            self.discover(
                                home,
                                ty,
                                &Hints {
                                    fbs: format!("{}{suffix}", table.name),
                                    wire: format!("{wire}{suffix}"),
                                },
                            );
                        }
                    }
                }
                Spec::Entry { map, wire } => {
                    if let Some(key) = map.key.as_deref() {
                        self.discover(
                            home,
                            key,
                            &Hints {
                                fbs: format!("{}Key", table.name),
                                wire: format!("{wire}Key"),
                            },
                        );
                    }
                    if let Some(value) = map.value.as_deref() {
                        self.discover(
                            home,
                            value,
                            &Hints {
                                fbs: format!("{}Value", table.name),
                                wire: format!("{wire}Value"),
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn finish(self) -> Projected {
        let Walk {
            scope,
            tables,
            claimed,
            foreign_index,
            ..
        } = self;
        let mut tuple_tables = HashMap::new();
        let mut entry_tables = HashMap::new();
        let mut arm_boxes = HashMap::new();
        let filler = Filler {
            scope,
            claimed: &claimed,
            foreign_index,
        };

        let mut out = Vec::with_capacity(tables.len());
        for (index, table) in tables.iter().enumerate() {
            let index = index as u32;
            match &table.spec {
                Spec::Tuple { wire, .. } => {
                    tuple_tables.insert(wire.clone(), index);
                }
                Spec::Entry { wire, .. } => {
                    entry_tables.insert(wire.clone(), index);
                }
                Spec::Boxed {
                    decl,
                    arm: Some(arm),
                    ..
                } => {
                    arm_boxes.insert((*decl, *arm), index);
                }
                _ => {}
            }
            out.push(filler.table(table));
        }

        Projected {
            projection: v1::FlatBuffersProjection {
                roots: roots(scope),
                tables: out,
            },
            tuple_tables,
            entry_tables,
            arm_boxes,
        }
    }
}

/// Fills one table's slots, once every table has an index.
struct Filler<'a, 'c> {
    scope: Scope<'a>,
    claimed: &'c HashMap<String, u32>,
    foreign_index: &'c HashMap<(String, String), u32>,
}

impl Filler<'_, '_> {
    fn table(&self, table: &Table<'_>) -> v1::FbTable {
        let (layout, slots, source) = match &table.spec {
            Spec::Struct { decl, def } => {
                let layout = fb::struct_table(&table.name, def)
                    .unwrap_or(fb::TableLayout { slots: Vec::new() });
                let slots = self.struct_slots(table.home, &table.name, &layout, def);
                (
                    layout,
                    slots,
                    v1::fb_table::Source::StructDeclaration(*decl),
                )
            }
            Spec::UnionWrapper { decl } => {
                let layout = fb::union_wrapper_table();
                let slots = layout
                    .slots
                    .iter()
                    .map(|slot| v1::FbSlot {
                        id: slot.id,
                        // The wrapper's value slot holds the FlatBuffers union
                        // itself; its arms are on `Union.arms`, each with the
                        // box it is isolated in, so there is no one wire kind
                        // to state here.
                        wire: None,
                        needs_null_default: false,
                        source: Some(v1::fb_slot::Source::Wrapped(true)),
                    })
                    .collect();
                (layout, slots, v1::fb_table::Source::UnionWrapper(*decl))
            }
            Spec::Boxed { decl, arm, wrapped } => {
                let layout = fb::union_arm_box_table();
                let (wire, needs_null_default) = self.reference_wire(table.home, wrapped);
                let slots = layout
                    .slots
                    .iter()
                    .map(|slot| v1::FbSlot {
                        id: slot.id,
                        wire: wire.clone(),
                        needs_null_default,
                        source: Some(v1::fb_slot::Source::Wrapped(true)),
                    })
                    .collect();
                let source = match arm {
                    Some(arm) => v1::fb_table::Source::ArmBox(v1::ArmBox {
                        declaration: *decl,
                        arm: *arm,
                    }),
                    None => v1::fb_table::Source::RootBox(*decl),
                };
                (layout, slots, source)
            }
            Spec::Tuple { tuple, source, .. } => {
                let layout = fb::tuple_table(&table.name, source)
                    .unwrap_or(fb::TableLayout { slots: Vec::new() });
                let slots = layout
                    .slots
                    .iter()
                    .zip(source.fields.iter())
                    .map(|(slot, field)| {
                        let position = match slot.source {
                            fb::SlotSource::TupleField { position } => position,
                            _ => 0,
                        };
                        let (wire, needs_null_default) = self.type_wire(
                            table.home,
                            field.r#type.as_ref(),
                            &format!("{}Field{position}", table.name),
                        );
                        v1::FbSlot {
                            id: slot.id,
                            wire,
                            needs_null_default,
                            source: Some(v1::fb_slot::Source::TuplePosition(position)),
                        }
                    })
                    .collect();
                (
                    layout,
                    slots,
                    v1::fb_table::Source::Tuple(tuple.unwrap_or_default()),
                )
            }
            Spec::Entry { map, .. } => {
                let layout = fb::map_entry_table();
                let (key_wire, key_null) = self.type_wire(
                    table.home,
                    map.key.as_deref(),
                    &format!("{}Key", table.name),
                );
                let (value_wire, value_null) = self.type_wire(
                    table.home,
                    map.value.as_deref(),
                    &format!("{}Value", table.name),
                );
                let slots = vec![
                    v1::FbSlot {
                        id: fb::MAP_ENTRY_KEY_ID,
                        wire: key_wire,
                        needs_null_default: key_null,
                        source: Some(v1::fb_slot::Source::Key(true)),
                    },
                    v1::FbSlot {
                        id: fb::MAP_ENTRY_VALUE_ID,
                        wire: value_wire,
                        needs_null_default: value_null,
                        source: Some(v1::fb_slot::Source::Value(true)),
                    },
                ];
                (
                    layout,
                    slots,
                    v1::fb_table::Source::MapEntry(v1::MapEntry {
                        path: vec![table.name.clone()],
                    }),
                )
            }
        };
        v1::FbTable {
            name: table.name.clone(),
            slots,
            vtable_slots: u32::try_from(layout.vtable_slots()).unwrap_or(u32::MAX),
            source: Some(source),
        }
    }

    fn struct_slots(
        &self,
        home: &v2::Package,
        owner: &str,
        layout: &fb::TableLayout,
        def: &v2::StructDef,
    ) -> Vec<v1::FbSlot> {
        let members: Vec<&v2::struct_member::Member> = def
            .members
            .iter()
            .filter_map(|member| member.member.as_ref())
            .collect();
        layout
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| match members.get(index) {
                Some(v2::struct_member::Member::Field(field)) => {
                    let hint = format!("{owner}{}", joined_camel(&field.name));
                    let (wire, needs_null_default) =
                        self.type_wire(home, field.r#type.as_ref(), &hint);
                    v1::FbSlot {
                        id: slot.id,
                        wire,
                        needs_null_default,
                        source: Some(v1::fb_slot::Source::Field(index as u32)),
                    }
                }
                Some(v2::struct_member::Member::Reserved(reserved)) => v1::FbSlot {
                    id: slot.id,
                    // The placeholder holding a retired ordinal is a `ubyte`,
                    // marked deprecated.
                    wire: Some(scalar_wire(v1::FbScalarType::Ubyte, 1)),
                    needs_null_default: false,
                    source: Some(v1::fb_slot::Source::RetiredOrdinal(reserved.ordinal)),
                },
                None => v1::FbSlot {
                    id: slot.id,
                    wire: None,
                    needs_null_default: false,
                    source: None,
                },
            })
            .collect()
    }

    /// What one type position holds on the wire, and whether a field at that
    /// position needs an explicit `= null` default (ADR-0019 decision 6).
    ///
    /// A reference into another package resolves to a table this package's
    /// schema does not declare, so no wire kind is stated for it; the
    /// reference itself is on the owning field's `TypeRef`.
    fn type_wire(
        &self,
        home: &v2::Package,
        ty: Option<&v2::FieldType>,
        hint: &str,
    ) -> (Option<v1::FbWire>, bool) {
        let Some(ty) = ty else {
            return (None, false);
        };
        match ty.kind.as_ref() {
            Some(v2::field_type::Kind::Primitive(primitive)) => {
                (Some(primitive_wire(*primitive)), false)
            }
            Some(v2::field_type::Kind::InlineScalar(td)) => (Some(scalar_def_wire(td)), false),
            Some(v2::field_type::Kind::Named(reference)) => self.reference_wire(home, reference),
            Some(v2::field_type::Kind::Array(array)) => {
                let (element, _) =
                    self.type_wire(home, array.element.as_deref(), &format!("{hint}Element"));
                let wire = element.map(|element| v1::FbWire {
                    kind: Some(v1::fb_wire::Kind::Vector(Box::new(v1::FbVector {
                        element: Some(Box::new(element)),
                        min: array.min,
                        max: array.max,
                    }))),
                });
                (wire, false)
            }
            Some(v2::field_type::Kind::Map(map)) => {
                let wire = self
                    .claimed
                    .get(&format!("{hint}Entry"))
                    .map(|entry| v1::FbWire {
                        kind: Some(v1::fb_wire::Kind::Map(v1::FbMap {
                            entry_table: *entry,
                            min: map.min,
                            max: map.max,
                        })),
                    });
                (wire, false)
            }
            Some(v2::field_type::Kind::Tuple(_)) => {
                let wire = self.claimed.get(hint).map(|table| v1::FbWire {
                    kind: Some(v1::fb_wire::Kind::Table(*table)),
                });
                (wire, false)
            }
            _ => (None, false),
        }
    }

    /// Where the model holds the declaration `name` of `declaring`: its own
    /// index for a declaration of the package being lowered, and the index
    /// the lowering gave it in `Model.foreign` otherwise.
    fn declaration_index(&self, declaring: &v2::Package, name: &str, local: bool) -> u32 {
        if local {
            declaring
                .decls
                .iter()
                .position(|decl| decl.name == name)
                .unwrap_or(0) as u32
        } else {
            self.foreign_index
                .get(&(declaring.name.clone(), name.to_string()))
                .copied()
                .unwrap_or(0)
        }
    }

    fn reference_wire(&self, home: &v2::Package, reference: &str) -> (Option<v1::FbWire>, bool) {
        let Some((decl, declaring)) = self.scope.resolve(home, reference) else {
            return (None, false);
        };
        let local = declaring.name == self.scope.package.name;
        match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => (Some(scalar_def_wire(td)), false),
            // A FlatBuffers enum field names the enum, and every typl enum is
            // emitted at one underlying width, `long`.
            Some(v2::decl::Kind::EnumDef(def)) => (
                Some(v1::FbWire {
                    kind: Some(v1::fb_wire::Kind::Enum(v1::FbEnum {
                        r#type: Some(v1::TypeRef {
                            reference: reference.to_string(),
                            resolved: true,
                            package: declaring.name.clone(),
                            foreign: reference.contains('.'),
                            index: self.declaration_index(declaring, &decl.name, local),
                            kind: v1::DeclKind::Enum as i32,
                        }),
                        width_bytes: 8,
                    })),
                }),
                fb::enum_field_needs_null_default(def),
            ),
            Some(v2::decl::Kind::EnumSetDef(def)) => (Some(int_width_wire(def.width)), false),
            Some(v2::decl::Kind::StructDef(_)) if local => (
                self.claimed.get(&decl.name).map(|index| v1::FbWire {
                    kind: Some(v1::fb_wire::Kind::Table(*index)),
                }),
                false,
            ),
            Some(v2::decl::Kind::UnionDef(_)) if local => (
                self.claimed.get(&decl.name).map(|index| v1::FbWire {
                    kind: Some(v1::fb_wire::Kind::UnionWrapper(*index)),
                }),
                false,
            ),
            _ => (None, false),
        }
    }
}

/// One root per declaration `root_table` names, in declaration order. The
/// bound is `max_size` over the whole scope — the number the codec emits as
/// `MAX_SIZE` — and a missing bound is attributed over the package alone,
/// which is what the Rust backend does today (design note §9 item 7).
fn roots(scope: Scope<'_>) -> Vec<v1::FbRoot> {
    let package = scope.package;
    let others = scope.projection_others(package);
    let packages = fb::Packages {
        package,
        others: &others,
    };
    package
        .decls
        .iter()
        .enumerate()
        .filter_map(|(index, decl)| {
            let root = fb::root_table(decl)?;
            let kind = match root {
                fb::RootTable::Own => v1::FbRootKind::Own,
                fb::RootTable::UnionWrapper => v1::FbRootKind::UnionWrapper,
                fb::RootTable::Box => v1::FbRootKind::Box,
            };
            let bound = match fb::max_size(packages, decl) {
                Some(size) => v1::fb_root::Bound::MaxSize(u32::try_from(size).unwrap_or(u32::MAX)),
                None => v1::fb_root::Bound::Unbounded(unbounded::attribute(package, decl)),
            };
            Some(v1::FbRoot {
                declaration: index as u32,
                root: kind as i32,
                bound: Some(bound),
            })
        })
        .collect()
}

fn primitive_wire(primitive: i32) -> v1::FbWire {
    match v2::PrimitiveType::try_from(primitive).unwrap_or(v2::PrimitiveType::Unspecified) {
        v2::PrimitiveType::Boolean => scalar_wire(v1::FbScalarType::Bool, 1),
        v2::PrimitiveType::Integer => scalar_wire(v1::FbScalarType::Long, 8),
        v2::PrimitiveType::Float => scalar_wire(v1::FbScalarType::Double, 8),
        v2::PrimitiveType::Bytes => v1::FbWire {
            kind: Some(v1::fb_wire::Kind::Bytes(true)),
        },
        v2::PrimitiveType::String | v2::PrimitiveType::Unspecified => v1::FbWire {
            kind: Some(v1::fb_wire::Kind::Text(true)),
        },
    }
}

/// The wire kind of a named or inline scalar, at the width the IR derived —
/// the table `fbs_scalar` spells as a keyword.
fn scalar_def_wire(td: &v2::TypeDef) -> v1::FbWire {
    match &td.width {
        Some(v2::type_def::Width::IntWidth(width)) => int_width_wire(*width),
        Some(v2::type_def::Width::FloatWidth(width)) => {
            match v2::FloatWidth::try_from(*width).unwrap_or(v2::FloatWidth::Unspecified) {
                v2::FloatWidth::F32 => scalar_wire(v1::FbScalarType::Float, 4),
                _ => scalar_wire(v1::FbScalarType::Double, 8),
            }
        }
        None => match td
            .backing
            .as_ref()
            .and_then(|backing| backing.kind.as_ref())
        {
            Some(v2::backing::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive)
                    .unwrap_or(v2::PrimitiveType::Unspecified)
                {
                    v2::PrimitiveType::Boolean => scalar_wire(v1::FbScalarType::Bool, 1),
                    v2::PrimitiveType::Bytes => v1::FbWire {
                        kind: Some(v1::fb_wire::Kind::Bytes(true)),
                    },
                    _ => v1::FbWire {
                        kind: Some(v1::fb_wire::Kind::Text(true)),
                    },
                }
            }
            _ => v1::FbWire {
                kind: Some(v1::fb_wire::Kind::Text(true)),
            },
        },
    }
}

fn int_width_wire(width: i32) -> v1::FbWire {
    match v2::IntWidth::try_from(width).unwrap_or(v2::IntWidth::Unspecified) {
        v2::IntWidth::U8 => scalar_wire(v1::FbScalarType::Ubyte, 1),
        v2::IntWidth::I8 => scalar_wire(v1::FbScalarType::Byte, 1),
        v2::IntWidth::U16 => scalar_wire(v1::FbScalarType::Ushort, 2),
        v2::IntWidth::I16 => scalar_wire(v1::FbScalarType::Short, 2),
        v2::IntWidth::U32 => scalar_wire(v1::FbScalarType::Uint, 4),
        v2::IntWidth::I32 => scalar_wire(v1::FbScalarType::Int, 4),
        v2::IntWidth::U64 => scalar_wire(v1::FbScalarType::Ulong, 8),
        _ => scalar_wire(v1::FbScalarType::Long, 8),
    }
}

fn scalar_wire(kind: v1::FbScalarType, width: u32) -> v1::FbWire {
    v1::FbWire {
        kind: Some(v1::fb_wire::Kind::Scalar(v1::FbScalar {
            r#type: kind as i32,
            width_bytes: width,
        })),
    }
}
