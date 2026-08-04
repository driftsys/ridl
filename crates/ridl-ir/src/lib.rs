//! RIDL intermediate representation.
//!
//! The v2 schema (`proto/ridl/ir/v2/ir.proto`) is compiled from its protobuf
//! source by `build.rs` (protox + prost-build, ADR-0006 decision 3) and
//! exposed as [`v2`]. v2 is the typl surface plus the ridl interaction layer
//! (ridl language reference §3–§14) with exact decimal values — every numeric
//! value is a canonical decimal string, never a floating-point field (ADR-0007
//! decision 9, ADR-0008 decision 12). The v1 schema was removed when its last
//! consumer moved to v2 (task 6 of the E2 plan), mirroring the E1 v0→v1
//! retirement.

pub mod v2 {
    //! IR v2 — the typl surface plus the ridl interaction layer (ridl
    //! language reference §3–§14) with exact decimal values (ADR-0007
    //! decision 9, ADR-0008 decision 12).

    include!(concat!(env!("OUT_DIR"), "/ridl.ir.v2.rs"));

    /// The descriptor pool over the compiled IR schema — the reflection data
    /// the canonical protobuf JSON surface needs (ADR-0014 decision 7).
    /// `build.rs` writes the `FileDescriptorSet` to `OUT_DIR` from the same
    /// `protox` compilation that generates the types above, so the pool and
    /// the types cannot disagree; every `expect` on this path leans on that.
    static DESCRIPTOR_POOL: std::sync::LazyLock<prost_reflect::DescriptorPool> =
        std::sync::LazyLock::new(|| {
            prost_reflect::DescriptorPool::decode(
                include_bytes!(concat!(env!("OUT_DIR"), "/ir_descriptor.binpb")).as_slice(),
            )
            .expect("the embedded descriptor set decodes: build.rs wrote it from the schema compilation that generated these types")
        });

    /// The `Package` message descriptor — the entry point of every
    /// reflection-path encoder and decoder in this module.
    pub(crate) fn package_descriptor() -> prost_reflect::MessageDescriptor {
        DESCRIPTOR_POOL
            .get_message_by_name("ridl.ir.v2.Package")
            .expect("ridl.ir.v2.Package is declared by the compiled schema")
    }

    /// Rebuilds a package as a `DynamicMessage` over the descriptor pool —
    /// the step `prost-reflect` needs before rendering a text encoding.
    /// Transcoding goes through the wire encoding, whose decoder enforces
    /// prost's fixed recursion limit, so a package whose composite nesting
    /// crosses that limit fails here — an input-dependent failure, not
    /// schema drift (ADR-0014 decision 12).
    fn transcode(package: &Package) -> Result<prost_reflect::DynamicMessage, prost::DecodeError> {
        let mut dynamic = prost_reflect::DynamicMessage::new(package_descriptor());
        dynamic.transcode_from(package)?;
        Ok(dynamic)
    }

    /// Derives the synthesized transport identity of an inline `T | E`
    /// result union (ADR-0008 decision 4): the enclosing interface name plus
    /// the interaction ordinal plus the ordered arm references. The single
    /// derivation every consumer — backends and the diff classifier — calls,
    /// so the identity stays stable under compatible evolution.
    pub fn fallible_transport_identity(
        interface: &str,
        ordinal: u32,
        fallible: &FallibleType,
    ) -> String {
        format!(
            "{interface}#{ordinal}:{ok}|{err}",
            ok = fallible.ok,
            err = fallible.err
        )
    }

    /// The error [`to_json_pretty`] and [`to_text_format`] return. The
    /// serialization surface is fallible on purpose — ADR-0014 decision 12,
    /// which retracts decision 7's infallible return: the reflection path
    /// transcodes through the wire encoding, whose decoder enforces prost's
    /// fixed recursion limit, and legal source can nest composites past it.
    #[derive(Debug)]
    pub struct SerializeError {
        /// The encoding being rendered when the transcode failed — named in
        /// the message, so a build requesting several IR emits attributes
        /// each failure to its own artifact.
        encoding: &'static str,
        source: prost::DecodeError,
    }

    impl std::fmt::Display for SerializeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "cannot render the package as {}: {}; the known cause \
                 is composite nesting deeper than the transcoding decoder's recursion limit",
                self.encoding, self.source
            )
        }
    }

    impl std::error::Error for SerializeError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.source)
        }
    }

    /// Renders a package as pretty-printed canonical protobuf JSON — the one
    /// dialect every IR surface carries: the `--emit ir-json` artifact, the
    /// baselines, and the goldens (ADR-0014 decision 1).
    ///
    /// A field holding its default is emitted rather than skipped (decision
    /// 2); an unset proto3 `optional` field is omitted entirely, never
    /// rendered as `null` (the answer to that decision's open item). 64-bit
    /// fields render as strings, the canonical mapping JavaScript consumers
    /// need (decision 8).
    ///
    /// Fallible on purpose (ADR-0014 decision 12, retracting decision 7's
    /// infallible return): the transcode into the dynamic message goes
    /// through the wire encoding, and a package whose composite nesting
    /// crosses prost's recursion limit fails there. That input is legal
    /// source, so the failure is returned rather than panicked on.
    pub fn to_json_pretty(package: &Package) -> Result<String, SerializeError> {
        let mut buf = Vec::new();
        let mut serializer = serde_json::Serializer::pretty(&mut buf);
        transcode(package)
            .map_err(|source| SerializeError {
                encoding: "canonical protobuf JSON",
                source,
            })?
            .serialize_with_options(
                &mut serializer,
                &prost_reflect::SerializeOptions::new().skip_default_fields(false),
            )
            .expect("rendering the transcoded message cannot fail: the input-dependent recursion limit binds the transcode above, and the writer is an in-memory Vec");
        Ok(String::from_utf8(buf).expect("serde_json emits UTF-8"))
    }

    /// Reads a package from canonical protobuf JSON — the inverse of
    /// [`to_json_pretty`]. Unknown fields are rejected (the `prost-reflect`
    /// default), so a snapshot written against a different schema fails
    /// loudly rather than dropping fields silently. A package whose
    /// composite nesting crosses prost's recursion limit fails in the
    /// transcode out of the dynamic message; that failure is mapped into
    /// the same error return, not expected on (ADR-0014 decision 12).
    pub fn from_json(text: &str) -> Result<Package, serde_json::Error> {
        let mut deserializer = serde_json::Deserializer::from_str(text);
        let dynamic =
            prost_reflect::DynamicMessage::deserialize(package_descriptor(), &mut deserializer)?;
        deserializer.end()?;
        dynamic.transcode_to().map_err(serde::de::Error::custom)
    }

    /// Renders a package in the protobuf text format — the inspection
    /// encoding (ADR-0014 decision 9): emittable, but not a recommended
    /// interchange form. Rendered `pretty`, with a field holding its default
    /// emitted rather than skipped (decision 2) and message fields printed
    /// in schema index order, so the output ordering is deterministic rather
    /// than incidental (decision 8).
    ///
    /// Fallible for the same reason [`to_json_pretty`] is (ADR-0014 decision
    /// 12): the transcode into the dynamic message goes through the wire
    /// encoding, and a package whose composite nesting crosses prost's
    /// recursion limit fails there. That input is legal source, so the
    /// failure is returned rather than panicked on.
    pub fn to_text_format(package: &Package) -> Result<String, SerializeError> {
        let dynamic = transcode(package).map_err(|source| SerializeError {
            encoding: "prototext",
            source,
        })?;
        Ok(dynamic.to_text_format_with_options(
            &prost_reflect::text_format::FormatOptions::new()
                .pretty(true)
                .skip_default_fields(false)
                .print_message_fields_in_index_order(true),
        ))
    }

    /// The error [`from_text_format`] returns: the input does not parse as
    /// prototext, or the parsed message does not transcode into the
    /// generated types. The transcode failure is the read direction of the
    /// recursion-limit failure mode (ADR-0014 decision 12) — input-dependent,
    /// so it is mapped into this return rather than expected on.
    #[derive(Debug)]
    pub enum TextFormatError {
        /// The input is not valid prototext for the `Package` schema.
        Parse(prost_reflect::text_format::ParseError),
        /// The parsed message cannot be rebuilt as a typed `Package`.
        Transcode(prost::DecodeError),
    }

    impl std::fmt::Display for TextFormatError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Parse(source) => {
                    write!(f, "cannot parse the text as an IR package: {source}")
                }
                Self::Transcode(source) => write!(
                    f,
                    "cannot rebuild the parsed prototext as a package: {source}; the known cause \
                     is composite nesting deeper than the transcoding decoder's recursion limit"
                ),
            }
        }
    }

    impl std::error::Error for TextFormatError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Parse(source) => Some(source),
                Self::Transcode(source) => Some(source),
            }
        }
    }

    /// Reads a package from the protobuf text format — the inverse of
    /// [`to_text_format`], required rather than speculative: without it the
    /// prototext emit has no round-trip test, and a write path with no read
    /// path is untested by construction (ADR-0014 decision 7). A package
    /// whose composite nesting crosses prost's recursion limit fails in the
    /// transcode out of the dynamic message; that failure is mapped into the
    /// error return, not expected on (ADR-0014 decision 12).
    pub fn from_text_format(text: &str) -> Result<Package, TextFormatError> {
        let dynamic = prost_reflect::DynamicMessage::parse_text_format(package_descriptor(), text)
            .map_err(TextFormatError::Parse)?;
        dynamic.transcode_to().map_err(TextFormatError::Transcode)
    }

    /// Encodes a package in the protobuf binary wire format — the canonical
    /// interchange encoding (ADR-0014 decision 9). Binary needs no
    /// descriptors: prost's generated encoding is schema-faithful by
    /// construction.
    pub fn to_binary(package: &Package) -> Vec<u8> {
        prost::Message::encode_to_vec(package)
    }

    /// Decodes a package from the protobuf binary wire format — the inverse
    /// of [`to_binary`].
    pub fn from_binary(bytes: &[u8]) -> Result<Package, prost::DecodeError> {
        prost::Message::decode(bytes)
    }

    /// One interface shape of a package (ridl §14.0): a declared `interface`,
    /// or the inline shape of a `service` (§14.5).
    ///
    /// **[`Package::interfaces`] is not the complete set.** A `service`
    /// declared with an inline body carries a full [`Interface`] inside its
    /// own `shape` oneof, which lives outside `interfaces`; a consumer that
    /// walks `interfaces` alone silently misses it. Six defects of exactly
    /// that shape were found independently across E2 — observer-stub
    /// lowering, both backends' transport identity, `ridl test`'s report, the
    /// Rust backend's collision check, and the desk check's span index.
    /// [`Package::shapes`] is the one walk that sees both, the way
    /// [`fallible_transport_identity`] is the one transport-identity
    /// derivation.
    ///
    /// This view is deliberately not a bare `&Interface`, because two of an
    /// inline shape's own fields are empty by construction and reading them
    /// is what produced two of those six defects:
    ///
    /// - [`Interface::name`] is `""` for an inline shape, so [`Self::name`]
    ///   carries the **identity** name instead — the interface's own name, or
    ///   the owning service's dotted global name. That is the name the diff
    ///   paths, the observer-stub scoping, and both backends' identity fields
    ///   already use.
    /// - [`Interface::visibility`] is `VISIBILITY_UNSPECIFIED` for an inline
    ///   shape; the owning [`Service`] carries the authoritative one, which
    ///   [`Self::visibility`] reads.
    ///
    /// The generated *type* name is not derived here on purpose: mangling is
    /// language-specific and stays with each backend.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct InterfaceShape<'a> {
        /// The name this shape is known by outside the package: an
        /// `interface` declaration's own name, or the owning service's dotted
        /// global name. Never `Interface::name` for an inline shape.
        pub name: &'a str,
        /// The interface body — its interactions and its doc envelope.
        pub interface: &'a Interface,
        /// The owning service, for an inline shape; `None` for a declared
        /// `interface`.
        pub service: Option<&'a Service>,
    }

    impl InterfaceShape<'_> {
        /// The authoritative visibility of this shape: the owning service's
        /// for an inline shape (an inline shape's own field is
        /// `VISIBILITY_UNSPECIFIED` by construction), the interface's own
        /// otherwise.
        pub fn visibility(&self) -> i32 {
            match self.service {
                Some(service) => service.visibility,
                None => self.interface.visibility,
            }
        }

        /// `true` when this shape is the inline body of a `service`.
        pub fn is_inline(&self) -> bool {
            self.service.is_some()
        }
    }

    impl Package {
        /// Every interface shape the package carries — the declared
        /// interfaces and the inline shapes of its services. See
        /// [`InterfaceShape`] for why walking [`Package::interfaces`] alone is
        /// a defect.
        ///
        /// The order is the one every consumer already walked: the declared
        /// interfaces in source order, then the services in source order. A
        /// service that names an interface after `:` yields nothing — its
        /// target is a declared interface and is already in the sequence, so
        /// yielding it again would visit one shape twice.
        pub fn shapes(&self) -> impl Iterator<Item = InterfaceShape<'_>> {
            let named = self.interfaces.iter().map(|interface| InterfaceShape {
                name: &interface.name,
                interface,
                service: None,
            });
            let inline =
                self.services
                    .iter()
                    .filter_map(|service| match service.shape.as_ref()? {
                        service::Shape::Inline(interface) => Some(InterfaceShape {
                            name: &service.name,
                            interface,
                            service: Some(service),
                        }),
                        service::Shape::InterfaceRef(_) => None,
                    });
            named.chain(inline)
        }
    }

    /// Every package named by a type reference in `package`.
    ///
    /// A resolved type-reference string is the fully qualified `pkg.Name` for
    /// a cross-package reference and the bare `Name` for a same-package one,
    /// never an import alias — the canonical form stated in
    /// `proto/ridl/ir/v2/ir.proto`, which also enumerates the fields carrying
    /// one. **That enumeration and this walk are edited together.** A
    /// reference-bearing field added there and not read here makes the package
    /// it names invisible to every caller asking what a package depends on.
    ///
    /// Every `oneof` below is matched exhaustively with no wildcard arm, so a
    /// variant added later fails to compile here rather than going unread.
    pub fn referenced_packages(package: &Package) -> std::collections::BTreeSet<String> {
        let mut found = std::collections::BTreeSet::new();
        for decl in &package.decls {
            walk_decl(decl, &mut found);
        }
        for interface in &package.interfaces {
            for interaction in &interface.interactions {
                walk_decl(interaction, &mut found);
            }
        }
        for service in &package.services {
            match &service.shape {
                Some(service::Shape::InterfaceRef(reference)) => qualifier(reference, &mut found),
                Some(service::Shape::Inline(interface)) => {
                    for interaction in &interface.interactions {
                        walk_decl(interaction, &mut found);
                    }
                }
                None => {}
            }
        }
        found
    }

    /// Records the package qualifier of a dotted reference. A bare reference
    /// is same-package and contributes nothing.
    fn qualifier(reference: &str, found: &mut std::collections::BTreeSet<String>) {
        if let Some((package, _)) = reference.rsplit_once('.') {
            found.insert(package.to_string());
        }
    }

    /// Records every reference in one declaration — a package-level one or an
    /// interaction inside an interface, which share the `Decl` envelope.
    fn walk_decl(decl: &Decl, found: &mut std::collections::BTreeSet<String>) {
        match &decl.kind {
            Some(decl::Kind::TypeDef(type_def)) => walk_type_def(type_def, found),
            Some(decl::Kind::ConstDef(const_def)) => {
                if let Some(reference) = &const_def.type_ref {
                    qualifier(reference, found);
                }
            }
            Some(decl::Kind::StructDef(struct_def)) => {
                for member in &struct_def.members {
                    match &member.member {
                        Some(struct_member::Member::Field(field)) => {
                            if let Some(field_type) = &field.r#type {
                                walk_field_type(field_type, found);
                            }
                        }
                        // A tombstone occupies an ordinal and names no type.
                        Some(struct_member::Member::Reserved(_)) | None => {}
                    }
                }
            }
            // An enum's variants are integers; it names no type.
            Some(decl::Kind::EnumDef(_)) => {}
            Some(decl::Kind::EnumSetDef(enum_set)) => {
                if let Some(reference) = &enum_set.backing_enum {
                    qualifier(reference, found);
                }
            }
            Some(decl::Kind::UnionDef(union_def)) => {
                for arm in &union_def.arms {
                    qualifier(&arm.type_ref, found);
                }
            }
            Some(decl::Kind::SignalDef(signal)) => qualifier(&signal.payload, found),
            Some(decl::Kind::EventDef(event)) => qualifier(&event.payload, found),
            Some(decl::Kind::CommandDef(command)) => {
                for param in &command.params {
                    if let Some(field_type) = &param.r#type {
                        walk_field_type(field_type, found);
                    }
                }
            }
            Some(decl::Kind::QueryDef(query)) => {
                for param in &query.params {
                    if let Some(field_type) = &param.r#type {
                        walk_field_type(field_type, found);
                    }
                }
                if let Some(return_type) = &query.return_type {
                    walk_return_type(return_type, found);
                }
            }
            Some(decl::Kind::FixedDef(fixed)) => {
                if let Some(field_type) = &fixed.payload {
                    walk_field_type(field_type, found);
                }
            }
            // A tombstone occupies an ordinal and names no type.
            Some(decl::Kind::ReservedSlot(_)) | None => {}
        }
    }

    /// The recursive half: a reference is reachable at arbitrary depth through
    /// tuples, arrays, maps, inline scalars, and streams.
    fn walk_field_type(field_type: &FieldType, found: &mut std::collections::BTreeSet<String>) {
        match &field_type.kind {
            Some(field_type::Kind::Named(reference)) => qualifier(reference, found),
            // A primitive names no package.
            Some(field_type::Kind::Primitive(_)) => {}
            Some(field_type::Kind::InlineScalar(type_def)) => walk_type_def(type_def, found),
            Some(field_type::Kind::Tuple(tuple)) => {
                for field in &tuple.fields {
                    if let Some(inner) = &field.r#type {
                        walk_field_type(inner, found);
                    }
                }
            }
            Some(field_type::Kind::Array(array)) => {
                if let Some(element) = &array.element {
                    walk_field_type(element, found);
                }
            }
            Some(field_type::Kind::Map(map)) => {
                if let Some(key) = &map.key {
                    walk_field_type(key, found);
                }
                if let Some(value) = &map.value {
                    walk_field_type(value, found);
                }
            }
            Some(field_type::Kind::Stream(stream)) => match &stream.element {
                Some(stream_type::Element::Named(reference)) => qualifier(reference, found),
                // STRING or BYTES only; names no package.
                Some(stream_type::Element::Primitive(_)) | None => {}
            },
            None => {}
        }
    }

    /// A `TypeDef`'s only reference is the constant a `match` bound names.
    fn walk_type_def(type_def: &TypeDef, found: &mut std::collections::BTreeSet<String>) {
        if let Some(constraint) = &type_def.constraint
            && let Some(reference) = &constraint.pattern_const
        {
            qualifier(reference, found);
        }
    }

    fn walk_return_type(return_type: &ReturnType, found: &mut std::collections::BTreeSet<String>) {
        match &return_type.kind {
            Some(return_type::Kind::Value(field_type)) => walk_field_type(field_type, found),
            Some(return_type::Kind::Fallible(fallible)) => {
                qualifier(&fallible.ok, found);
                qualifier(&fallible.err, found);
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod v2_round_trip {
    use crate::v2;

    /// Wraps an interaction kind in the shared `Decl` envelope. Visibility
    /// and `is_error` stay unset on interactions (ridl §14.1); the ordinal is
    /// the 1-based declaration order across all interactions of the
    /// enclosing interface (ridl §11).
    fn interaction(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            visibility: v2::Visibility::Unspecified as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            ordinal,
            kind: Some(kind),
        }
    }

    fn named_type(name: &str) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(name.to_string())),
        }
    }

    fn stream_of(element: v2::stream_type::Element) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                element: Some(element),
            })),
        }
    }

    /// A representative ridl package: one interface holding all five
    /// interaction kinds plus a reserved tombstone (ordinals 1–6, the
    /// tombstone counted, ridl §11), a strict-periodic and a defaulted
    /// range timing, a fallible query, and two services — a named
    /// reference and an inline shape holding a stream query.
    fn fixture() -> v2::Package {
        // signal speed : Speed @10ms — strict periodic stores the period
        // in both bounds (ADR-0008 decision 12).
        let speed = v2::SignalDef {
            payload: "Speed".to_string(),
            declared_init: None,
            init: Some(v2::InitValue {
                derivable: true,
                value: Some("0.0".to_string()),
            }),
            timing: Some(v2::Timing {
                mode: v2::TimingMode::StrictPeriodic as i32,
                min_us: Some("10000".to_string()),
                max_us: Some("10000".to_string()),
                default_applied: false,
            }),
        };

        // event doorOpened : DoorEvent — untimed in source, so the
        // configured default range is resolved at compile time (ridl §9.1).
        let door_opened = v2::EventDef {
            payload: "DoorEvent".to_string(),
            timing: Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("20000".to_string()),
                max_us: Some("500000".to_string()),
                default_applied: true,
            }),
        };

        // command setTarget(target : Speed) [ require target >= speed ]
        let set_target = v2::CommandDef {
            params: vec![v2::Param {
                name: "target".to_string(),
                r#type: Some(named_type("Speed")),
            }],
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Require as i32,
                source: "target >= speed".to_string(),
                signal_refs: vec!["speed".to_string()],
                param_refs: vec!["target".to_string()],
                uses_result: false,
                observer_id: "VehicleStatus.setTarget.require[0]".to_string(),
            }],
        };

        // query fetchFaults(page : PageSpec) : FaultPage | DiagError
        //   [ ensure result.count <= page.limit ]
        let fetch_faults = v2::QueryDef {
            params: vec![v2::Param {
                name: "page".to_string(),
                r#type: Some(named_type("PageSpec")),
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Ensure as i32,
                source: "result.count <= page.limit".to_string(),
                signal_refs: Vec::new(),
                param_refs: vec!["page".to_string()],
                uses_result: true,
                observer_id: "VehicleStatus.fetchFaults.ensure[0]".to_string(),
            }],
        };

        // fixed vin : Vin
        let vin = v2::FixedDef {
            payload: Some(named_type("Vin")),
        };

        let vehicle_status = v2::Interface {
            name: "VehicleStatus".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: "Vehicle status contract".to_string(),
            labels: Vec::new(),
            deprecated: None,
            interactions: vec![
                interaction("speed", 1, v2::decl::Kind::SignalDef(speed)),
                interaction("doorOpened", 2, v2::decl::Kind::EventDef(door_opened)),
                // reserved legacyMode — the tombstone keeps ordinal 3
                // occupied in the one interaction sequence (ridl §11).
                v2::Decl {
                    ordinal: 3,
                    kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
                        ordinal: 3,
                        name: Some("legacyMode".to_string()),
                        value: None,
                    })),
                    ..interaction("", 3, v2::decl::Kind::ReservedSlot(v2::Reserved::default()))
                },
                interaction("setTarget", 4, v2::decl::Kind::CommandDef(set_target)),
                interaction("fetchFaults", 5, v2::decl::Kind::QueryDef(fetch_faults)),
                interaction("vin", 6, v2::decl::Kind::FixedDef(vin)),
            ],
        };

        // query tailLogs(pattern : <string>) : <LogLine> — a stream param
        // and a stream return (ridl §12), inside the inline service shape.
        let tail_logs = v2::QueryDef {
            params: vec![v2::Param {
                name: "pattern".to_string(),
                r#type: Some(stream_of(v2::stream_type::Element::Primitive(
                    v2::PrimitiveType::String as i32,
                ))),
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(stream_of(
                    v2::stream_type::Element::Named("LogLine".to_string()),
                ))),
            }),
            contracts: Vec::new(),
        };

        // service veh.adas.status : VehicleStatus — named reference.
        let status_service = v2::Service {
            name: "veh.adas.status".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            shape: Some(v2::service::Shape::InterfaceRef(
                "VehicleStatus".to_string(),
            )),
        };
        // service veh.adas.logs { … } — inline shape, Interface.name == ""
        // (ridl §14.5).
        let logs_service = v2::Service {
            name: "veh.adas.logs".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            shape: Some(v2::service::Shape::Inline(v2::Interface {
                name: String::new(),
                visibility: v2::Visibility::Unspecified as i32,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                interactions: vec![interaction(
                    "tailLogs",
                    1,
                    v2::decl::Kind::QueryDef(tail_logs),
                )],
            })),
        };

        v2::Package {
            name: "veh.adas".to_string(),
            // One typl declaration proves the verbatim v1 surface rides
            // along unchanged in v2; package-level declarations carry
            // ordinal 0.
            decls: vec![v2::Decl {
                name: "Speed".to_string(),
                visibility: v2::Visibility::Public as i32,
                is_error: false,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                ordinal: 0,
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Unit("km/h".to_string())),
                    }),
                    constraint: None,
                    declared_init: None,
                    init: None,
                    width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
                })),
            }],
            interfaces: vec![vehicle_status],
            services: vec![status_service, logs_service],
        }
    }

    /// The typl vocabulary surface the interaction fixture does not reach:
    /// the boxed `inlineScalar` oneof member, genuine 64-bit integer fields
    /// (array and map bounds, length bounds, `Reserved.value`,
    /// `EnumValue.value`), a tuple, a map, a union, an enum set, a constant,
    /// and a set `deprecated`. A second fixture, so each stays readable; the
    /// same round-trip tests drive both.
    fn vocabulary_fixture() -> v2::Package {
        fn decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
            v2::Decl {
                name: name.to_string(),
                visibility: v2::Visibility::Public as i32,
                is_error: false,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                ordinal: 0,
                kind: Some(kind),
            }
        }

        fn field(name: &str, ordinal: u32, field_type: v2::FieldType) -> v2::Field {
            v2::Field {
                name: name.to_string(),
                ordinal,
                r#type: Some(field_type),
                declared_init: None,
                init: None,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
            }
        }

        // const MAX_RETRY : integer = 24
        let max_retry = v2::ConstDef {
            type_ref: Some("integer".to_string()),
            value: "24".to_string(),
            regex: None,
        };

        // enum Gear { PARK = 1  DRIVE = 2  reserved 7 } — the tombstone
        // retires the integer value, a genuine int64 field.
        let gear = v2::EnumDef {
            values: vec![
                v2::EnumValue {
                    name: "PARK".to_string(),
                    value: 1,
                    doc: String::new(),
                },
                v2::EnumValue {
                    name: "DRIVE".to_string(),
                    value: 2,
                    doc: String::new(),
                },
            ],
            reserved: vec![v2::Reserved {
                ordinal: 0,
                name: None,
                value: Some(7),
            }],
        };

        // enumset Warnings { LOW_FUEL = 0  ICE_RISK = 33 } — the standalone
        // form; bit 33 forces the u64 width and is a genuine int64 value.
        let warnings = v2::EnumSetDef {
            backing_enum: None,
            bits: vec![
                v2::EnumValue {
                    name: "LOW_FUEL".to_string(),
                    value: 0,
                    doc: String::new(),
                },
                v2::EnumValue {
                    name: "ICE_RISK".to_string(),
                    value: 33,
                    doc: String::new(),
                },
            ],
            width: v2::IntWidth::U64 as i32,
        };

        // type PlateText : string [1..86] — character length bounds, two
        // genuine uint64 fields behind proto3 `optional`.
        let plate_text = v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                min: None,
                max: None,
                step: None,
                len_min: Some(1),
                len_max: Some(86),
                pattern: None,
                pattern_const: None,
            }),
            declared_init: None,
            init: None,
            width: None,
        };

        // union Sample { speed : Speed  gear : Gear }
        let sample = v2::UnionDef {
            arms: vec![
                v2::UnionArm {
                    name: "speed".to_string(),
                    ordinal: 1,
                    type_ref: "Speed".to_string(),
                    doc: String::new(),
                },
                v2::UnionArm {
                    name: "gear".to_string(),
                    ordinal: 2,
                    type_ref: "Gear".to_string(),
                    doc: String::new(),
                },
            ],
            is_result: false,
            reserved: Vec::new(),
        };

        // retries : integer [0..24] = 3 — the boxed `inlineScalar` oneof
        // member: the committed regression guard for ADR-0014 Open item 2,
        // which established that the Rust-side `Box` is invisible to the
        // reflection path. The enclosing field carries the init; the nested
        // TypeDef's stays unset.
        let retries = v2::Field {
            declared_init: Some("3".to_string()),
            init: Some(v2::InitValue {
                derivable: true,
                value: Some("3".to_string()),
            }),
            ..field(
                "retries",
                1,
                v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                        backing: Some(v2::Backing {
                            kind: Some(v2::backing::Kind::Primitive(
                                v2::PrimitiveType::Integer as i32,
                            )),
                        }),
                        constraint: Some(v2::Constraint {
                            min: Some("0".to_string()),
                            max: Some("24".to_string()),
                            step: None,
                            len_min: None,
                            len_max: None,
                            pattern: None,
                            pattern_const: None,
                        }),
                        declared_init: None,
                        init: None,
                        width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
                    }))),
                },
            )
        };

        // position : (x : Speed, y : Speed) — an anonymous named-field
        // composite (typl §11).
        let position = field(
            "position",
            2,
            v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                    fields: vec![
                        v2::TupleField {
                            name: "x".to_string(),
                            r#type: Some(named_type("Speed")),
                        },
                        v2::TupleField {
                            name: "y".to_string(),
                            r#type: Some(named_type("Speed")),
                        },
                    ],
                })),
            },
        );

        // gears : [Gear; 1..4096] — array bounds are genuine uint64 fields.
        let gears = field(
            "gears",
            3,
            v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(named_type("Gear"))),
                    min: 1,
                    max: 4096,
                }))),
            },
        );

        // plates : { PlateText -> Gear } [0..53] — map bounds are genuine
        // uint64 fields. The field is deprecated, covering the optional
        // string on the Field envelope.
        let plates = v2::Field {
            deprecated: Some("superseded by gears".to_string()),
            ..field(
                "plates",
                4,
                v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                        key: Some(Box::new(named_type("PlateText"))),
                        value: Some(Box::new(named_type("Gear"))),
                        min: 0,
                        max: 53,
                    }))),
                },
            )
        };

        let snapshot = v2::StructDef {
            members: [retries, position, gears, plates]
                .into_iter()
                .map(|field| v2::StructMember {
                    member: Some(v2::struct_member::Member::Field(field)),
                })
                .collect(),
            fixed_layout: false,
        };

        v2::Package {
            name: "veh.vocab".to_string(),
            decls: vec![
                decl("MAX_RETRY", v2::decl::Kind::ConstDef(max_retry)),
                decl("Gear", v2::decl::Kind::EnumDef(gear)),
                decl("Warnings", v2::decl::Kind::EnumSetDef(warnings)),
                decl("PlateText", v2::decl::Kind::TypeDef(plate_text)),
                // The union is deprecated — the optional string on the Decl
                // envelope.
                v2::Decl {
                    deprecated: Some("use Snapshot".to_string()),
                    ..decl("Sample", v2::decl::Kind::UnionDef(sample))
                },
                decl("Snapshot", v2::decl::Kind::StructDef(snapshot)),
            ],
            interfaces: Vec::new(),
            services: Vec::new(),
        }
    }

    #[test]
    fn protobuf_round_trip_preserves_package() {
        let package = fixture();

        let buf = v2::to_binary(&package);
        let decoded = v2::from_binary(buf.as_slice()).expect("decode must succeed");

        assert_eq!(package, decoded);

        // The vocabulary fixture rides the same round trip.
        let vocabulary = vocabulary_fixture();
        let decoded_vocabulary =
            v2::from_binary(v2::to_binary(&vocabulary).as_slice()).expect("decode must succeed");
        assert_eq!(vocabulary, decoded_vocabulary);

        let interface = &decoded.interfaces[0];
        let ordinals: Vec<u32> = interface.interactions.iter().map(|d| d.ordinal).collect();
        assert_eq!(
            ordinals,
            [1, 2, 3, 4, 5, 6],
            "one ordinal sequence, tombstone counted (ridl §11)"
        );
        let Some(v2::decl::Kind::ReservedSlot(tombstone)) = &interface.interactions[2].kind else {
            panic!("ordinal 3 must decode as a reserved tombstone");
        };
        assert_eq!(tombstone.name.as_deref(), Some("legacyMode"));
        let Some(v2::service::Shape::Inline(inline)) = &decoded.services[1].shape else {
            panic!("veh.adas.logs must decode as an inline shape");
        };
        assert_eq!(inline.name, "", "an inline shape carries no name");
    }

    #[test]
    fn json_round_trip_preserves_package() {
        for package in [fixture(), vocabulary_fixture()] {
            let json = v2::to_json_pretty(&package).expect("the fixture serializes as IR JSON");
            let decoded = v2::from_json(&json).expect("json deserialization must succeed");

            assert_eq!(package, decoded);
        }
    }

    /// The prototext read path (ADR-0014 decision 7): both fixtures survive
    /// `to_text_format` then `from_text_format` unchanged. With the binary
    /// and JSON round trips above, this is what proves all three encodings
    /// carry the same IR.
    #[test]
    fn text_format_round_trip_preserves_package() {
        for package in [fixture(), vocabulary_fixture()] {
            let text = v2::to_text_format(&package).expect("the fixture serializes as prototext");
            let decoded = v2::from_text_format(&text).expect("prototext parsing must succeed");

            assert_eq!(package, decoded);
        }
    }

    /// The prototext options ADR-0014 decision 8 fixes — `pretty`,
    /// `skip_default_fields(false)`, `print_message_fields_in_index_order` —
    /// each asserted through a visible consequence, so silently dropping one
    /// fails here rather than changing every artifact unremarked. Any option
    /// set round-trips, which is why the round-trip test above cannot guard
    /// them.
    #[test]
    fn text_format_is_pretty_with_defaults_in_index_order() {
        let text = v2::to_text_format(&fixture()).expect("the fixture serializes as prototext");

        // pretty: nested messages are indented, one field per line.
        assert!(
            text.contains("\n  "),
            "pretty printing must indent nested fields, got: {text}"
        );
        // skip_default_fields(false): a field holding its default is present
        // (decision 2 — `ordinal: 0` is read, not inferred from absence).
        assert!(
            text.contains("is_error: false"),
            "a field holding its default must be emitted, got: {text}"
        );
        // print_message_fields_in_index_order: `name` is field 1 of
        // `Package`, so it opens the output.
        assert!(
            text.starts_with("name:"),
            "fields must print in schema index order, got: {text}"
        );
    }

    /// Parses emitted JSON the way ADR-0014 decision 11's conformance test
    /// requires: unknown fields rejected, trailing input rejected, and the
    /// result transcoded into the generated types.
    fn strict_parse(json: &str) -> v2::Package {
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let dynamic = prost_reflect::DynamicMessage::deserialize_with_options(
            v2::package_descriptor(),
            &mut deserializer,
            &prost_reflect::DeserializeOptions::new().deny_unknown_fields(true),
        )
        .expect("a strict conformant parser must accept the emitted JSON");
        deserializer.end().expect("no trailing input");
        dynamic
            .transcode_to()
            .expect("the strictly parsed message transcodes into the generated types")
    }

    /// The conformance claim of ADR-0014 decision 11: a conformant protobuf
    /// JSON parser configured to reject unknown fields accepts the emitted
    /// JSON. Re-reading tests that claim itself; asserting on the rendered
    /// text would only restate the serializer's behaviour back to itself.
    #[test]
    fn emitted_json_survives_a_strict_conformant_parse() {
        for package in [fixture(), vocabulary_fixture()] {
            let json = v2::to_json_pretty(&package).expect("the fixture serializes as IR JSON");
            assert_eq!(package, strict_parse(&json));
        }
    }

    #[test]
    fn json_renders_timing_bounds_and_fallible_arms_exactly() {
        let json = v2::to_json_pretty(&fixture()).expect("the fixture serializes as IR JSON");

        // Exactness is visible: timing bounds are exact-decimal microsecond
        // strings, never floating-point numbers (ADR-0008 decision 12) —
        // under the canonical lowerCamelCase field name (ADR-0014 decision 1).
        assert!(
            json.contains(r#""minUs": "10000""#),
            "the timing bound must be a JSON string, got: {json}"
        );
        // Both arms of the inline T | E return are visible by name.
        assert!(
            json.contains(r#""ok": "FaultPage""#),
            "the ok arm must render, got: {json}"
        );
        assert!(
            json.contains(r#""err": "DiagError""#),
            "the err arm must render, got: {json}"
        );
    }

    /// ADR-0014 decision 8's stringification, tested on genuine 64-bit
    /// fields. The timing assertion above proves nothing about it —
    /// `Timing.min_us` is `optional string` in the schema — so the claim
    /// needs fields whose wire type actually is `uint64` or `int64`.
    #[test]
    fn json_renders_64_bit_integer_fields_as_strings() {
        let json = v2::to_json_pretty(&vocabulary_fixture())
            .expect("the vocabulary fixture serializes as IR JSON");

        // uint64: the array's upper bound.
        assert!(
            json.contains(r#""max": "4096""#),
            "an array bound must be a JSON string, got: {json}"
        );
        // uint64 behind proto3 `optional`: the character length bound.
        assert!(
            json.contains(r#""lenMax": "86""#),
            "a length bound must be a JSON string, got: {json}"
        );
        // int64: the retired enum value and the enum-set bit position.
        assert!(
            json.contains(r#""value": "7""#),
            "a retired enum value must be a JSON string, got: {json}"
        );
        assert!(
            json.contains(r#""value": "33""#),
            "an enum-set bit position must be a JSON string, got: {json}"
        );
    }

    #[test]
    fn fallible_transport_identity_follows_the_derivation_rule() {
        // The ADR-0008 decision 4 rule: interface + interaction ordinal +
        // both arm references, in that order.
        let fallible = v2::FallibleType {
            ok: "FaultPage".to_string(),
            err: "DiagError".to_string(),
        };
        assert_eq!(
            v2::fallible_transport_identity("VehicleStatus", 9, &fallible),
            "VehicleStatus#9:FaultPage|DiagError"
        );

        // Derived from the fixture: the fallible query sits at ordinal 5.
        let package = fixture();
        let interface = &package.interfaces[0];
        let query_decl = &interface.interactions[4];
        let Some(v2::decl::Kind::QueryDef(query)) = &query_decl.kind else {
            panic!("ordinal 5 must be the fallible query");
        };
        let Some(v2::return_type::Kind::Fallible(arms)) = &query.return_type.as_ref().unwrap().kind
        else {
            panic!("fetchFaults must return a fallible type");
        };
        assert_eq!(
            v2::fallible_transport_identity(&interface.name, query_decl.ordinal, arms),
            "VehicleStatus#5:FaultPage|DiagError"
        );
    }

    /// `Package::shapes` yields the named interfaces first, then the inline
    /// shapes of the services — and each shape carries the name it is known by
    /// OUTSIDE the package. The fixture's inline shape has `Interface.name ==
    /// ""` by construction, so a walk that yielded the interface bare would
    /// hand every consumer the empty string; two of the six E2 defects were
    /// exactly that.
    #[test]
    fn shapes_walks_named_interfaces_and_inline_service_shapes() {
        let package = fixture();
        let walk: Vec<(&str, bool, usize)> = package
            .shapes()
            .map(|shape| {
                (
                    shape.name,
                    shape.is_inline(),
                    shape.interface.interactions.len(),
                )
            })
            .collect();
        assert_eq!(
            walk,
            [("VehicleStatus", false, 6), ("veh.adas.logs", true, 1)],
            "the named interface, then the inline shape under the service's \
             dotted name",
        );

        // The fixture's third shape-bearing declaration is `service
        // veh.adas.status : VehicleStatus`, which names an interface already in
        // the walk. Yielding it too would visit `VehicleStatus` twice.
        assert_eq!(package.services.len(), 2, "one reference form, one inline");
        assert!(
            !package
                .shapes()
                .any(|shape| shape.name == "veh.adas.status"),
            "a service naming an interface contributes no shape of its own",
        );
    }

    /// The owning service is carried because `Service.visibility` is the
    /// authoritative one: an inline shape's own field is
    /// `VISIBILITY_UNSPECIFIED` by construction, which is not "internal" and
    /// not "public".
    #[test]
    fn shape_visibility_reads_the_owning_service_for_an_inline_shape() {
        let package = fixture();
        let shapes: Vec<v2::InterfaceShape<'_>> = package.shapes().collect();

        let named = shapes[0];
        assert!(named.service.is_none());
        assert_eq!(named.visibility(), v2::Visibility::Public as i32);
        assert_eq!(named.visibility(), named.interface.visibility);

        let inline = shapes[1];
        assert_eq!(
            inline.interface.visibility,
            v2::Visibility::Unspecified as i32,
            "the trap: an inline shape's own visibility field is unset",
        );
        assert_eq!(
            inline.service.expect("an inline shape has an owner").name,
            "veh.adas.logs",
        );
        assert_eq!(
            inline.visibility(),
            v2::Visibility::Public as i32,
            "the accessor reads the owning service's, never the unset field",
        );
    }

    /// A package with no service at all still walks its interfaces, and a
    /// package with neither yields nothing — the emptiness both backends test
    /// for before emitting any interaction vocabulary.
    #[test]
    fn shapes_is_empty_only_when_the_package_declares_no_shape() {
        let mut package = fixture();
        package.services.clear();
        assert_eq!(package.shapes().count(), 1);

        package.interfaces.clear();
        assert_eq!(package.shapes().count(), 0);
    }

    /// A dotted reference contributes its qualifier; a bare one contributes
    /// nothing. Every recursive path through `walk_field_type` — array
    /// element, tuple field, map key, map value, stream element — carries a
    /// distinct qualifier, so no path's absence can hide behind another
    /// path's presence: deleting any one arm's body changes the expected set
    /// this test compares against, rather than leaving it unchanged.
    #[test]
    fn referenced_packages_finds_qualifiers_at_depth() {
        fn named(reference: &str) -> v2::FieldType {
            v2::FieldType {
                kind: Some(v2::field_type::Kind::Named(reference.to_string())),
                ..Default::default()
            }
        }

        fn fixed(payload: v2::FieldType) -> v2::decl::Kind {
            v2::decl::Kind::FixedDef(v2::FixedDef {
                payload: Some(payload),
            })
        }

        let package = v2::Package {
            name: "veh.cluster".to_string(),
            decls: vec![
                v2::Decl {
                    name: "Local".to_string(),
                    kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef {
                        payload: "Speed".to_string(),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                v2::Decl {
                    name: "Stamped".to_string(),
                    kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef {
                        payload: "ridl.std.Timestamp".to_string(),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                v2::Decl {
                    name: "ArrLabels".to_string(),
                    kind: Some(fixed(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                            element: Some(Box::new(named("veh.arr.Label"))),
                            min: 0,
                            max: 32,
                        }))),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                v2::Decl {
                    name: "TupThing".to_string(),
                    kind: Some(fixed(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                            fields: vec![v2::TupleField {
                                name: "x".to_string(),
                                r#type: Some(named("veh.tup.X")),
                            }],
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                v2::Decl {
                    name: "MapThing".to_string(),
                    kind: Some(fixed(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                            key: Some(Box::new(named("veh.key.X"))),
                            value: Some(Box::new(named("veh.val.X"))),
                            min: 0,
                            max: 8,
                        }))),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                v2::Decl {
                    name: "StreamThing".to_string(),
                    kind: Some(fixed(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                            element: Some(v2::stream_type::Element::Named(
                                "veh.strm.X".to_string(),
                            )),
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let found = v2::referenced_packages(&package);
        let expected: std::collections::BTreeSet<String> = [
            "ridl.std", "veh.arr", "veh.tup", "veh.key", "veh.val", "veh.strm",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        assert_eq!(
            found, expected,
            "each recursive path must contribute its own distinct qualifier"
        );
        assert!(
            !found.contains("Speed") && !found.contains("veh.cluster"),
            "a bare reference contributes no package: {found:?}",
        );
    }

    /// An empty package references nothing — the negative case the emit rule in
    /// `ridlc` depends on.
    #[test]
    fn referenced_packages_is_empty_without_references() {
        let package = v2::Package {
            name: "veh.solo".to_string(),
            ..Default::default()
        };
        assert!(v2::referenced_packages(&package).is_empty());
    }

    /// Below prost's recursion limit at two message levels per nesting level
    /// — the depth ADR-0014 decision 12 measured as round-tripping correctly.
    const NESTING_BELOW_LIMIT: usize = 45;
    /// Past the limit today. The tests assert the outcome — an error, never a
    /// panic — not the exact threshold, so a prost release that moves the
    /// limit moves these constants, not the assertions.
    const NESTING_PAST_LIMIT: usize = 60;

    /// One declaration whose payload nests `depth` levels of inline arrays —
    /// each level costs two message levels on the wire (`FieldType` plus
    /// `ArrayType`), the arithmetic ADR-0014 decision 12 records against
    /// prost's recursion limit.
    fn nested_package(depth: usize) -> v2::Package {
        let mut payload = v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Primitive(
                v2::PrimitiveType::Integer as i32,
            )),
        };
        for _ in 0..depth {
            payload = v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(payload)),
                    min: 1,
                    max: 1,
                }))),
            };
        }
        v2::Package {
            name: "veh.deep".to_string(),
            decls: vec![v2::Decl {
                name: "deep".to_string(),
                kind: Some(v2::decl::Kind::FixedDef(v2::FixedDef {
                    payload: Some(payload),
                })),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// The JSON form of [`nested_package`], built by hand: past the limit the
    /// serializer rejects the package, so its JSON cannot come from
    /// [`v2::to_json_pretty`]. The chosen depth stays under `serde_json`'s
    /// own recursion limit of 128, so the failure exercised is the transcode
    /// out of the dynamic message, not the JSON parse.
    fn nested_json(depth: usize) -> String {
        let mut payload = r#"{"primitive": "PRIMITIVE_TYPE_INTEGER"}"#.to_string();
        for _ in 0..depth {
            payload = format!(r#"{{"array": {{"element": {payload}, "min": "1", "max": "1"}}}}"#);
        }
        format!(
            r#"{{"name": "veh.deep", "decls": [{{"name": "deep", "fixedDef": {{"payload": {payload}}}}}]}}"#
        )
    }

    /// ADR-0014 decision 12: nesting past the transcoder's recursion limit is
    /// reachable from legal source, so serialization reports it as an error
    /// instead of panicking.
    #[test]
    fn json_serialization_past_the_nesting_limit_returns_an_error() {
        let err = v2::to_json_pretty(&nested_package(NESTING_PAST_LIMIT))
            .expect_err("serialization past the recursion limit must fail, not panic");
        assert!(
            err.to_string().contains("recursion limit"),
            "the error must name the nesting limit as the known cause, got: {err}"
        );
    }

    /// The read direction of the same defect: the transcode out of the
    /// dynamic message crosses the same recursion limit, and `from_json` maps
    /// it into its error return instead of expecting on it (ADR-0014
    /// decision 12).
    #[test]
    fn json_parse_past_the_nesting_limit_returns_an_error() {
        let error = v2::from_json(&nested_json(NESTING_PAST_LIMIT))
            .expect_err("parsing past the recursion limit must fail, not panic");

        // Assert *which* limit was reached. `serde_json` carries its own
        // recursion limit of 128, and this input clears the transcoding
        // decoder's limit by only one nesting level, so an `is_err()`
        // assertion alone would keep passing if the constant were raised —
        // while silently testing `serde_json`'s parser instead of the
        // transcode path this test exists to pin. prost says "recursion limit
        // reached"; `serde_json` says "recursion limit exceeded".
        let message = error.to_string();
        assert!(
            message.contains("recursion limit reached"),
            "the transcoding decoder's limit must be the one reached, not \
             serde_json's own; got: {message}"
        );
    }

    /// The bound must not tighten silently: below the limit the package still
    /// serializes and round-trips, so a change that narrows what the JSON
    /// surface accepts fails here.
    #[test]
    fn json_round_trip_below_the_nesting_limit_succeeds() {
        let package = nested_package(NESTING_BELOW_LIMIT);
        let json = v2::to_json_pretty(&package)
            .expect("below the recursion limit, serialization succeeds");
        let decoded = v2::from_json(&json).expect("below the recursion limit, parsing succeeds");
        assert_eq!(package, decoded);
    }

    /// The prototext form of [`nested_package`], built by hand for the same
    /// reason [`nested_json`] is: past the limit the serializer rejects the
    /// package, so its prototext cannot come from [`v2::to_text_format`].
    fn nested_text(depth: usize) -> String {
        let mut payload = "primitive: PRIMITIVE_TYPE_INTEGER".to_string();
        for _ in 0..depth {
            payload = format!("array {{ element {{ {payload} }} min: 1 max: 1 }}");
        }
        format!(
            r#"name: "veh.deep" decls {{ name: "deep" fixed_def {{ payload {{ {payload} }} }} }}"#
        )
    }

    /// The prototext write path carries the same recursion-limit failure mode
    /// as JSON — both go through the one transcode (ADR-0014 decision 12) —
    /// and reports it as an error naming its own encoding, never a panic.
    #[test]
    fn text_serialization_past_the_nesting_limit_returns_an_error() {
        let err = v2::to_text_format(&nested_package(NESTING_PAST_LIMIT))
            .expect_err("serialization past the recursion limit must fail, not panic");
        let message = err.to_string();
        assert!(
            message.contains("recursion limit"),
            "the error must name the nesting limit as the known cause, got: {message}"
        );
        assert!(
            message.contains("prototext"),
            "the error must name the encoding that failed, got: {message}"
        );
    }

    /// Runs `test` on a thread whose stack fits the text-format parser at
    /// these depths. `prost-reflect`'s text parser recurses once per message
    /// level with debug-build frames large enough that the default 2 MiB
    /// test-thread stack overflows near 45 array levels — under prost's own
    /// recursion limit, so the depths [`NESTING_BELOW_LIMIT`] and
    /// [`NESTING_PAST_LIMIT`] pin are unreachable on that stack. The
    /// production paths are unaffected: the toolchain writes prototext and
    /// never parses it (`ridl diff` and the baselines stay `.ir.json`,
    /// ADR-0014 decision 5), and the writer is capped by the transcode
    /// before it can recurse that deep.
    fn with_parser_stack(test: impl FnOnce() + Send + 'static) {
        let outcome = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(test)
            .expect("spawn the large-stack test thread")
            .join();
        if let Err(payload) = outcome {
            std::panic::resume_unwind(payload);
        }
    }

    /// The read direction: the text-format parser itself has no depth limit,
    /// so the failure is the transcode out of the dynamic message, mapped
    /// into the error return instead of expected on (ADR-0014 decision 12).
    #[test]
    fn text_parse_past_the_nesting_limit_returns_an_error() {
        with_parser_stack(|| {
            let error = v2::from_text_format(&nested_text(NESTING_PAST_LIMIT))
                .expect_err("parsing past the recursion limit must fail, not panic");

            // Assert *which* stage failed: prost's transcoding decoder says
            // "recursion limit reached", and a parse-stage failure would
            // render through the `Parse` variant instead.
            let message = error.to_string();
            assert!(
                message.contains("recursion limit reached"),
                "the transcode out of the dynamic message must be the failing \
                 stage, got: {message}"
            );
        });
    }

    /// The prototext bound must not tighten silently either: below the limit
    /// the package still serializes and round-trips.
    #[test]
    fn text_round_trip_below_the_nesting_limit_succeeds() {
        with_parser_stack(|| {
            let package = nested_package(NESTING_BELOW_LIMIT);
            let text = v2::to_text_format(&package)
                .expect("below the recursion limit, serialization succeeds");
            let decoded =
                v2::from_text_format(&text).expect("below the recursion limit, parsing succeeds");
            assert_eq!(package, decoded);
        });
    }
}
