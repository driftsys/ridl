//! Workspace automation, invoked as `cargo xtask <task>` (the alias lives
//! in `.cargo/config.toml`).
//!
//! Available tasks:
//!
//! - `codegen` regenerates the typed AST from
//!   `crates/ridl-syntax/family.ungram` (ADR-0007 decision 1).
//! - `descriptor-codegen` regenerates the catalog-descriptor accessors from
//!   `crates/ridl-descriptor/schema/catalog.fbs`.
//!
//! - `calibrate` dumps corpus findings and derives levels from reviewed labels.
//! - `freeze update|check` writes or checks the dumps of the frozen surfaces
//!   under `freeze/`.
//!
//! The code generation drift tests fail whenever the committed output is
//! stale, so a generated file can never silently diverge from its source.

mod calibrate;
mod codegen;
mod descriptor;
mod freeze;

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("codegen") => {
            let path = codegen::write_generated();
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Some("descriptor-codegen") => {
            let path = descriptor::write_generated();
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Some("calibrate") => match calibrate::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("calibrate: {error}");
                ExitCode::from(2)
            }
        },
        Some("freeze") => match freeze::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("freeze: {error}");
                ExitCode::from(1)
            }
        },
        _ => {
            eprintln!("usage: cargo xtask <codegen|descriptor-codegen|calibrate|freeze>");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The `ridl-rt` features that are not payload encodings.
    const NON_ENCODING_FEATURES: [&str; 1] = ["std"];

    /// `render_cargo_toml` writes each manifest encoding name as a feature of
    /// `ridl-rt`, so the encoding names and the encoding features of
    /// `ridl-rt` are the same set, apart from the listed exceptions. The test
    /// lives here because it reads a sibling crate's manifest, which a
    /// packaged crate does not have.
    #[test]
    fn ridl_rt_encoding_features_are_the_manifest_encoding_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let text = std::fs::read_to_string(root.join("crates/ridl-rt/Cargo.toml"))
            .expect("ridl-rt's manifest is readable");
        let manifest: toml::Table = text.parse().expect("ridl-rt's manifest parses");
        let features = manifest["features"].as_table().expect("a [features] table");
        let names = ridl_core::manifest::ENCODING_NAMES;
        for name in names {
            assert!(
                features.contains_key(name),
                "ridl-rt has no feature `{name}`"
            );
        }
        for feature in features.keys() {
            assert!(
                names.contains(&feature.as_str())
                    || NON_ENCODING_FEATURES.contains(&feature.as_str()),
                "ridl-rt feature `{feature}` is neither an encoding name nor a listed exception"
            );
        }
    }
}
