// Records the compiler version at build time, so that a binary run on a
// device without a compiler can still state the toolchain that built it.
fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=BENCH_RUSTC_VERSION={version}");
    println!("cargo:rerun-if-changed=build.rs");
}
