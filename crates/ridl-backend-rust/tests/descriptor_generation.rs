//! Descriptor-layer generation over the interaction-face fixture. Asserts that
//! `generate_face_with` emits the `Interface` and `Interaction` descriptors and the
//! interaction-kind traits over the runtime crate when `encodings` names a
//! codec, that each interface carries one size table per encoding
//! (`Sizes<FlatBuffers>`, `Sizes<Proto3>`) with every number a const
//! expression over the codec's `MAX_SIZE`, that the
//! buffer constants are sized from the right payloads, that an empty
//! `encodings` emits the domain types only, and that the pipeline `generate`
//! stays clean of the face.

use ridl_backend_rust::{WireEncoding, generate, generate_face_with, generate_with};
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
#[path = "support/rustc.rs"]
mod rustc;

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

/// The `Sizes<FlatBuffers>` implementation of `iface`, whitespace-stripped:
/// the text from its header to the next `impl`.
fn sizes_impl<'a>(dense_source: &'a str, iface: &str) -> &'a str {
    let header =
        format!("impl::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers>for{iface}{{");
    let at = dense_source
        .find(&header)
        .unwrap_or_else(|| panic!("{iface} has a Sizes<FlatBuffers> implementation"));
    let body = &dense_source[at + header.len()..];
    &body[..body.find("impl").unwrap_or(body.len())]
}

/// The expression of one `const` item of a `Sizes<FlatBuffers>`
/// implementation, whitespace-stripped: the text from `=` to the next item,
/// which `next` names.
fn sizes_item<'a>(sizes_impl: &'a str, item: &str, next: &str) -> &'a str {
    let header = format!("const{item}:");
    let at = sizes_impl
        .find(&header)
        .unwrap_or_else(|| panic!("the table carries `{item}`"));
    let body = &sizes_impl[at + header.len()..];
    let end = body
        .find(next)
        .unwrap_or_else(|| panic!("`{item}` is followed by `{next}`"));
    &body[..end]
}

/// The interface descriptor carries one size table per codec, and every
/// number in it is a const expression over the codec's own
/// `<T as Payload<FlatBuffers>>::MAX_SIZE`, never a literal the emitter would
/// have to keep equal to the codec's. `RESERVATIONS` has one row per member in
/// `MEMBERS` order — the signal, the event, the command, then the query as the
/// sum of its request and its reply. The two buffer maxima live in the
/// table only: the descriptor has no inherent alias of them. The
/// `EncodedSizes` rows the descriptor once carried are gone with
/// `PayloadInfo::max_size`.
#[test]
fn the_size_table_derives_from_max_size() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&face);

    assert!(
        !d.contains("EncodedSizes") && !d.contains("max_size:"),
        "no per-payload size row is emitted"
    );
    for iface in ["Cabin", "Horn", "Siren", "Valve", "Wiper"] {
        sizes_impl(&d, iface);
    }

    let cabin = sizes_impl(&d, "Cabin");
    let rows = sizes_item(cabin, "RESERVATIONS", "constMAX_BUFFER_SIZE");
    let expected_rows = format!(
        "&'static[::core::primitive::u64]=&[\
         {temperature}>>::MAX_SIZEas::core::primitive::u64,\
         {warning}>>::MAX_SIZEas::core::primitive::u64,\
         {level}>>::MAX_SIZEas::core::primitive::u64,\
         {window}>>::MAX_SIZEas::core::primitive::u64\
         +{average}>>::MAX_SIZEas::core::primitive::u64,\
         ];",
        temperature = max_size_path("Temperature"),
        warning = max_size_path("Warning"),
        level = max_size_path("Level"),
        window = max_size_path("Window"),
        average = max_size_path("Average"),
    );
    // prettyplease wraps a long path and then writes a trailing comma inside
    // the generic arguments and after the last row; neither comma is a token
    // of the table, so both sides are compared without them.
    let without_trailing_commas = |text: &str| text.replace(",>", ">").replace(",]", "]");
    assert_eq!(
        without_trailing_commas(rows),
        without_trailing_commas(&expected_rows),
        "Cabin's rows are the four members' sums over MAX_SIZE, in MEMBERS order"
    );

    // A fixed is never in flight: its row is `0u64`, and the signal after it
    // keeps its own row, so a table that dropped the fixed's row would shift
    // the signal's.
    let wiper = sizes_impl(&d, "Wiper");
    let rows = sizes_item(wiper, "RESERVATIONS", "constMAX_BUFFER_SIZE");
    let expected_rows = format!(
        "&'static[::core::primitive::u64]=&[0u64,{level}>>::MAX_SIZEas::core::primitive::u64,];",
        level = max_size_path("Level"),
    );
    assert_eq!(
        without_trailing_commas(rows),
        without_trailing_commas(&expected_rows),
        "Wiper's rows are the fixed's 0 and then the signal's MAX_SIZE"
    );

    let claim = sizes_item(cabin, "MAX_BUFFER_SIZE", "constEVENT_SOURCE_BUFFER_SIZE");
    let event = sizes_item(cabin, "EVENT_SOURCE_BUFFER_SIZE", "typeClaimBuffer");
    for (what, expression) in [("the claim buffer", claim), ("the event buffer", event)] {
        assert!(
            !expression.contains("usize=0") && !expression.contains("usize={0}"),
            "{what} of Cabin is sized over MAX_SIZE, not a literal: {expression}"
        );
    }
    // A size named in an array length is reached through the whole path
    // `<Self as Sizes<FlatBuffers>>::`, never through `Self::`: a program may
    // implement `Sizes` for this interface in a second encoding, and then
    // `Self::MAX_BUFFER_SIZE` names no one table (rustc E0283).
    let table = dense("<Self as ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers>>");
    let claim_max = format!("{table}::MAX_BUFFER_SIZE");
    let event_max = format!("{table}::EVENT_SOURCE_BUFFER_SIZE");
    let cabin = without_trailing_commas(cabin);
    assert!(
        cabin.contains(&format!(
            "typeClaimBuffer=[::core::primitive::u8;{claim_max}];"
        )),
        "the claim buffer type is the table's own maximum"
    );
    assert!(
        cabin.contains(&format!(
            "typeEventBuffer=[::core::primitive::u8;{event_max}];"
        )),
        "the event buffer type is the table's own maximum"
    );
    assert!(
        cabin.contains(&format!(
            "constCLAIM_BUFFER:Self::ClaimBuffer=[0u8;{claim_max}];"
        )),
        "the claim buffer is zeroed over the table's maximum"
    );
    assert!(
        cabin.contains(&format!(
            "constEVENT_BUFFER:Self::EventBuffer=[0u8;{event_max}];"
        )),
        "the event buffer is zeroed over the table's maximum"
    );

    // The table is the only home of the two maxima: the descriptor has no
    // inherent alias of them.
    for constant in ["MAX_BUFFER_SIZE", "EVENT_SOURCE_BUFFER_SIZE"] {
        let alias = format!("pubconst{constant}:");
        assert!(
            !without_trailing_commas(&d).contains(&alias),
            "the descriptor has no inherent `{constant}`"
        );
    }
}

/// The `<T as Payload<::ridl_rt::encoding::Proto3>>` marker, whitespace-stripped:
/// [`max_size_path`] in proto3.
fn proto3_max_size_path(type_name: &str) -> String {
    dense(&format!(
        "<{type_name} as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3"
    ))
}

/// The `Sizes<Proto3>` implementation of `iface`, whitespace-stripped:
/// [`sizes_impl`] in proto3.
fn proto3_sizes_impl<'a>(dense_source: &'a str, iface: &str) -> &'a str {
    let header = format!("impl::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3>for{iface}{{");
    let at = dense_source
        .find(&header)
        .unwrap_or_else(|| panic!("{iface} has a Sizes<Proto3> implementation"));
    let body = &dense_source[at + header.len()..];
    &body[..body.find("impl").unwrap_or(body.len())]
}

/// With `proto3` in the list, each interface carries a second size table,
/// `Sizes<Proto3>`, whose rows and maxima are const expressions over
/// `<T as Payload<Proto3>>::MAX_SIZE`, the proto3 codec's own bound, and
/// never over the FlatBuffers one. The FlatBuffers table beside it is the one
/// a FlatBuffers-only list emits, item for item, and the interface's doc
/// names both tables. A program compiled from the output reads the proto3
/// rows back as the sums of the proto3 bounds.
#[test]
fn the_proto3_size_table_is_built_from_proto3_bounds() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let both = generate_face_with(&package, &[WireEncoding::FlatBuffers, WireEncoding::Proto3])
        .expect("generate_face_with")
        .rust_source;
    let only = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&both);
    let without_trailing_commas = |text: &str| text.replace(",>", ">").replace(",]", "]");

    for iface in ["Cabin", "Horn", "Siren", "Valve", "Wiper"] {
        let proto3 = proto3_sizes_impl(&d, iface);
        assert!(
            !proto3.contains("encoding::FlatBuffers"),
            "{iface}'s proto3 table names no FlatBuffers item: {proto3}"
        );
        // The table ends at its last item; what follows it differs, because
        // the proto3 table comes next in one output and not in the other.
        let table = |text: &str| -> String {
            let end = text
                .find("EVENT_SOURCE_BUFFER_SIZE];}")
                .expect("the table ends with its event buffer");
            text[..end].to_string()
        };
        assert_eq!(
            table(sizes_impl(&d, iface)),
            table(sizes_impl(&dense(&only), iface)),
            "{iface}'s FlatBuffers table does not change when proto3 joins the list"
        );
    }

    let cabin = proto3_sizes_impl(&d, "Cabin");
    let rows = sizes_item(cabin, "RESERVATIONS", "constMAX_BUFFER_SIZE");
    let expected_rows = format!(
        "&'static[::core::primitive::u64]=&[\
         {temperature}>>::MAX_SIZEas::core::primitive::u64,\
         {warning}>>::MAX_SIZEas::core::primitive::u64,\
         {level}>>::MAX_SIZEas::core::primitive::u64,\
         {window}>>::MAX_SIZEas::core::primitive::u64\
         +{average}>>::MAX_SIZEas::core::primitive::u64,\
         ];",
        temperature = proto3_max_size_path("Temperature"),
        warning = proto3_max_size_path("Warning"),
        level = proto3_max_size_path("Level"),
        window = proto3_max_size_path("Window"),
        average = proto3_max_size_path("Average"),
    );
    assert_eq!(
        without_trailing_commas(rows),
        without_trailing_commas(&expected_rows),
        "Cabin's proto3 rows are the four members' sums over the proto3 MAX_SIZE"
    );
    let claim = sizes_item(cabin, "MAX_BUFFER_SIZE", "constEVENT_SOURCE_BUFFER_SIZE");
    for name in ["Level", "Window", "Average"] {
        assert!(
            claim.contains(&proto3_max_size_path(name)),
            "the proto3 claim buffer is sized over `{name}`'s proto3 bound: {claim}"
        );
    }
    let event = sizes_item(cabin, "EVENT_SOURCE_BUFFER_SIZE", "typeClaimBuffer");
    assert!(
        event.contains(&proto3_max_size_path("Warning")),
        "the proto3 event buffer is sized over `Warning`'s proto3 bound: {event}"
    );
    let table = dense("<Self as ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3>>");
    let cabin = without_trailing_commas(cabin);
    for item in [
        format!("typeClaimBuffer=[::core::primitive::u8;{table}::MAX_BUFFER_SIZE];"),
        format!("typeEventBuffer=[::core::primitive::u8;{table}::EVENT_SOURCE_BUFFER_SIZE];"),
        format!("constCLAIM_BUFFER:Self::ClaimBuffer=[0u8;{table}::MAX_BUFFER_SIZE];"),
        format!("constEVENT_BUFFER:Self::EventBuffer=[0u8;{table}::EVENT_SOURCE_BUFFER_SIZE];"),
    ] {
        assert!(
            cabin.contains(&item),
            "the proto3 buffers are sized over the proto3 table's own maxima: `{item}`"
        );
    }

    assert!(
        both.contains("Its size table in proto3 is `<Cabin as Sizes<Proto3>>`"),
        "the interface doc names the proto3 table"
    );
    assert!(
        both.contains("Its size table in FlatBuffers is `<Cabin as Sizes<FlatBuffers>>`"),
        "the interface doc still names the FlatBuffers table"
    );

    let program = format!(
        "#![allow(dead_code)]\n{both}\n\
         fn main() {{\n\
             use ::ridl_rt::contract::Sizes;\n\
             use ::ridl_rt::encoding::Proto3;\n\
             use ::ridl_rt::payload::Payload;\n\
             let rows = <Cabin as Sizes<Proto3>>::RESERVATIONS;\n\
             assert_eq!(rows.len(), 4);\n\
             assert_eq!(rows[0], <Temperature as Payload<Proto3>>::MAX_SIZE as u64);\n\
             assert_eq!(rows[1], <Warning as Payload<Proto3>>::MAX_SIZE as u64);\n\
             assert_eq!(rows[2], <Level as Payload<Proto3>>::MAX_SIZE as u64);\n\
             assert_eq!(rows[3], (<Window as Payload<Proto3>>::MAX_SIZE\n\
                 + <Average as Payload<Proto3>>::MAX_SIZE) as u64);\n\
             let claim = <Level as Payload<Proto3>>::MAX_SIZE\n\
                 .max(<Window as Payload<Proto3>>::MAX_SIZE)\n\
                 .max(<Average as Payload<Proto3>>::MAX_SIZE);\n\
             assert_eq!(<Cabin as Sizes<Proto3>>::MAX_BUFFER_SIZE, claim);\n\
             assert_eq!(<Cabin as Sizes<Proto3>>::EVENT_SOURCE_BUFFER_SIZE,\n\
                 <Warning as Payload<Proto3>>::MAX_SIZE);\n\
             assert_eq!(<Wiper as Sizes<Proto3>>::RESERVATIONS[0], 0);\n\
         }}\n"
    );
    rustc::run_program("proto3_size_table", &program);
}

/// A list that names `proto3` alone carries the descriptors, the proto3 size
/// table and the face, and nothing of FlatBuffers; the output compiles as a
/// program.
#[test]
fn a_proto3_only_package_has_a_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate_face_with(&package, &[WireEncoding::Proto3])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&source);
    for marker in [
        "impl::ridl_rt::contract::InterfaceforCabin",
        "impl::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3>forCabin",
        "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>forLevel",
        "pubmodcabin{",
        "pubmodvalve{",
    ] {
        assert!(
            d.contains(marker),
            "a proto3-only package carries `{marker}`, got:\n{source}"
        );
    }
    assert!(
        !d.contains("encoding::FlatBuffers"),
        "a proto3-only package names no FlatBuffers item"
    );
    assert!(
        source.contains("Its size table in proto3 is `<Cabin as Sizes<Proto3>>`")
            && !source.contains("Its size table in FlatBuffers"),
        "the interface doc names the one table the package carries"
    );
    rustc::run_program(
        "proto3_only_face",
        &format!("#![allow(dead_code)]\n{source}\nfn main() {{}}\n"),
    );
}

/// An interface a member of which has an induced payload with no proto3
/// bound is refused by this entry point, which refuses rather than skips:
/// the refusal names the induced message and the cause, and no descriptor
/// is emitted without its size table. The pipeline skips the interface
/// instead (`tests/induced_call_refusals.rs`).
#[test]
fn an_interface_with_an_unbounded_payload_is_refused() {
    let package = ir::compile_fixture("proto3_unbounded_call.ridl");
    let error = generate_face_with(&package, &[WireEncoding::Proto3])
        .expect_err("an interface with an unbounded payload is refused");
    assert!(
        error
            .message
            .starts_with("`pb.unbounded.call.ProbeSetParams` has no finite proto3 bound: ")
            && error.message.contains("(Overflow)"),
        "the refusal names the induced message and the cause, got: {}",
        error.message
    );
}

/// Every interaction descriptor's `ROW` indexes, in its interface's
/// `MEMBERS`, the member its `MEMBER` points at. Proven by compiling and
/// running the fixture's face, over every `impl Interaction` it emits.
#[test]
fn each_interaction_descriptor_has_its_row() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;

    const HEADER: &str = "impl ::ridl_rt::contract::Interaction for ";
    let descriptors: Vec<&str> = face
        .match_indices(HEADER)
        .map(|(at, _)| {
            let rest = &face[at + HEADER.len()..];
            rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .expect("an identifier follows the header")
        })
        .collect();
    assert_eq!(
        descriptors.len(),
        20,
        "the fixture's twenty interactions have a descriptor each: {descriptors:?}"
    );

    let checks: String = descriptors
        .iter()
        .map(|name| format!("    row_is_member::<{name}>();\n"))
        .collect();
    let program = format!(
        "#![allow(dead_code)]\n{face}\n\
         fn row_is_member<X: ::ridl_rt::contract::Interaction>() {{\n\
             let members = <X::Iface as ::ridl_rt::contract::Interface>::MEMBERS;\n\
             assert_eq!(members[X::ROW].ordinal, X::MEMBER.ordinal, \"{{}}\", X::MEMBER.name);\n\
             assert_eq!(members[X::ROW], *X::MEMBER, \"{{}}\", X::MEMBER.name);\n\
         }}\n\
         fn main() {{\n{checks}}}\n"
    );
    rustc::run_program("descriptor_rows", &program);
}

#[test]
fn the_fixture_compiles_clean() {
    let package = ir::compile_fixture("interaction_face.ridl");
    assert_eq!(package.name, "face.demo");
    assert_eq!(package.shapes().count(), 8);
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

/// `MAX_BUFFER_SIZE` of the table is the maximum over the call argument and
/// reply payloads, and over nothing else: a signal payload has a row in
/// `RESERVATIONS` but is not sized into the claim buffer.
#[test]
fn the_interface_buffer_constant_is_the_max_argument_and_reply_size() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let d = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );
    let cabin = sizes_impl(&d, "Cabin");
    let claim = sizes_item(cabin, "MAX_BUFFER_SIZE", "constEVENT_SOURCE_BUFFER_SIZE");

    // The call argument and reply types are sized in.
    assert!(
        claim.contains(&max_size_path("Level")),
        "command arg sized in"
    );
    assert!(
        claim.contains(&max_size_path("Window")),
        "query arg sized in"
    );
    assert!(
        claim.contains(&max_size_path("Average")),
        "query reply sized in"
    );
    // A signal payload is not an argument or a reply, so it is not sized in,
    // and an event payload is not either.
    assert!(
        !claim.contains(&max_size_path("Temperature")),
        "a signal payload is not in the argument/reply buffer",
    );
    assert!(
        !claim.contains(&max_size_path("Warning")),
        "an event payload is not in the argument/reply buffer",
    );
    // A signal-only interface has no call, so its claim buffer is empty.
    let horn = sizes_impl(&d, "Horn");
    assert_eq!(
        sizes_item(horn, "MAX_BUFFER_SIZE", "constEVENT_SOURCE_BUFFER_SIZE"),
        "::core::primitive::usize=0usize;",
        "Horn declares no call"
    );
}

/// `EVENT_SOURCE_BUFFER_SIZE` of the table is the maximum over the event
/// payloads, and over nothing else.
#[test]
fn the_event_source_constant_is_the_max_event_size() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let d = dense(
        &generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source,
    );
    let cabin = sizes_impl(&d, "Cabin");
    let event = sizes_item(cabin, "EVENT_SOURCE_BUFFER_SIZE", "typeClaimBuffer");

    // The event payload is sized into the event-source buffer.
    assert!(
        event.contains(&max_size_path("Warning")),
        "event payload sized in"
    );
    // A signal payload and the call payloads are not events, so they are not
    // sized in.
    for payload in ["Temperature", "Level", "Window", "Average"] {
        assert!(
            !event.contains(&max_size_path(payload)),
            "{payload} is not in the event-source buffer",
        );
    }
    // A signal-only interface has no event, so its event buffer is empty.
    let horn = sizes_impl(&d, "Horn");
    assert_eq!(
        sizes_item(horn, "EVENT_SOURCE_BUFFER_SIZE", "typeClaimBuffer"),
        "::core::primitive::usize=0usize;",
        "Horn declares no event"
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

/// `generate_with` is `generate` over the other packages of the build: the
/// domain types only, with no codec, no descriptor and no face.
#[test]
fn generate_with_emits_types_only() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate_with(&package, &[])
        .expect("generate_with")
        .rust_source;
    assert!(
        source.contains("pub struct Temperature("),
        "the domain types are emitted, got:\n{source}"
    );
    let d = dense(&source);
    for marker in CODEC_MARKERS {
        assert!(
            !d.contains(marker),
            "`generate_with` must not emit `{marker}`, got:\n{source}"
        );
    }
}

/// The name `WireEncoding::name` spells is the one `parse` reads it from.
#[test]
fn wire_encoding_name_round_trips_through_parse() {
    let encoding = WireEncoding::FlatBuffers;
    assert_eq!(encoding.name(), "flatbuffers");
    assert_eq!(WireEncoding::parse(encoding.name()), Some(encoding));
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
        message.contains("takes the options `encodings`, `wasm` and `transcode`")
            && message.contains("`codec`"),
        "got: {message}"
    );
}

fn option(key: &str, value: &str) -> v1::BackendOption {
    v1::BackendOption {
        key: key.to_string(),
        value: value.to_string(),
    }
}

/// `wasm = true` needs `flatbuffers` in `encodings`: the `wasm` module checks
/// FlatBuffers bytes, so a crate without that codec has nothing to export.
#[test]
fn wasm_without_flatbuffers_is_refused() {
    for options in [
        vec![option("wasm", "true")],
        vec![option("encodings", "proto3"), option("wasm", "true")],
    ] {
        let message = refused_options(options);
        assert!(
            message
                .contains("`[backend.rust] wasm = true` needs `encodings` to name `flatbuffers`"),
            "got: {message}"
        );
    }
    let message = refused_options(vec![
        option("encodings", "flatbuffers"),
        option("wasm", "1"),
    ]);
    assert!(message.contains("`wasm` is `1`"), "got: {message}");
}

/// `wasm = implied` is what `ridlc` sends when `codec = true` turns the module
/// on; with `flatbuffers` it is accepted like `true`.
#[test]
fn an_implied_wasm_is_accepted_with_flatbuffers() {
    backend_response(vec![
        option("encodings", "flatbuffers"),
        option("wasm", "implied"),
    ])
    .expect("a file is written");
}

/// Without `flatbuffers`, an implied `wasm` is refused with a message that
/// names `codec = true`; a written `wasm = true` keeps its own message.
#[test]
fn an_implied_wasm_without_flatbuffers_is_refused_with_the_implied_wording() {
    assert_eq!(
        refused_options(vec![
            option("encodings", "proto3"),
            option("wasm", "implied")
        ]),
        "`wasm` is implied by `codec = true` in the manifest, and `[backend.rust] encodings` \
         does not name `flatbuffers`, which every export of the `wasm` module checks"
    );
    let message = refused_options(vec![option("encodings", "proto3"), option("wasm", "true")]);
    assert!(
        message.starts_with("`[backend.rust] wasm = true` needs "),
        "got: {message}"
    );
}

/// `codec` is an option of the backend that writes the codec file, not of the
/// Rust backend.
#[test]
fn codec_on_the_rust_table_is_refused_by_the_rust_backend() {
    let message = refused_options(vec![
        option("encodings", "flatbuffers"),
        option("codec", "true"),
    ]);
    assert!(
        message.contains("`codec` is not one it knows"),
        "got: {message}"
    );
}

/// The module is written once per crate by `ridlc`, so the option leaves a
/// package's own file byte for byte as it is without the option.
#[test]
fn wasm_does_not_change_the_package_output() {
    let without = backend_response(vec![option("encodings", "flatbuffers")]).expect("a file");
    for wasm in ["true", "false"] {
        let with = backend_response(vec![
            option("encodings", "flatbuffers"),
            option("wasm", wasm),
        ])
        .expect("a file");
        assert_eq!(with, without, "wasm = {wasm}");
    }
}

/// The module lists the types that have a `Payload<FlatBuffers>`
/// implementation, sorted by qualified name, and exports the four functions
/// and no other.
#[test]
fn wasm_module_lists_the_payload_types_sorted() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let model = ridl_ir::codegen::lower(&package, &[]);
    let source = ridl_backend_rust::generate_wasm_module(&[7; 32], &[&model, &model])
        .expect("the module generates")
        .rust_source;
    let codec = backend_response(vec![option("encodings", "flatbuffers")]).expect("a file");
    let d = dense(&source);
    let head = "WASM_PAYLOAD_TYPES:&[&str]=&[";
    let start = d.find(head).expect("the list") + head.len();
    let end = start + d[start..].find(']').expect("the list ends");
    let names: Vec<&str> = d[start..end]
        .split(',')
        .map(|name| name.trim().trim_matches('"'))
        .filter(|name| !name.is_empty())
        .collect();
    assert!(!names.is_empty(), "got:\n{source}");
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(names, sorted, "sorted and each once");
    let impls = dense(&codec)
        .matches("::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>for")
        .count();
    assert_eq!(
        names.len(),
        impls,
        "one entry per Payload<FlatBuffers> impl"
    );
    for name in &names {
        let short = name.rsplit('.').next().expect("a name");
        assert!(
            dense(&codec).contains(&format!("FlatBuffers>for{short}{{")),
            "`{short}` has no Payload<FlatBuffers> impl"
        );
    }
    assert!(
        d.contains(
            "WASM_RULES:&[&str]=&[\"length\",\"pattern\",\"range\",\"step\",\"unique\",\"variant\",]"
        ),
        "got:\n{source}"
    );
    let exports: Vec<&str> = d
        .match_indices("pubunsafeextern\"C\"fn")
        .chain(d.match_indices("pubextern\"C\"fn"))
        .map(|(at, matched)| {
            let rest = &d[at + matched.len()..];
            &rest[..rest.find('(').expect("a parameter list")]
        })
        .collect();
    let mut exports = exports;
    exports.sort_unstable();
    assert_eq!(
        exports,
        [
            "ridl_alloc",
            "ridl_catalog_hash",
            "ridl_free",
            "ridl_type_id",
            "ridl_verify"
        ]
    );
    assert_eq!(source.matches("no_mangle").count(), 5);
    assert!(
        d.contains("pubfntype_id_of(name:&str)->::core::primitive::i32"),
        "got:\n{source}"
    );
    assert!(d.contains("#[cfg(all(target_arch=\"wasm32\",feature=\"wasm-exports\"))]modexports{"));
    assert!(ridl_backend_rust::generate_wasm_module(&[0; 31], &[&model]).is_err());
}

/// `wasm = false` is accepted with any `encodings`, or none, because it asks
/// for nothing; a value that is not `true` or `false` is refused.
#[test]
fn wasm_false_is_accepted_without_flatbuffers() {
    for options in [
        vec![option("wasm", "false")],
        vec![option("encodings", "proto3"), option("wasm", "false")],
    ] {
        backend_response(options.clone()).unwrap_or_else(|err| panic!("{options:?}: {err:?}"));
    }
    let message = refused_options(vec![option("encodings", "flatbuffers"), option("wasm", "")]);
    assert!(message.contains("`wasm` is ``"), "got: {message}");
}

/// The module lists a type only when its FlatBuffers root has a finite bound:
/// a root made unbounded drops out of the list, and the rest stay.
#[test]
fn wasm_module_leaves_out_an_unbounded_root() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let model = ridl_ir::codegen::lower(&package, &[]);
    let list = |model: &v1::Model| -> String {
        let source = ridl_backend_rust::generate_wasm_module(&[0; 32], &[model])
            .expect("the module generates")
            .rust_source;
        let d = dense(&source);
        let head = "WASM_PAYLOAD_TYPES:&[&str]=&[";
        let start = d.find(head).expect("the list") + head.len();
        d[start..start + d[start..].find(']').expect("the end")].to_string()
    };
    let before = list(&model);
    assert!(before.contains("\"face.demo.Temperature\""), "{before}");
    let mut unbounded = model.clone();
    let projection = unbounded.flatbuffers.as_mut().expect("a projection");
    let root = projection
        .roots
        .iter_mut()
        .find(|root| {
            root.tuple.is_none()
                && model.declarations[root.declaration as usize]
                    .name
                    .as_ref()
                    .is_some_and(|name| name.declared == "Temperature")
        })
        .expect("Temperature has a root");
    root.bound = Some(v1::fb_root::Bound::Unbounded(Default::default()));
    let after = list(&unbounded);
    assert_eq!(
        after,
        before.replace("\"face.demo.Temperature\",", ""),
        "only Temperature drops out"
    );
}

/// The fallback arms of the generated code, which no rule or error of the
/// current `ridl-rt` reaches: a rule the list does not name is
/// `2 + WASM_RULES.len()`, and a `VerifyError` added later is 1.
#[test]
fn wasm_module_keeps_its_fallback_codes() {
    let source = ridl_backend_rust::generate_wasm_module(&[0; 32], &[])
        .expect("the module generates")
        .rust_source;
    let d = dense(&source);
    assert!(
        d.contains("_=>2+WASM_RULES.len()as::core::primitive::i32,"),
        "{source}"
    );
    assert!(d.contains("::core::result::Result::Err(_)=>1,"), "{source}");
}

/// `proto3` is accepted through the contract. Alone, it emits the domain
/// types, the proto3 codec, the proto3 size table and the face, and nothing
/// of FlatBuffers; with `flatbuffers`, both codecs and both size tables are
/// emitted side by side under one face.
#[test]
fn proto3_is_accepted_through_the_contract() {
    let alone = backend_response(vec![encodings_option("proto3")]).expect("a file is written");
    let d = dense(&alone);
    assert!(
        d.contains("Payload<::ridl_rt::encoding::Proto3>for"),
        "the proto3 codec is emitted, got:\n{alone}"
    );
    assert!(
        d.contains("Sizes<::ridl_rt::encoding::Proto3>for"),
        "the proto3 size table is emitted, got:\n{alone}"
    );
    assert!(
        alone.contains("pub mod valve"),
        "the face is emitted, got:\n{alone}"
    );
    assert!(
        !d.contains("encoding::FlatBuffers"),
        "nothing of FlatBuffers without `flatbuffers`, got:\n{alone}"
    );

    let both =
        backend_response(vec![encodings_option("flatbuffers,proto3")]).expect("a file is written");
    let d = dense(&both);
    assert!(d.contains("Payload<::ridl_rt::encoding::Proto3>for"));
    assert!(d.contains("Payload<::ridl_rt::encoding::FlatBuffers>for"));
    assert!(d.contains("Sizes<::ridl_rt::encoding::FlatBuffers>for"));
    assert!(d.contains("Sizes<::ridl_rt::encoding::Proto3>for"));
    assert!(both.contains("pub mod valve"));
}

/// `podbuf` is accepted: the package carries the podbuf codec, the
/// `Payload<Podbuf>` implementations and the `Sizes<Podbuf>` table of its
/// interface, whose claim and event buffers are 8-aligned.
#[test]
fn podbuf_is_accepted() {
    let source = backend_response(vec![encodings_option("podbuf")]).expect("a file is written");
    let d = dense(&source);
    assert!(source.contains("PodbufHead"), "got: {source}");
    assert!(
        d.contains("Payload<::ridl_rt::encoding::Podbuf>for"),
        "got: {source}"
    );
    assert!(
        d.contains("Sizes<::ridl_rt::encoding::Podbuf>for"),
        "got: {source}"
    );
    assert!(
        d.contains("typeClaimBuffer=::ridl_rt::podbuf::Aligned<"),
        "got: {source}"
    );
    assert!(
        !d.contains("encoding::FlatBuffers") && !d.contains("encoding::Proto3"),
        "nothing of the other encodings without them, got: {source}"
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
    let response = response_over(edit, vec![encodings_option("flatbuffers")]);
    assert!(response.files.is_empty(), "no file for a malformed model");
    response
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

/// `Backend::generate` over the fixture's model after `edit`, with `options`.
fn response_over(
    edit: impl FnOnce(&mut ridl_ir::codegen::v1::Model),
    options: Vec<v1::BackendOption>,
) -> v1::CodegenResponse {
    use ridl_ir::codegen::Backend as _;

    let package = ir::compile_fixture("interaction_face.ridl");
    let mut model = ridl_ir::codegen::lower(&package, &[]);
    edit(&mut model);
    let request = v1::CodegenRequest {
        model: Some(model),
        options,
        artifact_base: "face_demo".to_string(),
        ..Default::default()
    };
    ridl_backend_rust::Backend.generate(&request)
}

/// A types-only build reads no catalog hash, so a model whose `Catalog.hash`
/// is malformed is still emitted when the request names no encoding: the
/// hash is checked only on the path that emits a descriptor.
#[test]
fn a_types_only_build_does_not_read_the_catalog_hash() {
    let response = response_over(
        |model| model.catalog.as_mut().expect("a catalog").hash.truncate(31),
        Vec::new(),
    );
    assert!(
        response.diagnostics.is_empty(),
        "no refusal for a types-only build, got: {:?}",
        response.diagnostics
    );
    let text = response
        .files
        .iter()
        .find_map(|file| match &file.content {
            Some(v1::generated_file::Content::Text(text)) => Some(text.clone()),
            _ => None,
        })
        .expect("a file is written");
    assert!(text.contains("pub struct Temperature("), "got:\n{text}");
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

/// A tuple root gets a `Payload` implementation under the induced struct's own
/// name, so a descriptor's reservation row, which names
/// `<T as Payload<FlatBuffers>>::MAX_SIZE`, resolves (driftsys/ridl#713).
#[test]
fn an_induced_argument_struct_has_a_payload_impl() {
    let package = ir::compile_fixture("induced_params.ridl");
    let source =
        ridl_backend_rust::generate_pipeline(&package, &[WireEncoding::FlatBuffers], false, &[])
            .expect("generate_pipeline")
            .rust_source;
    let d = dense(&source);
    for induced in [
        "PumpBothParams",
        "PumpStatusParams",
        "PumpDrainParams",
        "PumpDrainReply",
    ] {
        assert!(
            d.contains(&dense(&format!(
                "impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for {induced}"
            ))),
            "`{induced}` has a Payload implementation:\n{source}"
        );
    }
    assert!(
        d.contains("pubstructPumpStatusParams{}"),
        "a call with no parameter induces a struct with no field:\n{source}"
    );
    // The one-parameter call keeps its declared named type, so it induces
    // nothing to implement `Payload` for.
    assert!(
        !d.contains("PumpOneParams"),
        "a single named parameter induces no struct:\n{source}"
    );
}

/// A two-parameter call carries a face: its descriptor's `Args` is the induced
/// argument struct, and the interface is not skipped (driftsys/ridl#713).
#[test]
fn a_two_parameter_call_carries_a_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&face);
    assert!(
        d.contains("typeArgs=ValveSetBothParams;"),
        "the call's Args is the induced argument struct:\n{face}"
    );
    assert!(
        d.contains("typeArgs=ValveStatusParams;"),
        "a zero-parameter call's Args is the induced empty struct:\n{face}"
    );
    assert!(
        !d.contains("__RIDL_NO_FACE_Valve"),
        "and the interface is not skipped:\n{face}"
    );
}
