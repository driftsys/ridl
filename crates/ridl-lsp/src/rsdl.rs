//! Hover and go-to-definition in an `.rsdl` file (docs/ROADMAP.md epic E6.15,
//! rsdl reference §4).
//!
//! The cursor's reference and its target come from
//! [`ridl_sem::rsdl::reference_at`], which reads both from the checked system
//! model; this module only turns a target into a declaration site and into
//! hover markdown. A component, an instance and the system are declared in
//! `.rsdl` files and render from the model. An interface and a service are
//! declared in ridl and render as a hover on the ridl declaration does.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, package_of, service_catalog};
use ridl_sem::rsdl::{
    CheckedSystem, DeclAttrs, InterfaceId, MemberRef, Target, UNIT_INSTANCE, reference_at,
};
use ridl_sem::{Symbol, SymbolKind, check_package, check_system, resolve_package};
use ridl_syntax::ast::AstNode;
use rowan::{TextRange, TextSize};

use crate::hover::{self, HoverInfo};
use crate::nav;

/// The hover for the rsdl reference at `offset` in `file`, a file of `pkg`.
pub fn hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let system = check_system(db, ws, std);
    let found = reference_at(db, ws, std, &system, file, offset)?;
    let markdown = match &found.target {
        Target::System(index) => system_markdown(&system, *index),
        Target::Component(index) => component_markdown(&system, *index),
        Target::Instance {
            component,
            instance,
        } => instance_markdown(&system, *component, *instance)?,
        Target::Service(name) | Target::Interface(InterfaceId::Inline { service: name }) => {
            service_markdown(db, ws, std, name)?
        }
        Target::Interface(InterfaceId::Declared { package, name }) => {
            let symbol = interface_symbol(db, ws, std, package, name)?;
            hover::symbol_markdown(db, ws, std, pkg, &symbol)
        }
    };
    Some(HoverInfo {
        markdown,
        range: found.range,
    })
}

/// The declaration site of what the rsdl reference at `offset` in `file` names.
pub fn definition(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<(InputFile, TextRange)> {
    let system = check_system(db, ws, std);
    let found = reference_at(db, ws, std, &system, file, offset)?;
    let site = match &found.target {
        Target::System(index) => system.systems[*index].name.site,
        Target::Component(index) => system.components[*index].name.site,
        Target::Instance {
            component,
            instance,
        } => system.components[*component].instances.as_ref()?[*instance].site,
        Target::Service(name) | Target::Interface(InterfaceId::Inline { service: name }) => {
            return service_site(db, ws, std, name);
        }
        Target::Interface(InterfaceId::Declared { package, name }) => {
            let symbol = interface_symbol(db, ws, std, package, name)?;
            return Some((symbol.file, symbol.range));
        }
    };
    Some((site.file, site.range))
}

/// The declared `interface` `name` of `package`, read from that package's
/// resolved names as navigation on a ridl reference reads it.
fn interface_symbol(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    package: &str,
    name: &str,
) -> Option<Symbol> {
    let target = if package == std.name(db) {
        std
    } else {
        package_of(db, ws, package.to_string())?
    };
    resolve_package(db, ws, target, std)
        .symbols
        .get(name)
        .filter(|symbol| symbol.package == package && symbol.kind == SymbolKind::Interface)
        .cloned()
}

/// The declaring package of the service `name`, from the service catalog.
fn service_package(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    name: &str,
) -> Option<Package> {
    let entry = service_catalog(db, ws, std).entries.get(name)?.clone();
    package_of(db, ws, entry.package)
}

/// The dotted name of the `service` declaration the catalog holds for `name`:
/// the first one in file order, as the catalog keeps the first (RIDL-140).
fn service_site(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    name: &str,
) -> Option<(InputFile, TextRange)> {
    let package = service_package(db, ws, std, name)?;
    package.files(db).iter().find_map(|file| {
        nav::source_file(db, *file).services().find_map(|service| {
            let dotted = service.name()?;
            (dotted.text() == name).then(|| (*file, dotted.syntax().text_range()))
        })
    })
}

/// A ridl service as a hover on its own declaration renders it.
fn service_markdown(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    name: &str,
) -> Option<String> {
    let package = service_package(db, ws, std, name)?;
    let ir = check_package(db, ws, package, std).ir;
    let service = ir.services.iter().find(|service| service.name == name)?;
    Some(hover::render_service(service))
}

fn system_markdown(system: &CheckedSystem, index: usize) -> String {
    let decl = &system.systems[index];
    let mut out = format!("```rsdl\nsystem {}.{}\n```", decl.package, decl.name.name);
    push_lines(&mut out, "Members", &decl.members);
    push_attrs(&mut out, &decl.attrs);
    out
}

fn component_markdown(system: &CheckedSystem, index: usize) -> String {
    let decl = &system.components[index];
    let mut out = format!(
        "```rsdl\ncomponent {}.{}\n```",
        decl.package, decl.name.name
    );
    match &decl.instances {
        None => out.push_str(&format!(
            "\n\n**Instances:** the unit instance `{UNIT_INSTANCE}`"
        )),
        Some(instances) if !instances.is_empty() => {
            let names: Vec<String> = instances
                .iter()
                .map(|instance| format!("`{}`", instance.name))
                .collect();
            out.push_str(&format!("\n\n**Instances:** {}", names.join(", ")));
        }
        Some(_) => {}
    }
    push_lines(&mut out, "Offers", &decl.offers);
    push_lines(&mut out, "Requires", &decl.requires);
    if decl.external {
        out.push_str("\n\n**External:** no implementation in this workspace (rsdl §3.2)");
    }
    push_attrs(&mut out, &decl.attrs);
    out
}

fn instance_markdown(system: &CheckedSystem, component: usize, instance: usize) -> Option<String> {
    let decl = &system.components[component];
    let name = &decl.instances.as_ref()?[instance].name;
    Some(format!(
        "```rsdl\n{}.{}.{name}\n```\n\nInstance `{name}` of the component `{}` (rsdl §7)",
        decl.package, decl.name.name, decl.name.name
    ))
}

/// `**Label:** `a`, `b`` over the references of `lines`, as written.
fn push_lines(out: &mut String, label: &str, lines: &[MemberRef]) {
    if lines.is_empty() {
        return;
    }
    let names: Vec<String> = lines
        .iter()
        .map(|line| format!("`{}`", line.reference.text()))
        .collect();
    out.push_str(&format!("\n\n**{label}:** {}", names.join(", ")));
}

/// The family attributes, in the order a ridl declaration's hover shows them.
fn push_attrs(out: &mut String, attrs: &DeclAttrs) {
    if !attrs.labels.is_empty() {
        out.push_str(&format!("\n\n**Labels:** {}", attrs.labels.join(", ")));
    }
    if let Some(reason) = &attrs.deprecated {
        out.push_str(&format!("\n\n**Deprecated:** {reason}"));
    }
}
