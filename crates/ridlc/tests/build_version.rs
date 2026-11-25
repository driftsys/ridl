#[path = "../build_support/version.rs"]
mod ridlc_version;

#[test]
fn release_stamp_normalises_binary_tags() {
    let normalise = ridlc_version::normalise_version;
    for value in ["0.23.0", "v0.23.0", "editor-v0.23.0", "  editor-v0.23.0 "] {
        assert_eq!(normalise(value), "0.23.0", "{value}");
    }
    for blank in ["", " \n\t "] {
        assert_eq!(normalise(blank), env!("CARGO_PKG_VERSION"));
    }
    assert_eq!(normalise("editor-veditor-v0.23.0"), "editor-v0.23.0");
    assert_eq!(normalise("v9.9.9-test"), "9.9.9-test");
}
