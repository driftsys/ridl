//! The two transitive typl facts the model states once: the resolved init
//! (typl §5.8) and the closure of a declaration (design note D-5).
//!
//! Both exist in two copies in the tree today — `ridl-backend-rust`'s
//! `defaults.rs` and `derives.rs`, mirrored in `ridl-backend-ts`'s
//! `init_function` family — and both walks cross packages, which a codegen
//! plugin cannot do. They are computed here, once, over the lowering's scope.

use std::collections::HashSet;

use super::resolve::{Scope, is_foreign};
use super::v1;
use crate::v2;

/// The scalar class of a backing, total over every shape the IR can carry —
/// `backing_scalar` in `ridl-backend-rust` and its copy in
/// `ridl-backend-ts`. A unit backing implies float (typl §5.1); an absent
/// backing is float; an unspecified primitive backing is bytes.
pub(crate) fn scalar_class(td: &v2::TypeDef) -> v1::ScalarClass {
    match td
        .backing
        .as_ref()
        .and_then(|backing| backing.kind.as_ref())
    {
        Some(v2::backing::Kind::Unit(_)) => v1::ScalarClass::Float,
        Some(v2::backing::Kind::Primitive(primitive)) => {
            match v2::PrimitiveType::try_from(*primitive).unwrap_or(v2::PrimitiveType::Unspecified)
            {
                v2::PrimitiveType::Boolean => v1::ScalarClass::Boolean,
                v2::PrimitiveType::Integer => v1::ScalarClass::Integer,
                v2::PrimitiveType::Float => v1::ScalarClass::Float,
                v2::PrimitiveType::String => v1::ScalarClass::String,
                v2::PrimitiveType::Bytes | v2::PrimitiveType::Unspecified => v1::ScalarClass::Bytes,
            }
        }
        None => v1::ScalarClass::Float,
    }
}

// ---------------------------------------------------------------------
// The resolved init (typl §5.8)
// ---------------------------------------------------------------------

/// What is known about the position a value fills — the `Slot` of
/// `defaults.rs`, without the generated-name hint, which is a Rust idiom
/// rather than a fact.
#[derive(Clone, Copy, Default)]
pub(crate) struct Position<'a> {
    /// The resolved init text of the enclosing field.
    pub init_value: Option<&'a str>,
    /// The `= value` the source declared on the enclosing field.
    pub declared_init: Option<&'a str>,
    /// The enclosing field's one-level `InitValue.derivable` flag; `None` at
    /// a tuple field and a collection element, which carry no `InitValue`.
    pub flag: Option<bool>,
}

/// The leaf-recursion rule, applied once: whether every position this one
/// transitively contains has an init.
pub(crate) struct Inits<'a> {
    scope: Scope<'a>,
    visiting: HashSet<(String, String)>,
}

impl<'a> Inits<'a> {
    pub fn new(scope: Scope<'a>) -> Self {
        Self {
            scope,
            visiting: HashSet::new(),
        }
    }

    /// One declaration's own init.
    pub fn declaration(&mut self, home: &'a v2::Package, decl: &v2::Decl) -> v1::Init {
        let (derivable, value) = match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => (
                self.type_def(td),
                td.init.as_ref().and_then(|i| i.value.clone()),
            ),
            _ => (self.decl_derivable(home, decl), None),
        };
        let declared = match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => td.declared_init.clone(),
            _ => None,
        };
        let one_level = match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => {
                td.init.as_ref().is_some_and(|value| value.derivable)
            }
            _ => false,
        };
        init_with(derivable, value, declared.is_some(), one_level)
    }

    /// One struct field's init.
    pub fn field(&mut self, home: &'a v2::Package, field: &v2::Field) -> v1::Init {
        let resolved = field.init.as_ref();
        let position = Position {
            init_value: resolved.and_then(|i| i.value.as_deref()),
            declared_init: field.declared_init.as_deref(),
            flag: Some(resolved.is_some_and(|i| i.derivable)),
        };
        let derivable = match field.r#type.as_ref() {
            Some(ty) => self.position(home, ty, position),
            None => false,
        };
        init_with(
            derivable,
            resolved.and_then(|i| i.value.clone()),
            field.declared_init.is_some(),
            resolved.is_some_and(|i| i.derivable),
        )
    }

    /// One tuple field's init. A tuple field carries no `InitValue` of its
    /// own, so the position has no flag (`defaults::tuple_default_expr`).
    pub fn tuple_field(&mut self, home: &'a v2::Package, field: &v2::TupleField) -> v1::Init {
        let derivable = match field.r#type.as_ref() {
            Some(ty) => self.position(home, ty, Position::default()),
            None => false,
        };
        init(derivable, None, false)
    }

    /// A whole tuple: derivable exactly when every field is.
    pub fn tuple(&mut self, home: &'a v2::Package, tuple: &v2::TupleType) -> v1::Init {
        let derivable = self.tuple_derivable(home, tuple);
        init(derivable, None, false)
    }

    /// A signal's resolved channel init (RIDL-109), as the IR carries it.
    pub fn signal(&mut self, value: Option<&v2::InitValue>, declared: bool) -> v1::Init {
        init_with(
            value.is_some_and(|i| i.derivable),
            value.and_then(|i| i.value.clone()),
            declared,
            value.is_some_and(|i| i.derivable),
        )
    }

    fn type_def(&mut self, td: &v2::TypeDef) -> bool {
        let Some(resolved) = td.init.as_ref() else {
            return false;
        };
        resolved.derivable && scalar_value(scalar_class(td), resolved.value.as_deref())
    }

    fn decl_derivable(&mut self, home: &'a v2::Package, decl: &v2::Decl) -> bool {
        let key = (home.name.clone(), decl.name.clone());
        if !self.visiting.insert(key.clone()) {
            // A composite that reaches itself. TYPL-206 rejects one upstream,
            // and this walk does not trust that gate (`defaults`'s C1b guard).
            return false;
        }
        let derivable = match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => self.type_def(td),
            Some(v2::decl::Kind::StructDef(def)) => def.members.iter().all(|member| {
                match &member.member {
                    Some(v2::struct_member::Member::Field(field)) => {
                        self.field_derivable(home, field)
                    }
                    // A tombstone emits no field (typl §7.4).
                    _ => true,
                }
            }),
            // The value 0 if declared, else the lowest declared value
            // (typl §5.8); a member is needed either way.
            Some(v2::decl::Kind::EnumDef(def)) => !def.values.is_empty(),
            // The empty set (typl §5.8, §9).
            Some(v2::decl::Kind::EnumSetDef(_)) => true,
            Some(v2::decl::Kind::UnionDef(def)) => match def.arms.first() {
                Some(arm) => self.reference_derivable(home, &arm.type_ref, Position::default()),
                None => false,
            },
            // A constant emits no value to default, and an interaction rides
            // `Interface.interactions` rather than a package declaration.
            _ => false,
        };
        self.visiting.remove(&key);
        derivable
    }

    fn field_derivable(&mut self, home: &'a v2::Package, field: &v2::Field) -> bool {
        let resolved = field.init.as_ref();
        let position = Position {
            init_value: resolved.and_then(|i| i.value.as_deref()),
            declared_init: field.declared_init.as_deref(),
            flag: Some(resolved.is_some_and(|i| i.derivable)),
        };
        match field.r#type.as_ref() {
            Some(ty) => self.position(home, ty, position),
            None => false,
        }
    }

    fn tuple_derivable(&mut self, home: &'a v2::Package, tuple: &v2::TupleType) -> bool {
        tuple
            .fields
            .iter()
            .all(|field| match field.r#type.as_ref() {
                Some(ty) => self.position(home, ty, Position::default()),
                None => false,
            })
    }

    fn position(&mut self, home: &'a v2::Package, ty: &v2::FieldType, at: Position<'_>) -> bool {
        // An absent optional field defaults to absence.
        if ty.optional {
            return true;
        }
        match ty.kind.as_ref() {
            Some(v2::field_type::Kind::Named(reference)) => {
                self.reference_derivable(home, reference, at)
            }
            Some(v2::field_type::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive)
                    .unwrap_or(v2::PrimitiveType::Unspecified)
                {
                    v2::PrimitiveType::Integer
                    | v2::PrimitiveType::Float
                    | v2::PrimitiveType::Boolean => true,
                    v2::PrimitiveType::String | v2::PrimitiveType::Bytes => at.flag != Some(false),
                    v2::PrimitiveType::Unspecified => false,
                }
            }
            Some(v2::field_type::Kind::InlineScalar(td)) => {
                at.flag != Some(false) && scalar_value(scalar_class(td), at.init_value)
            }
            Some(v2::field_type::Kind::Tuple(tuple)) => self.tuple_derivable(home, tuple),
            Some(v2::field_type::Kind::Array(array)) => {
                let Some(element) = array.element.as_deref() else {
                    return false;
                };
                let inner = Position {
                    flag: at.flag,
                    ..Position::default()
                };
                if array.min == array.max {
                    array.max == 0 || self.position(home, element, inner)
                } else if array.min == 0 {
                    true
                } else {
                    self.position(home, element, inner)
                }
            }
            Some(v2::field_type::Kind::Map(map)) => {
                if map.min == 0 {
                    return true;
                }
                let inner = Position {
                    flag: at.flag,
                    ..Position::default()
                };
                let (Some(key), Some(value)) = (map.key.as_deref(), map.value.as_deref()) else {
                    return false;
                };
                self.position(home, key, inner) && self.position(home, value, inner)
            }
            // A stream is an interaction-position type (ridl §12.3) and has
            // no init in a data position.
            _ => false,
        }
    }

    /// A named reference. A reference the scope does not resolve is the one
    /// case the IR's own one-level `InitValue.derivable` flag is trusted, and
    /// then only when the position carries no declared init — which is
    /// exactly the case `defaults.rs` and the TypeScript backend trust it in.
    fn reference_derivable(
        &mut self,
        home: &'a v2::Package,
        reference: &str,
        at: Position<'_>,
    ) -> bool {
        match self.scope.resolve(home, reference) {
            Some((decl, declaring)) => match &decl.kind {
                Some(v2::decl::Kind::TypeDef(td)) => {
                    if !self.type_def(td) {
                        return false;
                    }
                    match at.declared_init {
                        Some(declared) => scalar_value(scalar_class(td), Some(declared)),
                        None => true,
                    }
                }
                _ => self.decl_derivable(declaring, decl),
            },
            None if is_foreign(reference) => {
                // The remote backing is not resolvable, so a declared init on
                // such a position cannot be faithfully wrapped.
                at.declared_init.is_none() && at.flag == Some(true)
            }
            // A bare name no declaration answers to.
            None => false,
        }
    }
}

/// Whether a scalar class can be constructed from the init text the position
/// carries — `defaults::scalar_default_value` as a predicate.
fn scalar_value(class: v1::ScalarClass, value: Option<&str>) -> bool {
    match class {
        v1::ScalarClass::Float | v1::ScalarClass::Integer => value.is_some(),
        v1::ScalarClass::Boolean | v1::ScalarClass::String => true,
        // A declared byte-string init has no faithful literal form, so a
        // position carrying one has no derivable init at all.
        v1::ScalarClass::Bytes => value.is_none_or(str::is_empty),
        v1::ScalarClass::Unspecified => false,
    }
}

fn init(derivable: bool, value: Option<String>, declared: bool) -> v1::Init {
    init_with(derivable, value, declared, false)
}

/// [`init`] with the IR's own one-level `InitValue.derivable` stated beside
/// the resolved answer. The two differ: `derivable` is the transitive rule of
/// typl §5.8 applied over the scope, `one_level` is the flag the IR carries
/// on the position itself, which a printer reads where the scope resolves
/// nothing.
fn init_with(derivable: bool, value: Option<String>, declared: bool, one_level: bool) -> v1::Init {
    let source = if declared {
        v1::InitSource::Declared
    } else if derivable {
        v1::InitSource::Derived
    } else {
        v1::InitSource::None
    };
    v1::Init {
        derivable,
        value,
        source: source as i32,
        one_level,
    }
}

// ---------------------------------------------------------------------
// The closure (design note D-5)
// ---------------------------------------------------------------------

/// The transitive walk `derives.rs` makes, stated as facts rather than as one
/// target's trait list.
pub(crate) struct Closures<'a> {
    scope: Scope<'a>,
    visiting: HashSet<(String, String)>,
}

impl<'a> Closures<'a> {
    pub fn new(scope: Scope<'a>) -> Self {
        Self {
            scope,
            visiting: HashSet::new(),
        }
    }

    pub fn declaration(&mut self, home: &'a v2::Package, decl: &v2::Decl) -> v1::Closure {
        let mut found = v1::Closure::default();
        self.walk_decl(home, decl, &mut found);
        found
    }

    pub fn tuple(&mut self, home: &'a v2::Package, tuple: &v2::TupleType) -> v1::Closure {
        let mut found = v1::Closure::default();
        self.walk_tuple(home, tuple, &mut found);
        found
    }

    fn walk_decl(&mut self, home: &'a v2::Package, decl: &v2::Decl, found: &mut v1::Closure) {
        let key = (home.name.clone(), decl.name.clone());
        if !self.visiting.insert(key.clone()) {
            found.reaches_cycle = true;
            return;
        }
        match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => class_fact(scalar_class(td), found),
            // A fieldless enum and an enum set are both an integer at the
            // leaf; neither constrains anything.
            Some(v2::decl::Kind::EnumDef(_)) | Some(v2::decl::Kind::EnumSetDef(_)) => {}
            Some(v2::decl::Kind::StructDef(def)) => {
                for member in &def.members {
                    if let Some(v2::struct_member::Member::Field(field)) = &member.member {
                        match field.r#type.as_ref() {
                            Some(ty) => self.walk_type(home, ty, found),
                            None => found.reaches_stream_or_unspecified = true,
                        }
                    }
                }
            }
            Some(v2::decl::Kind::UnionDef(def)) => {
                for arm in &def.arms {
                    self.walk_reference(home, &arm.type_ref, found);
                }
            }
            // A constant carries no closure, and an interaction is not a
            // package declaration. Kept total.
            _ => found.reaches_unresolved = true,
        }
        self.visiting.remove(&key);
    }

    fn walk_tuple(
        &mut self,
        home: &'a v2::Package,
        tuple: &v2::TupleType,
        found: &mut v1::Closure,
    ) {
        for field in &tuple.fields {
            match field.r#type.as_ref() {
                Some(ty) => self.walk_type(home, ty, found),
                None => found.reaches_stream_or_unspecified = true,
            }
        }
    }

    fn walk_type(&mut self, home: &'a v2::Package, ty: &v2::FieldType, found: &mut v1::Closure) {
        match ty.kind.as_ref() {
            Some(v2::field_type::Kind::Named(reference)) => {
                self.walk_reference(home, reference, found);
            }
            Some(v2::field_type::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive)
                    .unwrap_or(v2::PrimitiveType::Unspecified)
                {
                    v2::PrimitiveType::Float => found.reaches_float = true,
                    v2::PrimitiveType::Integer | v2::PrimitiveType::Boolean => {}
                    v2::PrimitiveType::String | v2::PrimitiveType::Bytes => {
                        found.reaches_text_or_bytes = true;
                    }
                    v2::PrimitiveType::Unspecified => found.reaches_stream_or_unspecified = true,
                }
            }
            Some(v2::field_type::Kind::InlineScalar(td)) => class_fact(scalar_class(td), found),
            Some(v2::field_type::Kind::Tuple(tuple)) => self.walk_tuple(home, tuple, found),
            Some(v2::field_type::Kind::Array(array)) => {
                found.reaches_collection = true;
                match array.element.as_deref() {
                    Some(element) => self.walk_type(home, element, found),
                    None => found.reaches_stream_or_unspecified = true,
                }
            }
            Some(v2::field_type::Kind::Map(map)) => {
                found.reaches_collection = true;
                match map.key.as_deref() {
                    Some(key) => self.walk_type(home, key, found),
                    None => found.reaches_stream_or_unspecified = true,
                }
                match map.value.as_deref() {
                    Some(value) => self.walk_type(home, value, found),
                    None => found.reaches_stream_or_unspecified = true,
                }
            }
            // A stream, and a position carrying no type at all.
            _ => found.reaches_stream_or_unspecified = true,
        }
    }

    fn walk_reference(&mut self, home: &'a v2::Package, reference: &str, found: &mut v1::Closure) {
        if is_foreign(reference) {
            found.reaches_foreign = true;
        }
        match self.scope.resolve(home, reference) {
            // A reference that names a constant is not a type position a
            // printer can carry.
            Some((decl, _)) if matches!(decl.kind, Some(v2::decl::Kind::ConstDef(_))) => {
                found.reaches_unresolved = true;
            }
            Some((decl, declaring)) => self.walk_decl(declaring, decl, found),
            None => found.reaches_unresolved = true,
        }
    }
}

fn class_fact(class: v1::ScalarClass, found: &mut v1::Closure) {
    match class {
        v1::ScalarClass::Float => found.reaches_float = true,
        v1::ScalarClass::String | v1::ScalarClass::Bytes => found.reaches_text_or_bytes = true,
        v1::ScalarClass::Integer | v1::ScalarClass::Boolean => {}
        v1::ScalarClass::Unspecified => found.reaches_stream_or_unspecified = true,
    }
}
