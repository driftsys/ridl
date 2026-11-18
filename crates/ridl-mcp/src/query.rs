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
    /// The workspace root, a package directory or a source file, relative to the server's working directory unless absolute.
    pub path: String,
    /// Optional unsaved source files to apply without writing them to disk.
    pub overlays: Option<Vec<OverlayInput>>,
    /// The declaration name as Name or pkg.Name.
    pub name: String,
    /// The package name in which a bare name resolves, including import aliases.
    pub from: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct InterfaceInput {
    /// The workspace root, a package directory or a source file, relative to the server's working directory unless absolute.
    pub path: String,
    /// Optional unsaved source files to apply without writing them to disk.
    pub overlays: Option<Vec<OverlayInput>>,
    /// The interface name as Name or pkg.Name.
    pub interface: String,
    /// The package name in which a bare name resolves, including import aliases.
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
    pub fn canonical_name(&self, package: &str) -> String {
        match self {
            Self::Service(service) => service.name.clone(),
            _ => format!("{package}.{}", self.name()),
        }
    }
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
    package
        .decls
        .iter()
        .map(Item::Decl)
        .chain(
            package
                .shapes()
                .filter(|shape| !shape.is_inline())
                .map(|shape| Item::Interface(shape.interface)),
        )
        .chain(package.services.iter().map(Item::Service))
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
    let item = declarations(ir).find(|item| match item {
        Item::Service(service) => service.name == format!("{package}.{name}"),
        _ => item.name() == name,
    })?;
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
        .flat_map(|p| declarations(p).map(|d| d.canonical_name(&p.name)))
        .filter(|canonical| canonical.to_lowercase().contains(&name.to_lowercase()))
        .collect();
    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(10);
    let mut message = format!("no declaration `{name}` in this workspace");
    if !suggestions.is_empty() {
        message.push_str(&format!("; suggestions: {}", suggestions.join(", ")));
    }
    ToolError::Request(message)
}
pub fn find<'a>(
    snap: &'a Snapshot,
    name: &str,
    from: Option<&str>,
) -> Result<Found<'a>, ToolError> {
    if snap.output.checked.is_empty()
        && snap
            .output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ridl_core::Severity::Error)
    {
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
        .map(|f| f.item.canonical_name(&f.package))
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
        Item::Service(_) => ("service", v2::Visibility::Public as i32),
    };
    Ok(ResolveOutput {
        name: found.symbol.map_or_else(
            || found.item.name().to_string(),
            |symbol| symbol.name.clone(),
        ),
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
            "`{}` is {}; use ridl_list_interactions",
            input.name,
            if matches!(found.item, Item::Service(_)) {
                "a service"
            } else {
                "an interface"
            }
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
    let interface = match found.item {
        Item::Interface(interface) => interface,
        Item::Service(service) => {
            if let Some(interface) = service.shapes.iter().find_map(|shape| match &shape.kind {
                Some(v2::service_shape::Kind::Inline(interface)) => Some(interface),
                _ => None,
            }) {
                interface
            } else {
                let names: Vec<_> = service
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.kind {
                        Some(v2::service_shape::Kind::InterfaceRef(name)) => {
                            Some(if name.contains('.') {
                                name.clone()
                            } else {
                                format!("{}.{}", found.package, name)
                            })
                        }
                        _ => None,
                    })
                    .collect();
                return Err(ToolError::Request(format!(
                    "`{}` is a list-form service; use ridl_list_interactions on its interfaces: {}",
                    input.name,
                    names.join(", ")
                )));
            }
        }
        Item::Decl(_) => {
            return Err(ToolError::Request(format!(
                "`{}` is not an interface; use ridl_describe_type",
                input.name
            )));
        }
    };
    let (doc, labels, deprecated) = match found.item {
        Item::Service(service) => (&service.doc, &service.labels, &service.deprecated),
        _ => (&interface.doc, &interface.labels, &interface.deprecated),
    };
    Ok(InteractionsOutput {
        interface: InterfaceHeader {
            name: found.item.name().to_string(),
            package: found.package.clone(),
            doc: doc.clone(),
            labels: labels.clone(),
            deprecated: deprecated.clone(),
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
    fn service_aliases_and_ambiguity_preserve_canonical_names() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        std::fs::write(
            copy.0.join("ridl.toml"),
            "[workspace]\nmembers = [\"a\", \"b\", \"c\"]\n",
        )
        .unwrap();
        std::fs::create_dir(copy.0.join("c")).unwrap();
        std::fs::write(
            copy.0.join("c/ridl.toml"),
            "[package]\nname = \"fx.c\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        std::fs::write(
            copy.0.join("c/c.ridl"),
            "package fx.c\nimport fx.b.Diag as FaultAccess\nservice Diag {}\n",
        )
        .unwrap();
        let snap = snapshot(copy.0.to_str().unwrap(), &[]).unwrap();
        assert_eq!(snap.status().errors, 0);
        let out = resolve(&snap, &input("FaultAccess", Some("fx.c"))).unwrap();
        assert_eq!(out.name, "Diag");
        assert_eq!(out.alias.as_deref(), Some("FaultAccess"));
        assert_eq!(out.package, "fx.b");
        let interactions = list_interactions(&snap, &input("FaultAccess", Some("fx.c"))).unwrap();
        assert_eq!(interactions.interactions[0]["name"], "readSpeed");
        let error = message(resolve(&snap, &input("Diag", None)).unwrap_err());
        assert_eq!(
            error,
            "`Diag` is ambiguous: fx.b.Diag, fx.c.Diag; pass `pkg.Name` or `from` to choose one"
        );
    }

    #[test]
    fn list_service_errors_name_interfaces_in_the_declaring_package() {
        let path = format!("{}/a/a.ridl", fixture("ws"));
        let source = std::fs::read_to_string(&path).unwrap() + "\ninterface Status {}\n";
        let snap = snapshot(
            &fixture("ws"),
            &[crate::types::OverlayInput { path, source }],
        )
        .unwrap();
        assert_eq!(snap.status().errors, 0);
        let error = message(
            list_interactions(&snap, &input("fx.b.StatusService", Some("fx.a"))).unwrap_err(),
        );
        assert!(error.ends_with("interfaces: fx.b.Status"), "{error}");
        let suggested = error.rsplit_once("interfaces: ").unwrap().1;
        let out = list_interactions(&snap, &input(suggested, Some("fx.a"))).unwrap();
        assert_eq!(out.interface.package, "fx.b");
        assert_eq!(out.interactions.len(), 5);
    }

    #[test]
    fn inline_service_header_keeps_service_documentation() {
        let path = format!("{}/b/b.ridl", fixture("ws"));
        let source = std::fs::read_to_string(&path).unwrap().replace("service Diag", "/// Diagnostic access.\n/// @labels PRIVATE\n/// @deprecated \"Use the replacement.\"\nservice Diag");
        let snap = snapshot(
            &fixture("ws"),
            &[crate::types::OverlayInput { path, source }],
        )
        .unwrap();
        assert_eq!(snap.status().errors, 0);
        let header = list_interactions(&snap, &input("Diag", None))
            .unwrap()
            .interface;
        assert_eq!(header.doc, "Diagnostic access.");
        assert_eq!(header.labels, ["PRIVATE"]);
        assert_eq!(header.deprecated.as_deref(), Some("Use the replacement."));
        assert!(header.number > 0);
        assert!(header.provisional);
    }

    #[test]
    fn services_resolve_by_declared_name_and_package_join() {
        let snap = snap();
        for (name, from) in [
            ("StatusService", None),
            ("fx.b.StatusService", None),
            ("StatusService", Some("fx.b")),
            ("Diag", Some("fx.b")),
        ] {
            let out = resolve(&snap, &input(name, from)).unwrap();
            assert_eq!(out.kind, "service");
            assert_eq!(out.package, "fx.b");
            assert_eq!(out.visibility, "public");
            let declared = if name == "Diag" {
                "Diag"
            } else {
                "StatusService"
            };
            assert_eq!(out.name, declared);
            assert_eq!(
                serde_json::to_value(out.location).unwrap(),
                crate::snapshot::tests::name_location(
                    "b/b.ridl",
                    &format!("service {declared}"),
                    declared
                )
            );
        }
        let error = message(resolve(&snap, &input("statusservice", None)).unwrap_err());
        assert!(error.contains("fx.b.StatusService"), "{error}");
        assert!(!error.contains("fx.b.fx.b"), "{error}");
    }

    #[test]
    fn inline_service_lists_interactions_and_list_service_names_its_interfaces() {
        let snap = snap();
        for (name, from) in [("fx.b.Diag", None), ("Diag", Some("fx.b"))] {
            let out = list_interactions(&snap, &input(name, from)).unwrap();
            let header = &out.interface;
            assert_eq!(header.name, "fx.b.Diag");
            assert_eq!(header.package, "fx.b");
            assert_eq!(out.interactions.len(), 1);
            assert_eq!(out.interactions[0]["name"], "readSpeed");
            assert!(out.interactions[0].get("queryDef").is_some());
            assert!(out.location.is_some());
        }
        let error = message(list_interactions(&snap, &input("StatusService", None)).unwrap_err());
        assert!(error.contains("list-form service"), "{error}");
        assert!(error.contains("Status"), "{error}");
        for name in ["StatusService", "Diag"] {
            let error = message(describe_type(&snap, &input(name, None)).unwrap_err());
            assert!(error.contains("service"), "{error}");
            assert!(error.contains("ridl_list_interactions"), "{error}");
        }
    }

    #[test]
    fn an_unknown_name_with_no_close_match() {
        let error = resolve(&snap(), &input("Zzzqqq", None)).err().unwrap();
        assert_eq!(message(error), "no declaration `Zzzqqq` in this workspace");
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
    fn a_clean_empty_package_can_resolve_standard_declarations() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        std::fs::remove_dir_all(copy.0.join("a")).unwrap();
        std::fs::remove_dir_all(copy.0.join("b")).unwrap();
        std::fs::write(
            copy.0.join("ridl.toml"),
            "[package]\nname = \"empty\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        let snap = snapshot(copy.0.to_str().unwrap(), &[]).unwrap();
        assert!(snap.output.checked.is_empty());
        assert_eq!(snap.status().errors, 0);
        for name in ["Version", "ridl.std.Version", "Duration"] {
            let out = resolve(&snap, &input(name, None)).unwrap();
            assert_eq!(out.package, "ridl.std");
            assert!(out.location.is_none());
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
        let snapshot = snap();
        let out = describe_type(&snapshot, &input("Reading", None)).unwrap();
        let declaration = snapshot
            .output
            .checked
            .iter()
            .find(|p| p.ir.name == "fx.a")
            .unwrap()
            .ir
            .decls
            .iter()
            .find(|d| d.name == "Reading")
            .unwrap();
        assert_eq!(out.declaration, serde_json::to_value(declaration).unwrap());
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
        let snapshot = snap();
        let out = list_interactions(&snapshot, &input("Status", None)).unwrap();
        let package = &snapshot
            .output
            .checked
            .iter()
            .find(|p| p.ir.name == "fx.b")
            .unwrap()
            .ir;
        let shape = package
            .shapes()
            .find(|shape| shape.name == "Status")
            .unwrap();
        assert_eq!(
            serde_json::to_value(&out.interactions).unwrap(),
            serde_json::to_value(&shape.interface.interactions).unwrap()
        );
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

    #[test]
    fn resolve_reports_visibility() {
        assert_eq!(
            resolve(&snap(), &input("PrivateLevel", None))
                .unwrap()
                .visibility,
            "internal"
        );
        assert_eq!(
            resolve(&snap(), &input("Speed", None)).unwrap().visibility,
            "public"
        );
    }
    #[test]
    fn resolve_reports_each_kind() {
        for (name, kind) in [
            ("Reading", "struct"),
            ("Health", "enum"),
            ("Outcome", "union"),
            ("Status", "interface"),
            ("HEALTH_PATTERN", "const"),
            ("HealthSet", "enumset"),
        ] {
            assert_eq!(
                resolve(&snap(), &input(name, None)).unwrap().kind,
                kind,
                "{name}"
            );
        }
    }
    #[test]
    fn list_interactions_reports_the_header() {
        let out = list_interactions(&snap(), &input("Status", None)).unwrap();
        let header = out.interface;
        assert_eq!(header.name, "Status");
        assert_eq!(header.package, "fx.b");
        assert_eq!(header.doc, "The status interface.");
        assert_eq!(header.labels, ["PRIVATE"]);
        assert_eq!(
            header.deprecated.as_deref(),
            Some("Use the next status interface.")
        );
        assert_eq!(header.number, 1);
        assert!(header.provisional);
    }
    #[test]
    fn locations_are_complete() {
        let out = resolve(&snap(), &input("Speed", None)).unwrap();
        assert_eq!(
            serde_json::to_value(out.location).unwrap(),
            crate::snapshot::tests::name_location("a/a.ridl", "type Speed", "Speed")
        );
    }
    /// The location comes from the source map's line index, which counts
    /// lines by byte offset and columns by character. A CRLF file with a
    /// multi-byte character on an earlier line pins both: a byte-counted
    /// column or a CRLF pair counted as two line breaks moves the result.
    #[test]
    fn locations_count_lines_and_columns_in_a_crlf_file_with_multibyte_text() {
        let path = format!("{}/a/a.ridl", fixture("ws"));
        let disk = std::fs::read_to_string(&path).unwrap();
        let source = disk
            .replace("kilometres per hour", "kilom\u{e8}tres \u{e0} l'heure")
            .replace('\n', "\r\n");
        let snap = snapshot(
            &fixture("ws"),
            &[crate::types::OverlayInput {
                path: path.clone(),
                source,
            }],
        )
        .unwrap();
        let out = resolve(&snap, &input("Speed", None)).unwrap();
        assert_eq!(
            serde_json::to_value(out.location).unwrap(),
            serde_json::json!({"path": path,
                "start": {"line": 4, "column": 6},
                "end": {"line": 4, "column": 11}})
        );
    }
    #[test]
    fn no_alias_without_an_alias() {
        assert!(
            resolve(&snap(), &input("Level", Some("fx.b")))
                .unwrap()
                .alias
                .is_none()
        );
    }
}
