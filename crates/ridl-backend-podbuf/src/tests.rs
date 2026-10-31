use ridl_ir::v2;

use crate::{GenerateError, Generated, generate};

fn compile(source: &str) -> v2::Package {
    ridlc::compile("fixture.typl", source).package
}

fn generate_fixture(source: &str) -> Generated {
    generate(&compile(source)).expect("generate")
}

fn generate_fixture_err(source: &str) -> GenerateError {
    generate(&compile(source)).expect_err("a refusal")
}

const FAULT_EVENT: &str = "package veh.cluster\n\
struct FaultEvent {\n\
  code : integer [0..65535]\n\
  message : string [0..256]\n\
  timestamp : integer [-9223372036854775808..9223372036854775807]\n\
}\n";

const DOOR: &str = "package veh.cluster\n\
struct DoorPayload {\n\
  sensorId : integer [0..15]\n\
  isOpen : boolean\n\
}\n";

const TOO_LARGE: &str = "package limits\n\
struct Huge {\n\
  items : [integer [0..255]; 0..70000]\n\
}\n";

#[test]
fn fault_event_head_is_declared_with_pads_and_static_asserts() {
    let h = generate_fixture(FAULT_EVENT).header_source;
    assert!(
        h.contains("typedef struct veh_cluster_fault_event_head {"),
        "{h}"
    );
    assert!(h.contains("_Alignas(8) uint16_t code;"), "{h}");
    assert!(h.contains("uint8_t _pad2[2];"), "{h}");
    assert!(h.contains("ridl_podbuf_desc message;"), "{h}");
    assert!(h.contains("uint8_t _pad12[4];"), "{h}");
    assert!(h.contains("\n  int64_t timestamp;"), "{h}");
    assert!(
        h.contains("static inline int64_t veh_cluster_fault_event_timestamp("),
        "{h}"
    );
    assert!(
        h.contains("_Static_assert(sizeof(veh_cluster_fault_event_head) == 24"),
        "{h}"
    );
    assert!(
        h.contains("_Static_assert(offsetof(veh_cluster_fault_event_head, timestamp) == 16"),
        "{h}"
    );
}

#[test]
fn the_header_refuses_a_big_endian_target() {
    assert!(
        generate_fixture(DOOR)
            .header_source
            .contains("#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__\n#error")
    );
}

#[test]
fn a_type_above_the_limits_is_refused_naming_it() {
    let err = generate_fixture_err(TOO_LARGE);
    assert!(err.message.contains("`limits.Huge`"), "{}", err.message);
}

/// A model whose tuple members carry no name — one written before
/// `position_name` existed — names them by position.
#[test]
fn a_tuple_member_with_no_name_is_named_by_its_position() {
    let package = compile(
        "package q\n\
         struct S {\n  t : (a: integer [0..255], b: integer [0..255])\n}\n",
    );
    let mut model = ridl_ir::codegen::lower(&package, &[]);
    let projection = model.podbuf.as_mut().expect("a projection");
    for head in &mut projection.heads {
        for field in &mut head.fields {
            if let Some(source) = field.source.as_mut() {
                source.position_name.clear();
            }
        }
    }
    let h = crate::generate_model(&package, &[], &model)
        .expect("generate")
        .header_source;
    assert!(h.contains("_Alignas(8) uint8_t f1;"), "{h}");
    assert!(h.contains("\n  uint8_t f2;"), "{h}");
    assert!(!h.contains("uint8_t a;"), "{h}");
    assert!(h.contains("q_st_f2_at("), "{h}");
}

/// A member whose pinned snake case is a C23 keyword (`staticAssert` is
/// `static_assert`) is escaped with a trailing underscore, as every reserved
/// word is, for a struct field and for a tuple member. The test pins the eight
/// C23 spellings that are not also C11 keywords, except `bool`, `true` and
/// `false`, and `asm`, the GNU-dialect keyword.
#[test]
fn a_member_named_by_a_c23_keyword_is_escaped() {
    let h = generate_fixture(
        "package q\n\
         struct S {\n  staticAssert : integer [0..255]\n  typeof : integer [0..255]\n\
         \x20 alignas : integer [0..255]\n  alignof : integer [0..255]\n\
         \x20 threadLocal : integer [0..255]\n  typeofUnqual : integer [0..255]\n\
         \x20 asm : integer [0..255]\n\
         \x20 t : (constexpr: integer [0..255], nullptr: integer [0..255])\n}\n",
    )
    .header_source;
    assert!(h.contains("_Alignas(8) uint8_t static_assert_;"), "{h}");
    assert!(h.contains("q_s_static_assert_at("), "{h}");
    for member in [
        "typeof_",
        "alignas_",
        "alignof_",
        "thread_local_",
        "typeof_unqual_",
        "asm_",
    ] {
        assert!(
            h.contains(&format!("\n  uint8_t {member};")),
            "{member}: {h}"
        );
        assert!(h.contains(&format!("q_s_{member}at(")), "{member}: {h}");
    }
    assert!(h.contains("_Alignas(8) uint8_t constexpr_;"), "{h}");
    assert!(h.contains("\n  uint8_t nullptr_;"), "{h}");
    assert!(h.contains("offsetof(q_st_head, nullptr_) == 1"), "{h}");
    for bare in [
        " static_assert;",
        " typeof;",
        " alignas;",
        " alignof;",
        " thread_local;",
        " typeof_unqual;",
        " asm;",
        " constexpr;",
        " nullptr;",
    ] {
        assert!(!h.contains(bare), "{bare}: {h}");
    }
}

/// A `NestedTuple` head whose `wire` is empty, as a producer older than that
/// field writes it, is named from its path: `pod_parts_hub_size_3_head`
/// for position 3 of `Hub.size`, where a producer that fills `wire` gives
/// `pod_parts_hub_size_field3_head`.
#[test]
fn a_nested_tuple_with_no_wire_name_is_named_from_its_path() {
    use ridl_ir::codegen::v1;

    let dir = tempfile::tempdir().expect("a temporary directory");
    let write = |path: &str, text: &str| {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "ridl.toml",
        "[workspace]\nmembers = [\"parts\", \"vehicle\"]\n",
    );
    write(
        "parts/ridl.toml",
        "[package]\nname = \"pod.parts\"\nversion = \"1.0.0\"\n",
    );
    write(
        "parts/parts.typl",
        "package pod.parts\n\nstruct Hub {\n  size : (w: integer [0..10], h: integer [0..10], \
         inner: (p: integer [0..10], q: integer [0..10]))\n}\n",
    );
    write(
        "vehicle/ridl.toml",
        "[package]\nname = \"pod.vehicle\"\nversion = \"1.0.0\"\n",
    );
    write(
        "vehicle/vehicle.typl",
        "package pod.vehicle\n\nimport pod.parts.Hub\n\nstruct Axle {\n  hub : Hub\n}\n",
    );
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, dir.path()).expect("the workspace loads");
    let mut packages: Vec<v2::Package> = output
        .checked
        .iter()
        .map(|checked| checked.ir.clone())
        .collect();
    packages.push(output.std_ir.clone());
    let others: Vec<&v2::Package> = packages.iter().collect();
    let vehicle = packages
        .iter()
        .find(|package| package.name == "pod.vehicle")
        .expect("pod.vehicle");

    let mut model = ridl_ir::codegen::lower(vehicle, &others);
    let projection = model.podbuf.as_mut().expect("a podbuf projection");
    let mut cleared = 0;
    for head in &mut projection.heads {
        if let Some(v1::podbuf_head::Source::NestedTuple(nested)) = head.source.as_mut() {
            nested.wire.clear();
            cleared += 1;
        }
    }
    assert_eq!(cleared, 2, "the heads of `Hub.size` and of its position 3");
    let header = crate::generate_model(vehicle, &others, &model)
        .expect("generate")
        .header_source;
    assert!(
        header.contains("typedef struct pod_parts_hub_size_3_head {"),
        "{header}"
    );
    assert!(
        !header.contains("pod_parts_hub_size_field3_head"),
        "{header}"
    );
}

/// The two fallback names of `Header::type_name`, which checked input does
/// not reach: a tuple of this package whose wire name is empty is named
/// `<prefix>_tuple<N>_head` by its index in `Model.tuples`, and a head with
/// no source `<prefix>_head<N>` by its index in the head list.
#[test]
fn a_head_with_no_wire_name_or_no_source_takes_a_fallback_name() {
    use ridl_ir::codegen::v1;

    let dir = tempfile::tempdir().expect("a temporary directory");
    let write = |path: &str, text: &str| {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write("ridl.toml", "[workspace]\nmembers = [\"parts\"]\n");
    write(
        "parts/ridl.toml",
        "[package]\nname = \"pod.parts\"\nversion = \"1.0.0\"\n",
    );
    write(
        "parts/parts.typl",
        "package pod.parts\n\nstruct Hub {\n  size : (w: integer [0..10], h: integer [0..10])\n}\n",
    );
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, dir.path()).expect("the workspace loads");
    let mut packages: Vec<v2::Package> = output
        .checked
        .iter()
        .map(|checked| checked.ir.clone())
        .collect();
    packages.push(output.std_ir.clone());
    let others: Vec<&v2::Package> = packages.iter().collect();
    let parts = &packages[0];
    let model = ridl_ir::codegen::lower(parts, &others);
    let tuple_head = |model: &v1::Model| -> usize {
        let projection = model.podbuf.as_ref().expect("a podbuf projection");
        projection
            .heads
            .iter()
            .position(|head| matches!(head.source, Some(v1::podbuf_head::Source::Tuple(0))))
            .expect("the head of tuple 0")
    };

    let mut unnamed = model.clone();
    unnamed.tuples[0]
        .name
        .as_mut()
        .expect("a name")
        .wire
        .clear();
    let header = crate::generate_model(parts, &others, &unnamed)
        .expect("generate")
        .header_source;
    assert!(
        header.contains("typedef struct pod_parts_tuple0_head {"),
        "{header}"
    );
    assert!(!header.contains("pod_parts_hub_size_head"), "{header}");

    let mut sourceless = model.clone();
    let at = tuple_head(&sourceless);
    sourceless.podbuf.as_mut().unwrap().heads[at].source = None;
    let header = crate::generate_model(parts, &others, &sourceless)
        .expect("generate")
        .header_source;
    assert!(
        header.contains(&format!("typedef struct pod_parts_head{at} {{")),
        "{header}"
    );
}
