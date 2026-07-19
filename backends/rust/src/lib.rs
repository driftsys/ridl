//! IR package to Rust source text (ADR-0004 section 7, ADR-0006 decision 1).
//!
//! The walking-skeleton emission over IR v1: each `TypeDef` declaration
//! becomes `pub struct Name(pub f64);` and each `ConstDef` whose type
//! resolves to one of those type declarations becomes
//! `pub const NAME: Type = Type(value);`. Composite declarations (structs,
//! enums, enum sets, unions) are not emitted yet — the full backend over the
//! v1 surface is the E1.12 task. The source is built as a
//! `proc_macro2::TokenStream` via `quote` and formatted with `prettyplease`,
//! never by shelling out to `rustfmt`.

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::v1;

/// A failure to generate Rust source from a package.
///
/// Carried as a value so codegen stays total: no stage in the pipeline panics
/// (ADR-0004 section 5). The `compile` driver folds `message` into its
/// diagnostic list.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates Rust source text for `package`.
///
/// Emission order is all type structs first (in declaration order), then all
/// consts (in declaration order). A `ConstDef` whose `type_ref` does not name
/// a `TypeDef` declaration in `package` — a primitive-typed or unresolved
/// const — is skipped rather than emitted or panicked on: the checker
/// upstream of codegen owns those diagnostics.
///
/// The call is total: it returns [`GenerateError`] rather than panicking. A
/// typl name can lex as a valid identifier yet still be a Rust keyword (for
/// example `fn`), which cannot appear where the generated code uses it. Every
/// emitted name is validated up front with `syn::parse_str::<syn::Ident>`,
/// before any token is built, because `format_ident!` itself panics on a name
/// that is neither a legal identifier nor a keyword. When any emitted name is
/// invalid, the whole call fails with one message listing the offending names.
/// Raw-identifier escaping and name mangling are an E1.12 backend decision,
/// not resolved here.
pub fn generate(package: &v1::Package) -> Result<String, GenerateError> {
    let type_names: Vec<&str> = package
        .decls
        .iter()
        .filter(|decl| matches!(decl.kind, Some(v1::decl::Kind::TypeDef(_))))
        .map(|decl| decl.name.as_str())
        .collect();

    let mut invalid_names: Vec<String> = Vec::new();
    for name in &type_names {
        if !is_rust_ident(name) {
            invalid_names.push((*name).to_string());
        }
    }
    for decl in &package.decls {
        // Mirror `generate_const`: a const whose type does not resolve is
        // skipped, so its name is never emitted and never validated.
        if let Some(v1::decl::Kind::ConstDef(const_def)) = &decl.kind
            && const_resolves(const_def, &type_names)
            && !is_rust_ident(&decl.name)
        {
            invalid_names.push(decl.name.clone());
        }
    }
    if !invalid_names.is_empty() {
        return Err(GenerateError {
            message: format!(
                "generated Rust would use invalid identifier(s): {}",
                invalid_names.join(", ")
            ),
        });
    }

    let structs = package.decls.iter().filter_map(|decl| match &decl.kind {
        Some(v1::decl::Kind::TypeDef(_)) => Some(generate_struct(&decl.name)),
        _ => None,
    });
    let consts = package.decls.iter().filter_map(|decl| {
        let Some(v1::decl::Kind::ConstDef(const_def)) = &decl.kind else {
            return None;
        };
        generate_const(&decl.name, const_def, &type_names)
    });

    let tokens = quote! {
        #(#structs)*
        #(#consts)*
    };

    let file: syn::File = syn::parse2(tokens).map_err(|err| GenerateError {
        message: format!("generated Rust does not parse: {err}"),
    })?;
    Ok(prettyplease::unparse(&file))
}

/// Reports whether `name` parses as a Rust identifier. Unlike
/// `proc_macro2::Ident::new`, `syn::parse_str::<syn::Ident>` rejects reserved
/// keywords such as `fn`, so this catches exactly the names that would later
/// fail to parse as generated code.
fn is_rust_ident(name: &str) -> bool {
    syn::parse_str::<syn::Ident>(name).is_ok()
}

/// Whether a const's type reference names a `TypeDef` declaration of the
/// same package.
fn const_resolves(const_def: &v1::ConstDef, type_names: &[&str]) -> bool {
    const_def
        .type_ref
        .as_deref()
        .is_some_and(|type_ref| type_names.contains(&type_ref))
}

fn generate_struct(name: &str) -> TokenStream {
    let name = format_ident!("{}", name);
    quote! { pub struct #name(pub f64); }
}

fn generate_const(
    name: &str,
    const_def: &v1::ConstDef,
    type_names: &[&str],
) -> Option<TokenStream> {
    if !const_resolves(const_def, type_names) {
        return None;
    }
    // The IR carries the value as a canonical decimal string (ADR-0007
    // decision 9); the f64 newtype emission is the walking-skeleton shape, so
    // a value the string cannot express as f64 skips the const rather than
    // panicking.
    let value: f64 = const_def.value.parse().ok()?;
    let type_name = format_ident!("{}", const_def.type_ref.as_deref()?);
    let const_name = format_ident!("{}", name);
    let value = Literal::f64_suffixed(value);
    Some(quote! { pub const #const_name: #type_name = #type_name(#value); })
}

#[cfg(test)]
mod tests {
    use super::generate;
    use ridl_ir::v1;
    use std::process::Command;

    fn decl(name: &str, kind: v1::decl::Kind) -> v1::Decl {
        v1::Decl {
            name: name.to_string(),
            visibility: v1::Visibility::Public as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            kind: Some(kind),
        }
    }

    fn type_def(unit: &str) -> v1::decl::Kind {
        v1::decl::Kind::TypeDef(v1::TypeDef {
            backing: Some(v1::Backing {
                kind: Some(v1::backing::Kind::Unit(unit.to_string())),
            }),
            constraint: None,
            declared_init: None,
            init: None,
            width: Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F32 as i32)),
        })
    }

    fn fixture() -> v1::Package {
        v1::Package {
            name: "vehicle".to_string(),
            decls: vec![
                decl("Speed", type_def("km/h")),
                decl(
                    "MAX_SPEED",
                    v1::decl::Kind::ConstDef(v1::ConstDef {
                        type_ref: Some("Speed".to_string()),
                        value: "250".to_string(),
                        regex: None,
                    }),
                ),
            ],
        }
    }

    #[test]
    fn type_named_with_a_rust_keyword_is_an_error_not_a_panic() {
        let package = v1::Package {
            name: "bad".to_string(),
            decls: vec![decl("fn", type_def("m"))],
        };

        let error = generate(&package).expect_err("a Rust keyword type name must not generate");

        assert!(
            error.message.contains("fn"),
            "the error must name the offending identifier, got: {}",
            error.message
        );
    }

    #[test]
    fn generates_struct_and_const_for_fixture_package() {
        let source = generate(&fixture()).expect("the fixture package generates valid Rust");

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
        let source = generate(&fixture()).expect("the fixture package generates valid Rust");

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
