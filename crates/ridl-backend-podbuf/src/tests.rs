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
