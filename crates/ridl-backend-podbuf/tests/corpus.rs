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
        let header = support::header(&packages, name);
        let pair = head_body(&header, "pod_parts_clash_pair_head");
        assert!(pair.contains("uint8_t f1;"), "{name}: {pair}");
        assert!(pair.contains("uint8_t f2;"), "{name}: {pair}");
        assert!(!pair.contains("min_speed"), "{name}: {pair}");
        let presence = head_body(&header, "pod_parts_clash_presence_head");
        assert!(presence.contains("uint8_t f1;"), "{name}: {presence}");
        assert!(presence.contains("uint8_t f2;"), "{name}: {presence}");
        assert!(!presence.contains("has_x"), "{name}: {presence}");
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
    assert!(parts.contains("pod_parts_clash_clear_left_at("), "{parts}");
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
