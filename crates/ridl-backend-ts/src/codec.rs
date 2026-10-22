//! The codec module `<base>.codec.ts`, written from the codegen model only.
//!
//! The module wraps the object API that `flatc --ts --gen-object-api` writes
//! for the package's `.fbs` in plain-object functions: one
//! `encode<Name>(value)` and one `decode<Name>(bytes)` per payload type, which
//! is every FlatBuffers root of the package with a finite bound. A private
//! converter pair `pack<Name>` and `unpack<Name>` is written for each struct
//! a payload type reaches. The module also exports the catalog's name and
//! hash, the qualified names of the payload types, the text of the rule codes
//! of the wasm module, and one member table per declared interface.
//!
//! The conversion covers named scalars, enums, enum sets and structs. A
//! payload type that reaches any other kind is refused with a
//! [`GenerateError`] that names it.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use ridl_ir::codegen::v1;

use crate::{GenerateError, GeneratedTs};

/// The text of the rule codes the wasm module reports, rule code `n` at
/// index `n - 2`: the order of `WASM_RULES` in the generated crate.
const RULES: [&str; 6] = ["length", "pattern", "range", "step", "unique", "variant"];

/// The length of `Catalog.hash`.
const HASH_LEN: usize = 32;

/// Generates `<artifact_base>.codec.ts` for the package `model` describes.
/// The returned source starts at the imports; the caller writes the
/// generated marker and the header above it.
pub fn generate_codec(
    model: &v1::Model,
    artifact_base: &str,
) -> Result<GeneratedTs, GenerateError> {
    Codec::new(model)?.emit(artifact_base)
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

struct Codec<'a> {
    model: &'a v1::Model,
    package: &'a str,
    /// The declarations that are payload types, by index, with the bound
    /// of each root that has one.
    declaration_bounded: HashMap<usize, bool>,
    /// The induced tuples that are roots, by index, with whether each has a
    /// finite bound.
    tuple_bounded: HashMap<usize, bool>,
    /// The structs a payload type reaches, by index.
    reached: HashSet<usize>,
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
        for payload in codec.payload_types() {
            let qualified = codec.qualified(payload.name);
            if payload
                .declaration
                .is_some_and(|declaration| declaration.visibility() == v1::Visibility::Internal)
            {
                return refuse(format!(
                    "`{qualified}` is an internal payload type, which `<base>.ts` does not \
                     export, so the codec module cannot name it"
                ));
            }
            match payload.declaration {
                Some(declaration) => codec.reach(declaration, &qualified)?,
                None => return refuse(not_converted(&qualified, "a tuple")),
            }
        }
        Ok(codec)
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

    /// Records the structs `declaration` reaches, and refuses a kind the
    /// module does not convert. `qualified` names the payload type.
    fn reach(
        &mut self,
        declaration: &'a v1::Declaration,
        qualified: &str,
    ) -> Result<(), GenerateError> {
        use v1::declaration::Kind;
        match &declaration.kind {
            Some(Kind::Scalar(scalar)) => {
                if scalar.class() == v1::ScalarClass::Bytes {
                    return refuse(not_converted(qualified, "bytes"));
                }
                Ok(())
            }
            Some(Kind::Enum(_) | Kind::EnumSet(_)) => Ok(()),
            Some(Kind::Union(_)) => refuse(not_converted(qualified, "a union")),
            Some(Kind::Struct(shape)) => {
                for field in struct_fields(shape) {
                    let Some(ty) = field.r#type.as_ref() else {
                        continue;
                    };
                    if ty.optional {
                        return refuse(not_converted(qualified, "an optional field"));
                    }
                    match &ty.kind {
                        Some(v1::r#type::Kind::Named(reference)) => {
                            let target = self.local(reference, qualified)?;
                            if let Some(Kind::Struct(_)) = &target.kind {
                                let index = reference.index as usize;
                                if self.reached.insert(index) {
                                    self.reach(target, qualified)?;
                                }
                            } else {
                                self.reach(target, qualified)?;
                            }
                        }
                        Some(v1::r#type::Kind::Primitive(primitive)) => {
                            if *primitive == v1::PrimitiveType::Bytes as i32 {
                                return refuse(not_converted(qualified, "bytes"));
                            }
                        }
                        Some(v1::r#type::Kind::Inline(scalar)) => {
                            if scalar.class() == v1::ScalarClass::Bytes {
                                return refuse(not_converted(qualified, "bytes"));
                            }
                        }
                        Some(v1::r#type::Kind::Tuple(_)) => {
                            return refuse(not_converted(qualified, "a tuple"));
                        }
                        Some(v1::r#type::Kind::Array(_)) => {
                            return refuse(not_converted(qualified, "an array"));
                        }
                        Some(v1::r#type::Kind::Map(_)) => {
                            return refuse(not_converted(qualified, "a map"));
                        }
                        Some(v1::r#type::Kind::Stream(_)) | None => {
                            return refuse(not_converted(qualified, "a stream"));
                        }
                    }
                }
                self.reached.insert(index_of(self.model, declaration));
                Ok(())
            }
            Some(Kind::Constant(_)) | None => Ok(()),
        }
    }

    /// The local declaration `reference` names, or a refusal when it names
    /// a declaration of another package.
    fn local(
        &self,
        reference: &v1::TypeRef,
        qualified: &str,
    ) -> Result<&'a v1::Declaration, GenerateError> {
        if reference.foreign || !reference.resolved {
            return refuse(not_converted(qualified, "a foreign reference"));
        }
        match self.model.declarations.get(reference.index as usize) {
            Some(declaration) => Ok(declaration),
            None => refuse(format!(
                "malformed codegen model: `{}` names declaration {}, which does not exist",
                reference.reference, reference.index
            )),
        }
    }

    fn emit(&self, artifact_base: &str) -> Result<GeneratedTs, GenerateError> {
        let mut out = String::new();
        let segments = self.package.replace('.', "/");
        out.push_str("import * as flatbuffers from \"flatbuffers\";\n");
        out.push_str("import type { MemberRef, PayloadCodec } from \"@driftsys/ridl-runtime\";\n");
        let _ = writeln!(out, "import type * as t from \"./{artifact_base}.js\";");
        out.push_str(
            "// `flatc --ts --gen-object-api` writes this module when its `-o` is the directory\n\
             // of this file.\n",
        );
        let _ = writeln!(out, "import * as fb from \"./{segments}.js\";");
        out.push('\n');

        let catalog = self.model.catalog.as_ref();
        let name = catalog.map_or("", |catalog| catalog.package.as_str());
        let hash: Vec<String> = catalog
            .map(|catalog| catalog.hash.iter().map(u8::to_string).collect())
            .unwrap_or_default();
        out.push_str("/** The name of the catalog unit this package belongs to. */\n");
        let _ = writeln!(out, "export const CATALOG_NAME = \"{name}\";");
        out.push_str("/** The hash of the catalog unit, which the wasm module carries. */\n");
        let _ = writeln!(
            out,
            "export const CATALOG_HASH: Uint8Array = Uint8Array.from([{}]);",
            hash.join(", ")
        );
        let payloads = self.payload_types();
        let mut names: Vec<String> = payloads
            .iter()
            .map(|payload| self.qualified(payload.name))
            .collect();
        names.sort();
        out.push_str(
            "/** The qualified names of the payload types of this package, which name the wasm \
             type ids. */\n",
        );
        out.push_str("export const PAYLOAD_TYPES = [\n");
        for name in &names {
            let _ = writeln!(out, "  \"{name}\",");
        }
        out.push_str("] as const;\n");
        out.push_str(
            "/** The text of each rule code the wasm module reports, rule code n at index n - 2. \
             */\n",
        );
        let rules: Vec<String> = RULES.iter().map(|rule| format!("\"{rule}\"")).collect();
        let _ = writeln!(out, "export const RULES = [{}] as const;", rules.join(", "));

        let payload_names: BTreeSet<&str> = payloads.iter().map(|payload| payload.name).collect();
        for (index, declaration) in self.model.declarations.iter().enumerate() {
            let name = declared_name(declaration);
            if payload_names.contains(name) && self.declaration_bounded.get(&index) == Some(&true) {
                self.emit_payload(&mut out, declaration)?;
            }
            if self.reached.contains(&index) {
                self.emit_converters(&mut out, declaration)?;
            }
        }

        out.push('\n');
        for payload in &payloads {
            let _ = writeln!(
                out,
                "const codec{name}: PayloadCodec<t.{name}> = {{ type: \"{qualified}\", encode: \
                 encode{name}, decode: decode{name} }};",
                name = payload.name,
                qualified = self.qualified(payload.name),
            );
        }

        for interface in &self.model.interfaces {
            let Some(v1::interface::Identity::Declared(spelled)) = &interface.identity else {
                continue;
            };
            self.emit_interface(&mut out, interface, &spelled.declared)?;
        }
        Ok(GeneratedTs { source: out })
    }

    fn emit_payload(
        &self,
        out: &mut String,
        declaration: &v1::Declaration,
    ) -> Result<(), GenerateError> {
        use v1::declaration::Kind;
        let name = declared_name(declaration);
        let qualified = self.qualified(name);
        out.push('\n');
        let _ = writeln!(out, "/** Encodes a `{qualified}` payload. */");
        let _ = writeln!(
            out,
            "export function encode{name}(value: t.{name}): Uint8Array {{"
        );
        out.push_str("  const builder = new flatbuffers.Builder();\n");
        let (packed, root) = match &declaration.kind {
            Some(Kind::Struct(_)) => (format!("pack{name}(value)"), None),
            Some(Kind::Enum(_)) => (format!("new fb.{name}BoxT(BigInt(value))"), Some(true)),
            Some(Kind::Scalar(_) | Kind::EnumSet(_)) => {
                (format!("new fb.{name}BoxT(value)"), Some(false))
            }
            _ => {
                return refuse(format!(
                    "malformed codegen model: the payload type `{qualified}` is not a scalar, \
                     an enum, an enum set or a struct"
                ));
            }
        };
        let _ = writeln!(out, "  builder.finish({packed}.pack(builder));");
        out.push_str("  return builder.asUint8Array();\n}\n");
        let _ = writeln!(
            out,
            "/** Decodes a `{qualified}` payload the codec has verified. */"
        );
        let _ = writeln!(
            out,
            "export function decode{name}(bytes: Uint8Array): t.{name} {{"
        );
        match root {
            None => {
                let _ = writeln!(
                    out,
                    "  return unpack{name}(fb.{name}.getRootAs{name}(new \
                     flatbuffers.ByteBuffer(bytes)).unpack());"
                );
            }
            Some(is_enum) => {
                let read = format!(
                    "fb.{name}Box.getRootAs{name}Box(new flatbuffers.ByteBuffer(bytes)).unpack().value"
                );
                if is_enum {
                    let _ = writeln!(out, "  return Number({read}) as t.{name};");
                } else {
                    let _ = writeln!(out, "  return {read} as t.{name};");
                }
            }
        }
        out.push_str("}\n");
        Ok(())
    }

    fn emit_converters(
        &self,
        out: &mut String,
        declaration: &v1::Declaration,
    ) -> Result<(), GenerateError> {
        let Some(v1::declaration::Kind::Struct(shape)) = &declaration.kind else {
            return Ok(());
        };
        let name = declared_name(declaration);
        let qualified = self.qualified(name);
        let fields = struct_fields(shape);
        out.push('\n');
        let _ = writeln!(out, "function pack{name}(value: t.{name}): fb.{name}T {{");
        let _ = writeln!(out, "  const object = new fb.{name}T();");
        let mut unpacked = Vec::new();
        for field in &fields {
            let spelled = field.name.as_ref();
            let declared = spelled.map_or("", |name| name.declared.as_str());
            let camel = accessor(declared);
            let (pack, unpack) = self.field_rule(field, &qualified)?;
            let _ = writeln!(
                out,
                "  object.{camel} = {};",
                pack.replace("{}", &format!("value.{declared}"))
            );
            unpacked.push(format!(
                "    {declared}: {},",
                unpack.replace("{}", &format!("object.{camel}"))
            ));
        }
        out.push_str("  return object;\n}\n");
        let _ = writeln!(
            out,
            "function unpack{name}(object: fb.{name}T): t.{name} {{"
        );
        out.push_str("  return {\n");
        for line in unpacked {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("  };\n}\n");
        Ok(())
    }

    /// The pack and unpack expressions of `field`, with `{}` standing for
    /// the value read.
    fn field_rule(
        &self,
        field: &v1::Field,
        qualified: &str,
    ) -> Result<(String, String), GenerateError> {
        use v1::declaration::Kind;
        match field.r#type.as_ref().and_then(|ty| ty.kind.as_ref()) {
            Some(v1::r#type::Kind::Named(reference)) => {
                let target = self.local(reference, qualified)?;
                let name = declared_name(target);
                Ok(match &target.kind {
                    Some(Kind::Enum(_)) => (
                        "BigInt({})".to_string(),
                        format!("Number({{}}) as t.{name}"),
                    ),
                    Some(Kind::Struct(_)) => {
                        (format!("pack{name}({{}})"), format!("unpack{name}({{}})"))
                    }
                    Some(Kind::Union(_)) => return refuse(not_converted(qualified, "a union")),
                    _ => ("{}".to_string(), format!("{{}} as t.{name}")),
                })
            }
            _ => Ok(("{}".to_string(), "{}".to_string())),
        }
    }

    fn emit_interface(
        &self,
        out: &mut String,
        interface: &v1::Interface,
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
            let mut roles = Vec::new();
            let mut missing = None;
            for (role, payload) in self.roles(interaction, name, member)? {
                match payload {
                    Ok(type_name) => roles.push(format!("{role}: codec{type_name}")),
                    Err(type_name) => {
                        missing = Some(type_name);
                        break;
                    }
                }
            }
            if let Some(type_name) = missing {
                let _ = writeln!(
                    out,
                    "  // {member} has no entry: {} has no finite FlatBuffers bound",
                    self.qualified(type_name)
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

    /// The roles of `interaction`, each with the name of its payload type:
    /// `Ok` when the type has a codec, `Err` when it has no finite bound.
    #[allow(clippy::type_complexity)]
    fn roles(
        &self,
        interaction: &'a v1::Interaction,
        interface: &str,
        member: &str,
    ) -> Result<Vec<(&'static str, Result<&'a str, &'a str>)>, GenerateError> {
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
        &self,
        named: Option<&'a v1::Payload>,
        tuple: Option<&v1::TupleRef>,
        path: &str,
    ) -> Result<Result<&'a str, &'a str>, GenerateError> {
        if let Some(reference) = named.and_then(|payload| payload.r#type.as_ref()) {
            let declaration = self.local(
                reference,
                &format!("{}.{}", reference.package, reference.reference),
            )?;
            let name = declared_name(declaration);
            let bounded = self.declaration_bounded.get(&(reference.index as usize)) == Some(&true);
            return Ok(if bounded { Ok(name) } else { Err(name) });
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
            return Ok(if bounded { Ok(name) } else { Err(name) });
        }
        refuse(format!(
            "`{path}` has a payload that is not a named type or an induced tuple, which the codec \
             module does not carry"
        ))
    }
}

fn not_converted(qualified: &str, kind: &str) -> String {
    format!("`{qualified}` reaches {kind}, which the codec module does not convert yet")
}

/// The name `flatc --ts` gives a field's member. The `.fbs` field is
/// `ridl_ir::name::snake_case` of the declared name, and `flatc` writes it in
/// lower camel case: each `_` is dropped and the character after it is upper
/// case; every other character is kept, digits included.
fn accessor(declared: &str) -> String {
    let snake = ridl_ir::name::snake_case(declared);
    let mut out = String::with_capacity(snake.len());
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

fn index_of(model: &v1::Model, declaration: &v1::Declaration) -> usize {
    model
        .declarations
        .iter()
        .position(|candidate| std::ptr::eq(candidate, declaration))
        .unwrap_or(usize::MAX)
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
    use super::accessor;

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
        ] {
            assert_eq!(accessor(declared), expected, "{declared}");
        }
    }
}
