//! Stamps the binary with the normalized release version, or the local crate version.
#[path = "build_support/version.rs"]
mod version;
fn main() {
    println!(
        "cargo:rustc-env=RIDL_BUILD_TARGET={}",
        std::env::var("TARGET").expect("Cargo target")
    );
    println!("cargo:rerun-if-env-changed=RIDL_BUILD_VERSION");
    let value = std::env::var("RIDL_BUILD_VERSION").unwrap_or_default();
    let version = version::normalise_version(&value);
    println!("cargo:rustc-env=RIDL_BUILD_VERSION={version}");
}
