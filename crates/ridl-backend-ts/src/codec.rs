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
//! A package with a signal that has a member table entry also gets
//! `signalChecks(codec)`, which returns one `SignalCheckEntry` of the runtime
//! package per such signal, for the `checks` option of a session. The entry's check refuses a payload that the wasm codec rejects.
//! Each non-internal interface with a command, a query, a signal, an event or a
//! supported stream member also gets `<Interface>Client`, a class over the
//! runtime ports that its members need (`Caller`, `SignalReader`,
//! `EventSource`, `StreamCaller`): `bind` checks the
//! catalog hash of the port and of the codec, resolves each payload type id
//! once and registers the check of each signal with the port. One method per
//! command and query verifies the request before it is sent and the reply
//! before it is decoded. A signal gets `subscribeX`, `x`, `readX` and `watchX`,
//! whose samples carry the decoded value, and an event gets `subscribeX`; one
//! `onEvent` delivers the occurrences as the union `<Interface>Event`. A stream
//! query or command is a method too: it verifies the request before opening the
//! stream, verifies named elements before decoding, decodes raw text as UTF-8
//! and preserves raw byte streams. Uploads encode and verify whole named
//! elements, and split raw bytes or text at the member limit. A query whose
//! reply is an inline `T | E` has no method, and neither has a signal with an
//! init override or without an init function of its payload type. A stream
//! member has no method with two or more parameter streams, with an upload
//! whose reply is an inline `T | E`, or when its request, named element or
//! upload reply has no finite FlatBuffers bound.
//!
//! A named scalar converts through the `make<Name>Unchecked` of its package's
//! domain module when the module emits one (the codec has verified the bytes,
//! so the value is not checked twice), and by a type assertion otherwise. A
//! named scalar of another package converts that way too, an enum set
//! of another package by a checking function of the module, and a payload type
//! of another package that is a named scalar or an enum set goes through that
//! package's own codec module. A struct, a union or an enum of another
//! package needs a converter of its own, which the module does not write yet:
//! a payload type that reaches one is refused with a [`GenerateError`] that
//! names both.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use ridl_ir::codegen::v1;
use ridl_ir::v2;

use crate::{GenerateError, GeneratedTs, RUNTIME};

/// The text of the rule codes the wasm module reports, rule code `n` at
/// index `n - 2`: the order of `WASM_RULES` in the generated crate.
const RULES: [&str; 7] = [
    "length", "pattern", "range", "step", "unique", "utf8", "variant",
];

/// The length of `Catalog.hash`.
const HASH_LEN: usize = 32;

/// Generates `<artifact_base>.codec.ts` for the package `model` describes,
/// or `None` for a package with neither a payload root that gets a codec,
/// local or foreign, nor a supported stream call. The returned source
/// starts at the imports; the caller writes the generated marker and the
/// header above it.
///
/// Without the IR package this cannot tell which payload types get an
/// `init<Name>` function in `<base>.ts`, so it takes the `derivable` fact of
/// the model for it, which is looser than the emitter's own rule. Use
/// [`generate_codec_with`] when the package is at hand.
pub fn generate_codec(
    model: &v1::Model,
    artifact_base: &str,
) -> Result<Option<GeneratedTs>, GenerateError> {
    generate_codec_inner(model, artifact_base, None)
}

/// [`generate_codec`] for the package `package` that `model` describes: a
/// signal gets methods only when `<base>.ts` exports the `init<Name>` function
/// of its payload type, which the domain emitter decides from `package`.
pub fn generate_codec_with(
    model: &v1::Model,
    package: &v2::Package,
    artifact_base: &str,
) -> Result<Option<GeneratedTs>, GenerateError> {
    let inits = crate::init_function_names(package, model);
    generate_codec_inner(model, artifact_base, Some(&inits))
}

fn generate_codec_inner(
    model: &v1::Model,
    artifact_base: &str,
    init_functions: Option<&BTreeSet<String>>,
) -> Result<Option<GeneratedTs>, GenerateError> {
    let mut codec = Codec::new(model, init_functions)?;
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
    Ok(Codec::new(model, None)?
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
    /// The conversion is a type assertion or nothing on the pack side: an
    /// array of such elements is copied, not mapped.
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

/// A command or query that the client class of its interface carries as a
/// method, with the types and names that its signature needs.
struct CallMember<'a> {
    interaction: &'a v1::Interaction,
    params: Vec<&'a v1::Param>,
    /// The request is the one named parameter itself, not an induced tuple.
    named_request: bool,
    /// The type the reply decodes to; `None` for a command.
    reply_type: Option<String>,
    param_types: Vec<String>,
    /// The method's parameter names, which shadow no name the method body uses.
    bindings: Vec<String>,
    /// The response bound in milliseconds, as a decimal number, when the
    /// member records one in the range a port accepts.
    bound_ms: Option<String>,
}

/// A signal or an event that the client class of its interface carries, with
/// the types its methods need.
struct FeedMember<'a> {
    interaction: &'a v1::Interaction,
    /// An event; a signal otherwise.
    event: bool,
    /// The type the payload decodes to.
    value_type: String,
    /// The `init<Name>` function of `<base>.ts` that gives the channel init of
    /// a signal; `None` for an event.
    init: Option<String>,
}

/// A stream direction with a resolved payload codec when its elements are named.
struct StreamElement {
    form: &'static str,
    element: Option<String>,
    element_type: String,
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
    /// Whether a converter calls a `make<Name>Unchecked` of this package, so
    /// that `t` is a value import.
    needs_domain_values: bool,
    /// The packages a converter calls a `make<Name>Unchecked` of, so that
    /// their imports are value imports.
    foreign_values: BTreeSet<String>,
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
    /// The commands and queries the client classes carry, by interface number
    /// and ordinal.
    call_members: BTreeMap<(u32, u32), CallMember<'a>>,
    /// The commands, queries, signals and events of a non-internal interface
    /// that the client class leaves out, with the reason.
    member_omissions: BTreeMap<(u32, u32), String>,
    /// The signals and events the client classes carry, by interface number
    /// and ordinal.
    feed_members: BTreeMap<(u32, u32), FeedMember<'a>>,
    /// The declarations of this package that `<base>.ts` gives an
    /// `init<Name>` function, when the caller knows; otherwise the model's
    /// `derivable` fact stands in for it.
    init_functions: Option<BTreeSet<String>>,
    /// The member table entries of the signals that have one, as
    /// `<Interface>.<member>`, in declaration order.
    signal_entries: Vec<String>,
    needs_domain: bool,
}

impl<'a> Codec<'a> {
    fn new(
        model: &'a v1::Model,
        init_functions: Option<&BTreeSet<String>>,
    ) -> Result<Self, GenerateError> {
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
            needs_domain_values: false,
            foreign_values: BTreeSet::new(),
            used_locals: BTreeSet::new(),
            needs_absent: false,
            reached_open: BTreeSet::new(),
            foreign_open_sets: BTreeMap::new(),
            reached_strict: BTreeSet::new(),
            foreign_strict_sets: BTreeMap::new(),
            stream_calls: BTreeMap::new(),
            stream_omissions: BTreeMap::new(),
            call_members: BTreeMap::new(),
            member_omissions: BTreeMap::new(),
            feed_members: BTreeMap::new(),
            init_functions: init_functions.cloned(),
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
                let roles = codec.roles(interaction, &spelled.declared, member)?;
                codec.discover_call(interface, slot, interaction, &spelled.declared, &roles)?;
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
            "t",
            "fb",
            "CATALOG_NAME",
            "CATALOG_HASH",
            "PAYLOAD_TYPES",
            "RULES",
        ] {
            register(reserved.to_string(), reserved.to_string())?;
        }
        if self.has_signals() {
            for helper in ["signalChecks", "signalCheck"] {
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
                let has_client = interface.slots.iter().any(|slot| {
                    let key = (interface.number, slot.ordinal);
                    self.call_members.contains_key(&key)
                        || self.feed_members.contains_key(&key)
                        || self.stream_calls.contains_key(&key)
                });
                if has_client {
                    let class = format!("{}Client", spelled.declared);
                    register(class.clone(), format!("{} client class", spelled.declared))?;
                    // The method names of the class, each with what took it:
                    // `Err` names the member that generates it, `Ok` the
                    // method of the class that the module writes for another
                    // purpose.
                    let mut taken: BTreeMap<String, Result<&str, String>> = BTreeMap::from([
                        ("bind".to_string(), Ok("the static method `bind`")),
                        ("constructor".to_string(), Ok("the constructor")),
                        ("onEvent".to_string(), Ok("the method `onEvent`")),
                    ]);
                    let mut has_event = false;
                    for slot in &interface.slots {
                        let key = (interface.number, slot.ordinal);
                        let (interaction, methods) = if let Some(call) = self.call_members.get(&key)
                        {
                            let member = interaction_member(call.interaction);
                            (call.interaction, vec![member.to_string()])
                        } else if let Some(stream) = self.stream_calls.get(&key) {
                            let member = interaction_member(stream.interaction);
                            (stream.interaction, vec![member.to_string()])
                        } else if let Some(feed) = self.feed_members.get(&key) {
                            let member = interaction_member(feed.interaction);
                            let pascal = pascal_member(member);
                            has_event |= feed.event;
                            let methods = if feed.event {
                                vec![format!("subscribe{pascal}")]
                            } else {
                                vec![
                                    format!("subscribe{pascal}"),
                                    member.to_string(),
                                    format!("read{pascal}"),
                                    format!("watch{pascal}"),
                                ]
                            };
                            (feed.interaction, methods)
                        } else {
                            continue;
                        };
                        let member = interaction_member(interaction);
                        for method in methods {
                            match taken.get(&method) {
                                Some(Ok(reserved)) => {
                                    return refuse(format!(
                                        "`{}.{member}` in package `{}` collides with {reserved} of the \
                                         client class `{class}`, which the codec module also declares",
                                        spelled.declared, self.package
                                    ));
                                }
                                Some(Err(other)) => {
                                    return refuse(format!(
                                        "`{}.{member}` in package `{}` generates the method \
                                         `{method}`, which collides with the method `{method}` that \
                                         `{}.{other}` generates in the client class `{class}`",
                                        spelled.declared, self.package, spelled.declared
                                    ));
                                }
                                None => {
                                    taken.insert(method, Err(member.to_string()));
                                }
                            }
                        }
                    }
                    if has_event {
                        register(
                            format!("{}Event", spelled.declared),
                            format!("{} event union", spelled.declared),
                        )?;
                    }
                }
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
                        pack: "{}".to_string(),
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
                        match self.unchecked_call(nested, home, name, scalar) {
                            Some(call) => Rule {
                                unpack: format!("{call}(Uint8Array.from({{}}))"),
                                ..bytes_rule(&spelled)
                            },
                            None => bytes_rule(&spelled),
                        }
                    }
                    Some(Kind::Scalar(scalar)) => Rule {
                        pack: if scalar.class() == v1::ScalarClass::String {
                            text_pack(qualified)
                        } else {
                            "{}".to_string()
                        },
                        unpack: match self.unchecked_call(nested, home, name, scalar) {
                            Some(call) if scalar.class() == v1::ScalarClass::String => {
                                format!("{call}({{}} as string)")
                            }
                            Some(call) => format!("{call}({{}})"),
                            None => format!("{{}} as {spelled}"),
                        },
                        plain: scalar.class() != v1::ScalarClass::String,
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
                    Ok(v1::PrimitiveType::String) => string_rule(qualified),
                    _ => copy_rule(),
                })
            }
            Some(TypeKind::Inline(scalar)) => Ok(match scalar.class() {
                v1::ScalarClass::Bytes => bytes_rule("Uint8Array"),
                v1::ScalarClass::String => string_rule(qualified),
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
                // The converter of a tuple takes the path of the position that
                // holds it, so that a string inside it reports the payload type
                // and the field path instead of the name of the induced tuple.
                Ok(Rule {
                    pack: format!("pack{name}({{}}, \"{qualified}.{{path}}\")"),
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
                            template
                                .replace("{}", "element")
                                .replace("{path}", "{path}[]")
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
                        key_rule
                            .pack
                            .replace("{}", "key")
                            .replace("{path}", "{path}.key"),
                        value_rule
                            .pack
                            .replace("{}", "value")
                            .replace("{path}", "{path}.value"),
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

    /// The call that builds a value of the named scalar `name` without a check,
    /// when its module emits `make<Name>Unchecked`: the codec has verified the
    /// bytes, so no value is checked twice on the receive path. `home` is the
    /// package of a foreign type, and `local` says the type is of this package.
    /// It records the value import the call needs.
    fn unchecked_call(
        &mut self,
        local: bool,
        home: &str,
        name: &str,
        scalar: &v1::Scalar,
    ) -> Option<String> {
        if !crate::scalar_has_unchecked(scalar) {
            return None;
        }
        let module = if local {
            self.needs_domain_values = true;
            "t".to_string()
        } else {
            self.foreign_values.insert(home.to_string());
            package_alias(home)
        };
        Some(format!("{module}.make{name}Unchecked"))
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
                "const codec{name}: $rt.PayloadCodec<{spelled}> = {{ type: \"{qualified}\", encode: \
                 encode{name}, decode: decode{name} }};",
                name = payload.name,
                qualified = self.qualified(payload.name),
            );
        }
        for (constant, foreign) in &self.foreign_payloads {
            let alias = package_alias(&foreign.package);
            let _ = writeln!(
                body,
                "const {constant}: $rt.PayloadCodec<{alias}.{name}> = {{ type: \"{package}.{name}\", \
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
        if !self.signal_entries.is_empty() {
            self.emit_signal_checks(&mut body);
        }

        let mut out = String::new();
        // The builder, `<base>.ts` and `flatc`'s barrel are named only by a
        // local payload type's functions and converters; a package whose
        // payload roots are all foreign has no table, so `flatc` writes no
        // barrel for it to import.
        let local = self.has_local_payload();
        // One namespace import carries every runtime name, so no declaration
        // of the package can collide with one. A module that uses only types
        // of the runtime imports it as a type.
        let uses_runtime_values = !self.signal_entries.is_empty()
            || !self.stream_calls.is_empty()
            || !self.call_members.is_empty()
            || !self.feed_members.is_empty()
            || body.contains("$rt.checkWellFormed(");
        let uses_payload_codec = !self.used_locals.is_empty() || !self.foreign_payloads.is_empty();
        if uses_runtime_values {
            let _ = writeln!(
                out,
                "import * as {RUNTIME} from \"@driftsys/ridl-runtime\";"
            );
        } else if uses_payload_codec {
            let _ = writeln!(
                out,
                "import type * as {RUNTIME} from \"@driftsys/ridl-runtime\";"
            );
        }
        if local {
            out.push_str("import * as flatbuffers from \"flatbuffers\";\n");
        }
        if self.has_domain_types() {
            let kind = if self.needs_domain_values {
                ""
            } else {
                "type "
            };
            let _ = writeln!(out, "import {kind}* as t from \"./{artifact_base}.js\";");
        }
        for package in &self.foreign_types {
            let kind = if self.foreign_values.contains(package) {
                ""
            } else {
                "type "
            };
            let _ = writeln!(
                out,
                "import {kind}* as {} from \"./{package}.js\";",
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
        let qualified = self.qualified(name);
        self.emit_encode_decode(
            out,
            name,
            name,
            table,
            &qualified,
            &format!(", \"{qualified}\""),
        );
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
        pack_arguments: &str,
    ) {
        out.push('\n');
        let _ = writeln!(out, "/** Encodes a `{qualified}` payload. */");
        let _ = writeln!(
            out,
            "export function encode{name}(value: {spelled}): Uint8Array {{"
        );
        out.push_str("  const builder = new flatbuffers.Builder();\n");
        let _ = writeln!(
            out,
            "  builder.finish(pack{name}(value{pack_arguments}).pack(builder));"
        );
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
        &mut self,
        out: &mut String,
        declaration: &v1::Declaration,
    ) -> Result<(), GenerateError> {
        use v1::declaration::Kind;
        let name = declared_name(declaration);
        let qualified = self.qualified(name);
        let boxed = match &declaration.kind {
            Some(Kind::Struct(_) | Kind::Union(_)) => {
                self.emit_encode_decode(out, name, &format!("t.{name}"), name, &qualified, "");
                return Ok(());
            }
            Some(Kind::Enum(shape)) if shape.open => {
                (format!("new fb.{name}BoxT(pack{name}(value))"), Read::Open)
            }
            Some(Kind::EnumSet(shape)) if shape.open => (
                format!("new fb.{name}BoxT(pack{name}(value))"),
                Read::Assertion,
            ),
            Some(Kind::Enum(_)) => (format!("new fb.{name}BoxT(value)"), Read::Strict),
            Some(Kind::EnumSet(_)) => (format!("new fb.{name}BoxT(value)"), Read::Strict),
            Some(Kind::Scalar(scalar)) if scalar.class() == v1::ScalarClass::Bytes => (
                format!("new fb.{name}BoxT(Array.from(value))"),
                match self.unchecked_call(true, "", name, scalar) {
                    Some(call) => Read::Unchecked(call, "Uint8Array.from({})"),
                    None => Read::Bytes,
                },
            ),
            Some(Kind::Scalar(scalar)) => (
                if scalar.class() == v1::ScalarClass::String {
                    format!("new fb.{name}BoxT($rt.checkWellFormed(value, \"{qualified}\"))")
                } else {
                    format!("new fb.{name}BoxT(value)")
                },
                match self.unchecked_call(true, "", name, scalar) {
                    Some(call) if scalar.class() == v1::ScalarClass::String => {
                        Read::Unchecked(call, "{} as string")
                    }
                    Some(call) => Read::Unchecked(call, "{}"),
                    None => Read::Assertion,
                },
            ),
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
            Read::Unchecked(call, argument) => {
                let _ = writeln!(out, "  return {call}({});", argument.replace("{}", &read));
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
                        "number | null",
                        format!("raw === null ? absent(\"{qualified}\", \"value\") : raw"),
                    )
                } else {
                    ("number", "raw".to_string())
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
    /// outside int32 is refused. The enum set pair refuses a value
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
                    "function pack{name}(value: t.{name}Value): number {{\n  \
                     const raw = typeof value === \"number\" ? value : value.unknown;\n  \
                     if (!Number.isInteger(raw) || raw < -2147483648 || raw > 2147483647) {{\n    \
                     throw new RuleError(\"variant\", `value ${{raw}} of {qualified} is not an \
                     int32`);\n  }}\n  \
                     return raw;\n}}"
                );
                let _ = writeln!(
                    out,
                    "function unpack{name}(raw: number | null): t.{name}Value {{\n  \
                     const n = raw ?? 0;\n  \
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
        // A tuple converter also takes the path of the position that holds the
        // tuple. The pack templates of its fields name the tuple itself, so the
        // prefix of each is replaced by that path.
        let tuple = object == "tuple";
        let at_unused = if fields.iter().any(|f| f.rule.pack.contains("{path}")) {
            ""
        } else {
            "_"
        };
        let at_parameter = if tuple {
            format!(", {at_unused}at: string")
        } else {
            String::new()
        };
        let _ = writeln!(
            out,
            "function pack{name}({unused}value: {spelled}{at_parameter}): fb.{table}T {{"
        );
        let _ = writeln!(out, "  const {object} = new fb.{table}T();");
        for field in fields {
            let mut packed = fill_pack(
                &field.rule.pack,
                &format!("value.{}", field.declared),
                &field.declared,
            );
            if tuple {
                packed = packed.replace(&format!("\"{qualified}."), "at + \".");
            }
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
                        fill_pack(&rule.pack, "value.value", declared)
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
                        fill_pack(&rule.pack, "value.value", declared),
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

    /// Selects the command, query, signal or event that the client class of
    /// `name` carries as methods, or records why it leaves the member out.
    /// `roles` is the result of [`Codec::roles`] for the member. A fixed
    /// member and a member of an internal interface have no method.
    fn discover_call(
        &mut self,
        interface: &'a v1::Interface,
        slot: &'a v1::InteractionSlot,
        interaction: &'a v1::Interaction,
        name: &str,
        roles: &[(&'static str, Result<String, String>)],
    ) -> Result<(), GenerateError> {
        use v1::interaction::Shape;
        if interface.visibility() == v1::Visibility::Internal {
            return Ok(());
        }
        let member = interaction
            .name
            .as_ref()
            .map_or("", |n| n.declared.as_str());
        let path = format!("{name}.{member}");
        let key = (interface.number, slot.ordinal);
        let (params, request_payload, query) = match &interaction.shape {
            Some(Shape::Command(c)) => (Some(&c.params), c.request.as_ref(), None),
            Some(Shape::Query(q)) => (Some(&q.params), q.request.as_ref(), Some(q)),
            Some(Shape::Signal(_) | Shape::Event(_)) => (None, None, None),
            _ => return Ok(()),
        };
        if let Some((_, Err(unbounded))) = roles.iter().find(|(_, payload)| payload.is_err()) {
            self.member_omissions
                .insert(key, format!("{unbounded} has no finite FlatBuffers bound"));
            return Ok(());
        }
        let Some(params) = params else {
            return self.discover_feed(key, interaction, &path);
        };
        if let Some(q) = query
            && matches!(
                q.reply.as_ref().and_then(|r| r.kind.as_ref()),
                Some(v1::reply::Kind::Fallible(_))
            )
            && q.reply_payload.is_none()
            && q.reply_tuple.is_none()
        {
            self.member_omissions
                .insert(key, "inline fallible replies have no codec".to_string());
            return Ok(());
        }
        let params: Vec<&'a v1::Param> = params.iter().collect();
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
        let reply_type = match query {
            Some(q) => Some(if let Some(tuple) = &q.reply_tuple {
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
                return refuse(format!(
                    "malformed codegen model: `{path}` has a reply with no payload"
                ));
            }),
            None => None,
        };
        let bindings = stream_bindings(&params, name);
        let bound_ms = interaction
            .timing
            .as_ref()
            .and_then(|timing| timing.max_us.as_deref())
            .and_then(milliseconds);
        self.call_members.insert(
            key,
            CallMember {
                interaction,
                params,
                named_request: request_payload.is_some(),
                reply_type,
                param_types,
                bindings,
                bound_ms,
            },
        );
        Ok(())
    }

    /// Selects the signal or event that the client class carries as methods,
    /// or records why it leaves the member out. A signal needs the init
    /// function of its payload type in `<base>.ts`, because the sample of a
    /// signal that has no value yet carries the init value.
    fn discover_feed(
        &mut self,
        key: (u32, u32),
        interaction: &'a v1::Interaction,
        path: &str,
    ) -> Result<(), GenerateError> {
        use v1::interaction::Shape;
        let (payload, signal) = match &interaction.shape {
            Some(Shape::Signal(s)) => (s.payload.as_ref(), Some(s)),
            Some(Shape::Event(e)) => (e.payload.as_ref(), None),
            _ => return Ok(()),
        };
        let Some(reference) = payload.and_then(|p| p.r#type.as_ref()) else {
            return refuse(format!(
                "malformed codegen model: `{path}` has a payload with no type"
            ));
        };
        let init = match signal {
            None => None,
            Some(signal) => {
                if signal.declared_init.is_some() {
                    self.member_omissions.insert(
                        key,
                        "its `= value` init override has no TypeScript form yet".to_string(),
                    );
                    return Ok(());
                }
                match self.resolve(reference, path)? {
                    Target::Local(_, declaration)
                        if match &self.init_functions {
                            Some(names) => names.contains(declared_name(declaration)),
                            None => declaration.init.as_ref().is_some_and(|init| init.derivable),
                        } =>
                    {
                        Some(format!("t.init{}", declared_name(declaration)))
                    }
                    Target::Local(..) => {
                        self.member_omissions
                            .insert(key, "its payload type has no init function".to_string());
                        return Ok(());
                    }
                    Target::Foreign(..) => {
                        self.member_omissions.insert(
                            key,
                            "its payload type is declared in another package, whose init function this module does not import"
                                .to_string(),
                        );
                        return Ok(());
                    }
                }
            }
        };
        let value_type = self.ts_type(
            &v1::Type {
                kind: Some(v1::r#type::Kind::Named(reference.clone())),
                ..Default::default()
            },
            path,
        )?;
        if init.is_some() {
            self.needs_domain_values = true;
        }
        self.feed_members.insert(
            key,
            FeedMember {
                interaction,
                event: signal.is_none(),
                value_type,
                init,
            },
        );
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

    /// Writes the method of a stream query or command in the client class of
    /// its interface. The method verifies the request before it opens the
    /// stream, verifies each named element and plain reply before it decodes
    /// it, decodes raw text as UTF-8 and passes raw bytes through. An upload
    /// encodes and verifies whole named elements, and splits raw bytes or text
    /// at the member limit. The bound is always the caller's: a stream has no
    /// default, because the declared maximum response time does not describe
    /// the whole stream.
    fn emit_stream_method(&self, out: &mut String, member: &str, stream: &StreamCall<'a>) {
        let params = stream
            .bindings
            .iter()
            .zip(&stream.param_types)
            .map(|(binding, ty)| format!("{binding}: {ty}"));
        let mut signature: Vec<String> = params.collect();
        signature.push("opts: Omit<$rt.CallOptions, \"signal\">".to_string());
        let extra = vec![
            "@param opts - `opts.bound` is the response bound in milliseconds. A stream method does not read `opts.signal`: cancel the returned stream, or abort the upload, to end the call."
                .to_string(),
        ];
        out.push('\n');
        out.push_str(&self.call_docs(
            stream.interaction,
            &stream.params,
            &stream.bindings,
            "  ",
            &extra,
        ));
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
        let _ = writeln!(out, "  {member}({}): {returns} {{", signature.join(", "));
        let entry_binding = &stream.entry_binding;
        let _ = writeln!(out, "    const entry = {entry_binding};");
        let ids = format!("this.#ids.{member}");
        let limit = if stream.interaction.limit.is_some() {
            "entry.limit"
        } else {
            "undefined"
        };
        if stream.element.is_some() {
            let index = usize::from(stream.duplex_reply.is_some());
            let _ = writeln!(
                out,
                "    const element = entry.streams[{index}].element;\n    const elementType = {ids}.element;"
            );
        }
        if stream
            .duplex_reply
            .as_ref()
            .is_some_and(|reply| reply.element.is_some())
        {
            let _ = writeln!(
                out,
                "    const replyElement = entry.streams[0].element;\n    const replyElementType = {ids}.replyElement;"
            );
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
            if fields.is_empty() {
                "{}".to_string()
            } else {
                format!("{{ {} }}", fields.join(", "))
            }
        };
        let _ = writeln!(
            out,
            "    const args = $rt.encodeRequest(this.#codec, entry.request, {ids}.request, {value}, {limit});"
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
                "    const raw = this.#port.{method}(entry.iface, entry.ord, args, opts);"
            );
            let upload_limit = if stream.interaction.limit.is_some() {
                "entry.limit"
            } else {
                "16128"
            };
            let adapter = match stream.form {
                "elements" => format!(
                    "$rt.encodeUpload(raw.upload, (value: {}) => element.encode(value), (bytes) => this.#codec.verify(elementType, bytes), {upload_limit})",
                    stream.element_type
                ),
                "text" => format!("$rt.textUpload(raw.upload, {upload_limit})"),
                _ => format!("$rt.byteUpload(raw.upload, {upload_limit})"),
            };
            let _ = writeln!(out, "    const upload = {adapter};");
            if let Some(reply) = &stream.duplex_reply {
                match reply.form {
                    "elements" => out.push_str("    const stream = raw.stream.pipeThrough($rt.decodeStream((bytes) => replyElement.decode(bytes), (bytes) => this.#codec.verify(replyElementType, bytes)));\n"),
                    "text" => out.push_str("    const stream = raw.stream.pipeThrough($rt.decodeTextStream());\n"),
                    _ => out.push_str("    const stream = raw.stream;\n"),
                }
                out.push_str("    return { upload, stream };\n  }\n");
            } else if stream.reply.is_some() {
                let _ = writeln!(
                    out,
                    "    const reply = raw.reply.then((bytes) => $rt.decodeReply(this.#codec, entry.reply, {ids}.reply, bytes));\n    return {{ upload, reply }};\n  }}"
                );
            } else {
                out.push_str("    return { upload, accepted: raw.accepted };\n  }\n");
            }
            return;
        }
        out.push_str(
            "    const raw = this.#port.queryStream(entry.iface, entry.ord, args, opts);\n",
        );
        match stream.form {
            "elements" => out.push_str("    return raw.pipeThrough($rt.decodeStream((bytes) => element.decode(bytes), (bytes) => this.#codec.verify(elementType, bytes)));\n"),
            "text" => out.push_str("    return raw.pipeThrough($rt.decodeTextStream());\n"),
            _ => out.push_str("    return raw;\n"),
        }
        out.push_str("  }\n");
    }

    /// The doc comment of a call: the member's doc, its parameter tags, then
    /// `extra` tags, indented by `indent`.
    fn call_docs(
        &self,
        i: &v1::Interaction,
        params: &[&v1::Param],
        bindings: &[String],
        indent: &str,
        extra: &[String],
    ) -> String {
        let parts = crate::TsDocParts {
            doc: &i.doc,
            links: &i.links,
            see: &i.see,
            since: &i.since,
            rules: &i.rules,
            deprecated: i.deprecated.as_deref(),
        };
        self.with_doc_ctx(|ctx| {
            let mut tags = Vec::new();
            for (param, binding) in params.iter().zip(bindings) {
                let doc = crate::linked(ctx, &param.doc, &param.links);
                if !doc.is_empty() {
                    tags.push(format!("@param {binding} - {doc}"));
                }
            }
            tags.extend_from_slice(extra);
            crate::jsdoc(ctx, indent, &parts, &tags)
        })
    }

    /// Runs `render` with the rendering context of the doc comments of the
    /// module.
    fn with_doc_ctx<R>(&self, render: impl FnOnce(&crate::Ctx) -> R) -> R {
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
        render(&ctx)
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
        self.emit_client(out, interface, name);
        Ok(())
    }

    /// Writes `<name>Client`, the class that carries each command, query,
    /// signal and event of the interface as methods over the ports its kinds
    /// need, when the interface has one, and `<name>Event`, the union that
    /// `onEvent` delivers, when it has an event. `bind` checks the catalog
    /// hash of the port and of the codec, resolves each payload type id once
    /// and registers the check of each signal.
    fn emit_client(&self, out: &mut String, interface: &v1::Interface, name: &str) {
        enum Item<'m, 'a> {
            Call(&'m CallMember<'a>),
            Feed(&'m FeedMember<'a>),
            Stream(&'m StreamCall<'a>),
        }
        let items: Vec<Item<'_, 'a>> = interface
            .slots
            .iter()
            .filter_map(|slot| {
                let key = (interface.number, slot.ordinal);
                self.call_members
                    .get(&key)
                    .map(Item::Call)
                    .or_else(|| self.feed_members.get(&key).map(Item::Feed))
                    .or_else(|| self.stream_calls.get(&key).map(Item::Stream))
            })
            .collect();
        if items.is_empty() {
            return;
        }
        let class = format!("{name}Client");
        let member_of = |interaction: &v1::Interaction| -> String {
            interaction
                .name
                .as_ref()
                .map_or("", |n| n.declared.as_str())
                .to_string()
        };
        let member_name = |item: &Item<'_, 'a>| -> String {
            match item {
                Item::Call(call) => member_of(call.interaction),
                Item::Feed(feed) => member_of(feed.interaction),
                Item::Stream(stream) => member_of(stream.interaction),
            }
        };
        let has_calls = items.iter().any(|item| matches!(item, Item::Call(_)));
        let has_streams = items.iter().any(|item| matches!(item, Item::Stream(_)));
        let has_signals = items
            .iter()
            .any(|item| matches!(item, Item::Feed(feed) if !feed.event));
        let events: Vec<&FeedMember<'a>> = items
            .iter()
            .filter_map(|item| match item {
                Item::Feed(feed) if feed.event => Some(*feed),
                _ => None,
            })
            .collect();
        let mut ports = Vec::new();
        // `StreamCaller` extends `Caller`, so it covers the calls too.
        if has_streams {
            ports.push("$rt.StreamCaller");
        } else if has_calls {
            ports.push("$rt.Caller");
        }
        if has_signals {
            ports.push("$rt.SignalReader");
        }
        if !events.is_empty() {
            ports.push("$rt.EventSource");
        }
        let port_type = ports.join(" & ");
        let parts = crate::TsDocParts {
            doc: &interface.doc,
            links: &interface.links,
            see: &interface.see,
            since: &interface.since,
            rules: &interface.rules,
            deprecated: interface.deprecated.as_deref(),
        };
        let mut docs = self.with_doc_ctx(|ctx| crate::jsdoc(ctx, "", &parts, &[]));
        if docs.is_empty() {
            docs = format!("/** The client of interface `{name}`. */\n");
        }
        if !events.is_empty() {
            let _ = writeln!(
                out,
                "\n/** One occurrence of an event of interface `{name}`, as `{class}.onEvent` delivers it. */\nexport type {name}Event ="
            );
            for (index, feed) in events.iter().enumerate() {
                let member = member_of(feed.interaction);
                let end = if index + 1 == events.len() { ";" } else { "" };
                let _ = writeln!(
                    out,
                    "  | {{ readonly event: \"{member}\"; readonly occurrence: $rt.Occurrence<{}> }}{end}",
                    feed.value_type
                );
            }
        }
        // A stream member's table entry is captured under a name no parameter
        // or table of the module can take, so a method body never names a
        // table that a parameter or a local could shadow.
        for item in &items {
            if let Item::Stream(stream) = item {
                let member = member_of(stream.interaction);
                let _ = writeln!(out, "\nconst {} = {name}.{member};", stream.entry_binding);
            }
        }
        out.push('\n');
        out.push_str(&docs);
        let _ = writeln!(out, "export class {class} {{");
        let _ = writeln!(
            out,
            "  readonly #port: {port_type};\n  readonly #codec: $rt.Codec;"
        );
        let ids: Vec<String> = items
            .iter()
            .map(|item| {
                let shape = match item {
                    Item::Call(call) if call.reply_type.is_some() => {
                        "readonly request: number; readonly reply: number".to_string()
                    }
                    Item::Call(_) => "readonly request: number".to_string(),
                    Item::Feed(_) => "readonly payload: number".to_string(),
                    Item::Stream(stream) => {
                        let mut shape = "readonly request: number".to_string();
                        if stream.element.is_some() {
                            shape.push_str("; readonly element: number");
                        }
                        if stream.reply.is_some() {
                            shape.push_str("; readonly reply: number");
                        }
                        if stream
                            .duplex_reply
                            .as_ref()
                            .is_some_and(|reply| reply.element.is_some())
                        {
                            shape.push_str("; readonly replyElement: number");
                        }
                        shape
                    }
                };
                format!("readonly {}: {{ {shape} }}", member_name(item))
            })
            .collect();
        let _ = writeln!(out, "  readonly #ids: {{ {} }};\n", ids.join("; "));
        let _ = writeln!(
            out,
            "  private constructor(port: {port_type}, codec: $rt.Codec) {{\n    this.#port = port;\n    this.#codec = codec;\n    this.#ids = {{"
        );
        for item in &items {
            let member = member_name(item);
            match item {
                Item::Call(call) => {
                    let reply = if call.reply_type.is_some() {
                        format!(", reply: codec.typeId({name}.{member}.reply.type)")
                    } else {
                        String::new()
                    };
                    let _ = writeln!(
                        out,
                        "      {member}: {{ request: codec.typeId({name}.{member}.request.type){reply} }},"
                    );
                }
                Item::Feed(_) => {
                    let _ = writeln!(
                        out,
                        "      {member}: {{ payload: codec.typeId({name}.{member}.payload.type) }},"
                    );
                }
                Item::Stream(stream) => {
                    let entry = &stream.entry_binding;
                    let mut ids = vec![format!("request: codec.typeId({entry}.request.type)")];
                    if stream.element.is_some() {
                        let index = usize::from(stream.duplex_reply.is_some());
                        ids.push(format!(
                            "element: codec.typeId({entry}.streams[{index}].element.type)"
                        ));
                    }
                    if stream.reply.is_some() {
                        ids.push(format!("reply: codec.typeId({entry}.reply.type)"));
                    }
                    if stream
                        .duplex_reply
                        .as_ref()
                        .is_some_and(|reply| reply.element.is_some())
                    {
                        ids.push(format!(
                            "replyElement: codec.typeId({entry}.streams[0].element.type)"
                        ));
                    }
                    let _ = writeln!(out, "      {member}: {{ {} }},", ids.join(", "));
                }
            }
        }
        out.push_str("    };\n  }\n\n");
        let registers = if has_signals {
            ", and registers the check of each signal with the port"
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "  /**\n   * Binds the client to a port and a loaded codec{registers}. Throws `CatalogMismatch` when\n   * the catalog hash of the port or of the codec differs from `CATALOG_HASH`.\n   */\n  static bind<P extends {port_type}>(port: P, codec: $rt.Codec): {class} {{\n    $rt.checkCatalog(CATALOG_HASH, port, codec);\n    const client = new {class}(port, codec);"
        );
        for item in &items {
            if let Item::Feed(feed) = item
                && !feed.event
            {
                let member = member_of(feed.interaction);
                let _ = writeln!(
                    out,
                    "    {{\n      const entry = signalCheck(codec, {name}.{member}, client.#ids.{member}.payload);\n      port.checkWith(entry.iface, entry.ord, entry.check);\n    }}"
                );
            }
        }
        out.push_str("    return client;\n  }\n");
        for item in &items {
            let member = member_name(item);
            match item {
                Item::Feed(feed) => {
                    self.emit_feed_methods(out, name, &member, feed);
                }
                Item::Stream(stream) => {
                    self.emit_stream_method(out, &member, stream);
                }
                Item::Call(call) => {
                    let mut signature: Vec<String> = call
                        .bindings
                        .iter()
                        .zip(&call.param_types)
                        .map(|(binding, ty)| format!("{binding}: {ty}"))
                        .collect();
                    let (opts, bound, note) = match &call.bound_ms {
                    Some(ms) => (
                        "opts?: Partial<$rt.CallOptions>",
                        format!("{{ ...opts, bound: opts?.bound ?? {ms} }}"),
                        format!("The bound defaults to the declared maximum response time, {ms} ms."),
                    ),
                    None => (
                        "opts: $rt.CallOptions",
                        "opts".to_string(),
                        "The member has no maximum response time inside the range of a port bound, so `opts.bound` is required."
                            .to_string(),
                    ),
                };
                    signature.push(opts.to_string());
                    let abort = if call.reply_type.is_none() {
                        "`opts.signal` aborts the call; a provider that already received the command may still run it."
                    } else {
                        "`opts.signal` aborts the call."
                    };
                    let mut extra = vec![format!("@param opts - {note} {abort}")];
                    if call.reply_type.is_none() {
                        extra.push(
                            "@returns Resolves when the provider accepted the command.".to_string(),
                        );
                    }
                    out.push('\n');
                    out.push_str(&self.call_docs(
                        call.interaction,
                        &call.params,
                        &call.bindings,
                        "  ",
                        &extra,
                    ));
                    let returns = call.reply_type.as_deref().unwrap_or("void");
                    let _ = writeln!(
                        out,
                        "  async {member}({}): Promise<{returns}> {{",
                        signature.join(", ")
                    );
                    let value = if call.named_request {
                        call.bindings[0].clone()
                    } else {
                        let fields = call
                            .params
                            .iter()
                            .zip(&call.bindings)
                            .map(|(p, b)| {
                                format!(
                                    "{}: {b}",
                                    p.name.as_ref().map_or("", |n| n.declared.as_str())
                                )
                            })
                            .collect::<Vec<_>>();
                        if fields.is_empty() {
                            "{}".to_string()
                        } else {
                            format!("{{ {} }}", fields.join(", "))
                        }
                    };
                    let _ = writeln!(
                        out,
                        "    const args = $rt.encodeRequest(this.#codec, {name}.{member}.request, this.#ids.{member}.request, {value});"
                    );
                    if call.reply_type.is_some() {
                        let _ = writeln!(
                            out,
                            "    const raw = await this.#port.query({name}.{member}.iface, {name}.{member}.ord, args, {bound});\n    return $rt.decodeReply(this.#codec, {name}.{member}.reply, this.#ids.{member}.reply, raw);"
                        );
                    } else {
                        let _ = writeln!(
                            out,
                            "    await this.#port.command({name}.{member}.iface, {name}.{member}.ord, args, {bound});"
                        );
                    }
                    out.push_str("  }\n");
                }
            }
        }
        if let Some(first) = events.first() {
            let first_member = member_of(first.interaction);
            out.push_str(
                "\n  /**\n   * Calls `listener` with every accepted occurrence of every subscribed event of the\n   * interface, in arrival order, and returns the function that removes it. An occurrence\n   * that the port refused, and one whose payload fails the verification of this class\n   * (a rule failure included), has `occurrence.detected` set to `\"corrupt\"` and no value.\n   */\n",
            );
            let _ = writeln!(
                out,
                "  onEvent(listener: (event: {name}Event) => void): () => void {{\n    return this.#port.listen({name}.{first_member}.iface, (raw) => {{\n      switch (raw.ord) {{"
            );
            for feed in &events {
                let member = member_of(feed.interaction);
                let _ = writeln!(
                    out,
                    "        case {name}.{member}.ord:\n          listener({{\n            event: \"{member}\",\n            occurrence: $rt.decodeOccurrence(this.#codec, {name}.{member}.payload, this.#ids.{member}.payload, raw),\n          }});\n          break;"
                );
            }
            out.push_str("      }\n    });\n  }\n");
        }
        for slot in &interface.slots {
            let key = (interface.number, slot.ordinal);
            if let Some(reason) = self
                .member_omissions
                .get(&key)
                .or_else(|| self.stream_omissions.get(&key))
                && let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                    &slot.occupant
            {
                let member = interaction
                    .name
                    .as_ref()
                    .map_or("", |n| n.declared.as_str());
                let _ = writeln!(out, "\n  // {member} has no method: {reason}");
            }
        }
        out.push_str("}\n");
    }

    /// Writes the methods of one signal or event of a client class: `subscribe`
    /// for both, and for a signal the held value, the next value and the
    /// watcher.
    fn emit_feed_methods(&self, out: &mut String, name: &str, member: &str, feed: &FeedMember<'_>) {
        let pascal = pascal_member(member);
        let target = format!("{name}.{member}.iface, {name}.{member}.ord");
        let ty = &feed.value_type;
        let subscribe_note = "The promise resolves when the provider answered; `opts.bound` and `opts.signal` limit the wait, and the promise rejects with a `CallError` of the category `down` when the session has ended.";
        let kind = if feed.event { "event" } else { "signal" };
        out.push('\n');
        let _ = writeln!(
            out,
            "  /** Subscribes to {kind} `{member}`. {subscribe_note} */\n  subscribe{pascal}(opts?: $rt.WaitOptions): Promise<void> {{\n    return this.#port.subscribe({target}, opts);\n  }}"
        );
        let Some(init) = &feed.init else {
            return;
        };
        let decode = format!(
            "$rt.decodeSample(this.#codec, {name}.{member}.payload, this.#ids.{member}.payload, raw, {init})"
        );
        out.push('\n');
        out.push_str(&self.call_docs(
            feed.interaction,
            &[],
            &[],
            "  ",
            &[
                "@returns The held sample at once. This method never throws: after the session ended it keeps returning the last held sample (`readX` and `subscribeX` reject with `down`). Before the first publish it is the init sample: provenance `init` and the init value of the type. Under `init` and `invalid` the value is the last good one when there is one, else the init value. A live payload that fails verification becomes provenance `invalid` with `detected` set."
                    .to_string(),
            ],
        ));
        let _ = writeln!(
            out,
            "  {member}(): $rt.Sample<{ty}> {{\n    const raw = this.#port.sample({target});\n    return raw === undefined ? $rt.initSample({init}) : {decode};\n  }}"
        );
        let _ = writeln!(
            out,
            "\n  /** Waits for the next publish of signal `{member}`. `opts.bound` and `opts.signal` limit the wait. */\n  async read{pascal}(opts?: $rt.WaitOptions): Promise<$rt.Sample<{ty}>> {{\n    const raw = await this.#port.read({target}, opts);\n    return {decode};\n  }}"
        );
        let _ = writeln!(
            out,
            "\n  /** Calls `listener` with every accepted publish of signal `{member}` and returns the function that removes it. */\n  watch{pascal}(listener: (sample: $rt.Sample<{ty}>) => void): () => void {{\n    return this.#port.watch({target}, (raw) => listener({decode}));\n  }}"
        );
    }

    /// Writes `signalChecks`, which gives `connect` one check per signal that
    /// has a member table entry.
    fn emit_signal_checks(&self, out: &mut String) {
        out.push_str(
            "\nfunction signalCheck(codec: $rt.Codec, entry: { readonly iface: number; readonly ord: number; readonly payload: { readonly type: string } }, typeId: number = codec.typeId(entry.payload.type)): $rt.SignalCheckEntry {\n  return {\n    iface: entry.iface,\n    ord: entry.ord,\n    check: (bytes) => {\n      const result = codec.verify(typeId, bytes);\n      if (result.ok) return undefined;\n      if (result.kind !== \"rule\") return \"corrupt\";\n      const rule = (RULES as readonly string[]).find((known) => known === result.rule);\n      if (rule === undefined) throw new $rt.CodecError(\"unknown rule during signal verification\");\n      return { rule: rule as (typeof RULES)[number] };\n    },\n  };\n}\n",
        );
        out.push_str(
            "\n/** Returns the check of each signal of this package, for the `checks` option of `connect`. The checks resolve the type ids of `codec` once, here. */\nexport function signalChecks(codec: $rt.Codec): readonly $rt.SignalCheckEntry[] {\n  return [\n",
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

/// The declared name of an interaction.
fn interaction_member(interaction: &v1::Interaction) -> &str {
    interaction
        .name
        .as_ref()
        .map_or("", |name| name.declared.as_str())
}

/// The member name with its first character in upper case, as the methods of
/// the client class spell it after `subscribe`, `read` and `watch`.
fn pascal_member(member: &str) -> String {
    let mut chars = member.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The response bound `max_us` microseconds, as a decimal number of
/// milliseconds, or `None` when it is not a positive value that a port
/// accepts (at most 2147483647 ms).
fn milliseconds(max_us: &str) -> Option<String> {
    let us: u64 = max_us.parse().ok()?;
    if us == 0 || us > 2_147_483_647_000 {
        return None;
    }
    let (whole, rest) = (us / 1000, us % 1000);
    if rest == 0 {
        return Some(whole.to_string());
    }
    let fraction = format!("{rest:03}");
    Some(format!("{whole}.{}", fraction.trim_end_matches('0')))
}

/// Allocates local bindings without shadowing generated locals or helper functions.
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
        "args",
        "raw",
        "bytes",
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
    /// The `make<Name>Unchecked` call of a named scalar that has one, with the
    /// argument over the box value (`{}`).
    Unchecked(String, &'static str),
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
fn string_rule(qualified: &str) -> Rule {
    Rule {
        pack: text_pack(qualified),
        unpack: "{} as string".to_string(),
        plain: false,
        nullable: false,
        vector: false,
    }
}

/// The pack template of a string position. `flatc` writes the string through
/// `TextEncoder`, which replaces an unpaired surrogate with U+FFFD without an
/// error, so the converter checks well-formedness first. `{path}` stands for the
/// path of the field inside the payload type `qualified`; see [`fill_pack`].
fn text_pack(qualified: &str) -> String {
    format!("$rt.checkWellFormed({{}}, \"{qualified}.{{path}}\")")
}

/// Fills a pack template with the value read and the path of the position. An
/// empty path names the payload type itself.
fn fill_pack(template: &str, read: &str, path: &str) -> String {
    let filled = if path.is_empty() {
        template.replace(".{path}", "")
    } else {
        template.replace("{path}", path)
    };
    filled.replace("{}", read)
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
