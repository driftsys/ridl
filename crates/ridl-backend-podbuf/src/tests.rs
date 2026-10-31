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

/// One named type of each box form: an integer, a float and a boolean scalar,
/// an enum, an enum set, a bounded string, bytes with unequal bounds and bytes
/// with equal bounds, and `bytes [0]`, whose head has no byte.
const BOXES: &str = "package p.boxes\n\
type Count : integer [0..1000]\n\
type Level : float [-1.0e300..1.0e300]\n\
type Flag : boolean\n\
enum Gear {\n  PARK = 0\n  DRIVE = 1\n}\n\
enumset Gears : Gear\n\
type Label : string [0..16]\n\
type Blob : bytes [0..16]\n\
type Mac : bytes [6]\n\
type Empty : bytes [0]\n";

/// Every named scalar, enum and enum set gets a head named as the head of a
/// declaration, with one `value` member at offset 0, its pads and its layout
/// assertions, and the two accessors of a root head with the return type of
/// its form.
#[test]
fn a_boxed_root_has_a_head_with_one_value_member_and_two_accessors() {
    let h = generate_fixture(BOXES).header_source;
    struct Case {
        decl: &'static str,
        member: &'static str,
        size: u32,
        pad: Option<&'static str>,
        at: &'static str,
        whole: &'static str,
    }
    // Every head is 8 bytes.
    let cases = [
        Case {
            decl: "count",
            member: "uint16_t value",
            size: 2,
            pad: Some("uint8_t _pad2[6];"),
            at: "static inline uint16_t p_boxes_count_value_at(const uint8_t *h, uint32_t sz) {",
            whole: "static inline uint16_t p_boxes_count_value(const uint8_t *buf) {",
        },
        Case {
            decl: "level",
            member: "double value",
            size: 8,
            pad: None,
            at: "static inline double p_boxes_level_value_at(const uint8_t *h, uint32_t sz) {",
            whole: "static inline double p_boxes_level_value(const uint8_t *buf) {",
        },
        Case {
            decl: "flag",
            member: "uint8_t value",
            size: 1,
            pad: Some("uint8_t _pad1[7];"),
            at: "static inline uint8_t p_boxes_flag_value_at(const uint8_t *h, uint32_t sz) {",
            whole: "static inline uint8_t p_boxes_flag_value(const uint8_t *buf) {",
        },
        Case {
            decl: "gear",
            member: "uint8_t value",
            size: 1,
            pad: Some("uint8_t _pad1[7];"),
            at: "static inline uint8_t p_boxes_gear_value_at(const uint8_t *h, uint32_t sz) {",
            whole: "static inline uint8_t p_boxes_gear_value(const uint8_t *buf) {",
        },
        Case {
            decl: "gears",
            member: "uint8_t value",
            size: 1,
            pad: Some("uint8_t _pad1[7];"),
            at: "static inline uint8_t p_boxes_gears_value_at(const uint8_t *h, uint32_t sz) {",
            whole: "static inline uint8_t p_boxes_gears_value(const uint8_t *buf) {",
        },
        Case {
            decl: "label",
            member: "ridl_podbuf_desc value",
            size: 8,
            pad: None,
            at: "static inline const char *p_boxes_label_value_at(const uint8_t *buf, const uint8_t *h, uint32_t sz, uint32_t *len) {",
            whole: "static inline const char *p_boxes_label_value(const uint8_t *buf, uint32_t *len) {",
        },
        Case {
            decl: "blob",
            member: "ridl_podbuf_desc value",
            size: 8,
            pad: None,
            at: "static inline const uint8_t *p_boxes_blob_value_at(const uint8_t *buf, const uint8_t *h, uint32_t sz, uint32_t *len) {",
            whole: "static inline const uint8_t *p_boxes_blob_value(const uint8_t *buf, uint32_t *len) {",
        },
        Case {
            decl: "mac",
            member: "uint8_t value[6]",
            size: 6,
            pad: Some("uint8_t _pad6[2];"),
            at: "static inline const uint8_t *p_boxes_mac_value_at(const uint8_t *h, uint32_t sz) {",
            whole: "static inline const uint8_t *p_boxes_mac_value(const uint8_t *buf) {",
        },
    ];
    for Case {
        decl,
        member,
        size,
        pad,
        at,
        whole,
    } in cases
    {
        let name = format!("p_boxes_{decl}_head");
        let guard = format!("P_BOXES_{}_HEAD_DEFINED", decl.to_uppercase());
        let mut body = format!("  _Alignas(8) {member};\n");
        if let Some(pad) = pad {
            body.push_str(&format!("  {pad}\n"));
        }
        let block = format!(
            "#ifndef {guard}\n#define {guard}\ntypedef struct {name} {{\n{body}}} {name};\n"
        );
        assert!(h.contains(&block), "{decl}: {block}\n{h}");
        for line in [
            format!("_Static_assert(sizeof({name}) == 8, \"{name} size\");"),
            format!("_Static_assert(_Alignof({name}) == 8, \"{name} alignment\");"),
            format!("_Static_assert(offsetof({name}, value) == 0, \"{name}.value offset\");"),
            format!(
                "_Static_assert(sizeof((({name} *)0)->value) == {size}, \"{name}.value size\");"
            ),
        ] {
            assert!(h.contains(&line), "{decl}: {line}\n{h}");
        }
        assert!(h.contains(at), "{decl}: {at}\n{h}");
        assert!(h.contains(whole), "{decl}: {whole}\n{h}");
    }
    // `bytes [0]` generates, and has no head and no accessor.
    assert!(!h.contains("p_boxes_empty"), "{h}");
    // No member is named `arm`.
    assert!(!h.contains(" arm;"), "{h}");
}

/// The box heads are written after the heads of the projection, and their
/// accessors after the other accessors.
#[test]
fn the_box_heads_follow_the_projection_heads() {
    let h = generate_fixture(
        "package p.order\n\
         type Count : integer [0..1000]\n\
         struct Pair {\n  a : integer [0..255]\n}\n",
    )
    .header_source;
    let at = |needle: &str| h.find(needle).unwrap_or_else(|| panic!("{needle}: {h}"));
    assert!(at("typedef struct p_order_pair_head") < at("typedef struct p_order_count_head"));
    assert!(at("p_order_pair_a_at(") < at("p_order_count_value_at("));
    assert!(at("typedef struct p_order_count_head") < at("p_order_pair_a_at("));
}

/// A constant equal to the guard macro of a head is refused, for the head of
/// a box and for the head of a struct: the `#ifndef` would drop the head.
#[test]
fn a_constant_equal_to_a_head_guard_is_refused() {
    let err = generate_fixture_err("package p.g\nenum Gear {\n  PARK = 0\n  HEAD_DEFINED = 1\n}\n");
    assert!(
        err.message.contains("`P_G_GEAR_HEAD_DEFINED`")
            && err.message.contains("the enum value `HEAD_DEFINED`")
            && err.message.contains("the guard of `p_g_gear_head`"),
        "{}",
        err.message
    );
    let err = generate_fixture_err(
        "package p.g\nenum Gear {\n  PARK = 0\n  BOX_HEAD_DEFINED = 1\n}\n\
         struct GearBox {\n  a : integer [0..255]\n}\n",
    );
    assert!(
        err.message.contains("`P_G_GEAR_BOX_HEAD_DEFINED`")
            && err.message.contains("the enum value `BOX_HEAD_DEFINED`")
            && err.message.contains("the guard of `p_g_gear_box_head`"),
        "{}",
        err.message
    );
}

/// The accessor of a box can collide with the accessor of a struct: `Gear`'s
/// `p_g_gear_value_at` and the field `at` of `GearValue`, which the existing
/// claims refuse, naming both claimants.
#[test]
fn a_box_accessor_equal_to_a_struct_accessor_is_refused() {
    let err = generate_fixture_err(
        "package p.g\nenum Gear {\n  PARK = 0\n  DRIVE = 1\n}\n\
         struct GearValue {\n  at : integer [0..255]\n}\n",
    );
    assert!(
        err.message.contains("`p_g_gear_value_at`")
            && err
                .message
                .contains("the accessor of `at` of the head of `GearValue`")
            && err
                .message
                .contains("the accessor of `value` of the head of `Gear`"),
        "{}",
        err.message
    );
}

/// A box head whose name equals the name of a tuple head is refused, and the
/// message names both owners.
#[test]
fn a_box_head_equal_to_a_tuple_head_is_refused_naming_both() {
    let err = generate_fixture_err(
        "package p.t\ntype HubSize : integer [0..255]\n\
         struct Hub {\n  size : (width: integer [0..10], height: integer [0..10])\n}\n",
    );
    assert!(
        err.message.contains("`p_t_hub_size_head`")
            && err.message.contains("the head of `HubSize`")
            && err.message.contains("the head of the tuple at `Hub.size`"),
        "{}",
        err.message
    );
}

/// A box with no podbuf bound refuses the header, as a struct does.
#[test]
fn a_box_with_no_size_bound_is_refused_naming_it() {
    let err = generate_fixture_err("package p.huge\ntype Huge : string [0..1100000000]\n");
    assert!(
        err.message.contains(
            "`p.huge.Huge` has no podbuf size bound: a largest buffer above 4294967295 bytes"
        ),
        "{}",
        err.message
    );
}

/// The owner texts of `Header::owner_text` for the heads of a union arm, a
/// map entry, and a tuple whose model carries no declaration path.
#[test]
fn the_owner_text_names_an_arm_a_map_entry_and_a_tuple_with_no_path() {
    let package = compile(
        "package p.o\n\
         struct W {\n  r : integer [0..10]\n}\n\
         struct V {\n  s : integer [0..10]\n}\n\
         union U {\n  a : W\n  b : V\n}\n\
         struct M {\n  byKey : [integer [0..10] : integer [0..10]; 0..4]\n}\n\
         struct T {\n  pair : (x: integer [0..10], y: integer [0..10])\n}\n",
    );
    let mut model = ridl_ir::codegen::lower(&package, &[]);
    for tuple in &mut model.tuples {
        tuple.origin = None;
    }
    let projection = model.podbuf.clone().expect("a projection");
    assert!(projection.refused.is_empty(), "{:?}", projection.refused);
    let header = crate::Header::new(&model, &projection);
    let texts: Vec<String> = (0..header.heads.len())
        .map(|at| header.owner_text(at))
        .collect();
    for expected in [
        "the head of the arm `a` of `U`",
        "the entry head of the map at `M.byKey`",
        "the head of the tuple `TPair`",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected}: {texts:?}"
        );
    }
    assert!(!texts.iter().any(|text| text == "a head"), "{texts:?}");
}

/// The owner texts of the heads an importing header defines again: a foreign
/// declaration and a tuple nested in it.
#[test]
fn the_owner_text_names_a_foreign_head_and_a_nested_tuple() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, &path).expect("the workspace loads");
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
    let model = ridl_ir::codegen::lower(vehicle, &others);
    let projection = model.podbuf.clone().expect("a projection");
    let header = crate::Header::new(&model, &projection);
    let texts: Vec<String> = (0..header.heads.len())
        .map(|at| header.owner_text(at))
        .collect();
    for expected in [
        "the head of `pod.parts.Wheel`",
        "the head of the tuple at `pod.parts.Hub.size`",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected}: {texts:?}"
        );
    }
    assert!(!texts.iter().any(|text| text == "a head"), "{texts:?}");
}

/// `Header::new` skips a box whose head is unset and a box whose declaration
/// index is out of range, as the Rust codec skips them.
#[test]
fn a_box_with_no_head_or_an_unknown_declaration_is_skipped() {
    use ridl_ir::codegen::v1;

    let package = compile("package p.s\ntype Count : integer [0..1000]\n");
    let model = ridl_ir::codegen::lower(&package, &[]);
    let projection = model.podbuf.clone().expect("a projection");
    assert_eq!(projection.boxes.len(), 1, "{projection:?}");
    let base = crate::Header::new(&model, &projection).heads.len();
    assert_eq!(base, projection.heads.len() + 1);

    let mut no_head = projection.clone();
    no_head.boxes[0].head = None;
    assert_eq!(
        crate::Header::new(&model, &no_head).heads.len(),
        projection.heads.len()
    );
    let h = crate::generate_model(
        &package,
        &[],
        &v1::Model {
            podbuf: Some(no_head),
            ..model.clone()
        },
    )
    .expect("generate")
    .header_source;
    assert!(!h.contains("p_s_count"), "{h}");

    let mut unknown = projection.clone();
    unknown.boxes[0].declaration = 999;
    assert_eq!(
        crate::Header::new(&model, &unknown).heads.len(),
        projection.heads.len()
    );
    let h = crate::generate_model(
        &package,
        &[],
        &v1::Model {
            podbuf: Some(unknown),
            ..model
        },
    )
    .expect("generate")
    .header_source;
    assert!(!h.contains("p_s_count"), "{h}");
}
