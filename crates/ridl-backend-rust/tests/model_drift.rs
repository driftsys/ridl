//! The fact-level drift test (the codegen model design note §8.1).
//!
//! No backend reads the lowered codegen model yet: this one derives the
//! scalar classes, the name transforms, the init rule and the derive rule
//! from the raw IR, and `ridl_ir::codegen::lower` derives the same facts
//! again for the model a plugin will read. Two derivations of one fact drift,
//! and the pull request that introduces the divergence is where it has to
//! fail — so the generated Rust is parsed back with `syn`, the facts the
//! model also states are extracted, and each is asserted equal to the model's
//! field, over the fixtures this crate already compiles.
//!
//! What is compared: each generated item's name against the declaration's own
//! spelling or the tuple's Rust induced name; each struct field's identifier
//! against `Field.name.snake`; each union variant against `Arm.name.camel`;
//! each enum variant's discriminant against `EnumValue.value`; the presence
//! of `impl Default` against `Init.derivable`; the `#[derive]` list against
//! the Rust rule stated over `Closure`; and each `MAX_SIZE` constant against
//! the declaration's FlatBuffers root bound.
//!
//! It is the form of drift test stage P2b can write. When a layer of this
//! backend stops deriving a fact and reads it from the model instead, that
//! fact is a function of the model by construction and leaves this file.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;
use support::ir::compile_fixture;

/// What the generated source says about one item.
#[derive(Default, Debug)]
struct Item {
    /// The `#[derive(...)]` list, in the order it was written.
    derives: Vec<String>,
    /// A struct's field identifiers, in order.
    fields: Vec<String>,
    /// An enum's variants, `(name, discriminant)` where one was written.
    variants: Vec<(String, Option<i64>)>,
}

#[derive(Default)]
struct Source {
    items: BTreeMap<String, Item>,
    /// Every type an `impl Default for T` was written for.
    defaults: BTreeSet<String>,
    /// Every `const MAX_SIZE: usize = N;` by the type it was written for.
    max_sizes: BTreeMap<String, u64>,
}

/// Reads the generated Rust back into the facts the model also states.
fn read_back(source: &str) -> Source {
    let file = syn::parse_file(source).expect("the backend emits parseable Rust");
    let mut out = Source::default();
    for item in &file.items {
        match item {
            syn::Item::Struct(item) => {
                let fields = match &item.fields {
                    syn::Fields::Named(named) => named
                        .named
                        .iter()
                        .map(|field| {
                            field
                                .ident
                                .as_ref()
                                .expect("a named field carries an identifier")
                                .to_string()
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                out.items.insert(
                    item.ident.to_string(),
                    Item {
                        derives: derives(&item.attrs),
                        fields,
                        variants: Vec::new(),
                    },
                );
            }
            syn::Item::Enum(item) => {
                let variants = item
                    .variants
                    .iter()
                    .map(|variant| {
                        let value = variant
                            .discriminant
                            .as_ref()
                            .and_then(|(_, expr)| literal_i64(expr));
                        (variant.ident.to_string(), value)
                    })
                    .collect();
                out.items.insert(
                    item.ident.to_string(),
                    Item {
                        derives: derives(&item.attrs),
                        fields: Vec::new(),
                        variants,
                    },
                );
            }
            syn::Item::Impl(item) => {
                let Some(target) = path_ident(&item.self_ty) else {
                    continue;
                };
                if let Some((_, path, _)) = &item.trait_
                    && path
                        .segments
                        .last()
                        .is_some_and(|segment| segment.ident == "Default")
                {
                    out.defaults.insert(target.clone());
                }
                for inner in &item.items {
                    let syn::ImplItem::Const(constant) = inner else {
                        continue;
                    };
                    if constant.ident == "MAX_SIZE"
                        && let Some(value) = literal_i64(&constant.expr)
                        && let Ok(value) = u64::try_from(value)
                    {
                        out.max_sizes.insert(target.clone(), value);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut found = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if let Some(ident) = meta.path.get_ident() {
                found.push(ident.to_string());
            }
            Ok(())
        });
    }
    found
}

fn path_ident(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn literal_i64(expr: &syn::Expr) -> Option<i64> {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Int(value) => value.base10_parse().ok(),
            _ => None,
        },
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            literal_i64(&unary.expr).map(|value| -value)
        }
        _ => None,
    }
}

/// The derive list the Rust printer writes, stated over the model's closure
/// facts alone (design note §3.5). This is the rule as prose in `derives.rs`
/// makes it, and the whole point of the comparison: the backend computes it
/// from the IR, the model states the facts, and the two must agree.
fn expected_derives(closure: &v1::Closure, ordered: bool) -> Vec<String> {
    let refused = closure.reaches_foreign
        || closure.reaches_unresolved
        || closure.reaches_cycle
        || closure.reaches_stream_or_unspecified;
    let copy = !refused && !closure.reaches_text_or_bytes && !closure.reaches_collection;
    let eq = !refused && !closure.reaches_float;
    let mut traits = vec!["Debug".to_string(), "Clone".to_string()];
    if copy {
        traits.push("Copy".to_string());
    }
    traits.push("PartialEq".to_string());
    if eq {
        traits.push("Eq".to_string());
        traits.push("Hash".to_string());
    }
    if ordered {
        traits.push("PartialOrd".to_string());
        if eq {
            traits.push("Ord".to_string());
        }
    }
    traits
}

/// True for a named scalar over an integer or a float backing: ordering is a
/// named-scalar property, which is a fact of the declaration's own kind.
fn is_ordered(declaration: &v1::Declaration) -> bool {
    matches!(
        declaration.kind.as_ref(),
        Some(v1::declaration::Kind::Scalar(scalar))
            if matches!(
                v1::ScalarClass::try_from(scalar.class),
                Ok(v1::ScalarClass::Float) | Ok(v1::ScalarClass::Integer)
            )
    )
}

fn assert_source_agrees(label: &str, model: &v1::Model, source: &Source) {
    let projection = model.flatbuffers.as_ref().expect("a lowered projection");
    let mut checked = 0usize;
    for (index, declaration) in model.declarations.iter().enumerate() {
        let name = &declaration.name.as_ref().expect("a name").declared;
        // A constant emits a `const` item, not a type; there is no derive
        // list, no field list and no `Default` for it.
        if matches!(declaration.kind, Some(v1::declaration::Kind::Constant(_))) {
            continue;
        }
        let item = source
            .items
            .get(name)
            .unwrap_or_else(|| panic!("{label}: the generated crate declares no `{name}`"));
        let closure = declaration.closure.as_ref().expect("a closure");
        assert_eq!(
            item.derives,
            expected_derives(closure, is_ordered(declaration)),
            "{label}: `{name}` derives {:?}, the model's closure {closure:?} states \
             {:?}",
            item.derives,
            expected_derives(closure, is_ordered(declaration))
        );

        let init = declaration.init.as_ref().expect("an init");
        assert_eq!(
            source.defaults.contains(name),
            init.derivable,
            "{label}: `{name}` has an `impl Default` = {}, the model states derivable = {}",
            source.defaults.contains(name),
            init.derivable
        );

        match declaration.kind.as_ref() {
            Some(v1::declaration::Kind::Struct(def)) => {
                let expected: Vec<String> = def
                    .slots
                    .iter()
                    .filter_map(|slot| match slot.occupant.as_ref()? {
                        v1::slot::Occupant::Field(field) => {
                            Some(field.name.as_ref().expect("a field name").snake.clone())
                        }
                        v1::slot::Occupant::Retired(_) => None,
                    })
                    .collect();
                assert_eq!(
                    item.fields, expected,
                    "{label}: `{name}` emits fields {:?}, the model states {expected:?}",
                    item.fields
                );
            }
            Some(v1::declaration::Kind::Enum(def)) => {
                let expected: Vec<(String, Option<i64>)> = def
                    .values
                    .iter()
                    .map(|value| {
                        (
                            value.name.as_ref().expect("a value name").declared.clone(),
                            Some(value.value),
                        )
                    })
                    .collect();
                assert_eq!(
                    item.variants, expected,
                    "{label}: `{name}` emits variants {:?}, the model states {expected:?}",
                    item.variants
                );
            }
            Some(v1::declaration::Kind::Union(def)) => {
                let expected: Vec<(String, Option<i64>)> = def
                    .arms
                    .iter()
                    .map(|arm| (arm.name.as_ref().expect("an arm name").camel.clone(), None))
                    .collect();
                assert_eq!(
                    item.variants, expected,
                    "{label}: `{name}` emits variants {:?}, the model states {expected:?}",
                    item.variants
                );
            }
            _ => {}
        }

        // The codec's `MAX_SIZE` is the projection's own bound for the
        // declaration's root, where the root has one.
        if let Some(root) = projection
            .roots
            .iter()
            .find(|root| root.declaration == index as u32)
            && let Some(v1::fb_root::Bound::MaxSize(bound)) = root.bound.as_ref()
            && let Some(emitted) = source.max_sizes.get(name)
        {
            assert_eq!(
                *emitted,
                u64::from(*bound),
                "{label}: `{name}` emits MAX_SIZE = {emitted}, the model states {bound}"
            );
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "{label}: the fixture must declare something to compare"
    );

    // Every induced tuple generates a struct named for the path that reached
    // it, with the fields the model lists.
    for tuple in &model.tuples {
        let name = &tuple.name.as_ref().expect("an induced name").rust;
        if name.is_empty() {
            // An interaction position: the face refuses a call whose reply is
            // not a named type, so no struct is generated and the model
            // leaves both names empty.
            continue;
        }
        let item = source
            .items
            .get(name)
            .unwrap_or_else(|| panic!("{label}: the generated crate declares no tuple `{name}`"));
        let expected: Vec<String> = tuple
            .fields
            .iter()
            .map(|field| field.name.as_ref().expect("a field name").snake.clone())
            .collect();
        assert_eq!(
            item.fields, expected,
            "{label}: tuple `{name}` emits fields {:?}, the model states {expected:?}",
            item.fields
        );
        let closure = tuple.closure.as_ref().expect("a closure");
        assert_eq!(
            item.derives,
            expected_derives(closure, false),
            "{label}: tuple `{name}` derives {:?}, the model's closure states {:?}",
            item.derives,
            expected_derives(closure, false)
        );
        assert_eq!(
            source.defaults.contains(name),
            tuple.init.as_ref().expect("an init").derivable,
            "{label}: tuple `{name}`'s `impl Default` and the model's init must agree"
        );
    }
}

fn check(label: &str, package: &v2::Package) {
    let generated = ridl_backend_rust::generate(package).expect("generate");
    let source = read_back(&generated.rust_source);
    let model = codegen::lower(package, &[]);
    assert_source_agrees(label, &model, &source);
}

/// The wire backends' cruise-control fixture, reached by a relative path
/// rather than copied: it is the one fixture in the tree that carries a
/// union, an induced tuple and a collection together, which this crate's two
/// own fixtures do not (`ridl-backend-flatbuffers`'s corpus test reuses it
/// the same way, for the same reason).
fn compile_wire_fixture(file_name: &str) -> v2::Package {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-proto/tests/fixtures")
        .join(file_name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let output = ridlc::compile(&path.display().to_string(), &text);
    assert!(
        output.diagnostics.is_empty(),
        "{} must compile with no diagnostic, got: {:?}",
        path.display(),
        output.diagnostics,
    );
    output.package
}

#[test]
fn the_generated_crate_states_the_facts_the_model_states() {
    check(
        "flatbuffers_roundtrip.ridl",
        &compile_fixture("flatbuffers_roundtrip.ridl"),
    );
    check(
        "interaction_face.ridl",
        &compile_fixture("interaction_face.ridl"),
    );
    check("cruise.ridl", &compile_wire_fixture("cruise.ridl"));
}
