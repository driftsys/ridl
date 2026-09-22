//! The lowering: one IR package, over a stated scope, into one
//! [`v1::Model`](super::v1::Model).
//!
//! Total by construction (design note D-1). Malformed IR is carried as a fact
//! — a [`TupleCollision`](super::v1::TupleCollision), an
//! [`FbUnbounded`](super::v1::FbUnbounded) with its attribution, a
//! [`Type`](super::v1::Type) with no kind — rather than turned into an error,
//! because every printer refuses those today with its own message and must
//! keep doing so byte for byte.
//!
//! Positional correspondence is an invariant: `declarations[i]` is lowered
//! from `Package.decls[i]`, `interfaces[i]` from the `i`-th shape of
//! `Package::shapes()`, a `Struct.slots[j]` from `StructDef.members[j]`, an
//! `Interface.slots[j]` from `Interface.interactions[j]`.

use std::collections::HashMap;

use super::clauses;
use super::facts::{Closures, Inits, scalar_class};
use super::flatbuffers;
use super::names::{dotted_name, joined_camel, spellings};
use super::resolve::{Scope, is_foreign};
use super::v1;
use crate::name::camel_case;
use crate::projection::flatbuffers as fb;
use crate::v2;

/// Lowers one package over the scope `others` (design note D-1).
///
/// The scope is part of the model: the output of every backend depends on it,
/// and a plugin must be able to tell "this reference names a package I was
/// not given" from "this reference names nothing".
pub fn lower(package: &v2::Package, others: &[&v2::Package]) -> v1::Model {
    let scope = Scope { package, others };
    let mut lowering = Lowering {
        scope,
        inits: Inits::new(scope),
        closures: Closures::new(scope),
        foreign: Vec::new(),
        foreign_at: HashMap::new(),
        tuples: Vec::new(),
        tuple_at: HashMap::new(),
        collisions: Vec::new(),
    };

    let declarations: Vec<v1::Declaration> = package
        .decls
        .iter()
        .enumerate()
        .map(|(index, decl)| {
            let path = Path::declaration(index as u32, decl, &decl.name);
            lowering.declaration(package, decl, Some(path))
        })
        .collect();
    lowering.drain_tuples();

    let interfaces = lowering.shapes();
    lowering.drain_tuples();

    let mut model = v1::Model {
        name: Some(dotted_name(&package.name)),
        scope: Some(v1::Scope {
            package: package.name.clone(),
            others: others.iter().map(|other| other.name.clone()).collect(),
        }),
        declarations,
        tuples: lowering
            .tuples
            .iter()
            .map(|entry| entry.lowered.clone().unwrap_or_default())
            .collect(),
        interfaces,
        services: lowering.services(),
        catalog: Some(lowering.catalog()),
        flatbuffers: None,
        foreign: lowering.foreign.clone(),
        tuple_collisions: lowering.collisions.clone(),
    };

    // The FlatBuffers projection is its own walk over the IR, in the order the
    // `.fbs` emitter reaches its tables; it is keyed back into the model by
    // the wire induced names the walk above assigned.
    let tuple_index: HashMap<String, u32> = lowering
        .tuples
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.named)
        .map(|(index, entry)| (entry.wire.clone(), index as u32))
        .collect();
    let projected = flatbuffers::project(scope, &tuple_index);
    patch_flatbuffers(&mut model, &projected);
    model.flatbuffers = Some(projected.projection);
    model
}

/// The path that reached one type position: where the model records the
/// origin of a tuple found there, and the two induced names it would take.
#[derive(Clone)]
struct Path {
    base: Base,
    segments: Vec<String>,
    rust: String,
    wire: String,
    visibility: i32,
    /// False where no backend has a naming rule for the origin, which is
    /// every interaction position today.
    named: bool,
}

#[derive(Clone, Copy)]
enum Base {
    Declaration(u32),
    Interaction(u32, u32),
    /// Reached inside a declaration of another package, which has no index in
    /// `Model.declarations` to name.
    Foreign,
}

impl Path {
    fn declaration(index: u32, decl: &v2::Decl, owner: &str) -> Self {
        Self {
            base: Base::Declaration(index),
            segments: Vec::new(),
            rust: camel_case(owner),
            wire: owner.to_string(),
            visibility: decl.visibility,
            named: true,
        }
    }

    fn foreign(owner: &str, visibility: i32) -> Self {
        Self {
            base: Base::Foreign,
            segments: Vec::new(),
            rust: camel_case(owner),
            wire: owner.to_string(),
            visibility,
            named: false,
        }
    }

    fn interaction(interface: u32, slot: u32, visibility: i32, segments: Vec<String>) -> Self {
        Self {
            base: Base::Interaction(interface, slot),
            segments,
            rust: String::new(),
            wire: String::new(),
            visibility,
            named: false,
        }
    }

    /// A struct field, where both induced names begin.
    fn field(&self, name: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(name.to_string());
        Self {
            base: self.base,
            segments,
            rust: format!("{}{}", self.rust, camel_case(name)),
            wire: format!("{}{}", self.wire, joined_camel(name)),
            visibility: self.visibility,
            named: self.named,
        }
    }

    /// A positional word: `Element`, `Key`, `Value`.
    fn step(&self, word: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(word.to_string());
        Self {
            base: self.base,
            segments,
            rust: format!("{}{word}", self.rust),
            wire: format!("{}{word}", self.wire),
            visibility: self.visibility,
            named: self.named,
        }
    }

    /// A tuple field: the Rust name extends by the field's CamelCase, the
    /// wire name by the position (design note D-2).
    fn tuple_field(&self, name: &str, position: usize) -> Self {
        let mut segments = self.segments.clone();
        segments.push(name.to_string());
        Self {
            base: self.base,
            segments,
            rust: format!("{}{}", self.rust, camel_case(name)),
            wire: format!("{}Field{position}", self.wire),
            visibility: self.visibility,
            named: self.named,
        }
    }

    fn origin(&self) -> Option<v1::induced_tuple::Origin> {
        match self.base {
            Base::Declaration(index) => {
                Some(v1::induced_tuple::Origin::Declaration(v1::TuplePath {
                    declaration: index,
                    segments: self.segments.clone(),
                }))
            }
            Base::Interaction(interface, slot) => Some(v1::induced_tuple::Origin::Interaction(
                v1::InteractionPath {
                    interface,
                    slot,
                    segments: self.segments.clone(),
                },
            )),
            Base::Foreign => None,
        }
    }
}

/// One tuple lifted out of the type graph, before its own fields are lowered.
struct TupleEntry<'a> {
    source: v2::TupleType,
    home: &'a v2::Package,
    path: Path,
    rust: String,
    wire: String,
    named: bool,
    lowered: Option<v1::InducedTuple>,
}

struct Lowering<'a> {
    scope: Scope<'a>,
    inits: Inits<'a>,
    closures: Closures<'a>,
    foreign: Vec<v1::ForeignDeclaration>,
    foreign_at: HashMap<(String, String), u32>,
    tuples: Vec<TupleEntry<'a>>,
    tuple_at: HashMap<String, usize>,
    collisions: Vec<v1::TupleCollision>,
}

impl<'a> Lowering<'a> {
    // -----------------------------------------------------------------
    // Declarations
    // -----------------------------------------------------------------

    fn declaration(
        &mut self,
        home: &'a v2::Package,
        decl: &v2::Decl,
        path: Option<Path>,
    ) -> v1::Declaration {
        let is_constant = matches!(decl.kind, Some(v2::decl::Kind::ConstDef(_)));
        v1::Declaration {
            name: Some(spellings(&decl.name)),
            visibility: decl.visibility,
            is_error: decl.is_error,
            doc: decl.doc.clone(),
            labels: decl.labels.clone(),
            deprecated: decl.deprecated.clone(),
            closure: (!is_constant).then(|| self.closures.declaration(home, decl)),
            init: (!is_constant).then(|| self.inits.declaration(home, decl)),
            kind: self.declaration_kind(home, decl, path),
        }
    }

    fn declaration_kind(
        &mut self,
        home: &'a v2::Package,
        decl: &v2::Decl,
        path: Option<Path>,
    ) -> Option<v1::declaration::Kind> {
        let path = path.unwrap_or_else(|| Path::foreign(&decl.name, decl.visibility));
        match decl.kind.as_ref()? {
            v2::decl::Kind::TypeDef(td) => {
                Some(v1::declaration::Kind::Scalar(self.scalar(td, true)))
            }
            v2::decl::Kind::ConstDef(cd) => {
                Some(v1::declaration::Kind::Constant(self.constant(home, cd)))
            }
            v2::decl::Kind::StructDef(def) => Some(v1::declaration::Kind::Struct(v1::Struct {
                slots: def
                    .members
                    .iter()
                    .map(|member| self.slot(home, member, &path))
                    .collect(),
                fixed_layout: def.fixed_layout,
            })),
            v2::decl::Kind::EnumDef(def) => Some(v1::declaration::Kind::Enum(enum_body(def))),
            v2::decl::Kind::EnumSetDef(def) => {
                Some(v1::declaration::Kind::EnumSet(self.enum_set(home, def)))
            }
            v2::decl::Kind::UnionDef(def) => {
                Some(v1::declaration::Kind::Union(self.union(home, def)))
            }
            // An interaction rides `Interface.interactions`, never a package
            // declaration, and a reserved slot is not a declaration either.
            _ => None,
        }
    }

    fn slot(&mut self, home: &'a v2::Package, member: &v2::StructMember, path: &Path) -> v1::Slot {
        match &member.member {
            Some(v2::struct_member::Member::Field(field)) => v1::Slot {
                ordinal: field.ordinal,
                occupant: Some(v1::slot::Occupant::Field(Box::new(
                    self.field(home, field, path),
                ))),
            },
            Some(v2::struct_member::Member::Reserved(reserved)) => v1::Slot {
                ordinal: reserved.ordinal,
                occupant: Some(v1::slot::Occupant::Retired(retired(reserved))),
            },
            None => v1::Slot {
                ordinal: 0,
                occupant: None,
            },
        }
    }

    fn field(&mut self, home: &'a v2::Package, field: &v2::Field, path: &Path) -> v1::Field {
        let at = path.field(&field.name);
        v1::Field {
            name: Some(spellings(&field.name)),
            r#type: field.r#type.as_ref().map(|ty| self.type_at(home, ty, &at)),
            declared_init: field.declared_init.clone(),
            init: Some(self.inits.field(home, field)),
            doc: field.doc.clone(),
            labels: field.labels.clone(),
            deprecated: field.deprecated.clone(),
        }
    }

    /// A named scalar, or the shape of an inline one. An inline scalar's
    /// `declared_init` stays unset: the enclosing field's is authoritative
    /// (`ir.proto`, `FieldType.inline_scalar`).
    fn scalar(&mut self, td: &v2::TypeDef, declared: bool) -> v1::Scalar {
        v1::Scalar {
            class: scalar_class(td) as i32,
            unit: match td
                .backing
                .as_ref()
                .and_then(|backing| backing.kind.as_ref())
            {
                Some(v2::backing::Kind::Unit(unit)) => Some(unit.clone()),
                _ => None,
            },
            constraint: td.constraint.as_ref().map(constraint),
            vacuous: crate::v2::constraint_is_vacuous(td.constraint.as_ref()),
            declared_init: declared.then(|| td.declared_init.clone()).flatten(),
            width: td.width.as_ref().map(|width| match width {
                v2::type_def::Width::IntWidth(value) => v1::scalar::Width::IntWidth(*value),
                v2::type_def::Width::FloatWidth(value) => v1::scalar::Width::FloatWidth(*value),
            }),
        }
    }

    fn constant(&mut self, home: &'a v2::Package, cd: &v2::ConstDef) -> v1::Constant {
        let typed = match (cd.regex.as_deref(), cd.type_ref.as_deref()) {
            (Some(regex), _) => Some(v1::constant::Typed::RegexBody(
                strip_regex_delimiters(regex).to_string(),
            )),
            (None, Some(reference)) => Some(match primitive_keyword(reference) {
                Some(primitive) => v1::constant::Typed::Primitive(primitive as i32),
                None => match self.type_ref(home, reference) {
                    reference if reference.resolved => v1::constant::Typed::Named(reference),
                    reference => v1::constant::Typed::Unresolved(reference.reference),
                },
            }),
            (None, None) => None,
        };
        v1::Constant {
            value: cd.value.clone(),
            typed,
        }
    }

    fn enum_set(&mut self, home: &'a v2::Package, def: &v2::EnumSetDef) -> v1::EnumSet {
        let mask = def.bits.iter().fold(0i64, |mask, bit| {
            if (0..=63).contains(&bit.value) {
                mask | (1i64 << bit.value)
            } else {
                mask
            }
        });
        v1::EnumSet {
            backing_enum: def
                .backing_enum
                .as_deref()
                .map(|reference| self.type_ref(home, reference)),
            bits: def.bits.iter().map(enum_value).collect(),
            width: def.width,
            declared_mask: mask,
        }
    }

    fn union(&mut self, home: &'a v2::Package, def: &v2::UnionDef) -> v1::Union {
        v1::Union {
            arms: def
                .arms
                .iter()
                .map(|arm| v1::Arm {
                    name: Some(spellings(&arm.name)),
                    ordinal: arm.ordinal,
                    r#type: Some(self.type_ref(home, &arm.type_ref)),
                    doc: arm.doc.clone(),
                    discriminant: fb::union_arm_discriminant(arm),
                    // Patched once the projection has its table indices.
                    flatbuffers_box: None,
                })
                .collect(),
            retired: def.reserved.iter().map(retired).collect(),
            is_result: def.is_result,
        }
    }

    // -----------------------------------------------------------------
    // Type positions
    // -----------------------------------------------------------------

    fn type_at(&mut self, home: &'a v2::Package, ty: &v2::FieldType, at: &Path) -> v1::Type {
        v1::Type {
            optional: ty.optional,
            kind: self.type_kind(home, ty, at),
        }
    }

    fn type_kind(
        &mut self,
        home: &'a v2::Package,
        ty: &v2::FieldType,
        at: &Path,
    ) -> Option<v1::r#type::Kind> {
        match ty.kind.as_ref()? {
            v2::field_type::Kind::Named(reference) => {
                Some(v1::r#type::Kind::Named(self.type_ref(home, reference)))
            }
            v2::field_type::Kind::Primitive(primitive) => {
                Some(v1::r#type::Kind::Primitive(*primitive))
            }
            v2::field_type::Kind::InlineScalar(td) => {
                Some(v1::r#type::Kind::Inline(self.scalar(td, false)))
            }
            v2::field_type::Kind::Tuple(tuple) => {
                let index = self.tuple_ref(home, tuple, at);
                Some(v1::r#type::Kind::Tuple(v1::TupleRef { index }))
            }
            v2::field_type::Kind::Array(array) => {
                let element = array
                    .element
                    .as_deref()
                    .map(|element| self.type_at(home, element, &at.step("Element")));
                Some(v1::r#type::Kind::Array(Box::new(v1::ArrayType {
                    element: element.map(Box::new),
                    min: array.min,
                    max: array.max,
                })))
            }
            v2::field_type::Kind::Map(map) => {
                let key = map
                    .key
                    .as_deref()
                    .map(|key| self.type_at(home, key, &at.step("Key")));
                let value = map
                    .value
                    .as_deref()
                    .map(|value| self.type_at(home, value, &at.step("Value")));
                Some(v1::r#type::Kind::Map(Box::new(v1::MapType {
                    key: key.map(Box::new),
                    value: value.map(Box::new),
                    min: map.min,
                    max: map.max,
                    // Patched once the projection has its table indices.
                    flatbuffers_entry_table: None,
                })))
            }
            v2::field_type::Kind::Stream(stream) => {
                Some(v1::r#type::Kind::Stream(self.stream(home, stream)))
            }
        }
    }

    fn stream(&mut self, home: &'a v2::Package, stream: &v2::StreamType) -> v1::StreamType {
        v1::StreamType {
            element: stream.element.as_ref().map(|element| match element {
                v2::stream_type::Element::Named(reference) => {
                    v1::stream_type::Element::Named(self.type_ref(home, reference))
                }
                v2::stream_type::Element::Primitive(primitive) => {
                    v1::stream_type::Element::Primitive(*primitive)
                }
            }),
        }
    }

    /// A resolved reference, flat: the declaring package, whether it is
    /// foreign, the index the declaration sits at, and its kind. A reference
    /// that names a constant is not resolved as a type.
    fn type_ref(&mut self, home: &'a v2::Package, reference: &str) -> v1::TypeRef {
        let foreign = is_foreign(reference);
        let Some((decl, declaring)) = self.scope.resolve(home, reference) else {
            return v1::TypeRef {
                reference: reference.to_string(),
                resolved: false,
                package: String::new(),
                foreign,
                index: 0,
                kind: v1::DeclKind::Unspecified as i32,
            };
        };
        let kind = decl_kind(decl);
        let local = declaring.name == self.scope.package.name;
        let index = if local {
            declaring
                .decls
                .iter()
                .position(|candidate| candidate.name == decl.name)
                .unwrap_or(0) as u32
        } else {
            self.foreign_declaration(declaring, decl)
        };
        v1::TypeRef {
            reference: reference.to_string(),
            resolved: kind != v1::DeclKind::Constant,
            package: declaring.name.clone(),
            foreign,
            index,
            kind: kind as i32,
        }
    }

    /// A copy of one foreign declaration, added to `Model.foreign` the first
    /// time this package reaches it.
    fn foreign_declaration(&mut self, declaring: &'a v2::Package, decl: &v2::Decl) -> u32 {
        let key = (declaring.name.clone(), decl.name.clone());
        if let Some(index) = self.foreign_at.get(&key) {
            return *index;
        }
        let index = self.foreign.len() as u32;
        self.foreign_at.insert(key, index);
        self.foreign.push(v1::ForeignDeclaration {
            package: declaring.name.clone(),
            declaration: None,
        });
        let lowered = self.declaration(declaring, decl, None);
        self.foreign[index as usize].declaration = Some(lowered);
        index
    }

    // -----------------------------------------------------------------
    // Induced tuples
    // -----------------------------------------------------------------

    /// Lifts one tuple position into `Model.tuples`, returning its index.
    ///
    /// A tuple whose induced name is already claimed by an identical tuple is
    /// the same generated type, so the claim is reused; one claimed by a
    /// different shape is a collision, recorded as a fact for the printer to
    /// refuse with its own message.
    fn tuple_ref(&mut self, home: &'a v2::Package, tuple: &v2::TupleType, at: &Path) -> u32 {
        if at.named
            && let Some(index) = self.tuple_at.get(&at.rust).copied()
        {
            let existing = &self.tuples[index];
            if existing.source == *tuple {
                return index as u32;
            }
            self.collisions.push(v1::TupleCollision {
                name: at.rust.clone(),
                first_fields: existing
                    .source
                    .fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect(),
                second_fields: tuple
                    .fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect(),
            });
        }
        let index = self.tuples.len();
        if at.named {
            self.tuple_at.insert(at.rust.clone(), index);
        }
        self.tuples.push(TupleEntry {
            source: tuple.clone(),
            home,
            path: at.clone(),
            rust: at.rust.clone(),
            wire: at.wire.clone(),
            named: at.named,
            lowered: None,
        });
        index as u32
    }

    /// Lowers every tuple the walk reached, in discovery order; lowering one
    /// tuple's fields can reach another, which is appended and picked up by a
    /// later pass of the same loop.
    fn drain_tuples(&mut self) {
        let mut index = 0;
        while index < self.tuples.len() {
            if self.tuples[index].lowered.is_some() {
                index += 1;
                continue;
            }
            let source = self.tuples[index].source.clone();
            let home = self.tuples[index].home;
            let path = self.tuples[index].path.clone();
            let named = self.tuples[index].named;
            let (rust, wire) = (
                self.tuples[index].rust.clone(),
                self.tuples[index].wire.clone(),
            );
            let fields: Vec<v1::Field> = source
                .fields
                .iter()
                .enumerate()
                .map(|(position, field)| {
                    let at = path.tuple_field(&field.name, position + 1);
                    v1::Field {
                        name: Some(spellings(&field.name)),
                        r#type: field.r#type.as_ref().map(|ty| self.type_at(home, ty, &at)),
                        declared_init: None,
                        init: Some(self.inits.tuple_field(home, field)),
                        doc: String::new(),
                        labels: Vec::new(),
                        deprecated: None,
                    }
                })
                .collect();
            let lowered = v1::InducedTuple {
                name: Some(v1::InducedName {
                    rust: if named { rust } else { String::new() },
                    wire: if named { wire } else { String::new() },
                }),
                fields,
                visibility: path.visibility,
                closure: Some(self.closures.tuple(home, &source)),
                init: Some(self.inits.tuple(home, &source)),
                // Patched once the projection has its table indices.
                flatbuffers_table: None,
                origin: path.origin(),
            };
            self.tuples[index].lowered = Some(lowered);
            index += 1;
        }
    }

    // -----------------------------------------------------------------
    // Interfaces, services, the catalog
    // -----------------------------------------------------------------

    fn shapes(&mut self) -> Vec<v1::Interface> {
        let package = self.scope.package;
        let shapes: Vec<v2::InterfaceShape<'a>> = package.shapes().collect();
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| self.interface(package, index as u32, shape))
            .collect()
    }

    fn interface(
        &mut self,
        home: &'a v2::Package,
        index: u32,
        shape: &v2::InterfaceShape<'a>,
    ) -> v1::Interface {
        let interface = shape.interface;
        let visibility = shape.visibility();
        let mut row = 0u32;
        let slots: Vec<v1::InteractionSlot> = interface
            .interactions
            .iter()
            .enumerate()
            .map(|(slot, decl)| {
                let ordinal = decl.ordinal;
                match decl.kind.as_ref() {
                    Some(v2::decl::Kind::ReservedSlot(reserved)) => v1::InteractionSlot {
                        ordinal,
                        occupant: Some(v1::interaction_slot::Occupant::Retired(retired(reserved))),
                    },
                    _ => {
                        let this_row = row;
                        row += 1;
                        v1::InteractionSlot {
                            ordinal,
                            occupant: Some(v1::interaction_slot::Occupant::Interaction(Box::new(
                                self.interaction(
                                    home,
                                    decl,
                                    index,
                                    slot as u32,
                                    this_row,
                                    visibility,
                                ),
                            ))),
                        }
                    }
                }
            })
            .collect();
        let inline_of_service = shape.service.and_then(|service| {
            home.services
                .iter()
                .position(|candidate| candidate.name == service.name)
                .map(|position| position as u32)
        });
        v1::Interface {
            number: interface.number,
            provisional: interface.provisional,
            visibility,
            doc: interface.doc.clone(),
            labels: interface.labels.clone(),
            deprecated: interface.deprecated.clone(),
            slots,
            inline_of_service,
            identity: Some(match shape.service {
                Some(service) => v1::interface::Identity::Service(dotted_name(&service.name)),
                None => v1::interface::Identity::Declared(spellings(&interface.name)),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn interaction(
        &mut self,
        home: &'a v2::Package,
        decl: &v2::Decl,
        interface: u32,
        slot: u32,
        row: u32,
        visibility: i32,
    ) -> v1::Interaction {
        let (kind, timing, shape) = match decl.kind.as_ref() {
            Some(v2::decl::Kind::SignalDef(signal)) => (
                v1::Kind::Signal,
                signal.timing.as_ref(),
                Some(v1::interaction::Shape::Signal(v1::SignalShape {
                    payload: Some(self.payload(home, &signal.payload)),
                    declared_init: signal.declared_init.clone(),
                    init: Some(
                        self.inits
                            .signal(signal.init.as_ref(), signal.declared_init.is_some()),
                    ),
                })),
            ),
            Some(v2::decl::Kind::EventDef(event)) => (
                v1::Kind::Event,
                event.timing.as_ref(),
                Some(v1::interaction::Shape::Event(v1::EventShape {
                    payload: Some(self.payload(home, &event.payload)),
                })),
            ),
            Some(v2::decl::Kind::CommandDef(command)) => {
                let params = self.params(home, &command.params, interface, slot, visibility);
                let request = single_param_type(&command.params)
                    .map(|reference| self.payload(home, reference));
                let clauses = self.clauses(home, &command.contracts, &command.params, None);
                (
                    v1::Kind::Command,
                    command.timing.as_ref(),
                    Some(v1::interaction::Shape::Command(v1::CommandShape {
                        params,
                        request,
                        clauses,
                    })),
                )
            }
            Some(v2::decl::Kind::QueryDef(query)) => {
                let params = self.params(home, &query.params, interface, slot, visibility);
                let request =
                    single_param_type(&query.params).map(|reference| self.payload(home, reference));
                let reply_named = query_reply_type(query);
                let reply = query.return_type.as_ref().map(|ret| {
                    let at =
                        Path::interaction(interface, slot, visibility, vec!["reply".to_string()]);
                    self.reply(home, ret, &at)
                });
                let reply_payload = reply_named.map(|reference| self.payload(home, reference));
                let clauses = self.clauses(home, &query.contracts, &query.params, reply_named);
                (
                    v1::Kind::Query,
                    query.timing.as_ref(),
                    Some(v1::interaction::Shape::Query(v1::QueryShape {
                        params,
                        request,
                        reply,
                        reply_payload,
                        clauses,
                    })),
                )
            }
            Some(v2::decl::Kind::FixedDef(fixed)) => {
                let at =
                    Path::interaction(interface, slot, visibility, vec!["payload".to_string()]);
                let payload = fixed.payload.as_ref().map(|ty| self.type_at(home, ty, &at));
                let named = fixed
                    .payload
                    .as_ref()
                    .and_then(named_type)
                    .map(|reference| self.payload(home, reference));
                (
                    v1::Kind::Fixed,
                    None,
                    Some(v1::interaction::Shape::Fixed(v1::FixedShape {
                        payload,
                        named,
                    })),
                )
            }
            _ => (v1::Kind::Unspecified, None, None),
        };
        v1::Interaction {
            name: Some(spellings(&decl.name)),
            kind: kind as i32,
            row,
            doc: decl.doc.clone(),
            labels: decl.labels.clone(),
            deprecated: decl.deprecated.clone(),
            timing: timing.map(timing_of),
            shape,
        }
    }

    fn params(
        &mut self,
        home: &'a v2::Package,
        params: &[v2::Param],
        interface: u32,
        slot: u32,
        visibility: i32,
    ) -> Vec<v1::Param> {
        params
            .iter()
            .map(|param| {
                let at = Path::interaction(
                    interface,
                    slot,
                    visibility,
                    vec!["param".to_string(), param.name.clone()],
                );
                v1::Param {
                    name: Some(spellings(&param.name)),
                    r#type: param.r#type.as_ref().map(|ty| self.type_at(home, ty, &at)),
                }
            })
            .collect()
    }

    fn reply(&mut self, home: &'a v2::Package, ret: &v2::ReturnType, at: &Path) -> v1::Reply {
        v1::Reply {
            kind: ret.kind.as_ref().map(|kind| match kind {
                v2::return_type::Kind::Value(value) => {
                    v1::reply::Kind::Value(self.type_at(home, value, at))
                }
                v2::return_type::Kind::Fallible(fallible) => {
                    v1::reply::Kind::Fallible(v1::Fallible {
                        ok: Some(self.type_ref(home, &fallible.ok)),
                        err: Some(self.type_ref(home, &fallible.err)),
                    })
                }
            }),
        }
    }

    fn clauses(
        &mut self,
        home: &'a v2::Package,
        contracts: &[v2::Contract],
        params: &[v2::Param],
        reply_named: Option<&str>,
    ) -> Vec<v1::Clause> {
        contracts
            .iter()
            .map(|contract| {
                let reply = match v2::ContractKind::try_from(contract.kind) {
                    Ok(v2::ContractKind::Ensure) => reply_named,
                    _ => None,
                };
                v1::Clause {
                    kind: contract.kind,
                    source: contract.source.clone(),
                    signal_refs: contract.signal_refs.clone(),
                    param_refs: contract.param_refs.clone(),
                    uses_result: contract.uses_result,
                    observer_id: contract.observer_id.clone(),
                    translation: Some(clauses::translate(
                        self.scope,
                        home,
                        &contract.source,
                        params,
                        reply,
                    )),
                }
            })
            .collect()
    }

    /// One payload: its resolved reference and the FlatBuffers bound the
    /// codec emits as `MAX_SIZE`, computed over the declaring package with
    /// the rest of the scope beside it.
    fn payload(&mut self, home: &'a v2::Package, reference: &str) -> v1::Payload {
        let max_size = self
            .scope
            .resolve(home, reference)
            .and_then(|(decl, declaring)| {
                let others = self.scope.projection_others(declaring);
                fb::max_size(
                    fb::Packages {
                        package: declaring,
                        others: &others,
                    },
                    decl,
                )
            });
        v1::Payload {
            r#type: Some(self.type_ref(home, reference)),
            flatbuffers_max_size: max_size.map(|size| u32::try_from(size).unwrap_or(u32::MAX)),
        }
    }

    fn services(&mut self) -> Vec<v1::Service> {
        let package = self.scope.package;
        let shapes: Vec<&str> = package.shapes().map(|shape| shape.name).collect();
        package
            .services
            .iter()
            .map(|service| {
                let interface_refs: Vec<String> = service
                    .shapes
                    .iter()
                    .filter_map(|slot| match slot.kind.as_ref()? {
                        v2::service_shape::Kind::InterfaceRef(reference) => Some(reference.clone()),
                        v2::service_shape::Kind::Inline(_) => None,
                    })
                    .collect();
                let inline = service.shapes.iter().any(|slot| {
                    matches!(slot.kind.as_ref(), Some(v2::service_shape::Kind::Inline(_)))
                });
                let inline_shape = inline
                    .then(|| {
                        shapes
                            .iter()
                            .position(|name| *name == service.name)
                            .map(|position| position as u32)
                    })
                    .flatten();
                v1::Service {
                    name: Some(dotted_name(&service.name)),
                    visibility: service.visibility,
                    doc: service.doc.clone(),
                    labels: service.labels.clone(),
                    deprecated: service.deprecated.clone(),
                    interface_refs,
                    inline_shape,
                }
            })
            .collect()
    }

    fn catalog(&mut self) -> v1::Catalog {
        v1::Catalog {
            package: self.scope.package.name.clone(),
            // The placeholder until E16.2 (driftsys/ridl#378).
            hash: vec![0u8; 32],
            retired: self
                .scope
                .package
                .retired
                .iter()
                .map(|entry| v1::RetiredInterface {
                    name: entry.name.clone(),
                    number: entry.number,
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------
// Patching the projection's table indices back into the model
// ---------------------------------------------------------------------

fn patch_flatbuffers(model: &mut v1::Model, projected: &flatbuffers::Projected) {
    for tuple in &mut model.tuples {
        let wire = tuple
            .name
            .as_ref()
            .map(|name| name.wire.clone())
            .unwrap_or_default();
        tuple.flatbuffers_table = projected.tuple_tables.get(&wire).copied();
        for (position, field) in tuple.fields.iter_mut().enumerate() {
            let hint = format!("{wire}Field{}", position + 1);
            if let Some(ty) = field.r#type.as_mut() {
                patch_type(ty, &hint, projected);
            }
        }
    }
    for (index, declaration) in model.declarations.iter_mut().enumerate() {
        let owner = declaration
            .name
            .as_ref()
            .map(|name| name.declared.clone())
            .unwrap_or_default();
        match declaration.kind.as_mut() {
            Some(v1::declaration::Kind::Struct(def)) => {
                for slot in &mut def.slots {
                    if let Some(v1::slot::Occupant::Field(field)) = slot.occupant.as_mut() {
                        let hint = format!(
                            "{owner}{}",
                            joined_camel(
                                field
                                    .name
                                    .as_ref()
                                    .map(|name| name.declared.as_str())
                                    .unwrap_or_default()
                            )
                        );
                        if let Some(ty) = field.r#type.as_mut() {
                            patch_type(ty, &hint, projected);
                        }
                    }
                }
            }
            Some(v1::declaration::Kind::Union(def)) => {
                for (arm_index, arm) in def.arms.iter_mut().enumerate() {
                    arm.flatbuffers_box = projected
                        .arm_boxes
                        .get(&(index as u32, arm_index as u32))
                        .copied();
                }
            }
            _ => {}
        }
    }
}

/// The entry table of every map this type reaches, by the wire induced name
/// of the map's own position — which is the name the walk that built the
/// tables recorded it under.
fn patch_type(ty: &mut v1::Type, hint: &str, projected: &flatbuffers::Projected) {
    match ty.kind.as_mut() {
        Some(v1::r#type::Kind::Array(array)) => {
            if let Some(element) = array.element.as_mut() {
                patch_type(element, &format!("{hint}Element"), projected);
            }
        }
        Some(v1::r#type::Kind::Map(map)) => {
            map.flatbuffers_entry_table = projected.entry_tables.get(hint).copied();
            if let Some(key) = map.key.as_mut() {
                patch_type(key, &format!("{hint}Key"), projected);
            }
            if let Some(value) = map.value.as_mut() {
                patch_type(value, &format!("{hint}Value"), projected);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------
// Small conversions
// ---------------------------------------------------------------------

fn constraint(source: &v2::Constraint) -> v1::Constraint {
    v1::Constraint {
        min: source.min.clone(),
        max: source.max.clone(),
        step: source.step.clone(),
        len_min: source.len_min,
        len_max: source.len_max,
        pattern: source
            .pattern
            .as_deref()
            .map(|pattern| strip_regex_delimiters(pattern).to_string()),
        pattern_const: source.pattern_const.clone(),
    }
}

fn retired(reserved: &v2::Reserved) -> v1::Retired {
    v1::Retired {
        name: reserved.name.as_deref().map(spellings),
        value: reserved.value,
    }
}

fn enum_value(value: &v2::EnumValue) -> v1::EnumValue {
    v1::EnumValue {
        name: Some(spellings(&value.name)),
        value: value.value,
        doc: value.doc.clone(),
    }
}

/// The zero member and the init member (typl §5.8): the value 0 if declared,
/// else the lowest declared value.
fn enum_body(def: &v2::EnumDef) -> v1::Enum {
    let zero = def.values.iter().position(|value| value.value == 0);
    let lowest = def
        .values
        .iter()
        .enumerate()
        .min_by_key(|(_, value)| value.value)
        .map(|(index, _)| index);
    v1::Enum {
        values: def.values.iter().map(enum_value).collect(),
        retired: def.reserved.iter().map(retired).collect(),
        zero_member: zero.map(|index| index as u32),
        init_member: zero.or(lowest).map(|index| index as u32),
    }
}

fn timing_of(timing: &v2::Timing) -> v1::Timing {
    v1::Timing {
        mode: timing.mode,
        min_us: timing.min_us.clone(),
        max_us: timing.max_us.clone(),
        default_applied: timing.default_applied,
    }
}

fn decl_kind(decl: &v2::Decl) -> v1::DeclKind {
    match decl.kind {
        Some(v2::decl::Kind::TypeDef(_)) => v1::DeclKind::Scalar,
        Some(v2::decl::Kind::ConstDef(_)) => v1::DeclKind::Constant,
        Some(v2::decl::Kind::StructDef(_)) => v1::DeclKind::Struct,
        Some(v2::decl::Kind::EnumDef(_)) => v1::DeclKind::Enum,
        Some(v2::decl::Kind::EnumSetDef(_)) => v1::DeclKind::EnumSet,
        Some(v2::decl::Kind::UnionDef(_)) => v1::DeclKind::Union,
        _ => v1::DeclKind::Unspecified,
    }
}

/// The single declared parameter's named type — the one call shape the
/// generated face carries today.
fn single_param_type(params: &[v2::Param]) -> Option<&str> {
    let [param] = params else {
        return None;
    };
    named_type(param.r#type.as_ref()?)
}

fn query_reply_type(query: &v2::QueryDef) -> Option<&str> {
    match query.return_type.as_ref()?.kind.as_ref()? {
        v2::return_type::Kind::Value(value) => named_type(value),
        v2::return_type::Kind::Fallible(_) => None,
    }
}

fn named_type(ty: &v2::FieldType) -> Option<&str> {
    match ty.kind.as_ref()? {
        v2::field_type::Kind::Named(name) => Some(name),
        _ => None,
    }
}

fn primitive_keyword(reference: &str) -> Option<v1::PrimitiveType> {
    match reference {
        "boolean" => Some(v1::PrimitiveType::Boolean),
        "integer" => Some(v1::PrimitiveType::Integer),
        "float" => Some(v1::PrimitiveType::Float),
        "string" => Some(v1::PrimitiveType::String),
        "bytes" => Some(v1::PrimitiveType::Bytes),
        _ => None,
    }
}

/// Strips a regex literal's surrounding `/…/`, leaving the pattern body — the
/// form `strip_regex_delimiters` leaves in two backends today.
fn strip_regex_delimiters(regex: &str) -> &str {
    regex
        .strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
        .unwrap_or(regex)
}
