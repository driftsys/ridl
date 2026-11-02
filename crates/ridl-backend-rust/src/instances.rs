//! The `instances` module of a generated crate: one constant for the number of
//! each instance of the system (rsdl reference §7).
//!
//! The module is one per crate, because the numbers belong to the root
//! workspace's system and not to one package. `ridlc` calls
//! [`generate_instances_module`] once with the lowered system and writes the
//! result beside the crate root.

use std::collections::BTreeMap;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::v2;

use crate::{GenerateError, Generated, ident, render};

/// A module of the tree: the constants it holds and the modules below it.
#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    constants: BTreeMap<String, Constant>,
}

/// One constant: the lock key it comes from, its value, and whether the lock
/// holds no entry for it yet.
struct Constant {
    key: String,
    number: i32,
    provisional: bool,
}

/// The source of the `instances` module for `system`: one
/// `pub const <NAME>: i32 = <number>;` per entry of every component's
/// `numbers`, inside nested `pub mod` blocks named by the component's
/// qualified name.
///
/// A module is named by a segment of the qualified name in snake case, escaped
/// as [`module_segment`](crate::module_segment) escapes a package segment. A
/// constant is named by its instance name in screaming snake case. Refuses a
/// system in which two instances map to one constant path, and names both lock
/// keys.
pub fn generate_instances_module(system: &v2::System) -> Result<Generated, GenerateError> {
    let mut root = Node::default();
    for component in &system.components {
        let qualified = component.qualified_name();
        let mut node = &mut root;
        for segment in qualified.split('.') {
            let segment = ridl_ir::name::snake_case(segment);
            node = node.children.entry(segment).or_default();
        }
        for number in &component.numbers {
            let name = ridl_ir::name::snake_case(&number.instance).to_uppercase();
            let key = format!("{qualified}/{}", number.instance);
            if let Some(existing) = node.constants.get(&name) {
                return Err(GenerateError {
                    message: format!(
                        "the instances `{}` and `{key}` both map to the constant `{name}` of \
                         the `instances` module",
                        existing.key
                    ),
                });
            }
            node.constants.insert(
                name,
                Constant {
                    key,
                    number: number.number,
                    provisional: number.provisional,
                },
            );
        }
    }
    render(items_of(&root, MODULE))
}

/// The name `ridlc` mounts the generated module under.
const MODULE: &str = "instances";

/// The items of `node`, whose module is named `parent`. A child module named
/// like its parent (component `veh.adas.Adas` gives `adas::adas`) carries
/// `#[allow(clippy::module_inception)]`, because the generated crate is linted
/// with `-D warnings`.
fn items_of(node: &Node, parent: &str) -> Vec<TokenStream> {
    let mut items = Vec::new();
    for (name, constant) in &node.constants {
        let name = format_ident!("{name}");
        let number = Literal::i32_unsuffixed(constant.number);
        let key = format!(" `{}` in `instances.lock`.", constant.key);
        let provisional = constant.provisional.then(|| {
            quote! {
                #[doc = ""]
                #[doc = " Provisional: no entry of `instances.lock` holds this number yet, and `ridl lock` writes it."]
            }
        });
        items.push(quote! {
            #[doc = #key]
            #provisional
            pub const #name: i32 = #number;
        });
    }
    for (segment, child) in &node.children {
        let module = ident(segment);
        let inner = items_of(child, segment);
        if inner.is_empty() {
            continue;
        }
        let inception = (segment == parent).then(|| quote! { #[allow(clippy::module_inception)] });
        items.push(quote! {
            #inception
            pub mod #module {
                #(#inner)*
            }
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One component: its package, its name and its `(instance, number,
    /// provisional)` entries.
    type Spec<'a> = (&'a str, &'a str, &'a [(&'a str, i32, bool)]);

    fn system_of(components: &[Spec<'_>]) -> v2::System {
        v2::System {
            components: components
                .iter()
                .map(|(package, name, numbers)| v2::Component {
                    package: package.to_string(),
                    name: name.to_string(),
                    numbers: numbers
                        .iter()
                        .map(|(instance, number, provisional)| v2::InstanceNumber {
                            instance: instance.to_string(),
                            number: *number,
                            provisional: *provisional,
                        })
                        .collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn every_instance_is_a_constant_under_its_component_path() {
        let source = generate_instances_module(&system_of(&[
            (
                "veh.adas",
                "Cruise",
                &[("primary", 1, false), ("backup", 2, false)],
            ),
            ("", "veh.ego.status", &[("Unit", 3, true)]),
        ]))
        .unwrap()
        .rust_source;
        assert!(source.contains("pub mod adas"), "{source}");
        assert!(source.contains("pub mod cruise"), "{source}");
        assert!(source.contains("pub const PRIMARY: i32 = 1;"), "{source}");
        assert!(source.contains("pub const BACKUP: i32 = 2;"), "{source}");
        assert!(source.contains("pub mod status"), "{source}");
        assert!(source.contains("pub const UNIT: i32 = 3;"), "{source}");
        assert!(
            source.contains("/// `veh.adas.Cruise/primary` in `instances.lock`."),
            "{source}"
        );
        assert_eq!(source.matches("Provisional:").count(), 1, "{source}");
    }

    /// A component named like the last segment of its package gives a module
    /// inside a module of the same name, which clippy's `module_inception`
    /// lint refuses under `-D warnings` unless it is allowed there.
    #[test]
    fn a_module_named_like_its_parent_allows_module_inception() {
        let source = generate_instances_module(&system_of(&[
            ("veh.adas", "Adas", &[("Unit", 1, false)]),
            ("veh.adas", "Cruise", &[("Unit", 2, false)]),
        ]))
        .unwrap()
        .rust_source;
        let allow = "#[allow(clippy::module_inception)]";
        assert_eq!(source.matches(allow).count(), 1, "{source}");
        let at = source.find(allow).unwrap();
        assert!(
            source[at + allow.len()..]
                .trim_start()
                .starts_with("pub mod adas {"),
            "{source}"
        );
        assert!(
            source[..at].trim_end().ends_with("pub mod adas {"),
            "{source}"
        );
    }

    #[test]
    fn two_keys_with_one_constant_path_are_refused() {
        let error = generate_instances_module(&system_of(&[
            ("veh.adas", "LaneKeep", &[("Unit", 1, false)]),
            ("", "veh.adas.lane_keep", &[("Unit", 2, false)]),
        ]))
        .unwrap_err();
        assert!(
            error.message.contains("veh.adas.LaneKeep/Unit"),
            "{}",
            error.message
        );
        assert!(
            error.message.contains("veh.adas.lane_keep/Unit"),
            "{}",
            error.message
        );
    }
}
