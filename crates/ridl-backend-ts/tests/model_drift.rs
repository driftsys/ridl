//! The fact-level drift test (the codegen model design note §8.1).
//!
//! No backend reads the lowered codegen model yet: this one derives the
//! scalar classes, the widths, the init rule and the import aliases from the
//! raw IR, and `ridl_ir::codegen::lower` derives the same facts again for the
//! model a plugin will read. Two derivations of one fact drift, and the pull
//! request that introduces the divergence is where it has to fail — so the
//! generated TypeScript is read back line by line, in the style this crate's
//! own tests use, and each fact the model also states is asserted equal to
//! the model's field.
//!
//! What is compared: each `export type`/`interface`/`enum` against the
//! declaration's own spelling, exported exactly when the model states the
//! declaration is public; each property name against `Field.name.declared`
//! with `?` exactly when the model states the position is optional; a
//! `bigint` brand exactly where the model states a 64-bit integer width; an
//! `init<Name>` function exactly when the model states the init is derivable;
//! and each `import * as x from './p'` against a package the model lists as
//! foreign, under `DottedName.underscored`.
//!
//! It is the form of drift test stage P2b can write. When this backend stops
//! deriving a fact and reads it from the model instead, that fact is a
//! function of the model by construction and leaves this file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ridl_ir::codegen::{self, v1};
use ridl_ir::v2;

/// One declared item of the generated module.
#[derive(Debug, PartialEq, Eq)]
struct Declared {
    exported: bool,
    /// The text after `=` for a `type` alias, empty otherwise.
    body: String,
    /// An interface's properties, `(name, optional)` in order.
    properties: Vec<(String, bool)>,
}

#[derive(Default)]
struct Module {
    items: BTreeMap<String, Declared>,
    /// Every `init<Name>` function, by the name it inits.
    inits: BTreeSet<String>,
    /// Every `import * as <alias> from './<package>';`, alias to package.
    imports: BTreeMap<String, String>,
}

/// A property line of an emitted interface: `name: Type;` or `name?: Type;`.
/// A doc-comment line and a blank line carry no property, and a line that
/// does not end in `;` is a continuation of a multi-line type.
fn property(line: &str) -> Option<(String, bool)> {
    if line.starts_with('*') || line.starts_with("/*") || !line.ends_with(';') {
        return None;
    }
    let (left, _) = line.split_once(':')?;
    let name = left.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '?')
    {
        return None;
    }
    Some((name.trim_end_matches('?').to_string(), name.ends_with('?')))
}

/// The interface or enum whose body the reader is inside: its name, whether
/// it was exported, and the properties read so far.
struct Open {
    name: String,
    exported: bool,
    properties: Vec<(String, bool)>,
}

/// Reads the generated module back into the facts the model also states.
fn read_back(source: &str) -> Module {
    let mut module = Module::default();
    let mut open: Option<Open> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("import * as ") {
            let (alias, rest) = rest
                .split_once(" from ")
                .expect("an import names its module");
            let package = rest
                .trim_matches(|c| c == '\'' || c == ';')
                .trim_start_matches("./")
                .to_string();
            module.imports.insert(alias.trim().to_string(), package);
            continue;
        }
        if let Some(block) = open.as_mut() {
            if trimmed == "}" {
                module.items.insert(
                    block.name.clone(),
                    Declared {
                        exported: block.exported,
                        body: String::new(),
                        properties: std::mem::take(&mut block.properties),
                    },
                );
                open = None;
                continue;
            }
            if let Some(found) = property(trimmed) {
                block.properties.push(found);
            }
            continue;
        }
        let (exported, rest) = match trimmed.strip_prefix("export ") {
            Some(rest) => (true, rest),
            None => (false, trimmed),
        };
        for keyword in ["interface ", "enum "] {
            if let Some(rest) = rest.strip_prefix(keyword) {
                open = Some(Open {
                    name: rest.trim_end_matches('{').trim().to_string(),
                    exported,
                    properties: Vec::new(),
                });
                break;
            }
        }
        if open.is_some() {
            continue;
        }
        if let Some(rest) = rest.strip_prefix("type ")
            && let Some((name, body)) = rest.split_once('=')
        {
            module.items.insert(
                name.trim().to_string(),
                Declared {
                    exported,
                    body: body.trim().trim_end_matches(';').to_string(),
                    properties: Vec::new(),
                },
            );
            continue;
        }
        if let Some(rest) = rest.strip_prefix("function init")
            && let Some((name, _)) = rest.split_once('(')
        {
            module.inits.insert(name.to_string());
        }
    }
    module
}

fn assert_module_agrees(label: &str, model: &v1::Model, module: &Module) {
    let mut checked = 0usize;
    for declaration in &model.declarations {
        let name = &declaration.name.as_ref().expect("a name").declared;
        // A constant emits a `const` binding, not a declared type.
        let properties = match declaration.kind.as_ref() {
            Some(v1::declaration::Kind::Constant(_)) | None => continue,
            Some(v1::declaration::Kind::Struct(def)) => def
                .slots
                .iter()
                .filter_map(|slot| match slot.occupant.as_ref()? {
                    v1::slot::Occupant::Field(field) => Some((
                        field.name.as_ref().expect("a field name").declared.clone(),
                        field.r#type.as_ref().is_some_and(|ty| ty.optional),
                    )),
                    v1::slot::Occupant::Retired(_) => None,
                })
                .collect(),
            Some(_) => Vec::new(),
        };
        let declared = module
            .items
            .get(name)
            .unwrap_or_else(|| panic!("{label}: the generated module declares no `{name}`"));
        let public = v1::Visibility::try_from(declaration.visibility) == Ok(v1::Visibility::Public);
        assert_eq!(
            declared.exported, public,
            "{label}: `{name}` is exported = {}, the model states public = {public}",
            declared.exported
        );
        if matches!(declaration.kind, Some(v1::declaration::Kind::Struct(_))) {
            assert_eq!(
                declared.properties, properties,
                "{label}: `{name}` emits properties {:?}, the model states {properties:?}",
                declared.properties
            );
        }
        if let Some(v1::declaration::Kind::Scalar(scalar)) = declaration.kind.as_ref() {
            let wide = matches!(
                scalar.width,
                Some(v1::scalar::Width::IntWidth(width))
                    if width == v1::IntWidth::U64 as i32 || width == v1::IntWidth::I64 as i32
            );
            assert_eq!(
                declared.body.starts_with("bigint"),
                wide,
                "{label}: `{name}` brands `{}`, the model states a 64-bit width = {wide}",
                declared.body
            );
        }
        let init = declaration.init.as_ref().expect("an init");
        assert_eq!(
            module.inits.contains(name),
            init.derivable,
            "{label}: `{name}` has an `init{name}` = {}, the model states derivable = {}",
            module.inits.contains(name),
            init.derivable
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "{label}: the fixture must declare something to compare"
    );

    let foreign: BTreeSet<String> = model
        .foreign
        .iter()
        .map(|foreign| foreign.package.clone())
        .collect();
    for (alias, package) in &module.imports {
        assert!(
            foreign.contains(package),
            "{label}: the module imports `{package}`, the model lists {foreign:?}"
        );
        assert_eq!(
            *alias,
            package.replace('.', "_"),
            "{label}: the import alias of `{package}` is its underscored form, which the model \
             carries as `DottedName.underscored`"
        );
    }
}

fn compile_wire_fixture(file_name: &str) -> v2::Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
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
fn the_generated_module_states_the_facts_the_model_states() {
    let package = compile_wire_fixture("cruise.ridl");
    let generated = ridl_backend_ts::generate(&package).expect("generate");
    let module = read_back(&generated.source);
    let model = codegen::lower(&package, &[]);
    assert_module_agrees("cruise.ridl", &model, &module);
}

/// The cross-package half: the import a foreign reference draws, and the
/// alias it is bound under.
#[test]
fn a_cross_package_module_states_the_facts_the_model_states() {
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-backend-proto/tests/fixtures/cross-package");
    let output = ridlc::compile_workspace(&mut db, &entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    assert!(
        output.diagnostics.is_empty(),
        "the cross-package fixture must compile with no diagnostic, got: {:?}",
        output.diagnostics,
    );
    let package = |name: &str| {
        output
            .checked
            .iter()
            .find(|checked| checked.ir.name == name)
            .unwrap_or_else(|| panic!("the fixture declares a {name} member"))
            .ir
            .clone()
    };
    let parts = package("proto.parts");
    let vehicle = package("proto.vehicle");
    let generated = ridl_backend_ts::generate(&vehicle).expect("generate");
    let module = read_back(&generated.source);
    let model = codegen::lower(&vehicle, &[&parts]);
    assert_module_agrees("proto.vehicle", &model, &module);
    assert!(
        !module.imports.is_empty(),
        "the referencing package imports its sibling"
    );
}
