//! The IR and codegen protocol schemas: every message field and enum value,
//! and the reserved numbers and names, compiled with protox from
//! `crates/ridl-ir/proto/`.

use prost_reflect::{Cardinality, DescriptorPool, Kind};
use std::path::Path;

const PROTOS: [&str; 5] = [
    "ridl/ir/v2/ir.proto",
    "ridl/ir/v2/system.proto",
    "ridl/codegen/v1/model.proto",
    "ridl/codegen/v1/deployment.proto",
    "ridl/codegen/v1/plugin.proto",
];

fn type_name(kind: &Kind) -> String {
    match kind {
        Kind::Double => "double".into(),
        Kind::Float => "float".into(),
        Kind::Int32 => "int32".into(),
        Kind::Int64 => "int64".into(),
        Kind::Uint32 => "uint32".into(),
        Kind::Uint64 => "uint64".into(),
        Kind::Sint32 => "sint32".into(),
        Kind::Sint64 => "sint64".into(),
        Kind::Fixed32 => "fixed32".into(),
        Kind::Fixed64 => "fixed64".into(),
        Kind::Sfixed32 => "sfixed32".into(),
        Kind::Sfixed64 => "sfixed64".into(),
        Kind::Bool => "bool".into(),
        Kind::String => "string".into(),
        Kind::Bytes => "bytes".into(),
        Kind::Message(m) => m.full_name().to_string(),
        Kind::Enum(e) => e.full_name().to_string(),
    }
}

/// `<pkg>.<Message>.<field> = <number> <label> <type>` (with ` oneof=<name>`
/// for a member of a declared oneof) and `<pkg>.<Enum>.<VALUE> = <number>`,
/// for the messages and enums of `files`, one `<pkg>.<Enum> values <names>`
/// line per enum that holds all its value names, sorted, and the reserved
/// numbers and names of each message and enum (see [`reserved_lines`]). A new
/// enum value removes the `values` line: `request_from_json` rejects an enum
/// name it does not know, so a plugin built against the older schema cannot
/// read a request that holds the new value.
/// The label is `repeated`, `required`, `optional` (explicit presence) or
/// `singular` (implicit presence).
fn pool_lines(pool: &DescriptorPool, files: &[&str]) -> Vec<String> {
    let mut lines = Vec::new();
    for message in pool.all_messages() {
        if !files.contains(&message.parent_file().name()) {
            continue;
        }
        for field in message.fields() {
            let label = match field.cardinality() {
                Cardinality::Repeated => "repeated",
                Cardinality::Required => "required",
                Cardinality::Optional if field.field_descriptor_proto().proto3_optional() => {
                    "optional"
                }
                Cardinality::Optional => "singular",
            };
            let mut line = format!(
                "{} = {} {label} {}",
                field.full_name(),
                field.number(),
                type_name(&field.kind())
            );
            if let Some(oneof) = field.containing_oneof().filter(|o| !o.is_synthetic()) {
                line.push_str(&format!(" oneof={}", oneof.name()));
            }
            lines.push(line);
        }
        let ranges = message
            .reserved_ranges()
            .map(|r| (r.start as i64, r.end as i64 - 1));
        reserved_lines(
            message.full_name(),
            ranges,
            message.reserved_names(),
            &mut lines,
        );
    }
    for en in pool.all_enums() {
        if !files.contains(&en.parent_file().name()) {
            continue;
        }
        let ranges = en
            .reserved_ranges()
            .map(|r| (*r.start() as i64, *r.end() as i64));
        reserved_lines(en.full_name(), ranges, en.reserved_names(), &mut lines);
        let mut names: Vec<String> = en.values().map(|v| v.name().to_string()).collect();
        names.sort();
        lines.push(format!("{} values {}", en.full_name(), names.join(" ")));
        for value in en.values() {
            lines.push(format!(
                "{}.{} = {}",
                en.full_name(),
                value.name(),
                value.number()
            ));
        }
    }
    lines
}

/// `<name> reserved <n>` for a reserved number, `<name> reserved-range <a>-<b>`
/// for a reserved range of more than one number (`a` and `b` inclusive), and
/// `<name> reserved-name <field>` for a reserved name. `ranges` holds
/// inclusive bounds.
fn reserved_lines<'a>(
    name: &str,
    ranges: impl Iterator<Item = (i64, i64)>,
    names: impl Iterator<Item = &'a str>,
    lines: &mut Vec<String>,
) {
    for (start, end) in ranges {
        if start == end {
            lines.push(format!("{name} reserved {start}"));
        } else {
            lines.push(format!("{name} reserved-range {start}-{end}"));
        }
    }
    for field in names {
        lines.push(format!("{name} reserved-name {field}"));
    }
}

/// The lines of [`pool_lines`] for the schemas of [`PROTOS`].
pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let include = root.join("crates/ridl-ir/proto");
    let set = protox::compile(PROTOS, [&include])
        .map_err(|e| format!("cannot compile the protocol schemas: {e}"))?;
    let pool = DescriptorPool::from_file_descriptor_set(set)
        .map_err(|e| format!("cannot build the descriptor pool: {e}"))?;
    Ok(pool_lines(&pool, &PROTOS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freeze::missing_lines;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNT: AtomicUsize = AtomicUsize::new(0);

    /// The dump lines of one schema file `t.proto` with the text `proto`.
    fn text(proto: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "xtask-freeze-protocol-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.proto"), proto).unwrap();
        let set = protox::compile(["t.proto"], [&dir]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let pool = DescriptorPool::from_file_descriptor_set(set).unwrap();
        pool_lines(&pool, &["t.proto"])
            .into_iter()
            .map(|l| l + "\n")
            .collect()
    }

    const BASELINE: &str = "syntax = \"proto3\";\npackage p;\nmessage M {\n  reserved 10, 11;\n  reserved 20 to 30;\n  reserved \"old\";\n  uint32 a = 1;\n}\nenum E {\n  E_ZERO = 0;\n  reserved 5;\n}\n";

    #[test]
    fn protocol_dump_lists_reserved_numbers_ranges_and_names() {
        let lines = text(BASELINE);
        for line in [
            "p.M reserved 10\n",
            "p.M reserved 11\n",
            "p.M reserved-range 20-30\n",
            "p.M reserved-name old\n",
            "p.E reserved 5\n",
        ] {
            assert!(lines.contains(line), "missing `{line}` in {lines}");
        }
    }

    #[test]
    fn removing_a_reserved_number_removes_a_line() {
        let head = BASELINE.replace("reserved 10, 11;", "reserved 10;");
        assert_eq!(
            missing_lines(&text(BASELINE), &text(&head)),
            vec!["p.M reserved 11"]
        );
    }

    #[test]
    fn a_new_enum_value_removes_the_value_set_line() {
        // A reader that rejects an unknown enum name cannot read a message
        // that holds the new value.
        let head = BASELINE.replace("E_ZERO = 0;", "E_ZERO = 0;\n  E_ONE = 1;");
        assert_eq!(
            missing_lines(&text(BASELINE), &text(&head)),
            vec!["p.E values E_ZERO"]
        );
        assert!(text(&head).contains("p.E values E_ONE E_ZERO\n"));
    }

    #[test]
    fn protocol_dump_lists_the_package_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lines = dump(root).unwrap();
        assert!(
            lines.iter().any(|l| l.starts_with("ridl.ir.v2.Package.")),
            "{lines:?}"
        );
    }
}
