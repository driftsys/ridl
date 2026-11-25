//! Stamps the binary with the normalized release version, or the local crate version.
#[path = "build_support/version.rs"]
mod version;
fn main() {
    println!(
        "cargo:rustc-env=RIDL_BUILD_TARGET={}",
        std::env::var("TARGET").expect("Cargo target")
    );
    println!("cargo:rerun-if-env-changed=RIDL_BUILD_VERSION");
    let value = std::env::var("RIDL_BUILD_VERSION").ok();
    println!(
        "cargo:rustc-env=RIDL_BUILD_VERSION_INPUT={}",
        value.as_deref().unwrap_or_default().trim()
    );
    let version = version::build_version(value.as_deref());
    println!("cargo:rustc-env=RIDL_BUILD_VERSION={version}");
}
