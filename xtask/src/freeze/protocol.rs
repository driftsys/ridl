//! The IR and codegen protocol schemas: every message field and enum value,
//! compiled with protox from `crates/ridl-ir/proto/`.

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
/// for a member of a declared oneof) and `<pkg>.<Enum>.<VALUE> = <number>`.
/// The label is `repeated`, `required`, `optional` (explicit presence) or
/// `singular` (implicit presence).
pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let include = root.join("crates/ridl-ir/proto");
    let set = protox::compile(PROTOS, [&include])
        .map_err(|e| format!("cannot compile the protocol schemas: {e}"))?;
    let pool = DescriptorPool::from_file_descriptor_set(set)
        .map_err(|e| format!("cannot build the descriptor pool: {e}"))?;
    let mut lines = Vec::new();
    for message in pool.all_messages() {
        if !PROTOS.contains(&message.parent_file().name()) {
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
    }
    for en in pool.all_enums() {
        if !PROTOS.contains(&en.parent_file().name()) {
            continue;
        }
        for value in en.values() {
            lines.push(format!(
                "{}.{} = {}",
                en.full_name(),
                value.name(),
                value.number()
            ));
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

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
