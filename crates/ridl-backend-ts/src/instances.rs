//! Instance numbers exported once per system, beside the package modules.

use std::collections::BTreeMap;

use ridl_ir::v2;

use crate::GenerateError;

/// Generates an object with one literal constant per instance, nested under
/// the snake case segments of each component's qualified name.
/// Refuses two lock keys that map to the same constant path.
pub fn generate_instances_module(system: &v2::System) -> Result<String, GenerateError> {
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
                return Err(GenerateError::Unrepresentable(format!(
                    "the instances `{}` and `{key}` both map to the constant `{name}` of \
                     the `instances` module",
                    existing.key
                )));
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
    let body = properties_of(&root, 1);
    if body.is_empty() {
        Ok("export const instances = {} as const;\n".to_string())
    } else {
        Ok(format!("export const instances = {{\n{body}}} as const;\n"))
    }
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    constants: BTreeMap<String, Constant>,
}

struct Constant {
    key: String,
    number: i32,
    provisional: bool,
}

fn properties_of(node: &Node, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let mut source = String::new();
    for (name, constant) in &node.constants {
        source.push_str(&format!(
            "{indent}/**\n{indent} * `{}` in `instances.lock`.\n",
            constant.key
        ));
        if constant.provisional {
            source.push_str(&format!(
                "{indent} *\n{indent} * Provisional: no entry of `instances.lock` holds this number yet, and `ridl lock` writes it.\n"
            ));
        }
        source.push_str(&format!(
            "{indent} */\n{indent}{}: {},\n",
            property_name(name),
            constant.number
        ));
    }
    for (name, child) in &node.children {
        let body = properties_of(child, depth + 1);
        if !body.is_empty() {
            source.push_str(&format!(
                "{indent}{}: {{\n{body}{indent}}},\n",
                property_name(name)
            ));
        }
    }
    source
}

fn property_name(name: &str) -> String {
    let mut chars = name.chars();
    let valid_start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$');
    if valid_start && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$') {
        name.to_string()
    } else {
        crate::ts_string(name)
    }
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
        .unwrap();
        assert!(source.contains("PRIMARY: 1,"), "{source}");
        assert!(source.contains("BACKUP: 2,"), "{source}");
        assert!(source.contains("UNIT: 3,"), "{source}");
        assert!(
            source.contains("`veh.adas.Cruise/primary` in `instances.lock`."),
            "{source}"
        );
        assert_eq!(source.matches("Provisional:").count(), 1, "{source}");
        assert!(source.contains("Provisional: no entry of `instances.lock` holds this number yet, and `ridl lock` writes it."), "{source}");
        insta::assert_snapshot!(source);
    }

    #[test]
    fn a_component_named_like_its_parent_keeps_both_segments() {
        let source = generate_instances_module(&system_of(&[
            ("veh.adas", "Adas", &[("Unit", 1, false)]),
            ("veh.adas", "Cruise", &[("Unit", 2, false)]),
        ]))
        .unwrap();
        insta::assert_snapshot!(source);
    }

    #[test]
    fn two_keys_with_one_constant_path_are_refused() {
        let error = generate_instances_module(&system_of(&[
            ("veh.adas", "LaneKeep", &[("Unit", 1, false)]),
            ("", "veh.adas.lane_keep", &[("Unit", 2, false)]),
        ]))
        .unwrap_err();
        let GenerateError::Unrepresentable(message) = error;
        assert!(message.contains("veh.adas.LaneKeep/Unit"), "{message}");
        assert!(message.contains("veh.adas.lane_keep/Unit"), "{message}");
    }

    #[test]
    fn instance_names_that_collide_after_casing_are_refused() {
        let error = generate_instances_module(&system_of(&[(
            "veh",
            "Cruise",
            &[("httpServer", 1, false), ("HTTPServer", 2, false)],
        )]))
        .unwrap_err();
        let GenerateError::Unrepresentable(message) = error;
        assert!(message.contains("veh.Cruise/httpServer"), "{message}");
        assert!(message.contains("veh.Cruise/HTTPServer"), "{message}");
    }

    #[test]
    fn names_use_snake_case_and_quote_invalid_identifiers() {
        let source = generate_instances_module(&system_of(&[
            (
                "veh.HTTPServer",
                "LaneKeep",
                &[("primaryUnit", 1, false), ("2nd", 2, false)],
            ),
            ("default", "2nd", &[("Unit", 3, false)]),
        ]))
        .unwrap();
        assert!(source.contains("http_server: {"), "{source}");
        assert!(source.contains("lane_keep: {"), "{source}");
        assert!(source.contains("PRIMARY_UNIT: 1,"), "{source}");
        assert!(source.contains("'2ND': 2,"), "{source}");
        assert!(source.contains("default: {"), "{source}");
        assert!(source.contains("'2nd': {"), "{source}");
        insta::assert_snapshot!(source);
    }

    #[test]
    fn empty_components_add_no_keys() {
        let empty = "export const instances = {} as const;\n";
        assert_eq!(
            generate_instances_module(&v2::System::default()).unwrap(),
            empty
        );
        assert_eq!(
            generate_instances_module(&system_of(&[("veh.adas", "Unused", &[])])).unwrap(),
            empty
        );
        let source = generate_instances_module(&system_of(&[
            ("veh.adas", "Unused", &[]),
            ("veh.ego", "Status", &[("Unit", 3, false)]),
        ]))
        .unwrap();
        assert!(!source.contains("adas"), "{source}");
        assert!(!source.contains("unused"), "{source}");
        insta::assert_snapshot!(source);
    }
}
