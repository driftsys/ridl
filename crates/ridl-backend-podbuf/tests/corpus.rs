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
/// (driftsys/ridl#1112): a tuple at a tuple position, a tuple as an array
/// element, and the two same-shape tuples of `Pair`.
#[test]
fn a_head_both_headers_define_has_one_text() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    let packages = support::workspace(&path);
    let parts = guarded_blocks(&support::header(&packages, "pod.parts"));
    let vehicle = guarded_blocks(&support::header(&packages, "pod.vehicle"));
    for guard in [
        "POD_PARTS_HUB_SIZE_FIELD3_HEAD_DEFINED",
        "POD_PARTS_HUB_SIZE_HEAD_DEFINED",
        "POD_PARTS_PAIR_A_HEAD_DEFINED",
        "POD_PARTS_PAIR_B_HEAD_DEFINED",
        "POD_PARTS_PAIR_HEAD_DEFINED",
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
    assert!(shared >= 5, "{shared} shared guards");
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
