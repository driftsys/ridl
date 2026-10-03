//! Pure declaration queries over a checked workspace.
use crate::snapshot::{Snapshot, ToolError};
use crate::types::{Location, OverlayInput, WorkspaceStatus};
use ridl_ir::v2::{self, decl};
use ridl_sem::Symbol;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct NameInput {
    pub path: String,
    pub overlays: Option<Vec<OverlayInput>>,
    pub name: String,
    pub from: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct InterfaceInput {
    pub path: String,
    pub overlays: Option<Vec<OverlayInput>>,
    pub interface: String,
    pub from: Option<String>,
}
impl From<InterfaceInput> for NameInput {
    fn from(input: InterfaceInput) -> Self {
        Self {
            path: input.path,
            overlays: input.overlays,
            name: input.interface,
            from: input.from,
        }
    }
}

pub struct Found<'a> {
    pub package: String,
    pub item: Item<'a>,
    pub symbol: Option<&'a Symbol>,
    pub alias: Option<String>,
}
#[derive(Clone, Copy)]
pub enum Item<'a> {
    Decl(&'a v2::Decl),
    Interface(&'a v2::Interface),
    Service(&'a v2::Service),
}
impl Item<'_> {
    pub fn name(&self) -> &str {
        match self {
            Self::Decl(d) => &d.name,
            Self::Interface(i) => &i.name,
            Self::Service(s) => &s.name,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ResolveOutput {
    pub name: String,
    pub package: String,
    pub kind: String,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub location: Option<Location>,
    pub workspace: WorkspaceStatus,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DescribeOutput {
    pub declaration: serde_json::Value,
    pub package: String,
    pub location: Option<Location>,
    pub workspace: WorkspaceStatus,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct InterfaceHeader {
    pub name: String,
    pub package: String,
    pub doc: String,
    pub labels: Vec<String>,
    pub deprecated: Option<String>,
    pub number: u32,
    pub provisional: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct InteractionsOutput {
    pub interface: InterfaceHeader,
    pub interactions: Vec<serde_json::Value>,
    pub location: Option<Location>,
    pub workspace: WorkspaceStatus,
}

pub fn packages(snap: &Snapshot) -> String {
    let mut names: Vec<_> = snap
        .output
        .checked
        .iter()
        .map(|c| c.ir.name.as_str())
        .collect();
    names.sort();
    names.dedup();
    names.join(", ")
}
fn declarations(package: &v2::Package) -> impl Iterator<Item = Item<'_>> {
    package.decls.iter().map(Item::Decl).chain(
        package
            .shapes()
            .filter(|shape| !shape.is_inline())
            .map(|shape| Item::Interface(shape.interface)),
    )
}
fn in_package<'a>(snap: &'a Snapshot, package: &str, name: &str) -> Option<Found<'a>> {
    let (ir, resolution) = if package == "ridl.std" {
        (&snap.output.std_ir, None)
    } else {
        let i = snap
            .output
            .checked
            .iter()
            .position(|c| c.ir.name == package)?;
        (
            &snap.output.checked[i].ir,
            Some(&snap.output.resolutions[i]),
        )
    };
    let item = declarations(ir).find(|i| i.name() == name)?;
    let symbol = resolution
        .and_then(|r| r.symbols.get(name))
        .filter(|s| s.package == package);
    Some(Found {
        package: package.into(),
        item,
        symbol,
        alias: None,
    })
}
fn unknown(snap: &Snapshot, name: &str) -> ToolError {
    let mut suggestions: Vec<_> = snap
        .output
        .checked
        .iter()
        .map(|c| &c.ir)
        .chain(std::iter::once(&snap.output.std_ir))
        .flat_map(|p| declarations(p).map(|d| format!("{}.{}", p.name, d.name())))
        .filter(|canonical| canonical.to_lowercase().contains(&name.to_lowercase()))
        .collect();
    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(10);
    ToolError::Request(format!(
        "no declaration `{name}` in this workspace; suggestions: {}",
        suggestions.join(", ")
    ))
}
pub fn find<'a>(
    snap: &'a Snapshot,
    name: &str,
    from: Option<&str>,
) -> Result<Found<'a>, ToolError> {
    if snap.output.checked.is_empty() {
        return Err(ToolError::Request("the workspace has errors that stop it from being checked; run ridl_check on the same path".into()));
    }
    if let Some((package, name_in_package)) = name.rsplit_once('.') {
        return in_package(snap, package, name_in_package).ok_or_else(|| unknown(snap, name));
    }
    if let Some(from) = from {
        let i = snap
            .output
            .checked
            .iter()
            .position(|c| c.ir.name == from)
            .ok_or_else(|| {
                ToolError::Request(format!(
                    "no package `{from}` in this workspace; packages: {}",
                    packages(snap)
                ))
            })?;
        let symbol = snap.output.resolutions[i]
            .symbols
            .get(name)
            .ok_or_else(|| unknown(snap, name))?;
        let mut found = in_package(snap, &symbol.package, &symbol.name).ok_or_else(|| {
            ToolError::Request(format!(
                "package `{}` has no checked IR for `{}`; run ridl_check on the same path",
                symbol.package, symbol.name
            ))
        })?;
        if symbol.name != name {
            found.alias = Some(name.into());
        }
        return Ok(found);
    }
    let mut matches: Vec<_> = snap
        .output
        .checked
        .iter()
        .map(|c| c.ir.name.as_str())
        .chain(std::iter::once("ridl.std"))
        .filter_map(|p| in_package(snap, p, name))
        .collect();
    if matches.len() == 1 {
        return Ok(matches.remove(0));
    }
    if matches.is_empty() {
        return Err(unknown(snap, name));
    }
    let mut names: Vec<_> = matches
        .iter()
        .map(|f| format!("{}.{}", f.package, f.item.name()))
        .collect();
    names.sort();
    Err(ToolError::Request(format!(
        "`{name}` is ambiguous: {}; pass `pkg.Name` or `from` to choose one",
        names.join(", ")
    )))
}
pub fn location(snap: &Snapshot, found: &Found<'_>) -> Option<Location> {
    found.symbol.map(|s| snap.location(s.file, s.range))
}
pub fn resolve(snap: &Snapshot, input: &NameInput) -> Result<ResolveOutput, ToolError> {
    let found = find(snap, &input.name, input.from.as_deref())?;
    let (kind, visibility) = match found.item {
        Item::Interface(i) => ("interface", i.visibility),
        Item::Decl(d) => (
            match d.kind.as_ref() {
                Some(decl::Kind::TypeDef(_)) => "type",
                Some(decl::Kind::ConstDef(_)) => "const",
                Some(decl::Kind::StructDef(_)) => "struct",
                Some(decl::Kind::EnumDef(_)) => "enum",
                Some(decl::Kind::EnumSetDef(_)) => "enumset",
                Some(decl::Kind::UnionDef(_)) => "union",
                _ => {
                    return Err(ToolError::Request(format!(
                        "`{}` has no declaration kind; run ridl_check on the same path",
                        input.name
                    )));
                }
            },
            d.visibility,
        ),
        Item::Service(_) => unreachable!("find does not return services"),
    };
    Ok(ResolveOutput {
        name: found.item.name().into(),
        package: found.package.clone(),
        kind: kind.into(),
        visibility: if visibility == v2::Visibility::Internal as i32 {
            "internal"
        } else {
            "public"
        }
        .into(),
        alias: found.alias.clone(),
        location: location(snap, &found),
        workspace: snap.status(),
    })
}
pub fn describe_type(snap: &Snapshot, input: &NameInput) -> Result<DescribeOutput, ToolError> {
    let found = find(snap, &input.name, input.from.as_deref())?;
    let Item::Decl(decl) = found.item else {
        return Err(ToolError::Request(format!(
            "`{}` is an interface; use ridl_list_interactions",
            input.name
        )));
    };
    Ok(DescribeOutput {
        declaration: serde_json::to_value(decl).expect("IR declarations serialize"),
        package: found.package.clone(),
        location: location(snap, &found),
        workspace: snap.status(),
    })
}
pub fn list_interactions(
    snap: &Snapshot,
    input: &NameInput,
) -> Result<InteractionsOutput, ToolError> {
    let found = find(snap, &input.name, input.from.as_deref())?;
    let Item::Interface(interface) = found.item else {
        return Err(ToolError::Request(format!(
            "`{}` is not an interface; use ridl_describe_type",
            input.name
        )));
    };
    Ok(InteractionsOutput {
        interface: InterfaceHeader {
            name: interface.name.clone(),
            package: found.package.clone(),
            doc: interface.doc.clone(),
            labels: interface.labels.clone(),
            deprecated: interface.deprecated.clone(),
            number: interface.number,
            provisional: interface.provisional,
        },
        interactions: interface
            .interactions
            .iter()
            .map(|d| serde_json::to_value(d).expect("IR declarations serialize"))
            .collect(),
        location: location(snap, &found),
        workspace: snap.status(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{snapshot, tests::fixture};
    fn input(name: &str, from: Option<&str>) -> NameInput {
        NameInput {
            path: fixture("ws"),
            overlays: None,
            name: name.into(),
            from: from.map(str::to_string),
        }
    }
    fn snap() -> Snapshot {
        snapshot(&fixture("ws"), &[]).unwrap()
    }
    fn message(error: ToolError) -> String {
        match error {
            ToolError::Request(message) => message,
            _ => panic!("request error"),
        }
    }
    #[test]
    fn resolve_a_bare_unique_name() {
        let out = resolve(&snap(), &input("Speed", None)).unwrap();
        assert_eq!(out.package, "fx.a");
        assert_eq!(out.kind, "type");
        let location = out.location.unwrap();
        assert!(location.path.ends_with("a/a.ridl"));
        assert_eq!(location.start.line, 4);
    }
    #[test]
    fn resolve_an_ambiguous_bare_name_lists_candidates() {
        let error = message(resolve(&snap(), &input("Level", None)).err().unwrap());
        assert!(error.contains("fx.a.Level"));
        assert!(error.contains("fx.b.Level"));
        assert!(error.contains("pass `pkg.Name` or `from` to choose one"));
    }
    #[test]
    fn resolve_a_bare_name_from_a_package_uses_its_view() {
        assert_eq!(
            resolve(&snap(), &input("Level", Some("fx.b")))
                .unwrap()
                .package,
            "fx.b"
        );
    }
    #[test]
    fn resolve_an_alias() {
        let out = resolve(&snap(), &input("ALevel", Some("fx.b"))).unwrap();
        assert_eq!(out.package, "fx.a");
        assert_eq!(out.name, "Level");
        assert_eq!(out.alias.as_deref(), Some("ALevel"));
    }
    #[test]
    fn resolve_a_canonical_name() {
        assert_eq!(
            resolve(&snap(), &input("fx.a.sub.Gear", None))
                .unwrap()
                .package,
            "fx.a.sub"
        );
    }
    #[test]
    fn resolve_finds_a_std_declaration() {
        for name in ["Version", "Duration"] {
            let out = resolve(&snap(), &input(name, None)).unwrap();
            assert_eq!(out.package, "ridl.std");
            assert!(out.location.is_none());
            assert_eq!(
                describe_type(&snap(), &input(name, None)).unwrap().package,
                "ridl.std"
            );
        }
    }
    #[test]
    fn an_unknown_name_suggests_close_names() {
        assert!(
            message(resolve(&snap(), &input("speed", None)).err().unwrap()).contains("fx.a.Speed")
        );
    }
    #[test]
    fn describe_a_struct() {
        let out = describe_type(&snap(), &input("Reading", None)).unwrap();
        let fields = out.declaration["structDef"]["members"].as_array().unwrap();
        assert_eq!(
            fields
                .iter()
                .map(|m| m["field"]["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["level", "health"]
        );
        assert!(out.declaration.get("doc").is_none_or(|d| d == ""));
    }
    #[test]
    fn describe_keeps_the_doc() {
        assert_eq!(
            describe_type(&snap(), &input("Speed", None))
                .unwrap()
                .declaration["doc"],
            "Vehicle speed in kilometres per hour."
        );
    }
    #[test]
    fn describe_an_interface_is_refused() {
        assert!(
            message(
                describe_type(&snap(), &input("Status", None))
                    .err()
                    .unwrap()
            )
            .contains("ridl_list_interactions")
        );
    }
    #[test]
    fn list_every_interaction_kind() {
        let out = list_interactions(&snap(), &input("Status", None)).unwrap();
        assert_eq!(out.interactions.len(), 5);
        for (i, kind) in [
            "signalDef",
            "eventDef",
            "commandDef",
            "queryDef",
            "fixedDef",
        ]
        .iter()
        .enumerate()
        {
            assert!(out.interactions[i].get(*kind).is_some());
            assert_eq!(out.interactions[i]["ordinal"], i + 1);
        }
    }
}
