//! The acceptance check: the `veh-cluster` workspace emits C headers whose
//! text is pinned, so a later change has to be looked at.

mod support;

#[test]
fn the_veh_cluster_headers_are_pinned() {
    let packages = support::corpus("veh-cluster");
    insta::assert_snapshot!("veh_cluster", support::header(&packages, "veh.cluster"));
    insta::assert_snapshot!("veh_common", support::header(&packages, "veh.common"));
}

/// A package that lays out heads of another package inline defines those
/// heads itself, behind the guard macro the other package's header defines
/// too.
#[test]
fn the_two_package_headers_are_pinned() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    insta::assert_snapshot!("pod_parts", support::header(&packages, "pod.parts"));
    insta::assert_snapshot!("pod_vehicle", support::header(&packages, "pod.vehicle"));
}

/// A tuple member is named by its typl name in the header of the package that
/// declares the struct and in the header of the package that lays it out
/// inline, so the two definitions of the tuple's head are the same text.
#[test]
fn a_foreign_nested_tuple_keeps_its_member_names() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    for name in ["pod.parts", "pod.vehicle"] {
        let header = support::header(&packages, name);
        let start = header
            .find("typedef struct pod_parts_hub_size_head {")
            .unwrap_or_else(|| panic!("{name}: no hub size head"));
        let end = start + header[start..].find("} pod_parts_hub_size_head;").unwrap();
        let body = &header[start..end];
        assert!(
            body.contains("uint8_t width;"),
            "{name}: no width in {body}"
        );
        assert!(
            body.contains("uint8_t height;"),
            "{name}: no height in {body}"
        );
        assert!(!body.contains("f1"), "{name}: positional name in {body}");
        assert!(
            header.contains("offsetof(pod_parts_hub_size_head, height) == 1"),
            "{name}: the layout check names the member"
        );
    }
}

/// A tuple member is spelled as a struct field is: the pinned snake case, with
/// a trailing underscore when C reserves the word, in both headers.
#[test]
fn a_foreign_nested_tuple_member_is_spelled_as_a_c_member() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    for name in ["pod.parts", "pod.vehicle"] {
        let header = support::header(&packages, name);
        let body = head_body(&header, "pod_parts_gauge_span_head");
        assert!(body.contains("uint8_t low_mark;"), "{name}: {body}");
        assert!(body.contains("uint8_t int_;"), "{name}: {body}");
        assert!(
            header.contains("offsetof(pod_parts_gauge_span_head, int_) == 1"),
            "{name}: the layout check spells the member"
        );
    }
    let parts = support::header(&packages, "pod.parts");
    assert!(
        parts.contains("pod_parts_gauge_span_low_mark_at("),
        "{parts}"
    );
    assert!(parts.contains("pod_parts_gauge_span_int_at("), "{parts}");
}

/// A tuple whose spelled member names would clash — two members equal under
/// snake case, or a member equal to the presence test of an optional member —
/// falls back to positional names in both headers, and the build succeeds. A
/// tuple next to it whose names do not clash keeps them.
#[test]
fn a_tuple_whose_spelled_names_clash_falls_back_to_positions() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    for name in ["pod.parts", "pod.vehicle"] {
        let package = packages.iter().find(|p| p.name == name).unwrap();
        let others: Vec<&ridl_ir::v2::Package> = packages.iter().collect();
        let built = ridl_backend_podbuf::generate_with(package, &others);
        assert!(
            built.is_ok(),
            "{name}: the build refused: {}",
            built
                .as_ref()
                .err()
                .map(|e| e.message.as_str())
                .unwrap_or("")
        );
        let header = built.unwrap().header_source;
        let pair = head_body(&header, "pod_parts_clash_pair_head");
        assert!(pair.contains("uint8_t f1;"), "{name}: {pair}");
        assert!(pair.contains("uint8_t f2;"), "{name}: {pair}");
        assert!(!pair.contains("min_speed"), "{name}: {pair}");
        let presence = head_body(&header, "pod_parts_clash_presence_head");
        assert!(presence.contains("uint8_t f1;"), "{name}: {presence}");
        assert!(presence.contains("uint8_t f2;"), "{name}: {presence}");
        assert!(!presence.contains("has_x"), "{name}: {presence}");
        // The reserved word `int` is spelled `int_`, and its presence test
        // is `has_int`, so the stems are compared without the underscore.
        let trim = head_body(&header, "pod_parts_clash_trim_head");
        assert!(trim.contains("uint8_t f1;"), "{name}: {trim}");
        assert!(trim.contains("uint8_t f2;"), "{name}: {trim}");
        assert!(!trim.contains("int_"), "{name}: {trim}");
        // A required `x` has no presence test, so `hasX` next to it is no
        // clash.
        let plain = head_body(&header, "pod_parts_clash_plain_head");
        assert!(plain.contains("uint8_t x;"), "{name}: {plain}");
        assert!(plain.contains("uint8_t has_x;"), "{name}: {plain}");
        assert!(!plain.contains("f1"), "{name}: {plain}");
        let clear = head_body(&header, "pod_parts_clash_clear_head");
        assert!(clear.contains("uint8_t left;"), "{name}: {clear}");
        assert!(clear.contains("uint8_t right;"), "{name}: {clear}");
        assert!(!clear.contains("f1"), "{name}: {clear}");
    }
    let parts = support::header(&packages, "pod.parts");
    assert!(parts.contains("pod_parts_clash_pair_f1_at("), "{parts}");
    assert!(
        parts.contains("pod_parts_clash_presence_has_f1_at("),
        "{parts}"
    );
    assert!(parts.contains("pod_parts_clash_presence_f2_at("), "{parts}");
    assert!(parts.contains("pod_parts_clash_trim_has_f1_at("), "{parts}");
    assert!(parts.contains("pod_parts_clash_plain_x_at("), "{parts}");
    assert!(parts.contains("pod_parts_clash_plain_has_x_at("), "{parts}");
    assert!(parts.contains("pod_parts_clash_clear_left_at("), "{parts}");
}

/// Every head the two headers both define is defined by the same text: the
/// guarded `typedef` and the `_Static_assert` lines after it, byte for byte.
/// The nested tuple heads are the ones the two headers once spelled apart
/// (driftsys/ridl#1112): a tuple at a tuple position, a tuple as the element
/// of a fixed-count array, the two same-shape tuples of `Pair`, and the two
/// positions `A.bC` and `AB.c` that share one entry. The element head of a
/// variable-count array is a tail head, which the importing header does not
/// define.
#[test]
fn a_head_both_headers_define_has_one_text() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    let parts = guarded_blocks(&support::header(&packages, "pod.parts"));
    let vehicle = guarded_blocks(&support::header(&packages, "pod.vehicle"));
    for guard in [
        "POD_PARTS_HUB_SIZE_FIELD3_HEAD_DEFINED",
        "POD_PARTS_HUB_SIZE_HEAD_DEFINED",
        "POD_PARTS_HUB_GRID_ELEMENT_HEAD_DEFINED",
        "POD_PARTS_PAIR_A_HEAD_DEFINED",
        "POD_PARTS_PAIR_B_HEAD_DEFINED",
        "POD_PARTS_PAIR_HEAD_DEFINED",
        "POD_PARTS_ABC_HEAD_DEFINED",
        "POD_PARTS_A_HEAD_DEFINED",
        "POD_PARTS_AB_HEAD_DEFINED",
    ] {
        assert!(parts.contains_key(guard), "pod.parts has no {guard}");
        assert!(vehicle.contains_key(guard), "pod.vehicle has no {guard}");
    }
    // A head reached only through a tail descriptor is not laid out inline,
    // so the importing header does not define it.
    let element = "POD_PARTS_HUB_ITEMS_ELEMENT_HEAD_DEFINED";
    assert!(parts.contains_key(element), "pod.parts has no {element}");
    assert!(
        !vehicle.contains_key(element),
        "pod.vehicle defines {element}"
    );
    let mut shared = 0;
    for (guard, text) in &parts {
        let Some(other) = vehicle.get(guard) else {
            continue;
        };
        shared += 1;
        assert_eq!(text, other, "{guard} differs between the two headers");
    }
    assert!(shared >= 9, "{shared} shared guards");
}

/// Two positions whose induced Rust names are one share one entry in the
/// declaring package (`A.bC` and `AB.c`, both `ABC`), so both headers type
/// the two members with `pod_parts_abc_head`, the head of the position
/// reached first, and neither spells a second head.
#[test]
fn two_positions_that_share_one_entry_share_one_head_in_both_headers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    for name in ["pod.parts", "pod.vehicle"] {
        let header = support::header(&packages, name);
        let a = head_body(&header, "pod_parts_a_head");
        assert!(a.contains("pod_parts_abc_head b_c;"), "{name}: {a}");
        let ab = head_body(&header, "pod_parts_ab_head");
        assert!(ab.contains("pod_parts_abc_head c;"), "{name}: {ab}");
        assert_eq!(
            header
                .matches("typedef struct pod_parts_abc_head {")
                .count(),
            1,
            "{name}: one definition of the shared head"
        );
        // The nested pair `N.bC.1` and `NB.cD.2`, which `pod.vehicle` reaches
        // in the reverse order: the shared head keeps the declaring package's
        // wire name, and no head is named by the second position.
        let nbc = head_body(&header, "pod_parts_nbc_head");
        assert!(
            nbc.contains("pod_parts_nbc_field1_head d_p;"),
            "{name}: {nbc}"
        );
        let nbcd = head_body(&header, "pod_parts_nbcd_head");
        assert!(
            nbcd.contains("pod_parts_nbc_field1_head p;"),
            "{name}: {nbcd}"
        );
        assert!(!header.contains("pod_parts_nbcd_field2_head"), "{name}");
    }
}

/// Two tuples of one package that induce one wire name are refused by the
/// declaring header, through its C name claim, whatever their layouts.
#[test]
fn the_declaring_header_refuses_two_tuples_under_one_wire_name() {
    for (shape, source) in [
        (
            "two layouts",
            "package w.a\n\nstruct Hub {\n  \
             size       : (w: integer [0..255], inner: (p: integer [0..255], q: integer [0..255]))\n  \
             sizeField2 : (a: integer [0..255], b: integer [0..255], c: integer [0..255])\n}\n",
        ),
        (
            "one layout",
            "package w.a\n\nstruct Hub {\n  \
             sizeField2 : (a: integer [0..255], b: integer [0..255])\n  \
             size       : (x: integer [0..255], inner: (a: integer [0..255], b: integer [0..255]))\n}\n",
        ),
    ] {
        let packages = workspace_from(&[("a", "w.a", source)]);
        let package = packages.iter().find(|p| p.name == "w.a").unwrap();
        let others: Vec<&ridl_ir::v2::Package> = packages.iter().collect();
        let error = ridl_backend_podbuf::generate_with(package, &others)
            .err()
            .unwrap_or_else(|| panic!("{shape}: the declaring header was not refused"));
        assert!(
            error
                .message
                .contains("`w_a_hub_size_field2_head` is claimed by"),
            "{shape}: {}",
            error.message
        );
    }
}

/// Two positions of one declaration whose induced names are one
/// (`S.aInner` and `S.a.inner`, both `SAInner`) share one entry, so the
/// declaring header and a header that imports `S` define one head for both
/// by the same text, and neither header is refused: the accessors of the
/// two positions are distinct.
#[test]
fn two_positions_of_one_declaration_that_share_one_entry_have_one_text() {
    let packages = workspace_from(&[
        (
            "a",
            "w.a",
            "package w.a\n\nstruct S {\n  \
             aInner : (p: integer [0..255], q: integer [0..255])\n  \
             a      : (inner: (p: integer [0..255], q: integer [0..255]))\n}\n",
        ),
        (
            "b",
            "w.b",
            "package w.b\n\nimport w.a.S\n\nstruct Local {\n  s : S\n}\n",
        ),
    ]);
    let declaring = support::header(&packages, "w.a");
    for accessor in ["w_a_s_a_inner_at(", "w_a_s_a_at(", "w_a_sa_inner_at("] {
        assert!(declaring.contains(accessor), "no {accessor} in {declaring}");
    }
    let s = head_body(&declaring, "w_a_s_head");
    assert!(s.contains("w_a_sa_inner_head a_inner;"), "{s}");
    let a = head_body(&declaring, "w_a_sa_head");
    assert!(a.contains("w_a_sa_inner_head inner;"), "{a}");
    let declaring = guarded_blocks(&declaring);
    let importing = guarded_blocks(&support::header(&packages, "w.b"));
    for guard in [
        "W_A_SA_INNER_HEAD_DEFINED",
        "W_A_SA_HEAD_DEFINED",
        "W_A_S_HEAD_DEFINED",
    ] {
        let text = declaring
            .get(guard)
            .unwrap_or_else(|| panic!("w.a has no {guard}"));
        let other = importing
            .get(guard)
            .unwrap_or_else(|| panic!("w.b has no {guard}"));
        assert_eq!(text, other, "{guard} differs between the two headers");
    }
}

/// The packages of a workspace written from `members` (directory, package
/// name, source), followed by the standard package.
fn workspace_from(members: &[(&str, &str, &str)]) -> Vec<ridl_ir::v2::Package> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let list = members
        .iter()
        .map(|(d, _, _)| format!("\"{d}\""))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        dir.path().join("ridl.toml"),
        format!("[workspace]\nmembers = [{list}]\n"),
    )
    .expect("write");
    for (d, name, text) in members {
        let member = dir.path().join(d);
        std::fs::create_dir_all(&member).expect("mkdir");
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .expect("write");
        std::fs::write(member.join(format!("{d}.typl")), text).expect("write");
    }
    support::workspace(dir.path())
}

/// A tuple head names the tuple's own head, not a head of the same shape:
/// `Pair` has two members with two heads in both headers, and no head of
/// `pod.parts` that `pod.vehicle` defines names a head of `pod.vehicle`.
#[test]
fn an_inline_tuple_names_its_own_head() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    for name in ["pod.parts", "pod.vehicle"] {
        let header = support::header(&packages, name);
        let pair = head_body(&header, "pod_parts_pair_head");
        assert!(pair.contains("pod_parts_pair_a_head a;"), "{name}: {pair}");
        assert!(pair.contains("pod_parts_pair_b_head b;"), "{name}: {pair}");
        let size = head_body(&header, "pod_parts_hub_size_head");
        assert!(
            size.contains("pod_parts_hub_size_field3_head inner;"),
            "{name}: {size}"
        );
    }
    let vehicle = support::header(&packages, "pod.vehicle");
    for (start, _) in vehicle.match_indices("typedef struct pod_parts_") {
        let end = start + vehicle[start..].find("\n}").unwrap();
        let body = &vehicle[start..end];
        assert!(
            !body.contains("pod_vehicle_"),
            "a head of pod.parts names a head of pod.vehicle: {body}"
        );
    }
    let own = head_body(&vehicle, "pod_vehicle_axle_head");
    assert!(own.contains("pod_vehicle_axle_own_head own;"), "{own}");
}

/// Each `#ifndef <GUARD>_HEAD_DEFINED` paragraph of `header`, by guard: the
/// guarded `typedef` and the `_Static_assert` lines that follow it, up to the
/// blank line.
fn guarded_blocks(header: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for (start, _) in header.match_indices("#ifndef ") {
        let line_end = header[start..].find('\n').unwrap();
        let guard = &header[start + "#ifndef ".len()..start + line_end];
        if !guard.ends_with("_HEAD_DEFINED") {
            continue;
        }
        let end = header[start..]
            .find("\n\n")
            .map_or(header.len(), |at| start + at);
        out.insert(guard.to_string(), header[start..end].to_string());
    }
    out
}

/// The text of the `typedef struct <name> { ... }` body in `header`.
fn head_body<'h>(header: &'h str, name: &str) -> &'h str {
    let open = format!("typedef struct {name} {{");
    let start = header
        .find(&open)
        .unwrap_or_else(|| panic!("no head {name} in {header}"));
    let end = start + header[start..].find(&format!("}} {name};")).unwrap();
    &header[start..end]
}
