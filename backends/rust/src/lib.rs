//! IR module to Rust source text (ADR-0004 section 7, ADR-0006 decision 1).
//!
//! Each `TypeDef` becomes `pub struct Name(pub f64);` and each `ConstDef`
//! becomes `pub const NAME: Type = Type(value);`. The source is built as a
//! `proc_macro2::TokenStream` via `quote` and formatted with `prettyplease`,
//! never by shelling out to `rustfmt`.

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::{ConstDef, Module, TypeDef};

/// Generates Rust source text for `module`.
///
/// Emission order is all structs first (in module order), then all consts
/// (in module order). A `ConstDef` whose `type_name` does not name a
/// `TypeDef` in `module` is skipped rather than emitted or panicked on — the
/// checker upstream of codegen owns that diagnostic; E0 assumes the IR it
/// receives is well-formed.
pub fn generate(module: &Module) -> String {
    let structs = module.types.iter().map(generate_struct);
    let consts = module
        .consts
        .iter()
        .filter_map(|const_def| generate_const(const_def, module));

    let tokens = quote! {
        #(#structs)*
        #(#consts)*
    };

    let file: syn::File =
        syn::parse2(tokens).expect("generated tokens must parse as a well-formed Rust file");
    prettyplease::unparse(&file)
}

fn generate_struct(type_def: &TypeDef) -> TokenStream {
    let name = format_ident!("{}", type_def.name);
    quote! { pub struct #name(pub f64); }
}

fn generate_const(const_def: &ConstDef, module: &Module) -> Option<TokenStream> {
    let type_def = module
        .types
        .iter()
        .find(|t| t.name == const_def.type_name)?;

    let const_name = format_ident!("{}", const_def.name);
    let type_name = format_ident!("{}", type_def.name);
    let value = Literal::f64_suffixed(const_def.value);

    Some(quote! { pub const #const_name: #type_name = #type_name(#value); })
}

#[cfg(test)]
mod tests {
    use super::generate;
    use ridl_ir::{ConstDef, Module, Range, TypeDef};
    use std::process::Command;

    fn fixture() -> Module {
        Module {
            name: "vehicle".to_string(),
            types: vec![TypeDef {
                name: "Speed".to_string(),
                unit: "km/h".to_string(),
                range: Some(Range {
                    min: 0.0,
                    max: 250.0,
                    step: 0.5,
                }),
            }],
            consts: vec![ConstDef {
                name: "MAX_SPEED".to_string(),
                type_name: "Speed".to_string(),
                value: 250.0,
            }],
        }
    }

    #[test]
    fn generates_struct_and_const_for_fixture_module() {
        let source = generate(&fixture());

        assert!(
            source.contains("pub struct Speed(pub f64)"),
            "generated source must declare the Speed newtype, got:\n{source}"
        );
        assert!(
            source.contains("pub const MAX_SPEED: Speed"),
            "generated source must declare the MAX_SPEED constant, got:\n{source}"
        );
    }

    #[test]
    fn generated_source_compiles_with_rustc() {
        let source = generate(&fixture());

        let dir = std::env::temp_dir();
        let unique = format!(
            "ridl_backend_rust_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must read a time after the unix epoch")
                .as_nanos()
        );
        let source_path = dir.join(format!("{unique}.rs"));
        let rmeta_path = dir.join(format!("{unique}.rmeta"));

        std::fs::write(&source_path, &source).expect("must write generated source to a temp file");

        let status = Command::new("rustc")
            .args([
                "--edition",
                "2024",
                "--crate-type",
                "lib",
                "--emit",
                "metadata",
            ])
            .arg("-o")
            .arg(&rmeta_path)
            .arg(&source_path)
            .status()
            .expect("rustc must be installed and runnable for this test to be meaningful");

        std::fs::remove_file(&source_path).ok();
        std::fs::remove_file(&rmeta_path).ok();

        assert!(
            status.success(),
            "generated source must compile with rustc, source:\n{source}"
        );
    }
}
