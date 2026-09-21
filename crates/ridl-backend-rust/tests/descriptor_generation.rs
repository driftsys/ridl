//! Descriptor-layer generation over the interaction-face fixture (Lane M stage
//! M3, Task 1). Asserts that `generate_face` emits the `Interface` and
//! `Interaction` descriptors and the interaction-kind traits over the runtime
//! crate, that the buffer constants are sized from the right payloads, and
//! that the pipeline `generate` stays clean of the face.

use ridl_backend_rust::{generate, generate_face};

#[path = "support/ir.rs"]
mod ir;

/// Whitespace-stripped copy, so an assertion is robust to how prettyplease
/// spaces a token stream.
fn dense(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The `<T as Payload<ReprC>>` marker a buffer constant sizes from,
/// whitespace-stripped. `Payload<ReprC>` appears only in the generated buffer
/// constants, so its presence for a type name says that type is sized into a
/// buffer. The `MAX_SIZE` suffix is left off because prettyplease may wrap the
/// path and insert a trailing comma inside the generic arguments.
fn max_size_path(type_name: &str) -> String {
    dense(&format!(
        "<{type_name} as ::ridl_rt::payload::Payload<::ridl_rt::encoding::ReprC"
    ))
}

#[test]
fn the_fixture_compiles_clean() {
    let package = ir::compile_fixture("interaction_face.ridl");
    assert_eq!(package.name, "face.demo");
    assert_eq!(package.shapes().count(), 2);
}

#[test]
fn generate_face_emits_the_interface_and_interaction_descriptors() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face(&package).expect("generate_face").rust_source;
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
    assert!(
        d.contains("::ridl_rt::contract::CatalogHash([0u8;32])"),
        "zero catalog hash placeholder",
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
        d.contains("constPROVISIONAL:bool=true"),
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

    // Every payload's encoded sizes are all absent (the M3 placeholder).
    assert!(
        d.contains("::ridl_rt::contract::EncodedSizes{proto3:None,flatbuffers:None,repr_c:None"),
        "encoded sizes are all None",
    );

    // Ordinals and names are carried on the member rows.
    assert!(
        d.contains("ordinal:::ridl_rt::contract::Ordinal(3)"),
        "setLevel ordinal"
    );
    assert!(d.contains("name:\"setLevel\""), "member source name");
}

#[test]
fn generate_face_emits_the_contract_clause_bodies() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let d = dense(&generate_face(&package).expect("generate_face").rust_source);

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
    let d = dense(&generate_face(&package).expect("generate_face").rust_source);

    assert!(
        d.contains("constMAX_BUFFER_SIZE:usize"),
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
    let d = dense(&generate_face(&package).expect("generate_face").rust_source);

    assert!(
        d.contains("constEVENT_SOURCE_BUFFER_SIZE:usize"),
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
/// It used to name no runtime path at all outside a named scalar's
/// constructor. Stage K5 changed that deliberately: design note D-1, as
/// amended, puts the `Payload<FlatBuffers>` implementations in `generate`'s
/// own output, because a consumer of a generated package needs the codec
/// whether or not it ever dispatches. So the assertion is no longer "no
/// runtime path"; it is that every runtime path belongs to one of the two
/// things this entry point emits — the constructors and the codec — and that
/// none belongs to the face.
#[test]
fn the_pipeline_generate_stays_clean_of_the_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let plain = generate(&package).expect("generate").rust_source;
    let face = generate_face(&package).expect("generate_face").rust_source;

    assert_ne!(plain, face, "the two entry points must differ");
    // The modules the pipeline entry point may name: `payload` for a named
    // scalar's `Violation` and `Rule` and for the codec's `Payload` trait,
    // `flatbuffers` for the codec's shared reading and writing helpers, and
    // `encoding` for the `FlatBuffers` marker the implementation is written
    // over. `contract` and `port` are the face's, and neither may appear.
    let permitted = [
        "ridl_rt::payload::",
        "ridl_rt::flatbuffers::",
        "ridl_rt::encoding::FlatBuffers",
    ];
    let dense_plain = dense(&plain);
    for (index, _) in dense_plain.match_indices("ridl_rt") {
        let rest = &dense_plain[index..];
        let permitted = permitted
            .iter()
            .any(|path| dense_plain[..index].ends_with("::") && rest.starts_with(path));
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

/// The codec reaches the pipeline entry point, and only it: the face fixture
/// is regenerated by stage K7, so `generate_face` still carries the
/// hand-written `Payload<ReprC>` implementations rather than the generated
/// ones, and nothing in this stage changes that file.
#[test]
fn the_pipeline_generate_carries_the_flatbuffers_codec() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let plain = dense(&generate(&package).expect("generate").rust_source);
    let face = dense(&generate_face(&package).expect("generate_face").rust_source);

    let codec = "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>forWarning";
    assert!(
        plain.contains(codec),
        "the pipeline generate emits a `Payload<FlatBuffers>` per root table",
    );
    assert!(
        !face.contains(codec),
        "the companion entry point does not, until stage K7 moves the face onto `Wire`",
    );
}
