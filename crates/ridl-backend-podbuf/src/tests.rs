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
/// word is, for a struct field and for a tuple member; each of the eight C23
/// keywords is pinned.
#[test]
fn a_member_named_by_a_c23_keyword_is_escaped() {
    let h = generate_fixture(
        "package q\n\
         struct S {\n  staticAssert : integer [0..255]\n  typeof : integer [0..255]\n\
         \x20 alignas : integer [0..255]\n  alignof : integer [0..255]\n\
         \x20 threadLocal : integer [0..255]\n  typeofUnqual : integer [0..255]\n\
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
        " constexpr;",
        " nullptr;",
    ] {
        assert!(!h.contains(bare), "{bare}: {h}");
    }
}
