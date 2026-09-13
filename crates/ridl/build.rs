//! Stamps the binary with its release version. The release workflow sets
//! `RIDL_BUILD_VERSION` to the `editor-v*` tag; a local build reports the
//! crate version, so a bug report can name the build it came from.

fn main() {
    println!("cargo:rerun-if-env-changed=RIDL_BUILD_VERSION");
    let version = std::env::var("RIDL_BUILD_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=RIDL_BUILD_VERSION={version}");
}
