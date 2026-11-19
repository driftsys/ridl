//! The proto3 layout carried by the lowered codegen model.

use ridl_backend_rust::WireEncoding;
use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;
use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;

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
    (model, projected)
}

fn message<'a>(p: &'a v1::Proto3Projection, name: &str) -> &'a v1::Proto3Message {
    p.messages.iter().find(|m| m.name == name).expect(name)
}

fn kind(field: &v1::Proto3Field) -> &v1::proto3_wire::Kind {
    field.wire.as_ref().unwrap().kind.as_ref().unwrap()
}

fn default_rule(p: &v1::Proto3Projection, owner: &str, position: usize) -> bool {
    message(p, owner).fields[position]
        .wire
        .as_ref()
        .expect("a wire")
        .default_is_legal
}

#[test]
fn a_ranged_scalar_default_is_legal_only_when_the_range_holds_zero() {
    for (ty, expected) in [
        ("integer [0..10]", true),
        ("integer [1..10]", false),
        ("float [-1.5..1.5 step 1.0]", false),
        ("float [-1.0..1.0 step 0.5]", true),
        ("integer", true),
        ("float", true),
        ("boolean", true),
    ] {
        let (_, p) = projection(&format!(
            "package p.defaults\ntype Number : {ty}\nstruct Values {{\n inline : {ty}\n named : Number\n}}\n"
        ));
        assert_eq!(default_rule(&p, "NumberBox", 0), expected, "{ty} box");
        assert_eq!(default_rule(&p, "Values", 0), expected, "{ty} inline");
        assert_eq!(default_rule(&p, "Values", 1), expected, "{ty} named");
    }
}

#[test]
fn a_text_or_bytes_default_is_legal_only_without_a_length_floor() {
    for ty in ["string", "bytes"] {
        for (constraint, expected) in [("[0..8]", true), ("[1..8]", false), ("", true)] {
            let inline = if constraint.is_empty() {
                "Data".to_string()
            } else {
                format!("{ty} {constraint}")
            };
            let (_, p) = projection(&format!(
                "package p.defaults\ntype Data : {ty} {constraint}\nstruct Values {{\n inline : {inline}\n named : Data\n}}\n"
            ));
            assert_eq!(
                default_rule(&p, "DataBox", 0),
                expected,
                "{ty} box {constraint}"
            );
            assert_eq!(
                default_rule(&p, "Values", 0),
                expected,
                "{ty} inline {constraint}"
            );
            assert_eq!(
                default_rule(&p, "Values", 1),
                expected,
                "{ty} named {constraint}"
            );
        }
    }
}

#[test]
fn an_enum_default_is_legal_with_a_zero_member_or_when_open() {
    for (openness, value, expected) in [
        ("", 0, true),
        ("", 1, false),
        ("open ", 0, true),
        ("open ", 1, true),
    ] {
        let (_, p) = projection(&format!(
            "package p.defaults\n{openness}enum Mode {{\n VALUE = {value}\n}}\nstruct Values {{\n mode : Mode\n}}\n"
        ));
        assert_eq!(
            default_rule(&p, "ModeBox", 0),
            expected,
            "{openness}{value} box"
        );
        assert_eq!(
            default_rule(&p, "Values", 0),
            expected,
            "{openness}{value} field"
        );
    }
}

#[test]
fn an_enum_set_and_a_message_default_are_always_legal() {
    let (model, p) = projection(
        "package p.defaults\n\
         enumset Flags {\n BIT = 1\n}\n\
         type Positive : integer [1..10]\n\
         struct Item {\n required : Positive\n}\n\
         union Choice {\n item : Item\n number : Positive\n}\n\
         struct Values {\n flags : Flags\n item : Item\n choice : Choice\n pair : (required: Positive, flag: boolean)\n}\n",
    );
    assert!(default_rule(&p, "FlagsBox", 0));
    for position in 0..4 {
        assert!(default_rule(&p, "Values", position));
    }
    assert!(default_rule(&p, "Choice", 0));
    assert!(!default_rule(&p, "Choice", 1));
    let tuple = &model.tuples[0].name.as_ref().unwrap().wire;
    assert!(!default_rule(&p, tuple, 0));
    assert!(default_rule(&p, tuple, 1));
    assert!(!default_rule(&p, "Item", 0));
}

#[test]
fn a_collection_default_is_legal_only_without_a_count_floor() {
    for (min, expected) in [(0, true), (1, false)] {
        let (_, p) = projection(&format!(
            "package p.defaults\nstruct Values {{\n numbers : [integer [1..10]; {min}..3]\n entries : [integer [0..10] : boolean; {min}..3]\n}}\n"
        ));
        assert_eq!(default_rule(&p, "Values", 0), expected, "array {min}");
        assert_eq!(default_rule(&p, "Values", 1), expected, "map {min}");
        let v1::proto3_wire::Kind::Array(array) = kind(&message(&p, "Values").fields[0]) else {
            panic!("an array")
        };
        assert!(!array.element.as_ref().unwrap().default_is_legal);
    }
    let (_, p) =
        projection("package p.defaults\nstruct Values {\n numbers : [integer [0..10]; 1..3]\n}\n");
    let v1::proto3_wire::Kind::Array(array) = kind(&message(&p, "Values").fields[0]) else {
        panic!("an array")
    };
    assert!(!default_rule(&p, "Values", 0));
    assert!(array.element.as_ref().unwrap().default_is_legal);
}

#[test]
fn a_map_key_and_value_carry_their_own_default_rule() {
    let (_, p) = projection(
        "package p.defaults\n\
         type Key : string [1..8]\n\
         struct Maps {\n strictKey : [Key : integer [0..10]; 0..3]\n\
         strictValue : [integer [0..10] : bytes [1..8]; 0..3]\n}\n",
    );
    for (position, key, value) in [(0, false, true), (1, true, false)] {
        let v1::proto3_wire::Kind::Map(map) = kind(&message(&p, "Maps").fields[position]) else {
            panic!("a map")
        };
        assert_eq!(
            map.key.as_ref().unwrap().default_is_legal,
            key,
            "key {position}"
        );
        assert_eq!(
            map.value.as_ref().unwrap().default_is_legal,
            value,
            "value {position}"
        );
        assert!(default_rule(&p, "Maps", position));
    }
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
fn an_enum_map_key_in_ir_keeps_the_projections_integer_key_rule() {
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
fn an_unsupported_message_and_its_users_are_refused() {
    let (model, p) = projection(
        "package p.omitted\n\nstruct Bad {\n  values : [[integer; 0..2]; 0..2]\n}\n\nstruct User {\n  bad : Bad\n}\n\nstruct Holder {\n  pair : (bad: User, good: boolean)\n}\n\nstruct Good {\n  value : boolean\n}\n\nstruct Last {\n  good : Good\n}\n",
    );
    assert_eq!(p.refused.len(), 4);
    let tuple_name = &model.tuples[0].name.as_ref().unwrap().rust;
    let tuple = refusal(&p, &model, tuple_name);
    assert_eq!(
        tuple.cause,
        v1::Proto3RefusalCause::RefusedDependency as i32
    );
    assert_eq!(tuple.member, None);
    assert_eq!(
        tuple.message,
        format!(
            "`{}` needs the refused message `User`: {}",
            model.tuples[0].name.as_ref().unwrap().wire,
            refusal(&p, &model, "User").message
        )
    );
    assert_eq!(
        refusal(&p, &model, "Holder").message,
        format!(
            "`Holder` needs the refused message `{}`: {}",
            model.tuples[0].name.as_ref().unwrap().wire,
            tuple.message
        )
    );
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

fn refusal<'a>(
    p: &'a v1::Proto3Projection,
    model: &v1::Model,
    name: &str,
) -> &'a v1::Proto3Refused {
    p.refused
        .iter()
        .find(|entry| match entry.source.unwrap() {
            v1::proto3_refused::Source::Declaration(index) => {
                model.declarations[index as usize]
                    .name
                    .as_ref()
                    .unwrap()
                    .declared
                    == name
            }
            v1::proto3_refused::Source::Tuple(index) => {
                model.tuples[index as usize].name.as_ref().unwrap().rust == name
            }
        })
        .unwrap_or_else(|| panic!("no refusal for {name}: {:?}", p.refused))
}

fn package(source: &str) -> v2::Package {
    let compiled = ridlc::compile("refusal.typl", source);
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    compiled.package
}

fn field(package: &mut v2::Package, declaration: usize, position: usize) -> &mut v2::Field {
    let Some(v2::decl::Kind::StructDef(def)) = package.decls[declaration].kind.as_mut() else {
        panic!("a struct")
    };
    let Some(v2::struct_member::Member::Field(field)) = def.members[position].member.as_mut()
    else {
        panic!("a field")
    };
    field
}

fn check_refusal(
    package: &v2::Package,
    name: &str,
    cause: v1::Proto3RefusalCause,
    member: Option<&str>,
) {
    let backend =
        ridl_backend_rust::generate_pipeline(package, &[WireEncoding::Proto3], false, &[])
            .expect_err("the codec refuses this input");
    let model = codegen::lower(package, &[]);
    let p = model.proto3.as_ref().unwrap();
    let entry = refusal(p, &model, name);
    assert_eq!(entry.cause, cause as i32);
    assert_eq!(entry.member.as_deref(), member);
    assert_eq!(entry.message, backend.message);
    assert!(!p.messages.iter().any(|message| {
        match (message.source, entry.source) {
            (
                Some(
                    v1::proto3_message::Source::StructDeclaration(a)
                    | v1::proto3_message::Source::UnionDeclaration(a)
                    | v1::proto3_message::Source::RootBox(a),
                ),
                Some(v1::proto3_refused::Source::Declaration(b)),
            ) => a == b,
            (
                Some(v1::proto3_message::Source::Tuple(a)),
                Some(v1::proto3_refused::Source::Tuple(b)),
            ) => a == b,
            _ => false,
        }
    }));
    check_indices(&model, p);
}

#[test]
fn an_optional_array_field_is_refused() {
    check_refusal(
        &package("package p.refusal\nstruct Bad {\n values : [integer; 0..2]?\n}\n"),
        "Bad",
        v1::Proto3RefusalCause::OptionalCollection,
        Some("Bad.values"),
    );
}

#[test]
fn an_optional_map_field_is_refused() {
    check_refusal(
        &package("package p.refusal\nstruct Bad {\n values : [integer : boolean; 0..2]?\n}\n"),
        "Bad",
        v1::Proto3RefusalCause::OptionalCollection,
        Some("Bad.values"),
    );
}

#[test]
fn an_optional_tuple_collection_is_refused() {
    let p = package(
        "package p.refusal\nstruct Holder {\n \
         pair : (values: [integer; 0..2]?, flag: boolean)\n}\n",
    );
    let model = codegen::lower(&p, &[]);
    let name = &model.tuples[0].name.as_ref().unwrap().rust;
    check_refusal(
        &p,
        name,
        v1::Proto3RefusalCause::OptionalCollection,
        Some(&format!("{name}.values")),
    );
}

#[test]
fn an_optional_array_element_is_refused() {
    let mut p = package("package p.refusal\nstruct Bad {\n values : [integer; 0..2]\n}\n");
    let Some(v2::field_type::Kind::Array(a)) =
        field(&mut p, 0, 0).r#type.as_mut().unwrap().kind.as_mut()
    else {
        panic!("an array")
    };
    a.element.as_mut().unwrap().optional = true;
    check_refusal(&p, "Bad", v1::Proto3RefusalCause::OptionalElement, None);
}

#[test]
fn optional_map_keys_and_values_are_refused() {
    for key in [true, false] {
        let mut p =
            package("package p.refusal\nstruct Bad {\n values : [integer : boolean; 0..2]\n}\n");
        let Some(v2::field_type::Kind::Map(m)) =
            field(&mut p, 0, 0).r#type.as_mut().unwrap().kind.as_mut()
        else {
            panic!("a map")
        };
        if key {
            m.key.as_mut().unwrap()
        } else {
            m.value.as_mut().unwrap()
        }
        .optional = true;
        check_refusal(&p, "Bad", v1::Proto3RefusalCause::OptionalElement, None);
    }
}

#[test]
fn an_array_of_arrays_is_refused() {
    check_refusal(
        &package("package p.refusal\nstruct Bad {\n values : [[integer; 0..2]; 0..2]\n}\n"),
        "Bad",
        v1::Proto3RefusalCause::NestedCollection,
        None,
    );
}

#[test]
fn a_map_with_a_collection_value_is_refused() {
    check_refusal(
        &package(
            "package p.refusal\nstruct Bad {\n values : [integer : [boolean; 0..2]; 0..2]\n}\n",
        ),
        "Bad",
        v1::Proto3RefusalCause::NestedCollection,
        None,
    );
}

#[test]
fn a_map_with_a_float_key_is_refused() {
    check_refusal(
        &package("package p.refusal\nstruct Bad {\n values : [float : boolean; 0..2]\n}\n"),
        "Bad",
        v1::Proto3RefusalCause::MapKey,
        None,
    );
}

#[test]
fn a_retired_enum_value_outside_int32_is_refused() {
    check_refusal(
        &package("package p.refusal\nenum Bad {\n OFF = 0\n reserved 2147483648\n}\n"),
        "Bad",
        v1::Proto3RefusalCause::EnumValue,
        None,
    );
}

#[test]
fn an_enum_with_several_values_outside_int32_lists_each() {
    for (user, name, member) in [
        ("", "Bad", None),
        (
            "struct Holder {\n mode : Bad\n}\n",
            "Holder",
            Some("Holder.mode"),
        ),
    ] {
        let p = package(&format!(
            "package p.refusal\n{user}\
             enum Bad {{\n OFF = 0\n reserved 2147483648\n reserved 2147483649\n}}\n"
        ));
        check_refusal(&p, name, v1::Proto3RefusalCause::EnumValue, member);
        let model = codegen::lower(&p, &[]);
        let projection = model.proto3.as_ref().unwrap();
        assert_eq!(
            projection.refused.len(),
            if user.is_empty() { 2 } else { 4 }
        );
        let first = refusal(projection, &model, name);
        let entries: Vec<_> = projection
            .refused
            .iter()
            .filter(|entry| entry.source == first.source)
            .collect();
        assert_eq!(entries.len(), 2);
        for (entry, value) in entries.iter().zip([2147483648_i64, 2147483649]) {
            assert_eq!(entry.cause, v1::Proto3RefusalCause::EnumValue as i32);
            assert_eq!(entry.member.as_deref(), member);
            assert!(
                entry
                    .message
                    .contains(&format!("is {value}, outside int32"))
            );
        }
        assert_ne!(entries[0].message, entries[1].message);
    }
}

#[test]
fn a_named_retired_enum_value_in_ir_is_refused() {
    let mut p = package("package p.refusal\nenum Bad {\n OFF = 0\n}\n");
    let Some(v2::decl::Kind::EnumDef(e)) = p.decls[0].kind.as_mut() else {
        panic!("an enum")
    };
    e.reserved.push(v2::Reserved {
        name: Some("OLD".into()),
        value: Some(i64::from(i32::MAX) + 1),
        ..Default::default()
    });
    check_refusal(
        &p,
        "Bad",
        v1::Proto3RefusalCause::EnumValue,
        Some("Bad.OLD"),
    );
}

#[test]
fn a_live_enum_value_outside_int32_is_refused() {
    let mut p = package("package p.refusal\nenum Bad {\n OFF = 0\n}\n");
    let Some(v2::decl::Kind::EnumDef(e)) = p.decls[0].kind.as_mut() else {
        panic!("an enum")
    };
    e.values[0].value = i64::from(i32::MIN) - 1;
    check_refusal(
        &p,
        "Bad",
        v1::Proto3RefusalCause::EnumValue,
        Some("Bad.OFF"),
    );
}

fn bad_number(number: u32) -> v2::Package {
    let mut p = package("package p.refusal\nstruct Bad {\n value : boolean\n}\n");
    field(&mut p, 0, 0).ordinal = number;
    p
}

#[test]
fn a_field_number_in_the_reserved_range_is_refused() {
    for number in [19000, 19999] {
        check_refusal(
            &bad_number(number),
            "Bad",
            v1::Proto3RefusalCause::FieldNumber,
            Some("Bad.value"),
        );
    }
}

#[test]
fn a_field_number_above_the_limit_is_refused() {
    check_refusal(
        &bad_number(536870912),
        "Bad",
        v1::Proto3RefusalCause::FieldNumber,
        Some("Bad.value"),
    );
}

#[test]
fn field_number_zero_is_refused() {
    check_refusal(
        &bad_number(0),
        "Bad",
        v1::Proto3RefusalCause::FieldNumber,
        Some("Bad.value"),
    );
}

#[test]
fn an_empty_strict_enum_is_refused() {
    let mut p = package("package p.refusal\nenum Bad {\n OFF = 0\n}\n");
    let Some(v2::decl::Kind::EnumDef(e)) = p.decls[0].kind.as_mut() else {
        panic!("an enum")
    };
    e.values.clear();
    check_refusal(&p, "Bad", v1::Proto3RefusalCause::Other, None);
}

const CHAIN: &str = "package p.refusal\n\
    struct First {\n next : [User; 0..2]\n}\n\
    struct User {\n bad : [integer : Bad; 0..2]\n}\n\
    struct Bad {\n values : [[integer; 0..2]; 0..2]\n}\n\
    struct Good {\n value : boolean\n}\n\
    struct Last {\n good : Good\n}\n";

#[test]
fn a_user_of_a_refused_struct_is_refused_for_the_dependency() {
    let (model, p) = projection(CHAIN);
    let bad = refusal(&p, &model, "Bad");
    for (name, other) in [("User", "Bad"), ("First", "User")] {
        let entry = refusal(&p, &model, name);
        assert_eq!(
            entry.cause,
            v1::Proto3RefusalCause::RefusedDependency as i32
        );
        assert_eq!(entry.member, None);
        assert_eq!(
            entry.message,
            format!(
                "`{name}` needs the refused message `{other}`: {}",
                refusal(&p, &model, other).message
            )
        );
    }
    assert_eq!(bad.cause, v1::Proto3RefusalCause::NestedCollection as i32);
    assert_eq!(
        p.refused.iter().map(|r| r.source).collect::<Vec<_>>(),
        [0, 1, 2].map(|i| Some(v1::proto3_refused::Source::Declaration(i)))
    );
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
    check_indices(&model, &p);
}

#[test]
fn a_refused_struct_that_holds_itself_terminates() {
    let mut p = package(
        "package p.refusal\nstruct Bad {\n \
         selfValue : boolean\n values : [integer; 0..2]?\n}\n",
    );
    field(&mut p, 0, 0).r#type.as_mut().unwrap().kind =
        Some(v2::field_type::Kind::Named("Bad".into()));
    check_refusal(
        &p,
        "Bad",
        v1::Proto3RefusalCause::OptionalCollection,
        Some("Bad.values"),
    );
    let model = codegen::lower(&p, &[]);
    assert_eq!(model.proto3.unwrap().refused.len(), 1);
}

#[test]
fn several_refused_members_keep_the_projections_walk_order() {
    let mut p = package(
        "package p.refusal\n\
         struct Bad {\n first : [integer; 0..2]?\n \
         second : [integer : boolean; 0..2]?\n}\n\
         struct Holder {\n pair : (values: [integer; 0..2]?, flag: boolean)\n}\n",
    );
    field(&mut p, 0, 0).ordinal = 3;
    let model = codegen::lower(&p, &[]);
    let projection = model.proto3.as_ref().unwrap();
    assert_eq!(projection.refused.len(), 4);
    assert_eq!(projection.refused[0].member.as_deref(), Some("Bad.first"));
    assert_eq!(projection.refused[1].member.as_deref(), Some("Bad.second"));
    assert_eq!(
        projection.refused[2].source,
        Some(v1::proto3_refused::Source::Declaration(1))
    );
    assert_eq!(
        projection.refused[3].source,
        Some(v1::proto3_refused::Source::Tuple(0))
    );
}

fn check_indices(model: &v1::Model, p: &v1::Proto3Projection) {
    fn wire(w: &v1::Proto3Wire, model: &v1::Model, p: &v1::Proto3Projection) {
        match w.kind.as_ref().unwrap() {
            v1::proto3_wire::Kind::Message(r) => match r.target.unwrap() {
                v1::proto3_message_ref::Target::Message(i) => {
                    assert!((i as usize) < p.messages.len())
                }
                v1::proto3_message_ref::Target::Foreign(i) => {
                    assert!((i as usize) < model.foreign.len());
                }
            },
            v1::proto3_wire::Kind::Array(a) => wire(a.element.as_ref().unwrap(), model, p),
            v1::proto3_wire::Kind::Map(m) => {
                wire(m.key.as_ref().unwrap(), model, p);
                wire(m.value.as_ref().unwrap(), model, p);
            }
            _ => {}
        }
    }
    for m in &p.messages {
        match m.source.unwrap() {
            v1::proto3_message::Source::StructDeclaration(i)
            | v1::proto3_message::Source::UnionDeclaration(i)
            | v1::proto3_message::Source::RootBox(i) => {
                assert!((i as usize) < model.declarations.len());
            }
            v1::proto3_message::Source::Tuple(i) => assert!((i as usize) < model.tuples.len()),
        }
        for f in &m.fields {
            wire(f.wire.as_ref().unwrap(), model, p);
        }
    }
    for r in &p.refused {
        match r.source.unwrap() {
            v1::proto3_refused::Source::Declaration(i) => {
                assert!((i as usize) < model.declarations.len());
            }
            v1::proto3_refused::Source::Tuple(i) => assert!((i as usize) < model.tuples.len()),
        }
    }
}

#[test]
fn every_message_index_points_inside_the_list() {
    for source in [
        CHAIN,
        "package p.refusal\n\
         struct Bad {\n values : [integer; 0..2]?\n}\n\
         struct Good {\n value : boolean\n}\n\
         struct Last {\n good : Good\n}\n",
    ] {
        let (model, p) = projection(source);
        assert!(!p.refused.is_empty());
        check_indices(&model, &p);
    }
}

#[test]
fn an_outside_int32_enum_refuses_its_local_and_foreign_users() {
    for foreign in [false, true] {
        for retired in [false, true] {
            let mut remote = package("package p.remote\nenum Mode {\n OFF = 0\n}\n");
            let Some(v2::decl::Kind::EnumDef(e)) = remote.decls[0].kind.as_mut() else {
                panic!("an enum")
            };
            if retired {
                e.reserved.push(v2::Reserved {
                    name: Some("OLD".into()),
                    value: Some(i64::from(i32::MAX) + 1),
                    ..Default::default()
                });
            } else {
                e.values[0].value = i64::from(i32::MIN) - 1;
            }
            let mut local = package("package p.local\nstruct Holder {\n mode : boolean\n}\n");
            if foreign {
                field(&mut local, 0, 0).r#type.as_mut().unwrap().kind =
                    Some(v2::field_type::Kind::Named("p.remote.Mode".into()));
            } else {
                local.decls.push(remote.decls[0].clone());
                field(&mut local, 0, 0).r#type.as_mut().unwrap().kind =
                    Some(v2::field_type::Kind::Named("Mode".into()));
            }
            let others = if foreign { vec![&remote] } else { vec![] };
            let backend = ridl_backend_rust::generate_pipeline(
                &local,
                &[WireEncoding::Proto3],
                false,
                &others,
            )
            .expect_err("the enum is refused");
            let model = codegen::lower(&local, &others);
            let p = model.proto3.as_ref().unwrap();
            let r = refusal(p, &model, "Holder");
            assert_eq!(r.cause, v1::Proto3RefusalCause::EnumValue as i32);
            assert_eq!(r.member.as_deref(), Some("Holder.mode"));
            assert_eq!(r.message, backend.message);
            check_indices(&model, p);
        }
    }
}

/// Every corpus workspace, with its sibling packages and standard package.
fn corpus_packages() -> Vec<(String, Vec<v2::Package>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut entries: Vec<_> = std::fs::read_dir(&root)
        .expect("the corpus directory is readable")
        .map(|entry| entry.expect("a corpus entry").path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "the corpus must not be empty");
    entries
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .expect("a named entry")
                .to_string_lossy()
                .into_owned();
            let mut db = RidlDatabase::default();
            let output = ridlc::compile_workspace(&mut db, &entry).expect("a corpus entry loads");
            let mut packages: Vec<_> = output
                .checked
                .into_iter()
                .map(|checked| checked.ir)
                .collect();
            packages.push(output.std_ir);
            (name, packages)
        })
        .collect()
}

#[test]
fn every_corpus_projection_survives_the_json_round_trip() {
    let mut with_messages = 0usize;
    for (entry, packages) in corpus_packages() {
        let others: Vec<_> = packages.iter().collect();
        for package in &packages {
            let model = codegen::lower(package, &others);
            let projection = model
                .proto3
                .as_ref()
                .expect("every package has a proto3 projection");
            let json = codegen::to_json_pretty(&model).expect("the model serializes");
            let decoded = codegen::from_json(&json).expect("the model reads back");
            assert_eq!(decoded.proto3, model.proto3, "{entry}: {}", package.name);
            let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
            assert!(
                value["proto3"].is_object(),
                "{entry}: {} carries its projection in JSON",
                package.name
            );
            if !projection.messages.is_empty() {
                with_messages += 1;
                assert!(
                    value["proto3"]["messages"]
                        .as_array()
                        .is_some_and(|messages| !messages.is_empty())
                );
            }
        }
    }
    assert!(
        with_messages > 0,
        "the corpus offers messages to round trip"
    );
}

#[test]
fn every_corpus_index_points_inside_its_list() {
    fn wire(
        model: &v1::Model,
        projection: &v1::Proto3Projection,
        w: &v1::Proto3Wire,
        checked: &mut [usize; 5],
    ) {
        match w.kind.as_ref().expect("a wire kind") {
            v1::proto3_wire::Kind::Message(reference) => {
                match reference.target.expect("a message target") {
                    v1::proto3_message_ref::Target::Message(index) => {
                        assert!(
                            (index as usize) < projection.messages.len(),
                            "a message index is inside the projection"
                        );
                        checked[2] += 1;
                    }
                    v1::proto3_message_ref::Target::Foreign(index) => {
                        assert!(
                            (index as usize) < model.foreign.len(),
                            "a foreign index is inside the model"
                        );
                        checked[4] += 1;
                    }
                }
            }
            v1::proto3_wire::Kind::Enumeration(enumeration) => {
                let reference = enumeration.r#type.as_ref().expect("an enum reference");
                let (count, seen) = if reference.foreign {
                    (model.foreign.len(), &mut checked[4])
                } else {
                    (model.declarations.len(), &mut checked[3])
                };
                assert!(
                    (reference.index as usize) < count,
                    "an enum index is inside its list"
                );
                *seen += 1;
            }
            v1::proto3_wire::Kind::Array(array) => {
                wire(
                    model,
                    projection,
                    array.element.as_deref().expect("an element"),
                    checked,
                );
            }
            v1::proto3_wire::Kind::Map(map) => {
                wire(
                    model,
                    projection,
                    map.key.as_deref().expect("a key"),
                    checked,
                );
                wire(
                    model,
                    projection,
                    map.value.as_deref().expect("a value"),
                    checked,
                );
            }
            _ => {}
        }
    }
    let mut checked = [0usize; 5];
    for (entry, packages) in corpus_packages() {
        let others: Vec<_> = packages.iter().collect();
        for package in &packages {
            let model = codegen::lower(package, &others);
            let projection = model
                .proto3
                .as_ref()
                .expect("every package has a proto3 projection");
            for message in &projection.messages {
                let (index, count, seen) = match message.source.expect("a message source") {
                    v1::proto3_message::Source::StructDeclaration(index)
                    | v1::proto3_message::Source::UnionDeclaration(index)
                    | v1::proto3_message::Source::RootBox(index) => {
                        (index, model.declarations.len(), &mut checked[0])
                    }
                    v1::proto3_message::Source::Tuple(index) => {
                        (index, model.tuples.len(), &mut checked[1])
                    }
                };
                assert!(
                    (index as usize) < count,
                    "{entry}: {}.{} source index is inside its list",
                    package.name,
                    message.name
                );
                *seen += 1;
                for field in &message.fields {
                    wire(
                        &model,
                        projection,
                        field.wire.as_ref().expect("a wire"),
                        &mut checked,
                    );
                }
            }
            for refused in &projection.refused {
                let (index, count, seen) = match refused.source.expect("a refusal source") {
                    v1::proto3_refused::Source::Declaration(index) => {
                        (index, model.declarations.len(), &mut checked[0])
                    }
                    v1::proto3_refused::Source::Tuple(index) => {
                        (index, model.tuples.len(), &mut checked[1])
                    }
                };
                assert!(
                    (index as usize) < count,
                    "{entry}: {} refusal source index is inside its list",
                    package.name
                );
                *seen += 1;
            }
        }
    }
    // The ridlc corpus has no foreign proto3 index. Keep its bound checks above,
    // but require coverage only for the index kinds the corpus contains.
    for (list, count) in ["declaration source", "tuple", "message", "enum reference"]
        .into_iter()
        .zip(checked)
    {
        assert!(count > 0, "the corpus offers {list} indices to check");
    }
    println!(
        "indices checked (declaration source, tuple, message, enum reference, foreign): {checked:?}"
    );
}

fn two_packages(a: &str, b: &str) -> Vec<v2::Package> {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    )
    .unwrap();
    for (name, source) in [("a", a), ("b", b)] {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("ridl.toml"),
            format!("[package]\nname = \"p.{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join("types.typl"), source).unwrap();
    }
    let output = ridlc::compile_workspace(&mut RidlDatabase::default(), root.path()).unwrap();
    assert!(
        !output
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "{:?}",
        output.diagnostics
    );
    output.checked.into_iter().map(|c| c.ir).collect()
}

#[test]
fn foreign_tuples_are_listed_only_in_the_declaring_package() {
    let packages = two_packages(
        "package p.a\nstruct A {\n pair : (value: boolean, nested: (flag: boolean, count: integer))\n}\nstruct Refused {\n pair : (values: [integer; 0..2]?)\n}\n",
        "package p.b\nimport p.a.A\nimport p.a.Refused\nstruct B {\n value : A\n refused : Refused\n}\n",
    );
    let others: Vec<_> = packages.iter().collect();
    for package in &packages {
        let model = codegen::lower(package, &others);
        let p = model.proto3.as_ref().unwrap();
        assert_eq!(model.tuples.len(), 3);
        if package.name == "p.b" {
            assert!(p.refused.is_empty());
            assert_eq!(
                p.messages
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>(),
                ["B"]
            );
            for tuple in &model.tuples {
                let name = tuple.name.as_ref().unwrap();
                assert!(name.rust.is_empty() && name.wire.is_empty());
            }
            assert!(
                matches!(kind(&message(p, "B").fields[0]), v1::proto3_wire::Kind::Message(r) if matches!(r.target, Some(v1::proto3_message_ref::Target::Foreign(_))))
            );
        } else {
            assert_eq!(package.name, "p.a");
            message(p, "A");
            let refused_tuple = p
                .refused
                .iter()
                .find(|entry| matches!(entry.source, Some(v1::proto3_refused::Source::Tuple(_))))
                .expect("the declaring package lists the refused tuple");
            assert_eq!(
                refused_tuple.cause,
                v1::Proto3RefusalCause::OptionalCollection as i32
            );
            assert_eq!(p.refused.len(), 2);
            assert_eq!(
                refusal(p, &model, "Refused").cause,
                v1::Proto3RefusalCause::RefusedDependency as i32
            );
            for (index, tuple) in model.tuples.iter().enumerate() {
                let name = tuple.name.as_ref().unwrap();
                assert!(!name.rust.is_empty() && !name.wire.is_empty());
                if refused_tuple.source == Some(v1::proto3_refused::Source::Tuple(index as u32)) {
                    assert_eq!(
                        refused_tuple.member.as_deref(),
                        Some(format!("{}.values", name.rust).as_str())
                    );
                    continue;
                }
                assert_eq!(
                    message(p, &name.wire).source,
                    Some(v1::proto3_message::Source::Tuple(index as u32))
                );
            }
        }
    }
}

#[test]
fn collection_message_references_are_remapped_after_a_refusal() {
    let (_, p) = projection(
        "package p.remap\nstruct Bad {\n values : [integer; 0..2]?\n}\nstruct Item {\n x : boolean\n}\nstruct Holder {\n items : [Item; 0..2]\n byKey : [integer [0..9] : Item; 0..2]\n}\n",
    );
    assert_eq!(
        p.messages
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["Item", "Holder"]
    );
    let holder = message(&p, "Holder");
    let v1::proto3_wire::Kind::Array(array) = kind(&holder.fields[0]) else {
        panic!("an array")
    };
    let v1::proto3_wire::Kind::Map(map) = kind(&holder.fields[1]) else {
        panic!("a map")
    };
    for wire in [
        array.element.as_deref().unwrap(),
        map.value.as_deref().unwrap(),
    ] {
        assert!(
            matches!(wire.kind.as_ref(), Some(v1::proto3_wire::Kind::Message(r)) if r.target == Some(v1::proto3_message_ref::Target::Message(0)))
        );
    }
}

fn union_arm(package: &mut v2::Package, declaration: usize) -> &mut v2::UnionArm {
    let Some(v2::decl::Kind::UnionDef(def)) = package.decls[declaration].kind.as_mut() else {
        panic!("a union")
    };
    &mut def.arms[0]
}

fn union_package() -> v2::Package {
    package("package p.refusal\nunion Bad {\n value : Item\n}\nstruct Item {\n flag : boolean\n}\n")
}

#[test]
fn union_arm_field_numbers_are_checked_after_resolution() {
    for number in [0, 19000, 536870912] {
        let mut p = union_package();
        union_arm(&mut p, 0).ordinal = number;
        check_refusal(
            &p,
            "Bad",
            v1::Proto3RefusalCause::FieldNumber,
            Some("Bad.value"),
        );
    }
    let mut p = union_package();
    let arm = union_arm(&mut p, 0);
    arm.ordinal = 0;
    arm.type_ref = "Missing".into();
    check_refusal(&p, "Bad", v1::Proto3RefusalCause::Other, None);
}

#[test]
fn field_numbers_beside_the_reserved_range_and_at_the_limit_are_accepted() {
    for number in [18999, 20000, 536870911] {
        let p = bad_number(number);
        let model = codegen::lower(&p, &[]);
        let projected = model.proto3.as_ref().unwrap();
        assert!(projected.refused.is_empty(), "{number}");
        assert_eq!(message(projected, "Bad").fields[0].number, number);
        ridl_backend_rust::generate_pipeline(&p, &[WireEncoding::Proto3], false, &[]).unwrap();
    }
}

#[test]
fn an_empty_union_is_refused() {
    let mut p = union_package();
    let Some(v2::decl::Kind::UnionDef(def)) = p.decls[0].kind.as_mut() else {
        panic!("a union")
    };
    def.arms.clear();
    check_refusal(&p, "Bad", v1::Proto3RefusalCause::Other, None);
}

#[test]
fn an_empty_open_enum_and_its_user_keep_their_messages() {
    let mut p =
        package("package p.open\nopen enum Mode {\n ON = 1\n}\nstruct User {\n mode : Mode\n}\n");
    let Some(v2::decl::Kind::EnumDef(def)) = p.decls[0].kind.as_mut() else {
        panic!("an enum")
    };
    def.values.clear();
    let model = codegen::lower(&p, &[]);
    let projected = model.proto3.as_ref().unwrap();
    assert!(projected.refused.is_empty());
    for name in ["ModeBox", "User"] {
        assert!(default_rule(projected, name, 0));
    }
    ridl_backend_rust::generate_pipeline(&p, &[WireEncoding::Proto3], false, &[]).unwrap();
}

#[test]
fn malformed_field_types_report_the_projections_refusal() {
    for case in [
        "field",
        "array",
        "key",
        "value",
        "unresolved",
        "primitive",
        "width",
        "int_width",
        "float_width",
    ] {
        let ty = match case {
            "array" => "[integer; 0..2]",
            "key" | "value" => "[integer : boolean; 0..2]",
            "width" | "int_width" => "integer [0..255]",
            "float_width" => "float [0.0..1.0]",
            _ => "boolean",
        };
        let mut p = package(&format!(
            "package p.refusal\nstruct Bad {{\n value : {ty}\n}}\n"
        ));
        let f = field(&mut p, 0, 0);
        match case {
            "field" => f.r#type = None,
            "array" => {
                let Some(v2::field_type::Kind::Array(a)) = f.r#type.as_mut().unwrap().kind.as_mut()
                else {
                    panic!("an array")
                };
                a.element = None;
            }
            "key" | "value" => {
                let Some(v2::field_type::Kind::Map(m)) = f.r#type.as_mut().unwrap().kind.as_mut()
                else {
                    panic!("a map")
                };
                if case == "key" {
                    m.key = None;
                } else {
                    m.value = None;
                }
            }
            "unresolved" => {
                f.r#type.as_mut().unwrap().kind =
                    Some(v2::field_type::Kind::Named("Missing".into()))
            }
            "primitive" => {
                f.r#type.as_mut().unwrap().kind = Some(v2::field_type::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                ))
            }
            _ => {
                let Some(v2::field_type::Kind::InlineScalar(s)) =
                    f.r#type.as_mut().unwrap().kind.as_mut()
                else {
                    panic!("a scalar")
                };
                s.width = match case {
                    "int_width" => Some(v2::type_def::Width::IntWidth(0)),
                    "float_width" => Some(v2::type_def::Width::FloatWidth(0)),
                    _ => None,
                };
            }
        }
        println!("malformed case: {case}");
        check_refusal(
            &p,
            "Bad",
            v1::Proto3RefusalCause::Other,
            if case == "field" {
                Some("Bad.value")
            } else {
                None
            },
        );
    }
}

#[test]
fn a_union_enum_refusal_names_the_using_arm() {
    let mut p =
        package("package p.refusal\nunion Bad {\n value : Mode\n}\nenum Mode {\n OFF = 0\n}\n");
    let Some(v2::decl::Kind::EnumDef(def)) = p.decls[1].kind.as_mut() else {
        panic!("an enum")
    };
    def.values[0].value = i64::from(i32::MAX) + 1;
    check_refusal(
        &p,
        "Bad",
        v1::Proto3RefusalCause::EnumValue,
        Some("Bad.value"),
    );
}

#[test]
fn a_refused_dependency_uses_the_first_field_and_first_failure() {
    let (_, p) = projection(
        "package p.first\nstruct User {\n first : Bad\n second : Worse\n}\nstruct Bad {\n first : [integer; 0..2]?\n second : [boolean; 0..2]?\n}\nstruct Worse {\n values : [integer; 0..2]?\n}\n",
    );
    let user = &p.refused[0];
    assert_eq!(user.cause, v1::Proto3RefusalCause::RefusedDependency as i32);
    assert_eq!(user.member, None);
    assert_eq!(p.refused[1].member.as_deref(), Some("Bad.first"));
    assert_eq!(
        user.message,
        format!(
            "`User` needs the refused message `Bad`: {}",
            p.refused[1].message
        )
    );
}

#[test]
fn a_foreign_enum_default_follows_its_zero_member_and_openness() {
    for (open, value, expected) in [("", 1, false), ("", 0, true), ("open ", 1, true)] {
        let remote = package(&format!(
            "package p.remote\n{open}enum Mode {{\n VALUE = {value}\n}}\n"
        ));
        let mut local = package("package p.local\nstruct User {\n mode : boolean\n}\n");
        field(&mut local, 0, 0).r#type.as_mut().unwrap().kind =
            Some(v2::field_type::Kind::Named("p.remote.Mode".into()));
        let model = codegen::lower(&local, &[&remote]);
        let p = model.proto3.as_ref().unwrap();
        assert_eq!(default_rule(p, "User", 0), expected);
        assert!(
            matches!(kind(&message(p, "User").fields[0]), v1::proto3_wire::Kind::Enumeration(e) if e.r#type.as_ref().unwrap().foreign)
        );
    }
}

#[test]
fn a_foreign_struct_is_kept_even_when_its_own_projection_refuses_it() {
    let packages = two_packages(
        "package p.a\nstruct Remote {\n values : [integer; 0..2]?\n}\n",
        "package p.b\nimport p.a.Remote\nstruct Bad {\n values : [integer; 0..2]?\n}\nstruct User {\n remote : Remote\n}\n",
    );
    let others: Vec<_> = packages.iter().collect();
    for package in &packages {
        let model = codegen::lower(package, &others);
        let p = model.proto3.as_ref().unwrap();
        if package.name == "p.a" {
            assert_eq!(
                refusal(p, &model, "Remote").cause,
                v1::Proto3RefusalCause::OptionalCollection as i32
            );
            assert!(p.messages.is_empty());
        } else {
            assert_eq!(
                p.messages
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>(),
                ["User"]
            );
            let v1::proto3_wire::Kind::Message(r) = kind(&message(p, "User").fields[0]) else {
                panic!("a message")
            };
            assert_eq!(r.target, Some(v1::proto3_message_ref::Target::Foreign(0)));
            assert_eq!(
                model.foreign[0]
                    .declaration
                    .as_ref()
                    .unwrap()
                    .name
                    .as_ref()
                    .unwrap()
                    .declared,
                "Remote"
            );
            assert_eq!(p.refused.len(), 1);
            assert_eq!(
                refusal(p, &model, "Bad").cause,
                v1::Proto3RefusalCause::OptionalCollection as i32
            );
        }
    }
}

#[test]
fn foreign_scalars_enums_and_enum_sets_work_as_elements_and_keys() {
    let remote = package(
        "package p.remote\ntype Count : integer [0..255]\nenum Mode {\n OFF = 0\n ON = 1\n}\nenumset Flags {\n ONE = 0\n}\n",
    );
    let mut local = package(
        "package p.local\nstruct User {\n counts : [boolean; 0..2]\n modes : [boolean; 0..2]\n flags : [boolean; 0..2]\n byCount : [integer : boolean; 0..2]\n byMode : [integer : boolean; 0..2]\n byFlags : [integer : boolean; 0..2]\n byBool : [boolean : boolean; 0..2]\n}\n",
    );
    for (position, name) in ["Count", "Mode", "Flags"].into_iter().enumerate() {
        let reference = v2::field_type::Kind::Named(format!("p.remote.{name}"));
        let Some(v2::field_type::Kind::Array(a)) = field(&mut local, 0, position)
            .r#type
            .as_mut()
            .unwrap()
            .kind
            .as_mut()
        else {
            panic!("an array")
        };
        a.element.as_mut().unwrap().kind = Some(reference.clone());
        let Some(v2::field_type::Kind::Map(m)) = field(&mut local, 0, position + 3)
            .r#type
            .as_mut()
            .unwrap()
            .kind
            .as_mut()
        else {
            panic!("a map")
        };
        m.key.as_mut().unwrap().kind = Some(reference);
    }
    let model = codegen::lower(&local, &[&remote]);
    let p = model.proto3.as_ref().unwrap();
    assert!(p.refused.is_empty(), "{:?}", p.refused);
    let user = message(p, "User");
    for position in 0..3 {
        let v1::proto3_wire::Kind::Array(a) = kind(&user.fields[position]) else {
            panic!("an array")
        };
        let v1::proto3_wire::Kind::Map(m) = kind(&user.fields[position + 3]) else {
            panic!("a map")
        };
        assert!(a.packed);
        assert_eq!(a.element, m.key);
        match a.element.as_ref().unwrap().kind.as_ref().unwrap() {
            v1::proto3_wire::Kind::Scalar(s) => {
                assert_eq!(s.r#type, v1::Proto3ScalarType::Uint32 as i32)
            }
            v1::proto3_wire::Kind::Enumeration(e) => assert!(e.r#type.as_ref().unwrap().foreign),
            other => panic!("unexpected wire: {other:?}"),
        }
    }
    let v1::proto3_wire::Kind::Map(m) = kind(&user.fields[6]) else {
        panic!("a map")
    };
    assert!(
        matches!(m.key.as_ref().unwrap().kind.as_ref(), Some(v1::proto3_wire::Kind::Scalar(s)) if s.r#type == v1::Proto3ScalarType::Bool as i32)
    );
    ridl_backend_rust::proto3_layout(&local, &[&remote]).unwrap();
}

#[test]
fn a_missing_union_arm_type_has_the_same_text_at_the_backend_model_entry() {
    use codegen::Backend;
    let mut model = codegen::lower(&union_package(), &[]);
    let Some(v1::declaration::Kind::Union(def)) = model.declarations[0].kind.as_mut() else {
        panic!("a union")
    };
    def.arms[0].r#type = None;
    let response = ridl_backend_rust::Backend.generate(&v1::CodegenRequest {
        model: Some(model),
        options: vec![v1::BackendOption {
            key: "encodings".into(),
            value: "proto3".into(),
        }],
        ..Default::default()
    });
    assert!(response.files.is_empty());
    assert_eq!(response.diagnostics.len(), 1);
    assert_eq!(response.diagnostics[0].message, "`Bad.value` names no type");
}
