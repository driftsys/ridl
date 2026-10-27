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
