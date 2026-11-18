//! The proto3 layout carried by the lowered codegen model.

use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;
use ridl_ir::codegen::{self, v1};

fn projection(source: &str) -> (v1::Model, v1::Proto3Projection) {
    let path = if source.contains("\ninterface ") {
        "projection.ridl"
    } else {
        "projection.typl"
    };
    let compiled = ridlc::compile(path, source);
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let model = codegen::lower(&compiled.package, &[]);
    let projected = model.proto3.clone().expect("a proto3 projection");
    assert!(projected.refused.is_empty());
    (model, projected)
}

fn message<'a>(p: &'a v1::Proto3Projection, name: &str) -> &'a v1::Proto3Message {
    p.messages.iter().find(|m| m.name == name).expect(name)
}

fn kind(field: &v1::Proto3Field) -> &v1::proto3_wire::Kind {
    field.wire.as_ref().unwrap().kind.as_ref().unwrap()
}

#[test]
fn a_struct_field_takes_its_ordinal_and_its_scalar() {
    let (_, p) = projection(
        "package p.scalar\n\nstruct Scalars {\n\
         u8 : integer [0..255]\n  u16 : integer [0..65535]\n\
         u32 : integer [0..4294967295]\n  u64 : integer [0..9223372036854775807]\n\
         i8 : integer [-128..127]\n  i16 : integer [-32768..32767]\n\
         i32 : integer [-2147483648..2147483647]\n  i64 : integer [-4294967296..4294967296]\n\
         f32 : float [0.0..250.0 step 0.5]\n  f64 : float\n\
         flag : boolean\n  plain : integer\n  text : string [0..16]\n  data : bytes [0..8]\n}\n",
    );
    let m = message(&p, "Scalars");
    assert_eq!(
        m.source,
        Some(v1::proto3_message::Source::StructDeclaration(0))
    );
    use v1::Proto3ScalarType::*;
    use v1::Proto3WireType::*;
    let expected = [
        (Uint32, Varint),
        (Uint32, Varint),
        (Uint32, Varint),
        (Uint64, Varint),
        (Sint32, Varint),
        (Sint32, Varint),
        (Sint32, Varint),
        (Sint64, Varint),
        (Float, I32),
        (Double, I64),
        (Bool, Varint),
        (Sint64, Varint),
    ];
    assert_eq!(m.fields.len(), 14);
    for (index, (ty, wire)) in expected.into_iter().enumerate() {
        let field = &m.fields[index];
        assert_eq!(field.number, index as u32 + 1);
        assert_eq!(
            kind(field),
            &v1::proto3_wire::Kind::Scalar(v1::Proto3Scalar {
                r#type: ty as i32,
                wire_type: wire as i32,
            })
        );
    }
    for (index, name, wire) in [
        (12, "text", v1::proto3_wire::Kind::Text(true)),
        (13, "data", v1::proto3_wire::Kind::Bytes(true)),
    ] {
        assert_eq!(m.fields[index].number, index as u32 + 1);
        assert_eq!(
            m.fields[index].source,
            Some(v1::proto3_field::Source::Field(name.into()))
        );
        assert_eq!(kind(&m.fields[index]), &wire);
    }
}

#[test]
fn fields_are_in_number_order_and_a_retired_slot_has_none() {
    let source = "package p.order\n\nstruct Record {\n  first : integer\n  reserved old\n  finalValue : boolean\n}\n";
    let compiled = ridlc::compile("order.typl", source);
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    let mut package = compiled.package;
    let Some(ridl_ir::v2::decl::Kind::StructDef(def)) = package.decls[0].kind.as_mut() else {
        panic!("a struct");
    };
    // Source ordinals are contiguous; reorder the IR to test number sorting.
    def.members.reverse();
    let model = codegen::lower(&package, &[]);
    let p = model.proto3.unwrap();
    let m = message(&p, "Record");
    assert_eq!(
        m.fields.iter().map(|f| f.number).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        m.fields[0].source,
        Some(v1::proto3_field::Source::Field("first".into()))
    );
    assert_eq!(
        m.fields[1].source,
        Some(v1::proto3_field::Source::Field("finalValue".into()))
    );
}

#[test]
fn a_union_lists_its_arms_by_ordinal() {
    let (_, p) = projection(
        "package p.choices\n\ntype Count : integer [0..255]\n\ntype Label : string [0..8]\n\nunion Pick {\n  count : Count\n  reserved old\n  label : Label\n}\n",
    );
    let m = message(&p, "Pick");
    assert_eq!(
        m.source,
        Some(v1::proto3_message::Source::UnionDeclaration(2))
    );
    assert_eq!(
        m.fields
            .iter()
            .map(|f| (f.number, f.source.clone()))
            .collect::<Vec<_>>(),
        [
            (1, Some(v1::proto3_field::Source::Arm("count".into()))),
            (3, Some(v1::proto3_field::Source::Arm("label".into()))),
        ]
    );
}

#[test]
fn a_named_scalar_an_enum_and_an_enum_set_get_a_box() {
    let (_, p) = projection(
        "package p.boxes\n\ntype Count : integer [0..255]\n\nenum Mode {\n  OFF = 0\n  ON = 1\n}\n\nenumset Flags {\n  FIRST = 0\n  LAST = 63\n}\n\nconst LIMIT : integer = 3\n",
    );
    assert_eq!(
        p.messages
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["CountBox", "ModeBox", "FlagsBox"]
    );
    for (index, m) in p.messages.iter().enumerate() {
        assert_eq!(
            m.source,
            Some(v1::proto3_message::Source::RootBox(index as u32))
        );
        assert_eq!(m.fields.len(), 1);
        assert_eq!(m.fields[0].number, 1);
        assert_eq!(
            m.fields[0].source,
            Some(v1::proto3_field::Source::BoxValue(true))
        );
    }
    assert!(
        matches!(kind(&p.messages[1].fields[0]), v1::proto3_wire::Kind::Enumeration(e) if e.r#type.as_ref().unwrap().index == 1)
    );
    assert!(
        matches!(kind(&p.messages[2].fields[0]), v1::proto3_wire::Kind::Scalar(s) if s.r#type == v1::Proto3ScalarType::Uint64 as i32)
    );
}

#[test]
fn a_tuple_numbers_its_positions_from_one() {
    let (model, p) = projection(
        "package p.tuple\n\nstruct Holder {\n  pair : (left: integer [0..255], right: boolean)\n}\n",
    );
    assert_eq!(p.messages.len(), 2);
    let m = message(&p, &model.tuples[0].name.as_ref().unwrap().wire);
    assert_eq!(m.source, Some(v1::proto3_message::Source::Tuple(0)));
    assert_eq!(
        m.fields
            .iter()
            .map(|f| (f.number, f.source.clone()))
            .collect::<Vec<_>>(),
        [
            (1, Some(v1::proto3_field::Source::TuplePosition(0))),
            (2, Some(v1::proto3_field::Source::TuplePosition(1))),
        ]
    );
    assert!(
        matches!(kind(&message(&p, "Holder").fields[0]), v1::proto3_wire::Kind::Message(r) if r.target == Some(v1::proto3_message_ref::Target::Message(1)))
    );
}

#[test]
fn an_array_of_scalars_or_enums_is_packed_and_any_other_is_not() {
    let (_, p) = projection(
        "package p.array\n\nenum Mode {\n  OFF = 0\n}\n\nenumset Flags {\n  ONE = 0\n}\n\nstruct Item {\n  x : boolean\n}\n\nstruct Arrays {\n  numbers : [integer [0..4294967295]; 0..3]\n  modes : [Mode; 0..3]\n  flags : [Flags; 0..3]\n  labels : [string [0..8]; 0..3]\n  blobs : [bytes [0..8]; 0..3]\n  items : [Item; 0..3]\n}\n",
    );
    let m = message(&p, "Arrays");
    assert_eq!(m.fields.len(), 6);
    for (index, field) in m.fields.iter().enumerate() {
        let v1::proto3_wire::Kind::Array(array) = kind(field) else {
            panic!("an array")
        };
        assert_eq!(array.packed, index < 3);
        assert!(matches!(
            (
                index,
                array.element.as_ref().unwrap().kind.as_ref().unwrap()
            ),
            (0 | 2, v1::proto3_wire::Kind::Scalar(_))
                | (1, v1::proto3_wire::Kind::Enumeration(_))
                | (3, v1::proto3_wire::Kind::Text(true))
                | (4, v1::proto3_wire::Kind::Bytes(true))
                | (5, v1::proto3_wire::Kind::Message(_))
        ));
    }
}

#[test]
fn a_map_carries_its_key_and_value_wires() {
    let (_, p) = projection(
        "package p.map\n\nstruct Item {\n  x : boolean\n}\n\nstruct Maps {\n  byText : [string [0..8] : integer [0..4294967295]; 0..3]\n  byNumber : [integer [0..4294967295] : Item; 0..3]\n}\n",
    );
    let m = message(&p, "Maps");
    assert_eq!(m.fields.len(), 2);
    let maps: Vec<_> = m
        .fields
        .iter()
        .map(|f| match kind(f) {
            v1::proto3_wire::Kind::Map(map) => map,
            other => panic!("a map, got {other:?}"),
        })
        .collect();
    assert_eq!(
        maps[0].key.as_ref().unwrap().kind,
        Some(v1::proto3_wire::Kind::Text(true))
    );
    let scalar = v1::proto3_wire::Kind::Scalar(v1::Proto3Scalar {
        r#type: v1::Proto3ScalarType::Uint32 as i32,
        wire_type: v1::Proto3WireType::Varint as i32,
    });
    assert_eq!(maps[0].value.as_ref().unwrap().kind.as_ref(), Some(&scalar));
    assert_eq!(maps[1].key.as_ref().unwrap().kind.as_ref(), Some(&scalar));
    assert!(
        matches!(maps[1].value.as_ref().unwrap().kind.as_ref(), Some(v1::proto3_wire::Kind::Message(r)) if r.target == Some(v1::proto3_message_ref::Target::Message(0)))
    );
}

#[test]
fn an_enum_map_key_in_ir_keeps_the_codecs_integer_key_rule() {
    let compiled = ridlc::compile(
        "enum_key.typl",
        "package p.keys\n\nenum Mode {\n  OFF = 0\n}\n\nstruct Table {\n  entries : [integer : boolean; 0..2]\n}\n",
    );
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    let mut package = compiled.package;
    // The source checker restricts map keys; the codec also accepts this IR.
    let Some(ridl_ir::v2::decl::Kind::StructDef(def)) = package.decls[1].kind.as_mut() else {
        panic!("a struct")
    };
    let Some(ridl_ir::v2::struct_member::Member::Field(field)) = def.members[0].member.as_mut()
    else {
        panic!("a field")
    };
    let Some(ridl_ir::v2::field_type::Kind::Map(map)) =
        field.r#type.as_mut().unwrap().kind.as_mut()
    else {
        panic!("a map")
    };
    map.key.as_mut().unwrap().kind = Some(ridl_ir::v2::field_type::Kind::Named("Mode".into()));
    let model = codegen::lower(&package, &[]);
    let p = model.proto3.unwrap();
    let v1::proto3_wire::Kind::Map(map) = kind(&message(&p, "Table").fields[0]) else {
        panic!("a map")
    };
    assert!(matches!(
        map.key.as_ref().unwrap().kind,
        Some(v1::proto3_wire::Kind::Enumeration(_))
    ));
}

#[test]
fn a_struct_that_holds_itself_refers_to_its_own_index() {
    let compiled = ridlc::compile(
        "recursive.typl",
        "package p.recursive\n\nstruct Node {\n  children : [integer; 0..2]\n}\n",
    );
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    let mut package = compiled.package;
    // Recursive source types draw TYPL-206; construct the reference in IR.
    let Some(ridl_ir::v2::decl::Kind::StructDef(def)) = package.decls[0].kind.as_mut() else {
        panic!("a struct")
    };
    let Some(ridl_ir::v2::struct_member::Member::Field(field)) = def.members[0].member.as_mut()
    else {
        panic!("a field")
    };
    let Some(ridl_ir::v2::field_type::Kind::Array(array)) =
        field.r#type.as_mut().unwrap().kind.as_mut()
    else {
        panic!("an array")
    };
    array.element.as_mut().unwrap().kind =
        Some(ridl_ir::v2::field_type::Kind::Named("Node".into()));
    let model = codegen::lower(&package, &[]);
    let p = model.proto3.unwrap();
    let m = message(&p, "Node");
    let v1::proto3_wire::Kind::Array(array) = kind(&m.fields[0]) else {
        panic!("an array")
    };
    assert!(
        matches!(array.element.as_ref().unwrap().kind.as_ref(), Some(v1::proto3_wire::Kind::Message(r)) if r.target == Some(v1::proto3_message_ref::Target::Message(0)))
    );
}

#[test]
fn a_member_typed_in_another_package_has_the_local_wire() {
    let root = tempfile::tempdir().unwrap();
    let write = |path: &str, text: &str| {
        let path = root.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
    for name in ["a", "b"] {
        write(
            &format!("{name}/ridl.toml"),
            &format!("[package]\nname = \"p.{name}\"\nversion = \"1.0.0\"\n"),
        );
    }
    write(
        "a/a.typl",
        "package p.a\n\nstruct Remote {\n  x : boolean\n}\n\nenum Mode {\n  OFF = 0\n}\n\ntype Count : integer [0..255]\n",
    );
    write(
        "b/b.typl",
        "package p.b\n\nimport p.a.Remote\nimport p.a.Mode\nimport p.a.Count\n\nstruct Local {\n  remote : Remote\n  mode : Mode\n  count : Count\n}\n",
    );
    let mut db = RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, root.path()).unwrap();
    assert!(
        !output
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    let packages: Vec<_> = output
        .checked
        .iter()
        .map(|c| &c.ir)
        .chain(std::iter::once(&output.std_ir))
        .collect();
    let local = packages.iter().find(|p| p.name == "p.b").unwrap();
    let model = codegen::lower(local, &packages);
    let p = model.proto3.as_ref().unwrap();
    let m = message(p, "Local");
    let v1::proto3_wire::Kind::Message(r) = kind(&m.fields[0]) else {
        panic!("a message")
    };
    let Some(v1::proto3_message_ref::Target::Foreign(at)) = r.target else {
        panic!("foreign")
    };
    assert_eq!(model.foreign[at as usize].package, "p.a");
    assert_eq!(
        model.foreign[at as usize]
            .declaration
            .as_ref()
            .unwrap()
            .name
            .as_ref()
            .unwrap()
            .declared,
        "Remote"
    );
    let v1::proto3_wire::Kind::Enumeration(e) = kind(&m.fields[1]) else {
        panic!("an enum")
    };
    assert_eq!(e.r#type.as_ref().unwrap().package, "p.a");
    assert!(e.r#type.as_ref().unwrap().foreign);
    assert!(
        matches!(kind(&m.fields[2]), v1::proto3_wire::Kind::Scalar(s) if s.r#type == v1::Proto3ScalarType::Uint32 as i32)
    );
}

#[test]
fn an_empty_package_has_an_empty_projection() {
    let (model, p) = projection("package p.empty\n");
    assert_eq!(p, v1::Proto3Projection::default());
    assert_eq!(model.proto3, Some(v1::Proto3Projection::default()));
}

#[test]
fn an_unsupported_message_and_its_users_are_omitted() {
    let (_, p) = projection(
        "package p.omitted\n\nstruct Bad {\n  values : [[integer; 0..2]; 0..2]\n}\n\nstruct User {\n  bad : Bad\n}\n\nstruct Holder {\n  pair : (bad: User, good: boolean)\n}\n\nstruct Good {\n  value : boolean\n}\n\nstruct Last {\n  good : Good\n}\n",
    );
    assert!(p.refused.is_empty());
    assert_eq!(
        p.messages
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["Good", "Last"]
    );
    assert!(
        matches!(kind(&message(&p, "Last").fields[0]), v1::proto3_wire::Kind::Message(r) if r.target == Some(v1::proto3_message_ref::Target::Message(0)))
    );
}
