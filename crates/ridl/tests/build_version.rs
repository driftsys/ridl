#[path = "../build_support/version.rs"]
mod ridl_version;

#[test]
fn release_stamp_normalises_binary_tags() {
    let normalise = ridl_version::normalise_version;
    for value in ["0.23.0", "v0.23.0", "editor-v0.23.0", "  editor-v0.23.0 "] {
        assert_eq!(normalise(value), "0.23.0", "{value}");
    }
    for blank in ["", " \n\t "] {
        assert_eq!(normalise(blank), env!("CARGO_PKG_VERSION"));
    }
    assert_eq!(normalise("editor-veditor-v0.23.0"), "editor-v0.23.0");
    assert_eq!(normalise("v9.9.9-test"), "9.9.9-test");
}

#[test]
fn compiled_build_stamp_is_the_normalised_plain_release() {
    let stamp = env!("RIDL_BUILD_VERSION");
    let release = ridl_core::release::parse_release(stamp);
    assert!(
        release.is_some(),
        "compiled build stamp must be a plain release: {stamp:?}"
    );
    let input = env!("RIDL_BUILD_VERSION_INPUT");
    assert_eq!(stamp, ridl_version::normalise_version(input));
}
