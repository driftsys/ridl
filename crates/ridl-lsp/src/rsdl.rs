//! Hover and go-to-definition in an `.rsdl` file (rsdl reference §4).
//!
//! The cursor's reference and its target come from
//! [`ridl_sem::rsdl::reference_at`], which reads both from the checked system
//! model; this module only turns a target into a declaration site and into
//! hover markdown. A component, an instance and the system are declared in
//! `.rsdl` files and render from the model. An interface and a service are
//! declared in ridl and render as a hover on the ridl declaration does.
//!
//! The name of an rsdl declaration — a system, a component, a distribution, a
//! deployment or a machine — has a hover too, and every rsdl hover shows the
//! declaration's doc after its facts (ADR-0026). A body line with a doc adds
//! that doc below the hover of the declaration the line names.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, package_of, service_catalog};
use ridl_ir::v2;
use ridl_sem::docs::DocInfo;
use ridl_sem::rsdl::{
    CheckedSystem, DeclAttrs, InterfaceId, MemberRef, Target, UNIT_INSTANCE, reference_at,
};
use ridl_sem::{Symbol, SymbolKind, check_package, check_system, resolve_package};
use ridl_syntax::ast::AstNode;
use rowan::{TextRange, TextSize};

use crate::hover::{self, DocParts, HoverInfo};
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
    let resolve = |target: &str| hover::link_location(db, ws, std, pkg, target);
    let Some(found) = reference_at(db, ws, std, &system, file, offset) else {
        return declaration_hover(&system, file, offset, &resolve);
    };
    let mut markdown = match &found.target {
        Target::System(index) => system_markdown(&system, *index, &resolve),
        Target::Component(index) => component_markdown(&system, *index, &resolve),
        Target::Instance {
            component,
            instance,
        } => instance_markdown(&system, *component, *instance)?,
        Target::Service(name) | Target::Interface(InterfaceId::Inline { service: name }) => {
            service_markdown(db, ws, std, name, &resolve)?
        }
        Target::Interface(InterfaceId::Declared { package, name }) => {
            let symbol = interface_symbol(db, ws, std, package, name)?;
            hover::symbol_markdown(db, ws, std, pkg, &symbol)
        }
    };
    if let Some(line) = line_at(&system, file, offset)
        && !line.doc.doc.trim().is_empty()
    {
        markdown.push_str("\n\n---\n\n**This line:**");
        hover::push_doc_sections(&mut markdown, &line_parts(line), &[], &resolve);
    }
    Some(HoverInfo {
        markdown,
        range: found.range,
    })
}

/// The hover for the name of an rsdl declaration at `offset` in `file`.
fn declaration_hover(
    system: &CheckedSystem,
    file: InputFile,
    offset: TextSize,
    resolve: &dyn Fn(&str) -> Option<lsp_types::Location>,
) -> Option<HoverInfo> {
    let at =
        |site: &ridl_sem::rsdl::Site| site.file == file && site.range.contains_inclusive(offset);
    let (markdown, range) =
        if let Some(index) = system.systems.iter().position(|d| at(&d.name.site)) {
            (
                system_markdown(system, index, resolve),
                system.systems[index].name.site.range,
            )
        } else if let Some(index) = system.components.iter().position(|d| at(&d.name.site)) {
            (
                component_markdown(system, index, resolve),
                system.components[index].name.site.range,
            )
        } else if let Some(decl) = system.distributions.iter().find(|d| at(&d.name.site)) {
            let mut out = format!(
                "```rsdl\ndistribution {}.{}\n```",
                decl.package, decl.name.name
            );
            push_lines(&mut out, "Members", &decl.members);
            push_docs(&mut out, &decl.doc, &decl.links, &decl.attrs, resolve);
            (out, decl.name.site.range)
        } else if let Some(decl) = system.deployments.iter().find(|d| at(&d.name.site)) {
            let target = decl
                .system
                .as_ref()
                .map(|reference| format!(" for {}", reference.text()))
                .unwrap_or_default();
            let mut out = format!(
                "```rsdl\ndeployment {}.{}{target}\n```",
                decl.package, decl.name.name
            );
            let machines: Vec<String> = decl
                .machines
                .iter()
                .map(|machine| format!("`{}`", machine.name.name))
                .collect();
            if !machines.is_empty() {
                out.push_str(&format!("\n\n**Machines:** {}", machines.join(", ")));
            }
            push_docs(&mut out, &decl.doc, &decl.links, &decl.attrs, resolve);
            (out, decl.name.site.range)
        } else {
            let (deployment, machine) = system.deployments.iter().find_map(|deployment| {
                let machine = deployment.machines.iter().find(|m| at(&m.name.site))?;
                Some((deployment, machine))
            })?;
            let mut out = format!(
                "```rsdl\nmachine {}.{}.{}\n```",
                deployment.package, deployment.name.name, machine.name.name
            );
            push_lines(&mut out, "Members", &machine.members);
            if machine.external {
                out.push_str("\n\n**External:** no implementation in this workspace (rsdl §3.5)");
            }
            push_docs(
                &mut out,
                &machine.doc,
                &machine.links,
                &machine.attrs,
                resolve,
            );
            (out, machine.name.site.range)
        };
    Some(HoverInfo { markdown, range })
}

/// The body line whose reference is at `offset` in `file`, in any rsdl
/// declaration.
fn line_at(system: &CheckedSystem, file: InputFile, offset: TextSize) -> Option<&MemberRef> {
    let systems = system.systems.iter().flat_map(|d| &d.members);
    let components = system
        .components
        .iter()
        .flat_map(|d| d.offers.iter().chain(&d.requires));
    let distributions = system.distributions.iter().flat_map(|d| &d.members);
    let machines = system
        .deployments
        .iter()
        .flat_map(|d| &d.machines)
        .flat_map(|m| &m.members);
    systems
        .chain(components)
        .chain(distributions)
        .chain(machines)
        .find(|line| {
            line.reference.site.file == file && line.reference.site.range.contains_inclusive(offset)
        })
}

/// The doc fields of a body line; a line takes no labels or deprecation.
fn line_parts(line: &MemberRef) -> DocParts<'_> {
    DocParts {
        doc: &line.doc.doc,
        links: &line.links,
        since: &line.doc.since,
        ..DocParts::default()
    }
}

/// Appends the doc sections of an rsdl declaration: its doc and `@since`,
/// then the family attributes `labels` and `deprecated`, in the order a ridl
/// declaration's hover shows them.
fn push_docs(
    out: &mut String,
    doc: &DocInfo,
    links: &[v2::DocLink],
    attrs: &DeclAttrs,
    resolve: &dyn Fn(&str) -> Option<lsp_types::Location>,
) {
    let parts = DocParts {
        doc: &doc.doc,
        links,
        since: &doc.since,
        labels: &attrs.labels,
        deprecated: attrs.deprecated.as_deref(),
    };
    hover::push_doc_sections(out, &parts, &[], resolve);
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
    resolve: &dyn Fn(&str) -> Option<lsp_types::Location>,
) -> Option<String> {
    let package = service_package(db, ws, std, name)?;
    let ir = check_package(db, ws, package, std).ir;
    let service = ir.services.iter().find(|service| service.name == name)?;
    Some(hover::render_service(service, resolve))
}

fn system_markdown(
    system: &CheckedSystem,
    index: usize,
    resolve: &dyn Fn(&str) -> Option<lsp_types::Location>,
) -> String {
    let decl = &system.systems[index];
    let mut out = format!("```rsdl\nsystem {}.{}\n```", decl.package, decl.name.name);
    push_lines(&mut out, "Members", &decl.members);
    push_docs(&mut out, &decl.doc, &decl.links, &decl.attrs, resolve);
    out
}

fn component_markdown(
    system: &CheckedSystem,
    index: usize,
    resolve: &dyn Fn(&str) -> Option<lsp_types::Location>,
) -> String {
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
    push_docs(&mut out, &decl.doc, &decl.links, &decl.attrs, resolve);
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
