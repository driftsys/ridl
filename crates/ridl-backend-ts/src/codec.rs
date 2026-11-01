//! The codec module `<base>.codec.ts`, written from the codegen model only.
//!
//! The module wraps the object API that `flatc --ts --gen-object-api` writes
//! for the package's `.fbs` in plain-object functions: one
//! `encode<Name>(value)` and one `decode<Name>(bytes)` per payload type, which
//! is every FlatBuffers root of the package with a finite bound, except an
//! internal one, whose type `<base>.ts` does not export. A private converter
//! pair `pack<Name>` and `unpack<Name>` is written for each struct, union and
//! induced tuple a payload type reaches. An induced tuple of an interaction
//! is exported as an `interface`, or as the type alias
//! `Record<string, never>` when it has no field. The module also exports the catalog's name
//! and hash, the qualified names of the payload types, the text of the rule
//! codes of the wasm module, and one member table per declared interface,
//! whose entry carries the member's payload limit when the model records one.
//!
//! A named scalar or an enum set of another package converts by a type
//! assertion, and a payload type of another package that is one goes through
//! that package's own codec module. A struct, a union or an enum of another
//! package needs a converter of its own, which the module does not write yet:
//! a payload type that reaches one is refused with a [`GenerateError`] that
//! names both.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use ridl_ir::codegen::v1;

use crate::{GenerateError, GeneratedTs};

/// The text of the rule codes the wasm module reports, rule code `n` at
/// index `n - 2`: the order of `WASM_RULES` in the generated crate.
const RULES: [&str; 6] = ["length", "pattern", "range", "step", "unique", "variant"];

/// The largest integer a TypeScript number holds exactly.
const MAX_SAFE: u64 = 9_007_199_254_740_991;

/// The length of `Catalog.hash`.
const HASH_LEN: usize = 32;

/// Generates `<artifact_base>.codec.ts` for the package `model` describes,
/// or `None` for a package with no payload root that gets a codec, local or
/// foreign, which has nothing the module could carry. The returned source
/// starts at the imports; the caller writes the generated marker and the
/// header above it.
pub fn generate_codec(
    model: &v1::Model,
    artifact_base: &str,
) -> Result<Option<GeneratedTs>, GenerateError> {
    let mut codec = Codec::new(model)?;
    if !codec.has_codec_payload() {
        return Ok(None);
    }
    codec.emit(artifact_base).map(Some)
}

/// The packages whose codec modules the module of `model` imports: those of
/// a payload type of another package that is a named scalar or an enum set.
/// A foreign payload type is a payload root that gets a codec, so the set is
/// non-empty only when [`generate_codec`] writes the module
/// (`has_codec_payload`). `ridlc` reads it to write `ridl.std.codec.ts` only
/// when a module imports it.
pub fn codec_imports(model: &v1::Model) -> Result<BTreeSet<String>, GenerateError> {
    Ok(Codec::new(model)?
        .foreign_payloads
        .values()
        .map(|foreign| foreign.package.clone())
        .collect())
}

fn refuse<T>(message: String) -> Result<T, GenerateError> {
    Err(GenerateError::Unrepresentable(message))
}

/// One payload type: a FlatBuffers root of the package with a finite bound.
struct PayloadType<'a> {
    /// The declared name, or the induced tuple's `InducedName.rust`.
    name: &'a str,
    declaration: Option<&'a v1::Declaration>,
}

/// A declaration a type reference resolves to.
enum Target<'a> {
    /// A declaration of this package, with its index.
    Local(usize, &'a v1::Declaration),
    /// A declaration of the package named, from `Model.foreign`.
    Foreign(&'a str, &'a v1::Declaration),
}

/// The pack and unpack expressions of one type position, with `{}` standing
/// for the value read.
struct Rule {
    pack: String,
    unpack: String,
    /// The conversion is a type assertion or nothing: an array of such
    /// elements is copied, not mapped.
    plain: bool,
    /// The `flatc` object API holds the value as `T | null`: a nested table.
    nullable: bool,
    /// The `flatc` object API holds the value as a `T[]` that is never
    /// `null`: a vector, which is an array, a map or bytes.
    vector: bool,
}

/// One field of a struct or an induced tuple, with its conversion.
struct RecordField {
    /// The plain-object property.
    declared: String,
    /// The `flatc` member.
    accessor: String,
    rule: Rule,
    optional: bool,
    /// The slot is `= null` in the `.fbs` while the field is required: an
    /// enum with no zero member.
    null_default: bool,
}

/// A payload type of another package, encoded by that package's codec module.
struct ForeignPayload {
    package: String,
    name: String,
}

struct Codec<'a> {
    model: &'a v1::Model,
    package: &'a str,
    /// The declarations that are payload types, by index, with the bound
    /// of each root that has one.
    declaration_bounded: HashMap<usize, bool>,
    /// The induced tuples that are roots, by index, with whether each has a
    /// finite bound.
    tuple_bounded: HashMap<usize, bool>,
    /// The structs and unions a payload type reaches, by index.
    reached: HashSet<usize>,
    /// The induced tuples a payload type reaches or is, by index.
    reached_tuples: HashSet<usize>,
    /// The payload types of other packages the interfaces name, by the
    /// `codec<Name>` constant that carries each.
    foreign_payloads: BTreeMap<String, ForeignPayload>,
    /// The packages whose types the module names, for the type imports.
    foreign_types: BTreeSet<String>,
    /// The local payload types an interface entry names, by declared name:
    /// the only ones whose `codec<Name>` constant the module declares.
    used_locals: BTreeSet<String>,
    /// Whether a converter reads a required field the buffer may leave out.
    needs_absent: bool,
    /// The open enums and open enum sets of this package that a payload type
    /// reaches, by index: each gets a converter pair of its own.
    reached_open: BTreeSet<usize>,
}

impl<'a> Codec<'a> {
    fn new(model: &'a v1::Model) -> Result<Self, GenerateError> {
        let hash_len = model
            .catalog
            .as_ref()
            .map_or(0, |catalog| catalog.hash.len());
        if hash_len != HASH_LEN {
            return refuse(format!(
                "malformed codegen model: `Catalog.hash` is {hash_len} bytes, not {HASH_LEN}"
            ));
        }
        if let Some(collision) = model.tuple_collisions.first() {
            return refuse(format!(
                "the induced tuple `{}` has two shapes, ({}) and ({}); the codec module cannot \
                 name both",
                collision.name,
                collision.first_fields.join(", "),
                collision.second_fields.join(", "),
            ));
        }
        let declared: HashSet<&str> = model
            .declarations
            .iter()
            .filter_map(|declaration| declaration.name.as_ref())
            .map(|name| name.declared.as_str())
            .collect();
        for tuple in &model.tuples {
            let Some(v1::induced_tuple::Origin::Interaction(path)) = &tuple.origin else {
                continue;
            };
            let name = tuple_name(tuple);
            if declared.contains(name) {
                return refuse(format!(
                    "`{name}` is both a declaration of the package and the induced tuple of \
                     `{}`",
                    interaction_path(model, path)
                ));
            }
        }
        let mut codec = Codec {
            model,
            package: model
                .scope
                .as_ref()
                .map_or("", |scope| scope.package.as_str()),
            declaration_bounded: HashMap::new(),
            tuple_bounded: HashMap::new(),
            reached: HashSet::new(),
            reached_tuples: HashSet::new(),
            foreign_payloads: BTreeMap::new(),
            foreign_types: BTreeSet::new(),
            used_locals: BTreeSet::new(),
            needs_absent: false,
            reached_open: BTreeSet::new(),
        };
        let roots = model
            .flatbuffers
            .as_ref()
            .map_or(&[][..], |projection| projection.roots.as_slice());
        for root in roots {
            let bounded = matches!(root.bound, Some(v1::fb_root::Bound::MaxSize(_)));
            match root.tuple {
                Some(tuple) => {
                    codec.tuple_bounded.insert(tuple as usize, bounded);
                }
                None => {
                    codec
                        .declaration_bounded
                        .insert(root.declaration as usize, bounded);
                }
            }
        }
        for root in roots {
            if !matches!(root.bound, Some(v1::fb_root::Bound::MaxSize(_))) {
                continue;
            }
            match root.tuple {
                Some(tuple) => {
                    let index = tuple as usize;
                    let Some(induced) = model.tuples.get(index) else {
                        return refuse(format!(
                            "malformed codegen model: a root names tuple {index}, which does not \
                             exist"
                        ));
                    };
                    let qualified = codec.qualified(tuple_name(induced));
                    codec.reach_tuple(index, &qualified)?;
                }
                None => {
                    let index = root.declaration as usize;
                    let Some(declaration) = model.declarations.get(index) else {
                        return refuse(format!(
                            "malformed codegen model: a root names declaration {index}, which \
                             does not exist"
                        ));
                    };
                    // An internal root has no codec: `<base>.ts` does not
                    // export its type, so no signature can name it.
                    if declaration.visibility() == v1::Visibility::Internal {
                        continue;
                    }
                    let qualified = codec.qualified(declared_name(declaration));
                    codec.reach_declaration(index, declaration, &qualified)?;
                }
            }
        }
        for interface in &model.interfaces {
            let Some(v1::interface::Identity::Declared(spelled)) = &interface.identity else {
                continue;
            };
            for slot in &interface.slots {
                let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = &slot.occupant
                else {
                    continue;
                };
                // A member that carries a stream has no entry: its request
                // holds only the non-stream parameters, so it is not a plain
                // call. The Rust backend leaves it out of the face the same.
                if !interaction.streams.is_empty() {
                    continue;
                }
                let member = interaction
                    .name
                    .as_ref()
                    .map_or("", |name| name.declared.as_str());
                codec.roles(interaction, &spelled.declared, member)?;
            }
        }
        codec.check_names()?;
        Ok(codec)
    }

    /// Whether the module has anything to write: a payload root that gets a
    /// codec, local or foreign (an interface table over foreign scalars is
    /// one). A package without one, `ridl.std` included, gets no file.
    fn has_codec_payload(&self) -> bool {
        self.has_local_payload() || !self.foreign_payloads.is_empty()
    }

    /// Whether the package has a payload type of its own: the one case the
    /// module needs `flatc`'s barrel, `<base>.ts` and the builder.
    fn has_local_payload(&self) -> bool {
        !self.payload_types().is_empty()
    }

    /// Refuses two declarations of the module that spell the same name: a
    /// reserved name of the file (an import alias or an export of the
    /// module) taken by an interface or an induced tuple, and two
    /// `codec<Name>` constants that spell the same. Each name is registered
    /// with the declaration it stands for, and the second one is refused.
    fn check_names(&self) -> Result<(), GenerateError> {
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        let mut register = |identifier: String, label: String| -> Result<(), GenerateError> {
            match names.get(&identifier) {
                Some(other) => refuse(format!(
                    "`{label}` in package `{}` collides with `{other}`, which the codec module \
                     also declares",
                    self.package
                )),
                None => {
                    names.insert(identifier, label);
                    Ok(())
                }
            }
        };
        for reserved in [
            "flatbuffers",
            "PayloadCodec",
            "t",
            "fb",
            "CATALOG_NAME",
            "CATALOG_HASH",
            "PAYLOAD_TYPES",
            "RULES",
        ] {
            register(reserved.to_string(), reserved.to_string())?;
        }
        if !self.reached_open.is_empty() {
            register("RuleError".to_string(), "RuleError".to_string())?;
        }
        for package in &self.foreign_types {
            let alias = package_alias(package);
            register(alias.clone(), alias)?;
        }
        let foreign_codecs: BTreeSet<&str> = self
            .foreign_payloads
            .values()
            .map(|foreign| foreign.package.as_str())
            .collect();
        for package in foreign_codecs {
            let alias = format!("{}_codec", package_alias(package));
            register(alias.clone(), alias)?;
        }
        for payload in self.payload_types() {
            if payload
                .declaration
                .is_some_and(|declaration| declaration.visibility() == v1::Visibility::Internal)
            {
                continue;
            }
            let name = payload.name;
            if payload.declaration.is_none() {
                register(name.to_string(), name.to_string())?;
            }
            for prefix in ["encode", "decode"] {
                register(format!("{prefix}{name}"), name.to_string())?;
            }
            // The constant is declared only when an entry names it.
            if self.used_locals.contains(name) {
                register(format!("codec{name}"), name.to_string())?;
            }
        }
        // In declaration order, so that the refusal names the same pair on
        // every run.
        let mut reached: Vec<&usize> = self.reached.iter().collect();
        reached.sort();
        for index in reached {
            let name = declared_name(&self.model.declarations[*index]);
            for prefix in ["pack", "unpack"] {
                register(format!("{prefix}{name}"), name.to_string())?;
            }
        }
        for index in &self.reached_open {
            let name = declared_name(&self.model.declarations[*index]);
            for prefix in ["pack", "unpack"] {
                register(format!("{prefix}{name}"), name.to_string())?;
            }
        }
        let mut reached_tuples: Vec<&usize> = self.reached_tuples.iter().collect();
        reached_tuples.sort();
        for index in reached_tuples {
            let name = tuple_name(&self.model.tuples[*index]);
            for prefix in ["pack", "unpack"] {
                register(format!("{prefix}{name}"), name.to_string())?;
            }
        }
        for (constant, foreign) in &self.foreign_payloads {
            register(
                constant.clone(),
                format!("{}.{}", foreign.package, foreign.name),
            )?;
        }
        for interface in &self.model.interfaces {
            if let Some(v1::interface::Identity::Declared(spelled)) = &interface.identity {
                register(spelled.declared.clone(), spelled.declared.clone())?;
            }
        }
        Ok(())
    }

    /// Whether `ty` names an open enum of this package.
    fn is_open_enum(&self, ty: &v1::Type) -> bool {
        let Some(v1::r#type::Kind::Named(reference)) = &ty.kind else {
            return false;
        };
        matches!(
            self.resolve(reference, "").ok(),
            Some(Target::Local(_, declaration))
                if matches!(&declaration.kind, Some(v1::declaration::Kind::Enum(shape)) if shape.open)
        )
    }

    /// The payload types, in the order of the roots.
    fn payload_types(&self) -> Vec<PayloadType<'a>> {
        let roots = self
            .model
            .flatbuffers
            .as_ref()
            .map_or(&[][..], |projection| projection.roots.as_slice());
        roots
            .iter()
            .filter(|root| matches!(root.bound, Some(v1::fb_root::Bound::MaxSize(_))))
            .filter_map(|root| match root.tuple {
                Some(tuple) => self
                    .model
                    .tuples
                    .get(tuple as usize)
                    .map(|tuple| PayloadType {
                        name: tuple_name(tuple),
                        declaration: None,
                    }),
                None => self
                    .model
                    .declarations
                    .get(root.declaration as usize)
                    .map(|declaration| PayloadType {
                        name: declared_name(declaration),
                        declaration: Some(declaration),
                    }),
            })
            .collect()
    }

    fn qualified(&self, name: &str) -> String {
        format!("{}.{name}", self.package)
    }

    /// The declaration `reference` names, local or foreign. `qualified`
    /// names the payload type for the refusal.
    fn resolve(
        &self,
        reference: &v1::TypeRef,
        qualified: &str,
    ) -> Result<Target<'a>, GenerateError> {
        if !reference.resolved {
            return refuse(format!(
                "`{qualified}` reaches `{}`, which no package in scope declares",
                reference.reference
            ));
        }
        let index = reference.index as usize;
        if reference.foreign {
            return match self.model.foreign.get(index) {
                Some(foreign) => match &foreign.declaration {
                    Some(declaration) => Ok(Target::Foreign(&foreign.package, declaration)),
                    None => refuse(format!(
                        "malformed codegen model: foreign declaration {index} has no declaration"
                    )),
                },
                None => refuse(format!(
                    "malformed codegen model: `{}` names foreign declaration {index}, which does \
                     not exist",
                    reference.reference
                )),
            };
        }
        match self.model.declarations.get(index) {
            Some(declaration) => Ok(Target::Local(index, declaration)),
            None => refuse(format!(
                "malformed codegen model: `{}` names declaration {index}, which does not exist",
                reference.reference
            )),
        }
    }

    /// Records the converters the local `declaration` needs, and refuses a
    /// kind the module does not convert. `qualified` names the payload type.
    fn reach_declaration(
        &mut self,
        index: usize,
        declaration: &'a v1::Declaration,
        qualified: &str,
    ) -> Result<(), GenerateError> {
        use v1::declaration::Kind;
        match &declaration.kind {
            Some(Kind::Struct(shape)) => {
                if self.reached.insert(index) {
                    for field in struct_fields(shape) {
                        if let Some(ty) = field.r#type.as_ref() {
                            self.reach_type(ty, qualified)?;
                        }
                    }
                }
                Ok(())
            }
            Some(Kind::Union(union)) => {
                if self.reached.insert(index) {
                    for arm in &union.arms {
                        if let Some(reference) = arm.r#type.as_ref() {
                            self.reach_reference(reference, qualified)?;
                        }
                    }
                }
                Ok(())
            }
            Some(Kind::Enum(shape)) => {
                if shape.open {
                    self.reached_open.insert(index);
                }
                Ok(())
            }
            Some(Kind::EnumSet(shape)) => {
                if shape.open {
                    self.reached_open.insert(index);
                }
                Ok(())
            }
            Some(Kind::Scalar(_) | Kind::Constant(_)) | None => Ok(()),
        }
    }

    fn reach_reference(
        &mut self,
        reference: &v1::TypeRef,
        qualified: &str,
    ) -> Result<(), GenerateError> {
        match self.resolve(reference, qualified)? {
            Target::Local(index, declaration) => {
                self.reach_declaration(index, declaration, qualified)
            }
            Target::Foreign(package, declaration) => match foreign_kind(declaration) {
                Some(word) => refuse(foreign_refusal(
                    &format!("`{qualified}` reaches"),
                    package,
                    declared_name(declaration),
                    word,
                )),
                None => {
                    // The converter names the type through the package's
                    // alias, which the name check needs before emit.
                    self.foreign_types.insert(package.to_string());
                    Ok(())
                }
            },
        }
    }

    fn reach_type(&mut self, ty: &v1::Type, qualified: &str) -> Result<(), GenerateError> {
        use v1::r#type::Kind;
        match &ty.kind {
            Some(Kind::Named(reference)) => self.reach_reference(reference, qualified),
            Some(Kind::Primitive(_) | Kind::Inline(_)) => Ok(()),
            Some(Kind::Tuple(tuple)) => self.reach_tuple(tuple.index as usize, qualified),
            Some(Kind::Array(array)) => match array.element.as_deref() {
                Some(element) => {
                    refuse_optional(element, qualified, "an array element")?;
                    self.reach_type(element, qualified)
                }
                None => refuse(format!(
                    "malformed codegen model: an array of `{qualified}` has no element type"
                )),
            },
            Some(Kind::Map(map)) => match (map.key.as_deref(), map.value.as_deref()) {
                (Some(key), Some(value)) => {
                    refuse_optional(key, qualified, "a map key")?;
                    refuse_optional(value, qualified, "a map value")?;
                    self.reach_type(key, qualified)?;
                    self.reach_type(value, qualified)
                }
                _ => refuse(format!(
                    "malformed codegen model: a map of `{qualified}` has no key or value type"
                )),
            },
            Some(Kind::Stream(_)) | None => refuse(not_converted(qualified, "a stream")),
        }
    }

    fn reach_tuple(&mut self, index: usize, qualified: &str) -> Result<(), GenerateError> {
        if !self.reached_tuples.insert(index) {
            return Ok(());
        }
        let Some(tuple) = self.model.tuples.get(index) else {
            return refuse(format!(
                "malformed codegen model: `{qualified}` names tuple {index}, which does not exist"
            ));
        };
        for field in &tuple.fields {
            if let Some(ty) = field.r#type.as_ref() {
                self.reach_type(ty, qualified)?;
            }
        }
        Ok(())
    }

    /// The `flatc` table of the induced tuple at `index`.
    fn tuple_table(&self, index: usize) -> Result<&'a v1::FbTable, GenerateError> {
        let tuple = &self.model.tuples[index];
        let table = tuple
            .flatbuffers_table
            .and_then(|table| self.tables().get(table as usize));
        match table {
            Some(table) => Ok(table),
            None => refuse(format!(
                "malformed codegen model: the induced tuple `{}` has no FlatBuffers table",
                tuple_name(tuple)
            )),
        }
    }

    fn tables(&self) -> &'a [v1::FbTable] {
        self.model
            .flatbuffers
            .as_ref()
            .map_or(&[][..], |projection| projection.tables.as_slice())
    }

    /// The table of the struct at `index`, when the projection has one.
    fn struct_table(&self, index: usize) -> Option<&'a v1::FbTable> {
        self.tables().iter().find(|table| {
            matches!(table.source, Some(v1::fb_table::Source::StructDeclaration(i)) if i as usize == index)
        })
    }

    /// The TypeScript type of a type position, with the `t.` prefix on a
    /// named type of this package and the package alias on a foreign one.
    fn ts_type(&mut self, ty: &v1::Type, qualified: &str) -> Result<String, GenerateError> {
        use v1::r#type::Kind;
        match &ty.kind {
            Some(Kind::Named(reference)) => match self.resolve(reference, qualified)? {
                Target::Local(_, declaration) => Ok(value_type(declaration)),
                Target::Foreign(package, declaration) => {
                    self.foreign_types.insert(package.to_string());
                    Ok(format!(
                        "{}.{}",
                        package_alias(package),
                        declared_name(declaration)
                    ))
                }
            },
            Some(Kind::Primitive(primitive)) => Ok(primitive_ts(*primitive).to_string()),
            Some(Kind::Inline(scalar)) => Ok(scalar_ts(scalar).to_string()),
            Some(Kind::Tuple(tuple)) => {
                let Some(induced) = self.model.tuples.get(tuple.index as usize) else {
                    return refuse(format!(
                        "malformed codegen model: `{qualified}` names tuple {}, which does not \
                         exist",
                        tuple.index
                    ));
                };
                let mut fields = Vec::with_capacity(induced.fields.len());
                for field in &induced.fields {
                    let (optional, spelled) = match field.r#type.as_ref() {
                        Some(ty) => (ty.optional, self.ts_type(ty, qualified)?),
                        None => (false, "unknown".to_string()),
                    };
                    let marker = if optional { "?" } else { "" };
                    fields.push(format!(
                        "{}{marker}: {spelled}",
                        field
                            .name
                            .as_ref()
                            .map_or("", |name| name.declared.as_str())
                    ));
                }
                if fields.is_empty() {
                    Ok("{}".to_string())
                } else {
                    Ok(format!("{{ {} }}", fields.join("; ")))
                }
            }
            // An element, a key or a value is never optional here: the reach
            // pass refused one before any type is spelled.
            Some(Kind::Array(array)) => {
                let element = match array.element.as_deref() {
                    Some(element) => self.ts_type(element, qualified)?,
                    None => "unknown".to_string(),
                };
                Ok(format!("readonly {}[]", parenthesized(element)))
            }
            Some(Kind::Map(map)) => {
                let key = match map.key.as_deref() {
                    Some(key) => self.ts_type(key, qualified)?,
                    None => "unknown".to_string(),
                };
                let value = match map.value.as_deref() {
                    Some(value) => self.ts_type(value, qualified)?,
                    None => "unknown".to_string(),
                };
                Ok(format!("ReadonlyArray<readonly [{key}, {value}]>"))
            }
            Some(Kind::Stream(_)) | None => refuse(not_converted(qualified, "a stream")),
        }
    }

    /// The conversion of one type position.
    fn rule(&mut self, ty: &v1::Type, qualified: &str) -> Result<Rule, GenerateError> {
        use v1::declaration::Kind;
        use v1::r#type::Kind as TypeKind;
        match &ty.kind {
            Some(TypeKind::Named(reference)) => {
                let (spelled, declaration, nested) = match self.resolve(reference, qualified)? {
                    Target::Local(_, declaration) => (
                        format!("t.{}", declared_name(declaration)),
                        declaration,
                        true,
                    ),
                    Target::Foreign(package, declaration) => {
                        // `reach_reference` refused a foreign struct, union
                        // or enum before any converter is written.
                        if foreign_kind(declaration).is_some() {
                            return refuse(format!(
                                "malformed codegen model: `{qualified}` reaches `{package}.{}`, \
                                 which the reach pass did not refuse",
                                declared_name(declaration)
                            ));
                        }
                        self.foreign_types.insert(package.to_string());
                        (
                            format!("{}.{}", package_alias(package), declared_name(declaration)),
                            declaration,
                            false,
                        )
                    }
                };
                let name = declared_name(declaration);
                Ok(match &declaration.kind {
                    Some(Kind::Enum(shape)) if shape.open && nested => Rule {
                        pack: format!("pack{name}({{}})"),
                        unpack: format!("unpack{name}({{}})"),
                        plain: false,
                        nullable: false,
                        vector: false,
                    },
                    Some(Kind::EnumSet(shape)) if shape.open && nested => Rule {
                        pack: format!("pack{name}({{}})"),
                        unpack: format!("{{}} as {spelled}"),
                        plain: false,
                        nullable: false,
                        vector: false,
                    },
                    Some(Kind::Enum(_)) => Rule {
                        pack: "BigInt({})".to_string(),
                        unpack: format!("Number({{}}) as {spelled}"),
                        plain: false,
                        nullable: false,
                        vector: false,
                    },
                    Some(Kind::Struct(_) | Kind::Union(_)) if nested => Rule {
                        pack: format!("pack{name}({{}})"),
                        unpack: format!("unpack{name}({{}})"),
                        plain: false,
                        nullable: true,
                        vector: false,
                    },
                    Some(Kind::Scalar(scalar)) if scalar.class() == v1::ScalarClass::Bytes => {
                        bytes_rule(&spelled)
                    }
                    Some(Kind::Scalar(_) | Kind::EnumSet(_)) => Rule {
                        pack: "{}".to_string(),
                        unpack: format!("{{}} as {spelled}"),
                        plain: true,
                        nullable: false,
                        vector: false,
                    },
                    _ => {
                        return refuse(format!(
                            "malformed codegen model: `{qualified}` reaches `{}`, which is not a \
                             type",
                            reference.reference
                        ));
                    }
                })
            }
            Some(TypeKind::Primitive(primitive)) => {
                Ok(match v1::PrimitiveType::try_from(*primitive) {
                    Ok(v1::PrimitiveType::Bytes) => bytes_rule("Uint8Array"),
                    Ok(v1::PrimitiveType::String) => string_rule(),
                    _ => copy_rule(),
                })
            }
            Some(TypeKind::Inline(scalar)) => Ok(match scalar.class() {
                v1::ScalarClass::Bytes => bytes_rule("Uint8Array"),
                v1::ScalarClass::String => string_rule(),
                _ => copy_rule(),
            }),
            Some(TypeKind::Tuple(tuple)) => {
                let Some(induced) = self.model.tuples.get(tuple.index as usize) else {
                    return refuse(format!(
                        "malformed codegen model: `{qualified}` names tuple {}, which does not \
                         exist",
                        tuple.index
                    ));
                };
                let name = tuple_name(induced);
                Ok(Rule {
                    pack: format!("pack{name}({{}})"),
                    unpack: format!("unpack{name}({{}})"),
                    plain: false,
                    nullable: true,
                    vector: false,
                })
            }
            Some(TypeKind::Array(array)) => {
                let Some(element) = array.element.as_deref() else {
                    return refuse(format!(
                        "malformed codegen model: an array of `{qualified}` has no element type"
                    ));
                };
                let rule = self.rule(element, qualified)?;
                // An element that converts by assertion or not at all is
                // copied. TypeScript refuses an assertion of a whole array
                // to a branded element type, so an unpack that asserts maps
                // each element.
                let spread = |template: &str| {
                    if template == "{}" {
                        "[...{}]".to_string()
                    } else {
                        format!(
                            "{{}}.map((element) => {})",
                            template.replace("{}", "element")
                        )
                    }
                };
                Ok(Rule {
                    pack: if rule.plain {
                        "[...{}]".to_string()
                    } else {
                        spread(&rule.pack)
                    },
                    unpack: spread(&rule.unpack),
                    plain: false,
                    nullable: false,
                    vector: true,
                })
            }
            Some(TypeKind::Map(map)) => {
                let (Some(key), Some(value)) = (map.key.as_deref(), map.value.as_deref()) else {
                    return refuse(format!(
                        "malformed codegen model: a map of `{qualified}` has no key or value type"
                    ));
                };
                let entry = map
                    .flatbuffers_entry_table
                    .and_then(|table| self.tables().get(table as usize));
                let Some(entry) = entry else {
                    return refuse(format!(
                        "malformed codegen model: a map of `{qualified}` has no entry table"
                    ));
                };
                let key_rule = self.rule(key, qualified)?;
                let value_rule = self.rule(value, qualified)?;
                let unpack_key = self.guarded("entry.key", &key_rule, qualified, "key");
                let unpack_value = self.guarded("entry.value", &value_rule, qualified, "value");
                Ok(Rule {
                    pack: format!(
                        "{{}}.map(([key, value]) => {{ const entry = new fb.{}T(); entry.key = {}; \
                         entry.value = {}; return entry; }})",
                        entry.name,
                        key_rule.pack.replace("{}", "key"),
                        value_rule.pack.replace("{}", "value"),
                    ),
                    unpack: format!("{{}}.map((entry) => [{unpack_key}, {unpack_value}] as const)"),
                    plain: false,
                    nullable: false,
                    vector: true,
                })
            }
            Some(TypeKind::Stream(_)) | None => refuse(not_converted(qualified, "a stream")),
        }
    }

    /// The unpack of `rule` over the member `read`, guarded by `absent`
    /// when the `flatc` member is nullable and the position is required.
    fn guarded(&mut self, read: &str, rule: &Rule, qualified: &str, field: &str) -> String {
        let unpack = rule.unpack.replace("{}", read);
        if !rule.nullable {
            return unpack;
        }
        self.needs_absent = true;
        format!("{read} === null ? absent(\"{qualified}\", \"{field}\") : {unpack}")
    }

    fn emit(&mut self, artifact_base: &str) -> Result<GeneratedTs, GenerateError> {
        let mut body = String::new();
        let catalog = self.model.catalog.as_ref();
        let name = catalog.map_or("", |catalog| catalog.package.as_str());
        let hash: Vec<String> = catalog
            .map(|catalog| catalog.hash.iter().map(u8::to_string).collect())
            .unwrap_or_default();
        body.push_str("/** The name of the catalog unit this package belongs to. */\n");
        let _ = writeln!(body, "export const CATALOG_NAME = \"{name}\";");
        body.push_str("/** The hash of the catalog unit, which the wasm module carries. */\n");
        let _ = writeln!(
            body,
            "export const CATALOG_HASH: Uint8Array = Uint8Array.from([{}]);",
            hash.join(", ")
        );
        let payloads = self.payload_types();
        let mut names: Vec<String> = payloads
            .iter()
            .map(|payload| self.qualified(payload.name))
            .collect();
        names.sort();
        body.push_str(
            "/** The qualified names of the payload types of this package, which name the wasm \
             type ids. */\n",
        );
        body.push_str("export const PAYLOAD_TYPES = [\n");
        for name in &names {
            let _ = writeln!(body, "  \"{name}\",");
        }
        body.push_str("] as const;\n");
        body.push_str(
            "/** The text of each rule code the wasm module reports, rule code n at index n - 2. \
             */\n",
        );
        let rules: Vec<String> = RULES.iter().map(|rule| format!("\"{rule}\"")).collect();
        let _ = writeln!(
            body,
            "export const RULES = [{}] as const;",
            rules.join(", ")
        );

        // The induced tuples of the interactions, as the plain-object types
        // `<base>.ts` has none for.
        for (index, tuple) in self.model.tuples.iter().enumerate() {
            if !self.is_payload_tuple(index, tuple) {
                continue;
            }
            self.emit_tuple_interface(&mut body, index, tuple)?;
        }

        let payload_names: BTreeSet<&str> = payloads.iter().map(|payload| payload.name).collect();
        for (index, declaration) in self.model.declarations.iter().enumerate() {
            let name = declared_name(declaration);
            if payload_names.contains(name)
                && self.declaration_bounded.get(&index) == Some(&true)
                && declaration.visibility() != v1::Visibility::Internal
            {
                self.emit_payload(&mut body, declaration)?;
            }
            if self.reached.contains(&index) {
                self.emit_converters(&mut body, index, declaration)?;
            }
            if self.reached_open.contains(&index) {
                self.emit_open_converters(&mut body, declaration);
            }
        }
        for (index, tuple) in self.model.tuples.iter().enumerate() {
            if self.is_payload_tuple(index, tuple) {
                self.emit_tuple_payload(&mut body, index, tuple)?;
            }
            if self.reached_tuples.contains(&index) {
                self.emit_tuple_converters(&mut body, index, tuple)?;
            }
        }

        body.push('\n');
        for payload in &payloads {
            if payload
                .declaration
                .is_some_and(|declaration| declaration.visibility() == v1::Visibility::Internal)
                || !self.used_locals.contains(payload.name)
            {
                continue;
            }
            let spelled = match payload.declaration {
                Some(declaration) => value_type(declaration),
                None => payload.name.to_string(),
            };
            let _ = writeln!(
                body,
                "const codec{name}: PayloadCodec<{spelled}> = {{ type: \"{qualified}\", encode: \
                 encode{name}, decode: decode{name} }};",
                name = payload.name,
                qualified = self.qualified(payload.name),
            );
        }
        for (constant, foreign) in &self.foreign_payloads {
            let alias = package_alias(&foreign.package);
            let _ = writeln!(
                body,
                "const {constant}: PayloadCodec<{alias}.{name}> = {{ type: \"{package}.{name}\", \
                 encode: {alias}_codec.encode{name}, decode: {alias}_codec.decode{name} }};",
                name = foreign.name,
                package = foreign.package,
            );
        }

        for interface in &self.model.interfaces {
            let Some(v1::interface::Identity::Declared(spelled)) = &interface.identity else {
                continue;
            };
            self.emit_interface(&mut body, interface, &spelled.declared)?;
        }

        let mut out = String::new();
        let segments = self.package.replace('.', "/");
        // The builder, `<base>.ts` and `flatc`'s barrel are named only by a
        // local payload type's functions and converters; a package whose
        // payload roots are all foreign has no table, so `flatc` writes no
        // barrel for it to import.
        let local = self.has_local_payload();
        if local {
            out.push_str("import * as flatbuffers from \"flatbuffers\";\n");
        }
        // The one import from the runtime package, written only when a
        // `codec<Name>` constant uses it, so that `deno lint` reads nothing
        // unused in a module with no interface.
        if !self.used_locals.is_empty() || !self.foreign_payloads.is_empty() {
            out.push_str("import type { PayloadCodec } from \"@driftsys/ridl-runtime\";\n");
        }
        if local {
            let _ = writeln!(out, "import type * as t from \"./{artifact_base}.js\";");
        }
        for package in &self.foreign_types {
            let _ = writeln!(
                out,
                "import type * as {} from \"./{package}.js\";",
                package_alias(package)
            );
        }
        if local {
            out.push_str(
                "// `flatc --ts --gen-object-api` writes this module when its `-o` is the \
                 directory\n// of this file.\n",
            );
            let _ = writeln!(out, "import * as fb from \"./{segments}.js\";");
        }
        let foreign_codecs: BTreeSet<&str> = self
            .foreign_payloads
            .values()
            .map(|foreign| foreign.package.as_str())
            .collect();
        for package in foreign_codecs {
            let _ = writeln!(
                out,
                "import * as {}_codec from \"./{package}.codec.js\";",
                package_alias(package)
            );
        }
        out.push('\n');
        if self.needs_absent {
            out.push_str(
                "/** Throws for a required field the buffer leaves out. */\n\
                 function absent(type: string, field: string): never {\n  \
                 throw new RangeError(`field ${field} of ${type} is absent`);\n}\n\n",
            );
        }
        if !self.reached_open.is_empty() {
            out.push_str(
                "/** Thrown for a value the codec refuses, with the text of the rule it breaks. */\n\
                 export class RuleError extends RangeError {\n  \
                 readonly rule: (typeof RULES)[number];\n  \
                 constructor(rule: (typeof RULES)[number], message: string) {\n    \
                 super(message);\n    \
                 this.name = \"RuleError\";\n    \
                 this.rule = rule;\n  }\n}\n\n",
            );
        }
        out.push_str(&body);
        Ok(GeneratedTs { source: out })
    }

    /// Whether the induced tuple at `index` is a payload type: a root with a
    /// finite bound, induced by an interaction.
    fn is_payload_tuple(&self, index: usize, tuple: &v1::InducedTuple) -> bool {
        matches!(
            tuple.origin,
            Some(v1::induced_tuple::Origin::Interaction(_))
        ) && self.tuple_bounded.get(&index) == Some(&true)
    }

    fn emit_tuple_interface(
        &mut self,
        out: &mut String,
        index: usize,
        tuple: &'a v1::InducedTuple,
    ) -> Result<(), GenerateError> {
        let name = tuple_name(tuple);
        let qualified = self.qualified(name);
        let origin = match &tuple.origin {
            Some(v1::induced_tuple::Origin::Interaction(path)) => {
                interaction_path(self.model, path)
            }
            _ => String::new(),
        };
        let _ = self.tuple_table(index)?;
        out.push('\n');
        let _ = writeln!(out, "/** The induced tuple of `{origin}`. */");
        if tuple.fields.is_empty() {
            // An empty `interface` is what `deno lint` refuses as
            // `no-empty-interface`; the alias names the same object type.
            let _ = writeln!(out, "export type {name} = Record<string, never>;");
            return Ok(());
        }
        let _ = writeln!(out, "export interface {name} {{");
        for field in &tuple.fields {
            let declared = field
                .name
                .as_ref()
                .map_or("", |name| name.declared.as_str());
            let (optional, spelled) = match field.r#type.as_ref() {
                Some(ty) => (ty.optional, self.ts_type(ty, &qualified)?),
                None => (false, "unknown".to_string()),
            };
            let marker = if optional { "?" } else { "" };
            let _ = writeln!(out, "  {declared}{marker}: {spelled};");
        }
        out.push_str("}\n");
        Ok(())
    }

    fn emit_tuple_payload(
        &mut self,
        out: &mut String,
        index: usize,
        tuple: &'a v1::InducedTuple,
    ) -> Result<(), GenerateError> {
        let name = tuple_name(tuple);
        let table = &self.tuple_table(index)?.name;
        self.emit_encode_decode(out, name, name, table, &self.qualified(name));
        Ok(())
    }

    /// The `encode`/`decode` pair of a payload type whose converters are
    /// `pack<name>`/`unpack<name>` over the `flatc` table `table`.
    fn emit_encode_decode(
        &self,
        out: &mut String,
        name: &str,
        spelled: &str,
        table: &str,
        qualified: &str,
    ) {
        out.push('\n');
        let _ = writeln!(out, "/** Encodes a `{qualified}` payload. */");
        let _ = writeln!(
            out,
            "export function encode{name}(value: {spelled}): Uint8Array {{"
        );
        out.push_str("  const builder = new flatbuffers.Builder();\n");
        let _ = writeln!(out, "  builder.finish(pack{name}(value).pack(builder));");
        out.push_str("  return builder.asUint8Array();\n}\n");
        let _ = writeln!(
            out,
            "/** Decodes a `{qualified}` payload the codec has verified. */"
        );
        let _ = writeln!(
            out,
            "export function decode{name}(bytes: Uint8Array): {spelled} {{"
        );
        let _ = writeln!(
            out,
            "  return unpack{name}(fb.{table}.getRootAs{table}(new \
             flatbuffers.ByteBuffer(bytes)).unpack());"
        );
        out.push_str("}\n");
    }

    fn emit_payload(
        &self,
        out: &mut String,
        declaration: &v1::Declaration,
    ) -> Result<(), GenerateError> {
        use v1::declaration::Kind;
        let name = declared_name(declaration);
        let qualified = self.qualified(name);
        let boxed = match &declaration.kind {
            Some(Kind::Struct(_) | Kind::Union(_)) => {
                self.emit_encode_decode(out, name, &format!("t.{name}"), name, &qualified);
                return Ok(());
            }
            Some(Kind::Enum(shape)) if shape.open => {
                (format!("new fb.{name}BoxT(pack{name}(value))"), Read::Open)
            }
            Some(Kind::EnumSet(shape)) if shape.open => (
                format!("new fb.{name}BoxT(pack{name}(value))"),
                Read::Assertion,
            ),
            Some(Kind::Enum(_)) => (format!("new fb.{name}BoxT(BigInt(value))"), Read::Enum),
            Some(Kind::Scalar(scalar)) if scalar.class() == v1::ScalarClass::Bytes => {
                (format!("new fb.{name}BoxT(Array.from(value))"), Read::Bytes)
            }
            Some(Kind::Scalar(_) | Kind::EnumSet(_)) => {
                (format!("new fb.{name}BoxT(value)"), Read::Assertion)
            }
            _ => {
                return refuse(format!(
                    "malformed codegen model: the payload type `{qualified}` is not a scalar, \
                     an enum, an enum set, a struct or a union"
                ));
            }
        };
        let (packed, read_as) = boxed;
        let spelled = value_type(declaration);
        out.push('\n');
        let _ = writeln!(out, "/** Encodes a `{qualified}` payload. */");
        let _ = writeln!(
            out,
            "export function encode{name}(value: {spelled}): Uint8Array {{"
        );
        out.push_str("  const builder = new flatbuffers.Builder();\n");
        let _ = writeln!(out, "  builder.finish({packed}.pack(builder));");
        out.push_str("  return builder.asUint8Array();\n}\n");
        let _ = writeln!(
            out,
            "/** Decodes a `{qualified}` payload the codec has verified. */"
        );
        let _ = writeln!(
            out,
            "export function decode{name}(bytes: Uint8Array): {spelled} {{"
        );
        let read = format!(
            "fb.{name}Box.getRootAs{name}Box(new flatbuffers.ByteBuffer(bytes)).unpack().value"
        );
        match read_as {
            Read::Enum => {
                let _ = writeln!(out, "  return Number({read}) as t.{name};");
            }
            Read::Open => {
                let _ = writeln!(out, "  return unpack{name}({read});");
            }
            Read::Assertion => {
                let _ = writeln!(out, "  return {read} as t.{name};");
            }
            Read::Bytes => {
                let _ = writeln!(out, "  return Uint8Array.from({read}) as t.{name};");
            }
        }
        out.push_str("}\n");
        Ok(())
    }

    /// The converter pair of an open enum or an open enum set. The enum pair
    /// keeps every value: an undeclared one is `{ unknown: n }`, and a value
    /// beyond the safe integers is refused. The enum set pair refuses a value
    /// that does not fit the width of the set.
    fn emit_open_converters(&self, out: &mut String, declaration: &v1::Declaration) {
        use v1::declaration::Kind;
        let name = declared_name(declaration);
        let qualified = self.qualified(name);
        out.push('\n');
        match &declaration.kind {
            Some(Kind::Enum(shape)) => {
                let declared: Vec<String> = shape
                    .values
                    .iter()
                    .map(|value| value.value.to_string())
                    .collect();
                let _ = writeln!(
                    out,
                    "function pack{name}(value: t.{name}Value): bigint {{\n  \
                     const raw = typeof value === \"number\" ? value : value.unknown;\n  \
                     if (!Number.isSafeInteger(raw)) {{\n    \
                     throw new RuleError(\"variant\", `value ${{raw}} of {qualified} is not a \
                     safe integer`);\n  }}\n  \
                     return BigInt(raw);\n}}"
                );
                let _ = writeln!(
                    out,
                    "function unpack{name}(raw: bigint | null): t.{name}Value {{\n  \
                     const value = raw ?? 0n;\n  \
                     if (value > {MAX_SAFE}n || value < -{MAX_SAFE}n) {{\n    \
                     throw new RuleError(\"variant\", `value ${{value}} of {qualified} is outside \
                     the safe integers`);\n  }}\n  \
                     const n = Number(value);\n  \
                     return ([{declared}] as readonly number[]).includes(n)\n    \
                     ? n as t.{name}\n    : {{ unknown: n }};\n}}",
                    declared = declared.join(", ")
                );
            }
            Some(Kind::EnumSet(shape)) => {
                let (wide, signed, bits) = match shape.width() {
                    v1::IntWidth::U8 => (false, false, 8),
                    v1::IntWidth::I8 => (false, true, 8),
                    v1::IntWidth::U16 => (false, false, 16),
                    v1::IntWidth::I16 => (false, true, 16),
                    v1::IntWidth::U32 => (false, false, 32),
                    v1::IntWidth::I32 => (false, true, 32),
                    v1::IntWidth::U64 => (true, false, 64),
                    v1::IntWidth::I64 | v1::IntWidth::Unspecified => (true, true, 64),
                };
                let (low, high) = if signed {
                    (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
                } else {
                    (0, (1i128 << bits) - 1)
                };
                let (suffix, base, check) = if wide {
                    ("n", "bigint", "")
                } else {
                    ("", "number", "!Number.isInteger(value) || ")
                };
                let _ = writeln!(
                    out,
                    "function pack{name}(value: t.{name}): {base} {{\n  \
                     if ({check}value < {low}{suffix} || value > {high}{suffix}) {{\n    \
                     throw new RuleError(\"range\", `value ${{value}} of {qualified} does not fit \
                     {bits} bits`);\n  }}\n  \
                     return value;\n}}"
                );
            }
            _ => {}
        }
    }

    fn emit_converters(
        &mut self,
        out: &mut String,
        index: usize,
        declaration: &'a v1::Declaration,
    ) -> Result<(), GenerateError> {
        use v1::declaration::Kind;
        let name = declared_name(declaration);
        let qualified = self.qualified(name);
        match &declaration.kind {
            Some(Kind::Struct(shape)) => {
                let table = self.struct_table(index);
                let mut fields = Vec::new();
                for (slot_index, slot) in shape.slots.iter().enumerate() {
                    let Some(v1::slot::Occupant::Field(field)) = &slot.occupant else {
                        continue;
                    };
                    let Some(ty) = field.r#type.as_ref() else {
                        continue;
                    };
                    let declared = field
                        .name
                        .as_ref()
                        .map_or("", |name| name.declared.as_str());
                    let null_default = table.is_some_and(|table| {
                        table.slots.iter().any(|fb_slot| {
                            matches!(fb_slot.source, Some(v1::fb_slot::Source::Field(i)) if i as usize == slot_index)
                                && fb_slot.needs_null_default
                        })
                    });
                    fields.push(RecordField {
                        declared: declared.to_string(),
                        accessor: accessor(declared),
                        rule: self.rule(ty, &qualified)?,
                        optional: ty.optional,
                        // An open enum has no absent value: 0 is a value of it.
                        null_default: null_default && !self.is_open_enum(ty),
                    });
                }
                self.emit_record(
                    out,
                    name,
                    &format!("t.{name}"),
                    name,
                    "object",
                    &fields,
                    &qualified,
                );
                Ok(())
            }
            Some(Kind::Union(union)) => self.emit_union(out, name, union, &qualified),
            _ => Ok(()),
        }
    }

    fn emit_tuple_converters(
        &mut self,
        out: &mut String,
        index: usize,
        tuple: &'a v1::InducedTuple,
    ) -> Result<(), GenerateError> {
        let name = tuple_name(tuple);
        let qualified = self.qualified(name);
        let table = self.tuple_table(index)?;
        let spelled = if self.is_payload_tuple(index, tuple) {
            name.to_string()
        } else {
            self.ts_type(
                &v1::Type {
                    optional: false,
                    kind: Some(v1::r#type::Kind::Tuple(v1::TupleRef {
                        index: index as u32,
                    })),
                },
                &qualified,
            )?
        };
        let mut fields = Vec::new();
        for (position, field) in tuple.fields.iter().enumerate() {
            let Some(ty) = field.r#type.as_ref() else {
                continue;
            };
            let declared = field
                .name
                .as_ref()
                .map_or("", |name| name.declared.as_str());
            let null_default = table.slots.iter().any(|fb_slot| {
                matches!(fb_slot.source, Some(v1::fb_slot::Source::TuplePosition(p)) if p as usize == position + 1)
                    && fb_slot.needs_null_default
            });
            fields.push(RecordField {
                declared: declared.to_string(),
                accessor: format!("field{}", position + 1),
                rule: self.rule(ty, &qualified)?,
                optional: ty.optional,
                null_default: null_default && !self.is_open_enum(ty),
            });
        }
        self.emit_record(
            out,
            name,
            &spelled,
            &table.name,
            "tuple",
            &fields,
            &qualified,
        );
        Ok(())
    }

    /// The converter pair of a struct or a tuple: `pack<name>` fills the
    /// `flatc` object `table`T member by member, `unpack<name>` builds the
    /// plain object `spelled`.
    #[allow(clippy::too_many_arguments)]
    fn emit_record(
        &mut self,
        out: &mut String,
        name: &str,
        spelled: &str,
        table: &str,
        object: &str,
        fields: &[RecordField],
        qualified: &str,
    ) {
        out.push('\n');
        // A record with no field reads nothing from its parameter, which
        // `deno lint` accepts under a `_` prefix.
        let unused = if fields.is_empty() { "_" } else { "" };
        let _ = writeln!(
            out,
            "function pack{name}({unused}value: {spelled}): fb.{table}T {{"
        );
        let _ = writeln!(out, "  const {object} = new fb.{table}T();");
        for field in fields {
            let packed = field
                .rule
                .pack
                .replace("{}", &format!("value.{}", field.declared));
            if field.optional {
                // The object API holds a vector as `T[]`, never `null`, and
                // `pack` always writes it, so an absent optional vector
                // packs as the empty vector: the wire does not keep absent
                // and empty apart (the record's known gaps).
                let absent = if field.rule.vector { "[]" } else { "null" };
                let _ = writeln!(
                    out,
                    "  {object}.{} = value.{} === undefined ? {absent} : {packed};",
                    field.accessor, field.declared
                );
            } else {
                let _ = writeln!(out, "  {object}.{} = {packed};", field.accessor);
            }
        }
        let _ = writeln!(out, "  return {object};\n}}");
        let _ = writeln!(
            out,
            "function unpack{name}({unused}object: fb.{table}T): {spelled} {{"
        );
        let mut required = Vec::new();
        let mut optional = Vec::new();
        for field in fields {
            let read = format!("object.{}", field.accessor);
            // An optional vector is read as present: the object API gives
            // `[]` for a vector the buffer leaves out.
            if field.optional && !field.rule.vector {
                let unpacked = field.rule.unpack.replace("{}", &read);
                optional.push(format!(
                    "  if ({read} !== null) result.{} = {unpacked};",
                    field.declared
                ));
                continue;
            }
            let unpacked = if field.null_default {
                self.needs_absent = true;
                format!(
                    "{read} === null ? absent(\"{qualified}\", \"{}\") : {}",
                    field.declared,
                    field.rule.unpack.replace("{}", &read)
                )
            } else {
                self.guarded(&read, &field.rule, qualified, &field.declared)
            };
            required.push(format!("{}: {unpacked}", field.declared));
        }
        if optional.is_empty() {
            if required.is_empty() {
                out.push_str("  return {};\n");
            } else if object == "tuple" {
                let _ = writeln!(out, "  return {{ {} }};", required.join(", "));
            } else {
                out.push_str("  return {\n");
                for line in &required {
                    let _ = writeln!(out, "    {line},");
                }
                out.push_str("  };\n");
            }
        } else {
            let _ = writeln!(out, "  const result: {spelled} = {{");
            for line in &required {
                let _ = writeln!(out, "    {line},");
            }
            out.push_str("  };\n");
            for line in &optional {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("  return result;\n");
        }
        out.push_str("}\n");
    }

    fn emit_union(
        &mut self,
        out: &mut String,
        name: &str,
        union: &'a v1::Union,
        qualified: &str,
    ) -> Result<(), GenerateError> {
        let mut packs = Vec::new();
        let mut unpacks = Vec::new();
        for arm in &union.arms {
            let Some(reference) = arm.r#type.as_ref() else {
                continue;
            };
            let declared = arm.name.as_ref().map_or("", |name| name.declared.as_str());
            let snake = arm.name.as_ref().map_or("", |name| name.snake.as_str());
            let ty = v1::Type {
                optional: false,
                kind: Some(v1::r#type::Kind::Named(reference.clone())),
            };
            let rule = self.rule(&ty, qualified)?;
            let arm_box = arm
                .flatbuffers_box
                .and_then(|table| self.tables().get(table as usize))
                .map(|table| table.name.as_str());
            let (packed, read) = match arm_box {
                Some(box_name) => (
                    format!(
                        "new fb.{box_name}T({})",
                        rule.pack.replace("{}", "value.value")
                    ),
                    format!("(object.value as fb.{box_name}T).value"),
                ),
                None => {
                    let arm_table = match self.resolve(reference, qualified)? {
                        Target::Local(_, declaration) => declared_name(declaration),
                        Target::Foreign(..) => {
                            return refuse(format!(
                                "malformed codegen model: the arm `{declared}` of `{qualified}` \
                                 is a foreign table"
                            ));
                        }
                    };
                    (
                        rule.pack.replace("{}", "value.value"),
                        format!("object.value as fb.{arm_table}T"),
                    )
                }
            };
            packs.push(format!(
                "    case \"{declared}\":\n      return new fb.{name}T(fb.{name}Union.{snake}, \
                 {packed});"
            ));
            unpacks.push(format!(
                "    case fb.{name}Union.{snake}:\n      return {{ kind: \"{declared}\", value: \
                 {} }};",
                rule.unpack.replace("{}", &read)
            ));
        }
        out.push('\n');
        let _ = writeln!(out, "function pack{name}(value: t.{name}): fb.{name}T {{");
        out.push_str("  switch (value.kind) {\n");
        for case in &packs {
            out.push_str(case);
            out.push('\n');
        }
        out.push_str("  }\n}\n");
        let _ = writeln!(
            out,
            "function unpack{name}(object: fb.{name}T): t.{name} {{"
        );
        out.push_str("  switch (object.valueType) {\n");
        for case in &unpacks {
            out.push_str(case);
            out.push('\n');
        }
        let _ = writeln!(
            out,
            "    default:\n      throw new RangeError(\"union {qualified} carries no arm\");"
        );
        out.push_str("  }\n}\n");
        Ok(())
    }

    fn emit_interface(
        &mut self,
        out: &mut String,
        interface: &'a v1::Interface,
        name: &str,
    ) -> Result<(), GenerateError> {
        let number = interface.number;
        out.push('\n');
        let _ = writeln!(
            out,
            "/** The members of interface `{name}`, interface number `{number}`. */"
        );
        let _ = writeln!(out, "export const {name} = {{");
        let _ = writeln!(out, "  iface: {number},");
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = &slot.occupant
            else {
                continue;
            };
            let member = interaction
                .name
                .as_ref()
                .map_or("", |name| name.declared.as_str());
            // A member that carries a stream has no entry (see `Codec::new`).
            if !interaction.streams.is_empty() {
                let _ = writeln!(out, "  // {member} has no entry: it carries a stream");
                continue;
            }
            let mut roles = Vec::new();
            let mut missing = None;
            for (role, payload) in self.roles(interaction, name, member)? {
                match payload {
                    Ok(constant) => roles.push(format!("{role}: {constant}")),
                    Err(qualified) => {
                        missing = Some(qualified);
                        break;
                    }
                }
            }
            if let Some(qualified) = missing {
                let _ = writeln!(
                    out,
                    "  // {member} has no entry: {qualified} has no finite FlatBuffers bound"
                );
                continue;
            }
            let kind = match interaction.kind() {
                v1::Kind::Signal => "signal",
                v1::Kind::Event => "event",
                v1::Kind::Command => "command",
                v1::Kind::Query => "query",
                v1::Kind::Fixed => "fixed",
                v1::Kind::Unspecified => {
                    return refuse(format!(
                        "malformed codegen model: `{name}.{member}` has no kind"
                    ));
                }
            };
            // `limit` is written only when the catalog records one: absent,
            // the member has the default ceiling of its kind.
            if let Some(limit) = interaction.limit {
                roles.push(format!("limit: {limit}"));
            }
            let _ = writeln!(
                out,
                "  {member}: {{ iface: {number}, ord: {}, kind: \"{kind}\", {} }},",
                slot.ordinal,
                roles.join(", ")
            );
        }
        out.push_str("} as const;\n");
        Ok(())
    }

    /// The roles of `interaction`, each with its payload type: `Ok` with the
    /// `codec<Name>` constant when the type has a codec, `Err` with the
    /// qualified name when it has no finite bound.
    #[allow(clippy::type_complexity)]
    fn roles(
        &mut self,
        interaction: &'a v1::Interaction,
        interface: &str,
        member: &str,
    ) -> Result<Vec<(&'static str, Result<String, String>)>, GenerateError> {
        use v1::interaction::Shape;
        let path = format!("{interface}.{member}");
        Ok(match &interaction.shape {
            Some(Shape::Signal(signal)) => {
                vec![(
                    "payload",
                    self.payload(signal.payload.as_ref(), None, &path)?,
                )]
            }
            Some(Shape::Event(event)) => {
                vec![(
                    "payload",
                    self.payload(event.payload.as_ref(), None, &path)?,
                )]
            }
            Some(Shape::Fixed(fixed)) => {
                vec![("payload", self.payload(fixed.named.as_ref(), None, &path)?)]
            }
            Some(Shape::Command(command)) => vec![(
                "request",
                self.payload(
                    command.request.as_ref(),
                    command.request_tuple.as_ref(),
                    &path,
                )?,
            )],
            Some(Shape::Query(query)) => {
                let mut roles = vec![(
                    "request",
                    self.payload(query.request.as_ref(), query.request_tuple.as_ref(), &path)?,
                )];
                let fallible = matches!(
                    query.reply.as_ref().and_then(|reply| reply.kind.as_ref()),
                    Some(v1::reply::Kind::Fallible(_))
                );
                if query.reply_payload.is_some() || query.reply_tuple.is_some() || !fallible {
                    roles.push((
                        "reply",
                        self.payload(
                            query.reply_payload.as_ref(),
                            query.reply_tuple.as_ref(),
                            &path,
                        )?,
                    ));
                }
                roles
            }
            None => {
                return refuse(format!("malformed codegen model: `{path}` has no shape"));
            }
        })
    }

    /// The payload type of one role: the named payload, or the induced
    /// tuple.
    fn payload(
        &mut self,
        named: Option<&'a v1::Payload>,
        tuple: Option<&v1::TupleRef>,
        path: &str,
    ) -> Result<Result<String, String>, GenerateError> {
        if let Some(payload) = named
            && let Some(reference) = payload.r#type.as_ref()
        {
            let qualified = format!("{}.{}", reference.package, reference.reference);
            return match self.resolve(reference, &qualified)? {
                Target::Local(index, declaration) => {
                    let name = declared_name(declaration);
                    if declaration.visibility() == v1::Visibility::Internal {
                        return refuse(format!(
                            "`{path}` has the payload type `{}`, which is internal; `<base>.ts` \
                             does not export it, so the codec module cannot name it",
                            self.qualified(name)
                        ));
                    }
                    let bounded = self.declaration_bounded.get(&index) == Some(&true);
                    Ok(if bounded {
                        self.used_locals.insert(name.to_string());
                        Ok(format!("codec{name}"))
                    } else {
                        Err(self.qualified(name))
                    })
                }
                Target::Foreign(package, declaration) => {
                    let name = declared_name(declaration);
                    if let Some(word) = foreign_kind(declaration) {
                        return refuse(foreign_refusal(
                            &format!("`{path}` has the payload type"),
                            package,
                            name,
                            word,
                        ));
                    }
                    let qualified = format!("{package}.{name}");
                    if payload.flatbuffers_max_size.is_none() {
                        return Ok(Err(qualified));
                    }
                    self.foreign_types.insert(package.to_string());
                    let constant = format!(
                        "codec{}{name}",
                        ridl_ir::name::pascal_case(&package.replace('.', "_"))
                    );
                    // Two foreign payload types can spell one constant
                    // (`a.b` / `C` and `a` / `BC`); the second is refused.
                    if let Some(other) = self.foreign_payloads.get(&constant)
                        && (other.package != package || other.name != name)
                    {
                        return refuse(format!(
                            "`{qualified}` in package `{}` collides with `{}.{}`, which the codec \
                             module also declares",
                            self.package, other.package, other.name
                        ));
                    }
                    self.foreign_payloads.insert(
                        constant.clone(),
                        ForeignPayload {
                            package: package.to_string(),
                            name: name.to_string(),
                        },
                    );
                    Ok(Ok(constant))
                }
            };
        }
        if let Some(tuple) = tuple {
            let Some(induced) = self.model.tuples.get(tuple.index as usize) else {
                return refuse(format!(
                    "malformed codegen model: `{path}` names tuple {}, which does not exist",
                    tuple.index
                ));
            };
            let name = tuple_name(induced);
            let bounded = self.tuple_bounded.get(&(tuple.index as usize)) == Some(&true);
            return Ok(if bounded {
                self.used_locals.insert(name.to_string());
                Ok(format!("codec{name}"))
            } else {
                Err(self.qualified(name))
            });
        }
        refuse(format!(
            "`{path}` has a payload that is not a named type or an induced tuple, which the codec \
             module does not carry"
        ))
    }
}

/// Refuses an optional `ty` at a position that is not a table field, as the
/// generated Rust codec does: FlatBuffers has no absent vector element, map
/// key or map value, so the object API has no form for one.
fn refuse_optional(ty: &v1::Type, qualified: &str, what: &str) -> Result<(), GenerateError> {
    if ty.optional {
        return refuse(format!(
            "`{qualified}` reaches {what} that is optional, which FlatBuffers cannot represent: \
             only a table field may be absent"
        ));
    }
    Ok(())
}

fn not_converted(qualified: &str, kind: &str) -> String {
    format!("`{qualified}` reaches {kind}, which the codec module does not convert yet")
}

/// The word of a foreign declaration's kind when the module needs a
/// converter for it; `None` for a named scalar or an enum set, which
/// convert by assertion.
fn foreign_kind(declaration: &v1::Declaration) -> Option<&'static str> {
    use v1::declaration::Kind;
    match &declaration.kind {
        Some(Kind::Struct(_)) => Some("struct"),
        Some(Kind::Union(_)) => Some("union"),
        Some(Kind::Enum(_)) => Some("enum"),
        _ => None,
    }
}

fn foreign_refusal(site: &str, package: &str, name: &str, word: &str) -> String {
    format!(
        "{site} `{package}.{name}`, a {word} of another package, which the codec module does not \
         convert yet"
    )
}

/// How the decode function of a boxed payload type reads the value of its box.
enum Read {
    /// `Number(box.value)` of a strict enum.
    Enum,
    /// The unpack function of an open enum.
    Open,
    /// A type assertion over the box value.
    Assertion,
    /// `Uint8Array.from` of the box value.
    Bytes,
}

/// The TypeScript type of a value of `declaration`: `t.<Name>`, or
/// `t.<Name>Value` for an open enum.
fn value_type(declaration: &v1::Declaration) -> String {
    let name = declared_name(declaration);
    match &declaration.kind {
        Some(v1::declaration::Kind::Enum(shape)) if shape.open => format!("t.{name}Value"),
        _ => format!("t.{name}"),
    }
}

/// The conversion of a position whose value is copied as it is.
fn copy_rule() -> Rule {
    Rule {
        pack: "{}".to_string(),
        unpack: "{}".to_string(),
        plain: true,
        nullable: false,
        vector: false,
    }
}

/// A string: `flatc` reads one as `string | Uint8Array | null`.
fn string_rule() -> Rule {
    Rule {
        pack: "{}".to_string(),
        unpack: "{} as string".to_string(),
        plain: true,
        nullable: false,
        vector: false,
    }
}

/// Bytes: `flatc` holds a `[ubyte]` as `number[]`.
fn bytes_rule(spelled: &str) -> Rule {
    Rule {
        pack: "Array.from({})".to_string(),
        unpack: format!("Uint8Array.from({{}}) as {spelled}"),
        plain: false,
        nullable: false,
        vector: true,
    }
}

/// The TypeScript import alias of a package, as `<base>.ts` spells it.
fn package_alias(package: &str) -> String {
    package.replace('.', "_")
}

/// The TypeScript type of a bare primitive, as `<base>.ts` spells it.
fn primitive_ts(primitive: i32) -> &'static str {
    match v1::PrimitiveType::try_from(primitive) {
        Ok(v1::PrimitiveType::Boolean) => "boolean",
        Ok(v1::PrimitiveType::Integer) => "bigint",
        Ok(v1::PrimitiveType::Float) => "number",
        Ok(v1::PrimitiveType::String) => "string",
        Ok(v1::PrimitiveType::Bytes) => "Uint8Array",
        _ => "unknown",
    }
}

/// The TypeScript type of an inline scalar, as `<base>.ts` spells it: a
/// 64-bit integer is a `bigint`.
fn scalar_ts(scalar: &v1::Scalar) -> &'static str {
    match scalar.class() {
        v1::ScalarClass::Float => "number",
        v1::ScalarClass::Integer => match scalar.width {
            Some(v1::scalar::Width::IntWidth(width))
                if width == v1::IntWidth::U64 as i32 || width == v1::IntWidth::I64 as i32 =>
            {
                "bigint"
            }
            _ => "number",
        },
        v1::ScalarClass::Boolean => "boolean",
        v1::ScalarClass::String => "string",
        v1::ScalarClass::Bytes => "Uint8Array",
        v1::ScalarClass::Unspecified => "unknown",
    }
}

/// An element type that is a union or an object type, parenthesized for
/// the `[]` suffix.
fn parenthesized(spelled: String) -> String {
    if spelled.contains(' ') {
        format!("({spelled})")
    } else {
        spelled
    }
}

/// The names `flatc --ts` 25.12.19 escapes with a trailing `_`, from the
/// keyword list of its TypeScript generator, `src/idl_gen_ts.cpp`. The check
/// runs on the lower camel case name, so `my_class` is `myClass` and is not
/// escaped.
const FLATC_TS_KEYWORDS: [&str; 47] = [
    "arguments",
    "as",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// The name `flatc --ts` gives a field's member. The `.fbs` field is
/// `ridl_ir::name::snake_case` of the declared name, and `flatc` writes it in
/// lower camel case: each `_` is dropped and the character after it is upper
/// case; every other character is kept, digits included. A name in
/// [`FLATC_TS_KEYWORDS`] gets a trailing `_`, as `flatc` writes it.
fn accessor(declared: &str) -> String {
    let snake = ridl_ir::name::snake_case(declared);
    let mut out = String::with_capacity(snake.len() + 1);
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    if FLATC_TS_KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

fn declared_name(declaration: &v1::Declaration) -> &str {
    declaration
        .name
        .as_ref()
        .map_or("", |name| name.declared.as_str())
}

fn tuple_name(tuple: &v1::InducedTuple) -> &str {
    tuple.name.as_ref().map_or("", |name| name.rust.as_str())
}

/// The live fields of a struct, in slot order.
fn struct_fields(shape: &v1::Struct) -> Vec<&v1::Field> {
    shape
        .slots
        .iter()
        .filter_map(|slot| match &slot.occupant {
            Some(v1::slot::Occupant::Field(field)) => Some(&**field),
            _ => None,
        })
        .collect()
}

/// `<Interface>.<member>` of an interaction path.
fn interaction_path(model: &v1::Model, path: &v1::InteractionPath) -> String {
    let Some(interface) = model.interfaces.get(path.interface as usize) else {
        return String::new();
    };
    let interface_name = match &interface.identity {
        Some(v1::interface::Identity::Declared(name)) => name.declared.clone(),
        Some(v1::interface::Identity::Service(name)) => name.dotted.clone(),
        None => String::new(),
    };
    let member = interface
        .slots
        .get(path.slot as usize)
        .and_then(|slot| match &slot.occupant {
            Some(v1::interaction_slot::Occupant::Interaction(interaction)) => {
                interaction.name.as_ref().map(|name| name.declared.clone())
            }
            _ => None,
        })
        .unwrap_or_default();
    format!("{interface_name}.{member}")
}

#[cfg(test)]
mod tests {
    use super::{FLATC_TS_KEYWORDS, accessor};

    /// The list is the one `flatc` 25.12.19 escapes, read from the keyword
    /// table of its TypeScript generator, `src/idl_gen_ts.cpp`, and checked
    /// against its output for a table with every name as a field.
    #[test]
    fn the_keyword_list_is_the_one_flatc_escapes() {
        assert_eq!(
            FLATC_TS_KEYWORDS,
            [
                "arguments",
                "as",
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "debugger",
                "default",
                "delete",
                "do",
                "else",
                "enum",
                "export",
                "extends",
                "false",
                "finally",
                "for",
                "function",
                "if",
                "implements",
                "import",
                "in",
                "instanceof",
                "interface",
                "let",
                "new",
                "null",
                "package",
                "private",
                "protected",
                "public",
                "return",
                "static",
                "super",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "typeof",
                "var",
                "void",
                "while",
                "with",
                "yield",
            ]
        );
    }

    /// `flatc --ts` lower-camels the snake case `.fbs` name, so an acronym
    /// or a SCREAMING_SNAKE name is not spelled as declared.
    #[test]
    fn the_accessor_is_the_lower_camel_case_of_the_snake_case_name() {
        for (declared, expected) in [
            ("code", "code"),
            ("setLevel", "setLevel"),
            ("getVIN", "getVin"),
            ("HTTPServer", "httpServer"),
            ("MAX_RPM", "maxRpm"),
            ("sensor2Value", "sensor2Value"),
            // A TypeScript keyword gets the `_` that `flatc` appends; a
            // name that camel-cases to a keyword is one, and `eval`,
            // `object` and `toString` are not on `flatc`'s list.
            ("default", "default_"),
            ("new", "new_"),
            ("class", "class_"),
            ("myClass", "myClass"),
            ("eval", "eval"),
            ("object", "object"),
            ("toString", "toString"),
        ] {
            assert_eq!(accessor(declared), expected, "{declared}");
        }
    }
}
