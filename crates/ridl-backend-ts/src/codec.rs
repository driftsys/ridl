//! The codec module `<base>.codec.ts`, written from the codegen model only.
//!
//! The module wraps the object API that `flatc --ts --gen-object-api` writes
//! for the package's `.fbs` in plain-object functions: one
//! `encode<Name>(value)` and one `decode<Name>(bytes)` per payload type, which
//! is every FlatBuffers root of the package with a finite bound, except an
//! internal one, whose type `<base>.ts` does not export. A private converter
//! pair `pack<Name>` and `unpack<Name>` is written for each struct, union and
//! induced tuple a payload type reaches. Each strict enum and strict enum set
//! it reaches gets an `unpack<Name>` that refuses a value the declaration does
//! not name. Each open enum gets a pair `pack<Name>` and `unpack<Name>` that
//! keeps every value, and each open enum set gets a `pack<Name>` that refuses a
//! value outside the width of the set with the `range` rule. An induced tuple of an interaction
//! is exported as an `interface`, or as the type alias
//! `Record<string, never>` when it has no field. The module also exports the
//! `RuleError` class when it reaches an enum or an enum set, the catalog's name
//! and hash, the qualified names of the payload types, the text of the rule
//! codes of the wasm module, and one member table per declared interface,
//! whose entry carries the member's payload limit when the model records one.
//! A package with a signal also gets `signalChecks(codec)`, which returns one
//! `SignalCheckEntry` of the runtime package per signal, for the `checks` option
//! of a session. The entry's check refuses a payload that the wasm codec rejects.
//! Stream queries and commands get typed functions over a runtime `StreamSession`:
//! they verify requests before sending, verify named elements before decoding,
//! decode raw text as UTF-8, and preserve raw byte streams. Uploads encode and
//! verify whole named elements, and split raw bytes or text at the member limit.
//! Typed functions are omitted for calls with two or more parameter streams,
//! uploads with an inline `T | E` reply, and calls whose request, named element
//! or upload reply has no finite FlatBuffers bound.
//!
//! A named scalar of another package converts by a type assertion, an enum set
//! of another package by a checking function of the module, and a payload type
//! of another package that is a named scalar or an enum set goes through that
//! package's own codec module. A struct, a union or an enum of another
//! package needs a converter of its own, which the module does not write yet:
//! a payload type that reaches one is refused with a [`GenerateError`] that
//! names both.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use ridl_ir::codegen::v1;

use crate::{GenerateError, GeneratedTs};

/// The text of the rule codes the wasm module reports, rule code `n` at
/// index `n - 2`: the order of `WASM_RULES` in the generated crate.
const RULES: [&str; 7] = [
    "length", "pattern", "range", "step", "unique", "utf8", "variant",
];

/// The largest integer a TypeScript number holds exactly.
const MAX_SAFE: u64 = 9_007_199_254_740_991;

/// The length of `Catalog.hash`.
const HASH_LEN: usize = 32;

/// Generates `<artifact_base>.codec.ts` for the package `model` describes,
/// or `None` for a package with neither a payload root that gets a codec,
/// local or foreign, nor a supported stream call. The returned source
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

/// Where the codec module of `package` finds `flatc`'s barrel: the import path,
/// without the `./` prefix and the `.js` suffix, and the comment that names the
/// directory `flatc`'s `-o` has to be. Both come from one test, so they cannot
/// disagree.
///
/// A package name with two or more components uses its segments, so
/// `veh.cabin` gives `veh/cabin`, and `-o` is the directory of the module. A
/// package name with one component would give the path of its own domain
/// module, so its barrel is under `flatc_out/`. No package name has a segment
/// with an underscore (ADR-0002), so that directory cannot be the first segment
/// of another package's barrel path.
fn flatc_barrel(package: &str) -> (String, &'static str) {
    if package.contains('.') {
        (
            package.replace('.', "/"),
            "// `flatc --ts --gen-object-api` writes this module when its `-o` is the \
             directory\n// of this file.\n",
        )
    } else {
        (
            format!("flatc_out/{package}"),
            "// `flatc --ts --gen-object-api` writes this module when its `-o` is the \
             flatc_out\n// subdirectory of this file's directory.\n",
        )
    }
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
#[derive(Clone)]
struct ForeignPayload {
    package: String,
    name: String,
}

/// A supported stream call whose payload codecs were resolved during discovery.
struct StreamCall<'a> {
    interaction: &'a v1::Interaction,
    params: Vec<&'a v1::Param>,
    named_request: bool,
    upload: bool,
    reply: Option<(String, String)>,
    duplex_reply: Option<StreamElement>,
    request: String,
    element: Option<String>,
    element_type: String,
    form: &'static str,
    entry_binding: String,
    bindings: Vec<String>,
    param_types: Vec<String>,
}

/// A stream direction with a resolved payload codec when its elements are named.
struct StreamElement {
    form: &'static str,
    element: Option<String>,
    element_type: String,
}

impl StreamCall<'_> {
    fn uses_adapter(&self, upload: bool, form: &str) -> bool {
        (self.upload == upload && self.form == form)
            || (!upload
                && self
                    .duplex_reply
                    .as_ref()
                    .is_some_and(|reply| reply.form == form))
    }
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
    /// The open enum sets of other packages a payload type reaches, by
    /// qualified name: each gets a function that checks a value against the
    /// width of the set.
    foreign_open_sets: BTreeMap<String, (&'a str, &'a v1::Declaration)>,
    /// The strict enums and strict enum sets of this package that a payload
    /// type reaches, by index: each gets an unpack function that refuses a
    /// value the declaration does not name.
    reached_strict: BTreeSet<usize>,
    /// The strict enum sets of other packages a payload type reaches, by
    /// qualified name: each gets a function that checks a value against the
    /// declared bits.
    foreign_strict_sets: BTreeMap<String, (&'a str, &'a v1::Declaration)>,
    stream_calls: BTreeMap<(u32, u32), StreamCall<'a>>,
    stream_omissions: BTreeMap<(u32, u32), String>,
    /// The member table entries of the signals that have one, as
    /// `<Interface>.<member>`, in declaration order.
    signal_entries: Vec<String>,
    needs_domain: bool,
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
            foreign_open_sets: BTreeMap::new(),
            reached_strict: BTreeSet::new(),
            foreign_strict_sets: BTreeMap::new(),
            stream_calls: BTreeMap::new(),
            stream_omissions: BTreeMap::new(),
            signal_entries: Vec::new(),
            needs_domain: false,
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
                if !interaction.streams.is_empty() {
                    codec.discover_stream(interface, slot, interaction, &spelled.declared)?;
                    continue;
                }
                let member = interaction
                    .name
                    .as_ref()
                    .map_or("", |name| name.declared.as_str());
                codec.roles(interaction, &spelled.declared, member)?;
            }
        }
        let mut names = codec.check_names()?;
        for stream in codec.stream_calls.values_mut() {
            let mut binding = "streamEntry".to_string();
            let mut suffix = 1;
            while !names.insert(binding.clone()) {
                binding = format!("streamEntry{suffix}");
                suffix += 1;
            }
            stream.bindings = stream_bindings(&stream.params, &binding);
            stream.entry_binding = binding;
        }
        Ok(codec)
    }

    /// Whether the module has anything to write: a payload root that gets a
    /// codec, local or foreign (an interface table over foreign scalars is
    /// one), or a supported stream call. A package with neither gets no file;
    /// `ridl.std` is written only when another codec module imports it.
    fn has_codec_payload(&self) -> bool {
        self.has_local_payload()
            || !self.foreign_payloads.is_empty()
            || !self.stream_calls.is_empty()
    }

    /// Whether the package has a payload type of its own: the one case the
    /// module needs `flatc`'s barrel, `<base>.ts` and the builder.
    fn has_local_payload(&self) -> bool {
        !self.payload_types().is_empty()
    }

    /// Whether a declared interface has a signal, which gets a check in
    /// `signalChecks`.
    fn has_signals(&self) -> bool {
        self.model.interfaces.iter().any(|interface| {
            matches!(
                interface.identity,
                Some(v1::interface::Identity::Declared(_))
            ) && interface.slots.iter().any(|slot| {
                matches!(
                    &slot.occupant,
                    Some(v1::interaction_slot::Occupant::Interaction(interaction))
                        if interaction.kind() == v1::Kind::Signal
                )
            })
        })
    }

    fn has_domain_types(&self) -> bool {
        self.needs_domain
            || self.payload_types().iter().any(|payload| {
                payload
                    .declaration
                    .is_some_and(|d| d.visibility() != v1::Visibility::Internal)
            })
    }

    /// Refuses two declarations of the module that spell the same name: a
    /// reserved name of the file (an import alias or an export of the
    /// module) taken by an interface or an induced tuple, and two
    /// `codec<Name>` constants that spell the same. Each name is registered
    /// with the declaration it stands for, and the second one is refused.
    fn check_names(&self) -> Result<BTreeSet<String>, GenerateError> {
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
        if !self.stream_calls.is_empty() {
            for helper in [
                "StreamSession",
                "StreamOptions",
                "CatalogMismatch",
                "CodecError",
                "StreamError",
                "checkStreamSession",
                "encodeStreamRequest",
            ] {
                register(
                    helper.to_string(),
                    format!("runtime stream helper {helper}"),
                )?;
            }
            if self.stream_calls.values().any(|s| s.reply.is_some()) {
                register(
                    "decodeStreamReply".to_string(),
                    "stream reply helper".to_string(),
                )?;
            }
            for (upload, form, helper) in [
                (false, "elements", "decodeStream"),
                (false, "text", "decodeTextStream"),
                (true, "elements", "encodeUpload"),
                (true, "bytes", "byteUpload"),
                (true, "text", "textUpload"),
            ] {
                if self
                    .stream_calls
                    .values()
                    .any(|stream| stream.uses_adapter(upload, form))
                {
                    register(
                        helper.to_string(),
                        format!("runtime stream helper {helper}"),
                    )?;
                }
            }
        }
        if self.has_signals() {
            let mut helpers = vec![
                "signalChecks",
                "signalCheck",
                "Codec",
                "SignalCheckEntry",
                "CodecError",
            ];
            if !self.stream_calls.is_empty() {
                // The stream helpers already registered `CodecError`.
                helpers.pop();
            }
            for helper in helpers {
                register(
                    helper.to_string(),
                    format!("runtime signal helper {helper}"),
                )?;
            }
        }
        if self.needs_rule_error() {
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
        for index in &self.reached_strict {
            let name = declared_name(&self.model.declarations[*index]);
            register(format!("unpack{name}"), name.to_string())?;
        }
        // The checking functions of the enum sets of other packages are named
        // from the package alias and the set name, so two sets can spell one
        // function.
        for (package, declaration) in self.foreign_open_sets.values() {
            let name = declared_name(declaration);
            register(foreign_set_pack(package, name), format!("{package}.{name}"))?;
        }
        for (package, declaration) in self.foreign_strict_sets.values() {
            let name = declared_name(declaration);
            register(
                foreign_set_unpack(package, name),
                format!("{package}.{name}"),
            )?;
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
                register(
                    format!("{}.iface", spelled.declared),
                    format!("{} interface-table metadata key `iface`", spelled.declared),
                )?;
                for slot in &interface.slots {
                    if let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                        &slot.occupant
                    {
                        let member = interaction
                            .name
                            .as_ref()
                            .map_or("", |n| n.declared.as_str());
                        let path = format!("{}.{member}", spelled.declared);
                        register(path.clone(), path)?;
                    }
                    if let Some(stream) = self.stream_calls.get(&(interface.number, slot.ordinal)) {
                        let member = stream
                            .interaction
                            .name
                            .as_ref()
                            .map_or("", |n| n.declared.as_str());
                        register(
                            format!("{}_{member}", spelled.declared),
                            format!("{}.{member}", spelled.declared),
                        )?;
                    }
                }
            }
        }
        Ok(names.into_keys().collect())
    }

    /// Whether a converter of the module throws a `RuleError`: some enum or
    /// enum set is reached.
    fn needs_rule_error(&self) -> bool {
        !self.reached_open.is_empty()
            || !self.foreign_open_sets.is_empty()
            || !self.reached_strict.is_empty()
            || !self.foreign_strict_sets.is_empty()
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
                } else {
                    self.reached_strict.insert(index);
                }
                Ok(())
            }
            Some(Kind::EnumSet(shape)) => {
                if shape.open {
                    self.reached_open.insert(index);
                } else {
                    self.reached_strict.insert(index);
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
                    if let Some(v1::declaration::Kind::EnumSet(shape)) = &declaration.kind {
                        if shape.open {
                            self.foreign_open_sets.insert(
                                format!("{package}.{}", declared_name(declaration)),
                                (package, declaration),
                            );
                        } else {
                            self.foreign_strict_sets.insert(
                                format!("{package}.{}", declared_name(declaration)),
                                (package, declaration),
                            );
                        }
                    }
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
                Target::Local(_, declaration) => {
                    self.needs_domain = true;
                    Ok(value_type(declaration))
                }
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
                let (spelled, declaration, nested, home) = match self
                    .resolve(reference, qualified)?
                {
                    Target::Local(_, declaration) => (
                        format!("t.{}", declared_name(declaration)),
                        declaration,
                        true,
                        "",
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
                            package,
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
                    Some(Kind::EnumSet(shape)) if shape.open => Rule {
                        pack: format!("{}({{}})", foreign_set_pack(home, name)),
                        unpack: format!("{{}} as {spelled}"),
                        plain: false,
                        nullable: false,
                        vector: false,
                    },
                    Some(Kind::Enum(_)) => Rule {
                        pack: "BigInt({})".to_string(),
                        unpack: format!("unpack{name}({{}})"),
                        plain: false,
                        nullable: false,
                        vector: false,
                    },
                    Some(Kind::EnumSet(_)) => Rule {
                        pack: "{}".to_string(),
                        unpack: if nested {
                            format!("unpack{name}({{}})")
                        } else {
                            format!("{}({{}})", foreign_set_unpack(home, name))
                        },
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
                    Some(Kind::Scalar(_)) => Rule {
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
            if self.reached_strict.contains(&index) {
                self.emit_strict_converters(&mut body, declaration);
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

        for (package, declaration) in self.foreign_open_sets.values() {
            if let Some(v1::declaration::Kind::EnumSet(shape)) = &declaration.kind {
                let name = declared_name(declaration);
                body.push('\n');
                emit_set_pack(
                    &mut body,
                    &foreign_set_pack(package, name),
                    &format!("{}.{name}", package_alias(package)),
                    &format!("{package}.{name}"),
                    shape,
                );
            }
        }

        for (package, declaration) in self.foreign_strict_sets.values() {
            if let Some(v1::declaration::Kind::EnumSet(shape)) = &declaration.kind {
                let name = declared_name(declaration);
                body.push('\n');
                emit_strict_set_unpack(
                    &mut body,
                    &foreign_set_unpack(package, name),
                    &format!("{}.{name}", package_alias(package)),
                    &format!("{package}.{name}"),
                    shape,
                );
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

        if !self.stream_calls.is_empty() {
            self.emit_stream_helpers(&mut body);
        }
        for interface in &self.model.interfaces {
            let Some(v1::interface::Identity::Declared(spelled)) = &interface.identity else {
                continue;
            };
            self.emit_interface(&mut body, interface, &spelled.declared)?;
        }
        if !self.signal_entries.is_empty() {
            self.emit_signal_checks(&mut body);
        }

        let mut out = String::new();
        // The builder, `<base>.ts` and `flatc`'s barrel are named only by a
        // local payload type's functions and converters; a package whose
        // payload roots are all foreign has no table, so `flatc` writes no
        // barrel for it to import.
        let local = self.has_local_payload();
        if !self.signal_entries.is_empty() {
            out.push_str(
                "import type { Codec, SignalCheckEntry } from \"@driftsys/ridl-runtime\";\n",
            );
        }
        if self.stream_calls.is_empty() && !self.signal_entries.is_empty() {
            out.push_str("import { CodecError } from \"@driftsys/ridl-runtime\";\n");
        }
        if !self.stream_calls.is_empty() {
            out.push_str(
                "import type { StreamSession, StreamOptions } from \"@driftsys/ridl-runtime\";\n",
            );
            let mut helpers = vec!["CatalogMismatch", "CodecError", "StreamError"];
            for (upload, form, helper) in [
                (false, "elements", "decodeStream"),
                (false, "text", "decodeTextStream"),
                (true, "elements", "encodeUpload"),
                (true, "bytes", "byteUpload"),
                (true, "text", "textUpload"),
            ] {
                if self
                    .stream_calls
                    .values()
                    .any(|s| s.uses_adapter(upload, form))
                {
                    helpers.push(helper);
                }
            }
            let _ = writeln!(
                out,
                "import {{ {} }} from \"@driftsys/ridl-runtime\";",
                helpers.join(", ")
            );
        }
        if local {
            out.push_str("import * as flatbuffers from \"flatbuffers\";\n");
        }
        // Import the payload codec type only when a `codec<Name>` constant
        // uses it, so that modules without one have no unused type import.
        if !self.used_locals.is_empty() || !self.foreign_payloads.is_empty() {
            out.push_str("import type { PayloadCodec } from \"@driftsys/ridl-runtime\";\n");
        }
        if self.has_domain_types() {
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
            let (barrel, comment) = flatc_barrel(self.package);
            out.push_str(comment);
            let _ = writeln!(out, "import * as fb from \"./{barrel}.js\";");
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
        if self.needs_rule_error() {
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
            Some(Kind::Enum(_)) => (format!("new fb.{name}BoxT(BigInt(value))"), Read::Strict),
            Some(Kind::EnumSet(_)) => (format!("new fb.{name}BoxT(value)"), Read::Strict),
            Some(Kind::Scalar(scalar)) if scalar.class() == v1::ScalarClass::Bytes => {
                (format!("new fb.{name}BoxT(Array.from(value))"), Read::Bytes)
            }
            Some(Kind::Scalar(_)) => (format!("new fb.{name}BoxT(value)"), Read::Assertion),
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
            Read::Strict => {
                let _ = writeln!(out, "  return unpack{name}({read});");
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

    /// The unpack function of a strict enum or a strict enum set. It refuses a
    /// value the declaration does not name with the `variant` rule, as every
    /// Rust codec does. An enum with no zero member has no default value, so a
    /// value the buffer leaves out is reported as absent, as a field is.
    fn emit_strict_converters(&mut self, out: &mut String, declaration: &v1::Declaration) {
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
                // `flatc` holds the value as `T | null` when the enum has no
                // zero member.
                let nullable = !shape.values.iter().any(|value| value.value == 0);
                let (parameter, read) = if nullable {
                    self.needs_absent = true;
                    (
                        "number | bigint | null",
                        format!("raw === null ? absent(\"{qualified}\", \"value\") : Number(raw)"),
                    )
                } else {
                    ("number | bigint", "Number(raw)".to_string())
                };
                let _ = writeln!(
                    out,
                    "function unpack{name}(raw: {parameter}): t.{name} {{\n  \
                     const n = {read};\n  \
                     if (!([{declared}] as readonly number[]).includes(n)) {{\n    \
                     throw new RuleError(\"variant\", `value ${{raw}} is not a value of \
                     {qualified}`);\n  }}\n  \
                     return n as t.{name};\n}}",
                    declared = declared.join(", ")
                );
            }
            Some(Kind::EnumSet(shape)) => {
                emit_strict_set_unpack(
                    out,
                    &format!("unpack{name}"),
                    &format!("t.{name}"),
                    &qualified,
                    shape,
                );
            }
            _ => {}
        }
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
                emit_set_pack(
                    out,
                    &format!("pack{name}"),
                    &format!("t.{name}"),
                    &qualified,
                    shape,
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

    /// Validates stream numbering and shape before selecting supported calls.
    fn discover_stream(
        &mut self,
        interface: &'a v1::Interface,
        slot: &'a v1::InteractionSlot,
        interaction: &'a v1::Interaction,
        name: &str,
    ) -> Result<(), GenerateError> {
        let member = interaction
            .name
            .as_ref()
            .map_or("", |n| n.declared.as_str());
        let path = format!("{name}.{member}");
        let (params, reply) = match &interaction.shape {
            Some(v1::interaction::Shape::Query(q)) if interaction.kind() == v1::Kind::Query => (
                &q.params,
                matches!(q.reply.as_ref().and_then(|r| r.kind.as_ref()),
                    Some(v1::reply::Kind::Value(ty)) if matches!(ty.kind, Some(v1::r#type::Kind::Stream(_)))),
            ),
            Some(v1::interaction::Shape::Command(c)) if interaction.kind() == v1::Kind::Command => {
                (&c.params, false)
            }
            _ => {
                return refuse(format!(
                    "malformed codegen model: `{path}` has streams on a non-call shape"
                ));
            }
        };
        let uploads = params
            .iter()
            .filter(|p| {
                p.r#type
                    .as_ref()
                    .is_some_and(|ty| matches!(ty.kind, Some(v1::r#type::Kind::Stream(_))))
            })
            .count();
        if interaction.streams.len() != uploads + usize::from(reply) {
            return refuse(format!(
                "malformed codegen model: `{path}` has inconsistent stream metadata"
            ));
        }
        for (index, stream) in interaction.streams.iter().enumerate() {
            let expected = index as u32 + u32::from(!reply);
            let elements = stream.form() == v1::StreamForm::Elements;
            if stream.no != expected
                || stream.form() == v1::StreamForm::Unspecified
                || elements != stream.element.is_some()
            {
                return refuse(format!(
                    "malformed codegen model: `{path}` has invalid stream number, form or element"
                ));
            }
        }
        let key = (interface.number, slot.ordinal);
        let omission = if uploads > 1 {
            Some("multiple parameter streams are not supported")
        } else {
            None
        };
        if let Some(reason) = omission {
            self.stream_omissions.insert(key, reason.to_string());
            return Ok(());
        }
        let (request_payload, request_tuple, query) = match &interaction.shape {
            Some(v1::interaction::Shape::Query(q)) => {
                (q.request.as_ref(), q.request_tuple.as_ref(), Some(q))
            }
            Some(v1::interaction::Shape::Command(c)) => {
                (c.request.as_ref(), c.request_tuple.as_ref(), None)
            }
            _ => unreachable!("validated call"),
        };
        if let Some(q) = query
            && matches!(
                q.reply.as_ref().and_then(|r| r.kind.as_ref()),
                Some(v1::reply::Kind::Fallible(_))
            )
            && q.reply_payload.is_none()
            && q.reply_tuple.is_none()
        {
            self.stream_omissions
                .insert(key, "inline fallible replies have no codec".to_string());
            return Ok(());
        }
        let duplex = uploads == 1 && reply;
        let shape = &interaction.streams[usize::from(duplex)];
        let previous_locals = self.used_locals.clone();
        let previous_payloads = self.foreign_payloads.clone();
        let previous_types = self.foreign_types.clone();
        let previous_domain = self.needs_domain;
        let request = self.payload(request_payload, request_tuple, &path)?;
        let result = if uploads == 1 && !duplex {
            query
                .map(|q| self.payload(q.reply_payload.as_ref(), q.reply_tuple.as_ref(), &path))
                .transpose()?
        } else {
            None
        };
        let (form, element, element_type) = self.stream_element(shape, &path)?;
        let duplex_reply = if duplex {
            Some(self.stream_element(&interaction.streams[0], &path)?)
        } else {
            None
        };
        let missing = request
            .as_ref()
            .err()
            .or_else(|| element.as_ref().and_then(|e| e.as_ref().err()))
            .or_else(|| result.as_ref().and_then(|r| r.as_ref().err()))
            .or_else(|| {
                duplex_reply
                    .as_ref()
                    .and_then(|(_, e, _)| e.as_ref().and_then(|e| e.as_ref().err()))
            });
        if let Some(missing) = missing {
            self.stream_omissions
                .insert(key, format!("{missing} has no finite FlatBuffers bound"));
            self.used_locals = previous_locals;
            self.foreign_payloads = previous_payloads;
            self.foreign_types = previous_types;
            self.needs_domain = previous_domain;
            return Ok(());
        }
        let params: Vec<_> = params
            .iter()
            .filter(|p| {
                !matches!(
                    p.r#type.as_ref().and_then(|t| t.kind.as_ref()),
                    Some(v1::r#type::Kind::Stream(_))
                )
            })
            .collect();
        let mut param_types = Vec::new();
        for param in &params {
            let Some(ty) = &param.r#type else {
                return refuse(format!(
                    "malformed codegen model: `{path}` has a parameter without a type"
                ));
            };
            let mut spelled = self.ts_type(ty, &path)?;
            if ty.optional {
                spelled.push_str(" | undefined");
            }
            param_types.push(spelled);
        }
        // Every payload result is Ok after the missing-bound branch above.
        // These expects depend on that branch, not on documentation or names.
        let reply = if let (Some(q), Some(result)) = (query, result) {
            let ty = if let Some(tuple) = &q.reply_tuple {
                tuple_name(&self.model.tuples[tuple.index as usize]).to_string()
            } else if let Some(reference) = q.reply_payload.as_ref().and_then(|p| p.r#type.as_ref())
            {
                self.ts_type(
                    &v1::Type {
                        kind: Some(v1::r#type::Kind::Named(reference.clone())),
                        ..Default::default()
                    },
                    &path,
                )?
            } else {
                unreachable!("resolved reply payload")
            };
            Some((result.expect("bounded reply"), ty))
        } else {
            None
        };
        self.stream_calls.insert(
            key,
            StreamCall {
                interaction,
                params,
                named_request: request_payload.is_some(),
                upload: uploads == 1,
                reply,
                duplex_reply: duplex_reply.map(|(form, element, element_type)| StreamElement {
                    form,
                    element: element.map(|e| e.expect("bounded reply element")),
                    element_type,
                }),
                request: request.expect("bounded request"),
                element: element.map(|e| e.expect("bounded element")),
                element_type,
                form,
                entry_binding: String::new(),
                bindings: Vec::new(),
                param_types,
            },
        );
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn stream_element(
        &mut self,
        shape: &v1::StreamShape,
        path: &str,
    ) -> Result<(&'static str, Option<Result<String, String>>, String), GenerateError> {
        Ok(match shape.form() {
            v1::StreamForm::Elements => {
                // The metadata loop above requires an element for this form.
                let reference = shape.element.as_ref().expect("validated element");
                let max_size = shape
                    .sizes
                    .as_ref()
                    .and_then(|sizes| sizes.flatbuffers.as_ref())
                    .and_then(|state| match state.state {
                        Some(v1::size_state::State::Bounded(n)) => Some(n),
                        _ => None,
                    });
                let payload = v1::Payload {
                    r#type: Some(reference.clone()),
                    flatbuffers_max_size: max_size,
                    sizes: shape.sizes.clone(),
                };
                let element = self.payload(Some(&payload), None, path)?;
                let ty = self.ts_type(
                    &v1::Type {
                        kind: Some(v1::r#type::Kind::Named(reference.clone())),
                        ..Default::default()
                    },
                    path,
                )?;
                ("elements", Some(element), ty)
            }
            v1::StreamForm::Bytes => ("bytes", None, "Uint8Array".to_string()),
            v1::StreamForm::Text => ("text", None, "string".to_string()),
            v1::StreamForm::Unspecified => unreachable!("validated form"),
        })
    }

    fn emit_stream_helpers(&self, out: &mut String) {
        out.push_str("\nfunction checkStreamSession(session: StreamSession, limit: number | undefined): void {\n  const hash = session.codec.catalogHash;\n  if (hash.length !== CATALOG_HASH.length || CATALOG_HASH.some((byte, i) => byte !== hash[i])) {\n    throw new CatalogMismatch(\n      Array.from(CATALOG_HASH, (b) => b.toString(16).padStart(2, \"0\")).join(\"\"),\n      Array.from(hash, (b) => b.toString(16).padStart(2, \"0\")).join(\"\"),\n    );\n  }\n  if (limit === 0) throw new StreamError({ category: \"corrupt\" });\n}\n");
        out.push_str("\nfunction encodeStreamRequest<T>(session: StreamSession, payload: PayloadCodec<T>, typeId: number, value: T, limit: number | undefined): Uint8Array {\n  let bytes: Uint8Array;\n  try {\n    bytes = payload.encode(value);\n  } catch (error) {\n");
        out.push_str("    if (typeof error === \"object\" && error !== null && \"name\" in error && error.name === \"RuleError\" && \"rule\" in error && (RULES as readonly unknown[]).includes(error.rule)) {\n      throw new StreamError({ category: \"contract\", contract: { category: \"invalid-value\", rule: error.rule as typeof RULES[number] } }, { cause: error });\n    }\n");
        out.push_str("    throw new StreamError({ category: \"corrupt\" }, { cause: error });\n  }\n  if (limit !== undefined && bytes.length > limit) throw new StreamError({ category: \"corrupt\" });\n  const result = session.codec.verify(typeId, bytes);\n  if (!result.ok) {\n    if (result.kind === \"malformed\") throw new StreamError({ category: \"corrupt\" });\n    if (result.kind === \"rule\" && (RULES as readonly string[]).includes(result.rule ?? \"\")) {\n      throw new StreamError({ category: \"contract\", contract: { category: \"invalid-value\", rule: result.rule as typeof RULES[number] } });\n    }\n    throw new CodecError(\"unknown payload type or rule during request verification\");\n  }\n  return bytes;\n}\n");
        if self.stream_calls.values().any(|s| s.reply.is_some()) {
            out.push_str("\nfunction decodeStreamReply<T>(session: StreamSession, payload: PayloadCodec<T>, typeId: number, bytes: Uint8Array): T {\n  const result = session.codec.verify(typeId, bytes);\n  if (!result.ok) {\n    if (result.kind === \"malformed\") throw new StreamError({ category: \"corrupt\" });\n    if (result.kind === \"rule\" && (RULES as readonly string[]).includes(result.rule ?? \"\")) {\n      throw new StreamError({ category: \"contract\", contract: { category: \"invalid-value\", rule: result.rule as typeof RULES[number] } });\n    }\n    throw new CodecError(\"unknown payload type or rule during reply verification\");\n  }\n  try {\n    return payload.decode(bytes);\n  } catch (error) {\n    if (typeof error === \"object\" && error !== null && \"name\" in error && error.name === \"RuleError\" && \"rule\" in error && (RULES as readonly unknown[]).includes(error.rule)) {\n      throw new StreamError({ category: \"contract\", contract: { category: \"invalid-value\", rule: error.rule as typeof RULES[number] } }, { cause: error });\n    }\n    throw new StreamError({ category: \"corrupt\" }, { cause: error });\n  }\n}\n");
        }
    }

    fn emit_stream_call(
        &self,
        out: &mut String,
        name: &str,
        member: &str,
        stream: &StreamCall<'a>,
    ) {
        let params = stream
            .bindings
            .iter()
            .zip(&stream.param_types)
            .map(|(binding, ty)| format!("{binding}: {ty}"));
        let mut signature = vec!["session: StreamSession".to_string()];
        signature.extend(params);
        signature.push("opts: StreamOptions".to_string());
        out.push('\n');
        let entry_binding = &stream.entry_binding;
        let _ = writeln!(out, "const {entry_binding} = {name}.{member};\n");
        out.push_str(&self.stream_docs(stream));
        let returns = if stream.upload {
            let result = if let Some(reply) = &stream.duplex_reply {
                format!("stream: ReadableStream<{}>", reply.element_type)
            } else {
                stream.reply.as_ref().map_or_else(
                    || "accepted: Promise<void>".to_string(),
                    |(_, ty)| format!("reply: Promise<{ty}>"),
                )
            };
            format!(
                "{{ upload: WritableStream<{}>; {result} }}",
                stream.element_type
            )
        } else {
            format!("ReadableStream<{}>", stream.element_type)
        };
        let _ = writeln!(
            out,
            "export function {name}_{member}({}): {returns} {{",
            signature.join(", ")
        );
        let _ = writeln!(out, "  const entry = {entry_binding};");
        let limit = if stream.interaction.limit.is_some() {
            "entry.limit"
        } else {
            "undefined"
        };
        let _ = writeln!(out, "  checkStreamSession(session, {limit});");
        out.push_str("  const requestType = session.codec.typeId(entry.request.type);\n");
        if stream.element.is_some() {
            let index = usize::from(stream.duplex_reply.is_some());
            let _ = writeln!(
                out,
                "  const element = entry.streams[{index}].element;\n  const elementType = session.codec.typeId(element.type);"
            );
        }
        if stream.reply.is_some() {
            out.push_str("  const replyType = session.codec.typeId(entry.reply.type);\n");
        }
        if stream
            .duplex_reply
            .as_ref()
            .is_some_and(|reply| reply.element.is_some())
        {
            out.push_str("  const replyElement = entry.streams[0].element;\n  const replyElementType = session.codec.typeId(replyElement.type);\n");
        }
        let value = if stream.named_request {
            stream.bindings[0].clone()
        } else {
            let fields = stream
                .params
                .iter()
                .zip(&stream.bindings)
                .map(|(p, b)| {
                    format!(
                        "{}: {b}",
                        p.name.as_ref().map_or("", |n| n.declared.as_str())
                    )
                })
                .collect::<Vec<_>>();
            format!("{{ {} }}", fields.join(", "))
        };
        let _ = writeln!(
            out,
            "  const args = encodeStreamRequest(session, entry.request, requestType, {value}, {limit});"
        );
        if stream.upload {
            let method = if stream.duplex_reply.is_some() {
                "queryDuplex"
            } else if stream.reply.is_some() {
                "queryWithUpload"
            } else {
                "commandStream"
            };
            let _ = writeln!(
                out,
                "  const raw = session.transport.{method}(entry.iface, entry.ord, args, opts);"
            );
            let upload_limit = if stream.interaction.limit.is_some() {
                "entry.limit"
            } else {
                "16128"
            };
            let adapter = match stream.form {
                "elements" => format!(
                    "encodeUpload(raw.upload, (value: {}) => element.encode(value), (bytes) => session.codec.verify(elementType, bytes), {upload_limit})",
                    stream.element_type
                ),
                "text" => format!("textUpload(raw.upload, {upload_limit})"),
                _ => format!("byteUpload(raw.upload, {upload_limit})"),
            };
            let _ = writeln!(out, "  const upload = {adapter};");
            if let Some(reply) = &stream.duplex_reply {
                match reply.form {
                    "elements" => out.push_str("  const stream = raw.stream.pipeThrough(decodeStream((bytes) => replyElement.decode(bytes), (bytes) => session.codec.verify(replyElementType, bytes)));\n"),
                    "text" => out.push_str("  const stream = raw.stream.pipeThrough(decodeTextStream());\n"),
                    _ => out.push_str("  const stream = raw.stream;\n"),
                }
                out.push_str("  return { upload, stream };\n}\n");
            } else if stream.reply.is_some() {
                out.push_str("  const reply = raw.reply.then((bytes) => decodeStreamReply(session, entry.reply, replyType, bytes));\n  return { upload, reply };\n}\n");
            } else {
                out.push_str("  return { upload, accepted: raw.accepted };\n}\n");
            }
            return;
        }
        out.push_str(
            "  const raw = session.transport.queryStream(entry.iface, entry.ord, args, opts);\n",
        );
        match stream.form {
            "elements" => out.push_str("  return raw.pipeThrough(decodeStream((bytes) => element.decode(bytes), (bytes) => session.codec.verify(elementType, bytes)));\n"),
            "text" => out.push_str("  return raw.pipeThrough(decodeTextStream());\n"),
            _ => out.push_str("  return raw;\n"),
        }
        out.push_str("}\n");
    }

    /// Reuses the domain renderer for canonical rules, links and doc tags.
    fn stream_docs(&self, stream: &StreamCall<'a>) -> String {
        let package = ridl_ir::v2::Package {
            name: self.package.to_string(),
            ..Default::default()
        };
        let output = crate::Output {
            imports: self.foreign_types.clone(),
            emitted: self
                .model
                .declarations
                .iter()
                .filter(|d| {
                    self.has_domain_types()
                        && d.visibility() != v1::Visibility::Internal
                        && crate::foreign_module_emits(self.model, self.package, d)
                })
                .map(|d| declared_name(d).to_string())
                .collect(),
        };
        let mut ctx = crate::Ctx::new(&package, self.model, output);
        // Qualify only links created from model targets, never literal doc text.
        ctx.local_doc_namespace = "t.";
        let i = stream.interaction;
        let parts = crate::TsDocParts {
            doc: &i.doc,
            links: &i.links,
            see: &i.see,
            since: &i.since,
            rules: &i.rules,
            deprecated: i.deprecated.as_deref(),
        };
        let mut tags = Vec::new();
        for (param, binding) in stream.params.iter().zip(&stream.bindings) {
            let doc = crate::linked(&ctx, &param.doc, &param.links);
            if !doc.is_empty() {
                tags.push(format!("@param {binding} - {doc}"));
            }
        }
        crate::jsdoc(&ctx, "", &parts, &tags)
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
            if !interaction.streams.is_empty() {
                let key = (interface.number, slot.ordinal);
                if let Some(stream) = self.stream_calls.get(&key) {
                    let element = stream
                        .element
                        .as_ref()
                        .map(|e| format!(", element: {e}"))
                        .unwrap_or_default();
                    let limit = interaction
                        .limit
                        .map(|n| format!(", limit: {n}"))
                        .unwrap_or_default();
                    let kind = if interaction.kind() == v1::Kind::Command {
                        "command"
                    } else {
                        "query"
                    };
                    let no = u32::from(stream.upload);
                    let mut streams = format!("{{ no: {no}, form: \"{}\"{element} }}", stream.form);
                    if let Some(reply) = &stream.duplex_reply {
                        let element = reply
                            .element
                            .as_ref()
                            .map(|e| format!(", element: {e}"))
                            .unwrap_or_default();
                        streams =
                            format!("{{ no: 0, form: \"{}\"{element} }}, {streams}", reply.form);
                    }
                    let reply = stream
                        .reply
                        .as_ref()
                        .map(|(codec, _)| format!(", reply: {codec}"))
                        .unwrap_or_default();
                    let _ = writeln!(
                        out,
                        "  {member}: {{ iface: {number}, ord: {}, kind: \"{kind}\", request: {}{reply}, streams: [{streams}]{limit} }},",
                        slot.ordinal, stream.request
                    );
                } else if let Some(reason) = self.stream_omissions.get(&key) {
                    let _ = writeln!(out, "  // {member} has no entry: {reason}");
                }
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
            if interaction.kind() == v1::Kind::Signal {
                self.signal_entries.push(format!("{name}.{member}"));
            }
        }
        out.push_str("} as const;\n");
        for slot in &interface.slots {
            if let Some(stream) = self.stream_calls.get(&(number, slot.ordinal)) {
                let member = stream
                    .interaction
                    .name
                    .as_ref()
                    .map_or("", |n| n.declared.as_str());
                self.emit_stream_call(out, name, member, stream);
            }
        }
        Ok(())
    }

    /// Writes `signalChecks`, which gives `connect` one check per signal that
    /// has a member table entry.
    fn emit_signal_checks(&self, out: &mut String) {
        out.push_str(
            "\nfunction signalCheck(codec: Codec, entry: { readonly iface: number; readonly ord: number; readonly payload: { readonly type: string } }): SignalCheckEntry {\n  const typeId = codec.typeId(entry.payload.type);\n  return {\n    iface: entry.iface,\n    ord: entry.ord,\n    check: (bytes) => {\n      const result = codec.verify(typeId, bytes);\n      if (result.ok) return undefined;\n      if (result.kind !== \"rule\") return \"corrupt\";\n      const rule = (RULES as readonly string[]).find((known) => known === result.rule);\n      if (rule === undefined) throw new CodecError(\"unknown rule during signal verification\");\n      return { rule: rule as (typeof RULES)[number] };\n    },\n  };\n}\n",
        );
        out.push_str(
            "\n/** Returns the check of each signal of this package, for the `checks` option of `connect`. The checks resolve the type ids of `codec` once, here. */\nexport function signalChecks(codec: Codec): readonly SignalCheckEntry[] {\n  return [\n",
        );
        for entry in &self.signal_entries {
            let _ = writeln!(out, "    signalCheck(codec, {entry}),");
        }
        out.push_str("  ];\n}\n");
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
        named: Option<&v1::Payload>,
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

/// Allocates local bindings without shadowing generated locals or helper imports.
fn stream_bindings(params: &[&v1::Param], entry_binding: &str) -> Vec<String> {
    let reserved = [
        "session",
        "opts",
        "entry",
        "element",
        "requestType",
        "elementType",
        "replyType",
        "replyElement",
        "replyElementType",
        "stream",
        "upload",
        "reply",
        "encodeUpload",
        "byteUpload",
        "textUpload",
        "decodeStreamReply",
        "args",
        "raw",
        "bytes",
        "checkStreamSession",
        "encodeStreamRequest",
        "decodeStream",
        "decodeTextStream",
        "undefined",
        "t",
        "await",
        "yield",
        "arguments",
        "eval",
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
    ];
    let mut occupied: HashSet<String> = reserved.into_iter().map(str::to_string).collect();
    occupied.insert(entry_binding.to_string());
    let declared: HashSet<&str> = params
        .iter()
        .filter_map(|p| p.name.as_ref())
        .map(|n| n.declared.as_str())
        .collect();
    params
        .iter()
        .map(|param| {
            let name = param.name.as_ref().map_or("", |n| n.declared.as_str());
            let mut binding = name.to_string();
            let mut suffix = 1;
            if occupied.contains(&binding) {
                loop {
                    binding = format!("{name}{suffix}");
                    suffix += 1;
                    if !declared.contains(binding.as_str()) && !occupied.contains(&binding) {
                        break;
                    }
                }
            }
            occupied.insert(binding.clone());
            binding
        })
        .collect()
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
/// converter for it; `None` for a named scalar, which converts by assertion,
/// and for an enum set, which converts by a checking function.
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

/// The function `function_name` that refuses a value of the open enum set
/// `shape`, written `ts_type` in the module, that does not fit the width of
/// the set, with the `range` rule.
fn emit_set_pack(
    out: &mut String,
    function_name: &str,
    ts_type: &str,
    qualified: &str,
    shape: &v1::EnumSet,
) {
    let (wide, signed, bits) = set_width(shape);
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
        "function {function_name}(value: {ts_type}): {base} {{\n  \
         if ({check}value < {low}{suffix} || value > {high}{suffix}) {{\n    \
         throw new RuleError(\"range\", `value ${{value}} of {qualified} does not fit \
         {bits} bits`);\n  }}\n  \
         return value;\n}}"
    );
}

/// Whether the width of `shape` is held as a `bigint`, whether it is signed,
/// and its number of bits.
fn set_width(shape: &v1::EnumSet) -> (bool, bool, u32) {
    match shape.width() {
        v1::IntWidth::U8 => (false, false, 8),
        v1::IntWidth::I8 => (false, true, 8),
        v1::IntWidth::U16 => (false, false, 16),
        v1::IntWidth::I16 => (false, true, 16),
        v1::IntWidth::U32 => (false, false, 32),
        v1::IntWidth::I32 => (false, true, 32),
        v1::IntWidth::U64 => (true, false, 64),
        v1::IntWidth::I64 | v1::IntWidth::Unspecified => (true, true, 64),
    }
}

/// The function `function_name` that returns a value of the strict enum set
/// `shape`, written `ts_type` in the module, or throws a `RuleError` with the
/// `variant` rule when the value has a bit that the set does not declare.
fn emit_strict_set_unpack(
    out: &mut String,
    function_name: &str,
    ts_type: &str,
    qualified: &str,
    shape: &v1::EnumSet,
) {
    let (wide, _, _) = set_width(shape);
    // The declared mask holds the bits 0 to 63 in a signed 64-bit field.
    let mask = shape.declared_mask as u64;
    let (base, undeclared) = if wide {
        (
            "bigint",
            format!("(BigInt.asUintN(64, value) & ~{mask}n) !== 0n"),
        )
    } else {
        ("number", format!("(value & ~{mask}) !== 0"))
    };
    let _ = writeln!(
        out,
        "function {function_name}(value: {base}): {ts_type} {{\n  \
         if ({undeclared}) {{\n    \
         throw new RuleError(\"variant\", `value ${{value}} has a bit that {qualified} does \
         not declare`);\n  }}\n  \
         return value as {ts_type};\n}}"
    );
}

/// The name of the function that checks a value of the strict enum set `name`
/// of `package` against the declared bits of the set.
fn foreign_set_unpack(package: &str, name: &str) -> String {
    format!("unpack_{}_{name}", package_alias(package))
}

/// The name of the function that checks a value of the open enum set `name`
/// of `package` against the width of the set.
fn foreign_set_pack(package: &str, name: &str) -> String {
    format!("pack_{}_{name}", package_alias(package))
}

/// How the decode function of a boxed payload type reads the value of its box.
enum Read {
    /// The unpack function of a strict enum or enum set.
    Strict,
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
