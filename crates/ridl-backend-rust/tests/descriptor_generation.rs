//! Descriptor-layer generation over the interaction-face fixture. Asserts that
//! `generate_face_with` emits the `Interface` and `Interaction` descriptors and the
//! interaction-kind traits over the runtime crate when `encodings` names a
//! codec, that the buffer constants are sized from the right payloads, that an
//! empty `encodings` emits the domain types only, and that the pipeline
//! `generate` stays clean of the face.

use ridl_backend_rust::{WireEncoding, generate, generate_face_with};
use ridl_ir::codegen::v1;

/// The `encodings` request option of the backend contract, with `value`.
fn encodings_option(value: &str) -> v1::BackendOption {
    v1::BackendOption {
        key: "encodings".to_string(),
        value: value.to_string(),
    }
}

#[path = "support/ir.rs"]
mod ir;

/// Whitespace-stripped copy, so an assertion is robust to how prettyplease
/// spaces a token stream.
fn dense(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The `<T as Payload<::ridl_rt::encoding::FlatBuffers>>` marker a buffer
/// constant sizes from, whitespace-stripped. The codec's own implementations
/// write `impl Payload<…> for T` and `<Self as Payload<…>>`, never
/// `<T as Payload<…>>` with the type's name, so the marker's presence for a
/// type name says that type is sized into a buffer. The `MAX_SIZE` suffix is
/// left off because prettyplease may wrap the path and insert a trailing comma
/// inside the generic arguments.
fn max_size_path(type_name: &str) -> String {
    dense(&format!(
        "<{type_name} as ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers"
    ))
}

/// Every `PayloadInfo.max_size` carries the model's own FlatBuffers bound
/// (`Payload.flatbuffers_max_size`, which `max_size` computed) in its
/// `flatbuffers` column and nothing else: `proto3` is `None` because this
/// backend emits no proto3 codec, and `repr_c` is `None` until the layout is defined
/// (driftsys/ridl#317). The payloads are read from the model the emitter reads, so
/// the number asserted is the model's, not one this test derives.
#[test]
fn encoded_sizes_carry_the_models_flatbuffers_bound() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&face);
    assert!(
        face.contains("FlatBuffers bound as the codegen model carries it"),
        "the descriptor doc comment states the sizes as filled, not deferred"
    );

    let model = ridl_ir::codegen::lower(&package, &[]);
    let mut seen = 0;
    for interface in &model.interfaces {
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                slot.occupant.as_ref()
            else {
                continue;
            };
            let payloads: Vec<&v1::Payload> = match interaction.shape.as_ref() {
                Some(v1::interaction::Shape::Signal(signal)) => signal.payload.iter().collect(),
                Some(v1::interaction::Shape::Event(event)) => event.payload.iter().collect(),
                Some(v1::interaction::Shape::Command(command)) => command.request.iter().collect(),
                Some(v1::interaction::Shape::Query(query)) => query
                    .request
                    .iter()
                    .chain(query.reply_payload.iter())
                    .collect(),
                Some(v1::interaction::Shape::Fixed(fixed)) => fixed.named.iter().collect(),
                None => Vec::new(),
            };
            for payload in payloads {
                let name = payload
                    .r#type
                    .as_ref()
                    .map(|reference| reference.reference.as_str())
                    .unwrap_or_default();
                let bound = payload
                    .flatbuffers_max_size
                    .unwrap_or_else(|| panic!("{name} is bounded in the fixture"));
                let expected = dense(&format!(
                    "::ridl_rt::contract::PayloadInfo {{
                        type_name: \"{name}\",
                        max_size: ::ridl_rt::contract::EncodedSizes {{
                            proto3: ::core::option::Option::None,
                            flatbuffers: ::core::option::Option::Some({bound}),
                            repr_c: ::core::option::Option::None,
                        }},
                    }}"
                ));
                assert!(d.contains(&expected), "{name}: expected {expected}");
                // The codec writes `MAX_SIZE` from another model field
                // (`FbRoot.bound`, `payload_impl` in `codec.rs`); the column
                // and the constant must carry the same number.
                assert_eq!(
                    codec_max_size(&d, name),
                    Some(bound),
                    "{name}: the codec's MAX_SIZE is the descriptor's flatbuffers column"
                );
                seen += 1;
            }
        }
    }
    assert!(
        seen >= 10,
        "the fixture's ten payload positions, over six types, were checked, saw {seen}"
    );
    assert!(
        !d.contains("flatbuffers:::core::option::Option::None"),
        "every payload of the fixture is sized"
    );
}

/// A payload the model carries no FlatBuffers bound for is emitted with
/// `None` in the `flatbuffers` column — not a number, and not the column of
/// another payload. The fixture's payloads are all bounded, so the model is
/// edited: `Horn.active`'s `Health` loses its bound before generation.
#[test]
fn a_payload_without_a_flatbuffers_bound_emits_none() {
    use ridl_ir::codegen::Backend as _;

    let package = ir::compile_fixture("interaction_face.ridl");
    let mut model = ridl_ir::codegen::lower(&package, &[]);
    let mut cleared = 0;
    for interface in &mut model.interfaces {
        for slot in &mut interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                slot.occupant.as_mut()
            else {
                continue;
            };
            let Some(v1::interaction::Shape::Signal(signal)) = interaction.shape.as_mut() else {
                continue;
            };
            let Some(payload) = signal.payload.as_mut() else {
                continue;
            };
            if payload.r#type.as_ref().map(|r| r.reference.as_str()) == Some("Health") {
                payload.flatbuffers_max_size = None;
                cleared += 1;
            }
        }
    }
    assert_eq!(cleared, 1, "`Horn.active` is the one `Health` signal");

    let request = v1::CodegenRequest {
        model: Some(model),
        options: vec![encodings_option("flatbuffers")],
        artifact_base: "face_demo".to_string(),
        ..Default::default()
    };
    let response = ridl_backend_rust::Backend.generate(&request);
    let text = response
        .files
        .iter()
        .find_map(|file| match &file.content {
            Some(v1::generated_file::Content::Text(text)) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a generated file, got {:?}", response.diagnostics));
    let d = dense(text);
    let expected = dense(
        "::ridl_rt::contract::PayloadInfo {
            type_name: \"Health\",
            max_size: ::ridl_rt::contract::EncodedSizes {
                proto3: ::core::option::Option::None,
                flatbuffers: ::core::option::Option::None,
                repr_c: ::core::option::Option::None,
            },
        }",
    );
    assert!(d.contains(&expected), "expected {expected}");
    assert_eq!(
        d.matches("flatbuffers:::core::option::Option::None")
            .count(),
        1,
        "every other payload keeps its bound"
    );
}

/// The `MAX_SIZE` the codec's `Payload<FlatBuffers>` impl for `type_name`
/// carries, read from the whitespace-stripped source between that impl's
/// header and the next impl header; `None` when the impl or the constant is
/// missing.
fn codec_max_size(dense_source: &str, type_name: &str) -> Option<u32> {
    const HEADER: &str = "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>for";
    const CONST: &str = "constMAX_SIZE:::core::primitive::usize=";
    let header = format!("{HEADER}{type_name}{{");
    let at = dense_source.find(&header)?;
    let body = &dense_source[at + header.len()..];
    let body = &body[..body.find(HEADER).unwrap_or(body.len())];
    let digits: String = body[body.find(CONST)? + CONST.len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[test]
fn the_fixture_compiles_clean() {
    let package = ir::compile_fixture("interaction_face.ridl");
    assert_eq!(package.name, "face.demo");
    assert_eq!(package.shapes().count(), 4);
}

#[test]
fn generate_face_emits_the_interface_and_interaction_descriptors() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&face);

    // One unit struct per interface, each with an Interface impl.
    assert!(d.contains("pubstructCabin;"), "Cabin unit struct");
    assert!(d.contains("pubstructHorn;"), "Horn unit struct");
    assert!(d.contains("impl::ridl_rt::contract::InterfaceforCabin"));
    assert!(d.contains("impl::ridl_rt::contract::InterfaceforHorn"));

    // The Interface trait's five constants.
    for constant in [
        "constCATALOG",
        "constNUMBER",
        "constPROVISIONAL",
        "constNAME",
        "constMEMBERS",
    ] {
        assert!(d.contains(constant), "missing interface {constant}");
    }
    assert!(d.contains("name:\"face.demo\""), "catalog package name");
    // The catalog hash is the model's `Catalog.hash` (ADR-0014 decision 15),
    // copied byte for byte, and not the all-zero placeholder.
    let model = ridl_ir::codegen::lower(&package, &[]);
    let hash = &model.catalog.as_ref().expect("a catalog").hash;
    assert!(
        hash.iter().any(|byte| *byte != 0),
        "the model's hash is not zero"
    );
    let bytes: Vec<String> = hash.iter().map(u8::to_string).collect();
    // prettyplease adds a trailing comma when it wraps the array.
    let emitted = format!("::ridl_rt::contract::CatalogHash([{}", bytes.join(","));
    assert!(
        d.contains(&format!("{emitted}])")) || d.contains(&format!("{emitted},])")),
        "the emitted catalog hash is the model's hash",
    );
    assert!(
        !d.contains("::ridl_rt::contract::CatalogHash([0u8;32])"),
        "no zero catalog hash placeholder",
    );

    // Number and provisional flag read straight from the IR, not invented.
    let cabin = package
        .shapes()
        .find(|shape| shape.name == "Cabin")
        .expect("Cabin interface")
        .interface;
    assert_eq!(cabin.number, 1);
    assert!(cabin.provisional);
    assert!(
        d.contains("::ridl_rt::contract::InterfaceNo(1)"),
        "Cabin number"
    );
    assert!(
        d.contains("::ridl_rt::contract::InterfaceNo(2)"),
        "Horn number"
    );
    assert!(
        d.contains("constPROVISIONAL:::core::primitive::bool=true"),
        "provisional from IR"
    );

    // Interaction descriptors: one unit struct plus an Interaction impl with
    // both required items.
    assert!(d.contains("pubstructCabinTemperature;"));
    assert!(d.contains("pubstructCabinWarning;"));
    assert!(d.contains("pubstructCabinSetLevel;"));
    assert!(d.contains("pubstructCabinAverage;"));
    assert!(d.contains("pubstructHornActive;"));
    assert!(d.contains("impl::ridl_rt::contract::InteractionforCabinTemperature"));
    assert!(d.contains("typeIface=Cabin;"), "Interaction::Iface");
    assert!(
        d.contains("constMEMBER:&'static::ridl_rt::contract::Member"),
        "Interaction::MEMBER",
    );
    assert!(
        d.contains("<Cabinas::ridl_rt::contract::Interface>::MEMBERS[0]"),
        "MEMBER points into MEMBERS by row index",
    );

    // The four interaction kinds the fixture carries, plus the signal-only one.
    assert!(d.contains("impl::ridl_rt::contract::SignalforCabinTemperature"));
    assert!(d.contains("impl::ridl_rt::contract::EventforCabinWarning"));
    assert!(d.contains("impl::ridl_rt::contract::CommandforCabinSetLevel"));
    assert!(d.contains("impl::ridl_rt::contract::QueryforCabinAverage"));
    assert!(d.contains("impl::ridl_rt::contract::SignalforHornActive"));

    // Payload and argument/reply associated types name the declared types.
    assert!(d.contains("typePayload=Temperature;"), "signal payload");
    assert!(d.contains("typePayload=Warning;"), "event payload");
    assert!(d.contains("typeArgs=Level;"), "command args");
    assert!(d.contains("typeArgs=Window;"), "query args");
    assert!(d.contains("typeReply=Average;"), "query reply");

    // The interaction kinds appear in the member rows.
    for kind in [
        "::ridl_rt::contract::Kind::Signal",
        "::ridl_rt::contract::Kind::Event",
        "::ridl_rt::contract::Kind::Command",
        "::ridl_rt::contract::Kind::Query",
    ] {
        assert!(d.contains(kind), "missing member kind {kind}");
    }

    // The encoded sizes are pinned payload by payload in
    // `encoded_sizes_carry_the_models_flatbuffers_bound`.

    // Ordinals and names are carried on the member rows.
    assert!(
        d.contains("ordinal:::ridl_rt::contract::Ordinal(3)"),
        "setLevel ordinal"
    );
    assert!(d.contains("name:\"setLevel\""), "member source name");
}

/// `CATALOG` carries the unit's name, the model's `Catalog.package`, not the
/// source package's: `face.demo` as a subpackage of unit `face` is generated
/// with `name: "face"`.
#[test]
fn the_catalog_name_is_the_unit_not_the_source_package() {
    let mut package = ir::compile_fixture("interaction_face.ridl");
    package.unit = "face".to_owned();
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&face);
    assert!(
        d.contains("name:\"face\""),
        "the catalog name is the unit:\n{face}"
    );
    assert!(
        !d.contains("name:\"face.demo\""),
        "the source package is not the catalog name:\n{face}"
    );
}

#[test]
fn generate_face_emits_the_contract_clause_bodies() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let d = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );

    // require level < 100 -> args.0 < 100 ; require window > 0 -> args.0 > 0 ;
    // ensure result >= 0 -> reply.0 >= 0.
    assert!(d.contains("args.0<100"), "command require translated");
    assert!(d.contains("args.0>0"), "query require translated");
    assert!(d.contains("reply.0>=0"), "query ensure translated");
    // The failing branch returns Err(()).
    assert!(d.contains("Err(())"), "a failing clause returns Err(())");
}

#[test]
fn the_interface_buffer_constant_is_the_max_argument_and_reply_size() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let d = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );

    assert!(
        d.contains("constMAX_BUFFER_SIZE:::core::primitive::usize"),
        "MAX_BUFFER_SIZE constant"
    );

    // The call argument and reply types are sized in.
    assert!(d.contains(&max_size_path("Level")), "command arg sized in");
    assert!(d.contains(&max_size_path("Window")), "query arg sized in");
    assert!(
        d.contains(&max_size_path("Average")),
        "query reply sized in"
    );
    // A signal payload is not an argument or a reply, so it is not sized in.
    assert!(
        !d.contains(&max_size_path("Temperature")),
        "a signal payload is not in the argument/reply buffer",
    );
}

#[test]
fn the_event_source_constant_is_the_max_event_size() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let d = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );

    assert!(
        d.contains("constEVENT_SOURCE_BUFFER_SIZE:::core::primitive::usize"),
        "EVENT_SOURCE_BUFFER_SIZE constant",
    );
    // The event payload is sized into the event-source buffer.
    assert!(
        d.contains(&max_size_path("Warning")),
        "event payload sized in"
    );
    // A signal-only interface's payload is not an event, so it is not sized in.
    assert!(
        !d.contains(&max_size_path("Health")),
        "a signal payload is not in the event-source buffer",
    );
}

/// The pipeline entry point emits the domain types and the payload codec, and
/// nothing of the interaction face.
///
/// `generate` emits the domain types only, so the runtime paths it names are
/// the constructors' and the conversions'. The permitted list below still
/// names the codec's paths — they were permitted while `generate` carried the
/// codec, and permitting them keeps this test about the one boundary it
/// guards: every runtime path belongs to the constructors or the codec, and
/// none belongs to the face. `generate_emits_types_only` is what pins that
/// the codec is absent.
#[test]
fn the_pipeline_generate_stays_clean_of_the_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let plain = generate(&package).expect("generate").rust_source;
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;

    assert_ne!(plain, face, "the two entry points must differ");
    // Every runtime item the pipeline entry point may name, spelled in full
    // and matched to the end of the item, so a name that only starts with a
    // permitted one does not pass. The first two are a named scalar's
    // constructor (typl value objects, Task 3); the rest are the codec's.
    // `flatbuffers` is matched by its module, because the codec names a
    // dozen reading and writing helpers from it and enumerating them here
    // would pin the helper set rather than the boundary this test is about.
    //
    // This is weaker than what the test once asserted, when the
    // only permitted items were `Violation` and `Rule`. It has to be: the
    // codec names `Payload`, `Ref`, `Encoded` and the two error types, and
    // `Ref` is the face's as well, so no list of `payload` items can
    // separate the two any more. The separation the test still makes is the
    // one that matters — `contract` and `port` are the face's alone, and the
    // two negative assertions below are what carry the invariant.
    let permitted_items = [
        "ridl_rt::payload::Violation",
        "ridl_rt::payload::Rule",
        "ridl_rt::payload::Payload",
        "ridl_rt::payload::Ref",
        "ridl_rt::payload::Encoded",
        "ridl_rt::payload::EncodeError",
        "ridl_rt::payload::VerifyError",
        "ridl_rt::payload::Malformed",
        // The trait every generated view implements, so that a field whose
        // accessor is `bytes` does not meet the fixed `bytes`
        // (driftsys/ridl#587).
        "ridl_rt::payload::View",
        "ridl_rt::encoding::FlatBuffers",
    ];
    let dense_plain = dense(&plain);
    for (index, _) in dense_plain.match_indices("ridl_rt") {
        let rest = &dense_plain[index..];
        if !dense_plain[..index].ends_with("::") {
            panic!(
                "the pipeline generate must name the runtime by absolute path, found `{}`",
                rest.chars().take(48).collect::<String>(),
            );
        }
        let permitted = rest.starts_with("ridl_rt::flatbuffers::")
            || permitted_items.iter().any(|item| {
                rest.strip_prefix(item).is_some_and(|after| {
                    !after.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            });
        assert!(
            permitted,
            "the pipeline generate must name no runtime path outside the constructors and the \
             codec, found `{}`",
            rest.chars().take(48).collect::<String>(),
        );
    }
    assert!(
        !plain.contains("impl ::ridl_rt::contract::Interface"),
        "the pipeline generate must emit no interface descriptor",
    );
    assert!(
        !dense_plain.contains("ridl_rt::port::"),
        "the pipeline generate must name no port",
    );
}

/// The codec reaches both entry points, and reaches the companion one from
/// the same emitter: the face is built over the FlatBuffers codec, so
/// the face is appended to the codec's own items rather than compiled over
/// implementations written for it.
#[test]
fn the_face_carries_the_flatbuffers_codec_once() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );

    let codec = "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>forWarning";
    assert_eq!(
        face.matches(codec).count(),
        1,
        "the face entry point emits one `Payload<FlatBuffers>` per root table, and exactly \
         once: the face emitter adds nothing the codec already emits",
    );
}

/// Neither entry point emits a `Wire` alias. The face entry point used to emit
/// one per package (design note D-11); `docs/technotes/rust-backend-name-collisions.md`
/// removed it (decision 5, driftsys/ridl#588), and every site that named it
/// writes `::ridl_rt::encoding::FlatBuffers`, so a declaration or an interface
/// named `Wire` collides with nothing.
#[test]
fn neither_entry_point_emits_a_wire_alias() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let plain = dense(&generate(&package).expect("generate").rust_source);
    let face = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );

    assert!(
        !face.contains("pubtypeWire"),
        "the face entry point emits no alias",
    );
    assert!(
        !plain.contains("pubtypeWire"),
        "the pipeline entry point emits no alias",
    );
    assert!(
        face.contains(&max_size_path("Level")),
        "the face sizes its buffers over the encoding's full path",
    );
}

/// The four things a codec brings with it, whitespace-stripped: a `Payload`
/// implementation, a FlatBuffers view, an interface descriptor and a face
/// module. A types-only package carries none of them.
const CODEC_MARKERS: [&str; 4] = [
    "Payload<",
    "FbView",
    "impl::ridl_rt::contract::Interface",
    "pubmodcabin",
];

/// An empty `encodings` emits the domain types only: the fixture's named
/// scalar is there, and no codec, no descriptor and no face is.
#[test]
fn no_encodings_emits_types_only() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate_face_with(&package, &[])
        .expect("generate_face_with")
        .rust_source;
    assert!(
        source.contains("pub struct Temperature("),
        "the domain types are emitted, got:\n{source}"
    );
    let d = dense(&source);
    for marker in CODEC_MARKERS {
        assert!(
            !d.contains(marker),
            "a types-only package must not contain `{marker}`, got:\n{source}"
        );
    }
}

/// `encodings` naming `flatbuffers` emits the codec, the descriptors and the
/// face beside the domain types.
#[test]
fn flatbuffers_emits_the_codec_descriptors_and_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&source);
    for marker in CODEC_MARKERS {
        assert!(
            d.contains(marker),
            "a package with the FlatBuffers codec must contain `{marker}`, got:\n{source}"
        );
    }
}

/// `generate` is the domain types only: no codec, whatever the package.
#[test]
fn generate_emits_types_only() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate(&package).expect("generate").rust_source;
    assert!(
        source.contains("pub struct Temperature("),
        "the domain types are emitted, got:\n{source}"
    );
    assert!(
        !source.contains("Payload<"),
        "`generate` emits no codec, got:\n{source}"
    );
}

/// `Backend::generate` over the fixture's model with `options`: the one
/// generated file's text, or the diagnostics when no file is written.
fn backend_response(options: Vec<v1::BackendOption>) -> Result<String, Vec<String>> {
    use ridl_ir::codegen::Backend as _;

    let package = ir::compile_fixture("interaction_face.ridl");
    let request = v1::CodegenRequest {
        model: Some(ridl_ir::codegen::lower(&package, &[])),
        options,
        artifact_base: "face_demo".to_string(),
        ..Default::default()
    };
    let response = ridl_backend_rust::Backend.generate(&request);
    let text = response.files.iter().find_map(|file| match &file.content {
        Some(v1::generated_file::Content::Text(text)) => Some(text.clone()),
        _ => None,
    });
    match text {
        Some(text) => Ok(text),
        None => Err(response
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()),
    }
}

/// The one refusal `Backend::generate` draws for `options`.
fn refused_options(options: Vec<v1::BackendOption>) -> String {
    let messages = backend_response(options).expect_err("the request is refused");
    assert_eq!(messages.len(), 1, "one refusal, got: {messages:?}");
    messages.into_iter().next().expect("one refusal")
}

/// A request with no option is a types-only build through the contract, the
/// way a manifest with no `[backend.rust]` table reaches the backend.
#[test]
fn the_backend_emits_types_only_without_the_option() {
    let text = backend_response(Vec::new()).expect("a file is written");
    assert!(text.contains("pub struct Temperature("), "got:\n{text}");
    let d = dense(&text);
    for marker in CODEC_MARKERS {
        assert!(!d.contains(marker), "got `{marker}` in:\n{text}");
    }
}

/// The option the contract reads is `encodings`; the former `wire-encoding`
/// key is refused like any other key, and the refusal names the one it reads.
#[test]
fn wire_encoding_is_refused() {
    let message = refused_options(vec![v1::BackendOption {
        key: "wire-encoding".to_string(),
        value: "flatbuffers".to_string(),
    }]);
    assert!(
        message.contains("`encodings`") && message.contains("`wire-encoding`"),
        "got: {message}"
    );
}

#[test]
fn an_unknown_key_is_refused() {
    let message = refused_options(vec![v1::BackendOption {
        key: "codec".to_string(),
        value: "flatbuffers".to_string(),
    }]);
    assert!(
        message.contains("takes one option, `encodings`") && message.contains("`codec`"),
        "got: {message}"
    );
}

/// An encoding this backend has no codec for yet is refused with the issue
/// that builds the codec, so a manifest author knows the name is right and
/// the codec is pending.
#[test]
fn proto3_is_refused_naming_its_issue() {
    let message = refused_options(vec![encodings_option("proto3")]);
    assert!(
        message.contains("`proto3`") && message.contains("driftsys/ridl#264"),
        "got: {message}"
    );
}

#[test]
fn repr_c_is_refused_naming_its_issue() {
    let message = refused_options(vec![encodings_option("repr-c")]);
    assert!(
        message.contains("`repr-c`") && message.contains("driftsys/ridl#317"),
        "got: {message}"
    );
}

#[test]
fn an_unknown_encoding_is_refused() {
    let message = refused_options(vec![encodings_option("msgpack")]);
    assert!(
        message.contains("`encodings` names `msgpack`"),
        "got: {message}"
    );
}

#[test]
fn a_repeated_encoding_is_refused() {
    let message = refused_options(vec![encodings_option("flatbuffers,flatbuffers")]);
    assert!(
        message.contains("`encodings` names `flatbuffers` twice"),
        "got: {message}"
    );
}

#[test]
fn an_empty_element_is_refused() {
    let message = refused_options(vec![encodings_option("flatbuffers,")]);
    assert!(
        message.contains("`encodings` names an empty element"),
        "got: {message}"
    );
}

/// The diagnostics `generate` returns for the fixture's model after `edit`,
/// asserting that no file is written. The request names the FlatBuffers
/// codec: the descriptors are what read the catalog hash, and a types-only
/// build emits none.
fn refused_with(edit: impl FnOnce(&mut ridl_ir::codegen::v1::Model)) -> Vec<String> {
    use ridl_ir::codegen::{Backend as _, v1};

    let package = ir::compile_fixture("interaction_face.ridl");
    let mut model = ridl_ir::codegen::lower(&package, &[]);
    edit(&mut model);
    let request = v1::CodegenRequest {
        model: Some(model),
        options: vec![encodings_option("flatbuffers")],
        artifact_base: "face_demo".to_string(),
        ..Default::default()
    };
    let response = ridl_backend_rust::Backend.generate(&request);
    assert!(response.files.is_empty(), "no file for a malformed model");
    response
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

/// A model whose `Catalog.hash` is not 32 bytes is malformed: the backend
/// refuses it with an error diagnostic and writes no file, rather than
/// padding the hash or skipping each interface.
#[test]
fn a_catalog_hash_that_is_not_32_bytes_refuses_the_model() {
    let short = refused_with(|model| model.catalog.as_mut().expect("a catalog").hash.truncate(31));
    assert_eq!(
        short,
        ["malformed codegen model: `Catalog.hash` is 31 bytes, not 32"]
    );
    let long = refused_with(|model| model.catalog.as_mut().expect("a catalog").hash.push(0));
    assert_eq!(
        long,
        ["malformed codegen model: `Catalog.hash` is 33 bytes, not 32"]
    );
}

/// A model with no `Catalog` has no hash, which is refused like a hash of
/// the wrong length.
#[test]
fn a_model_without_a_catalog_refuses_the_model() {
    let missing = refused_with(|model| model.catalog = None);
    assert_eq!(
        missing,
        ["malformed codegen model: `Catalog.hash` is 0 bytes, not 32"]
    );
}
