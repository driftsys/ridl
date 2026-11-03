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

#[test]
fn a_keyword_and_its_escaped_spelling_are_two_members() {
    let h = generate_fixture(
        "package p\n\
         struct S {\n  int : integer [0..255]\n  int_ : integer [0..255]\n\
         \x20 t : (int: integer [0..255], int_: integer [0..255])\n}\n",
    )
    .header_source;
    for spelling in [
        "uint8_t int_;",
        "uint8_t int__;",
        "p_s_int_at(",
        "p_s_int__at(",
        "p_s_int(",
        "p_s_int_(",
        "p_st_int_at(",
        "p_st_int__at(",
    ] {
        assert!(h.contains(spelling), "{spelling}: {h}");
    }
    assert!(!h.contains("uint8_t f1;"), "{h}");
}

#[test]
fn a_trailing_underscore_stays_in_the_accessor_names() {
    let h = generate_fixture(
        "package p\n\
         struct S {\n  low : integer [0..255]\n  low_ : integer [0..255]\n\
         \x20 top_ : integer [0..255]?\n}\n",
    )
    .header_source;
    for spelling in [
        "p_s_low_at(",
        "p_s_low__at(",
        "p_s_low(",
        "p_s_low_(",
        "p_s_has_top__at(",
    ] {
        assert!(h.contains(spelling), "{spelling}: {h}");
    }
}

#[test]
fn a_member_equal_to_a_presence_test_is_refused_in_a_struct_and_a_tuple() {
    for (source, name) in [
        (
            "package p\nstruct S {\n  x : integer [0..255]?\n  hasX : integer [0..255]\n}\n",
            "p_s_has_x_at",
        ),
        (
            "package p\nstruct S {\n  t : (x: integer [0..255]?, hasX: integer [0..255])\n}\n",
            "p_st_has_x_at",
        ),
        (
            "package p\nstruct S {\n  t : (int: integer [0..255]?, hasInt: integer [0..255])\n}\n",
            "p_st_has_int_at",
        ),
    ] {
        let err = generate_fixture_err(source);
        assert!(
            err.message.contains(&format!("`{name}`")),
            "{}",
            err.message
        );
        assert!(
            err.message.contains("the presence test of"),
            "{}",
            err.message
        );
        assert!(err.message.contains("the accessor of"), "{}", err.message);
    }
}

#[test]
fn a_tuple_member_equal_to_an_at_accessor_or_the_head_type_is_accepted() {
    for (source, spellings) in [
        (
            "package p\nstruct S {\n  t : (x: integer [0..255], xAt: integer [0..255])\n}\n",
            &["uint8_t x;", "uint8_t x_at;", "p_st_x_at(", "p_st_x_at_at("][..],
        ),
        (
            "package p\nstruct S {\n  t : (head: integer [0..255])\n}\n",
            &["uint8_t head;", "p_st_head_at("][..],
        ),
    ] {
        let generated = generate(&compile(source));
        assert!(generated.is_ok(), "{generated:?}");
        let h = generated.expect("generate").header_source;
        for spelling in spellings {
            assert!(h.contains(spelling), "{spelling}: {h}");
        }
    }
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
            && err.message.contains("the guard of the head of `Gear`"),
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
            && err.message.contains("the guard of the head of `GearBox`"),
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
        "the head of the arm `b` of `U`",
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
/// declaration, a tuple nested in it, the first and the second arm of a
/// foreign union, and the entry head of a foreign map.
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
        "the head of the arm `slick` of `pod.parts.Tyre`",
        "the head of the arm `flat` of `pod.parts.Tyre`",
        "the entry head of the map at `pod.parts.Table.byPair`",
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

/// A presence test whose `_at` function equals the accessor of another head
/// is refused, and the message names both claimants: `S`'s
/// `p_q_s_has_x_at` and the accessor of the field `xAt` of `SHas`.
#[test]
fn a_presence_test_equal_to_an_accessor_is_refused_naming_both() {
    let err = generate_fixture_err(
        "package p.q\n\
         struct S {\n  x : integer [0..255]?\n}\n\
         struct SHas {\n  xAt : integer [0..255]\n}\n",
    );
    assert!(
        err.message.contains("`p_q_s_has_x_at`")
            && err
                .message
                .contains("the presence test of `x` of the head of `S`")
            && err
                .message
                .contains("the accessor of `x_at` of the head of `SHas`"),
        "{}",
        err.message
    );
}

/// The fixed names of the preamble are claimed before any name the package
/// derives, so a package `ridl.podbuf` whose struct `Desc` has a field `at`
/// is refused instead of defining `ridl_podbuf_desc_at` a second time
/// (driftsys/ridl#1153).
#[test]
fn an_accessor_equal_to_a_preamble_helper_is_refused_naming_both() {
    let err =
        generate_fixture_err("package ridl.podbuf\nstruct Desc {\n  at : integer [0..10]\n}\n");
    assert!(
        err.message.contains("`ridl_podbuf_desc_at`")
            && err.message.contains("the preamble of every header")
            && err
                .message
                .contains("the accessor of `at` of the head of `Desc`"),
        "{}",
        err.message
    );
}

/// A constant equal to a preamble macro is refused the same way: the enum
/// `Preamble` of `ridl.podbuf` with a value `SIZE` would define
/// `RIDL_PODBUF_PREAMBLE_SIZE` again.
#[test]
fn a_constant_equal_to_a_preamble_macro_is_refused_naming_both() {
    let err = generate_fixture_err("package ridl.podbuf\nenum Preamble {\n  SIZE = 0\n}\n");
    assert!(
        err.message.contains("`RIDL_PODBUF_PREAMBLE_SIZE`")
            && err.message.contains("the preamble of every header")
            && err.message.contains("the enum value `SIZE`"),
        "{}",
        err.message
    );
}

#[test]
fn an_accessor_equal_to_a_stddef_type_is_refused_naming_both() {
    let err = generate_fixture_err("package max\nstruct Align {\n  t : integer [0..10]\n}\n");
    assert!(
        err.message.contains("`max_align_t`")
            && err.message.contains("claimed by `<stddef.h>` and by")
            && err
                .message
                .contains("the accessor of `t` of the head of `Align`"),
        "{}",
        err.message
    );
}

#[test]
fn a_constant_equal_to_a_stdint_limit_is_refused_naming_both() {
    let err = generate_fixture_err("package uint\nenum Least8 {\n  MAX = 0\n}\n");
    assert!(
        err.message.contains("`UINT_LEAST8_MAX`")
            && err.message.contains("claimed by `<stdint.h>` and by")
            && err.message.contains("the enum value `MAX`"),
        "{}",
        err.message
    );
}

/// Exercise the collision path for a C17 inventory written out in this test,
/// including names that the current source-to-C naming rules cannot derive.
/// The inventory is a second copy of `STANDARD_HEADER_NAMES`, not an
/// independent specification: the test fails when either copy holds a name the
/// other lacks, so a change to one has to be made to both on purpose.
#[test]
fn every_c17_standard_header_name_is_claimed() {
    let expected = "
        int8_t int16_t int32_t int64_t
        uint8_t uint16_t uint32_t uint64_t
        int_least8_t int_least16_t int_least32_t int_least64_t
        uint_least8_t uint_least16_t uint_least32_t uint_least64_t
        int_fast8_t int_fast16_t int_fast32_t int_fast64_t
        uint_fast8_t uint_fast16_t uint_fast32_t uint_fast64_t
        intptr_t uintptr_t intmax_t uintmax_t
        INT8_MIN INT16_MIN INT32_MIN INT64_MIN
        INT8_MAX INT16_MAX INT32_MAX INT64_MAX
        UINT8_MAX UINT16_MAX UINT32_MAX UINT64_MAX
        INT_LEAST8_MIN INT_LEAST16_MIN INT_LEAST32_MIN INT_LEAST64_MIN
        INT_LEAST8_MAX INT_LEAST16_MAX INT_LEAST32_MAX INT_LEAST64_MAX
        UINT_LEAST8_MAX UINT_LEAST16_MAX UINT_LEAST32_MAX UINT_LEAST64_MAX
        INT_FAST8_MIN INT_FAST16_MIN INT_FAST32_MIN INT_FAST64_MIN
        INT_FAST8_MAX INT_FAST16_MAX INT_FAST32_MAX INT_FAST64_MAX
        UINT_FAST8_MAX UINT_FAST16_MAX UINT_FAST32_MAX UINT_FAST64_MAX
        INTPTR_MIN INTPTR_MAX UINTPTR_MAX INTMAX_MIN INTMAX_MAX UINTMAX_MAX
        PTRDIFF_MIN PTRDIFF_MAX SIG_ATOMIC_MIN SIG_ATOMIC_MAX SIZE_MAX
        WCHAR_MIN WCHAR_MAX WINT_MIN WINT_MAX
        INT8_C INT16_C INT32_C INT64_C INTMAX_C
        UINT8_C UINT16_C UINT32_C UINT64_C UINTMAX_C
        size_t ptrdiff_t wchar_t max_align_t NULL offsetof
    ";
    let package = compile("package inventory\nstruct S {\n  x : integer [0..10]\n}\n");
    let model = ridl_ir::codegen::lower(&package, &[]);
    let projection = model.podbuf.as_ref().expect("a podbuf projection");
    let (_, mut claims) = crate::Header::new(&model, projection)
        .render_with_claims()
        .expect("generate");
    let mut table: Vec<&str> = crate::STANDARD_HEADER_NAMES.to_vec();
    table.sort_unstable();
    let mut inventory: Vec<&str> = expected.split_ascii_whitespace().collect();
    inventory.sort_unstable();
    assert_eq!(table, inventory, "the table and the inventory differ");
    let stddef = [
        "size_t",
        "ptrdiff_t",
        "wchar_t",
        "max_align_t",
        "NULL",
        "offsetof",
    ];
    for name in expected.split_ascii_whitespace() {
        let header = if stddef.contains(&name) {
            "`<stddef.h>`"
        } else {
            "`<stdint.h>`"
        };
        let err = claims
            .claim(
                name,
                "the test declaration".to_string(),
                crate::Identity {
                    package: "inventory".to_string(),
                    key: name.to_string(),
                    role: "test declaration".to_string(),
                },
            )
            .expect_err(name);
        assert!(
            err.message.contains(&format!("`{name}`"))
                && err.message.contains(&format!("claimed by {header} and by"))
                && err.message.contains("the test declaration"),
            "{}",
            err.message
        );
    }
}

#[test]
fn standard_header_type_families_are_refused_through_generation() {
    for (source, name) in [
        (
            "package int\nstruct Fast8 {\n  t : integer [0..10]\n}\n",
            "int_fast8_t",
        ),
        (
            "package uint\nstruct Fast16 {\n  t : integer [0..10]\n}\n",
            "uint_fast16_t",
        ),
        (
            "package int\nstruct Least32 {\n  t : integer [0..10]\n}\n",
            "int_least32_t",
        ),
        (
            "package uint\nstruct Least64 {\n  t : integer [0..10]\n}\n",
            "uint_least64_t",
        ),
    ] {
        let err = generate_fixture_err(source);
        assert!(
            err.message.contains(&format!("`{name}`"))
                && err.message.contains("claimed by `<stdint.h>` and by")
                && err.message.contains("the accessor of `t`"),
            "{}",
            err.message
        );
    }
}

#[test]
fn standard_header_limit_families_are_refused_through_generation() {
    for (source, name) in [
        ("package int\nenum Fast8 {\n  MIN = 0\n}\n", "INT_FAST8_MIN"),
        (
            "package int\nenum Fast16 {\n  MAX = 0\n}\n",
            "INT_FAST16_MAX",
        ),
        (
            "package uint\nenum Fast16 {\n  MAX = 0\n}\n",
            "UINT_FAST16_MAX",
        ),
        (
            "package int\nenum Least32 {\n  MIN = 0\n}\n",
            "INT_LEAST32_MIN",
        ),
        (
            "package int\nenum Least64 {\n  MAX = 0\n}\n",
            "INT_LEAST64_MAX",
        ),
        (
            "package sig\nenum Atomic {\n  MIN = 0\n}\n",
            "SIG_ATOMIC_MIN",
        ),
        (
            "package sig\nenum Atomic {\n  MAX = 0\n}\n",
            "SIG_ATOMIC_MAX",
        ),
    ] {
        let err = generate_fixture_err(source);
        assert!(
            err.message.contains(&format!("`{name}`"))
                && err.message.contains("claimed by `<stdint.h>` and by")
                && err.message.contains("the enum value"),
            "{}",
            err.message
        );
    }
}

#[test]
fn names_near_standard_header_names_are_accepted() {
    let h = generate_fixture("package max\nstruct Align {\n  time : integer [0..10]\n}\n")
        .header_source;
    assert!(h.contains("max_align_time("), "{h}");
    let h = generate_fixture("package uint\nenum Least8 {\n  MAXIMUM = 0\n}\n").header_source;
    assert!(h.contains("#define UINT_LEAST8_MAXIMUM 0"), "{h}");
    let h = generate_fixture("package uint\nenum Custom {\n  MAX = 0\n}\n").header_source;
    assert!(h.contains("#define UINT_CUSTOM_MAX 0"), "{h}");
    // A lowercase name of a reserved pattern (`uint*_t`) is not claimed.
    let h =
        generate_fixture("package uint\nstruct Custom {\n  t : integer [0..10]\n}\n").header_source;
    assert!(h.contains("uint_custom_t("), "{h}");
}

/// The include guard of the header is claimed too: the enum `RidlPodbuf` of
/// `ridl.podbuf` with a value `H` spells `RIDL_PODBUF_RIDL_PODBUF_H`, the
/// guard of its own header, and the `#define` would end the header early.
#[test]
fn a_constant_equal_to_the_include_guard_is_refused_naming_both() {
    let err = generate_fixture_err("package ridl.podbuf\nenum RidlPodbuf {\n  H = 0\n}\n");
    assert!(
        err.message.contains("`RIDL_PODBUF_RIDL_PODBUF_H`")
            && err.message.contains("the include guard of the header")
            && err.message.contains("the enum value `H`"),
        "{}",
        err.message
    );
}

/// `PREAMBLE_NAMES` is every identifier the preamble defines under the
/// `ridl_podbuf_` and `RIDL_PODBUF_` prefixes, no more and no fewer, so a
/// helper added to the preamble is claimed too.
#[test]
fn the_claimed_preamble_names_are_the_names_the_preamble_defines() {
    let mut defined: Vec<&str> = crate::PREAMBLE
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with("ridl_podbuf_") || word.starts_with("RIDL_PODBUF_"))
        .collect();
    defined.sort_unstable();
    defined.dedup();
    let mut claimed = crate::PREAMBLE_NAMES.to_vec();
    claimed.sort_unstable();
    assert_eq!(defined, claimed);
}

/// `Header::identity` keeps the three parts the build-wide check compares:
/// two heads of one package differ by their key, and one head's type name and
/// guard differ by their role, under one package.
#[test]
fn the_identity_of_a_head_carries_its_package_its_key_and_its_role() {
    let package = compile(
        "package p.i\n\
         struct S {\n  x : integer [0..10]\n}\n\
         struct T {\n  y : integer [0..10]\n}\n",
    );
    let model = ridl_ir::codegen::lower(&package, &[]);
    let projection = model.podbuf.clone().expect("a projection");
    let header = crate::Header::new(&model, &projection);
    assert_eq!(header.heads.len(), 2, "{:?}", header.type_names);
    let (s, t) = (header.identity(0, "head"), header.identity(1, "head"));
    assert_eq!(s.package, "p.i");
    assert_eq!(s.package, t.package);
    assert_ne!(s.key, t.key);
    assert_eq!(s.role, "head");
    let guard = header.identity(0, "guard");
    assert_eq!((&guard.package, &guard.key), (&s.package, &s.key));
    assert_ne!(guard, s);
}

/// Two claims of one identity stand for one entity only when their
/// definitions are equal: a head defined again with the same text is one
/// entity, a head defined again with another text is not.
#[test]
fn one_identity_with_two_definitions_is_not_one_entity() {
    let identity = crate::Identity {
        package: "a".to_string(),
        key: "ABXY".to_string(),
        role: "head".to_string(),
    };
    let claim = |definition: &str| crate::Claim {
        what: "a head".to_string(),
        identity: identity.clone(),
        definition: Some(definition.to_string()),
    };
    assert!(claim("uint8_t u;").same_entity(&claim("uint8_t u;")));
    assert!(!claim("uint8_t u;").same_entity(&claim("uint8_t w;")));
}

/// The build-wide check compares the definitions of a head and of its guard:
/// two headers that claim one name for one identity with two texts are
/// refused, and with one text are not. No workspace holds two packages of
/// one name, so the two views of `a.Inner` are two compiled packages named
/// `a`, one with one member and one with two, handed to the check together.
#[test]
fn one_identity_with_two_definitions_is_refused_by_the_check() {
    let one = compile("package a\nstruct Inner {\n  x : integer [0..10]\n}\n");
    let two =
        compile("package a\nstruct Inner {\n  x : integer [0..10]\n  y : integer [0..10]\n}\n");
    assert_eq!(crate::check_c_names(&[&one, &one]), Ok(()));
    let errors = crate::check_c_names(&[&one, &two]).expect_err("two texts are refused");
    let names: Vec<&str> = errors
        .iter()
        .filter_map(|error| error.message.strip_prefix("the C name `"))
        .map(|rest| rest.split('`').next().unwrap())
        .collect();
    assert_eq!(names, vec!["A_INNER_HEAD_DEFINED", "a_inner_head"]);
}

/// `check_c_names` leaves out a package whose projection refuses a head: a
/// struct whose field names a type that does not resolve, which checked
/// source does not reach, so the IR is edited after the compile. The package
/// also declares `CommonSpeed`, which would clash with `veh.common.Speed`;
/// with the package left out, the check passes.
#[test]
fn a_package_whose_projection_refuses_a_head_is_left_out_of_the_check() {
    let mut veh = compile(
        "package veh\ntype CommonSpeed : integer [0..100]\n\
         struct Bad {\n  s : integer [0..10]\n}\n",
    );
    let bad = veh
        .decls
        .iter_mut()
        .find(|decl| decl.name == "Bad")
        .expect("Bad");
    let Some(v2::decl::Kind::StructDef(def)) = bad.kind.as_mut() else {
        panic!("a struct");
    };
    let Some(v2::struct_member::Member::Field(field)) = def.members[0].member.as_mut() else {
        panic!("a field");
    };
    field.r#type.as_mut().expect("a type").kind =
        Some(v2::field_type::Kind::Named("Nowhere".to_string()));
    let common = compile("package veh.common\nstruct Speed {\n  v : integer [0..1000]\n}\n");
    let packages = [&veh, &common];
    let model = ridl_ir::codegen::lower(&veh, &packages);
    assert!(
        !model
            .podbuf
            .as_ref()
            .expect("a projection")
            .refused
            .is_empty(),
        "the edited struct is refused by the projection"
    );
    assert_eq!(crate::check_c_names(&packages), Ok(()));
}

/// Every header claims the preamble names and the names `<stdint.h>` and
/// `<stddef.h>` define, and these claims belong to no package, so two
/// unrelated packages, each with its own header, do not clash on them.
#[test]
fn two_unrelated_packages_do_not_clash_on_the_fixed_names() {
    let a = compile("package veh.a\nstruct Speed {\n  v : integer [0..1000]\n}\n");
    let b = compile("package veh.b\nstruct Load {\n  w : integer [0..1000]\n}\n");
    assert_eq!(crate::check_c_names(&[&a, &b]), Ok(()));
}

/// A strict enum, an open enum, a strict enum set and an open enum set.
const STRICTNESS: &str = "package veh.common\n\
enum Gear {\n  PARK = 0\n  DRIVE = 1\n}\n\
open enum DtcStatus {\n  INACTIVE = 0\n  ACTIVE = 1\n}\n\
enumset Warnings {\n  LOW_FUEL = 0\n}\n\
open enumset Lamps {\n  LOW_BEAM = 0\n}\n";

/// The constants of each enum and enum set follow a comment that states
/// whether a value outside them is valid: the rule the reader applies, not
/// how it is enforced, as the comment of the `.proto` and `.fbs` output does.
#[test]
fn the_header_states_whether_an_enum_or_an_enum_set_is_strict_or_open() {
    let h = generate_fixture(STRICTNESS).header_source;
    for expected in [
        "/* strict: a value outside these constants is not valid */\n\
         #define VEH_COMMON_GEAR_PARK 0\n\
         #define VEH_COMMON_GEAR_DRIVE 1\n",
        "/* open: a value outside these constants is valid and is kept */\n\
         #define VEH_COMMON_DTC_STATUS_INACTIVE 0\n\
         #define VEH_COMMON_DTC_STATUS_ACTIVE 1\n",
        "/* strict: a bit outside these constants is not valid */\n\
         #define VEH_COMMON_WARNINGS_LOW_FUEL (UINT64_C(1) << 0)\n",
        "/* open: a bit outside these constants is valid and is kept */\n\
         #define VEH_COMMON_LAMPS_LOW_BEAM (UINT64_C(1) << 0)\n",
    ] {
        assert!(h.contains(expected), "{expected}\nnot in:\n{h}");
    }
}

/// An open enum with no value and an open enum set with no bit write no
/// constant, so the header writes no comment for them either: a comment would
/// sit above nothing. `Retired` also pins that a `reserved` entry is not a
/// value and emits no constant.
#[test]
fn a_declaration_with_no_constant_has_no_strictness_comment() {
    let h = generate_fixture(
        "package veh.common\n\
         open enum Pending {}\n\
         open enum Retired {\n  reserved 3\n}\n\
         open enumset Nothing {}\n",
    )
    .header_source;
    assert!(!h.contains("these constants"), "{h}");
    // The one `#define` that names `Retired` is the guard of its box head.
    let constants: Vec<&str> = h
        .lines()
        .filter(|line| line.starts_with("#define VEH_COMMON_RETIRED_"))
        .filter(|line| !line.ends_with("_HEAD_DEFINED"))
        .collect();
    assert!(constants.is_empty(), "{constants:?}");
}
