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
