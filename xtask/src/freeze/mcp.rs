//! The MCP tool surface: the tool names and every property path of each input
//! and output schema, with each `enum` value and `const` of a path, from
//! `crates/ridl-mcp/tests/tools.json`.

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

/// Adds a line for each property of `node`, a `<path> enum <value>` line for
/// each value of its `enum` and a `<path> const <value>` line for its
/// `const`, and recurses into nested `properties`, `items`, `$ref` targets and
/// `anyOf` and `oneOf` branches. `depth` bounds the walk for a recursive
/// `$ref`.
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
    for branch in branches(node) {
        walk(tool, side, pointer, branch, defs, depth + 1, lines);
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
    if let Some(required) = node.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            lines.push(format!(
                "{tool} {side} {pointer}/properties/{} required",
                token(name)
            ));
        }
    }
    if let Some(properties) = node.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            let path = format!("{pointer}/properties/{}", token(name));
            lines.push(format!("{tool} {side} {path} {}", type_of(property, defs)));
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
            "t output /properties/a string",
            "t output /properties/a required",
            "t output /properties/b integer",
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
            "ridl_explain output /properties/kind const diagnostic",
            "ridl_explain output /properties/kind const diff_category",
            "ridl_explain output /properties/category string",
            "ridl_check input /properties/profile enum rsdl",
        ] {
            assert!(lines.iter().any(|l| l == line), "missing `{line}`");
        }
    }
}
