//! The MCP tool surface: the tool names and every property path of each input
//! and output schema, with its type, whether it is required, each `enum` value
//! and `const` of a path, and its `additionalProperties`, `minimum` and
//! `format`, from `crates/ridl-mcp/tests/tools.json`. A path inside a `oneOf`
//! branch carries `/oneOf/<n>`; an `anyOf` branch adds nothing to the path.

use serde_json::Value;
use std::path::Path;

/// RFC 6901 escaping of one reference token.
fn token(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// The type of a schema node: its `type` (a list joined with `|`), else the
/// types of the `$ref` target or of the `anyOf` or `oneOf` branches, else
/// `any`.
fn type_of(node: &Value, defs: &Value) -> String {
    let named = |v: &Value| match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => {
            let mut names: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
            names.sort();
            Some(names.join("|"))
        }
        _ => None,
    };
    if let Some(t) = node.get("type").and_then(named) {
        return t;
    }
    if let Some(target) = resolve_ref(node, defs) {
        return type_of(target, defs);
    }
    let branches: Vec<&Value> = branches(node).collect();
    if !branches.is_empty() {
        let mut names: Vec<String> = branches.iter().map(|b| type_of(b, defs)).collect();
        names.sort();
        names.dedup();
        return names.join("|");
    }
    "any".to_string()
}

/// The branches of `node`'s `anyOf` and `oneOf`.
fn branches(node: &Value) -> impl Iterator<Item = &Value> {
    ["anyOf", "oneOf"]
        .into_iter()
        .filter_map(|key| node.get(key).and_then(Value::as_array))
        .flatten()
}

/// A JSON value as a dump word: a string without its quotes, anything else
/// as JSON.
fn word(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

fn resolve_ref<'a>(node: &Value, defs: &'a Value) -> Option<&'a Value> {
    let name = node.get("$ref")?.as_str()?.strip_prefix("#/$defs/")?;
    defs.get(name)
}

/// Adds one `<path> required-set <names>` line for a schema object that has
/// `properties` or `required`, holding its required names sorted, two lines
/// for each property of `node`, its type and whether it is `required` or
/// `optional`, a `<path> enum <value>` line for each value of its
/// `enum`, a `<path> const <value>` line for its `const`, a
/// `<path> <keyword> <value>` line for each of its `additionalProperties`,
/// `minimum` and `format`, and recurses into nested `properties`, `items`,
/// `$ref` targets and `anyOf` and `oneOf` branches. An `anyOf` branch is
/// walked on the parent path, because each `anyOf` in the schemas wraps one
/// value and `null`. A `oneOf` branch is walked on `<pointer>/oneOf/<n>`,
/// because its branches are alternative shapes: a property that moves between
/// branches, or a reordering of the branches, removes a line. `depth` bounds
/// the walk for a recursive `$ref`.
fn walk(
    tool: &str,
    side: &str,
    pointer: &str,
    node: &Value,
    defs: &Value,
    depth: usize,
    lines: &mut Vec<String>,
) {
    if depth > 8 {
        return;
    }
    if let Some(target) = resolve_ref(node, defs) {
        walk(tool, side, pointer, target, defs, depth + 1, lines);
    }
    for branch in node
        .get("anyOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        walk(tool, side, pointer, branch, defs, depth + 1, lines);
    }
    for (index, branch) in node
        .get("oneOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let path = format!("{pointer}/oneOf/{index}");
        walk(tool, side, &path, branch, defs, depth + 1, lines);
    }
    for value in node
        .get("enum")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        lines.push(format!("{tool} {side} {pointer} enum {}", word(value)));
    }
    if let Some(value) = node.get("const") {
        lines.push(format!("{tool} {side} {pointer} const {}", word(value)));
    }
    for keyword in ["additionalProperties", "minimum", "format"] {
        if let Some(value) = node.get(keyword) {
            lines.push(format!("{tool} {side} {pointer} {keyword} {}", word(value)));
        }
    }
    let required: Vec<&str> = node
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let properties = node.get("properties").and_then(Value::as_object);
    // One line holds the whole required set of a schema object, so a property
    // that becomes required, or a new required one, removes it.
    if properties.is_some() || node.get("required").is_some() {
        let mut sorted: Vec<String> = required.iter().map(|name| token(name)).collect();
        sorted.sort();
        lines.push(
            std::iter::once(format!("{tool} {side} {pointer} required-set"))
                .chain(sorted)
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    // A required name that `properties` does not list still gets its line.
    for name in required
        .iter()
        .filter(|name| !properties.is_some_and(|p| p.contains_key(**name)))
    {
        lines.push(format!(
            "{tool} {side} {pointer}/properties/{} required",
            token(name)
        ));
    }
    if let Some(properties) = properties {
        for (name, property) in properties {
            let path = format!("{pointer}/properties/{}", token(name));
            lines.push(format!("{tool} {side} {path} {}", type_of(property, defs)));
            let requirement = if required.contains(&name.as_str()) {
                "required"
            } else {
                "optional"
            };
            lines.push(format!("{tool} {side} {path} {requirement}"));
            walk(tool, side, &path, property, defs, depth + 1, lines);
        }
    }
    if let Some(items) = node.get("items") {
        let path = format!("{pointer}/items");
        lines.push(format!("{tool} {side} {path} {}", type_of(items, defs)));
        walk(tool, side, &path, items, defs, depth + 1, lines);
    }
}

pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join("crates/ridl-mcp/tests/tools.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let tools: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let tools = tools
        .as_array()
        .ok_or_else(|| format!("{}: not an array", path.display()))?;
    let mut lines = Vec::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{}: a tool has no name", path.display()))?;
        lines.push(name.to_string());
        for (side, key) in [("input", "inputSchema"), ("output", "outputSchema")] {
            if let Some(schema) = tool.get(key) {
                let defs = schema.get("$defs").unwrap_or(&Value::Null);
                walk(name, side, "", schema, defs, 0, &mut lines);
            }
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walked(side: &str, schema: Value) -> Vec<String> {
        let defs = schema.get("$defs").cloned().unwrap_or(Value::Null);
        let mut lines = Vec::new();
        walk("t", side, "", &schema, &defs, 0, &mut lines);
        lines
    }

    #[test]
    fn walk_reaches_the_branches_of_a_one_of() {
        let lines = walked(
            "output",
            serde_json::json!({
                "oneOf": [
                    {"properties": {"a": {"type": "string"}}, "required": ["a"]},
                    {"properties": {"b": {"type": "integer"}}}
                ]
            }),
        );
        for line in [
            "t output /oneOf/0/properties/a string",
            "t output /oneOf/0/properties/a required",
            "t output /oneOf/1/properties/b integer",
        ] {
            assert!(
                lines.iter().any(|l| l == line),
                "missing `{line}` in {lines:?}"
            );
        }
    }

    #[test]
    fn a_property_that_moves_between_one_of_branches_removes_a_line() {
        let before = walked(
            "output",
            serde_json::json!({"oneOf": [
                {"type": "object", "properties": {"a": {"type": "string"}}},
                {"type": "object", "properties": {"b": {"type": "string"}}}
            ]}),
        );
        let after = walked(
            "output",
            serde_json::json!({"oneOf": [
                {"type": "object", "properties": {}},
                {"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "string"}}}
            ]}),
        );
        let line = "t output /oneOf/0/properties/a string";
        assert!(before.iter().any(|l| l == line), "{before:?}");
        assert!(!after.iter().any(|l| l == line), "{after:?}");
    }

    #[test]
    fn a_nested_one_of_carries_both_indices() {
        let lines = walked(
            "output",
            serde_json::json!({"oneOf": [
                {"type": "object"},
                {"type": "object", "properties": {"x": {"oneOf": [
                    {"type": "object", "properties": {"y": {"type": "integer"}}}
                ]}}}
            ]}),
        );
        let line = "t output /oneOf/1/properties/x/oneOf/0/properties/y integer";
        assert!(lines.iter().any(|l| l == line), "{lines:?}");
    }

    #[test]
    fn a_one_of_branch_that_is_a_ref_uses_the_branch_path() {
        let lines = walked(
            "output",
            serde_json::json!({
                "$defs": {"A": {"type": "object", "properties": {"z": {"type": "boolean"}}}},
                "oneOf": [{"$ref": "#/$defs/A"}]
            }),
        );
        let line = "t output /oneOf/0/properties/z boolean";
        assert!(lines.iter().any(|l| l == line), "{lines:?}");
    }

    #[test]
    fn any_of_branches_stay_on_the_parent_path() {
        let lines = walked(
            "input",
            serde_json::json!({"type": "object", "properties": {"n": {"anyOf": [
                {"type": "object", "properties": {"m": {"type": "string"}}},
                {"type": "null"}
            ]}}}),
        );
        let line = "t input /properties/n/properties/m string";
        assert!(lines.iter().any(|l| l == line), "{lines:?}");
    }

    #[test]
    fn each_property_is_required_or_optional() {
        // A property that becomes required, or one that becomes optional,
        // removes a line.
        let lines = walked(
            "input",
            serde_json::json!({
                "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
                "required": ["a"]
            }),
        );
        for line in [
            "t input /properties/a required",
            "t input /properties/b optional",
        ] {
            assert!(
                lines.iter().any(|l| l == line),
                "missing `{line}` in {lines:?}"
            );
        }
        assert!(
            !lines
                .iter()
                .any(|l| l == "t input /properties/b required"
                    || l == "t input /properties/a optional"),
            "{lines:?}"
        );
    }

    #[test]
    fn a_required_name_without_a_property_has_a_line() {
        let lines = walked(
            "input",
            serde_json::json!({
                "properties": {"a": {"type": "string"}},
                "required": ["a", "c"]
            }),
        );
        assert!(
            lines.iter().any(|l| l == "t input /properties/c required"),
            "{lines:?}"
        );
    }

    fn walked_text(schema: Value) -> String {
        walked("input", schema)
            .into_iter()
            .map(|l| l + "\n")
            .collect()
    }

    #[test]
    fn each_schema_object_has_its_whole_required_set() {
        let lines = walked(
            "input",
            serde_json::json!({
                "properties": {
                    "b": {"type": "string"},
                    "a": {"type": "object", "properties": {"x": {"type": "string"}}}
                },
                "required": ["b", "a"]
            }),
        );
        for line in [
            "t input  required-set a b",
            "t input /properties/a required-set",
        ] {
            assert!(
                lines.iter().any(|l| l == line),
                "missing `{line}` in {lines:?}"
            );
        }
    }

    #[test]
    fn a_node_with_required_and_no_properties_has_its_required_set() {
        let lines = walked("input", serde_json::json!({"anyOf": [{"required": ["a"]}]}));
        assert!(
            lines.iter().any(|l| l == "t input  required-set a"),
            "{lines:?}"
        );
    }

    #[test]
    fn a_new_required_property_removes_a_line() {
        use crate::freeze::missing_lines;
        let baseline = walked_text(serde_json::json!({
            "properties": {"a": {"type": "string"}},
            "required": ["a"]
        }));
        let head = walked_text(serde_json::json!({
            "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
            "required": ["a", "b"]
        }));
        assert_eq!(
            missing_lines(&baseline, &head),
            vec!["t input  required-set a"]
        );
        // A new optional property only adds lines.
        let head = walked_text(serde_json::json!({
            "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
            "required": ["a"]
        }));
        assert!(missing_lines(&baseline, &head).is_empty(), "{head}");
    }

    #[test]
    fn walk_lists_additional_properties_minimum_and_format() {
        let lines = walked(
            "output",
            serde_json::json!({
                "additionalProperties": true,
                "properties": {"n": {"type": "integer", "minimum": 0, "format": "uint32"}}
            }),
        );
        for line in [
            "t output  additionalProperties true",
            "t output /properties/n minimum 0",
            "t output /properties/n format uint32",
        ] {
            assert!(
                lines.iter().any(|l| l == line),
                "missing `{line}` in {lines:?}"
            );
        }
    }

    #[test]
    fn walk_lists_each_enum_value() {
        let lines = walked(
            "input",
            serde_json::json!({
                "$defs": {"P": {"enum": ["typl", "ridl"], "type": "string"}},
                "properties": {"p": {"$ref": "#/$defs/P"}}
            }),
        );
        for line in [
            "t input /properties/p string",
            "t input /properties/p enum typl",
            "t input /properties/p enum ridl",
        ] {
            assert!(
                lines.iter().any(|l| l == line),
                "missing `{line}` in {lines:?}"
            );
        }
    }

    #[test]
    fn walk_lists_a_const() {
        let lines = walked(
            "output",
            serde_json::json!({
                "properties": {"kind": {"const": "diagnostic", "type": "string"}}
            }),
        );
        assert!(
            lines
                .iter()
                .any(|l| l == "t output /properties/kind const diagnostic"),
            "{lines:?}"
        );
    }

    /// The paths of the `anyOf` nodes under `node` that are not a nullable
    /// wrapper: exactly one `{"type": "null"}` branch and one other branch.
    fn non_nullable_any_ofs(node: &Value, path: &str, found: &mut Vec<String>) {
        match node {
            Value::Object(map) => {
                if let Some(Value::Array(branches)) = map.get("anyOf") {
                    let nulls = branches
                        .iter()
                        .filter(|b| **b == serde_json::json!({"type": "null"}))
                        .count();
                    if branches.len() != 2 || nulls != 1 {
                        found.push(format!("{path}/anyOf"));
                    }
                }
                for (key, value) in map {
                    non_nullable_any_ofs(value, &format!("{path}/{key}"), found);
                }
            }
            Value::Array(items) => {
                for (index, value) in items.iter().enumerate() {
                    non_nullable_any_ofs(value, &format!("{path}/{index}"), found);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn the_any_of_check_flags_a_second_non_null_branch() {
        let schema = serde_json::json!({"properties": {
            "ok": {"anyOf": [{"type": "string"}, {"type": "null"}]},
            "bad": {"anyOf": [{"type": "string"}, {"type": "integer"}]},
            "three": {"anyOf": [{"type": "string"}, {"type": "integer"}, {"type": "null"}]},
        }});
        let mut found = Vec::new();
        non_nullable_any_ofs(&schema, "", &mut found);
        found.sort();
        assert_eq!(
            found,
            vec!["/properties/bad/anyOf", "/properties/three/anyOf"]
        );
    }

    #[test]
    fn every_any_of_in_the_tool_schemas_is_a_nullable_wrapper() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let text = std::fs::read_to_string(root.join("crates/ridl-mcp/tests/tools.json")).unwrap();
        let tools: Value = serde_json::from_str(&text).unwrap();
        let mut found = Vec::new();
        non_nullable_any_ofs(&tools, "", &mut found);
        assert!(
            found.is_empty(),
            "`anyOf` branches share their parent's path, so a second non-null shape needs \
indexing like `oneOf`; these `anyOf` nodes have one: {found:?}"
        );
    }

    #[test]
    fn mcp_dump_lists_every_tool() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lines = dump(root).unwrap();
        let mut names: Vec<&str> = lines
            .iter()
            .map(String::as_str)
            .filter(|l| !l.contains(' '))
            .collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 9, "{names:?}");
        let text = std::fs::read_to_string(root.join("crates/ridl-mcp/tests/tools.json")).unwrap();
        let tools: Value = serde_json::from_str(&text).unwrap();
        let mut expected: Vec<&str> = tools
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        expected.sort();
        assert_eq!(names, expected);
        assert!(
            lines
                .iter()
                .any(|l| l == "ridl_check input /properties/source null|string")
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("ridl_dependencies input ") && l.ends_with(" required"))
        );
        // `ridl_explain`'s output schema is a top-level `oneOf`.
        for line in [
            "ridl_explain output /oneOf/0/properties/kind const diagnostic",
            "ridl_explain output /oneOf/1/properties/kind const diff_category",
            "ridl_explain output /oneOf/1/properties/category string",
            "ridl_check input /properties/profile enum rsdl",
        ] {
            assert!(lines.iter().any(|l| l == line), "missing `{line}`");
        }
    }
}
